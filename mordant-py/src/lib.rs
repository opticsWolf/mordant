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
mod highlighter;
mod linter;
mod meta;
mod node;
mod options;
mod vscode_theme;
mod walker;

use document::Document;
use diagram::{diagram_html_renderer_extension, diagram_parser_extension, DiagramHtmlRendererOptions, DiagramParserOptions, PyDiagramHtmlRendererOptions, PyDiagramParserOptions};
use emoji::{emoji_html_renderer_extension, emoji_parser_extension, EmojiHtmlRendererOptions, EmojiParserOptions, PyEmojiHtmlRendererOptions, PyEmojiParserOptions};
use highlighter::{add_custom_theme, highlighting_html_renderer_extension, list_themes, list_syntaxes, load_builtin_themes, HighlightingRendererOptions, PyHighlighter, PyHighlightingMode};
use linter::{Diagnostic, FixResult, LintConfig, LintOptions, RuleMetadata};
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
    highlighting_options: Option<HighlightingRendererOptions>,
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
            highlighting_options: None,
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
    
    let base_ext = emoji_ext.and(diagram_ext);
    
    // Add highlighting extension if enabled
    if let Some(ref highlighting_opts) = render_cfg.highlighting_options {
        let highlighting_ext = highlighting_html_renderer_extension(highlighting_opts.clone());
        rushdown_lib::renderer::html::Renderer::with_extensions(
            opts,
            base_ext.and(highlighting_ext),
        )
    } else {
        rushdown_lib::renderer::html::Renderer::with_extensions(opts, base_ext)
    }
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
/// * `highlighting_theme` - Optional theme name for code highlighting (default: "InspiredGitHub")
/// * `highlighting_mode` - Optional highlighting mode ("Attribute" or "Class", default: "Attribute")
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
#[pyo3(signature = (source, gfm = false, parse_opts = None, render_opts = None, emoji_parse_opts = None, emoji_render_opts = None, diagram_parse_opts = None, diagram_render_opts = None, highlighting_theme = None, highlighting_mode = None))]
fn markdown_to_html(py: Python<'_>, source: &str, gfm: bool, parse_opts: Option<&ParseOptions>, render_opts: Option<&RenderOptions>, emoji_parse_opts: Option<&PyEmojiParserOptions>, emoji_render_opts: Option<&PyEmojiHtmlRendererOptions>, diagram_parse_opts: Option<&PyDiagramParserOptions>, diagram_render_opts: Option<&PyDiagramHtmlRendererOptions>, highlighting_theme: Option<&str>, highlighting_mode: Option<&str>) -> PyResult<String> {
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
            highlighting_options: None, // Will be set below
        }
    } else {
        RenderConfig {
            hard_wraps: false,
            xhtml: false,
            allows_unsafe: false,
            escaped_space: false,
            emoji_options: emoji_render_opts.map(|e| e.to_rushdown()).unwrap_or_default(),
            diagram_options: diagram_render_opts.map(|d| d.to_rushdown()).unwrap_or_default(),
            highlighting_options: None, // Will be set below
        }
    };

    // Add highlighting options if provided
    let final_render_cfg = if let Some(theme) = highlighting_theme {
        let mode = match highlighting_mode {
            Some("Class") => highlighter::HighlightingMode::Class,
            _ => highlighter::HighlightingMode::Attribute,
        };
        let mut cfg = render_cfg;
        cfg.highlighting_options = Some(HighlightingRendererOptions {
            theme: theme.to_string(),
            mode,
        });
        cfg
    } else {
        render_cfg
    };

    // Release GIL for CPU-heavy parse + render
    // String is Send, so it can cross the GIL boundary
    py.detach(move || {
        parse_and_render(source, gfm, &parse_cfg, &final_render_cfg)
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
/// * `lint_opts` - Optional LintOptions object (enable/disable rules, legacy)
/// * `lint_config` - Optional LintConfig object (full config with params, suppressions)
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
#[pyo3(signature = (source, gfm = false, parse_opts = None, emoji_opts = None, diagram_opts = None, lint_opts = None, lint_config = None))]
fn lint(
    py: Python<'_>,
    source: &str,
    gfm: bool,
    parse_opts: Option<&ParseOptions>,
    emoji_opts: Option<&PyEmojiParserOptions>,
    diagram_opts: Option<&PyDiagramParserOptions>,
    lint_opts: Option<&LintOptions>,
    lint_config: Option<&LintConfig>,
) -> PyResult<Vec<Diagnostic>> {
    let parse_cfg = parse_config_from(parse_opts, emoji_opts, diagram_opts);
    let lint_cfg = if let Some(cfg) = lint_config {
        let mut lc = linter::LintConfig {
            disable: cfg.disable.clone(),
            enable: cfg.enable.clone(),
            suppressions: linter::parse_suppressions(source),
            params: cfg.params.clone(),
            _enabled_when_default_false: None,
        };
        // Merge disable list from lint_opts if provided
        if let Some(opts) = lint_opts {
            lc.disable.extend(opts.disable.clone());
        }
        lc
    } else {
        let mut lc = lint_opts.map(|o| o.to_config()).unwrap_or_default();
        lc.suppressions = linter::parse_suppressions(source);
        lc
    };

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
/// * `lint_opts` - Optional LintOptions object (enable/disable rules, legacy)
/// * `default_language` - Language to insert for MD040 fixes (default: None)
/// * `lint_config` - Optional LintConfig object (full config with params, suppressions)
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
#[pyo3(signature = (source, gfm = false, parse_opts = None, emoji_opts = None, diagram_opts = None, lint_opts = None, default_language = None, lint_config = None))]
fn fix(
    py: Python<'_>,
    source: &str,
    gfm: bool,
    parse_opts: Option<&ParseOptions>,
    emoji_opts: Option<&PyEmojiParserOptions>,
    diagram_opts: Option<&PyDiagramParserOptions>,
    lint_opts: Option<&LintOptions>,
    default_language: Option<String>,
    lint_config: Option<&LintConfig>,
) -> PyResult<FixResult> {
    let parse_cfg = parse_config_from(parse_opts, emoji_opts, diagram_opts);
    let lint_cfg = if let Some(cfg) = lint_config {
        let mut lc = cfg.clone();
        lc.suppressions = linter::parse_suppressions(source);
        if let Some(opts) = lint_opts {
            lc.disable.extend(opts.disable.clone());
        }
        lc
    } else {
        let mut lc = lint_opts.map(|o| o.to_config()).unwrap_or_default();
        lc.suppressions = linter::parse_suppressions(source);
        lc
    };

    // Release GIL: parse + lint + fix produce plain-Rust values (Send).
    let outcome = py.detach(move || {
        let (arena, root_ref) = parse_only(source, gfm, &parse_cfg);
        linter::run_fix(source, &arena, root_ref, &lint_cfg, default_language.as_deref())
    });

    Ok(FixResult::from_outcome(outcome))
}


/// Return metadata for all registered lint rules.
///
/// # Returns
/// List of RuleMetadata objects, each with id, name, description, fixable, default_params
///
/// # Example
/// ```python
/// import mordant
/// for r in mordant.lint_rules():
///     print(r.id, r.name, r.fixable)
/// ```
#[pyfunction]
fn lint_rules() -> Vec<RuleMetadata> {
    linter::lint_rules()
}

/// Lint multiple files in parallel.
///
/// Each file is parsed and linted independently on a separate thread,
/// releasing the GIL for the entire batch operation.
///
/// # Arguments
/// * `files` - List of (filename, source) tuples
/// * `lint_config` - Optional LintConfig object
///
/// # Returns
/// List of (filename, list of Diagnostic) tuples
///
/// # Example
/// ```python
/// import mordant
/// files = [("a.md", "# A\n\n### C\n"), ("b.md", "# B\n\n## D\n")]
/// results = mordant.lint_many(files)
/// for name, diags in results:
///     print(f"{name}: {len(diags)} issues")
/// ```
#[pyfunction]
#[pyo3(signature = (files, lint_config = None))]
fn lint_many(
    py: Python<'_>,
    files: Vec<(String, String)>,
    lint_config: Option<&LintConfig>,
) -> PyResult<Vec<(String, Vec<Diagnostic>)>> {
    let cfg = lint_config
        .map(|c| c.clone())
        .unwrap_or_else(|| linter::LintConfig::default());

    // Release GIL for the entire batch (rayon threads run independently).
    let batch = py.detach(move || {
        linter::lint_many(&files, &cfg)
    });

    Ok(batch
        .into_iter()
        .map(|(name, violations)| {
            (
                name,
                violations
                    .into_iter()
                    .map(Diagnostic::from_violation)
                    .collect(),
            )
        })
        .collect())
}

/// Fix multiple files in parallel.
///
/// Each file is parsed, linted, and fixed independently on a separate thread.
///
/// # Arguments
/// * `files` - List of (filename, source) tuples
/// * `lint_config` - Optional LintConfig object
/// * `default_language` - Language to insert for MD040 fixes (default: None)
///
/// # Returns
/// List of (filename, FixResult) tuples
///
/// # Example
/// ```python
/// import mordant
/// files = [("a.md", "# A  \n\n\nText\n"), ("b.md", "# B\n\nText\n")]
/// results = mordant.fix_many(files)
/// for name, result in results:
///     print(f"{name}: fixed {len(result.fixed)} issues")
/// ```
#[pyfunction]
#[pyo3(signature = (files, lint_config = None, default_language = None))]
fn fix_many(
    py: Python<'_>,
    files: Vec<(String, String)>,
    lint_config: Option<&LintConfig>,
    default_language: Option<String>,
) -> PyResult<Vec<(String, FixResult)>> {
    let cfg = lint_config
        .map(|c| c.clone())
        .unwrap_or_default();

    // Release GIL for the entire batch.
    let batch = py.detach(move || {
        linter::fix_many(&files, &cfg, default_language.as_deref())
    });

    Ok(batch
        .into_iter()
        .map(|(name, outcome)| (name, FixResult::from_outcome(outcome)))
        .collect())
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
    // Load custom themes from .mordant/themes/ directory
    let _loaded = highlighter::load_builtin_themes();
    
    m.add_function(wrap_pyfunction!(markdown_to_html, m)?)?;
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(lint, m)?)?;
    m.add_function(wrap_pyfunction!(fix, m)?)?;
    m.add_function(wrap_pyfunction!(lint_rules, m)?)?;
    m.add_function(wrap_pyfunction!(lint_many, m)?)?;
    m.add_function(wrap_pyfunction!(fix_many, m)?)?;
    m.add_function(wrap_pyfunction!(add_custom_theme, m)?)?;
    m.add_function(wrap_pyfunction!(list_themes, m)?)?;
    m.add_function(wrap_pyfunction!(list_syntaxes, m)?)?;
    m.add_class::<linter::LintConfig>()?;
    m.add_class::<ParseOptions>()?;
    m.add_class::<RenderOptions>()?;
    m.add_class::<GfmOptions>()?;
    m.add_class::<ArenaOptions>()?;
    m.add_class::<LintOptions>()?;
    m.add_class::<PyEmojiParserOptions>()?;
    m.add_class::<PyEmojiHtmlRendererOptions>()?;
    m.add_class::<PyDiagramParserOptions>()?;
    m.add_class::<PyDiagramHtmlRendererOptions>()?;
    m.add_class::<PyHighlighter>()?;
    m.add_class::<PyHighlightingMode>()?;
    m.add_class::<Document>()?;
    m.add_class::<Node>()?;
    m.add_class::<Walker>()?;
    m.add_class::<Diagnostic>()?;
    m.add_class::<FixResult>()?;
    Ok(())
}
