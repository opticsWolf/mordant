"""Chunker tests — MarkdownChunker iterator, heading context, file I/O, and edge cases."""

import os
import tempfile

import mordant


# === Constructor & basic iteration ===

def test_empty_document():
    """Empty source yields no chunks."""
    chunker = mordant.MarkdownChunker("")
    chunks = list(chunker)
    assert chunks == []


def test_single_paragraph():
    """A lone paragraph is yielded as-is (no heading prefix)."""
    chunker = mordant.MarkdownChunker("Hello world")
    chunks = list(chunker)
    assert len(chunks) == 1
    assert chunks[0] == "Hello world"


def test_heading_not_yielded():
    """Headings update context but are never yielded as standalone chunks."""
    chunker = mordant.MarkdownChunker("# Title")
    chunks = list(chunker)
    assert chunks == []


def test_heading_plus_paragraph():
    """A heading followed by a paragraph yields one bare chunk (no heading prefix)."""
    text = "# Title\n\nHello world"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 1
    assert chunks[0] == "Hello world"  # bare chunk, no heading prefix


# === Heading context propagation ===

def test_heading_context_updates():
    """Each new heading updates current_header for subsequent blocks."""
    text = "# First\n\nPara one\n\n## Second\n\nPara two"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 2
    # Chunks are bare (no heading prefix)
    assert chunks[0] == "Para one"
    assert chunks[1] == "Para two"
    # current_header tracks the last heading seen
    assert chunker.current_header == "## Second"


def test_paragraph_before_heading():
    """A paragraph before any heading is yielded standalone (bare)."""
    text = "Intro text\n\n# Title\n\nBody text"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 2
    assert chunks[0] == "Intro text"
    assert chunks[1] == "Body text"  # bare, no heading prefix


def test_multiple_headings_no_content():
    """Consecutive headings with no content blocks yield nothing."""
    text = "# A\n\n## B\n\n### C"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert chunks == []


# === current_header property ===

def test_current_header_none_initially():
    """Before iteration, current_header is None."""
    chunker = mordant.MarkdownChunker("# Title\n\nHello")
    assert chunker.current_header is None


def test_current_header_after_heading():
    """After consuming a heading, current_header reflects it."""
    chunker = mordant.MarkdownChunker("# Title\n\nHello")
    next(chunker)  # consumes paragraph
    assert chunker.current_header == "# Title"


def test_current_header_persists_across_blocks():
    """current_header persists until a new heading is consumed."""
    text = "# Section\n\nPara one\n\nPara two"
    chunker = mordant.MarkdownChunker(text)
    next(chunker)  # yields "Para one"
    assert chunker.current_header == "# Section"
    next(chunker)  # yields "Para two"
    assert chunker.current_header == "# Section"


# === Code blocks ===

def test_code_block_standalone():
    """A code block with no heading is yielded standalone."""
    text = "```python\nprint('hi')\n```"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 1
    assert "print('hi')" in chunks[0]


def test_code_block_with_heading():
    """A code block after a heading is yielded as a bare chunk (no heading prefix)."""
    text = "# Code\n\n```python\nprint('hi')\n```"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 1
    assert not chunks[0].startswith("# Code")  # bare chunk
    assert "print('hi')" in chunks[0]


# === Lists ===

def test_list_standalone():
    """A list with no heading is yielded standalone."""
    text = "- item one\n- item two"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 1
    assert "- item one" in chunks[0]


def test_list_with_heading():
    """A list after a heading is yielded as a bare chunk (no heading prefix)."""
    text = "# Tasks\n\n- todo\n- done"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 1
    assert not chunks[0].startswith("# Tasks")  # bare chunk
    assert "- todo" in chunks[0]


# === Blockquotes ===

def test_blockquote_standalone():
    """A blockquote with no heading is yielded standalone."""
    text = "> quoted text"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 1
    assert "> quoted text" in chunks[0]


def test_blockquote_with_heading():
    """A blockquote after a heading is yielded as a bare chunk (no heading prefix)."""
    text = "# Note\n\n> important info"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 1
    assert not chunks[0].startswith("# Note")  # bare chunk
    assert "> important info" in chunks[0]


