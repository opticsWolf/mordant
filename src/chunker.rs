//! Markdown chunking engine for mordant (core).
//!
//! A lazy, low-copy chunking iterator over mordant's AST. Pure-Rust engine;
//! the PyO3 classes (`ExtractedChunk`, `MarkdownChunker`) live in
//! mordant-py/src/chunker.rs and delegate to [`MarkdownChunker`].
//!
//! DESIGN (corrected against the real mordant API):
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

use crate::ast::KindData;
use crate::parser::{NoParserOptions, Parser, ParserExtension, ParserExtensionFn, TableAstTransformer, TableParagraphTransformer};
use crate::text::BasicReader;

use std::fs::File;

use memmap2::Mmap;

// -----------------------------------------------------------------------------
// 1. Internal representation
// -----------------------------------------------------------------------------

/// Block type discriminator for chunk-relevant top-level node kinds.
/// Public so hosts can compare `block_type` strings deterministically.
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
        crate::parser::Options::default(),
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
/// memory-mapped file (opt-in via [`TextSource::from_mmap_path`]).
pub enum TextSource {
    Owned(String),
    /// The `File` is retained to keep the mapping valid (notably on Windows).
    Mapped { file: File, mmap: Mmap },
}

/// Error type for chunker construction from files.
#[derive(Debug)]
pub enum ChunkerError {
    Io(std::io::Error),
    InvalidUtf8(String),
}

