//! Markdown chunking engine for Mordant.
//!
//! A lazy, low-copy chunking iterator over rushdown's AST, exposed to Python as
//! `MarkdownChunker`.
//!
//! DESIGN (corrected against the real rushdown API):
//! - `Node::pos()` returns an **absolute byte offset** into the source for every
//!   node kind — for `Text` it is `text::Index::start()`, and for block nodes the
//!   parser stores `Segment::start() + block_offset` (an absolute offset). It is
//!   NOT a line number, despite what `node.rs`'s `.line` getter currently claims.
//! - Block nodes expose no end/span, so the end of top-level block `i` is taken as
//!   the start of block `i + 1` (and `source.len()` for the last). Slicing the
//!   original source between consecutive top-level starts preserves the raw
//!   markdown (`#`, `-`, `>`, code fences), then we `trim_end()` the inter-block
//!   blank lines.
//! - The `Arena` is dropped immediately after extraction; only `(kind, start, end)`
//!   is retained. Peak memory ≈ source + ~24 bytes/top-level-node.
//! - Parsing runs without the GIL via `py.detach()` (the method this codebase
//!   already uses), so Python threads keep running during AST construction.
//! - `Cow` avoids Rust-side allocation for standalone blocks; only header+block
//!   concatenation pays for `format!`. One copy is unavoidable at the `PyString`
//!   FFI boundary.

use pyo3::prelude::*;
use pyo3::exceptions::{PyIOError, PyValueError};
use std::borrow::Cow;
use std::fs::File;

use memmap2::Mmap;

use rushdown_lib::ast::KindData;
use rushdown_lib::parser::Parser;
use rushdown_lib::text::BasicReader;

// -----------------------------------------------------------------------------
// 1. Internal representation
// -----------------------------------------------------------------------------

/// Discriminant for chunk-relevant top-level node kinds. Our own `Copy` enum so
/// we don't retain anything that borrows the arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChunkKind {
    Heading,
    Paragraph,
    CodeBlock,
    List,
    Table,
    Blockquote,
    Other,
}

/// Byte-offset metadata for one top-level node. `start..end` indexes the original
/// source and always falls on UTF-8 boundaries (block starts are line starts).
#[derive(Clone, Copy, Debug)]
struct NodeInfo {
    kind: ChunkKind,
    start: usize,
    end: usize,
}

/// Parse `source`, walk the top-level children of the Document node, and record
/// `(kind, start, end)` for each. `end` is the next top-level node's start, or
/// `source.len()` for the last node. The full `Arena` is dropped on return.
fn extract_nodes(source: &str) -> Vec<NodeInfo> {
    let parser = Parser::new(); // == Parser::with_options(Options::default())
    let mut reader = BasicReader::new(source);
    let (arena, doc_ref) = parser.parse(&mut reader);

    // Pass 1: collect (kind, start) for each top-level child, in document order.
    let mut starts: Vec<(ChunkKind, usize)> = Vec::new();
    let mut child = arena[doc_ref].first_child();
    while let Some(cref) = child {
        let node = &arena[cref];
        // Defensive: synthetic nodes may lack a source position; skip them.
        if let Some(start) = node.pos() {
            let kind = match node.kind_data() {
                KindData::Heading(_) => ChunkKind::Heading,
                KindData::Paragraph(_) => ChunkKind::Paragraph,
                KindData::CodeBlock(_) => ChunkKind::CodeBlock,
                KindData::List(_) => ChunkKind::List,
                KindData::Table(_) => ChunkKind::Table,
                KindData::Blockquote(_) => ChunkKind::Blockquote,
                // ThematicBreak, HtmlBlock, LinkReferenceDefinition, etc.
                _ => ChunkKind::Other,
            };
            starts.push((kind, start));
        }
        child = arena[cref].next_sibling();
    }

    // Pass 2: derive each node's end from the following node's start.
    let n = starts.len();
    let mut nodes = Vec::with_capacity(n);
    for i in 0..n {
        let (kind, start) = starts[i];
        let end = if i + 1 < n { starts[i + 1].1 } else { source.len() };
        nodes.push(NodeInfo { kind, start, end });
    }
    nodes
}

// -----------------------------------------------------------------------------
// 2. Text source abstraction
// -----------------------------------------------------------------------------

/// Owns the source as either a validated `String` (default, safe) or a
/// memory-mapped file (opt-in via `from_file_mmap`).
enum TextSource {
    Owned(String),
    /// `_file` is retained to keep the mapping valid (notably on Windows).
    Mapped { _file: File, mmap: Mmap },
}

impl TextSource {
    /// Reconstruct the `&str` view. O(1), allocation-free.
    fn as_str(&self) -> &str {
        match self {
            TextSource::Owned(s) => s.as_str(),
            // SAFETY: UTF-8 was validated once when the mapping was created.
            // Invariant (documented on `from_file_mmap`): the underlying file must
            // not be modified or truncated while this chunker is alive.
            TextSource::Mapped { mmap, .. } => unsafe {
                std::str::from_utf8_unchecked(&mmap[..])
            },
        }
    }
}

// -----------------------------------------------------------------------------
// 3. PyO3-exposed chunker
// -----------------------------------------------------------------------------

