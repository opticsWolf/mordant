//! Fenced div extension for mordant.
//!
//! Pandoc-style fenced divs:
//!
//! ```markdown
//! ::: note
//! A note with **markup**.
//!
//! :::: outer
//! ::: inner
//! nested
//! :::
//! ::::
//! ```
//!
//! Class names can be given after the fence (`::: note warning`) or as an
//! attribute list (`::: {.note #id data-x="y"}`). Following Pandoc, a closing
//! fence uses at least as many colons as the opening fence; optional trailing
//! attributes on the closing fence are ignored.
//!
//! The body is parsed as child blocks: a line with 0–3 leading spaces is a
//! regular block (including a nested div), while 4+ spaces start an indented
//! code block inside the div.

use core::any::TypeId;
use core::fmt;
use std::fmt::Write;

use crate::ast::{
    pp_indent, Arena, KindData, NodeKind, NodeRef, NodeType, PrettyPrint, WalkStatus,
};
use crate::parser::{
    self, AnyBlockParser, BlockParser, NoParserOptions, Parser, ParserExtension, ParserExtensionFn,
    PRIORITY_FENCED_CODE_BLOCK,
};
use crate::renderer::html::{self, Renderer, RendererExtension, RendererExtensionFn};
use crate::renderer::{
    self, BoxRenderNode, NodeRenderer, NodeRendererRegistry, RenderNode, RendererOptions, TextWrite,
};
use crate::text::{self, Reader};
use crate::util::indent_width;
use crate::Result;

/// A fenced div node.
#[derive(Debug, Clone, Copy)]
pub struct FencedDiv {
    fence_length: usize,
}

impl FencedDiv {
    /// Creates a fenced div node with the given fence length.
    pub fn new(fence_length: usize) -> Self {
        Self { fence_length }
    }

    /// The number of colons used by the opening fence.
    pub fn fence_length(&self) -> usize {
        self.fence_length
    }
}

impl NodeKind for FencedDiv {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "FencedDiv"
    }
}

impl PrettyPrint for FencedDiv {
    fn pretty_print(&self, w: &mut dyn Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}FenceLength: {}", pp_indent(level), self.fence_length)
    }
}

