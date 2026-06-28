"""Tests for the Mermaid diagram extension in mordant."""

import mordant
import pytest


class TestMermaidBasic:
    """Test basic Mermaid diagram rendering."""

    def test_mermaid_basic_render(self):
        """Basic Mermaid diagram renders as <pre class='mermaid'>."""
        source = """```mermaid
graph LR
    A --- B
```"""
        html = mordant.markdown_to_html(source)
        assert '<pre class="mermaid">' in html
        assert "graph LR" in html
        assert "</pre>" in html

    def test_mermaid_script_injected(self):
        """Mermaid script is injected when diagrams are present."""
        source = """```mermaid
graph LR
    A --- B
```"""
        html = mordant.markdown_to_html(source)
        assert '<script type="module">' in html
        assert "import mermaid from" in html
        assert "mermaid.esm.min.mjs" in html

    def test_no_script_without_diagrams(self):
        """No Mermaid script when no diagrams are present."""
        source = "# Hello World"
        html = mordant.markdown_to_html(source)
        assert '<script type="module">' not in html
        assert "mermaid" not in html

    def test_mermaid_preserves_content(self):
        """Mermaid diagram content is preserved exactly."""
        source = """```mermaid
sequenceDiagram
    Alice->>Bob: Hello Bob
    Bob-->>Alice: Hi Alice
```"""
        html = mordant.markdown_to_html(source)
        assert "sequenceDiagram" in html
        assert "Alice-&gt;&gt;Bob: Hello Bob" in html
        assert "Bob--&gt;&gt;Alice: Hi Alice" in html


class TestMermaidOptions:
    """Test Mermaid diagram options."""

    def test_mermaid_disabled(self):
        """When mermaid is disabled, code block passes through unchanged."""
        opts = mordant.PyDiagramParserOptions(mermaid_enabled=False)
        source = """```mermaid
graph LR
    A --- B
```"""
        html = mordant.markdown_to_html(source, diagram_parse_opts=opts)
        # Should be a regular code block, not a diagram
        assert '<pre class="mermaid">' not in html
        assert '<script type="module">' not in html

    def test_custom_mermaid_url(self):
        """Custom Mermaid URL is used in the script tag."""
        opts = mordant.PyDiagramHtmlRendererOptions(
            mermaid_url="https://example.com/mermaid/custom.mjs"
        )
        source = """```mermaid
graph LR
    A --- B
```"""
        html = mordant.markdown_to_html(source, diagram_render_opts=opts)
        assert "https://example.com/mermaid/custom.mjs" in html

    def test_default_mermaid_url(self):
        """Default Mermaid URL is jsDelivr CDN."""
        opts = mordant.PyDiagramHtmlRendererOptions()
        source = """```mermaid
graph LR
    A --- B
```"""
        html = mordant.markdown_to_html(source, diagram_render_opts=opts)
        assert "cdn.jsdelivr.net/npm/mermaid@latest" in html


class TestMermaidParse:
    """Test Mermaid diagram AST access via parse()."""

    def test_parse_mermaid_diagram_node(self):
        """Parse returns Document with Diagram nodes accessible."""
        source = """```mermaid
graph LR
    A --- B
```"""
        doc = mordant.parse(source)
        diagram_nodes = [
            n for n in doc.walk("depth") if n.kind == "Diagram"
        ]
        assert len(diagram_nodes) == 1

    def test_diagram_node_properties(self):
        """Diagram nodes expose diagram_type and diagram_value."""
        source = """```mermaid
graph LR
    A --- B
```"""
        doc = mordant.parse(source)
        for node in doc.walk("depth"):
            if node.kind == "Diagram":
                assert node.diagram_type == "mermaid"
                assert "graph LR" in node.diagram_value
                break
        else:
            pytest.fail("No Diagram node found")

    def test_parse_mermaid_disabled(self):
        """When disabled, no Diagram nodes in AST."""
        opts = mordant.PyDiagramParserOptions(mermaid_enabled=False)
        source = """```mermaid
graph LR
    A --- B
```"""
        doc = mordant.parse(source, diagram_opts=opts)
        diagram_nodes = [
            n for n in doc.walk("depth") if n.kind == "Diagram"
        ]
        assert len(diagram_nodes) == 0


class TestMermaidMultiple:
    """Test multiple Mermaid diagrams in one document."""

    def test_multiple_diagrams(self):
        """Multiple Mermaid diagrams all render."""
        source = """```mermaid
graph LR
    A --- B
```

Some text.

```mermaid
sequenceDiagram
    Alice->>Bob: Hello
```"""
        html = mordant.markdown_to_html(source)
        assert html.count('<pre class="mermaid">') == 2
        # Script should only appear once
        assert html.count('<script type="module">') == 1

    def test_mixed_content(self):
        """Mermaid diagrams mixed with regular content."""
        source = """# Title

Some paragraph.

```mermaid
graph TD
    A --> B
```

More text.

- List item
- Another item
"""
        html = mordant.markdown_to_html(source)
        assert "<h1>" in html
        assert '<pre class="mermaid">' in html
        assert "<ul>" in html
        assert "graph TD" in html


class TestMermaidEdgeCases:
    """Test edge cases for Mermaid diagram rendering."""

    def test_empty_mermaid_block(self):
        """Empty Mermaid code block renders empty pre."""
        source = "```mermaid\n```"
        html = mordant.markdown_to_html(source)
        assert '<pre class="mermaid">' in html
        assert "</pre>" in html

    def test_mermaid_with_special_chars(self):
        """Mermaid with special HTML characters is escaped."""
        source = """```mermaid
graph LR
    A --> B[Click & Go]
```"""
        html = mordant.markdown_to_html(source)
        assert "graph LR" in html
        assert "Click &amp; Go" in html or "Click & Go" in html

    def test_mermaid_in_gfm(self):
        """Mermaid works in GFM mode."""
        source = """```mermaid
graph LR
    A --- B
```"""
        html = mordant.markdown_to_html(source, gfm=True)
        assert '<pre class="mermaid">' in html

    def test_mermaid_with_frontmatter(self):
        """Mermaid works alongside YAML frontmatter."""
        source = """---
title: Diagram Doc
---

```mermaid
graph LR
    A --- B
```"""
        doc = mordant.parse(source)
        assert doc.metadata["title"] == "Diagram Doc"
        diagram_nodes = [
            n for n in doc.walk("depth") if n.kind == "Diagram"
        ]
        assert len(diagram_nodes) == 1

    def test_mermaid_with_other_gfm(self):
        """Mermaid works alongside other GFM features."""
        source = """```mermaid
graph LR
    A --- B
```

~~strikethrough~~ and `code` and [link](url)
"""
        html = mordant.markdown_to_html(source, gfm=True)
        assert '<pre class="mermaid">' in html
        assert "<del>" in html
        assert "<code>" in html
        assert "<a " in html
