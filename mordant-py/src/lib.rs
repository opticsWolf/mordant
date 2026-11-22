//! Mordant Python Bindings
//!
//! A fast CommonMark + GFM Markdown parser for Python, powered by the mordant Rust library.
//!
//! GIL management: parse() and markdown_to_html() release the GIL during CPU-heavy
//! parsing and rendering via Python::detach(). The plain-Rust config structs
//! (ParseConfig, RenderConfig) are used to pass options into the GIL-free closure.

extern crate mordant as mordant_lib;

use pyo3::prelude::*;
use mordant_lib::extensions::{Engine, Engines as CoreEngines};
use mordant_lib::parser::ParserExtension;
use mordant_lib::renderer::html::RendererExtension;

mod document;
mod chunker;
mod diagram;
mod emoji;
mod errors;
mod footnote;
mod highlighter;
mod linter;
mod math;
mod meta;
mod node;
mod options;
mod vscode_theme;
mod walker;
mod mermaid_theme;

use document::Document;
use diagram::{diagram_html_renderer_extension, diagram_parser_extension, DiagramHtmlRendererOptions, DiagramParserOptions, PyDiagramHtmlRendererOptions, PyDiagramParserOptions};
use emoji::{emoji_html_renderer_extension, emoji_parser_extension, EmojiHtmlRendererOptions, EmojiParserOptions, PyEmojiHtmlRendererOptions, PyEmojiParserOptions};
use footnote::{footnote_html_renderer_extension, footnote_parser_extension, FootnoteHtmlRendererOptions, PyFootnoteHtmlRendererOptions};
use highlighter::{add_custom_syntax, add_custom_theme, detect_language, highlighting_html_renderer_extension, list_themes, list_syntaxes, load_builtin_themes, theme_background, HighlightingRendererOptions, PyHighlighter, PyHighlightingMode};
use linter::{Diagnostic, FixResult, LintConfig, LintOptions, RuleMetadata};
use math::{math_html_renderer_extension, math_parser_extension, MathParserOptions, MathInlineRendererOptions, MathRendererOptions, PyMathRendererOptions, KATEX_CSS, KATEX_VERSION};
use node::Node;
use options::{ArenaOptions, Engines, GfmFeature, GfmOptions, HighlighterOptions, ParseOptions, RenderOptions};
use walker::Walker;

// ---------------------------------------------------------------------------
// Plain-Rust option structs — safe to use without GIL (no Python references)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) struct ParseConfig {
    attributes: bool,
    auto_heading_ids: bool,
    escaped_space: bool,
    meta_table: bool,
    emoji_options: EmojiParserOptions,
    diagram_options: DiagramParserOptions,
    math_options: MathParserOptions,
    engines: CoreEngines,
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
            math_options: MathParserOptions::default(),
            engines: CoreEngines::default(),
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
    footnote_options: FootnoteHtmlRendererOptions,
    highlighting_options: Option<HighlightingRendererOptions>,
    math_options: MathRendererOptions,
    math_inline_options: MathInlineRendererOptions,
    engines: CoreEngines,
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
            footnote_options: FootnoteHtmlRendererOptions::default(),
            highlighting_options: None,
            math_options: MathRendererOptions::default(),
            math_inline_options: MathInlineRendererOptions::default(),
            engines: CoreEngines::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Core parsing/rendering logic (runs without GIL via Python::detach)
// ---------------------------------------------------------------------------

