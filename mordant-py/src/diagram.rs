//! Diagram extension for mordant.
//!
//! Parses ```mermaid code blocks and renders them as Mermaid diagrams.
//! Supports three rendering modes:
//! - **server**: Server-side SVG rendering via mermaid-rs-renderer (default).
//!   No CDN dependency. On render failure, falls back to raw <pre> output.
//! - **client**: Client-side rendering via Mermaid.js ESM module.
//!   Outputs raw <pre> blocks + a <script type="module"> tag.
//! - **hybrid**: Try server-side rendering; fall back to client-side
//!   (Mermaid.js ESM) if server rendering fails.

use pyo3::prelude::*;
use rushdown_lib::ast::{Arena, KindData, NodeKind, NodeRef, NodeType, PrettyPrint, WalkStatus, pp_indent};
use rushdown_lib::parser::{self, AnyAstTransformer, AstTransformer, ParserOptions};
use rushdown_lib::renderer::{self, html, PostRender, Render, RenderNode, TextWrite, NodeRendererRegistry, BoxRenderNode, NodeRenderer, RendererOptions};
use rushdown_lib::renderer::html::{RendererExtension, RendererExtensionFn};

use rushdown_lib::text::{Lines, Reader};
use rushdown_lib::context::{BoolValue, ContextKey, ContextKeyRegistry};
use rushdown_lib::{as_extension_data, as_extension_data_mut, as_kind_data, matches_kind, Result};
use std::fmt;
use std::fmt::Write as FmtWrite;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use core::any::TypeId;

use crate::mermaid_theme::{derive_mermaid_theme, MermaidThemeSpec};
use mermaid_rs_renderer::theme::Theme as MermaidRsTheme;
use mermaid_rs_renderer::{LayoutConfig, RenderOptions};

// ---------------------------------------------------------------------------
// AST Node Data
// ---------------------------------------------------------------------------

/// Diagram node data stored in the arena.
#[derive(Debug)]
pub struct Diagram {
    diagram_type: DiagramType,
    value: Lines,
}

/// An enum representing the type of a diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagramType {
    #[default]
    Mermaid,
}

impl Diagram {
    pub fn new(diagram_type: DiagramType) -> Self {
        Self {
            diagram_type,
            value: Lines::default(),
        }
    }

    pub fn diagram_type(&self) -> DiagramType {
        self.diagram_type
    }

    pub fn value(&self) -> &Lines {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<Lines>) {
        self.value = value.into();
    }
}

impl NodeKind for Diagram {
    fn typ(&self) -> NodeType {
        NodeType::LeafBlock
    }

    fn kind_name(&self) -> &'static str {
        "Diagram"
    }
}

impl PrettyPrint for Diagram {
    fn pretty_print(&self, w: &mut dyn FmtWrite, source: &str, level: usize) -> fmt::Result {
        writeln!(w, "{}DiagramType: {:?}", pp_indent(level), self.diagram_type())?;
        write!(w, "{}Value: ", pp_indent(level))?;
        writeln!(w, "[ ")?;
        for line in self.value.iter(source) {
            write!(w, "{}{}", pp_indent(level + 1), line)?;
        }
        writeln!(w)?;
        writeln!(w, "{}]", pp_indent(level))
    }
}

impl From<Diagram> for KindData {
    fn from(d: Diagram) -> Self {
        KindData::Extension(Box::new(d))
    }
}

// ---------------------------------------------------------------------------
// Parser Options
// ---------------------------------------------------------------------------

/// Options for the diagram parser.
#[derive(Debug, Clone, Default)]
pub struct DiagramParserOptions {
    #[allow(dead_code)]
    pub mermaid: MermaidParserOptions,
}

/// Options for the Mermaid diagram parser.
#[derive(Debug, Clone)]
pub struct MermaidParserOptions {
    #[allow(dead_code)]
    pub enabled: bool,
}

impl ParserOptions for DiagramParserOptions {}

