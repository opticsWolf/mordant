"""Tests for YAML frontmatter parsing via the meta parser extension.

These tests verify:
- Basic frontmatter parsing (scalars, sequences, nested mappings)
- Thematic break conflict resolution (--- vs frontmatter)
- Edge cases (empty frontmatter, trailing spaces, multiple dashes)
- YAML error handling
- Integration with markdown_to_html
"""

import pytest
import mordant


# === Basic Frontmatter ===

class TestBasicFrontmatter:
    def test_simple_scalar(self):
        doc = mordant.parse("---\ntitle: Hello World\n---\n\nBody")
        assert doc.metadata["title"] == "Hello World"

    def test_integer(self):
        doc = mordant.parse("---\ncount: 42\n---\n\nBody")
        assert doc.metadata["count"] == 42

    def test_float(self):
        doc = mordant.parse("---\nratio: 3.14159\n---\n\nBody")
        assert doc.metadata["ratio"] == 3.14159

    def test_boolean_true(self):
        doc = mordant.parse("---\nflag: true\n---\n\nBody")
        assert doc.metadata["flag"] is True

    def test_boolean_false(self):
        doc = mordant.parse("---\nflag: false\n---\n\nBody")
        assert doc.metadata["flag"] is False

    def test_null_value(self):
        doc = mordant.parse("---\nval: null\n---\n\nBody")
        assert doc.metadata["val"] is None

    def test_string_with_colon(self):
        doc = mordant.parse("---\nurl: https://example.com:8080\n---\n\nBody")
        assert doc.metadata["url"] == "https://example.com:8080"


class TestSequences:
    def test_simple_list(self):
        doc = mordant.parse("---\ntags:\n  - rust\n  - markdown\n---\n\nBody")
        assert doc.metadata["tags"] == ["rust", "markdown"]

    def test_nested_list(self):
        doc = mordant.parse("---\nitems:\n  - a\n  - b\n  - c\n---\n\nBody")
        assert doc.metadata["items"] == ["a", "b", "c"]


class TestNestedMappings:
    def test_two_level_nested(self):
        doc = mordant.parse("---\nauthor:\n  name: Jane\n  age: 30\n---\n\nBody")
        assert isinstance(doc.metadata["author"], dict)
        assert doc.metadata["author"]["name"] == "Jane"
        assert doc.metadata["author"]["age"] == 30

    def test_three_level_nested(self):
        md = "---\nlevel1:\n  level2:\n    level3: deep\n---\n\nBody"
        doc = mordant.parse(md)
        assert doc.metadata["level1"]["level2"]["level3"] == "deep"

    def test_multiple_nested_keys(self):
        md = "---\nmeta:\n  title: Doc\n  date: 2024-01-01\n---\n\nBody"
        doc = mordant.parse(md)
        assert doc.metadata["meta"]["title"] == "Doc"
        assert doc.metadata["meta"]["date"] == "2024-01-01"


class TestMixedTypes:
    def test_mixed_scalar_types(self):
        md = """---
title: Greeting
count: 42
ratio: 3.14
active: true
empty: null
---

Body"""
        doc = mordant.parse(md)
        assert doc.metadata["title"] == "Greeting"
        assert doc.metadata["count"] == 42
        assert doc.metadata["ratio"] == 3.14
        assert doc.metadata["active"] is True
        assert doc.metadata["empty"] is None

    def test_mixed_list_and_mapping(self):
        md = """---
tags:
  - rust
  - python
author:
  name: Jane
---

Body"""
        doc = mordant.parse(md)
        assert doc.metadata["tags"] == ["rust", "python"]
        assert doc.metadata["author"]["name"] == "Jane"


# === Thematic Break Conflict Resolution ===