impl std::fmt::Display for ChunkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChunkerError::Io(e) => write!(f, "{e}"),
            ChunkerError::InvalidUtf8(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ChunkerError {}

impl TextSource {
    /// Own the source as a validated `String`.
    pub fn owned(text: String) -> Self {
        TextSource::Owned(text)
    }

    /// Memory-map `path` instead of copying it.
    ///
    /// # Safety invariant (for [`TextSource::str`])
    /// The caller MUST NOT modify or truncate the file while the returned
    /// source (or any chunker built from it) is alive.
    pub fn from_mmap_path(path: &str) -> Result<Self, ChunkerError> {
        let file = File::open(path).map_err(ChunkerError::Io)?;
        // SAFETY: mapping is read-only; see the documented invariant above.
        let mmap = unsafe { Mmap::map(&file) }.map_err(ChunkerError::Io)?;
        Ok(TextSource::Mapped { file, mmap })
    }

    /// Reconstruct the `&str` view, validating UTF-8. One pass over the bytes;
    /// allocation-free for the `Owned` variant.
    pub fn str_checked(&self) -> Result<&str, ChunkerError> {
        match self {
            TextSource::Owned(s) => Ok(s.as_str()),
            TextSource::Mapped { mmap, .. } => std::str::from_utf8(&mmap[..])
                .map_err(|e| ChunkerError::InvalidUtf8(format!("Invalid UTF-8 in file: {e}"))),
        }
    }
}

// -----------------------------------------------------------------------------
// 3. Chunker engine
// -----------------------------------------------------------------------------

/// A single extracted block with metadata.
#[derive(Clone, Debug)]
pub struct Chunk {
    /// The block content with trailing whitespace trimmed.
    pub text: String,
    /// The block type discriminator.
    pub block_type: BlockType,
    /// Byte offset in original source (inclusive).
    pub start_offset: usize,
    /// Byte offset in original source (exclusive).
    pub end_offset: usize,
}

/// The source text as `&str` (free fn so callers only borrow the source,
/// keeping field-disjoint borrows of the chunker legal).
fn source_str(source: &TextSource) -> &str {
    // SAFETY: UTF-8 was validated when the chunker was constructed.
    match source {
        TextSource::Owned(s) => s.as_str(),
        TextSource::Mapped { mmap, .. } => unsafe {
            std::str::from_utf8_unchecked(&mmap[..])
        },
    }
}

/// Lazy chunking iterator over the markdown AST.
///
/// A heading updates the "current header" context; each subsequent top-level
/// body block is yielded as its own chunk.
pub struct MarkdownChunker {
    source: TextSource,
    nodes: Vec<NodeInfo>,
    index: usize,
    /// Byte range (already trimmed) of the current heading context, if any.
    current_header: Option<(usize, usize)>,
}

impl MarkdownChunker {
    /// Build a chunker from an owned string. Parses immediately.
    pub fn new(text: String) -> Self {
        let nodes = extract_nodes(&text);
        Self {
            source: TextSource::Owned(text),
            nodes,
            index: 0,
            current_header: None,
        }
    }

    /// Read `path`, validate UTF-8, and own the bytes as a `String` (safe path).
    pub fn from_file(path: &str) -> Result<Self, ChunkerError> {
        let bytes = std::fs::read(path).map_err(ChunkerError::Io)?;
        let text = String::from_utf8(bytes).map_err(|e| {
            ChunkerError::InvalidUtf8(format!("Invalid UTF-8 in file: {e}"))
        })?;
        Ok(Self::new(text))
    }

    /// Build a chunker from any [`TextSource`] (e.g. a memory-mapped file).
    /// Validates UTF-8 once up front.
    pub fn from_text_source(source: TextSource) -> Result<Self, ChunkerError> {
        // Validate before storing so later unchecked-style access stays sound.
        source.str_checked()?;
        let nodes = {
            let text = source.str_checked()?;
            extract_nodes(text)
        };
        Ok(Self {
            source,
            nodes,
            index: 0,
            current_header: None,
        })
    }

    /// The source text as `&str`.
    fn text(&self) -> &str {
        source_str(&self.source)
    }

    /// Advance to the next block chunk, or `None`.
    ///
    /// Yields bare `str` chunks — no heading prefixing. Headings update
    /// `current_header` but are NOT yielded. Other nodes (thematic breaks,
    /// etc.) are skipped.
    pub fn next_chunk(&mut self) -> Option<String> {
        let text = source_str(&self.source);

        while self.index < self.nodes.len() {
            let node = self.nodes[self.index]; // Copy
            self.index += 1;

            // Raw source of this block, minus trailing inter-block blank lines.
            let raw = text[node.start..node.end].trim_end();

            match node.block_type {
                BlockType::Heading => {
                    // Update heading context for embed-time injection.
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
    pub fn current_header(&self) -> Option<String> {
        let text = source_str(&self.source);
        self.current_header.map(|(s, e)| text[s..e].to_string())
    }

    /// Number of top-level nodes extracted (with a source position).
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get all chunks (no heading prefix).
    ///
    /// Headings are NOT included (they are context, not content).
    /// Other nodes (thematic breaks, etc.) are skipped.
    pub fn get_chunks(&mut self) -> Vec<Chunk> {
        let text = source_str(&self.source);
        let mut chunks = Vec::new();

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
                    chunks.push(Chunk {
                        text: raw.to_string(),
                        block_type: node.block_type,
                        start_offset: node.start,
                        end_offset: node.start + raw.len(),
                    });
                }
                BlockType::Other => {
                    // Skipped.
                }
            }
        }
        chunks
    }

    /// Get all chunks WITH heading context prefix.
    ///
    /// Each body chunk is prefixed with the current heading context
    /// (e.g., "# Title\n\nParagraph text"). Headings themselves are NOT yielded.
    pub fn get_chunks_with_context(&mut self) -> Vec<Chunk> {
        let text = source_str(&self.source);
        let mut chunks = Vec::new();

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

                    chunks.push(Chunk {
                        text: text_with_context,
                        block_type: node.block_type,
                        start_offset: node.start,
                        end_offset: node.start + raw.len(),
                    });
                }
                BlockType::Other => {
                    // Skipped.
                }
            }
        }
        chunks
    }

    /// Get all node types (including headings) as chunks.
    ///
    /// Unlike [`MarkdownChunker::get_chunks`], this includes Heading blocks and
    /// yields them with `block_type="Heading"`. Other nodes are skipped.
    pub fn get_all_chunks(&mut self) -> Vec<Chunk> {
        let text = source_str(&self.source);
        let mut chunks = Vec::new();

        for node in &self.nodes {
            match node.block_type {
                BlockType::Heading => {
                    let raw = text[node.start..node.end].trim_end();
                    let end = node.start + raw.len();
                    self.current_header = Some((node.start, end));
                    chunks.push(Chunk {
                        text: raw.to_string(),
                        block_type: BlockType::Heading,
                        start_offset: node.start,
                        end_offset: node.start + raw.len(),
                    });
                }
                BlockType::Paragraph
                | BlockType::CodeBlock
                | BlockType::List
                | BlockType::Table
                | BlockType::Blockquote
                | BlockType::Diagram => {
                    let raw = text[node.start..node.end].trim_end();
                    chunks.push(Chunk {
                        text: raw.to_string(),
                        block_type: node.block_type,
                        start_offset: node.start,
                        end_offset: node.start + raw.len(),
                    });
                }
                BlockType::Other => {
                    // Skipped.
                }
            }
        }
        chunks
    }

    /// Get all chunks as bare `String`s (equivalent to iterating directly).
    pub fn get_bare_chunks(&mut self) -> Vec<String> {
        let text = source_str(&self.source);
        let mut chunks = Vec::new();

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
                    chunks.push(raw.to_string());
                }
                BlockType::Other => {
                    // Skipped.
                }
            }
        }
        chunks
    }

    /// Compute overlap payloads for embedding.
    ///
    /// The tail of chunk N is prepended to chunk N+1 to maintain context across
    /// chunk boundaries. Overlap is never stored — it is purely a query/embed
    /// time transformation.
    ///
    /// Returns `(chunk_id, overlapped_text)` pairs.
    pub fn compute_overlap_payloads(&mut self, overlap_words: usize) -> Vec<(String, String)> {
        let text = source_str(&self.source);
        let mut payloads = Vec::new();

        let mut prev_tail: String = String::new();
        let mut chunk_index = 0usize;

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

                    payloads.push((format!("chunk:{}", chunk_index), embed_text));

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
        payloads
    }

    /// Get the delimiter string between two consecutive block types.
    ///
    /// Used for document reconstruction:
    ///   - List → List: "\n" (single newline, items belong together)
    ///   - Blockquote → Blockquote: "\n> " (re-attach quote marker)
    ///   - Everything else: "\n\n" (paragraph break)
    pub fn get_delimiter(prev: &str, curr: &str) -> String {
        if prev == "List" && curr == "List" {
            "\n".to_string()
        } else if prev == "Blockquote" && curr == "Blockquote" {
            "\n> ".to_string()
        } else {
            "\n\n".to_string()
        }
    }
}

