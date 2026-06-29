//! Mordant Python Bindings
//!
//! A fast CommonMark + GFM Markdown parser for Python, powered by the rushdown Rust library.
//!
//! GIL management: parse() and markdown_to_html() release the GIL during CPU-heavy
//! parsing and rendering via Python::detach(). The plain-Rust config structs
//! (ParseConfig, RenderConfig) are used to pass options into the GIL-free closure.

extern crate rushdown as rushdown_lib;

use pyo3::prelude::*;
use rushdown_lib::parser::ParserExtension;
use rushdown_lib::renderer::html::RendererExtension;

mod document;
mod diagram;
mod emoji;
mod errors;
mod linter;
mod meta;
mod node;
mod options;
mod walker;

use document::Document;
use diagram::{diagram_html_renderer_extension, diagram_parser_extension, DiagramHtmlRendererOptions, DiagramParserOptions, PyDiagramHtmlRendererOptions, PyDiagramParserOptions};
use emoji::{emoji_html_renderer_extension, emoji_parser_extension, EmojiHtmlRendererOptions, EmojiParserOptions, PyEmojiHtmlRendererOptions, PyEmojiParserOptions};
use linter::{Diagnostic, FixResult, LintOptions};
use node::Node;
use options::{ArenaOptions, GfmOptions, ParseOptions, RenderOptions};
use walker::Walker;

// ---------------------------------------------------------------------------
// Plain-Rust option structs — safe to use without GIL (no Python references)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ParseConfig {
    attributes: bool,
    auto_heading_ids: bool,
    escaped_space: bool,
    meta_table: bool,
    emoji_options: EmojiParserOptions,
    diagram_options: DiagramParserOptions,
}

impl Default for ParseConfig {
    fn default() -> Self {
        ParseConfig {
            attributes: false,
            auto_heading_ids: false,
            escaped_space: false,
            meta_table: false,
            emoji_options: EmojiParserOptions::default(),
            diagram_options: DiagramParserOptions::default(),
        }
    }
}

#[derive(Clone)]
struct RenderConfig {
    hard_wraps: bool,
    xhtml: bool,
    allows_unsafe: bool,
    escaped_space: bool,
    emoji_options: EmojiHtmlRendererOptions,
    diagram_options: DiagramHtmlRendererOptions,
}