impl Default for MermaidParserOptions {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Options for the diagram parser (Python-exposed).
#[pyclass(module = "mordant", name = "DiagramParserOptions")]
pub struct PyDiagramParserOptions {
    #[pyo3(get, set)]
    #[allow(dead_code)]
    mermaid_enabled: bool,
}

#[pymethods]
impl PyDiagramParserOptions {
    #[new]
    #[pyo3(signature = (mermaid_enabled = true))]
    fn new(mermaid_enabled: bool) -> Self {
        PyDiagramParserOptions { mermaid_enabled }
    }
}

impl PyDiagramParserOptions {
    pub fn to_rushdown(&self) -> DiagramParserOptions {
        DiagramParserOptions {
            mermaid: MermaidParserOptions {
                enabled: self.mermaid_enabled,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Render Mode
// ---------------------------------------------------------------------------

/// How Mermaid diagrams are rendered in HTML output.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum RenderMode {
    /// Server-side SVG rendering via mermaid-rs-renderer.
    /// No CDN dependency. On render failure, outputs raw <pre> (debug mode).
    #[default]
    Server,
    /// Client-side rendering via Mermaid.js ESM module.
    /// Outputs raw <pre> blocks + <script type="module"> tag.
    Client,
    /// Try server-side rendering first; fall back to client-side
    /// (Mermaid.js ESM) if server rendering fails.
    Hybrid,
}

// ---------------------------------------------------------------------------
// Renderer Options
// ---------------------------------------------------------------------------

/// Options for the diagram HTML renderer.
#[derive(Debug, Clone, Default)]
pub struct DiagramHtmlRendererOptions {
    pub mermaid: MermaidHtmlRenderingOptions,
}

impl RendererOptions for DiagramHtmlRendererOptions {}

/// Options for the Mermaid diagram HTML renderer.
#[derive(Debug, Clone)]
pub struct MermaidHtmlRenderingOptions {
    /// How to render diagrams.
    pub render_mode: RenderMode,
    /// URL to the Mermaid JavaScript module (only used for Client/Hybrid fallback).
    pub mermaid_url: String,
    /// Resolved Mermaid theme spec (native mermaid preset or derived syntect theme).
    /// `None` (default) keeps legacy behavior.
    pub theme_spec: MermaidThemeSpec,
}

impl Default for MermaidHtmlRenderingOptions {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::Server,
            mermaid_url: "https://cdn.jsdelivr.net/npm/mermaid@latest/dist/mermaid.esm.min.mjs".to_string(),
            theme_spec: MermaidThemeSpec::None,
        }
    }
}

/// Options for the diagram HTML renderer (Python-exposed).
#[pyclass(module = "mordant", name = "DiagramHtmlRendererOptions")]
#[derive(Clone)]
pub struct PyDiagramHtmlRendererOptions {
    #[pyo3(get, set)]
    render_mode: String,

    #[pyo3(get, set)]
    mermaid_url: Option<String>,