pub(crate) fn build_parser(
    gfm_opts: Option<&GfmOptions>,
    parse_cfg: &ParseConfig,
) -> mordant_lib::parser::Parser {
    let engines = parse_cfg.engines;
    let meta_ext = meta::meta_parser_extension(meta::MetaParserOptions { table: parse_cfg.meta_table });
    let emoji_ext = emoji_parser_extension(parse_cfg.emoji_options.clone());
    let diagram_ext = diagram_parser_extension(parse_cfg.diagram_options.clone());
    let math_ext = math_parser_extension(parse_cfg.math_options.clone());
    let footnote_ext = footnote_parser_extension();
    let definition_list_ext = mordant_lib::definition_list::definition_list_parser_extension();
    let fenced_div_ext = mordant_lib::fenced_div::fenced_div_parser_extension();
    let link_attributes_ext = mordant_lib::link_attributes::link_attributes_parser_extension();

    let gfm_features = gfm_opts.map(|o| o.features.clone());

    let parser_ext = mordant_lib::parser::ParserExtensionFn::new(move |p: &mut mordant_lib::parser::Parser| {
        if engines.is_enabled(Engine::Meta) {
            meta_ext.apply(p);
        }
        if engines.is_enabled(Engine::Emoji) {
            emoji_ext.apply(p);
        }
        if engines.is_enabled(Engine::Diagram) {
            diagram_ext.apply(p);
        }
        if engines.is_enabled(Engine::Math) {
            math_ext.apply(p);
        }
        if engines.is_enabled(Engine::DefinitionList) {
            definition_list_ext.apply(p);
        }
        if engines.is_enabled(Engine::FencedDiv) {
            fenced_div_ext.apply(p);
        }
        if engines.is_enabled(Engine::LinkAttributes) {
            link_attributes_ext.apply(p);
        }

        // GFM stays a separate opt-in bundle.
        use mordant_lib::parser::{
            LinkifyParser, NoParserOptions, StrikethroughParser, TableAstTransformer,
            TableParagraphTransformer, TaskListItemParagraphTransformer,
        };
        let features = gfm_features.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
        if features.contains(&GfmFeature::Table) {
            p.add_ast_transformer(TableAstTransformer::new, NoParserOptions, 0);
            p.add_paragraph_transformer(TableParagraphTransformer::new, NoParserOptions, 200);
        }
        if features.contains(&GfmFeature::Strikethrough) {
            p.add_inline_parser(StrikethroughParser::new, NoParserOptions, 500);
        }
        if features.contains(&GfmFeature::TaskList) {
            p.add_paragraph_transformer(TaskListItemParagraphTransformer::new, NoParserOptions, 500);
        }
        if features.contains(&GfmFeature::Linkify) {
            p.add_inline_parser(
                LinkifyParser::with_options,
                mordant_lib::parser::LinkifyOptions::default(),
                999,
            );
        }

        if engines.is_enabled(Engine::Footnotes) {
            footnote_ext.apply(p);
        }
    });

    let mut parser_opts = mordant_lib::parser::Options::default();
    parser_opts.attributes = parse_cfg.attributes;
    parser_opts.auto_heading_ids = parse_cfg.auto_heading_ids;
    parser_opts.escaped_space = parse_cfg.escaped_space;
    parser_opts.engines = engines;

    mordant_lib::parser::Parser::with_extensions(parser_opts, parser_ext)
}

fn build_renderer(render_cfg: &RenderConfig) -> mordant_lib::renderer::html::Renderer<'_> {
    let engines = render_cfg.engines;
    let mut opts = mordant_lib::renderer::html::Options::default();
    opts.hard_wraps = render_cfg.hard_wraps;
    opts.xhtml = render_cfg.xhtml;
    opts.allows_unsafe = render_cfg.allows_unsafe;
    opts.escaped_space = render_cfg.escaped_space;
    opts.engines = engines;

    let renderer_ext = mordant_lib::renderer::html::RendererExtensionFn::new(move |r: &mut mordant_lib::renderer::html::Renderer<'_, String>| {
        if engines.is_enabled(Engine::Emoji) {
            let ext = emoji::emoji_html_renderer_extension(render_cfg.emoji_options.clone());
            ext.apply(r);
        }
        if engines.is_enabled(Engine::Diagram) {
            let ext = diagram::diagram_html_renderer_extension(render_cfg.diagram_options.clone());
            ext.apply(r);
        }
        if engines.is_enabled(Engine::Math) {
            math::math_html_renderer_extension(render_cfg.math_options.clone()).apply(r);
            math::math_inline_html_renderer_extension(render_cfg.math_inline_options.clone()).apply(r);
        }
        if engines.is_enabled(Engine::Footnotes) {
            footnote_html_renderer_extension(render_cfg.footnote_options.clone()).apply(r);
        }
        if engines.is_enabled(Engine::DefinitionList) {
            mordant_lib::definition_list::definition_list_html_renderer_extension(
                mordant_lib::definition_list::DefinitionListHtmlRendererOptions::default(),
            )
            .apply(r);
        }
        if engines.is_enabled(Engine::FencedDiv) {
            mordant_lib::fenced_div::fenced_div_html_renderer_extension(
                mordant_lib::fenced_div::FencedDivHtmlRendererOptions::default(),
            )
            .apply(r);
        }
        // Highlighting is applied last so it overrides the default code-block renderer.
        if engines.is_enabled(Engine::Highlighter) {
            let highlighting_opts = render_cfg.highlighting_options.clone().unwrap_or_default();
            highlighting_html_renderer_extension(highlighting_opts).apply(r);
        }
    });

    mordant_lib::renderer::html::Renderer::with_extensions(opts, renderer_ext)
}

