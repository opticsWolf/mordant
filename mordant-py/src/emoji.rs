//! Emoji extension for mordant.
//!
//! Parses :shortcode: style emojis and renders them as Unicode characters
//! or custom HTML templates.

use pyo3::prelude::*;
use rushdown_lib::ast::{Arena, KindData, NodeKind, NodeRef, NodeType, PrettyPrint, WalkStatus, pp_indent};
use rushdown_lib::parser::{self, AnyInlineParser, InlineParser, Parser, ParserExtension, ParserOptions, PRIORITY_EMPHASIS};
use rushdown_lib::renderer::{self, html::{self as html_mod, Renderer, RendererExtension, RendererExtensionFn}, NodeRenderer, RendererOptions, RenderNode, TextWrite};
use rushdown_lib::text::{BlockReader, Reader};

use rushdown_lib::{Error as CoreError, Result};
use std::fmt;
use std::fmt::Write;
use std::string::String;
use std::vec::Vec;

// ---------------------------------------------------------------------------
// AST Node Data
// ---------------------------------------------------------------------------

/// Emoji node data stored in the arena.
#[derive(Debug)]
pub struct EmojiData {
    emoji: &'static emojis::Emoji,
}

impl EmojiData {
    pub fn new(emoji: &'static emojis::Emoji) -> Self {
        Self { emoji }
    }

    pub fn name(&self) -> &'static str {
        self.emoji.name()
    }

    pub fn shortcode(&self) -> Option<&str> {
        self.emoji.shortcode()
    }

    pub fn shortcodes(&self) -> impl Iterator<Item = &str> + Clone {
        self.emoji.shortcodes()
    }

    pub fn as_str(&self) -> &str {
        self.emoji.as_str()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.emoji.as_str().as_bytes()
    }
}

impl NodeKind for EmojiData {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }

    fn kind_name(&self) -> &'static str {
        "Emoji"
    }
}

impl PrettyPrint for EmojiData {
    fn pretty_print(&self, w: &mut dyn Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}name: {:?}", pp_indent(level), self.emoji.name())?;
        writeln!(
            w,
            "{}shortcodes: {:?}",
            pp_indent(level),
            self.emoji.shortcodes().collect::<Vec<_>>()
        )
    }
}

impl From<EmojiData> for KindData {
    fn from(e: EmojiData) -> Self {
        KindData::Extension(Box::new(e))
    }
}

// ---------------------------------------------------------------------------
// Parser Options
// ---------------------------------------------------------------------------

/// Options for the emoji parser.
#[derive(Debug, Clone, Default)]
pub struct EmojiParserOptions {
    /// An optional list of shortcodes to ignore when parsing emojis.
    pub blacklist: Vec<String>,
}

impl ParserOptions for EmojiParserOptions {}

/// Options for the emoji parser (Python-exposed).
#[pyclass(module = "mordant")]
pub struct PyEmojiParserOptions {
    /// A comma-separated list of emoji shortcodes to ignore.
    #[pyo3(get)]
    pub blacklist: Option<String>,
}

impl Default for PyEmojiParserOptions {
    fn default() -> Self {
        PyEmojiParserOptions { blacklist: None }
    }
}

#[pymethods]
impl PyEmojiParserOptions {
    #[new]
    fn new(blacklist: Option<String>) -> Self {
        PyEmojiParserOptions { blacklist }
    }
}

impl PyEmojiParserOptions {
    pub fn to_rushdown(&self) -> EmojiParserOptions {
        EmojiParserOptions { blacklist: self.to_blacklist() }
    }

    pub fn to_blacklist(&self) -> Vec<String> {
        self.blacklist.as_ref()
            .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default()
    }
}

/// Options for the emoji HTML renderer.
#[derive(Debug, Clone, Default)]
pub struct EmojiHtmlRendererOptions {
    /// A template string for rendering emojis. Supports {emoji}, {shortcode}, {name}.
    pub template: Option<String>,
}

impl RendererOptions for EmojiHtmlRendererOptions {}

/// Options for the emoji HTML renderer (Python-exposed).
#[pyclass(module = "mordant")]
pub struct PyEmojiHtmlRendererOptions {
    /// A template string for rendering emojis. Supports {emoji}, {shortcode}, {name}.
    #[pyo3(get)]
    pub template: Option<String>,
}