impl Default for RenderConfig {
    fn default() -> Self {
        RenderConfig {
            hard_wraps: false,
            xhtml: false,
            allows_unsafe: false,
            escaped_space: false,
            emoji_options: EmojiHtmlRendererOptions::default(),
            diagram_options: DiagramHtmlRendererOptions::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Core parsing/rendering logic (runs without GIL via Python::detach)
// ---------------------------------------------------------------------------

fn build_parser(
    gfm: bool,
    parse_cfg: &ParseConfig,
) -> rushdown_lib::parser::Parser {
    let meta_opts = meta::MetaParserOptions { table: parse_cfg.meta_table };
    let meta_ext = meta::meta_parser_extension(meta_opts);

    let emoji_ext = emoji_parser_extension(parse_cfg.emoji_options.clone());
    let diagram_ext = diagram_parser_extension(parse_cfg.diagram_options.clone());

    let mut parser_opts = rushdown_lib::parser::Options::default();
    parser_opts.attributes = parse_cfg.attributes;
    parser_opts.auto_heading_ids = parse_cfg.auto_heading_ids;
    parser_opts.escaped_space = parse_cfg.escaped_space;

    let parser_ext = meta_ext.and(emoji_ext).and(diagram_ext);

    if gfm {
        rushdown_lib::parser::Parser::with_extensions(
            parser_opts,
            parser_ext.and(rushdown_lib::parser::gfm(rushdown_lib::parser::GfmOptions::default())),
        )
    } else {
        rushdown_lib::parser::Parser::with_extensions(parser_opts, parser_ext)
    }
}

fn build_renderer(render_cfg: &RenderConfig) -> rushdown_lib::renderer::html::Renderer<'_> {
    let mut opts = rushdown_lib::renderer::html::Options::default();
    opts.hard_wraps = render_cfg.hard_wraps;
    opts.xhtml = render_cfg.xhtml;
    opts.allows_unsafe = render_cfg.allows_unsafe;
    opts.escaped_space = render_cfg.escaped_space;

    let emoji_ext = emoji_html_renderer_extension(render_cfg.emoji_options.clone());
    let diagram_ext = diagram_html_renderer_extension(render_cfg.diagram_options.clone());
    rushdown_lib::renderer::html::Renderer::with_extensions(opts, emoji_ext.and(diagram_ext))
}

// Parse + render to HTML string — returns Result<String, String>
fn parse_and_render(
    source: &str,
    gfm: bool,
    parse_cfg: &ParseConfig,
    render_cfg: &RenderConfig,
) -> Result<String, String> {
    let mut output = String::new();

    let parser = build_parser(gfm, parse_cfg);
    let renderer = build_renderer(render_cfg);

    let mut reader = rushdown_lib::text::BasicReader::new(source);
    let (arena, document_ref) = parser.parse(&mut reader);
    renderer
        .render(&mut output, source, &arena, document_ref)
        .map_err(|e| e.to_string())?;

    Ok(output)
}

// Parse only — returns arena + root ref (both plain Rust values, no Python)
fn parse_only(
    source: &str,
    gfm: bool,
    parse_cfg: &ParseConfig,
) -> (rushdown_lib::ast::Arena, rushdown_lib::ast::NodeRef) {
    let parser = build_parser(gfm, parse_cfg);
    let mut reader = rushdown_lib::text::BasicReader::new(source);
    parser.parse(&mut reader)
}

// Build a ParseConfig from the optional Python option objects. Shared by
// parse(), markdown_to_html() (parse half), and lint().
fn parse_config_from(
    parse_opts: Option<&ParseOptions>,
    emoji_opts: Option<&PyEmojiParserOptions>,
    diagram_opts: Option<&PyDiagramParserOptions>,
) -> ParseConfig {
    let emoji_options = emoji_opts.map(|e| e.to_rushdown()).unwrap_or_default();
    let diagram_options = diagram_opts.map(|d| d.to_rushdown()).unwrap_or_default();
    if let Some(opts) = parse_opts {
        ParseConfig {
            attributes: opts.attributes,
            auto_heading_ids: opts.auto_heading_ids,
            escaped_space: opts.escaped_space,
            meta_table: opts.meta_table,
            emoji_options,
            diagram_options,
        }
    } else {
        ParseConfig {
            attributes: false,
            auto_heading_ids: false,
            escaped_space: false,
            meta_table: false,
            emoji_options,
            diagram_options,
        }
    }
}

// ---------------------------------------------------------------------------
// Python-exposed functions
// ---------------------------------------------------------------------------

/// Convert Markdown source to HTML.
///
/// # Arguments
/// * `source` - Markdown source string
/// * `gfm` - Whether to enable GFM extensions (default: false)
/// * `parse_opts` - Optional ParseOptions object
/// * `render_opts` - Optional RenderOptions object
/// * `emoji_parse_opts` - Optional emoji parser options (blacklist)
/// * `emoji_render_opts` - Optional emoji renderer options (template)
/// * `diagram_parse_opts` - Optional diagram parser options
/// * `diagram_render_opts` - Optional diagram renderer options (mermaid_url)
///
/// # Returns
/// HTML string
///
/// # Example
/// ```python
/// import mordant
/// html = mordant.markdown_to_html("# Hello\n\nWorld")
/// ```
#[pyfunction]
#[pyo3(signature = (source, gfm = false, parse_opts = None, render_opts = None, emoji_parse_opts = None, emoji_render_opts = None, diagram_parse_opts = None, diagram_render_opts = None))]
fn markdown_to_html(py: Python<'_>, source: &str, gfm: bool, parse_opts: Option<&ParseOptions>, render_opts: Option<&RenderOptions>, emoji_parse_opts: Option<&PyEmojiParserOptions>, emoji_render_opts: Option<&PyEmojiHtmlRendererOptions>, diagram_parse_opts: Option<&PyDiagramParserOptions>, diagram_render_opts: Option<&PyDiagramHtmlRendererOptions>) -> PyResult<String> {
    // Extract plain-Rust configs (no Python references — safe for detach)
    let parse_cfg = parse_config_from(parse_opts, emoji_parse_opts, diagram_parse_opts);

    let render_cfg = if let Some(opts) = render_opts {
        RenderConfig {
            hard_wraps: opts.hard_wraps,
            xhtml: opts.xhtml,
            allows_unsafe: opts.allows_unsafe,
            escaped_space: opts.escaped_space,
            emoji_options: emoji_render_opts.map(|e| e.to_rushdown()).unwrap_or_default(),
            diagram_options: diagram_render_opts.map(|d| d.to_rushdown()).unwrap_or_default(),
        }
    } else {
        RenderConfig {
            hard_wraps: false,
            xhtml: false,
            allows_unsafe: false,
            escaped_space: false,
            emoji_options: emoji_render_opts.map(|e| e.to_rushdown()).unwrap_or_default(),
            diagram_options: diagram_render_opts.map(|d| d.to_rushdown()).unwrap_or_default(),
        }
    };

    // Release GIL for CPU-heavy parse + render
    // String is Send, so it can cross the GIL boundary
    py.detach(move || {
        parse_and_render(source, gfm, &parse_cfg, &render_cfg)
    }).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
}

/// Parse Markdown source into a Document AST.
///
/// # Arguments
/// * `source` - Markdown source string
/// * `gfm` - Whether to enable GFM extensions (default: false)
/// * `parse_opts` - Optional ParseOptions object
/// * `emoji_opts` - Optional emoji parser options (blacklist)
/// * `diagram_opts` - Optional diagram parser options
///
/// # Returns
/// Document object containing the parsed AST
///
/// # Example
/// ```python
/// import mordant
/// doc = mordant.parse("# Hello\n\nWorld")
/// print(doc.source)
/// ```
#[pyfunction]
#[pyo3(signature = (source, gfm = false, parse_opts = None, emoji_opts = None, diagram_opts = None))]
fn parse(py: Python<'_>, source: &str, gfm: bool, parse_opts: Option<&ParseOptions>, emoji_opts: Option<&PyEmojiParserOptions>, diagram_opts: Option<&PyDiagramParserOptions>) -> PyResult<Document> {
    // Extract plain-Rust config (no Python references — safe for detach)
    let parse_cfg = parse_config_from(parse_opts, emoji_opts, diagram_opts);

    // Release GIL for CPU-heavy parse.
    // Arena and NodeRef are now Send (via ExtensionData::Send bound),
    // so they can cross the GIL boundary.
    let (arena, root_ref) = py.detach(move || {
        parse_only(source, gfm, &parse_cfg)
    });
    Ok(Document::new(arena, source.to_string(), root_ref))
}

/// Lint Markdown source and return a list of Diagnostic objects.
///
/// Parses the source into the rushdown AST and evaluates the lint rules
/// against it. Rule identifiers follow markdownlint (MD0xx).
///
/// # Arguments
/// * `source` - Markdown source string
/// * `gfm` - Whether to enable GFM extensions (default: false)
/// * `parse_opts` - Optional ParseOptions object
/// * `emoji_opts` - Optional emoji parser options
/// * `diagram_opts` - Optional diagram parser options
/// * `lint_opts` - Optional LintOptions object (enable/disable rules)
///
/// # Returns
/// List of Diagnostic objects, sorted by (line, rule id)
///
/// # Example
/// ```python
/// import mordant
/// for d in mordant.lint("# A\n\n### C"):
///     print(d.rule, d.line, d.message)
/// ```
#[pyfunction]
#[pyo3(signature = (source, gfm = false, parse_opts = None, emoji_opts = None, diagram_opts = None, lint_opts = None))]
fn lint(
    py: Python<'_>,
    source: &str,
    gfm: bool,
    parse_opts: Option<&ParseOptions>,
    emoji_opts: Option<&PyEmojiParserOptions>,
    diagram_opts: Option<&PyDiagramParserOptions>,
    lint_opts: Option<&LintOptions>,
) -> PyResult<Vec<Diagnostic>> {
    let parse_cfg = parse_config_from(parse_opts, emoji_opts, diagram_opts);
    let lint_cfg = lint_opts.map(|o| o.to_config()).unwrap_or_default();

    // Release GIL: parse + lint produce plain-Rust values (Arena/NodeRef are
    // Send, Violation is Send), so the whole pipeline runs detached.
    let violations = py.detach(move || {
        let (arena, root_ref) = parse_only(source, gfm, &parse_cfg);
        linter::run_lint(source, &arena, root_ref, &lint_cfg)
    });

    Ok(violations
        .into_iter()
        .map(Diagnostic::from_violation)
        .collect())
}

/// Lint Markdown source and auto-correct the fixable issues.
///
/// Returns a FixResult with the corrected source (`.output`), the diagnostics
/// that were fixed (`.fixed`), and the ones that still need manual attention
/// (`.unfixable`). Only whitespace/formatting rules (MD009, MD012, MD047) are
/// auto-fixed; structural rules are reported but not changed. MD040 is fixed
/// only when `default_language` is supplied (it is inserted onto fences that
/// lack a language).
///
/// # Arguments
/// * `source` - Markdown source string
/// * `gfm` - Whether to enable GFM extensions (default: false)
/// * `parse_opts` / `emoji_opts` / `diagram_opts` - Optional parser options
/// * `lint_opts` - Optional LintOptions object (enable/disable rules)
/// * `default_language` - Language to insert for MD040 fixes (default: None)
///
/// # Example
/// ```python
/// import mordant
/// result = mordant.fix("# Title  \n\n\nText")
/// print(result.output)        # corrected Markdown
/// print(len(result.fixed))    # how many issues were fixed
/// print(result.unfixable)     # what still needs manual attention
/// ```
#[pyfunction]
#[pyo3(signature = (source, gfm = false, parse_opts = None, emoji_opts = None, diagram_opts = None, lint_opts = None, default_language = None))]
fn fix(
    py: Python<'_>,
    source: &str,
    gfm: bool,
    parse_opts: Option<&ParseOptions>,
    emoji_opts: Option<&PyEmojiParserOptions>,
    diagram_opts: Option<&PyDiagramParserOptions>,
    lint_opts: Option<&LintOptions>,
    default_language: Option<String>,
) -> PyResult<FixResult> {
    let parse_cfg = parse_config_from(parse_opts, emoji_opts, diagram_opts);
    let lint_cfg = lint_opts.map(|o| o.to_config()).unwrap_or_default();

    // Release GIL: parse + lint + fix produce plain-Rust values (Send).
    let outcome = py.detach(move || {
        let (arena, root_ref) = parse_only(source, gfm, &parse_cfg);
        linter::run_fix(source, &arena, root_ref, &lint_cfg, default_language.as_deref())
    });

    Ok(FixResult::from_outcome(outcome))
}

/// Mordant - A fast CommonMark + GFM Markdown parser for Python.
///
/// # Example
/// ```python
/// import mordant
/// html = mordant.markdown_to_html("# Hello\n\n**World**")
/// print(html)
/// ```
#[pymodule]
fn mordant(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(markdown_to_html, m)?)?;
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(lint, m)?)?;
    m.add_function(wrap_pyfunction!(fix, m)?)?;
    m.add_class::<ParseOptions>()?;
    m.add_class::<RenderOptions>()?;
    m.add_class::<GfmOptions>()?;
    m.add_class::<ArenaOptions>()?;
    m.add_class::<LintOptions>()?;
    m.add_class::<PyEmojiParserOptions>()?;
    m.add_class::<PyEmojiHtmlRendererOptions>()?;
    m.add_class::<PyDiagramParserOptions>()?;
    m.add_class::<PyDiagramHtmlRendererOptions>()?;
    m.add_class::<Document>()?;
    m.add_class::<Node>()?;
    m.add_class::<Walker>()?;
    m.add_class::<Diagnostic>()?;
    m.add_class::<FixResult>()?;
    Ok(())
}
