//! Configuration options exposed to Python.

use pyo3::prelude::*;

/// ParseOptions controls how the markdown parser behaves.
#[pyclass(module = "mordant", skip_from_py_object)]
#[derive(Clone)]
pub struct ParseOptions {
    pub smart: bool,
    pub attributes: bool,
    pub auto_heading_ids: bool,
    pub escaped_space: bool,
    pub meta_table: bool,
}

#[pymethods]
impl ParseOptions {
    #[new]
    #[pyo3(signature = (smart = false, attributes = false, auto_heading_ids = false, escaped_space = false, meta_table = false))]
    fn new(smart: bool, attributes: bool, auto_heading_ids: bool, escaped_space: bool, meta_table: bool) -> Self {
        ParseOptions {
            smart,
            attributes,
            auto_heading_ids,
            escaped_space,
            meta_table,
        }
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
}

/// RenderOptions controls how the HTML renderer behaves.
#[pyclass(module = "mordant", skip_from_py_object)]
#[derive(Clone)]
pub struct RenderOptions {
    pub hard_wraps: bool,
    pub xhtml: bool,
    pub allows_unsafe: bool,
    pub escaped_space: bool,
}

#[pymethods]
impl RenderOptions {
    #[new]
    #[pyo3(signature = (hard_wraps = false, xhtml = false, allows_unsafe = false, escaped_space = false))]
    fn new(hard_wraps: bool, xhtml: bool, allows_unsafe: bool, escaped_space: bool) -> Self {
        RenderOptions {
            hard_wraps,
            xhtml,
            allows_unsafe,
            escaped_space,
        }
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
