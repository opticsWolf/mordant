//! Configuration options exposed to Python.

use pyo3::prelude::*;

/// ParseOptions controls how the markdown parser behaves.
#[pyclass(module = "rushdown", skip_from_py_object)]
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
#[pyclass(module = "rushdown", skip_from_py_object)]
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

/// GfmOptions controls GitHub Flavored Markdown extensions.
#[pyclass(module = "rushdown", skip_from_py_object)]
#[derive(Clone)]
pub struct GfmOptions {
    #[pyo3(get, set)]
    tables: bool,
    #[pyo3(get, set)]
    strikethrough: bool,
    #[pyo3(get, set)]
    task_lists: bool,
    #[pyo3(get, set)]
    linkify: bool,
}

#[pymethods]
impl GfmOptions {
    #[new]
    #[pyo3(signature = (tables = true, strikethrough = true, task_lists = true, linkify = true))]
    fn new(tables: bool, strikethrough: bool, task_lists: bool, linkify: bool) -> Self {
        GfmOptions {
            tables,
            strikethrough,
            task_lists,
            linkify,
        }
    }
}

/// ArenaOptions controls the internal AST arena allocation.
#[pyclass(module = "rushdown", skip_from_py_object)]
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