# === Nested headings do not leak ===

def test_nested_heading_does_not_become_context():
    """A heading inside a blockquote must never become the context prefix."""
    text = "# Outer\n\n> # Nested\n\n> Quote text."
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    # The blockquote content is yielded as bare chunks (no prefix at all).
    # Nested headings inside blockquotes are parsed but don't set current_header.
    assert len(chunks) == 2
    # Neither chunk should be prefixed with "# Outer" (they're bare).
    for c in chunks:
        assert not c.startswith("# Outer")
    # current_header should still be "# Outer" (nested heading doesn't override it)
    assert chunker.current_header == "# Outer"


# === Thematic breaks (Other kind) ===

def test_thematic_break_skipped():
    """Thematic breaks are not yielded as chunks."""
    text = "# Title\n\nPara one\n\n---\n\nPara two"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    # The thematic break is skipped; two paragraphs are yielded.
    assert len(chunks) == 2
    assert chunks[0] == "Para one"
    assert chunks[1] == "Para two"


def test_thematic_break_does_not_reset_context():
    """Thematic breaks do NOT reset the heading context."""
    text = "# Title\n\nPara one\n\n***\n\nPara two"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    # Both paragraphs are bare chunks (no prefix).
    assert chunks[0] == "Para one"
    assert chunks[1] == "Para two"
    # current_header still tracks the heading.
    assert chunker.current_header == "# Title"


# === node_count property ===

def test_node_count_heading_and_paragraph():
    """node_count includes all top-level nodes (headings, paragraphs, etc.)."""
    text = "# Title\n\nHello world"
    chunker = mordant.MarkdownChunker(text)
    assert chunker.node_count == 2  # Heading + Paragraph


def test_node_count_empty():
    """Empty document has zero nodes."""
    chunker = mordant.MarkdownChunker("")
    assert chunker.node_count == 0


def test_node_count_includes_other():
    """node_count includes 'Other' nodes (thematic breaks, etc.)."""
    text = "# Title\n\n---"
    chunker = mordant.MarkdownChunker(text)
    # Heading + ThematicBreak
    assert chunker.node_count == 2


# === from_file ===

def test_from_file_basic(tmp_path):
    """from_file reads a file and chunks it correctly."""
    file_path = tmp_path / "test.md"
    file_path.write_text("# Hello\n\nWorld")
    chunker = mordant.MarkdownChunker.from_file(str(file_path))
    chunks = list(chunker)
    assert len(chunks) == 1
    assert chunks[0] == "World"  # bare chunk


def test_from_file_node_count(tmp_path):
    """from_file sets node_count correctly."""
    file_path = tmp_path / "test.md"
    file_path.write_text("# A\n\nPara\n\n## B\n\nMore")
    chunker = mordant.MarkdownChunker.from_file(str(file_path))
    assert chunker.node_count == 4  # Heading + Paragraph + Heading + Paragraph


def test_from_file_missing_file():
    """from_file raises on a nonexistent path."""
    import pytest
    with pytest.raises(OSError):
        mordant.MarkdownChunker.from_file("/nonexistent/path/file.md")


# === from_file_mmap ===

def test_from_file_mmap_basic(tmp_path):
    """from_file_mmap reads a file via mmap and chunks correctly."""
    file_path = tmp_path / "test.md"
    file_path.write_text("# Title\n\nSome content here")
    chunker = mordant.MarkdownChunker.from_file_mmap(str(file_path))
    chunks = list(chunker)
    assert len(chunks) == 1
    assert chunks[0] == "Some content here"  # bare chunk


