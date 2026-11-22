//! Configuration options exposed to Python.

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

use mordant_lib::extensions::{Engine, Engines as CoreEngines};

use crate::diagram::PyDiagramHtmlRendererOptions;
use crate::emoji::PyEmojiHtmlRendererOptions;
use crate::footnote::PyFootnoteHtmlRendererOptions;
use crate::math::PyMathRendererOptions;
use crate::highlighter::{list_themes, HighlightingMode, HighlightingRendererOptions};

/// ParseOptions controls how the markdown parser behaves.
#[pyclass(module = "mordant", skip_from_py_object)]
pub struct ParseOptions {
    pub smart: bool,
    pub attributes: bool,
    pub auto_heading_ids: bool,
    pub escaped_space: bool,
    pub meta_table: bool,
    /// A shared Python object so in-place changes (`opts.engines.emoji = False`)
    /// are reflected in subsequent calls.
    pub engines: Py<Engines>,
    pub engines_explicit: bool,
}

#[pymethods]
impl ParseOptions {
    #[new]
    #[pyo3(signature = (smart = false, attributes = false, auto_heading_ids = false, escaped_space = false, meta_table = false, engines = None))]
    fn new(py: Python<'_>, smart: bool, attributes: bool, auto_heading_ids: bool, escaped_space: bool, meta_table: bool, engines: Option<Py<Engines>>) -> PyResult<Self> {
        let engines_explicit = engines.is_some();
        Ok(ParseOptions {
            smart,
            attributes,
            auto_heading_ids,
            escaped_space,
            meta_table,
            engines: match engines {
                Some(e) => e,
                None => Py::new(py, Engines::default())?,
            },
            engines_explicit,
        })
    }

    #[getter]
    fn smart(&self) -> bool { self.smart }
    #[setter]
    fn set_smart(&mut self, v: bool) { self.smart = v }

    #[getter]
    fn attributes(&self) -> bool { self.attributes }
    #[setter]
    fn set_attributes(&mut self, v: bool) { self.attributes = v }

    #[getter]
    fn auto_heading_ids(&self) -> bool { self.auto_heading_ids }
    #[setter]
    fn set_auto_heading_ids(&mut self, v: bool) { self.auto_heading_ids = v }

    #[getter]
    fn escaped_space(&self) -> bool { self.escaped_space }
    #[setter]
    fn set_escaped_space(&mut self, v: bool) { self.escaped_space = v }

    #[getter]
    fn meta_table(&self) -> bool { self.meta_table }
    #[setter]
    fn set_meta_table(&mut self, v: bool) { self.meta_table = v }

    #[getter]
    fn engines(&self, py: Python<'_>) -> Py<Engines> { self.engines.clone_ref(py) }
    #[setter]
    fn set_engines(&mut self, v: Py<Engines>) {
        self.engines = v;
        self.engines_explicit = true;
    }
}

/// RenderOptions controls how the HTML renderer behaves.
#[pyclass(module = "mordant", skip_from_py_object)]
pub struct RenderOptions {
    pub hard_wraps: bool,
    pub xhtml: bool,
    pub allows_unsafe: bool,
    pub escaped_space: bool,
    pub engines: Py<Engines>,
    pub engines_explicit: bool,
    pub emoji: Option<Py<PyEmojiHtmlRendererOptions>>,
    pub diagram: Option<Py<PyDiagramHtmlRendererOptions>>,
    pub footnote: Option<Py<PyFootnoteHtmlRendererOptions>>,
    pub math: Option<Py<PyMathRendererOptions>>,
    pub highlighter: Option<Py<HighlighterOptions>>,
}

#[pymethods]
impl RenderOptions {
    #[new]
    #[pyo3(signature = (hard_wraps = false, xhtml = false, allows_unsafe = false, escaped_space = false, engines = None, emoji = None, diagram = None, footnote = None, math = None, highlighter = None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        hard_wraps: bool,
        xhtml: bool,
        allows_unsafe: bool,
        escaped_space: bool,
        engines: Option<Py<Engines>>,
        emoji: Option<Py<PyEmojiHtmlRendererOptions>>,
        diagram: Option<Py<PyDiagramHtmlRendererOptions>>,
        footnote: Option<Py<PyFootnoteHtmlRendererOptions>>,
        math: Option<Py<PyMathRendererOptions>>,
        highlighter: Option<Py<HighlighterOptions>>,
    ) -> PyResult<Self> {
        let engines_explicit = engines.is_some();
        Ok(RenderOptions {
            hard_wraps,
            xhtml,
            allows_unsafe,
            escaped_space,
            engines: match engines {
                Some(e) => e,
                None => Py::new(py, Engines::default())?,
            },
            engines_explicit,
            emoji,
            diagram,
            footnote,
            math,
            highlighter,
        })
    }

