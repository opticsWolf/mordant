//! Definition list extension for mordant.
//!
//! Implements PHP Markdown Extra style definition lists:
//!
//! ```markdown
//! Apple
//! : A fruit.
//!
//! Orange
//! : Citrus fruit.
//! : Another definition.
//! ```
//!
//! A term line followed by one or more definition lines starting with `:`
//! (with up to three leading spaces and whitespace after the colon) becomes a
//! `<dl>` with `<dt>` and `<dd>` children. The term must be the first line of
//! the paragraph, and definitions are limited to a single paragraph;
//! multi-paragraph/block definitions are out of scope.

use core::any::TypeId;
use core::fmt;
use std::fmt::Write;

use crate::ast::{Arena, KindData, NodeKind, NodeRef, NodeType, PrettyPrint, WalkStatus};
use crate::parser::{
    self, AnyParagraphTransformer, NoParserOptions, ParagraphTransformer, Parser, ParserExtension,
    ParserExtensionFn,
};
use crate::renderer::html::{self, Renderer, RendererExtension, RendererExtensionFn};
use crate::renderer::{
    self, BoxRenderNode, NodeRenderer, NodeRendererRegistry, RenderNode, RendererOptions, TextWrite,
};
use crate::text::{self, Reader, Segment};
use crate::util::is_blank;
use crate::{as_type_data_mut, Result};

/// A definition list (`<dl>`).
#[derive(Debug, Clone, Copy, Default)]
pub struct DefinitionList;

impl NodeKind for DefinitionList {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "DefinitionList"
    }
}

impl PrettyPrint for DefinitionList {
    fn pretty_print(&self, _w: &mut dyn Write, _source: &str, _level: usize) -> fmt::Result {
        Ok(())
    }
}

impl From<DefinitionList> for KindData {
    fn from(n: DefinitionList) -> Self {
        KindData::Extension(Box::new(n))
    }
}

/// A definition term (`<dt>`).
#[derive(Debug, Clone, Copy, Default)]
pub struct DefinitionTerm;

impl NodeKind for DefinitionTerm {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "DefinitionTerm"
    }
}

impl PrettyPrint for DefinitionTerm {
    fn pretty_print(&self, _w: &mut dyn Write, _source: &str, _level: usize) -> fmt::Result {
        Ok(())
    }
}

impl From<DefinitionTerm> for KindData {
    fn from(n: DefinitionTerm) -> Self {
        KindData::Extension(Box::new(n))
    }
}

/// A definition description (`<dd>`).
#[derive(Debug, Clone, Copy, Default)]
pub struct DefinitionDescription;

impl NodeKind for DefinitionDescription {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }

    fn kind_name(&self) -> &'static str {
        "DefinitionDescription"
    }
}

impl PrettyPrint for DefinitionDescription {
    fn pretty_print(&self, _w: &mut dyn Write, _source: &str, _level: usize) -> fmt::Result {
        Ok(())
    }
}

