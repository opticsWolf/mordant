//! LaTeX math rendering for Mordant, backed by the pure-Rust `katex-rs` crate.
//!
//! Efficiency model (verified against katex-rs 0.2.4 source):
//! - `KatexContext` (font metrics, symbol tables, function/environment registries)
//!   is expensive to build but `Send + Sync`, and every render takes `&ctx`. It is
//!   built ONCE in a `LazyLock` and shared read-only across all renders and threads.
//! - Rendered markup is memoized on `(display, output, latex)`: documents repeat
//!   formulas and rendering is the costly step. `Arc<str>` makes cache hits cheap.
//! - Rendering is pure-Rust CPU work, so it runs with the GIL released.
//!
//! Output format (`OutputFormat`, verified in types/settings.rs):
//! - "both"   -> HtmlAndMathml (default): styled HTML + MathML. Needs katex.css.
//! - "html"   -> Html only: styled HTML. Needs katex.css + fonts.
//! - "mathml" -> Mathml only: semantic MathML. Renders with no CSS/fonts in a
//!   MathML-capable engine (e.g. Chromium >= 109 / recent QtWebEngine).
//!
//! NOTE: `render_to_string` emits markup only, never CSS or fonts. For "both"/"html"
//! the consuming page must load KaTeX's stylesheet + web fonts.

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyString;

use katex::{render_to_string, KatexContext, OutputFormat, Settings, StrictMode, StrictSetting};

use rushdown_lib::ast::{Arena, CodeBlock, NodeRef, WalkStatus};
use rushdown_lib::{as_kind_data, Result};
use rushdown_lib::renderer::{self, html, NodeRenderer, RenderNode, TextWrite, NodeRendererRegistry, BoxRenderNode};
use rushdown_lib::renderer::html::{RendererExtension, RendererExtensionFn};

/// Built once, lazily. `KatexContext: Send + Sync` and renders take `&ctx`, so a
/// single global instance serves every thread.
static KATEX: LazyLock<KatexContext> = LazyLock::new(KatexContext::default);

/// `(display, output_discriminant, latex)` -> rendered markup.
///
/// `OutputFormat` derives `Eq` but not `Hash`, so we key on a small discriminant
/// instead of the enum. Unbounded, process-global: ideal for batch/CLI use, but
/// bound it (LRU) or scope it per pass in a long-running server.
static CACHE: LazyLock<RwLock<HashMap<(bool, u8, String), Arc<str>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Map a Python-facing string to an `OutputFormat`. Case-insensitive.
fn output_from_str(s: &str) -> Option<OutputFormat> {
    match s.to_ascii_lowercase().as_str() {
        "both" | "htmlandmathml" | "html_and_mathml" => Some(OutputFormat::HtmlAndMathml),
        "html" => Some(OutputFormat::Html),
        "mathml" => Some(OutputFormat::Mathml),
        _ => None,
    }
}

/// Stable cache discriminant for an `OutputFormat`.
fn output_disc(o: OutputFormat) -> u8 {
    match o {
        OutputFormat::HtmlAndMathml => 0,
        OutputFormat::Html => 1,
        OutputFormat::Mathml => 2,
    }
}

/// Render one LaTeX expression to KaTeX markup in the requested format, memoized.
///
/// Never fails: a KaTeX `ParseError` yields an HTML-escaped error span containing
/// the original source (KaTeX's `throw_on_error = false` behavior) instead of
/// aborting the surrounding document.
pub fn render_math_cached(latex: &str, display: bool, output: OutputFormat) -> Arc<str> {
    let key = (display, output_disc(output), latex.to_owned());

    if let Some(hit) = CACHE.read().unwrap().get(&key) {
        return hit.clone();
    }

    let markup: Arc<str> = Arc::from(render_one(latex, display, output));

    let mut w = CACHE.write().unwrap();
    w.entry(key).or_insert_with(|| markup.clone()).clone()
}

