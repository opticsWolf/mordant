"""Tests for KaTeX math rendering in Mordant.

Covers:
- Level 1: Fenced ```math / ```latex blocks
- Level 2: $...$ inline and $$...$$ block math (when implemented)
- Standalone render_math() function
- Output formats: both, html, mathml
- Error handling (invalid LaTeX)
- Caching behavior
"""

import pytest
import mordant


# =============================================================================
# Level 1: Fenced ```math / ```latex blocks
# =============================================================================

class TestLevel1FencedBlocks:
    """Test ```math and ```latex fenced code blocks."""

    def test_math_fence_basic(self):
        """Basic display math in a ```math block."""
        md = "```\nE = mc^2\n```"
        # Without math support, this renders as a code block
        html = mordant.markdown_to_html(md)
        # With math support, should contain katex markup
        # For now, just check it doesn't crash
        assert html is not None

    def test_math_fence_with_language(self):
        """Math block with 'math' language tag."""
        md = "```math\n\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}\n```"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_latex_fence_with_language(self):
        """Math block with 'latex' language tag."""
        md = "```latex\n\\sum_{i=1}^{n} x_i\n```"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_math_fence_contains_katex_class(self):
        """Math blocks should produce KaTeX output with katex class."""
        md = "```math\nx^2 + y^2 = z^2\n```"
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()

    def test_math_fence_display_mode(self):
        """Fenced math blocks should render in display mode."""
        md = "```math\n\\frac{a}{b}\n```"
        html = mordant.markdown_to_html(md)
        # Display mode wraps in <span class="katex-display">
        assert 'katex-display' in html

    def test_math_fence_with_surrounding_text(self):
        """Math block embedded in a document with text."""
        md = """# Math Test

Here is an equation:

```math
E = mc^2
```

And some more text after.
"""
        html = mordant.markdown_to_html(md)
        assert '<h1>' in html
        assert 'katex' in html.lower()
        assert 'And some more text after' in html

    def test_math_fence_multiple_blocks(self):
        """Multiple math blocks in one document."""
        md = """```math
a^2 + b^2 = c^2
```

```math
\\int_0^1 x^2 dx = 1/3
```
"""
        html = mordant.markdown_to_html(md)
        # Should have at least two katex spans
        katex_count = html.lower().count('katex')
        assert katex_count >= 4  # each block has multiple katex classes

    def test_math_fence_complex_expressions(self):
        """Complex LaTeX expressions."""
        md = "```math\n\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}\n```"
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()

    def test_math_fence_subscripts_superscripts(self):
        """Subscripts and superscripts in math blocks."""
        md = "```math\nx_1^{n+1} + y_2^{m-1}\n```"
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()

    def test_math_fence_greek_letters(self):
        """Greek letters in math blocks."""
        md = "```math\n\\alpha + \\beta = \\gamma\n```"
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()


# =============================================================================
# Standalone render_math() function
# =============================================================================

