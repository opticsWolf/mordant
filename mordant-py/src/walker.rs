//! Walker for traversing the AST tree.
//!
//! Supports depth-first (DFS) and breadth-first (BFS) traversal
//! as Python iterators via __iter__ and __next__.

use pyo3::prelude::*;
use rushdown_lib::ast::{Arena, NodeRef};
use std::rc::Rc;
use std::cell::RefCell;

use crate::node::Node;

/// A Python-accessible walker for the AST tree.
///
/// Iterate over all nodes in depth-first or breadth-first order.
///
/// # Example
/// ```python
/// doc = rushdown.parse("# Hello\n\n**World**")
/// for node in doc.walk("depth"):
///     print(node.kind, node.text)
/// ```
#[pyclass(module = "rushdown", unsendable)]
pub struct Walker {
    arena: Rc<RefCell<Arena>>,
    source: String,
    mode: String,
    stack: Vec<NodeRef>, // for depth-first
    queue: Vec<NodeRef>, // for breadth-first
}

#[pymethods]
impl Walker {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, _py: Python<'_>) -> PyResult<Option<Node>> {
        match self.mode.as_str() {
            "depth" => self.next_depth(),
            "breadth" => self.next_breadth(),
            _ => Ok(None),
        }
    }
}

impl Walker {
    /// Create a new depth-first walker.
    pub fn new_depth(arena: Rc<RefCell<Arena>>, root: NodeRef, source: String) -> Self {
        Walker {
            arena,
            source,
            mode: "depth".to_string(),
            stack: vec![root],
            queue: Vec::new(),
        }
    }

    /// Create a new breadth-first walker.
    pub fn new_breadth(arena: Rc<RefCell<Arena>>, root: NodeRef, source: String) -> Self {
        Walker {
            arena,
            source,
            mode: "breadth".to_string(),
            stack: Vec::new(),
            queue: vec![root],
        }
    }

    /// Depth-first (DFS) traversal.
    fn next_depth(&mut self) -> PyResult<Option<Node>> {
        if let Some(nref) = self.stack.pop() {
            // Push children in reverse order so first child is processed first
            let mut children = Vec::new();
            let mut child = self.arena.borrow()[nref].first_child();
            while let Some(c) = child {
                children.push(c);
                child = self.arena.borrow()[c].next_sibling();
            }
            // Push in reverse so first child is on top
            for c in children.into_iter().rev() {
                self.stack.push(c);
            }
            let node = Node::new(self.arena.clone(), nref, self.source.clone());
            return Ok(Some(node));
        }
        Ok(None)
    }

    /// Breadth-first (BFS) traversal.
    fn next_breadth(&mut self) -> PyResult<Option<Node>> {
        if !self.queue.is_empty() {
            let nref = self.queue.remove(0);
            // Enqueue children
            let mut child = self.arena.borrow()[nref].first_child();
            while let Some(c) = child {
                self.queue.push(c);
                child = self.arena.borrow()[c].next_sibling();
            }
            let node = Node::new(self.arena.clone(), nref, self.source.clone());
            return Ok(Some(node));
        }
        Ok(None)
    }
}