impl Default for PyEmojiHtmlRendererOptions {
    fn default() -> Self {
        PyEmojiHtmlRendererOptions { template: None }
    }
}

#[pymethods]
impl PyEmojiHtmlRendererOptions {
    #[new]
    fn new(template: Option<String>) -> Self {
        PyEmojiHtmlRendererOptions { template }
    }
}

impl PyEmojiHtmlRendererOptions {
    pub fn to_rushdown(&self) -> EmojiHtmlRendererOptions {
        EmojiHtmlRendererOptions { template: self.template.clone() }
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Inline parser for emoji shortcodes.
#[derive(Debug, Default)]
struct EmojiInlineParser {
    options: EmojiParserOptions,
}

impl EmojiInlineParser {
    fn with_options(options: EmojiParserOptions) -> Self {
        Self { options }
    }
}

impl EmojiInlineParser {
    fn is_blacklisted(&self, shortcode: &str) -> bool {
        self.options.blacklist.iter().any(|s| s == shortcode)
    }
}

impl InlineParser for EmojiInlineParser {
    fn trigger(&self) -> &[u8] {
        b":" 
    }

    fn parse(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut BlockReader,
        _ctx: &mut parser::Context,
    ) -> Option<NodeRef> {
        let (line, _) = reader.peek_line_bytes()?;
        if line.len() < 2 {
            return None;
        }

        let mut i = 1;
        while i < line.len() {
            let c = line[i];
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'+' {
                i += 1;
            } else {
                break;
            }
        }

        if i >= line.len() || line[i] != b':' {
            return None;
        }

        reader.advance(i + 1);
        let shortcode = unsafe { str::from_utf8_unchecked(&line[1..i]) };

        if self.is_blacklisted(shortcode) {
            return None;
        }

        emojis::get_by_shortcode(shortcode).map(|emoji| arena.new_node(EmojiData::new(emoji)))
    }
}

impl From<EmojiInlineParser> for AnyInlineParser {
    fn from(p: EmojiInlineParser) -> Self {
        AnyInlineParser::Extension(Box::new(p))
    }
}

// ---------------------------------------------------------------------------
// HTML Renderer
// ---------------------------------------------------------------------------

/// HTML renderer for emoji nodes.
struct EmojiHtmlRenderer<W: TextWrite> {
    _phantom: core::marker::PhantomData<W>,
    writer: html_mod::Writer,
    options: EmojiHtmlRendererOptions,
}

impl<W: TextWrite> EmojiHtmlRenderer<W> {
    pub fn new(html_opts: html_mod::Options, options: EmojiHtmlRendererOptions) -> Self {
        Self {
            _phantom: core::marker::PhantomData,
            writer: html_mod::Writer::with_options(html_opts),
            options,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for EmojiHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        _source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _context: &mut renderer::Context,
    ) -> Result<WalkStatus> {
        if entering {
            if let KindData::Extension(ref d) = arena[node_ref].kind_data() {
                if let Some(emoji_data) = (d.as_ref() as &dyn ::core::any::Any).downcast_ref::<EmojiData>() {
                    match &self.options.template {
                        Some(template) => {
                            let rendered = render_template(template, emoji_data);
                            self.writer.write_html(w, &rendered)?;
                        }
                        None => {
                            self.writer.write_html(w, emoji_data.as_str())?;
                        }
                    }
                }
            }
        }
        Ok(WalkStatus::Continue)
    }
}

impl<'r, W> NodeRenderer<'r, W> for EmojiHtmlRenderer<W>
where
    W: TextWrite + 'r,
{
    fn register_node_renderer_fn(self, nrr: &mut impl renderer::NodeRendererRegistry<'r, W>) {
        use core::any::TypeId;
        nrr.register_node_renderer_fn(TypeId::of::<EmojiData>(), renderer::BoxRenderNode::new(self));
    }
}

/// Render a template string with emoji data.
fn render_template(template: &str, emoji: &EmojiData) -> String {
    let mut out = String::with_capacity(template.len());
    let mut i = 0;

    while let Some(open_rel) = template[i..].find('{') {
        let open = i + open_rel;
        out.push_str(&template[i..open]);

        let rest = &template[open + 1..];
        if let Some(close_rel) = rest.find('}') {
            let key = &rest[..close_rel];
            let value = match key {
                "emoji" => emoji.as_str(),
                "shortcode" => emoji.shortcode().unwrap_or(""),
                "name" => emoji.name(),
                _ => {
                    out.push('{');
                    out.push_str(key);
                    out.push('}');
                    i = open + 1 + close_rel + 1;
                    continue;
                }
            };
            out.push_str(value);
            i = open + 1 + close_rel + 1;
        } else {
            out.push_str(&template[open..]);
            return out;
        }
    }

    out.push_str(&template[i..]);
    out
}

// ---------------------------------------------------------------------------
// Extension Functions
// ---------------------------------------------------------------------------

/// Create a parser extension for emoji shortcodes.
pub fn emoji_parser_extension(options: EmojiParserOptions) -> impl ParserExtension {
    parser::ParserExtensionFn::new(move |p: &mut Parser| {
        p.add_inline_parser(EmojiInlineParser::with_options, options, PRIORITY_EMPHASIS - 100);
    })
}

/// Create an HTML renderer extension for emoji nodes.
pub fn emoji_html_renderer_extension<'cb, W>(
    options: EmojiHtmlRendererOptions,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    RendererExtensionFn::new(move |r: &mut Renderer<'cb, W>| {
        r.add_node_renderer(EmojiHtmlRenderer::new, options);
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_with_emoji(source: &str) -> (Arena, NodeRef) {
        let ext = emoji_parser_extension(EmojiParserOptions::default());
        let parser = Parser::with_extensions(
            parser::Options::default(),
            ext,
        );
        let mut reader = rushdown_lib::text::BasicReader::new(source);
        parser.parse(&mut reader)
    }

    fn render_with_emoji(source: &str, options: EmojiHtmlRendererOptions) -> String {
        let parser_ext = emoji_parser_extension(EmojiParserOptions::default());
        let renderer_ext = emoji_html_renderer_extension(options);
        let html_opts = html_mod::Options { allows_unsafe: true, xhtml: false, ..html_mod::Options::default() };
        let mut result = String::new();
        let markdown_to_html = rushdown_lib::new_markdown_to_html(
            parser::Options::default(),
            html_opts,
            parser_ext,
            renderer_ext,
        );
        markdown_to_html(&mut result, source).unwrap();
        result
    }

    #[test]
    fn test_emoji_basic() {
        let (arena, doc_ref) = parse_with_emoji("I'm :joy:");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(_doc) = kd {
            let mut finder = EmojiFinder::new();
            rushdown_lib::ast::walk(&arena, doc_ref, &mut finder).unwrap();
            assert!(finder.found, "Should have found an Emoji node");
        } else {
            panic!("Expected Document node");
        }
    }

    struct EmojiFinder {
        found: bool,
    }

    impl EmojiFinder {
        fn new() -> Self {
            Self { found: false }
        }
    }

    impl rushdown_lib::ast::Walk<CoreError> for EmojiFinder {
        fn walk(&mut self, arena: &Arena, node_ref: NodeRef, entering: bool) -> Result<WalkStatus> {
            if entering {
                if let rushdown_lib::ast::KindData::Extension(ref kind) = arena[node_ref].kind_data() {
                    if let Some(_emoji_data) = (kind.as_ref() as &dyn ::core::any::Any).downcast_ref::<EmojiData>() {
                        self.found = true;
                    }
                }
            }
            Ok(WalkStatus::Continue)
        }
    }

    #[test]
    fn test_emoji_not_exists() {
        let (arena, doc_ref) = parse_with_emoji("I'm :joyjoy:");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(_doc) = kd {
            let mut finder = EmojiFinder::new();
            rushdown_lib::ast::walk(&arena, doc_ref, &mut finder).unwrap();
            assert!(!finder.found, "Unknown shortcode should not create an Emoji node");
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_emoji_blacklist() {
        let options = EmojiParserOptions {
            blacklist: vec!["joy".to_string()],
        };
        let ext = emoji_parser_extension(options);
        let parser = Parser::with_extensions(
            parser::Options::default(),
            ext,
        );
        let mut reader = rushdown_lib::text::BasicReader::new("I'm :joy:");
        let (arena, doc_ref) = parser.parse(&mut reader);
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(_doc) = kd {
            let mut finder = EmojiFinder::new();
            rushdown_lib::ast::walk(&arena, doc_ref, &mut finder).unwrap();
            assert!(!finder.found, "Blacklisted shortcode should not create an Emoji node");
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_emoji_render_unicode() {
        let html = render_with_emoji("I'm :joy:", EmojiHtmlRendererOptions::default());
        // :joy: maps to U+1F602 (😂) in the emojis crate
        assert!(html.contains("\u{1F602}"), "Should contain Unicode emoji: {}", html);
    }

    #[test]
    fn test_emoji_render_template() {
        let template = "<img src=\"https://example.com/{shortcode}.png\" />";
        let html = render_with_emoji("I'm :joy:", EmojiHtmlRendererOptions { template: Some(template.to_string()) });
        assert!(html.contains("https://example.com/joy.png"), "Should use template: {}", html);
    }

    #[test]
    fn test_emoji_render_template_name() {
        let template = "{name} emoji";
        let html = render_with_emoji("I'm :joy:", EmojiHtmlRendererOptions { template: Some(template.to_string()) });
        assert!(html.contains("joy"), "Should contain emoji name: {}", html);
    }

    #[test]
    fn test_emoji_inside_code_span() {
        // Emojis inside code spans should NOT be parsed
        let (arena, doc_ref) = parse_with_emoji("I'm `:joy:`");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(_doc) = kd {
            let mut finder = EmojiFinder::new();
            rushdown_lib::ast::walk(&arena, doc_ref, &mut finder).unwrap();
            assert!(!finder.found, "Emoji inside code span should not be parsed");
        } else {
            panic!("Expected Document node");
        }
    }

    struct EmojiCounter {
        count: usize,
    }

    impl EmojiCounter {
        fn new() -> Self {
            Self { count: 0 }
        }
    }

    impl rushdown_lib::ast::Walk<CoreError> for EmojiCounter {
        fn walk(&mut self, arena: &Arena, node_ref: NodeRef, entering: bool) -> Result<WalkStatus> {
            if entering {
                if let rushdown_lib::ast::KindData::Extension(ref kind) = arena[node_ref].kind_data() {
                    if let Some(_emoji_data) = (kind.as_ref() as &dyn ::core::any::Any).downcast_ref::<EmojiData>() {
                        self.count += 1;
                    }
                }
            }
            Ok(WalkStatus::Continue)
        }
    }

    #[test]
    fn test_emoji_multiple() {
        let (arena, doc_ref) = parse_with_emoji(":joy: :heart: :+1:");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(_doc) = kd {
            let mut counter = EmojiCounter::new();
            rushdown_lib::ast::walk(&arena, doc_ref, &mut counter).unwrap();
            assert_eq!(counter.count, 3, "Should have found 3 Emoji nodes");
        } else {
            panic!("Expected Document node");
        }
    }

    struct EmojiDataChecker {
        found: bool,
        name: Option<String>,
        has_shortcode: bool,
        has_shortcodes: bool,
    }

    impl EmojiDataChecker {
        fn new() -> Self {
            Self { found: false, name: None, has_shortcode: false, has_shortcodes: false }
        }
    }

    impl rushdown_lib::ast::Walk<CoreError> for EmojiDataChecker {
        fn walk(&mut self, arena: &Arena, node_ref: NodeRef, _entering: bool) -> Result<WalkStatus> {
            if let rushdown_lib::ast::KindData::Extension(ref kind) = arena[node_ref].kind_data() {
                if let Some(emoji_data) = (kind.as_ref() as &dyn ::core::any::Any).downcast_ref::<EmojiData>() {
                    self.found = true;
                    self.name = Some(emoji_data.name().to_string());
                    self.has_shortcode = emoji_data.shortcode().is_some();
                    self.has_shortcodes = emoji_data.shortcodes().count() > 0;
                }
            }
            Ok(WalkStatus::Continue)
        }
    }

    #[test]
    fn test_emoji_emoji_data() {
        let (arena, doc_ref) = parse_with_emoji(":smile:");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(_doc) = kd {
            let mut checker = EmojiDataChecker::new();
            rushdown_lib::ast::walk(&arena, doc_ref, &mut checker).unwrap();
            assert!(checker.found, "Should have found an Emoji node");
            assert!(checker.name.as_ref().map(|n| n.contains("smile") || n.contains("smiling")).unwrap_or(false), "Name should contain 'smile' or 'smiling': {:?}", checker.name);
            assert!(checker.has_shortcode, "Should have a shortcode");
            assert!(checker.has_shortcodes, "Should have shortcodes");
        } else {
            panic!("Expected Document node");
        }
    }
}