// Parse + render to HTML string — returns Result<String, String>
fn parse_and_render(
    source: &str,
    gfm_opts: Option<&GfmOptions>,
    parse_cfg: &ParseConfig,
    render_cfg: &RenderConfig,
) -> Result<String, String> {
    let mut output = String::new();

    let parser = build_parser(gfm_opts, parse_cfg);
    let renderer = build_renderer(render_cfg);

    let mut reader = mordant_lib::text::BasicReader::new(source);
    let (arena, document_ref) = parser.parse(&mut reader);
    renderer
        .render(&mut output, source, &arena, document_ref)
        .map_err(|e| e.to_string())?;

    Ok(output)
}

// Parse only — returns arena + root ref (both plain Rust values, no Python)
pub(crate) fn parse_only(
    source: &str,
    gfm_opts: Option<&GfmOptions>,
    parse_cfg: &ParseConfig,
) -> (mordant_lib::ast::Arena, mordant_lib::ast::NodeRef) {
    let parser = build_parser(gfm_opts, parse_cfg);
    let mut reader = mordant_lib::text::BasicReader::new(source);
    parser.parse(&mut reader)
}

fn engines_from_py(py: Python<'_>, engines: &Py<Engines>) -> CoreEngines {
    engines.bind(py).borrow().to_core()
}

/// The `linter` engine flag is enforced by the Python entry points: with it
/// disabled, lint reports nothing and fix leaves the source untouched.
fn linter_engine_enabled(
    py: Python<'_>,
    engines: Option<&Engines>,
    parse_opts: Option<&ParseOptions>,
) -> bool {
    engines
        .map(|e| e.to_core().get(Engine::Linter))
        .or_else(|| {
            parse_opts.map(|o| engines_from_py(py, &o.engines).get(Engine::Linter))
        })
        .unwrap_or(true)
}

/// Resolves the engine set used by both parsing and rendering.
///
/// An explicit `engines=` argument wins. Otherwise an explicit engine set on
/// either options object is used (the render options inherit the parse set
/// and vice versa). If both sides were configured differently, that is a
/// caller error rather than a silent content loss.
fn resolve_engines(
    py: Python<'_>,
    top: Option<&Engines>,
    parse_opts: Option<&ParseOptions>,
    render_opts: Option<&RenderOptions>,
) -> PyResult<(CoreEngines, bool)> {
    // `engines_explicit` covers constructor/setter assignment; also treat a
    // value mutated through the shared getter (`opts.engines.emoji = False`)
    // as explicit when it differs from the defaults.
    let option_engines = |e: &Py<Engines>| {
        let value = engines_from_py(py, e);
        (value != CoreEngines::default()).then_some(value)
    };
    let parse_set = top
        .map(|e| e.to_core())
        .or_else(|| {
            parse_opts
                .filter(|o| o.engines_explicit)
                .map(|o| engines_from_py(py, &o.engines))
                .or_else(|| parse_opts.and_then(|o| option_engines(&o.engines)))
        });
    let render_set = top
        .map(|e| e.to_core())
        .or_else(|| {
            render_opts
                .filter(|o| o.engines_explicit)
                .map(|o| engines_from_py(py, &o.engines))
                .or_else(|| render_opts.and_then(|o| option_engines(&o.engines)))
        });
    if top.is_none() && parse_set.is_some() && render_set.is_some() && parse_set != render_set {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "conflicting engine sets: parse_opts.engines and render_opts.engines differ; \
             use the engines= argument to configure both sides",
        ));
    }
    let effective = top
        .map(|e| e.to_core())
        .or(render_set)
        .or(parse_set)
        .unwrap_or_default();
    let explicit = top.is_some() || parse_set.is_some() || render_set.is_some();
    Ok((effective, explicit))
}