impl From<DefinitionDescription> for KindData {
    fn from(n: DefinitionDescription) -> Self {
        KindData::Extension(Box::new(n))
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct DefinitionListTransformer;

/// Returns the offset of the colon of a definition line, if it is one.
///
/// Requires at most three leading spaces and whitespace (or end of line)
/// after the colon so that emoji shortcodes such as `:smile:` are not
/// mistaken for definitions.
fn definition_colon(line: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < line.len() && i < 4 && line[i] == b' ' {
        i += 1;
    }
    if i > 3 || line.get(i) != Some(&b':') {
        return None;
    }
    match line.get(i + 1) {
        None | Some(b' ' | b'\t' | b'\r' | b'\n') => Some(i),
        _ => None,
    }
}

/// Trims indentation and trailing spaces from one source line while keeping
/// its line terminator. Dropping the terminator makes `Parser::parse_block`
/// skip the line entirely (the block reader would cross into the next line).
fn trim_line(segment: Segment, source: &str) -> Segment {
    let segment = segment.trim_left_space(source);
    let bytes = &source.as_bytes()[segment.start()..segment.stop()];
    let terminator = if bytes.ends_with(b"\r\n") {
        2
    } else if bytes.ends_with(b"\n") {
        1
    } else {
        0
    };
    let mut end = bytes.len() - terminator;
    while end > 0 && matches!(bytes[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    Segment::new(segment.start(), segment.start() + end + terminator)
}

fn line_content(segment: Segment, source: &str) -> &[u8] {
    let bytes = &source.as_bytes()[segment.start()..segment.stop()];
    let end = bytes.len() - if bytes.ends_with(b"\n") { 1 } else { 0 };
    let end = end - usize::from(end > 0 && bytes[end - 1] == b'\r');
    &bytes[..end]
}

fn content_is_blank(segment: Segment, source: &str) -> bool {
    is_blank(line_content(segment, source))
}

impl ParagraphTransformer for DefinitionListTransformer {
    fn transform(
        &self,
        arena: &mut Arena,
        paragraph_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut parser::Context,
    ) {
        let source = reader.source();
        let mut lines = as_type_data_mut!(arena, paragraph_ref, Block).take_source();
        // D5: the term must be the first line of the paragraph. This keeps
        // ordinary prose ending in a colon line intact.
        if lines.len() < 2
            || definition_colon(&lines[1].bytes(source)).is_none()
            || content_is_blank(lines[0], source)
        {
            as_type_data_mut!(arena, paragraph_ref, Block).put_back_source(lines);
            return;
        }

        let list_ref = arena.new_node(DefinitionList);
        if let Some(pos) = arena[paragraph_ref].pos() {
            arena[list_ref].set_pos(pos);
        }

        let term_ref = arena.new_node(DefinitionTerm);
        as_type_data_mut!(arena, term_ref, Block).append_source_line(trim_line(lines[0], source));
        list_ref.append_child_fast(arena, term_ref);

        let mut current_dd = None;
        let mut i = 1;
        while i < lines.len() {
            if content_is_blank(lines[i], source) {
                i += 1;
                continue;
            }
            if let Some(colon) = definition_colon(&lines[i].bytes(source)) {
                let dd_ref = arena.new_node(DefinitionDescription);
                let seg = lines[i];
                let content = trim_line(Segment::new(seg.start() + colon + 1, seg.stop()), source);
                if !content_is_blank(content, source) {
                    as_type_data_mut!(arena, dd_ref, Block).append_source_line(content);
                }
                list_ref.append_child_fast(arena, dd_ref);
                current_dd = Some(dd_ref);
                i += 1;
                continue;
            }
            // D6: a line followed by a definition line starts a new term
            // instead of being swallowed by the previous description.
            if i + 1 < lines.len() && definition_colon(&lines[i + 1].bytes(source)).is_some() {
                let new_term = arena.new_node(DefinitionTerm);
                as_type_data_mut!(arena, new_term, Block)
                    .append_source_line(trim_line(lines[i], source));
                list_ref.append_child_fast(arena, new_term);
                current_dd = None;
                i += 1;
                continue;
            }
            let Some(dd_ref) = current_dd else {
                break;
            };
            as_type_data_mut!(arena, dd_ref, Block).append_source_line(trim_line(lines[i], source));
            i += 1;
        }

        if !arena[list_ref].has_children() {
            as_type_data_mut!(arena, paragraph_ref, Block).put_back_source(lines);
            list_ref.remove(arena);
            return;
        }

        let Some(parent) = arena[paragraph_ref].parent() else {
            as_type_data_mut!(arena, paragraph_ref, Block).put_back_source(lines);
            list_ref.remove(arena);
            return;
        };
        parent.insert_before(arena, paragraph_ref, list_ref);
        if i >= lines.len() {
            paragraph_ref.remove(arena);
        } else {
            lines.drain(0..i);
            as_type_data_mut!(arena, paragraph_ref, Block).put_back_source(lines);
        }
    }
}

impl From<DefinitionListTransformer> for AnyParagraphTransformer {
    fn from(t: DefinitionListTransformer) -> Self {
        AnyParagraphTransformer::Extension(Box::new(t))
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

/// Options for the definition list HTML renderer.
///
/// `<dl>`/`<dt>`/`<dd>` attributes are filtered with
/// `AttributeFilters::paragraph()`; there is no dedicated definition-list
/// filter yet. Custom paragraph filters therefore also affect these tags.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefinitionListHtmlRendererOptions;

impl RendererOptions for DefinitionListHtmlRendererOptions {}

struct TagRenderer<W: TextWrite> {
    tag: &'static str,
    writer: html::Writer,
    options: html::Options,
    _phantom: core::marker::PhantomData<W>,
}

impl<W: TextWrite> TagRenderer<W> {
    fn new(tag: &'static str, options: html::Options) -> Self {
        Self {
            tag,
            writer: html::Writer::with_options(options.clone()),
            options,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for TagRenderer<W> {
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
            self.writer.write_safe_str(w, "<")?;
            self.writer.write_safe_str(w, self.tag)?;
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
            self.writer.write_safe_str(w, "</")?;
            self.writer.write_safe_str(w, self.tag)?;
            self.writer.write_safe_str(w, ">\n")?;
        }
        Ok(WalkStatus::Continue)
    }
}

macro_rules! tag_node_renderer {
    ($name:ident, $tag:expr, $kind:ty) => {
        struct $name<W: TextWrite> {
            inner: TagRenderer<W>,
        }

        impl<W: TextWrite> $name<W> {
            fn new(opts: html::Options, _options: DefinitionListHtmlRendererOptions) -> Self {
                Self {
                    inner: TagRenderer::<W>::new($tag, opts),
                }
            }
        }

        impl<W: TextWrite> RenderNode<W> for $name<W> {
            fn render_node<'a>(
                &self,
                w: &mut W,
                source: &'a str,
                arena: &'a Arena,
                node_ref: NodeRef,
                entering: bool,
                ctx: &mut renderer::Context,
            ) -> Result<WalkStatus> {
                self.inner
                    .render_node(w, source, arena, node_ref, entering, ctx)
            }
        }

        impl<'cb, W> NodeRenderer<'cb, W> for $name<W>
        where
            W: TextWrite + 'cb,
        {
            fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'cb, W>) {
                nrr.register_node_renderer_fn(TypeId::of::<$kind>(), BoxRenderNode::new(self));
            }
        }
    };
}

tag_node_renderer!(ListTagRenderer, "dl", DefinitionList);
tag_node_renderer!(TermTagRenderer, "dt", DefinitionTerm);
tag_node_renderer!(DescriptionTagRenderer, "dd", DefinitionDescription);

// ---------------------------------------------------------------------------
// Extension factories
// ---------------------------------------------------------------------------

/// Returns a parser extension that recognizes definition lists.
pub fn definition_list_parser_extension() -> impl ParserExtension {
    ParserExtensionFn::new(|p: &mut Parser| {
        p.add_paragraph_transformer(|| DefinitionListTransformer, NoParserOptions, 250);
    })
}

/// Returns an HTML renderer extension for definition lists.
pub fn definition_list_html_renderer_extension<'cb, W>(
    options: impl Into<DefinitionListHtmlRendererOptions>,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    RendererExtensionFn::new(move |r: &mut Renderer<'cb, W>| {
        let options = options.into();
        r.add_node_renderer(ListTagRenderer::new, options);
        r.add_node_renderer(TermTagRenderer::new, options);
        r.add_node_renderer(DescriptionTagRenderer::new, options);
    })
}

#[cfg(test)]
mod tests {
    use crate::extensions::{Engine, Engines};
    use crate::{markdown_to_html_string_with_options, parser, renderer::html};

    fn render(source: &str) -> String {
        let engines = Engines::only(&[Engine::DefinitionList]);
        let mut output = String::new();
        let mut parser_opts = parser::Options::default();
        parser_opts.engines = engines;
        let mut html_opts = html::Options::default();
        html_opts.engines = engines;
        markdown_to_html_string_with_options(&mut output, source, parser_opts, html_opts).unwrap();
        output
    }

    #[test]
    fn renders_terms_and_definitions() {
        let html = render("Apple\n: A fruit.\n\nOrange\n: Citrus fruit.\n");
        assert!(html.contains("<dl>"), "{html}");
        assert!(html.contains("<dt>\nApple"), "{html}");
        assert!(html.contains("<dd>\nA fruit."), "{html}");
        assert!(html.contains("<dt>\nOrange"), "{html}");
        assert!(html.contains("</dl>"), "{html}");
    }

    #[test]
    fn disabled_engine_leaves_source_untouched() {
        let mut output = String::new();
        let mut parser_opts = parser::Options::default();
        parser_opts.engines = Engines::none();
        let mut html_opts = html::Options::default();
        html_opts.engines = Engines::none();
        markdown_to_html_string_with_options(
            &mut output,
            "Apple\n: A fruit.\n",
            parser_opts,
            html_opts,
        )
        .unwrap();
        assert!(output.contains("<p>"));
        assert!(!output.contains("<dl>"));
    }
}