fn render_one(latex: &str, display: bool, output: OutputFormat) -> String {
    // Settings is cheap relative to a render; build per call, no shared macro state.
    let settings = Settings::builder()
        .display_mode(display)
        .output(output)
        // Warn-and-continue on questionable input rather than erroring out.
        .strict(StrictSetting::Mode(StrictMode::Warn))
        // `trust` defaults to false: blocks \includegraphics, \href, etc. Keep it
        // false because markdown / PDF-extracted math is typically untrusted.
        .build();

    match render_to_string(&*KATEX, latex, &settings) {
        Ok(markup) => markup,
        // Note: this fallback is an HTML <span>. In an HTML document body it renders
        // fine even alongside MathML; if you insert "mathml"-only output into a strict
        // MathML context, swap this for a <math><merror>...</merror></math>.
        Err(err) => error_fallback(latex, &err.to_string()),
    }
}

/// HTML-escaped fallback span. Both the source and the message are escaped: the
/// LaTeX is untrusted input and the message embeds a slice of it, so emitting
/// either raw would allow HTML/script injection.
fn error_fallback(latex: &str, message: &str) -> String {
    format!(
        "<span class=\"katex-error\" title=\"{}\">{}</span>",
        escape_html(message),
        escape_html(latex),
    )
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

// -----------------------------------------------------------------------------
// Python surface
// -----------------------------------------------------------------------------

/// mordant.render_math(latex, display=False, output="both") -> str
///
/// Render a LaTeX expression to KaTeX markup. `output` is one of "both"
/// (HTML+MathML, default), "html", or "mathml". Independent of the Markdown AST,
/// so it works with no parser changes. The GIL is released during the render.
///
/// ```python
/// import mordant
/// # MathML-only, for a QtWebEngine (Chromium >= 109) view with no CSS/fonts:
/// mordant.render_math(r"\int_0^\infty e^{-x^2}\,dx", display=True, output="mathml")
/// ```
#[pyfunction]
#[pyo3(signature = (latex, display = false, output = "both"))]
pub fn render_math(
    py: Python<'_>,
    latex: &str,
    display: bool,
    output: &str,
) -> PyResult<Py<PyString>> {
    let fmt = output_from_str(output).ok_or_else(|| {
        PyValueError::new_err(format!(
            "output must be 'html', 'mathml', or 'both', got {output:?}"
        ))
    })?;

    let markup = py.detach(move || render_math_cached(latex, display, fmt));
    Ok(PyString::new(py, &markup).unbind())
}

// -----------------------------------------------------------------------------
// Rushdown renderer extension — always intercepts ```math / ```latex blocks
// -----------------------------------------------------------------------------

/// Options for the math HTML renderer extension.
#[derive(Debug, Clone)]
pub struct MathRendererOptions {
    /// Output format for math blocks (default: HtmlAndMathml = "both").
    pub output: OutputFormat,
}

impl Default for MathRendererOptions {
    fn default() -> Self {
        Self {
            output: OutputFormat::HtmlAndMathml,
        }
    }
}

impl rushdown_lib::renderer::RendererOptions for MathRendererOptions {}

struct MathHtmlRenderer<W: TextWrite> {
    _phantom: core::marker::PhantomData<W>,
    writer: html::Writer,
    options: MathRendererOptions,
}

impl<W: TextWrite> MathHtmlRenderer<W> {
    fn new(html_opts: html::Options, options: MathRendererOptions) -> Self {
        Self {
            _phantom: core::marker::PhantomData,
            writer: html::Writer::with_options(html_opts),
            options,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for MathHtmlRenderer<W> {
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
            let kd = as_kind_data!(arena, node_ref, CodeBlock);
            let mut code = String::new();
            for line in kd.value().iter(source) {
                code.push_str(&line);
            }
            let lang = kd.language_str(source).unwrap_or("plaintext");

            if lang == "math" || lang == "latex" {
                let latex = code.trim_end_matches('\n');
                let markup = render_math_cached(latex, true, self.options.output);
                w.write_str(&markup)?;
            } else {
                // Render as plain code block (default behavior for non-math)
                self.writer.write_safe_str(w, "<pre><code")?;
                if let Some(lang_str) = kd.language_str(source) {
                    self.writer.write_safe_str(w, " class=\"language-")?;
                    self.writer.write(w, lang_str)?;
                    self.writer.write_safe_str(w, "\"")?;
                }
                self.writer.write_safe_str(w, ">")?;
                for line in kd.value().iter(source) {
                    self.writer.raw_write(w, &line)?;
                }
                self.writer.write_safe_str(w, "</code></pre>\n")?;
            }
        }
        Ok(WalkStatus::Continue)
    }
}


impl<'cb, W> NodeRenderer<'cb, W> for MathHtmlRenderer<W>
where
    W: TextWrite + 'cb,
{
    fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'cb, W>) {
        nrr.register_node_renderer_fn(TypeId::of::<CodeBlock>(), BoxRenderNode::new(self));
    }
}

/// Build a math renderer extension that is always active.
pub fn math_html_renderer_extension<'cb, W>(
    options: MathRendererOptions,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    RendererExtensionFn::new(move |r: &mut html::Renderer<'cb, W>| {
        r.add_node_renderer(MathHtmlRenderer::new, options);
    })
}