def test_from_file_mmap_large_file(tmp_path):
    """from_file_mmap handles a reasonably large file."""
    file_path = tmp_path / "large.md"
    lines = []
    for i in range(500):
        lines.append(f"## Section {i}")
        lines.append(f"Content for section {i} with enough text to fill lines.")
        lines.append("")
    file_path.write_text("\n".join(lines))
    chunker = mordant.MarkdownChunker.from_file_mmap(str(file_path))
    chunks = list(chunker)
    # 500 headings (not yielded) + 500 paragraphs (yielded)
    assert len(chunks) == 500
    # Last chunk is bare (no heading prefix).
    assert "Content for section 499" in chunks[-1]
    assert not chunks[-1].startswith("## Section 499")  # bare chunk


def test_from_file_mmap_missing_file():
    """from_file_mmap raises on a nonexistent path."""
    import pytest
    with pytest.raises(OSError):
        mordant.MarkdownChunker.from_file_mmap("/nonexistent/path/file.md")


# === Iterator protocol ===

def test_iterator_protocol():
    """MarkdownChunker supports iter() and next()."""
    chunker = mordant.MarkdownChunker("# A\n\nPara")
    it = iter(chunker)
    chunk = next(it)
    assert isinstance(chunk, str)
    assert chunk == "Para"  # bare chunk
    # Exhausted
    import pytest
    with pytest.raises(StopIteration):
        next(it)


def test_for_loop_iteration():
    """MarkdownChunker works with for-in loops."""
    text = "# H1\n\nP1\n\n## H2\n\nP2"
    chunker = mordant.MarkdownChunker(text)
    chunks = []
    for c in chunker:
        chunks.append(c)
    assert len(chunks) == 2
    assert chunks[0] == "P1"
    assert chunks[1] == "P2"


def test_multiple_iterations_exhaust_after_first():
    """The iterator can only be consumed once (index is not reset)."""
    chunker = mordant.MarkdownChunker("# A\n\nPara")
    first = list(chunker)
    assert len(first) == 1
    second = list(chunker)
    assert second == []  # already exhausted


# === Trailing whitespace trimming ===

def test_trailing_blank_lines_trimmed():
    """Trailing blank lines between blocks are trimmed from chunks."""
    text = "# Title\n\nPara one\n\n\n\n## Next\n\nPara two"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    # The chunk for "Para one" should not end with extra blank lines.
    assert chunks[0].endswith("Para one")
    assert chunks[1].endswith("Para two")


# === Mixed content document ===

def test_complex_document():
    """Full document with headings, paragraphs, lists, code, blockquotes, and breaks."""
    text = """# Introduction

Welcome to the guide.

## Getting Started

- Step one
- Step two

```python
print("hello")
```

> A helpful tip.

---

## Advanced

Final paragraph.
"""
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)

    # Expected yields (bare chunks, no heading prefix):
    # 1. "Welcome to the guide."
    # 2. "- Step one\n- Step two"
    # 3. "```python\nprint("hello")\n```"
    # 4. "> A helpful tip."
    # (thematic break skipped)
    # 5. "Final paragraph."
    assert len(chunks) == 5

    # All chunks are bare (no heading prefix).
    for c in chunks:
        assert not c.startswith("#")

    # Verify thematic break was skipped (context preserved through it).
    assert chunker.current_header == "## Advanced"


# === GFM tables ===

def test_table_with_heading(tmp_path):
    """GFM tables are yielded as bare chunks (no heading prefix)."""
    text = "# Data\n\n| A | B |\n|---|---|\n| 1 | 2 |"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 1
    assert not chunks[0].startswith("# Data")  # bare chunk
    assert "| A | B |" in chunks[0]


# === Edge cases ===

def test_only_thematic_breaks():
    """A document with only thematic breaks yields no chunks."""
    text = "---\n\n***\n\n---"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert chunks == []


def test_heading_only_no_content():
    """A heading with no following content yields nothing."""
    text = "# Alone"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert chunks == []


def test_ordered_list_with_heading():
    """Ordered lists work the same as unordered lists (bare chunks)."""
    text = "## Steps\n\n1. First\n2. Second\n3. Third"
    chunker = mordant.MarkdownChunker(text)
    chunks = list(chunker)
    assert len(chunks) == 1
    assert not chunks[0].startswith("## Steps")  # bare chunk
    assert "1. First" in chunks[0]
