//! Document wrapper for the rushdown AST.
//!
//! The Document owns the Arena (which holds all AST nodes) and the source string.
//! Node and Walker objects share the Arena via Rc<RefCell>.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyAny};
use pyo3::IntoPyObjectExt;
use rushdown_lib::ast::{Arena, Meta, NodeRef};
use rushdown_lib::util::StringMap;
use std::rc::Rc;
use std::cell::RefCell;

use crate::node::{self, Node};
use crate::walker::Walker;

/// A Python-accessible wrapper around the rushdown AST Document.
#[pyclass(module = "rushdown", unsendable)]
pub struct Document {
    arena: Rc<RefCell<Arena>>,
    source: String,
    root_ref: NodeRef,
}

#[pymethods]
impl Document {
    #[getter]
    fn source(&self) -> &str {
        &self.source
    }

    #[getter]
    fn kind(&self) -> &'static str {
        "Document"
    }

    #[pyo3(name = "type")]
    #[getter]
    fn type_(&self) -> String {
        "block".to_string()
    }

    #[getter]
    fn children(&self) -> Vec<Node> {
        let mut result = Vec::new();
        let mut child = self.arena.borrow()[self.root_ref].first_child();
        while let Some(nref) = child {
            result.push(Node::new(self.arena.clone(), nref, self.source.clone()));
            child = self.arena.borrow()[nref].next_sibling();
        }
        result
    }

    #[getter]
    fn metadata(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        use rushdown_lib::ast::KindData;

        // Check for YAML parse error comments inserted by the meta parser
        {
            let arena_borrow = self.arena.borrow();
            let kd = &arena_borrow[self.root_ref].kind_data();
            if let KindData::Document(_) = kd {
                let mut child = arena_borrow[self.root_ref].first_child();
                while let Some(nref) = child {
                    if let KindData::HtmlBlock(hb) = arena_borrow[nref].kind_data() {
                        let value_str = lines_to_string(hb.value());
                        if let Some(err_msg) = value_str.strip_prefix("<!-- Error parsing YAML metadata: ")
                            .and_then(|s| s.strip_suffix(" -->\n"))
                        {
                            // Found a YAML parse error - raise Python exception
                            return Err(pyo3::exceptions::PyValueError::new_err(err_msg.to_string()));
                        }
                    }
                    child = arena_borrow[nref].next_sibling();
                }
            }
        }

        // Read metadata from the AST (populated by the meta parser extension)
        let arena_borrow = self.arena.borrow();
        let kd = &arena_borrow[self.root_ref].kind_data();
        if let KindData::Document(doc) = kd {
            let meta = doc.metadata();
            if !meta.is_empty() {
                return meta_to_py(py, meta);
            }
        }

        // No frontmatter present
        Ok(pyo3::types::PyDict::new(py).into())
    }

    #[getter]
    fn text(&self) -> String {
        let mut result = String::new();
        let mut child = self.arena.borrow()[self.root_ref].first_child();
        while let Some(nref) = child {
            result.push_str(&node::collect_text(&self.arena, nref, &self.source));
            child = self.arena.borrow()[nref].next_sibling();
        }
        result
    }

    fn walk(&self, py: Python<'_>, mode: &str) -> PyResult<Py<Walker>> {
        let walker = match mode {
            "depth" => Py::new(py, crate::walker::Walker::new_depth(self.arena.clone(), self.root_ref, self.source.clone()))?,
            "breadth" => Py::new(py, crate::walker::Walker::new_breadth(self.arena.clone(), self.root_ref, self.source.clone()))?,
            _ => return Err(pyo3::exceptions::PyValueError::new_err(
                "mode must be 'depth' or 'breadth'",
            )),
        };
        Ok(walker)
    }

    fn __repr__(&self) -> String {
        format!("<Document source_len={}>", self.source.len())
    }
}

impl Document {
    pub fn new(arena: Arena, source: String, root_ref: NodeRef) -> Self {
        Document {
            arena: Rc::new(RefCell::new(arena)),
            source,
            root_ref,
        }
    }

    #[allow(dead_code)]
    pub fn arena(&self) -> &Rc<RefCell<Arena>> {
        &self.arena
    }
}

fn meta_to_py(py: Python<'_>, meta: &StringMap<Meta>) -> PyResult<Py<PyAny>> {
    let py_dict = PyDict::new(py);
    for (key, value) in meta.iter() {
        let val = meta_value_to_py(py, value)?;
        py_dict.set_item(key.clone(), val)?;
    }
    Ok(py_dict.into())
}

/// Convert a rushdown Lines enum to a String.
fn lines_to_string(lines: &rushdown_lib::text::Lines) -> String {
    match lines {
        rushdown_lib::text::Lines::Empty => String::new(),
        rushdown_lib::text::Lines::Segments(segments) => {
            segments.iter()
                .map(|seg| seg.str(""))
                .collect()
        }
        rushdown_lib::text::Lines::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn meta_value_to_py(py: Python<'_>, value: &Meta) -> PyResult<Py<PyAny>> {
    match value {
        Meta::Null => Ok(py.None()),
        Meta::Bool(b) => Ok((*b).into_py_any(py)?),
        Meta::Int(i) => Ok((*i).into_py_any(py)?),
        Meta::Float(f) => Ok((*f).into_py_any(py)?),
        Meta::String(s) => Ok(s.clone().into_py_any(py)?),
        Meta::Sequence(seq) => {
            let list = PyList::empty(py);
            for v in seq {
                list.append(meta_value_to_py(py, v)?)?;
            }
            Ok(list.into_any().into())
        }
        Meta::Mapping(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map.iter() {
                let val = meta_value_to_py(py, v)?;
                dict.set_item(k.clone(), val)?;
            }
            Ok(dict.into())
        }
    }
}