// -----------------------------------------------------------------------------
// Level 2: Inline $...$ math parser extension
// -----------------------------------------------------------------------------

use rushdown_lib::ast::{KindData, NodeKind, NodeType, PrettyPrint, pp_indent};
use core::fmt;
use core::fmt::Write as FmtWrite;
use rushdown_lib::parser::{self, AnyInlineParser, InlineParser, Parser, ParserExtension, ParserExtensionFn, ParserOptions, PRIORITY_EMPHASIS};
use rushdown_lib::text::Reader;
use rushdown_lib::as_extension_data;

/// Represents an inline math expression in the AST.
#[derive(Debug)]
pub struct MathData {
    /// The raw LaTeX source (without delimiters).
    latex: String,
    /// Whether this is display math (`$$...$$`) or inline math (`$...$`).
    display: bool,
}

impl MathData {
    pub fn new(latex: String, display: bool) -> Self {
        Self { latex, display }
    }

    pub fn latex(&self) -> &str {
        &self.latex
    }

    pub fn display(&self) -> bool {
        self.display
    }
}

impl NodeKind for MathData {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }

    fn kind_name(&self) -> &'static str {
        "Math"
    }
}

impl PrettyPrint for MathData {
    fn pretty_print(&self, w: &mut dyn FmtWrite, _source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}latex: {:?}", pp_indent(level), self.latex)?;
        writeln!(w, "{}display: {}", pp_indent(level), self.display)
    }
}

impl From<MathData> for KindData {
    fn from(m: MathData) -> Self {
        KindData::Extension(Box::new(m))
    }
}

/// Options for the math parser extension.
#[derive(Debug, Clone)]
pub struct MathParserOptions {
    /// Whether to enable inline `$...$` math (default: true).
    pub inline_math: bool,
    /// Whether to enable display `$$...$$` math (default: true).
    pub display_math: bool,
}

impl Default for MathParserOptions {
    fn default() -> Self {
        Self {
            inline_math: true,
            display_math: true,
        }
    }
}

impl ParserOptions for MathParserOptions {}

#[derive(Debug)]
struct MathParser {
    options: MathParserOptions,
}

impl MathParser {
    fn with_options(options: MathParserOptions) -> Self {
        Self { options }
    }
}

impl InlineParser for MathParser {
    fn trigger(&self) -> &[u8] {
        b"$"
    }