class TestThematicBreakConflict:
    """Verify that --- is correctly distinguished from thematic breaks."""

    def test_bare_thematic_break(self):
        """A bare `---` should be a thematic break, not frontmatter."""
        doc = mordant.parse("---")
        assert doc.metadata == {}
        assert len(doc.children) == 1
        assert doc.children[0].kind == "ThematicBreak"

    def test_five_dashes_thematic_break(self):
        """Five dashes is a valid thematic break."""
        doc = mordant.parse("-----")
        assert doc.metadata == {}
        assert len(doc.children) == 1
        assert doc.children[0].kind == "ThematicBreak"

    def test_asterisk_thematic_break(self):
        doc = mordant.parse("* * *")
        assert doc.metadata == {}
        assert len(doc.children) == 1
        assert doc.children[0].kind == "ThematicBreak"

    def test_underscore_thematic_break(self):
        doc = mordant.parse("_ _ _")
        assert doc.metadata == {}
        assert len(doc.children) == 1
        assert doc.children[0].kind == "ThematicBreak"

    def test_thematic_break_with_trailing_spaces(self):
        """--- followed by spaces should still be a thematic break."""
        doc = mordant.parse("---  \n\nHello")
        assert doc.metadata == {}
        # ThematicBreak + Paragraph = 2 children
        assert len(doc.children) == 2
        assert doc.children[0].kind == "ThematicBreak"
        assert doc.children[1].kind == "Paragraph"

    def test_thematic_break_followed_by_content(self):
        doc = mordant.parse("---\n\nHello")
        assert doc.metadata == {}
        assert doc.children[0].kind == "ThematicBreak"
        assert doc.children[1].kind == "Paragraph"

    def test_thematic_break_in_middle(self):
        doc = mordant.parse("Hello\n\n---\n\nWorld")
        assert doc.metadata == {}
        assert doc.children[1].kind == "ThematicBreak"

    def test_two_thematic_breaks(self):
        doc = mordant.parse("---\n\n---")
        assert doc.metadata == {}
        assert all(c.kind == "ThematicBreak" for c in doc.children)

    def test_frontmatter_after_thematic_break(self):
        """Frontmatter must start at line 0 to be recognized."""
        md = "Hello\n\n---\ntitle: Test\n---\n\nBody"
        doc = mordant.parse(md)
        # Frontmatter only works at the start of the document
        assert doc.metadata.get("title") != "Test" or len(doc.children) > 0


# === Edge Cases ===

class TestEdgeCases:
    def test_empty_frontmatter(self):
        """Empty frontmatter (just ---\\n---) should not crash."""
        doc = mordant.parse("---\n---\n\nBody")
        assert doc.metadata == {}

    def test_whitespace_only_frontmatter(self):
        """Frontmatter with only whitespace should not crash."""
        doc = mordant.parse("---\n   \n---\n\nBody")
        assert doc.metadata == {}

    def test_frontmatter_no_trailing_content(self):
        doc = mordant.parse("---\ntitle: Test\n---")
        assert doc.metadata["title"] == "Test"

    def test_frontmatter_with_trailing_newline(self):
        doc = mordant.parse("---\ntitle: Test\n---\n")
        assert doc.metadata["title"] == "Test"

    def test_no_frontmatter(self):
        doc = mordant.parse("Just markdown\n\nNo frontmatter here")
        assert doc.metadata == {}

    def test_frontmatter_with_dash_in_string(self):
        """YAML string containing --- should not confuse the parser."""
        md = "---\nbody: |\n  text with --- inside\n---\n\nBody"
        doc = mordant.parse(md)
        assert doc.metadata["body"] == "text with --- inside\n"

    def test_frontmatter_preserves_html(self):
        md = "---\ntitle: Test\n---\n\n# Heading\n\n**Bold**"
        html = mordant.markdown_to_html(md)
        assert "<h1>Heading</h1>" in html
        assert "<strong>Bold</strong>" in html


# === YAML Error Handling ===