// Build a ParseConfig from the optional Python option objects. Shared by
// parse(), markdown_to_html() (parse half), and lint().
fn parse_config_from(
    py: Python<'_>,
    parse_opts: Option<&ParseOptions>,
    emoji_opts: Option<&PyEmojiParserOptions>,
    diagram_opts: Option<&PyDiagramParserOptions>,
    engines: Option<CoreEngines>,
) -> ParseConfig {
    let emoji_options = emoji_opts.map(|e| e.to_mordant()).unwrap_or_default();
    let diagram_options = diagram_opts.map(|d| d.to_mordant()).unwrap_or_default();
    let engines = engines.unwrap_or_else(|| {
        parse_opts
            .map(|o| engines_from_py(py, &o.engines))
            .unwrap_or_default()
    });
    if let Some(opts) = parse_opts {
        ParseConfig {
            attributes: opts.attributes,
            auto_heading_ids: opts.auto_heading_ids,
            escaped_space: opts.escaped_space,
            meta_table: opts.meta_table,
            emoji_options,
            diagram_options,
            math_options: MathParserOptions::default(),
            engines,
        }
    } else {
        ParseConfig {
            attributes: false,
            auto_heading_ids: false,
            escaped_space: false,
            meta_table: false,
            emoji_options,
            diagram_options,
            math_options: MathParserOptions::default(),
            engines,
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
/// * `diagram_render_opts` - Optional diagram renderer options (mermaid_url, theme)
/// * `highlighting_theme` - Optional theme name for code highlighting (default: "InspiredGitHub")
/// * `highlighting_mode` - Optional highlighting mode ("Attribute" or "Class", default: "Attribute")
/// * `theme` - Optional single theme name applied to BOTH code highlighting and
///   Mermaid diagrams. Explicit `highlighting_theme` / `diagram_render_opts.theme`
///   override this convenience.
/// * `math_renderer_opts` - Optional math renderer options (output format).
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
#[pyo3(signature = (source, gfm_opts = None, parse_opts = None, render_opts = None, emoji_parse_opts = None, emoji_render_opts = None, diagram_parse_opts = None, diagram_render_opts = None, footnote_render_opts = None, highlighting_theme = None, highlighting_mode = None, theme = None, math_renderer_opts = None, engines = None))]
#[allow(clippy::too_many_arguments)]
fn markdown_to_html(py: Python<'_>, source: &str, gfm_opts: Option<&GfmOptions>, parse_opts: Option<&ParseOptions>, render_opts: Option<&RenderOptions>, emoji_parse_opts: Option<&PyEmojiParserOptions>, emoji_render_opts: Option<&PyEmojiHtmlRendererOptions>, diagram_parse_opts: Option<&PyDiagramParserOptions>, diagram_render_opts: Option<&PyDiagramHtmlRendererOptions>, footnote_render_opts: Option<&PyFootnoteHtmlRendererOptions>, highlighting_theme: Option<&str>, highlighting_mode: Option<&str>, theme: Option<&str>, math_renderer_opts: Option<&PyMathRendererOptions>, engines: Option<&Engines>) -> PyResult<String> {
    // Decide the effective engine set once. Parsing and rendering use the
    // same set so an enabled parser never produces nodes the renderer drops.
    let (mut effective_engines, engines_explicit) =
        resolve_engines(py, engines, parse_opts, render_opts)?;
    let nested_highlighter = render_opts
        .and_then(|o| o.highlighter.as_ref().map(|p| p.clone_ref(py)));
    if !engines_explicit {
        // D4: nested highlighter options imply the engine, but an explicit
        // engine set always wins.
        if nested_highlighter.is_some()
            || highlighting_theme.is_some()
            || theme.is_some()
        {
            effective_engines.set(Engine::Highlighter, true);
        }
    }

    // Extract plain-Rust configs (no Python references — safe for detach)
    let parse_cfg = parse_config_from(
        py,
        parse_opts,
        emoji_parse_opts,
        diagram_parse_opts,
        Some(effective_engines),
    );

    // Nested per-engine options on RenderOptions; legacy top-level kwargs
    // override their values but never re-enable a disabled engine.
    let nested_emoji = render_opts
        .and_then(|o| o.emoji.as_ref())
        .map(|p| p.bind(py).borrow().to_mordant());
    let nested_diagram = render_opts
        .and_then(|o| o.diagram.as_ref().map(|p| p.clone_ref(py)));
    let nested_footnote = render_opts
        .and_then(|o| o.footnote.as_ref())
        .map(|p| p.bind(py).borrow().to_mordant());
    let nested_math = render_opts
        .and_then(|o| o.math.as_ref())
        .map(|p| p.bind(py).borrow().to_mordant());
    // Single-command theming: `theme` fans out to both code highlighting and
    // diagram rendering. Explicit nested/legacy values win.
    let diagram_options = match nested_diagram {
        Some(d) => d.bind(py).borrow().merged(diagram_render_opts, theme),
        None => match diagram_render_opts {
            Some(d) => d.clone().merged(None, theme),
            None => PyDiagramHtmlRendererOptions::new("server", None, theme.map(str::to_string)),
        },
    }
    .to_mordant();
    let math_options = math_renderer_opts
        .as_ref()
        .map(|m| m.to_mordant())
        .or(nested_math)
        .unwrap_or_default();
    // Inline math uses the same output format as fenced math.
    let math_inline_options = MathInlineRendererOptions {
        output: math_options.output,
    };

    let mut render_cfg = if let Some(opts) = render_opts {
        RenderConfig {
            hard_wraps: opts.hard_wraps,
            xhtml: opts.xhtml,
            allows_unsafe: opts.allows_unsafe,
            escaped_space: opts.escaped_space,
            emoji_options: emoji_render_opts.map(|e| e.to_mordant()).or(nested_emoji).unwrap_or_default(),
            diagram_options: diagram_options.clone(),
            footnote_options: footnote_render_opts.map(|e| e.to_mordant()).or(nested_footnote).unwrap_or_default(),
            highlighting_options: None,
            math_options: math_options.clone(),
            math_inline_options: math_inline_options.clone(),
            engines: effective_engines,
        }
    } else {
        RenderConfig {
            hard_wraps: false,
            xhtml: false,
            allows_unsafe: false,
            escaped_space: false,
            emoji_options: emoji_render_opts.map(|e| e.to_mordant()).or(nested_emoji).unwrap_or_default(),
            diagram_options: diagram_options.clone(),
            footnote_options: footnote_render_opts.map(|e| e.to_mordant()).or(nested_footnote).unwrap_or_default(),
            highlighting_options: None,
            math_options: math_options.clone(),
            math_inline_options: math_inline_options.clone(),
            engines: effective_engines,
        }
    };

    if effective_engines.is_enabled(Engine::Highlighter) {
        let mut hl_opts = match &nested_highlighter {
            Some(opts) => opts.bind(py).borrow().to_renderer_options(),
            None => HighlightingRendererOptions::default(),
        };
        // Legacy kwargs override nested values when supplied.
        if let Some(theme) = highlighting_theme.or(theme) {
            hl_opts.theme = theme.to_string();
        }
        if let Some(mode) = highlighting_mode {
            hl_opts.mode = match mode.to_ascii_lowercase().as_str() {
                "class" => highlighter::HighlightingMode::Class,
                "attribute" => highlighter::HighlightingMode::Attribute,
                _ => {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "invalid highlighting mode '{mode}' (expected 'Attribute' or 'Class')"
                    )))
                }
            };
        }
        hl_opts.math_options = Some(math_options.clone());
        render_cfg.highlighting_options = Some(hl_opts);
    }

    // Release GIL for CPU-heavy parse + render
    // String is Send, so it can cross the GIL boundary
    py.detach(move || {
        parse_and_render(source, gfm_opts, &parse_cfg, &render_cfg)
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
#[pyo3(signature = (source, gfm_opts = None, parse_opts = None, emoji_opts = None, diagram_opts = None, engines = None))]
fn parse(py: Python<'_>, source: &str, gfm_opts: Option<&GfmOptions>, parse_opts: Option<&ParseOptions>, emoji_opts: Option<&PyEmojiParserOptions>, diagram_opts: Option<&PyDiagramParserOptions>, engines: Option<&Engines>) -> PyResult<Document> {
    // Extract plain-Rust config (no Python references — safe for detach)
    let parse_cfg = parse_config_from(py, parse_opts, emoji_opts, diagram_opts, engines.map(|e| e.to_core()));
    let gfm_owned: Option<GfmOptions> = gfm_opts.cloned();

    // Release GIL for CPU-heavy parse.
    // Arena and NodeRef are now Send (via ExtensionData::Send bound),
    // so they can cross the GIL boundary.
    let (arena, root_ref) = py.detach({
        let parse_cfg = parse_cfg.clone();
        let gfm_owned = gfm_owned.clone();
        move || parse_only(source, gfm_owned.as_ref(), &parse_cfg)
    });
    Ok(Document::new(arena, std::rc::Rc::from(source), root_ref, parse_cfg, gfm_owned))
}

/// Lint Markdown source and return a list of Diagnostic objects.
///
/// Parses the source into the mordant AST and evaluates the lint rules
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
#[pyo3(signature = (source, gfm_opts = None, parse_opts = None, emoji_opts = None, diagram_opts = None, lint_opts = None, lint_config = None, engines = None))]
fn lint(
    py: Python<'_>,
    source: &str,
    gfm_opts: Option<&GfmOptions>,
    parse_opts: Option<&ParseOptions>,
    emoji_opts: Option<&PyEmojiParserOptions>,
    diagram_opts: Option<&PyDiagramParserOptions>,
    lint_opts: Option<&LintOptions>,
    lint_config: Option<&LintConfig>,
    engines: Option<&Engines>,
) -> PyResult<Vec<Diagnostic>> {
    if !linter_engine_enabled(py, engines, parse_opts) {
        return Ok(Vec::new());
    }
    let parse_cfg = parse_config_from(py, parse_opts, emoji_opts, diagram_opts, engines.map(|e| e.to_core()));
    let lint_cfg = if let Some(cfg) = lint_config {
        let mut lc = linter::PlainLintConfig {
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
        let mut lc = lint_opts.map(|o| o.to_config().into_inner()).unwrap_or_default();
        lc.suppressions = linter::parse_suppressions(source);
        lc
    };

    // Release GIL: parse + lint produce plain-Rust values (Arena/NodeRef are
    // Send, Violation is Send), so the whole pipeline runs detached.
    let violations = py.detach(move || {
        let (arena, root_ref) = parse_only(source, gfm_opts, &parse_cfg);
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
#[pyo3(signature = (source, gfm_opts = None, parse_opts = None, emoji_opts = None, diagram_opts = None, lint_opts = None, default_language = None, lint_config = None, engines = None))]
fn fix(
    py: Python<'_>,
    source: &str,
    gfm_opts: Option<&GfmOptions>,
    parse_opts: Option<&ParseOptions>,
    emoji_opts: Option<&PyEmojiParserOptions>,
    diagram_opts: Option<&PyDiagramParserOptions>,
    lint_opts: Option<&LintOptions>,
    default_language: Option<String>,
    lint_config: Option<&LintConfig>,
    engines: Option<&Engines>,
) -> PyResult<FixResult> {
    if !linter_engine_enabled(py, engines, parse_opts) {
        return Ok(FixResult::from_outcome(linter::FixOutcome {
            output: source.to_string(),
            fixed: Vec::new(),
            unfixable: Vec::new(),
            remaining: Vec::new(),
        }));
    }
    let parse_cfg = parse_config_from(py, parse_opts, emoji_opts, diagram_opts, engines.map(|e| e.to_core()));
    let lint_cfg = if let Some(cfg) = lint_config {
        let mut lc = cfg.clone().into_inner();
        lc.suppressions = linter::parse_suppressions(source);
        if let Some(opts) = lint_opts {
            lc.disable.extend(opts.disable.clone());
        }
        lc
    } else {
        let mut lc = lint_opts.map(|o| o.to_config().into_inner()).unwrap_or_default();
        lc.suppressions = linter::parse_suppressions(source);
        lc
    };

    // Release GIL: parse + lint + fix produce plain-Rust values (Send).
    // The fix engine re-parses after every edit (stale AST offsets must
    // never be reused), so build the parser once and hand it to the
    // re-parsing fix loop.
    let outcome = py.detach(move || {
        let parser = build_parser(gfm_opts, &parse_cfg);
        linter::run_fix_with_parser(source, &parser, &lint_cfg, default_language.as_deref())
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
#[pyo3(signature = (files, lint_config = None, engines = None))]
fn lint_many(
    py: Python<'_>,
    files: Vec<(String, String)>,
    lint_config: Option<&LintConfig>,
    engines: Option<&Engines>,
) -> PyResult<Vec<(String, Vec<Diagnostic>)>> {
    if engines.is_some_and(|e| !e.to_core().get(Engine::Linter)) {
        return Ok(files.into_iter().map(|(name, _)| (name, Vec::new())).collect());
    }
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
#[pyo3(signature = (files, lint_config = None, default_language = None, engines = None))]
fn fix_many(
    py: Python<'_>,
    files: Vec<(String, String)>,
    lint_config: Option<&LintConfig>,
    default_language: Option<String>,
    engines: Option<&Engines>,
) -> PyResult<Vec<(String, FixResult)>> {
    if engines.is_some_and(|e| !e.to_core().get(Engine::Linter)) {
        return Ok(files
            .into_iter()
            .map(|(name, source)| {
                (
                    name,
                    FixResult::from_outcome(linter::FixOutcome {
                        output: source,
                        fixed: Vec::new(),
                        unfixable: Vec::new(),
                        remaining: Vec::new(),
                    }),
                )
            })
            .collect());
    }
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
    diagram::register_theme_lookup();
    
    m.add_function(wrap_pyfunction!(markdown_to_html, m)?)?;
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(math::render_math, m)?)?;
    m.add_function(wrap_pyfunction!(lint, m)?)?;
    m.add_function(wrap_pyfunction!(fix, m)?)?;
    m.add_function(wrap_pyfunction!(lint_rules, m)?)?;
    m.add_function(wrap_pyfunction!(lint_many, m)?)?;
    m.add_function(wrap_pyfunction!(fix_many, m)?)?;
    m.add_function(wrap_pyfunction!(add_custom_theme, m)?)?;
    m.add_function(wrap_pyfunction!(list_themes, m)?)?;
    m.add_function(wrap_pyfunction!(list_syntaxes, m)?)?;
    m.add_function(wrap_pyfunction!(add_custom_syntax, m)?)?;
    m.add_function(wrap_pyfunction!(detect_language, m)?)?;
    m.add_function(wrap_pyfunction!(theme_background, m)?)?;
    m.add_class::<linter::LintConfig>()?;
    m.add_class::<linter::RuleMetadata>()?;
    m.add_class::<ParseOptions>()?;
    m.add_class::<RenderOptions>()?;
    m.add_class::<Engines>()?;
    m.add_class::<HighlighterOptions>()?;
    m.add_class::<GfmOptions>()?;
    m.add_class::<GfmFeature>()?;
    m.add_class::<ArenaOptions>()?;
    m.add_class::<LintOptions>()?;
    m.add_class::<PyEmojiParserOptions>()?;
    m.add_class::<PyEmojiHtmlRendererOptions>()?;
    m.add_class::<PyDiagramParserOptions>()?;
    m.add_class::<PyDiagramHtmlRendererOptions>()?;
    m.add_class::<PyFootnoteHtmlRendererOptions>()?;
    m.add_class::<PyMathRendererOptions>()?;
    m.add_class::<PyHighlighter>()?;
    m.add_class::<PyHighlightingMode>()?;
    m.add_class::<Document>()?;
    m.add_class::<Node>()?;
    m.add_class::<Walker>()?;
    m.add_class::<chunker::PyExtractedChunk>()?;
    m.add_class::<chunker::PyMarkdownChunker>()?;
    m.add_class::<Diagnostic>()?;
    m.add_class::<FixResult>()?;
    m.add("KATEX_CSS", KATEX_CSS)?;
    m.add("KATEX_VERSION", KATEX_VERSION)?;
    Ok(())
}
