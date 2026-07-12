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


# =============================================================================
# Bug A: fenced ```math / ```latex must render as KaTeX even WITH a theme
# =============================================================================

class TestMathFenceWithHighlighting:
    """Regression for Bug A.

    md_viewer always passes a highlighting_theme. Previously the code highlighter
    (last-registered CodeBlock renderer) shadowed the math-fence renderer, so
    ```math / ```latex blocks were emitted as highlighted code instead of KaTeX.
    """

    def test_math_fence_with_highlighting(self):
        html = mordant.markdown_to_html(
            "```math\nE = mc^2\n```",
            highlighting_theme="InspiredGitHub",
        )
        assert "katex" in html.lower()
        assert "katex-display" in html
        assert "language-math" not in html  # not rendered as a code block

    def test_latex_fence_with_highlighting(self):
        html = mordant.markdown_to_html(
            "```latex\nE = mc^2\n```",
            highlighting_theme="InspiredGitHub",
        )
        assert "katex" in html.lower()
        assert "language-latex" not in html

    def test_other_code_blocks_still_highlighted(self):
        html = mordant.markdown_to_html(
            "```python\nx = 1\n```",
            highlighting_theme="InspiredGitHub",
        )
        assert "language-python" in html  # still highlighted
        assert "katex" not in html        # not mistaken for math


# =============================================================================
# Bug B: multi-line $$...$$ display math
# =============================================================================

class TestMultiLineDisplayMath:
    """Regression for Bug B: `$$` on its own line, content on following lines."""

    def test_multiline_display_math(self):
        html = mordant.markdown_to_html("$$\nE = mc^2\n$$")
        assert "katex" in html.lower()
        assert "katex-display" in html
        assert "E = mc" in html  # content preserved

    def test_multiline_display_math_with_highlighting(self):
        html = mordant.markdown_to_html(
            "$$\nE = mc^2\n$$",
            highlighting_theme="InspiredGitHub",
        )
        assert "katex" in html.lower()
        assert "E = mc" in html

    def test_inline_math_regression(self):
        html = mordant.markdown_to_html("Inline $x^2$ here.")
        assert "katex" in html.lower()
        assert "katex-display" not in html

    def test_single_line_display_math_regression(self):
        html = mordant.markdown_to_html("$$x^2$$")
        assert "katex" in html.lower()
        assert "katex-display" in html

    def test_unbalanced_display_math_stays_literal(self):
        html = mordant.markdown_to_html("$$\nthis has no closer")
        assert "katex" not in html.lower()
        assert "$$" in html
        assert "this has no closer" in html


# =============================================================================
# KATEX_CSS constant
# =============================================================================

class TestKatexCss:
    """Test the embedded KaTeX CSS constant."""

    def test_katex_css_exists(self):
        """mordant.KATEX_CSS is accessible and non-empty."""
        assert hasattr(mordant, "KATEX_CSS")
        assert isinstance(mordant.KATEX_CSS, str)
        assert len(mordant.KATEX_CSS) > 10000  # ~23KB minified

    def test_katex_css_contains_font_face(self):
        """CSS contains @font-face declarations."""
        assert "@font-face" in mordant.KATEX_CSS
        assert "KaTeX_Main" in mordant.KATEX_CSS

    def test_katex_css_contains_class_rules(self):
        """CSS contains .katex and .katex-display rules."""
        assert ".katex" in mordant.KATEX_CSS
        assert ".katex-display" in mordant.KATEX_CSS

    def test_katex_css_version(self):
        """CSS declares version 0.16.21."""
        assert "0.16.21" in mordant.KATEX_CSS


# =============================================================================
# MathRendererOptions
# =============================================================================

class TestMathRendererOptions:
    """Test the PyMathRendererOptions class and math_renderer_opts parameter."""

    def test_pymathrendereroptions_default(self):
        """PyMathRendererOptions() defaults to 'both'."""
        opts = mordant.PyMathRendererOptions()
        assert opts.output == "both"

    def test_pymathrendereroptions_html(self):
        """PyMathRendererOptions(output='html')."""
        opts = mordant.PyMathRendererOptions(output="html")
        assert opts.output == "html"

    def test_pymathrendereroptions_mathml(self):
        """PyMathRendererOptions(output='mathml')."""
        opts = mordant.PyMathRendererOptions(output="mathml")
        assert opts.output == "mathml"

    def test_math_renderer_opts_mathml(self):
        """markdown_to_html with math_renderer_opts=output='mathml' produces MathML (no katex-mathml wrapper)."""
        opts = mordant.PyMathRendererOptions(output="mathml")
        html = mordant.markdown_to_html(
            "```math\nE = mc^2\n```",
            highlighting_theme="InspiredGitHub",
            math_renderer_opts=opts,
        )
        assert "<math" in html
        # "mathml" output: MathML directly inside katex spans (no katex-mathml wrapper)
        assert "katex-mathml" not in html

    def test_math_renderer_opts_html_only(self):
        """markdown_to_html with math_renderer_opts=output='html' produces only KaTeX HTML."""
        opts = mordant.PyMathRendererOptions(output="html")
        html = mordant.markdown_to_html(
            "```math\nE = mc^2\n```",
            highlighting_theme="InspiredGitHub",
            math_renderer_opts=opts,
        )
        assert "katex" in html
        assert "<math" not in html

    def test_math_renderer_opts_both(self):
        """markdown_to_html with math_renderer_opts=output='both' produces both (katex-mathml wrapper)."""
        opts = mordant.PyMathRendererOptions(output="both")
        html = mordant.markdown_to_html(
            "```math\nE = mc^2\n```",
            highlighting_theme="InspiredGitHub",
            math_renderer_opts=opts,
        )
        assert "katex" in html
        assert "<math" in html
        # "both" output: MathML inside katex-mathml wrapper span
        assert "katex-mathml" in html

    def test_math_renderer_opts_inline_math(self):
        """math_renderer_opts also affects inline $...$ and block $$...$$."""
        opts = mordant.PyMathRendererOptions(output="mathml")
        html = mordant.markdown_to_html(
            "Inline $x^2$ and block $$y^2$$.",
            math_renderer_opts=opts,
        )
        assert "<math" in html
        assert "katex-mathml" not in html

    def test_math_renderer_opts_multiline_display(self):
        """math_renderer_opts works with multi-line $$...$."""
        opts = mordant.PyMathRendererOptions(output="mathml")
        html = mordant.markdown_to_html(
            "$$\na + b = c\n$$",
            math_renderer_opts=opts,
        )
        assert "<math" in html
        assert "katex-mathml" not in html

    def test_math_renderer_opts_without_highlighting(self):
        """math_renderer_opts works without a highlighting theme (math fence path)."""
        opts = mordant.PyMathRendererOptions(output="mathml")
        html = mordant.markdown_to_html(
            "```math\n" + "\\int_0^1 x dx" + "\n```",
            math_renderer_opts=opts,
        )
        assert "<math" in html
        assert "katex-mathml" not in html