/// Python class `mordant.MarkdownChunker`.
///
/// Lazy iterator yielding one chunk (a `str`) at a time. A heading updates the
/// "current header" context; each subsequent top-level block is yielded either
/// standalone or prefixed with that header.
#[pyclass(module = "mordant", name = "MarkdownChunker")]
pub struct PyMarkdownChunker {
    text: TextSource,
    nodes: Vec<NodeInfo>,
    index: usize,
    /// Byte range (already trimmed) of the current heading context, if any.
    current_header: Option<(usize, usize)>,
}

#[pymethods]
impl PyMarkdownChunker {
    /// MarkdownChunker(text)
    ///
    /// Build a chunker from a Python string. Parses immediately; the GIL is
    /// released during parsing.
    #[new]
    fn from_string(py: Python<'_>, text: String) -> PyResult<Self> {
        let nodes = py.detach(|| extract_nodes(&text));
        Ok(Self {
            text: TextSource::Owned(text),
            nodes,
            index: 0,
            current_header: None,
        })
    }

    /// MarkdownChunker.from_file(path)
    ///
    /// Read `path`, validate UTF-8, and own the bytes as a `String` (safe path).
    /// The GIL is released during parsing.
    #[staticmethod]
    fn from_file(py: Python<'_>, path: &str) -> PyResult<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        let text = String::from_utf8(bytes)
            .map_err(|e| PyValueError::new_err(format!("Invalid UTF-8 in file: {e}")))?;
        let nodes = py.detach(|| extract_nodes(&text));
        Ok(Self {
            text: TextSource::Owned(text),
            nodes,
            index: 0,
            current_header: None,
        })
    }

    /// MarkdownChunker.from_file_mmap(path)
    ///
    /// Zero-copy variant that memory-maps `path` instead of copying it. UTF-8 is
    /// validated once up front; iteration then reads the mapping via
    /// `from_utf8_unchecked`.
    ///
    /// # Safety invariant
    /// The caller MUST NOT modify or truncate the file while this chunker is
    /// alive. Doing so is undefined behavior and can crash the process (SIGBUS).
    /// Use this only for trusted, immutable files; prefer `from_file` otherwise.
    #[staticmethod]
    fn from_file_mmap(py: Python<'_>, path: &str) -> PyResult<Self> {
        let file = File::open(path)
            .map_err(|e| PyIOError::new_err(e.to_string()))?;
        // SAFETY: mapping is read-only; see the documented invariant above.
        let mmap = unsafe {
            Mmap::map(&file).map_err(|e| PyIOError::new_err(e.to_string()))?
        };

        // Validate UTF-8 exactly once.
        std::str::from_utf8(&mmap[..])
            .map_err(|e| PyValueError::new_err(format!("Invalid UTF-8 in file: {e}")))?;

        // The borrow of `mmap` ends when `detach` returns, before `mmap` is moved
        // into the struct; `as_str()` re-derives the view from the stored mapping.
        let nodes = {
            // SAFETY: validated immediately above.
            let text = unsafe { std::str::from_utf8_unchecked(&mmap[..]) };
            py.detach(|| extract_nodes(text))
        };

        Ok(Self {
            text: TextSource::Mapped { _file: file, mmap },
            nodes,
            index: 0,
            current_header: None,
        })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Advance to the next block chunk, or `None` (→ `StopIteration`).
    fn __next__(&mut self) -> Option<String> {
        // Borrows `self.text` only; `self.index`/`self.current_header` are disjoint
        // fields, so mutating them below is allowed while `text` is held.
        let text = self.text.as_str();

        while self.index < self.nodes.len() {
            let node = self.nodes[self.index]; // Copy
            self.index += 1;

            // Raw source of this block, minus trailing inter-block blank lines.
            let raw = text[node.start..node.end].trim_end();

            match node.kind {
                ChunkKind::Heading => {
                    // Update context; headings are not yielded on their own.
                    // Store the trimmed range so the getter / concatenation are clean.
                    let end = node.start + raw.len();
                    self.current_header = Some((node.start, end));
                }
                ChunkKind::Paragraph
                | ChunkKind::CodeBlock
                | ChunkKind::List
                | ChunkKind::Table
                | ChunkKind::Blockquote => {
                    let chunk: Cow<str> = match self.current_header {
                        Some((h_start, h_end)) => {
                            Cow::Owned(format!("{}\n\n{}", &text[h_start..h_end], raw))
                        }
                        None => Cow::Borrowed(raw),
                    };
                    // One copy at the FFI boundary (PyO3 converts String → PyString).
                    return Some(chunk.into_owned());
                }
                ChunkKind::Other => {
                    // Thematic breaks, HTML blocks, link-reference defs, etc.
                    // Ignored, and they do NOT reset the heading context.
                }
            }
        }
        None
    }

    /// The current heading context (last top-level heading seen), or `None`.
    #[getter]
    fn current_header(&self) -> Option<String> {
        let text = self.text.as_str();
        self.current_header.map(|(s, e)| text[s..e].to_string())
    }

    /// Number of top-level nodes extracted (with a source position).
    #[getter]
    fn node_count(&self) -> usize {
        self.nodes.len()
    }
}
