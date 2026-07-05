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
use pyo3::Bound;
use pyo3::types::{PyDict, PyList};
use pyo3::exceptions::{PyIOError, PyValueError};
use std::borrow::Cow;
use std::fs::File;

use memmap2::Mmap;

use rushdown_lib::ast::KindData;
use rushdown_lib::parser::{NoParserOptions, Parser, ParserExtension, ParserExtensionFn, TableAstTransformer, TableParagraphTransformer};
use rushdown_lib::text::BasicReader;

// -----------------------------------------------------------------------------
// 1. Internal representation
// -----------------------------------------------------------------------------

/// Block type discriminator for chunk-relevant top-level node kinds.
/// Public so Python can compare `block_type` strings deterministically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockType {
    Heading,
    Paragraph,
    CodeBlock,
    List,
    Table,
    Blockquote,
    Diagram,
    Other,
}

impl BlockType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockType::Heading => "Heading",
            BlockType::Paragraph => "Paragraph",
            BlockType::CodeBlock => "CodeBlock",
            BlockType::List => "List",
            BlockType::Table => "Table",
            BlockType::Blockquote => "Blockquote",
            BlockType::Diagram => "Diagram",
            BlockType::Other => "Other",
        }
    }
}

/// Parse `block_type_str` to a `BlockType`.
fn parse_block_type(s: &str) -> BlockType {
    match s {
        "Heading" => BlockType::Heading,
        "Paragraph" => BlockType::Paragraph,
        "CodeBlock" => BlockType::CodeBlock,
        "List" => BlockType::List,
        "Table" => BlockType::Table,
        "Blockquote" => BlockType::Blockquote,
        "Diagram" => BlockType::Diagram,
        _ => BlockType::Other,
    }
}

/// Byte-offset metadata for one top-level node. `start..end` indexes the original
/// source and always falls on UTF-8 boundaries (block starts are line starts).
#[derive(Clone, Copy, Debug)]
struct NodeInfo {
    block_type: BlockType,
    start: usize,
    end: usize,
}

