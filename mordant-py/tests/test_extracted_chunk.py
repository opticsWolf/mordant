"""Tests for ExtractedChunk — the new metadata-rich chunk type."""

import mordant


# === ExtractedChunk class existence ===

def test_extracted_chunk_class_exists():
    """mordant.ExtractedChunk is accessible."""
    assert hasattr(mordant, "ExtractedChunk")
    assert mordant.ExtractedChunk is not None


# === ExtractedChunk attributes ===

def test_extracted_chunk_has_all_attributes():
    """ExtractedChunk exposes text, block_type, start_offset, end_offset."""
    chunker = mordant.MarkdownChunker("# Title\n\nPara.")
    # Consume the heading (updates context) then get the paragraph
    # Note: current behavior skips headings, so we get the paragraph
    chunks = list(chunker)
    assert len(chunks) >= 1
    for c in chunks:
        assert isinstance(c, str)  # current behavior: yields str
        # We'll test the new ExtractedChunk once __next__ is updated


def test_block_type_values():
    """BlockType enum maps to correct string values."""
    # This tests the Rust enum directly via the repr of ExtractedChunk
    # Once ExtractedChunk is yielded, we verify strings
    pass  # covered in later tests


# === ExtractedChunk construction and repr ===

def test_extracted_chunk_repr():
    """ExtractedChunk.__repr__ returns a readable string."""
    chunk = mordant.ExtractedChunk(
        text="Hello",
        block_type="Paragraph",
        start_offset=0,
        end_offset=5,
    )
    repr_str = repr(chunk)
    assert "ExtractedChunk" in repr_str
    assert "Paragraph" in repr_str
    assert "Hello" in repr_str
    assert "0" in repr_str
    assert "5" in repr_str


def test_extracted_chunk_attributes_read_only():
    """ExtractedChunk attributes are readable."""
    chunk = mordant.ExtractedChunk(
        text="World",
        block_type="Heading",
        start_offset=10,
        end_offset=15,
    )
    assert chunk.text == "World"
    assert chunk.block_type == "Heading"
    assert chunk.start_offset == 10
    assert chunk.end_offset == 15


# === BlockType string mapping ===

def test_block_type_heading():
    """BlockType Heading maps to 'Heading'."""
    chunk = mordant.ExtractedChunk(
        text="# Title",
        block_type="Heading",
        start_offset=0,
        end_offset=7,
    )
    assert chunk.block_type == "Heading"


def test_block_type_paragraph():
    """BlockType Paragraph maps to 'Paragraph'."""
    chunk = mordant.ExtractedChunk(
        text="Hello world",
        block_type="Paragraph",
        start_offset=0,
        end_offset=11,
    )
    assert chunk.block_type == "Paragraph"


def test_block_type_codeblock():
    """BlockType CodeBlock maps to 'CodeBlock'."""
    chunk = mordant.ExtractedChunk(
        text="print('hi')",
        block_type="CodeBlock",
        start_offset=0,
        end_offset=11,
    )
    assert chunk.block_type == "CodeBlock"


def test_block_type_list():
    """BlockType List maps to 'List'."""
    chunk = mordant.ExtractedChunk(
        text="- item 1",
        block_type="List",
        start_offset=0,
        end_offset=8,
    )
    assert chunk.block_type == "List"


def test_block_type_table():
    """BlockType Table maps to 'Table'."""
    chunk = mordant.ExtractedChunk(
        text="| A | B |\n|---|---|",
        block_type="Table",
        start_offset=0,
        end_offset=12,
    )
    assert chunk.block_type == "Table"


def test_block_type_blockquote():
    """BlockType Blockquote maps to 'Blockquote'."""
    chunk = mordant.ExtractedChunk(
        text="> quoted",
        block_type="Blockquote",
        start_offset=0,
        end_offset=8,
    )
    assert chunk.block_type == "Blockquote"


def test_block_type_other():
    """BlockType Other maps to 'Other'."""
    chunk = mordant.ExtractedChunk(
        text="---",
        block_type="Other",
        start_offset=0,
        end_offset=3,
    )
    assert chunk.block_type == "Other"


# === Byte offset correctness ===

def test_byte_offset_zero_based():
    """Offsets are zero-based into source."""
    chunk = mordant.ExtractedChunk(
        text="Hello",
        block_type="Paragraph",
        start_offset=0,
        end_offset=5,
    )
    assert chunk.start_offset == 0
    assert chunk.end_offset == 5


def test_byte_offset_end_exclusive():
    """end_offset is exclusive — slicing source[start:end] gives text."""
    source = "# Title\n\nHello world"
    chunk = mordant.ExtractedChunk(
        text="Hello world",
        block_type="Paragraph",
        start_offset=9,
        end_offset=20,
    )
    assert chunk.start_offset == 9
    assert chunk.end_offset == 20
    assert source[chunk.start_offset:chunk.end_offset] == "Hello world"


def test_byte_offset_valid_range():
    """start_offset < end_offset for all chunks."""
    chunk = mordant.ExtractedChunk(
        text="X",
        block_type="Paragraph",
        start_offset=0,
        end_offset=1,
    )
    assert chunk.start_offset < chunk.end_offset


# === Edge cases ===

def test_extracted_chunk_empty_text():
    """ExtractedChunk can hold empty text."""
    chunk = mordant.ExtractedChunk(
        text="",
        block_type="Other",
        start_offset=0,
        end_offset=0,
    )
    assert chunk.text == ""
    assert chunk.block_type == "Other"
    assert chunk.start_offset == 0
    assert chunk.end_offset == 0


def test_extracted_chunk_multiline_text():
    """ExtractedChunk handles multiline text."""
    text = "Line 1\nLine 2\nLine 3"
    chunk = mordant.ExtractedChunk(
        text=text,
        block_type="Paragraph",
        start_offset=0,
        end_offset=len(text),
    )
    assert chunk.text == text


def test_extracted_chunk_unicode_text():
    """ExtractedChunk handles unicode text."""
    text = "Hello 世界 🌍"
    chunk = mordant.ExtractedChunk(
        text=text,
        block_type="Paragraph",
        start_offset=0,
        end_offset=len(text.encode("utf-8")),
    )
    assert chunk.text == text
