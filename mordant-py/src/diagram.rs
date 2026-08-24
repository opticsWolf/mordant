//! PyO3 bindings for the diagram engine (moved to core `mordant::diagram`).
//!
//! The pure parser/renderer lives behind the `diagram` feature of the core
//! crate; this module re-exports it and keeps the Python-exposed option
//! wrappers.

pub use mordant_lib::diagram::*;
pub use mordant_lib::mermaid_theme::*;

use pyo3::prelude::*;

// Wire the core mermaid-theme resolver up to the binding-side syntect theme
// registry (THEME_SET in `crate::highlighter`), so named code themes can be
// derived into Mermaid color schemes.
pub fn register_theme_lookup() {
    mordant_lib::mermaid_theme::register_syntax_theme_lookup(crate::highlighter::resolve_theme);
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
    pub fn to_mordant(&self) -> DiagramParserOptions {
        DiagramParserOptions {
            mermaid: MermaidParserOptions {
                enabled: self.mermaid_enabled,
            },
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
    pub fn to_mordant(&self) -> DiagramHtmlRendererOptions {
        let mode = match self.render_mode.as_str() {
            "client" => RenderMode::Client,
            "hybrid" => RenderMode::Hybrid,
            _ => RenderMode::Server,
        };
        let theme_spec = match &self.theme {
            Some(name) => mordant_lib::mermaid_theme::resolve_mermaid_theme(name),
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