/// Parse `source`, walk the top-level children of the Document node, and record
/// `(block_type, start, end)` for each. `end` is the next top-level node's start, or
/// `source.len()` for the last node. The full `Arena` is dropped on return.
fn extract_nodes(source: &str) -> Vec<NodeInfo> {
    // Build parser with GFM table transformers + diagram extension
    // so that tables and mermaid diagrams are correctly classified.
    let gfm_ext = ParserExtensionFn::new(|p: &mut Parser| {
        p.add_ast_transformer(TableAstTransformer::new, NoParserOptions, 0);
        p.add_paragraph_transformer(TableParagraphTransformer::new, NoParserOptions, 200);
    });

    // Diagram extension (mermaid → Diagram node)
    let diagram_ext = crate::diagram::diagram_parser_extension(
        crate::diagram::DiagramParserOptions::default(),
    );

    let parser = Parser::with_extensions(
        rushdown_lib::parser::Options::default(),
        gfm_ext.and(diagram_ext),
    );
    let mut reader = BasicReader::new(source);
    let (arena, doc_ref) = parser.parse(&mut reader);

    // Pass 1: collect (block_type, start) for each top-level child, in document order.
    let mut starts: Vec<(BlockType, usize)> = Vec::new();
    let mut child = arena[doc_ref].first_child();
    while let Some(cref) = child {
        let node = &arena[cref];
        // Defensive: synthetic nodes may lack a source position; skip them.
        if let Some(start) = node.pos() {
            let block_type = match node.kind_data() {
                KindData::Heading(_) => BlockType::Heading,
                KindData::Paragraph(_) => BlockType::Paragraph,
                KindData::CodeBlock(_) => BlockType::CodeBlock,
                KindData::List(_) => BlockType::List,
                KindData::Table(_) => BlockType::Table,
                KindData::Blockquote(_) => BlockType::Blockquote,
                KindData::Extension(ref d) => {
                    // Check if the extension is a Diagram
                    if (d.as_ref() as &dyn std::any::Any).is::<crate::diagram::Diagram>() {
                        BlockType::Diagram
                    } else {
                        BlockType::Other
                    }
                }
                // ThematicBreak, HtmlBlock, LinkReferenceDefinition, etc.
                _ => BlockType::Other,
            };
            starts.push((block_type, start));
        }
        child = arena[cref].next_sibling();
    }

    // Pass 2: derive each node's end from the following node's start.
    let n = starts.len();
    let mut nodes = Vec::with_capacity(n);
    for i in 0..n {
        let (block_type, start) = starts[i];
        let end = if i + 1 < n { starts[i + 1].1 } else { source.len() };
        nodes.push(NodeInfo { block_type, start, end });
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

/// Python class `mordant.ExtractedChunk`.
///
/// Represents a single extracted block from the markdown AST with metadata.
#[pyclass(module = "mordant", name = "ExtractedChunk")]
pub struct PyExtractedChunk {
    /// The block content with trailing whitespace trimmed.
    pub text: String,
    /// The block type discriminator.
    pub block_type: BlockType,
    /// Byte offset in original source (inclusive).
    pub start_offset: usize,
    /// Byte offset in original source (exclusive).
    pub end_offset: usize,
}

impl PyExtractedChunk {
    /// Factory for Rust-side construction.
    pub fn from_strings(
        text: String,
        block_type: String,
        start_offset: usize,
        end_offset: usize,
    ) -> Self {
        Self {
            text,
            block_type: parse_block_type(&block_type),
            start_offset,
            end_offset,
        }
    }
}

#[pymethods]
impl PyExtractedChunk {
    /// Create an ExtractedChunk from Python.
    #[new]
    fn new(
        text: String,
        block_type: String,
        start_offset: usize,
        end_offset: usize,
    ) -> Self {
        Self::from_strings(text, block_type, start_offset, end_offset)
    }

    /// Create an ExtractedChunk from Python (static alias).
    #[staticmethod]
    fn create(
        text: String,
        block_type: String,
        start_offset: usize,
        end_offset: usize,
    ) -> Self {
        Self::from_strings(text, block_type, start_offset, end_offset)
    }

    #[getter]
    fn text(&self) -> String {
        self.text.clone()
    }

    #[getter]
    fn block_type(&self) -> String {
        self.block_type.as_str().to_string()
    }

    #[getter]
    fn start_offset(&self) -> usize {
        self.start_offset
    }

    #[getter]
    fn end_offset(&self) -> usize {
        self.end_offset
    }

    fn __repr__(&self) -> String {
        format!(
            "ExtractedChunk(block_type='{}', text={:?}, start={}, end={})",
            self.block_type.as_str(), self.text, self.start_offset, self.end_offset
        )
    }
}

/// Python class `mordant.MarkdownChunker`.
///
/// Lazy iterator yielding one chunk (an `ExtractedChunk`) at a time.
/// A heading updates the "current header" context; each subsequent top-level
/// block is yielded as its own `ExtractedChunk`.
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
    ///
    /// Yields bare `str` chunks — no heading prefixing.
    /// Headings update `current_header` but are NOT yielded.
    /// Body blocks are yielded as their raw content (no heading context prepended).
    /// Other nodes (thematic breaks, etc.) are skipped.
    fn __next__(&mut self) -> Option<String> {
        let text = self.text.as_str();

        while self.index < self.nodes.len() {
            let node = self.nodes[self.index]; // Copy
            self.index += 1;

            // Raw source of this block, minus trailing inter-block blank lines.
            let raw = text[node.start..node.end].trim_end();

            match node.block_type {
                BlockType::Heading => {
                    // Update heading context for OKF's embed-time injection.
                    // Headings are NOT yielded — they are context, not content.
                    let end = node.start + raw.len();
                    self.current_header = Some((node.start, end));
                }
                BlockType::Paragraph
                | BlockType::CodeBlock
                | BlockType::List
                | BlockType::Table
                | BlockType::Blockquote
                | BlockType::Diagram => {
                    // Yield the body block as its own bare chunk (no heading prefix).
                    // OKF injects heading context at embed time for better embeddings.
                    return Some(raw.to_string());
                }
                BlockType::Other => {
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

    /// Get all chunks as `ExtractedChunk` objects (no heading prefix).
    ///
    /// Returns a list of `ExtractedChunk` with `text`, `block_type`,
    /// `start_offset`, and `end_offset` for each bare chunk.
    /// Headings are NOT included (they are context, not content).
    /// Other nodes (thematic breaks, etc.) are skipped.
    fn get_chunks<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let text = self.text.as_str();
        let chunks = PyList::empty(py);

        for node in &self.nodes {
            match node.block_type {
                BlockType::Heading => {
                    // Update heading context; headings are not yielded.
                    let raw = text[node.start..node.end].trim_end();
                    let end = node.start + raw.len();
                    self.current_header = Some((node.start, end));
                }
                BlockType::Paragraph
                | BlockType::CodeBlock
                | BlockType::List
                | BlockType::Table
                | BlockType::Blockquote
                | BlockType::Diagram => {
                    let raw = text[node.start..node.end].trim_end();
                    let chunk = PyExtractedChunk::from_strings(
                        raw.to_string(),
                        node.block_type.as_str().to_string(),
                        node.start,
                        node.start + raw.len(),
                    );
                    let item = Py::new(py, chunk)?;
                    chunks.append(item)?;
                }
                BlockType::Other => {
                    // Skipped.
                }
            }
        }
        Ok(chunks)
    }

    /// Get all chunks as `ExtractedChunk` objects WITH heading context prefix.
    ///
    /// Each body chunk is prefixed with the current heading context
    /// (e.g., "# Title\n\nParagraph text"). Headings themselves are
    /// NOT yielded.
    fn get_chunks_with_context<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let text = self.text.as_str();
        let chunks = PyList::empty(py);

        for node in &self.nodes {
            match node.block_type {
                BlockType::Heading => {
                    let raw = text[node.start..node.end].trim_end();
                    let end = node.start + raw.len();
                    self.current_header = Some((node.start, end));
                }
                BlockType::Paragraph
                | BlockType::CodeBlock
                | BlockType::List
                | BlockType::Table
                | BlockType::Blockquote
                | BlockType::Diagram => {
                    let raw = text[node.start..node.end].trim_end();

                    // Build prefixed text if there's a heading context.
                    let text_with_context: String = match self.current_header {
                        Some((h_start, h_end)) => {
                            format!("{}\n\n{}", &text[h_start..h_end], raw)
                        }
                        None => raw.to_string(),
                    };

                    let chunk = PyExtractedChunk::from_strings(
                        text_with_context,
                        node.block_type.as_str().to_string(),
                        node.start,
                        node.start + raw.len(),
                    );
                    let item = Py::new(py, chunk)?;
                    chunks.append(item)?;
                }
                BlockType::Other => {
                    // Skipped.
                }
            }
        }
        Ok(chunks)
    }

    /// Get all node types (including headings and other) as `ExtractedChunk` objects.
    ///
    /// Unlike `get_chunks()`, this includes Heading blocks and yields them
    /// as separate chunks with `block_type="Heading"`. Other nodes
    /// (thematic breaks, etc.) are skipped.
    fn get_all_chunks<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let text = self.text.as_str();
        let chunks = PyList::empty(py);

        for node in &self.nodes {
            match node.block_type {
                BlockType::Heading => {
                    let raw = text[node.start..node.end].trim_end();
                    let end = node.start + raw.len();
                    self.current_header = Some((node.start, end));
                    let chunk = PyExtractedChunk::from_strings(
                        raw.to_string(),
                        "Heading".to_string(),
                        node.start,
                        node.start + raw.len(),
                    );
                    let item = Py::new(py, chunk)?;
                    chunks.append(item)?;
                }
                BlockType::Paragraph
                | BlockType::CodeBlock
                | BlockType::List
                | BlockType::Table
                | BlockType::Blockquote
                | BlockType::Diagram => {
                    let raw = text[node.start..node.end].trim_end();
                    let chunk = PyExtractedChunk::from_strings(
                        raw.to_string(),
                        node.block_type.as_str().to_string(),
                        node.start,
                        node.start + raw.len(),
                    );
                    let item = Py::new(py, chunk)?;
                    chunks.append(item)?;
                }
                BlockType::Other => {
                    // Skipped.
                }
            }
        }
        Ok(chunks)
    }

    /// Get all chunks as bare `str` (backward compatible with existing iterator).
    ///
    /// This is equivalent to iterating the chunker directly.
    fn get_bare_chunks<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyList> {
        let text = self.text.as_str();
        let chunks = PyList::empty(py);

        for node in &self.nodes {
            match node.block_type {
                BlockType::Heading => {
                    let raw = text[node.start..node.end].trim_end();
                    let end = node.start + raw.len();
                    self.current_header = Some((node.start, end));
                }
                BlockType::Paragraph
                | BlockType::CodeBlock
                | BlockType::List
                | BlockType::Table
                | BlockType::Blockquote
                | BlockType::Diagram => {
                    let raw = text[node.start..node.end].trim_end();
                    chunks.append(raw.to_string()).unwrap();
                }
                BlockType::Other => {
                    // Skipped.
                }
            }
        }
        chunks
    }

    /// Get the delimiter string between two consecutive block types.
    ///
    /// Used for document reconstruction:
    ///   - List → List: "\n" (single newline, items belong together)
    ///   - Blockquote → Blockquote: "\n> " (re-attach quote marker)
    ///   - Everything else: "\n\n" (paragraph break)
    #[staticmethod]
    fn get_delimiter(prev: &str, curr: &str) -> String {
        if prev == "List" && curr == "List" {
            "\n".to_string()
        } else if prev == "Blockquote" && curr == "Blockquote" {
            "\n> ".to_string()
        } else {
            "\n\n".to_string()
        }
    }

    /// Compute overlap payloads for embedding.
    ///
    /// The tail of chunk N is prepended to chunk N+1 to maintain
    /// context across chunk boundaries. Overlap is never stored —
    /// it is purely a query/embed time transformation.
    ///
    /// Args:
    ///     overlap_words: Number of words from the tail of each chunk
    ///                    to prepend to the next chunk.
    ///
    /// Returns:
    ///     List of dicts with keys:
    ///       - chunk_id: "chunk:N"
    ///       - text: overlapped text for embedding
    fn compute_overlap_payloads<'py>(&mut self, py: Python<'py>, overlap_words: usize) -> PyResult<Bound<'py, PyList>> {
        let text = self.text.as_str();
        let payloads = PyList::empty(py);

        let mut prev_tail: String = String::new();
        let mut chunk_index = 0;

        for node in &self.nodes {
            match node.block_type {
                BlockType::Heading => {
                    let raw = text[node.start..node.end].trim_end();
                    let end = node.start + raw.len();
                    self.current_header = Some((node.start, end));
                }
                BlockType::Paragraph
                | BlockType::CodeBlock
                | BlockType::List
                | BlockType::Table
                | BlockType::Blockquote
                | BlockType::Diagram => {
                    let raw = text[node.start..node.end].trim_end();

                    // Build overlapped text.
                    let embed_text: String = if !prev_tail.is_empty() {
                        format!("{}\n\n{}", prev_tail, raw)
                    } else {
                        raw.to_string()
                    };

                    // Build chunk_id.
                    let chunk_id = format!("chunk:{}", chunk_index);

                    // Create payload dict.
                    let payload_dict = PyDict::new(py);
                    payload_dict.set_item(chunk_id, embed_text)?;

                    payloads.append(payload_dict)?;

                    // Compute tail from PURE chunk text (not the overlapped text).
                    let words: Vec<&str> = raw.split_whitespace().collect();
                    if overlap_words > 0 && !words.is_empty() {
                        let tail_start = if words.len() > overlap_words {
                            words.len() - overlap_words
                        } else {
                            0
                        };
                        prev_tail = words[tail_start..].join("  ");
                    } else {
                        prev_tail = String::new();
                    }

                    chunk_index += 1;
                }
                BlockType::Other => {
                    // Skipped.
                }
            }
        }
        Ok(payloads)
    }
}
