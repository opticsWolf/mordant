"""Tests for OKF Graph-specific chunker methods."""

import mordant


# === get_chunks() ===

def test_get_chunks_returns_extracted_chunks():
    """get_chunks() returns a list of ExtractedChunk objects."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara one\n\n## Sub\n\nPara two")
    chunks = chunker.get_chunks()
    assert len(chunks) == 2
    for c in chunks:
        assert isinstance(c, mordant.ExtractedChunk)
        assert hasattr(c, "text")
        assert hasattr(c, "block_type")
        assert hasattr(c, "start_offset")
        assert hasattr(c, "end_offset")


def test_get_chunks_bare_no_prefix():
    """get_chunks() returns bare chunks (no heading prefix)."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara one")
    chunks = chunker.get_chunks()
    assert len(chunks) == 1
    assert chunks[0].text == "Para one"
    assert not chunks[0].text.startswith("# Title")


def test_get_chunks_skips_headings():
    """get_chunks() does NOT include heading chunks."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara one\n\n## Sub\n\nPara two")
    chunks = chunker.get_chunks()
    block_types = [c.block_type for c in chunks]
    assert "Heading" not in block_types
    assert all(bt == "Paragraph" for bt in block_types)


def test_get_chunks_byte_offsets():
    """get_chunks() returns byte-exact offsets."""
    text = "# Title\n\nHello world"
    chunker = mordant.MarkdownChunker(text)
    chunks = chunker.get_chunks()
    assert len(chunks) == 1
    c = chunks[0]
    assert c.start_offset == 9  # after "# Title\n\n"
    assert c.end_offset == 20  # "Hello world"
    assert text[c.start_offset:c.end_offset] == "Hello world"


def test_get_chunks_empty_doc():
    """get_chunks() returns empty list for empty document."""
    chunker = mordant.MarkdownChunker("")
    chunks = chunker.get_chunks()
    assert len(chunks) == 0


def test_get_chunks_all_block_types():
    """get_chunks() handles all block types correctly."""
    text = """# Title

Paragraph text.

- item one
- item two

> quoted text

```python
code
```
"""
    chunker = mordant.MarkdownChunker(text)
    chunks = chunker.get_chunks()
    block_types = [c.block_type for c in chunks]
    assert "Paragraph" in block_types
    assert "List" in block_types
    assert "Blockquote" in block_types
    assert "CodeBlock" in block_types


# === get_all_chunks() ===

def test_get_all_chunks_includes_headings():
    """get_all_chunks() includes Heading chunks."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara one\n\n## Sub\n\nPara two")
    chunks = chunker.get_all_chunks()
    assert len(chunks) == 4
    block_types = [c.block_type for c in chunks]
    assert block_types == ["Heading", "Paragraph", "Heading", "Paragraph"]


def test_get_all_chunks_heading_text():
    """get_all_chunks() yields heading text without prefix."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara one")
    chunks = chunker.get_all_chunks()
    assert chunks[0].block_type == "Heading"
    assert chunks[0].text == "# Title"


# === get_chunks_with_context() ===

def test_get_chunks_with_context_prefixes_headings():
    """get_chunks_with_context() prefixes body chunks with heading."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara one\n\n## Sub\n\nPara two")
    chunks = chunker.get_chunks_with_context()
    assert len(chunks) == 2
    assert chunks[0].text == "# Title\n\nPara one"
    assert chunks[1].text == "## Sub\n\nPara two"


def test_get_chunks_with_context_first_chunk_no_heading():
    """First chunk before any heading has no prefix."""
    chunker = mordant.MarkdownChunker("Intro text\n\n# Title\n\nBody text")
    chunks = chunker.get_chunks_with_context()
    assert len(chunks) == 2
    assert chunks[0].text == "Intro text"  # no heading prefix
    assert chunks[1].text == "# Title\n\nBody text"  # has heading prefix


# === get_bare_chunks() ===

def test_get_bare_chunks_returns_str():
    """get_bare_chunks() returns a list of str objects."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara one")
    chunks = chunker.get_bare_chunks()
    assert len(chunks) == 1
    assert isinstance(chunks[0], str)
    assert chunks[0] == "Para one"


def test_get_bare_chunks_backward_compatible():
    """get_bare_chunks() is equivalent to iterating the chunker."""
    text = "# Title\n\nPara one\n\n## Sub\n\nPara two"
    chunker1 = mordant.MarkdownChunker(text)
    chunker2 = mordant.MarkdownChunker(text)
    iter_chunks = list(chunker1)
    method_chunks = chunker2.get_bare_chunks()
    assert iter_chunks == method_chunks


# === get_delimiter() ===

def test_get_delimiter_list_to_list():
    """get_delimiter() returns single newline for List→List."""
    delim = mordant.MarkdownChunker.get_delimiter("List", "List")
    assert delim == "\n"


def test_get_delimiter_blockquote_to_blockquote():
    """get_delimiter() returns quote marker for Blockquote→Blockquote."""
    delim = mordant.MarkdownChunker.get_delimiter("Blockquote", "Blockquote")
    assert delim == "\n> "


def test_get_delimiter_default():
    """get_delimiter() returns paragraph break for other combinations."""
    delim = mordant.MarkdownChunker.get_delimiter("Paragraph", "Paragraph")
    assert delim == "\n\n"
    delim = mordant.MarkdownChunker.get_delimiter("Heading", "Paragraph")
    assert delim == "\n\n"
    delim = mordant.MarkdownChunker.get_delimiter("CodeBlock", "List")
    assert delim == "\n\n"


# === compute_overlap_payloads() ===

def test_compute_overlap_payloads_basic():
    """compute_overlap_payloads() returns list of dicts with chunk_id and text."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara one\n\n## Sub\n\nPara two")
    payloads = chunker.compute_overlap_payloads(0)
    assert len(payloads) == 2
    # Dict format: {"chunk:0": "Para one", "chunk:1": "Para two"}
    assert "chunk:0" in payloads[0]
    assert "chunk:1" in payloads[1]
    assert payloads[0]["chunk:0"] == "Para one"
    assert payloads[1]["chunk:1"] == "Para two"


def test_compute_overlap_payloads_with_overlap():
    """compute_overlap_payloads() prepends tail words when overlap_words > 0."""
    chunker = mordant.MarkdownChunker("# Title\n\nFirst para second para third para.\n\n## Sub\n\nMore text here.")
    payloads = chunker.compute_overlap_payloads(2)
    assert len(payloads) == 2
    # First payload has no overlap (no previous chunk)
    assert payloads[0]["chunk:0"] == "First para second para third para."
    # Second payload has tail of first chunk prepended (with double spaces from join)
    assert "third" in payloads[1]["chunk:1"]
    assert "More text here" in payloads[1]["chunk:1"]
    # Verify overlap text is prepended (not just the raw chunk)
    assert payloads[1]["chunk:1"].startswith("third")


def test_compute_overlap_payloads_zero_overlap():
    """compute_overlap_payloads() with 0 overlap returns pure chunks."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara one\n\n## Sub\n\nPara two")
    payloads = chunker.compute_overlap_payloads(0)
    assert payloads[0]["chunk:0"] == "Para one"
    assert payloads[1]["chunk:1"] == "Para two"


def test_compute_overlap_payloads_empty_doc():
    """compute_overlap_payloads() returns empty list for empty document."""
    chunker = mordant.MarkdownChunker("")
    payloads = chunker.compute_overlap_payloads(64)
    assert len(payloads) == 0