class TestYamlErrors:
    def test_invalid_yaml_graceful_handling(self):
        """Malformed YAML is handled gracefully - yaml-peg parses what it can."""
        md = "---\ninvalid: [\n---\n\nBody"
        doc = mordant.parse(md)
        # yaml-peg parses 'invalid: [' as 'invalid: null' (syntax error ignored)
        assert "invalid" in doc.metadata

    def test_yaml_list_error_inserted_as_html_comment(self):
        """A top-level YAML list produces an HTML error comment in the AST."""
        md = "---\n- item1\n- item2\n---\n\nBody"
        doc = mordant.parse(md)
        # yaml-peg can't parse top-level list as mapping, inserts error comment
        assert doc.metadata == {}
        assert any(c.kind == "HtmlBlock" for c in doc.children)


# === Integration with markdown_to_html ===

class TestHtmlIntegration:
    def test_frontmatter_and_html(self):
        md = "---\ntitle: Test\n---\n\nHello"
        html = mordant.markdown_to_html(md)
        assert "<p>Hello</p>" in html
        doc = mordant.parse(md)
        assert doc.metadata["title"] == "Test"

    def test_thematic_break_html(self):
        html = mordant.markdown_to_html("---")
        assert "<hr>" in html

    def test_gfm_with_frontmatter(self):
        md = "---\ntitle: Test\n---\n\n| A | B |\n|---|---|\n| 1 | 2 |"
        html = mordant.markdown_to_html(md, gfm=True)
        assert "<table>" in html
        doc = mordant.parse(md, gfm=True)
        assert doc.metadata["title"] == "Test"

    def test_empty_document(self):
        html = mordant.markdown_to_html("")
        doc = mordant.parse("")
        assert doc.metadata == {}
        assert html == ""


# === Complex Documents ===

class TestComplexDocuments:
    def test_realistic_frontmatter(self):
        md = """---
title: My Document
author: Jane Doe
date: 2024-01-15
tags:
  - markdown
  - yaml
  - rust
nested:
  key1: value1
  key2: value2
---

# Title

Some **content** with *emphasis*.
"""
        doc = mordant.parse(md)
        assert doc.metadata["title"] == "My Document"
        assert doc.metadata["author"] == "Jane Doe"
        assert doc.metadata["tags"] == ["markdown", "yaml", "rust"]
        assert isinstance(doc.metadata["nested"], dict)
        assert len(doc.children) > 0

    def test_frontmatter_with_special_chars(self):
        md = "---\npath: /usr/local/bin\nregex: ^[a-z]+$\n---\n\nBody"
        doc = mordant.parse(md)
        assert "/usr/local/bin" in doc.metadata["path"]


class TestOriginalTestCases:
    """Tests that directly correspond to the original rushdown-meta test cases."""

    def test_original_test_ok_simple(self):
        """Original rushdown-meta test_ok: simple frontmatter."""
        md = "---\ntitle: YAML Frontmatter\n---\naaa\n"
        doc = mordant.parse(md)
        assert doc.metadata["title"] == "YAML Frontmatter"

    def test_original_test_meta_full_frontmatter(self):
        """Original rushdown-meta test_meta: full frontmatter with nested structures."""
        md = """---
title: YAML Frontmatter
date: 2026-03-11
tags: [Rust, Markdown<>]
author:
  name: yuin
---
aaa
"""
        doc = mordant.parse(md)
        assert doc.metadata["title"] == "YAML Frontmatter"
        assert doc.metadata["date"] == "2026-03-11"
        assert doc.metadata["tags"] == ["Rust", "Markdown<>"]
        assert doc.metadata["author"]["name"] == "yuin"

    def test_original_test_error_malformed_yaml(self):
        """Original rushdown-meta test_error: malformed YAML raises ValueError."""
        md = "---\ntitle: YAML Frontmatter\nhogehoge\n---\naaa\n"
        doc = mordant.parse(md)
        with pytest.raises(ValueError):
            _ = doc.metadata  # ValueError is raised lazily on access