    #[pyo3(get, set)]
    pub theme: Option<String>,
}

#[pymethods]
impl PyDiagramHtmlRendererOptions {
    #[new]
    #[pyo3(signature = (render_mode = "server", mermaid_url = None, theme = None))]
    pub fn new(render_mode: &str, mermaid_url: Option<String>, theme: Option<String>) -> Self {
        PyDiagramHtmlRendererOptions {
            render_mode: render_mode.to_string(),
            mermaid_url,
            theme,
        }
    }
}

impl PyDiagramHtmlRendererOptions {
    pub fn to_rushdown(&self) -> DiagramHtmlRendererOptions {
        let mode = match self.render_mode.as_str() {
            "client" => RenderMode::Client,
            "hybrid" => RenderMode::Hybrid,
            _ => RenderMode::Server,
        };
        let theme_spec = match &self.theme {
            Some(name) => crate::mermaid_theme::resolve_mermaid_theme(name),
            None => MermaidThemeSpec::None,
        };
        DiagramHtmlRendererOptions {
            mermaid: MermaidHtmlRenderingOptions {
                render_mode: mode,
                mermaid_url: self.mermaid_url.clone().unwrap_or_else(|| {
                    "https://cdn.jsdelivr.net/npm/mermaid@latest/dist/mermaid.esm.min.mjs".to_string()
                }),
                theme_spec,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// AST Transformer
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct DiagramAstTransformer {
    options: DiagramParserOptions,
}

impl DiagramAstTransformer {
    pub fn with_options(options: DiagramParserOptions) -> Self {
        Self { options }
    }
}

impl AstTransformer for DiagramAstTransformer {
    fn transform(
        &self,
        arena: &mut Arena,
        doc_ref: NodeRef,
        reader: &mut rushdown_lib::text::BasicReader,
        _ctx: &mut parser::Context,
    ) {
        let source = reader.source();
        let mut target_codes: Vec<NodeRef> = Vec::new();

        // Collect code blocks with mermaid language using recursive walk
        DiagramAstTransformer::collect_mermaid_blocks(arena, doc_ref, source, &mut target_codes, &self.options);

        // Replace mermaid code blocks with Diagram nodes
        for code_ref in target_codes {
            let _code_block = as_kind_data!(arena[code_ref], CodeBlock);
            let lines = _code_block.value().clone();
            let diagram = arena.new_node(Diagram::new(DiagramType::Mermaid));
            as_extension_data_mut!(arena, diagram, Diagram).set_value(lines);
            // Copy source position from original code block so the chunker
            // can slice the raw source correctly.
            if let Some(pos) = arena[code_ref].pos() {
                arena[diagram].set_pos(pos);
            }
            arena[code_ref]
                .parent()
                .unwrap()
                .replace_child(arena, code_ref, diagram);
        }
    }
}

impl DiagramAstTransformer {
    fn collect_mermaid_blocks(
        arena: &Arena,
        node_ref: NodeRef,
        source: &str,
        target_codes: &mut Vec<NodeRef>,
        options: &DiagramParserOptions,
    ) {
        if matches_kind!(arena[node_ref], CodeBlock) {
            let code_block = as_kind_data!(arena[node_ref], CodeBlock);
            if let Some(lang) = code_block.language_str(source) {
                if lang == "mermaid" && options.mermaid.enabled {
                    target_codes.push(node_ref);
                }
            }
        }

        // Recurse into children
        let mut child = arena[node_ref].first_child();
        while let Some(child_ref) = child {
            DiagramAstTransformer::collect_mermaid_blocks(arena, child_ref, source, target_codes, options);
            child = arena[child_ref].next_sibling();
        }
    }
}

impl From<DiagramAstTransformer> for AnyAstTransformer {
    fn from(t: DiagramAstTransformer) -> Self {
        AnyAstTransformer::Extension(Box::new(t))
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

const HAS_MERMAID_DIAGRAM: &str = "mordant-diagram-hmd";

/// Precomputed Mermaid theme for rendering (avoids per-node re-derivation).
struct ResolvedMermaidTheme {
    rs_theme: MermaidRsTheme,
    client_vars: HashMap<String, String>,
    /// `Some(name)` => client uses `theme: name` (native preset, no themeVariables).
    /// `None` => client uses `theme: 'base'` + derived `themeVariables`.
    client_native_name: Option<String>,
}

struct DiagramHtmlRenderer<W: TextWrite> {
    _phantom: core::marker::PhantomData<W>,
    _options: DiagramHtmlRendererOptions,
    writer: html::Writer,
    has_mermaid_diagram: ContextKey<BoolValue>,
    resolved: Option<ResolvedMermaidTheme>,
}

fn resolve_for_render(spec: &MermaidThemeSpec) -> Option<ResolvedMermaidTheme> {
    match spec {
        MermaidThemeSpec::Native { theme, name } => Some(ResolvedMermaidTheme {
            rs_theme: theme.clone(),
            client_vars: HashMap::new(),
            client_native_name: Some(name.clone()),
        }),
        MermaidThemeSpec::Derived(syn) => {
            let s = derive_mermaid_theme(syn);
            Some(ResolvedMermaidTheme {
                rs_theme: s.rs_theme,
                client_vars: s.client_vars,
                client_native_name: None,
            })
        }
        MermaidThemeSpec::None => None,
    }
}

impl<W: TextWrite> DiagramHtmlRenderer<W> {
    fn new(
        html_opts: html::Options,
        options: DiagramHtmlRendererOptions,
        reg: Rc<RefCell<ContextKeyRegistry>>,
    ) -> Self {
        let has_mermaid_diagram = reg
            .borrow_mut()
            .get_or_create::<BoolValue>(HAS_MERMAID_DIAGRAM);
        let resolved = resolve_for_render(&options.mermaid.theme_spec);
        Self {
            _phantom: core::marker::PhantomData,
            _options: options,
            writer: html::Writer::with_options(html_opts),
            has_mermaid_diagram,
            resolved,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for DiagramHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        ctx: &mut renderer::Context,
    ) -> Result<WalkStatus> {
        let kd = as_extension_data!(arena, node_ref, Diagram);
        if entering {
            let diagram_value: String = {
                let mut buf = String::new();
                for line in kd.value().iter(source) {
                    buf.push_str(&line);
                }
                buf
            };
            let mode = &self._options.mermaid.render_mode;

            match mode {
                RenderMode::Server => match &self.resolved {
                    Some(r) => match mermaid_rs_renderer::render_with_options(
                        &diagram_value,
                        RenderOptions {
                            theme: r.rs_theme.clone(),
                            layout: LayoutConfig::default(),
                        },
                    ) {
                        Ok(svg) => {
                            self.writer.write_html(w, &format!("<div class=\"mermaid\">{}</div>", svg))?;
                        }
                        Err(_) => {
                            self.writer.write_safe_str(w, "<pre class=\"mermaid\">\n")?;
                            for line in kd.value().iter(source) {
                                self.writer.raw_write(w, &line)?;
                            }
                            self.writer.write_safe_str(w, "</pre>\n")?;
                        }
                    },
                    None => match mermaid_rs_renderer::render(&diagram_value) {
                        Ok(svg) => {
                            self.writer.write_html(w, &format!("<div class=\"mermaid\">{}</div>", svg))?;
                        }
                        Err(_) => {
                            self.writer.write_safe_str(w, "<pre class=\"mermaid\">\n")?;
                            for line in kd.value().iter(source) {
                                self.writer.raw_write(w, &line)?;
                            }
                            self.writer.write_safe_str(w, "</pre>\n")?;
                        }
                    },
                },

                RenderMode::Client => {
                    // Pure client-side: output raw <pre> and flag for script injection
                    ctx.insert(self.has_mermaid_diagram, true);
                    self.writer.write_safe_str(w, "<pre class=\"mermaid\">\n")?;
                    for line in kd.value().iter(source) {
                        self.writer.raw_write(w, &line)?;
                    }
                }

                RenderMode::Hybrid => {
                    let rendered = match &self.resolved {
                        Some(r) => mermaid_rs_renderer::render_with_options(
                            &diagram_value,
                            RenderOptions {
                                theme: r.rs_theme.clone(),
                                layout: LayoutConfig::default(),
                            },
                        )
                        .ok(),
                        None => mermaid_rs_renderer::render(&diagram_value).ok(),
                    };
                    match rendered {
                        Some(svg) => {
                            self.writer.write_html(w, &format!("<div class=\"mermaid\">{}</div>", svg))?;
                        }
                        None => {
                            // Server failed — fall back to client-side
                            ctx.insert(self.has_mermaid_diagram, true);
                            self.writer.write_safe_str(w, "<pre class=\"mermaid\">\n")?;
                            for line in kd.value().iter(source) {
                                self.writer.raw_write(w, &line)?;
                            }
                        }
                    }
                }
            }
        } else {
            // Exiting node — close the appropriate tag
            let mode = &self._options.mermaid.render_mode;
            match mode {
                RenderMode::Server => self.writer.write_safe_str(w, "</div>\n")?,
                RenderMode::Client | RenderMode::Hybrid => self.writer.write_safe_str(w, "</pre>\n")?,
            }
        }
        Ok(WalkStatus::Continue)
    }
}

struct DiagramPostRenderHook<W: TextWrite> {
    _phantom: core::marker::PhantomData<W>,
    writer: html::Writer,
    options: DiagramHtmlRendererOptions,
    has_mermaid_diagram: ContextKey<BoolValue>,
    resolved: Option<ResolvedMermaidTheme>,
}

impl<W: TextWrite> DiagramPostRenderHook<W> {
    pub fn new(
        html_opts: html::Options,
        options: DiagramHtmlRendererOptions,
        reg: Rc<RefCell<ContextKeyRegistry>>,
    ) -> Self {
        let has_mermaid_diagram = reg
            .borrow_mut()
            .get_or_create::<BoolValue>(HAS_MERMAID_DIAGRAM);
        let resolved = resolve_for_render(&options.mermaid.theme_spec);
        Self {
            _phantom: core::marker::PhantomData,
            writer: html::Writer::with_options(html_opts),
            options,
            has_mermaid_diagram,
            resolved,
        }
    }
}

impl<W: TextWrite> PostRender<W> for DiagramPostRenderHook<W> {
    fn post_render(
        &self,
        w: &mut W,
        _source: &str,
        _arena: &Arena,
        _node_ref: NodeRef,
        _render: &dyn Render<W>,
        ctx: &mut renderer::Context,
    ) -> Result<()> {
        // Never inject script tag in server mode
        if self.options.mermaid.render_mode == RenderMode::Server {
            return Ok(());
        }

        // Client or Hybrid mode: inject Mermaid.js script only if at least
        // one diagram fell back to <pre> (flagged by has_mermaid_diagram).
        if *ctx.get(self.has_mermaid_diagram).unwrap_or(&false) {
            let url = &self.options.mermaid.mermaid_url;
            match &self.resolved {
                // Native mermaid theme: let mermaid.js apply it directly.
                Some(r) if r.client_native_name.is_some() => {
                    let name = r.client_native_name.as_ref().unwrap();
                    self.writer.write_html(
                        w,
                        &format!(
                            r#"<script type="module">
import mermaid from '{}';
mermaid.initialize({{ startOnLoad: true, theme: '{}' }});
</script>
"#,
                            url, name
                        ),
                    )?;
                }
                // Derived syntect theme: custom "base" theme + themeVariables.
                Some(r) => {
                    let vars = serde_json::to_string(&r.client_vars)
                        .unwrap_or_else(|_| " {}".to_string());
                    self.writer.write_html(
                        w,
                        &format!(
                            r#"<script type="module">
import mermaid from '{}';
mermaid.initialize({{ startOnLoad: true, theme: 'base', themeVariables: {} }});
</script>
"#,
                            url, vars
                        ),
                    )?;
                }
                // Legacy: bare import, no initialize.
                None => {
                    self.writer.write_html(
                        w,
                        &format!(
                            r#"<script type="module">
import mermaid from '{}';
</script>
"#,
                            url
                        ),
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl<'cb, W> NodeRenderer<'cb, W> for DiagramHtmlRenderer<W>
where
    W: TextWrite + 'cb,
{
    fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'cb, W>) {
        nrr.register_node_renderer_fn(TypeId::of::<Diagram>(), BoxRenderNode::new(self));
    }
}

// ---------------------------------------------------------------------------
// Extension Functions
// ---------------------------------------------------------------------------

/// Returns a parser extension that parses diagrams.
pub fn diagram_parser_extension(options: impl Into<DiagramParserOptions>) -> impl parser::ParserExtension {
    parser::ParserExtensionFn::new(|p: &mut parser::Parser| {
        p.add_ast_transformer(DiagramAstTransformer::with_options, options.into(), 100);
    })
}

/// Returns a renderer extension that renders diagrams in HTML.
pub fn diagram_html_renderer_extension<'cb, W>(
    options: impl Into<DiagramHtmlRendererOptions>,
) -> impl RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    RendererExtensionFn::new(move |r: &mut html::Renderer<'cb, W>| {
        let options = options.into();
        r.add_post_render_hook(DiagramPostRenderHook::new, options.clone(), 500);
        r.add_node_renderer(DiagramHtmlRenderer::new, options);
    })
}