/// Construct a [`BlockType`] from its string form (used by host bindings that
/// build [`Chunk`] values from dynamically-typed input).
pub fn block_type_from_str(s: &str) -> BlockType {
    parse_block_type(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_chunking() {
        let src = "# Title\n\nPara one.\n\n```rust\ncode\n```\n\n- a\n- b\n";
        let mut c = MarkdownChunker::new(src.to_string());
        assert!(c.node_count() >= 4);
        let chunks = c.get_chunks();
        let types: Vec<&str> = chunks.iter().map(|k| k.block_type.as_str()).collect();
        assert!(!types.contains(&"Heading"));
        assert!(types.contains(&"Paragraph"));
        assert!(types.contains(&"CodeBlock"));
        assert!(types.contains(&"List"));
    }

    #[test]
    fn get_all_chunks_includes_heading() {
        let src = "# Title\n\nBody.\n";
        let mut c = MarkdownChunker::new(src.to_string());
        let all = c.get_all_chunks();
        assert_eq!(all[0].block_type.as_str(), "Heading");
        assert_eq!(all[0].text, "# Title");
    }

    #[test]
    fn context_prefix() {
        let src = "# H\n\nBody text.\n";
        let mut c = MarkdownChunker::new(src.to_string());
        let chunks = c.get_chunks_with_context();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "# H\n\nBody text.");
    }

    #[test]
    fn current_header_tracks_headings() {
        let src = "# First\n\npara\n\n## Second\n\npara2\n";
        let mut c = MarkdownChunker::new(src.to_string());
        let _ = c.next_chunk(); // para under First
        assert_eq!(c.current_header().as_deref(), Some("# First"));
        let _ = c.next_chunk(); // para under Second
        assert_eq!(c.current_header().as_deref(), Some("## Second"));
    }

    #[test]
    fn bare_iteration_matches_get_bare_chunks() {
        let src = "Intro\n\n# H\n\nA\n\nB\n";
        let mut c1 = MarkdownChunker::new(src.to_string());
        let mut iterated = Vec::new();
        while let Some(s) = c1.next_chunk() {
            iterated.push(s);
        }
        let mut c2 = MarkdownChunker::new(src.to_string());
        assert_eq!(iterated, c2.get_bare_chunks());
        assert_eq!(iterated, vec!["Intro".to_string(), "A".to_string(), "B".to_string()]);
    }

    #[test]
    fn delimiters() {
        assert_eq!(MarkdownChunker::get_delimiter("List", "List"), "\n");
        assert_eq!(MarkdownChunker::get_delimiter("Blockquote", "Blockquote"), "\n> ");
        assert_eq!(MarkdownChunker::get_delimiter("Paragraph", "List"), "\n\n");
    }

    #[test]
    fn overlap_payloads() {
        let src = "one two three four\n\nfive six\n";
        let mut c = MarkdownChunker::new(src.to_string());
        let payloads = c.compute_overlap_payloads(2);
        assert_eq!(payloads.len(), 2);
        assert_eq!(payloads[0].0, "chunk:0");
        assert_eq!(payloads[0].1, "one two three four");
        assert!(payloads[1].1.starts_with("three  four\n\nfive six"));
    }

    #[test]
    fn block_type_roundtrip() {
        assert_eq!(block_type_from_str("Heading"), BlockType::Heading);
        assert_eq!(block_type_from_str("Diagram"), BlockType::Diagram);
        assert_eq!(block_type_from_str("whatever"), BlockType::Other);
    }

    #[test]
    fn offsets_index_original_source() {
        let src = "# T\n\nHello world.\n";
        let mut c = MarkdownChunker::new(src.to_string());
        let chunks = c.get_chunks();
        let ch = &chunks[0];
        assert_eq!(&src[ch.start_offset..ch.end_offset], "Hello world.");
    }
}
