//! PyO3 bindings for the chunking engine (moved to core `mordant::chunker`).
//!
//! The pure engine lives behind the `chunker` feature of the core crate; this
//! module re-exports it and keeps the Python-exposed `ExtractedChunk` and
//! `MarkdownChunker` classes, releasing the GIL around parsing.

use pyo3::prelude::*;
use pyo3::Bound;
use pyo3::types::{PyDict, PyList};
use pyo3::exceptions::{PyIOError, PyValueError};

pub use mordant_lib::chunker::{
    BlockType, Chunk, ChunkerError, MarkdownChunker as CoreMarkdownChunker, TextSource,
    block_type_from_str,
};

fn map_chunker_error(e: ChunkerError) -> PyErr {
    match e {
        ChunkerError::Io(err) => PyIOError::new_err(err.to_string()),
        ChunkerError::InvalidUtf8(msg) => PyValueError::new_err(msg),
    }
}

/// Python class `mordant.ExtractedChunk`.
///
/// Represents a single extracted block from the markdown AST with metadata.
#[pyclass(module = "mordant", name = "ExtractedChunk")]
pub struct PyExtractedChunk {
    inner: Chunk,
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
            inner: Chunk {
                text,
                block_type: block_type_from_str(&block_type),
                start_offset,
                end_offset,
            },
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
        self.inner.text.clone()
    }

    #[getter]
    fn block_type(&self) -> String {
        self.inner.block_type.as_str().to_string()
    }

    #[getter]
    fn start_offset(&self) -> usize {
        self.inner.start_offset
    }

    #[getter]
    fn end_offset(&self) -> usize {
        self.inner.end_offset
    }

    fn __repr__(&self) -> String {
        format!(
            "ExtractedChunk(block_type='{}', text={:?}, start={}, end={})",
            self.inner.block_type.as_str(), self.inner.text,
            self.inner.start_offset, self.inner.end_offset
        )
    }
}

/// Convert core `Chunk`s into a list of `ExtractedChunk`.
fn chunks_to_pylist<'py>(py: Python<'py>, chunks: Vec<Chunk>) -> PyResult<Bound<'py, PyList>> {
    let list = PyList::empty(py);
    for chunk in chunks {
        let item = Py::new(py, PyExtractedChunk { inner: chunk })?;
        list.append(item)?;
    }
    Ok(list)
}

/// Python class `mordant.MarkdownChunker`.
///
/// Lazy iterator yielding one chunk (an `ExtractedChunk`) at a time.
/// A heading updates the "current header" context; each subsequent top-level
/// block is yielded as its own `ExtractedChunk`.
#[pyclass(module = "mordant", name = "MarkdownChunker")]
pub struct PyMarkdownChunker {
    inner: CoreMarkdownChunker,
}

#[pymethods]
impl PyMarkdownChunker {
    /// MarkdownChunker(text)
    ///
    /// Build a chunker from a Python string. Parses immediately; the GIL is
    /// released during parsing.
    #[new]
    fn from_string(py: Python<'_>, text: String) -> PyResult<Self> {
        let inner = py.detach(move || CoreMarkdownChunker::new(text));
        Ok(Self { inner })
    }

    /// MarkdownChunker.from_file(path)
    ///
    /// Read `path`, validate UTF-8, and own the bytes as a `String` (safe path).
    /// The GIL is released during parsing.
    #[staticmethod]
    fn from_file(py: Python<'_>, path: &str) -> PyResult<Self> {
        // File I/O happens on the calling thread; only AST construction is detached.
        let bytes = std::fs::read(path).map_err(|e| PyIOError::new_err(e.to_string()))?;
        let text = String::from_utf8(bytes)
            .map_err(|e| PyValueError::new_err(format!("Invalid UTF-8 in file: {e}")))?;
        let inner = py.detach(move || CoreMarkdownChunker::new(text));
        Ok(Self { inner })
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
        let source = TextSource::from_mmap_path(path).map_err(map_chunker_error)?;
        let inner = py
            .detach(move || CoreMarkdownChunker::from_text_source(source))
            .map_err(map_chunker_error)?;
        Ok(Self { inner })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Advance to the next block chunk, or `None` (→ `StopIteration`).
    fn __next__(&mut self) -> Option<String> {
        self.inner.next_chunk()
    }

    /// The current heading context (last top-level heading seen), or `None`.
    #[getter]
    fn current_header(&self) -> Option<String> {
        self.inner.current_header()
    }

    /// Number of top-level nodes extracted (with a source position).
    #[getter]
    fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Get all chunks as `ExtractedChunk` objects (no heading prefix).
    fn get_chunks<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        chunks_to_pylist(py, self.inner.get_chunks())
    }

    /// Get all chunks as `ExtractedChunk` objects WITH heading context prefix.
    fn get_chunks_with_context<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        chunks_to_pylist(py, self.inner.get_chunks_with_context())
    }

    /// Get all node types (including headings and other) as `ExtractedChunk` objects.
    fn get_all_chunks<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        chunks_to_pylist(py, self.inner.get_all_chunks())
    }

    /// Get all chunks as bare `str` (backward compatible with existing iterator).
    fn get_bare_chunks<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyList> {
        let list = PyList::empty(py);
        for s in self.inner.get_bare_chunks() {
            list.append(s).unwrap();
        }
        list
    }

    /// Get the delimiter string between two consecutive block types.
    #[staticmethod]
    fn get_delimiter(prev: &str, curr: &str) -> String {
        CoreMarkdownChunker::get_delimiter(prev, curr)
    }

    /// Compute overlap payloads for embedding.
    ///
    /// Returns a list of dicts with keys:
    ///   - chunk_id: "chunk:N"
    ///   - text: overlapped text for embedding
    fn compute_overlap_payloads<'py>(
        &mut self,
        py: Python<'py>,
        overlap_words: usize,
    ) -> PyResult<Bound<'py, PyList>> {
        let payloads = PyList::empty(py);
        for (chunk_id, embed_text) in self.inner.compute_overlap_payloads(overlap_words) {
            let payload_dict = PyDict::new(py);
            payload_dict.set_item(chunk_id, embed_text)?;
            payloads.append(payload_dict)?;
        }
        Ok(payloads)
    }
}