    fn parse(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut rushdown_lib::text::BlockReader,
        _ctx: &mut parser::Context,
    ) -> Option<NodeRef> {
        let (line, _) = reader.peek_line_bytes()?;
        if line.len() < 2 || line[0] != b'$' {
            return None;
        }

        // Check for display math `$$`
        let is_display = line.len() >= 2 && line[1] == b'$';
        if is_display && !self.options.display_math {
            return None;
        }
        if !is_display && !self.options.inline_math {
            return None;
        }

        let delimiter_len = if is_display { 2 } else { 1 };

        // Find the closing delimiter
        let search_start = delimiter_len;
        if line.len() <= search_start {
            return None;
        }

        // For inline math, don't allow spaces right after/before $
        if !is_display {
            if line.len() > search_start && line[search_start] == b' ' {
                return None;
            }
        }

        // Search for closing delimiter
        let mut search = search_start;
        let mut found = None;
        while search + delimiter_len <= line.len() {
            let match_close = if is_display {
                line[search] == b'$' && line[search + 1] == b'$'
            } else {
                line[search] == b'$'
            };
            if match_close {
                // For inline math, don't allow space before closing $
                if !is_display && search > search_start && line[search - 1] == b' ' {
                    search += 1;
                    continue;
                }
                found = Some(search);
                break;
            }
            search += 1;
        }

        let end = found?;
        let latex_bytes = &line[search_start..end];
        let latex = core::str::from_utf8(latex_bytes).unwrap_or("").to_string();

        if latex.is_empty() {
            return None;
        }

        reader.advance(end + delimiter_len);
        Some(arena.new_node(MathData::new(latex, is_display)))
    }
}

impl From<MathParser> for AnyInlineParser {
    fn from(p: MathParser) -> Self {
        AnyInlineParser::Extension(Box::new(p))
    }
}

/// Returns a parser extension that parses `$...$` inline math.
pub fn math_parser_extension(options: MathParserOptions) -> impl ParserExtension {
    ParserExtensionFn::new(move |p: &mut Parser| {
        p.add_inline_parser(MathParser::with_options, options.clone(), PRIORITY_EMPHASIS - 50);
    })
}

// -----------------------------------------------------------------------------
// Level 2: Math HTML renderer extension (for inline $...$ nodes)
// -----------------------------------------------------------------------------

/// Options for the math inline HTML renderer.
#[derive(Debug, Clone)]
pub struct MathInlineRendererOptions {
    /// Output format for inline math (default: HtmlAndMathml = "both").
    pub output: OutputFormat,
}

impl Default for MathInlineRendererOptions {
    fn default() -> Self {
        Self {
            output: OutputFormat::HtmlAndMathml,
        }
    }
}

impl rushdown_lib::renderer::RendererOptions for MathInlineRendererOptions {}

struct MathInlineHtmlRenderer<W: TextWrite> {
    _phantom: core::marker::PhantomData<W>,
    options: MathInlineRendererOptions,
}

impl<W: TextWrite> MathInlineHtmlRenderer<W> {
    fn new(_html_opts: html::Options, options: MathInlineRendererOptions) -> Self {
        Self {
            _phantom: core::marker::PhantomData,
            options,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for MathInlineHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        _source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _ctx: &mut renderer::Context,
    ) -> Result<WalkStatus> {
        if entering {
            let math = as_extension_data!(arena, node_ref, MathData);
            let markup = render_math_cached(math.latex(), math.display(), self.options.output);
            w.write_str(&markup)?;
        }
        Ok(WalkStatus::Continue)
    }
}

impl<'cb, W> NodeRenderer<'cb, W> for MathInlineHtmlRenderer<W>
where
    W: TextWrite + 'cb,
{
    fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'cb, W>) {
        nrr.register_node_renderer_fn(TypeId::of::<MathData>(), BoxRenderNode::new(self));
    }
}

/// Build a math inline renderer extension for `$...$` nodes.
pub fn math_inline_html_renderer_extension<'cb, W>(
    options: MathInlineRendererOptions,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    RendererExtensionFn::new(move |r: &mut html::Renderer<'cb, W>| {
        r.add_node_renderer(MathInlineHtmlRenderer::new, options);
    })
}