impl From<FencedDiv> for KindData {
    fn from(n: FencedDiv) -> Self {
        KindData::Extension(Box::new(n))
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct FencedDivParser;

/// Returns the meaningful part of a line with surrounding spaces and the
/// line ending removed.
fn trim_line_end(line: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = line.len();
    while start < end && matches!(line[start], b' ' | b'\t') {
        start += 1;
    }
    while end > start && matches!(line[end - 1], b' ' | b'\t' | b'\r' | b'\n') {
        end -= 1;
    }
    &line[start..end]
}

/// Returns true if `line` closes a div opened with `fence_length` colons.
fn is_closing_fence(line: &[u8], current_pos: usize, fence_length: usize) -> bool {
    let (width, pos) = indent_width(line, current_pos);
    if width >= 4 {
        return false;
    }
    let mut i = pos;
    while i < line.len() && line[i] == b':' {
        i += 1;
    }
    if i - pos < fence_length {
        return false;
    }
    let rest = trim_line_end(&line[i..]);
    rest.is_empty() || (rest.first() == Some(&b'{') && rest.last() == Some(&b'}'))
}

impl BlockParser for FencedDivParser {
    fn trigger(&self) -> &[u8] {
        b":"
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BasicReader,
        ctx: &mut parser::Context,
    ) -> Option<(NodeRef, parser::State)> {
        let (line, segment) = reader.peek_line_bytes()?;
        let pos = ctx.block_offset()?;

        // A closing fence that reaches `open_blocks` (e.g. after a child
        // paragraph ended) must close the enclosing div instead of opening a
        // new one.
        if let Some(last) = ctx.last_opened_block() {
            let mut ancestor = Some(last);
            while let Some(node_ref) = ancestor {
                if let KindData::Extension(data) = arena[node_ref].kind_data() {
                    if let Some(div) =
                        (data.as_ref() as &dyn core::any::Any).downcast_ref::<FencedDiv>()
                    {
                        if is_closing_fence(line.as_ref(), pos, div.fence_length()) {
                            reader.advance_to_eol();
                            return None;
                        }
                    }
                }
                ancestor = arena[node_ref].parent();
            }
        }

        let mut i = pos;
        while i < line.len() && line[i] == b':' {
            i += 1;
        }
        let fence_length = i - pos;
        if fence_length < 3 {
            return None;
        }

        let node_ref = arena.new_node(FencedDiv::new(fence_length));
        // Let the parser assign the node position; doing it here with a
        // padding subtraction underflows for lines starting with a tab.

        let mut rest_pos = i;
        while rest_pos < line.len() && (line[rest_pos] == b' ' || line[rest_pos] == b'\t') {
            rest_pos += 1;
        }
        let rest_end = line
            .iter()
            .rposition(|b| !(*b == b' ' || *b == b'\t' || *b == b'\n' || *b == b'\r'))
            .map_or(rest_pos, |p| p + 1);
        if rest_end > rest_pos {
            let (line_number, _) = reader.position();
            let line_start = segment.start().saturating_sub(segment.padding());
            let rest_segment = text::Segment::new(line_start + rest_pos, line_start + rest_end);
            reader.set_position(line_number, rest_segment);
            if rest_segment.bytes(reader.source()).first() == Some(&b'{') {
                if let Some(attrs) = parser::parse_attributes(reader) {
                    *arena[node_ref].attributes_mut() = attrs;
                }
            } else {
                let classes = String::from_utf8_lossy(&rest_segment.bytes(reader.source()))
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if !classes.is_empty() {
                    arena[node_ref]
                        .attributes_mut()
                        .insert("class", classes.into());
                }
            }
        }
        reader.advance_to_eol();
        Some((node_ref, parser::State::HAS_CHILDREN))
    }

    fn cont(
        &self,
        _arena: &mut Arena,
        _node_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut parser::Context,
    ) -> Option<parser::State> {
        // Pure container semantics: never consume a line here. Closing fences
        // are recognized in `open`, which sees the enclosing block stack and
        // can therefore close the innermost matching div.
        reader.peek_line_bytes()?;
        Some(parser::State::HAS_CHILDREN)
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
}

impl From<FencedDivParser> for AnyBlockParser {
    fn from(p: FencedDivParser) -> Self {
        AnyBlockParser::Extension(Box::new(p))
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Options for the fenced div HTML renderer.
///
/// Div attributes are filtered with `AttributeFilters::paragraph()`; there
/// is no dedicated `div` filter yet. Custom paragraph filters therefore also
/// affect fenced divs.
#[derive(Debug, Clone, Copy, Default)]
pub struct FencedDivHtmlRendererOptions;

impl RendererOptions for FencedDivHtmlRendererOptions {}

struct FencedDivHtmlRenderer<W: TextWrite> {
    writer: html::Writer,
    options: html::Options,
    _phantom: core::marker::PhantomData<W>,
}

impl<W: TextWrite> FencedDivHtmlRenderer<W> {
    fn new(opts: html::Options, _options: FencedDivHtmlRendererOptions) -> Self {
        Self {
            writer: html::Writer::with_options(opts.clone()),
            options: opts,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for FencedDivHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _ctx: &mut renderer::Context,
    ) -> Result<WalkStatus> {
        if entering {
            self.writer.write_safe_str(w, "<div")?;
            let filter = self
                .options
                .attribute_filters
                .as_ref()
                .and_then(|f| f.paragraph());
            html::render_attributes_safe(
                w,
                source,
                arena[node_ref].attributes(),
                filter,
                self.options.allows_unsafe,
            )?;
            self.writer.write_safe_str(w, ">\n")?;
        } else {
            self.writer.write_safe_str(w, "</div>\n")?;
        }
        Ok(WalkStatus::Continue)
    }
}

impl<'cb, W> NodeRenderer<'cb, W> for FencedDivHtmlRenderer<W>
where
    W: TextWrite + 'cb,
{
    fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'cb, W>) {
        nrr.register_node_renderer_fn(TypeId::of::<FencedDiv>(), BoxRenderNode::new(self));
    }
}

// ---------------------------------------------------------------------------
// Extension factories
// ---------------------------------------------------------------------------

/// Returns a parser extension that recognizes fenced divs.
pub fn fenced_div_parser_extension() -> impl ParserExtension {
    ParserExtensionFn::new(|p: &mut Parser| {
        p.add_block_parser(
            || FencedDivParser,
            NoParserOptions,
            PRIORITY_FENCED_CODE_BLOCK + 10,
        );
    })
}

/// Returns an HTML renderer extension for fenced divs.
pub fn fenced_div_html_renderer_extension<'cb, W>(
    options: impl Into<FencedDivHtmlRendererOptions>,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    RendererExtensionFn::new(move |r: &mut Renderer<'cb, W>| {
        r.add_node_renderer(FencedDivHtmlRenderer::new, options.into());
    })
}

#[cfg(test)]
mod tests {
    use crate::extensions::{Engine, Engines};
    use crate::{markdown_to_html_string_with_options, parser, renderer::html};

    fn render(source: &str) -> String {
        let engines = Engines::only(&[Engine::FencedDiv]);
        let mut output = String::new();
        let mut parser_opts = parser::Options::default();
        parser_opts.engines = engines;
        let mut html_opts = html::Options::default();
        html_opts.engines = engines;
        markdown_to_html_string_with_options(&mut output, source, parser_opts, html_opts).unwrap();
        output
    }

    #[test]
    fn renders_classes_and_content() {
        let html = render("::: note warning\nHello *world*.\n:::\n");
        assert!(html.contains("<div class=\"note warning\">"), "{html}");
        assert!(html.contains("<em>world</em>"), "{html}");
        assert!(html.contains("</div>"), "{html}");
    }

    #[test]
    fn renders_attribute_lists_and_nested_divs() {
        let html = render("::: {.outer #top}\n::: inner\nnested\n:::\n:::\n");
        assert!(html.contains("id=\"top\""), "{html}");
        assert!(html.contains("class=\"outer\""), "{html}");
        assert!(html.matches("<div").count() >= 2, "{html}");
    }
}