    #[getter]
    fn hard_wraps(&self) -> bool { self.hard_wraps }
    #[setter]
    fn set_hard_wraps(&mut self, v: bool) { self.hard_wraps = v }

    #[getter]
    fn xhtml(&self) -> bool { self.xhtml }
    #[setter]
    fn set_xhtml(&mut self, v: bool) { self.xhtml = v }

    #[getter]
    fn allows_unsafe(&self) -> bool { self.allows_unsafe }
    #[setter]
    fn set_allows_unsafe(&mut self, v: bool) { self.allows_unsafe = v }

    #[getter]
    fn escaped_space(&self) -> bool { self.escaped_space }
    #[setter]
    fn set_escaped_space(&mut self, v: bool) { self.escaped_space = v }

    #[getter]
    fn engines(&self, py: Python<'_>) -> Py<Engines> { self.engines.clone_ref(py) }
    #[setter]
    fn set_engines(&mut self, v: Py<Engines>) {
        self.engines = v;
        self.engines_explicit = true;
    }

    #[getter]
    fn emoji(&self, py: Python<'_>) -> Option<Py<PyEmojiHtmlRendererOptions>> { self.emoji.as_ref().map(|p| p.clone_ref(py)) }
    #[setter]
    fn set_emoji(&mut self, v: Option<Py<PyEmojiHtmlRendererOptions>>) { self.emoji = v }

    #[getter]
    fn diagram(&self, py: Python<'_>) -> Option<Py<PyDiagramHtmlRendererOptions>> { self.diagram.as_ref().map(|p| p.clone_ref(py)) }
    #[setter]
    fn set_diagram(&mut self, v: Option<Py<PyDiagramHtmlRendererOptions>>) { self.diagram = v }

    #[getter]
    fn footnote(&self, py: Python<'_>) -> Option<Py<PyFootnoteHtmlRendererOptions>> { self.footnote.as_ref().map(|p| p.clone_ref(py)) }
    #[setter]
    fn set_footnote(&mut self, v: Option<Py<PyFootnoteHtmlRendererOptions>>) { self.footnote = v }

    #[getter]
    fn math(&self, py: Python<'_>) -> Option<Py<PyMathRendererOptions>> { self.math.as_ref().map(|p| p.clone_ref(py)) }
    #[setter]
    fn set_math(&mut self, v: Option<Py<PyMathRendererOptions>>) { self.math = v }

    #[getter]
    fn highlighter(&self, py: Python<'_>) -> Option<Py<HighlighterOptions>> {
        self.highlighter.as_ref().map(|p| p.clone_ref(py))
    }
    #[setter]
    fn set_highlighter(&mut self, v: Option<Py<HighlighterOptions>>) { self.highlighter = v }
}

/// Runtime switches for the optional engines bundled in the wheel.
///
/// All engines except syntax highlighting are enabled by default; use
/// `Engines.all()` to enable highlighting too. An engine can be turned off
/// here, or by passing `engines=` to `markdown_to_html()`, `parse()`, `lint()`
/// or `fix()`. GFM is configured separately with [`GfmOptions`].
#[pyclass(module = "mordant", from_py_object)]
#[derive(Clone, Copy)]
pub struct Engines {
    #[pyo3(get, set)]
    pub meta: bool,
    #[pyo3(get, set)]
    pub emoji: bool,
    #[pyo3(get, set)]
    pub footnotes: bool,
    #[pyo3(get, set)]
    pub linter: bool,
    #[pyo3(get, set)]
    pub diagram: bool,
    #[pyo3(get, set)]
    pub chunker: bool,
    #[pyo3(get, set)]
    pub math: bool,
    #[pyo3(get, set)]
    pub highlighter: bool,
    #[pyo3(get, set)]
    pub definition_list: bool,
    #[pyo3(get, set)]
    pub fenced_div: bool,
    #[pyo3(get, set)]
    pub link_attributes: bool,
}

impl Default for Engines {
    fn default() -> Self {
        Self::from_core(CoreEngines::default())
    }
}

impl Engines {
    pub fn to_core(self) -> CoreEngines {
        let mut core = CoreEngines::none();
        core.set(Engine::Meta, self.meta);
        core.set(Engine::Emoji, self.emoji);
        core.set(Engine::Footnotes, self.footnotes);
        core.set(Engine::Linter, self.linter);
        core.set(Engine::Diagram, self.diagram);
        core.set(Engine::Chunker, self.chunker);
        core.set(Engine::Math, self.math);
        core.set(Engine::Highlighter, self.highlighter);
        core.set(Engine::DefinitionList, self.definition_list);
        core.set(Engine::FencedDiv, self.fenced_div);
        core.set(Engine::LinkAttributes, self.link_attributes);
        core
    }

    pub(crate) fn from_core(core: CoreEngines) -> Self {
        Self {
            meta: core.meta,
            emoji: core.emoji,
            footnotes: core.footnotes,
            linter: core.linter,
            diagram: core.diagram,
            chunker: core.chunker,
            math: core.math,
            highlighter: core.highlighter,
            definition_list: core.definition_list,
            fenced_div: core.fenced_div,
            link_attributes: core.link_attributes,
        }
    }

    fn from_names_with_base(
        names: Vec<String>,
        enabled: bool,
        mut core: CoreEngines,
    ) -> PyResult<Self> {
        for name in names {
            let Some(engine) = Engine::from_name(&name) else {
                return Err(PyValueError::new_err(format!(
                    "unknown engine '{}' (expected one of: {})",
                    name,
                    Engine::ALL
                        .iter()
                        .map(|e| e.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            };
            core.set(engine, enabled);
        }
        Ok(Self::from_core(core))
    }
}

#[pymethods]
impl Engines {
    #[new]
    #[pyo3(signature = (meta = true, emoji = true, footnotes = true, linter = true, diagram = true, chunker = true, math = true, highlighter = false, definition_list = true, fenced_div = true, link_attributes = true))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        meta: bool,
        emoji: bool,
        footnotes: bool,
        linter: bool,
        diagram: bool,
        chunker: bool,
        math: bool,
        highlighter: bool,
        definition_list: bool,
        fenced_div: bool,
        link_attributes: bool,
    ) -> Self {
        Self {
            meta,
            emoji,
            footnotes,
            linter,
            diagram,
            chunker,
            math,
            highlighter,
            definition_list,
            fenced_div,
            link_attributes,
        }
    }

    /// All engines enabled, including syntax highlighting.
    #[staticmethod]
    fn all() -> Self {
        Self::from_core(CoreEngines::all())
    }

    /// All engines disabled.
    #[staticmethod]
    fn none() -> Self {
        Self::from_core(CoreEngines::none())
    }

    /// Enable only the named engines.
    #[staticmethod]
    fn only(names: Vec<String>) -> PyResult<Self> {
        Self::from_names_with_base(names, true, CoreEngines::none())
    }

    /// Disable the named engines, leaving the default engines enabled.
    ///
    /// Syntax highlighting stays off, as it is in `Engines()`. Use
    /// `Engines.all_without([...])` to start from every engine instead.
    #[staticmethod]
    fn without(names: Vec<String>) -> PyResult<Self> {
        Self::from_names_with_base(names, false, CoreEngines::default())
    }

    /// Disable the named engines, starting from every engine (including
    /// syntax highlighting).
    #[staticmethod]
    fn all_without(names: Vec<String>) -> PyResult<Self> {
        Self::from_names_with_base(names, false, CoreEngines::all())
    }

    /// Returns true if the named engine is enabled.
    fn has(&self, name: &str) -> PyResult<bool> {
        Engine::from_name(name)
            .map(|e| self.to_core().get(e))
            .ok_or_else(|| PyValueError::new_err(format!("unknown engine '{name}'")))
    }

    fn __repr__(&self) -> String {
        let core = self.to_core();
        let enabled = Engine::ALL
            .iter()
            .filter(|e| core.get(**e))
            .map(|e| e.name())
            .collect::<Vec<_>>()
            .join(", ");
        format!("Engines([{enabled}])")
    }
}

/// Options for the code-highlighting renderer when used through
/// `RenderOptions(highlighter=...)`.
#[pyclass(module = "mordant", from_py_object)]
#[derive(Clone)]
pub struct HighlighterOptions {
    pub theme: String,
    pub mode: String,
}

impl HighlighterOptions {
    fn checked(theme: &str, mode: &str) -> PyResult<Self> {
        let canonical_mode = match mode.to_ascii_lowercase().as_str() {
            "attribute" => "Attribute",
            "class" => "Class",
            _ => {
                return Err(PyValueError::new_err(format!(
                    "invalid highlighting mode '{mode}' (expected 'Attribute' or 'Class')"
                )))
            }
        };
        if !list_themes().iter().any(|t| t == theme) {
            return Err(PyValueError::new_err(format!(
                "unknown highlighting theme '{theme}'"
            )));
        }
        Ok(Self {
            theme: theme.to_string(),
            mode: canonical_mode.to_string(),
        })
    }

    pub fn to_renderer_options(&self) -> HighlightingRendererOptions {
        HighlightingRendererOptions {
            theme: self.theme.clone(),
            mode: if self.mode == "Class" {
                HighlightingMode::Class
            } else {
                HighlightingMode::Attribute
            },
            math_options: None,
        }
    }
}

#[pymethods]
impl HighlighterOptions {
    #[new]
    #[pyo3(signature = (theme = "InspiredGitHub", mode = "Attribute"))]
    fn new(theme: &str, mode: &str) -> PyResult<Self> {
        Self::checked(theme, mode)
    }

    #[getter]
    fn theme(&self) -> &str { &self.theme }

    #[setter]
    fn set_theme(&mut self, theme: String) -> PyResult<()> {
        if !list_themes().iter().any(|t| t == &theme) {
            return Err(PyValueError::new_err(format!(
                "unknown highlighting theme '{theme}'"
            )));
        }
        self.theme = theme;
        Ok(())
    }

    #[getter]
    fn mode(&self) -> &str { &self.mode }

    #[setter]
    fn set_mode(&mut self, mode: String) -> PyResult<()> {
        let canonical = match mode.to_ascii_lowercase().as_str() {
            "attribute" => "Attribute",
            "class" => "Class",
            _ => {
                return Err(PyValueError::new_err(format!(
                    "invalid highlighting mode '{mode}' (expected 'Attribute' or 'Class')"
                )))
            }
        };
        self.mode = canonical.to_string();
        Ok(())
    }
}

/// Individual GFM features that can be enabled/disabled.
#[pyclass(module = "mordant")]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GfmFeature {
    Table,
    Strikethrough,
    TaskList,
    Linkify,
}

/// GfmOptions controls GitHub Flavored Markdown extensions.
///
/// Pass a list of `GfmFeature` values to enable specific features.
/// Use `GfmOptions.all()` to enable everything, or `GfmOptions.none()` for none.
///
/// By default (no gfm_opts passed), GFM is completely disabled.
#[pyclass(module = "mordant", skip_from_py_object)]
#[derive(Clone)]
pub struct GfmOptions {
    pub(crate) features: Vec<GfmFeature>,
}

impl Default for GfmOptions {
    fn default() -> Self {
        Self {
            features: vec![
                GfmFeature::Table,
                GfmFeature::Strikethrough,
                GfmFeature::TaskList,
            ],
        }
    }
}

#[pymethods]
impl GfmOptions {
    #[new]
    #[pyo3(signature = (features = None))]
    fn new(features: Option<Vec<GfmFeature>>) -> Self {
        Self {
            features: features.unwrap_or_else(|| vec![
                GfmFeature::Table,
                GfmFeature::Strikethrough,
                GfmFeature::TaskList,
            ]),
        }
    }

    /// Enable all GFM features (tables, strikethrough, task lists, linkify).
    #[staticmethod]
    fn all() -> Self {
        Self {
            features: vec![
                GfmFeature::Table,
                GfmFeature::Strikethrough,
                GfmFeature::TaskList,
                GfmFeature::Linkify,
            ],
        }
    }

    /// Disable all GFM features.
    #[staticmethod]
    fn none() -> Self {
        Self { features: vec![] }
    }

    /// Returns true if the given feature is enabled.
    fn has(&self, feature: GfmFeature) -> bool {
        self.features.contains(&feature)
    }

    #[getter]
    fn features(&self) -> Vec<GfmFeature> {
        self.features.clone()
    }
}

/// ArenaOptions controls the internal AST arena allocation.
#[pyclass(module = "mordant", skip_from_py_object)]
#[derive(Clone)]
pub struct ArenaOptions {
    #[pyo3(get, set)]
    initial_size: usize,
}

#[pymethods]
impl ArenaOptions {
    #[new]
    #[pyo3(signature = (initial_size = 1024))]
    fn new(initial_size: usize) -> Self {
        ArenaOptions { initial_size }
    }
}