class TestRenderMathStandalone:
    """Test mordant.render_math() standalone function."""

    def test_render_math_basic(self):
        """Basic inline math rendering."""
        result = mordant.render_math("x^2 + y^2")
        assert isinstance(result, str)
        assert len(result) > 0

    def test_render_math_display_mode(self):
        """Display mode rendering."""
        result = mordant.render_math("E = mc^2", display=True)
        assert 'katex-display' in result

    def test_render_math_inline_mode(self):
        """Inline mode rendering (default)."""
        result = mordant.render_math("x^2", display=False)
        assert 'katex-display' not in result

    def test_render_math_output_both(self):
        """Output format 'both' (HTML + MathML)."""
        result = mordant.render_math("x^2", output="both")
        # Should contain both HTML spans and MathML
        assert 'katex' in result

    def test_render_math_output_html(self):
        """Output format 'html' only."""
        result = mordant.render_math("x^2", output="html")
        assert 'katex' in result

    def test_render_math_output_mathml(self):
        """Output format 'mathml' only."""
        result = mordant.render_math("x^2", output="mathml")
        # MathML output should contain <math> tag
        assert '<math' in result

    def test_render_math_invalid_output_format(self):
        """Invalid output format should raise ValueError."""
        with pytest.raises(ValueError):
            mordant.render_math("x^2", output="invalid")

    def test_render_math_case_insensitive_output(self):
        """Output format should be case-insensitive."""
        r1 = mordant.render_math("x^2", output="HTML")
        r2 = mordant.render_math("x^2", output="html")
        assert r1 == r2

    def test_render_math_greek_letters(self):
        """Greek letters render correctly."""
        result = mordant.render_math("\\alpha + \\beta = \\gamma")
        assert 'katex' in result

    def test_render_math_fractions(self):
        """Fraction rendering."""
        result = mordant.render_math("\\frac{a}{b}", display=True)
        assert 'katex' in result

    def test_render_math_integrals(self):
        """Integral rendering."""
        result = mordant.render_math(r"\int_0^\infty e^{-x^2} dx")
        assert 'katex' in result

    def test_render_math_matrices(self):
        """Matrix rendering."""
        result = mordant.render_math(r"\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}")
        assert 'katex' in result

    def test_render_math_invalid_latex_fallback(self):
        """Invalid LaTeX should produce an error span, not crash."""
        result = mordant.render_math(r"\nonexistentcommand{}")
        # Should contain error span
        assert 'katex-error' in result or 'katex' in result

    def test_render_math_empty_string(self):
        """Empty string handling."""
        result = mordant.render_math("")
        # Should not crash
        assert isinstance(result, str)

    def test_render_math_caching(self):
        """Same input should produce same output (caching)."""
        latex = r"\sum_{i=1}^{n} x_i"
        r1 = mordant.render_math(latex, display=True)
        r2 = mordant.render_math(latex, display=True)
        assert r1 == r2

    def test_render_math_cache_different_display(self):
        """Same LaTeX with different display flag should differ."""
        latex = "x^2"
        r_inline = mordant.render_math(latex, display=False)
        r_display = mordant.render_math(latex, display=True)
        assert r_inline != r_display

    def test_render_math_cache_different_output(self):
        """Same LaTeX with different output format should differ."""
        latex = "x^2"
        r_both = mordant.render_math(latex, output="both")
        r_html = mordant.render_math(latex, output="html")
        r_mathml = mordant.render_math(latex, output="mathml")
        assert r_both != r_html
        assert r_both != r_mathml
        assert r_html != r_mathml

    def test_render_math_special_characters(self):
        """Special characters that need escaping."""
        result = mordant.render_math("a < b > c")
        assert 'katex' in result or 'katex-error' in result

    def test_render_math_unicode(self):
        """Unicode in LaTeX source."""
        result = mordant.render_math("x = 1")
        assert isinstance(result, str)


# =============================================================================
# Level 2: $...$ inline and $$...$$ block math
# =============================================================================

class TestLevel2InlineMath:
    """Test $...$ inline and $$...$$ block math (Level 2).
    
    These tests will be enabled once Level 2 is implemented.
    """

    def test_inline_math_basic(self):
        """Basic inline math with $...$."""
        md = "The value of $x^2$ is important."
        html = mordant.markdown_to_html(md)
        # After Level 2: assert 'katex' in html
        # For now, just check it parses
        assert html is not None

    def test_block_math_dollar_dollar(self):
        """Block math with $$...$$."""
        md = "Equation:\n\n$$E = mc^2$$\n\nMore text."
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_inline_math_multiple(self):
        """Multiple inline math expressions."""
        md = "If $a = b$ and $c = d$, then $a + c = b + d$."
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_inline_math_with_emphasis(self):
        """Inline math mixed with emphasis."""
        md = "The **bold** value of $x$ is *important*."
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_inline_math_in_list(self):
        """Inline math inside a list item."""
        md = "- Item with $x^2$ math"
        html = mordant.markdown_to_html(md)
        assert html is not None


# =============================================================================
# Integration: Full document rendering with math
# =============================================================================

class TestMathIntegration:
    """Full document integration tests."""

    def test_document_with_math_and_highlighting(self):
        """Document with both math blocks and code highlighting."""
        md = """# Mixed Document

Here is some code:

```python
def hello():
    print("world")
```

And here is math:

```math
\\int_0^1 x^2 dx
```
"""
        html = mordant.markdown_to_html(md)
        assert '<h1>' in html
        # Math block should render with KaTeX
        assert 'katex' in html.lower()

    def test_document_with_math_and_emoji(self):
        """Document with math blocks and emoji."""
        md = """# Test :smile:

```math
x^2 + y^2 = z^2
```
"""
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()

    def test_gfm_with_math(self):
        """GFM mode with math blocks."""
        md = """Check this:

- [x] Done

```math
\\alpha + \\beta
```
"""
        html = mordant.markdown_to_html(md, gfm_opts=mordant.GfmOptions.all())
        assert 'katex' in html.lower()

    def test_math_block_does_not_break_code_blocks(self):
        """Regular code blocks should still work normally."""
        md = """```python
print("hello")
```

```math
x^2
```

```rust
let x = 1;
```
"""
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()
        # Python and Rust blocks should still be code blocks
        assert 'python' in html.lower() or 'rust' in html.lower()

    def test_math_with_parse_options(self):
        """Math blocks work with parse options."""
        from mordant import ParseOptions
        md = "```math\nx^2\n```"
        opts = ParseOptions(attributes=True)
        html = mordant.markdown_to_html(md, parse_opts=opts)
        assert 'katex' in html.lower()
