//! Node wrapper for the rushdown AST.
//!
//! Provides Python-accessible properties for all node kinds.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use rushdown_lib::ast::{Arena, KindData, NodeRef, Task, TableCellAlignment};
use std::rc::Rc;
use std::cell::RefCell;

use crate::emoji::EmojiData;

/// A Python-accessible wrapper around a rushdown AST node.
///
/// Holds a shared reference to the Arena (via Rc<RefCell>) and the source string.
#[pyclass(module = "rushdown", unsendable)]
pub struct Node {
    arena: Rc<RefCell<Arena>>,
    node_ref: NodeRef,
    source: String,
}

#[pymethods]
impl Node {
    /// Returns the kind name of this node (e.g. "Heading", "Paragraph", "Text").
    #[getter]
    fn kind(&self) -> PyResult<&'static str> {
        let arena_borrow = self.arena.borrow();
        let kd = &arena_borrow[self.node_ref].kind_data();
        Ok(kd.kind_name())
    }

    /// Returns the type of this node: "block" or "inline".
    #[pyo3(name = "type")]
    #[getter]
    fn type_(&self) -> PyResult<String> {
        let arena_borrow = self.arena.borrow();
        let td = arena_borrow[self.node_ref].type_data();
        match td {
            rushdown_lib::ast::TypeData::Block(_) => Ok("block".to_string()),
            rushdown_lib::ast::TypeData::Inline(_) => Ok("inline".to_string()),
            _ => Ok("unknown".to_string()),
        }
    }

    /// Returns the parent node, or None if this is the document root.
    #[getter]
    fn parent(&self) -> PyResult<Option<Node>> {
        if let Some(nref) = self.arena.borrow()[self.node_ref].parent() {
            let node = Node::new(self.arena.clone(), nref, self.source.clone());
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    /// Returns the child nodes as a list of Node objects.
    #[getter]
    fn children(&self) -> PyResult<Vec<Node>> {
        let mut result = Vec::new();
        let mut child = self.arena.borrow()[self.node_ref].first_child();
        while let Some(nref) = child {
            result.push(Node::new(self.arena.clone(), nref, self.source.clone()));
            child = self.arena.borrow()[nref].next_sibling();
        }
        Ok(result)
    }

    /// Returns the next sibling node, or None.
    #[getter]
    fn next_sibling(&self) -> PyResult<Option<Node>> {
        if let Some(nref) = self.arena.borrow()[self.node_ref].next_sibling() {
            let node = Node::new(self.arena.clone(), nref, self.source.clone());
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    /// Returns the previous sibling node, or None.
    #[getter]
    fn previous_sibling(&self) -> PyResult<Option<Node>> {
        if let Some(nref) = self.arena.borrow()[self.node_ref].previous_sibling() {
            let node = Node::new(self.arena.clone(), nref, self.source.clone());
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    /// Returns True if this node has child nodes.
    #[getter]
    fn has_children(&self) -> PyResult<bool> {
        Ok(self.arena.borrow()[self.node_ref].has_children())
    }

    /// Returns the resolved text content of this node.
    #[getter]
    fn text(&self) -> PyResult<String> {
        Ok(collect_text(&self.arena, self.node_ref, &self.source))
    }

    /// Returns the attributes (HTML attributes) as a Python dict.
    #[getter]
    fn attributes(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let arena_borrow = self.arena.borrow();
        let attrs = arena_borrow[self.node_ref].attributes();
        let py_dict = PyDict::new(py);
        for (key, value) in attrs.iter() {
            // Attributes are MultilineValue - convert to string for Python
            let val_str: String = value.str(&self.source).into_owned();
            py_dict.set_item(key.clone(), val_str)?;
        }
        Ok(py_dict.into_any().into())
    }

    /// Returns the heading level (1-6) for Heading nodes, or None.
    #[getter]
    fn level(&self) -> PyResult<Option<u8>> {
        if let KindData::Heading(h) = &self.arena.borrow()[self.node_ref].kind_data() {
            Ok(Some(h.level()))
        } else {
            Ok(None)
        }
    }

    /// Returns the link/image destination URL, or None.
    #[getter]
    fn destination(&self) -> PyResult<Option<String>> {
        match &self.arena.borrow()[self.node_ref].kind_data() {
            KindData::Link(l) => Ok(Some(l.destination_str(&self.source).to_string())),
            KindData::Image(l) => Ok(Some(l.destination_str(&self.source).to_string())),
            _ => Ok(None),
        }
    }

    /// Returns the link/image title, or None.
    #[getter]
    fn title(&self) -> PyResult<Option<String>> {
        match &self.arena.borrow()[self.node_ref].kind_data() {
            KindData::Link(l) => Ok(l.title_str(&self.source).map(|s| s.into_owned())),
            KindData::Image(l) => Ok(l.title_str(&self.source).map(|s| s.into_owned())),
            _ => Ok(None),
        }
    }

    /// Returns the code block language, or None.
    #[getter]
    fn language(&self) -> PyResult<Option<String>> {
        if let KindData::CodeBlock(cb) = &self.arena.borrow()[self.node_ref].kind_data() {
            Ok(cb.language_str(&self.source).map(|s| s.to_string()))
        } else {
            Ok(None)
        }
    }

    /// Returns the code block content, or empty string.
    #[getter]
    fn code(&self) -> PyResult<String> {
        if let KindData::CodeBlock(cb) = &self.arena.borrow()[self.node_ref].kind_data() {
            let mut result = String::new();
            for line in cb.value().iter(&self.source) {
                result.push_str(&line);
            }
            Ok(result)
        } else {
            Ok(String::new())
        }
    }

    /// Returns the table cell alignment ("left", "center", "right", "none"), or None.
    #[getter]
    fn alignment(&self) -> PyResult<Option<String>> {
        if let KindData::TableCell(tc) = &self.arena.borrow()[self.node_ref].kind_data() {
            Ok(Some(alignment_to_str(tc.alignment())))
        } else {
            Ok(None)
        }
    }

    /// Returns whether this list is tight (no blank lines between items), or None.
    #[getter]
    fn is_tight(&self) -> PyResult<Option<bool>> {
        if let KindData::List(l) = &self.arena.borrow()[self.node_ref].kind_data() {
            Ok(Some(l.is_tight()))
        } else {
            Ok(None)
        }
    }

    /// Returns the starting number for ordered lists, or 0 for unordered lists.
    #[getter]
    fn start(&self) -> PyResult<Option<u32>> {
        if let KindData::List(l) = &self.arena.borrow()[self.node_ref].kind_data() {
            Ok(Some(l.start()))
        } else {
            Ok(None)
        }
    }

    /// Returns the list marker character ('-', '+', '.', ')'), or None.
    #[getter]
    fn marker(&self) -> PyResult<Option<String>> {
        if let KindData::List(l) = &self.arena.borrow()[self.node_ref].kind_data() {
            Ok(Some((l.marker() as char).to_string()))
        } else {
            Ok(None)
        }
    }

    /// Returns whether this list item is a task list item, or None.
    #[getter]
    fn is_task(&self) -> PyResult<Option<bool>> {
        if let KindData::ListItem(li) = &self.arena.borrow()[self.node_ref].kind_data() {
            Ok(Some(li.is_task()))
        } else {
            Ok(None)
        }
    }

    /// Returns the task status ("active" or "completed"), or None.
    #[getter]
    fn task_status(&self) -> PyResult<Option<String>> {
        if let KindData::ListItem(li) = &self.arena.borrow()[self.node_ref].kind_data() {
            Ok(li.task().map(|t| match t {
                Task::Active => "active".to_string(),
                Task::Completed => "completed".to_string(),
                _ => "unknown".to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Returns the emoji character (Unicode string), or None if not an emoji node.
    #[getter]
    fn emoji(&self) -> PyResult<Option<String>> {
        match &self.arena.borrow()[self.node_ref].kind_data() {
            KindData::Extension(ref d) => {
                if let Some(emoji_data) = (d.as_ref() as &dyn ::core::any::Any).downcast_ref::<EmojiData>() {
                    Ok(Some(emoji_data.as_str().to_string()))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Returns the first GitHub shortcode for this emoji, or None if not an emoji node.
    #[getter]
    fn shortcode(&self) -> PyResult<Option<String>> {
        match &self.arena.borrow()[self.node_ref].kind_data() {
            KindData::Extension(ref d) => {
                if let Some(emoji_data) = (d.as_ref() as &dyn ::core::any::Any).downcast_ref::<EmojiData>() {
                    Ok(emoji_data.shortcode().map(|s| s.to_string()))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Returns the emoji name (e.g., "joy" for :joy:), or None if not an emoji node.
    #[getter]
    fn name(&self) -> PyResult<Option<String>> {
        match &self.arena.borrow()[self.node_ref].kind_data() {
            KindData::Extension(ref d) => {
                if let Some(emoji_data) = (d.as_ref() as &dyn ::core::any::Any).downcast_ref::<EmojiData>() {
                    Ok(Some(emoji_data.name().to_string()))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    /// Returns the source line number (0-indexed), or None.
    #[getter]
    fn line(&self) -> PyResult<Option<usize>> {
        Ok(self.arena.borrow()[self.node_ref].pos())
    }

    fn __repr__(&self) -> String {
        let kind = self.kind().unwrap_or("Unknown");
        format!(
            "<Node kind={} ref={}>",
            kind, self.node_ref
        )
    }
}

impl Node {
    /// Create a new Node from an Arena reference and node reference.
    pub fn new(arena: Rc<RefCell<Arena>>, node_ref: NodeRef, source: String) -> Self {
        Node { arena, node_ref, source }
    }
}

/// Collect text content recursively from all descendants.
pub fn collect_text(arena: &Rc<RefCell<Arena>>, node_ref: NodeRef, source: &str) -> String {
    let arena_borrow = arena.borrow();
    let mut result = String::new();
    let mut child = arena_borrow[node_ref].first_child();
    while let Some(nref) = child {
        let kd = arena_borrow[nref].kind_data();
        match kd {
            KindData::Text(t) => {
                result.push_str(t.str(source));
            }
            KindData::CodeSpan(c) => {
                result.push_str(c.str(source).as_ref());
            }
            KindData::RawHtml(r) => {
                result.push_str(r.str(source).as_ref());
            }
            KindData::Strong(_s) => {
                result.push_str(&collect_text(arena, nref, source));
            }
            KindData::Emphasis(_e) => {
                result.push_str(&collect_text(arena, nref, source));
            }
            KindData::Link(_l) => {
                result.push_str(&collect_text(arena, nref, source));
            }
            KindData::Image(_i) => {
                result.push_str(&collect_text(arena, nref, source));
            }
            KindData::Strikethrough(_s) => {
                result.push_str(&collect_text(arena, nref, source));
            }
            _ => {
                result.push_str(&collect_text(arena, nref, source));
            }
        }
        child = arena_borrow[nref].next_sibling();
    }
    result
}

pub(crate) fn alignment_to_str(a: TableCellAlignment) -> String {
    match a {
        TableCellAlignment::Left => "left".to_string(),
        TableCellAlignment::Center => "center".to_string(),
        TableCellAlignment::Right => "right".to_string(),
        TableCellAlignment::None => "none".to_string(),
        _ => "unknown".to_string(),
    }
}
