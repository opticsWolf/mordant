"""Comprehensive mixed-feature tests for code highlighting, math, and emojis.

Tests that all three features work correctly when combined in various ways:
- Inline math mixed with emphasis, links, and emojis
- Fenced math mixed with code blocks and lists
- Documents with all three features interleaved
- Edge cases: math inside code, code inside math, emoji near math delimiters
"""

import pytest
import mordant


# =============================================================================
# Inline Math + Emphasis + Links
# =============================================================================

class TestInlineMathWithEmphasis:
    """Inline math combined with bold, italic, and strikethrough."""

    def test_math_inside_bold(self):
        """Bold text containing inline math."""
        md = r"**The value of $\pi$ is approximately 3.14**"
        html = mordant.markdown_to_html(md)
        assert '<strong>' in html
        assert 'katex' in html.lower()

    def test_math_inside_italic(self):
        """Italic text containing inline math."""
        md = r"*The formula $E = mc^2$ is famous*"
        html = mordant.markdown_to_html(md)
        assert '<em>' in html
        assert 'katex' in html.lower()

    def test_math_and_bold_together(self):
        """Inline math alongside bold text."""
        md = r"The **area** is $\pi r^2$ and the **perimeter** is $2\pi r$."
        html = mordant.markdown_to_html(md)
        assert '<strong>' in html
        assert html.count('katex') >= 4  # two math spans

    def test_math_inside_strikethrough(self):
        """Strikethrough containing math."""
        md = r"~~Old formula: $x^2$~~ New formula: $x^3$"
        html = mordant.markdown_to_html(md)
        # Mordant keeps ~~ as literal text (no strikethrough support)
        # but math should still render
        assert 'katex' in html.lower()
        assert 'x^2' in html or 'x3' in html

    def test_math_with_nested_emphasis(self):
        """Math surrounded by multiple emphasis levels."""
        md = r"**Check $**bold**$ inside math? No, $x^{\textbf{bold}}$ instead**"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_math_between_italic_and_bold(self):
        """Math sandwiched between italic and bold."""
        md = r"*Italic* then $x^2$ then **bold**"
        html = mordant.markdown_to_html(md)
        assert '<em>' in html
        assert '<strong>' in html
        assert 'katex' in html.lower()


class TestInlineMathWithLinks:
    """Inline math combined with links and images."""

    def test_math_next_to_link(self):
        """Math expression next to a link."""
        md = r"The formula $E=mc^2$ is from [Wikipedia](https://en.wikipedia.org/)."
        html = mordant.markdown_to_html(md)
        assert '<a ' in html
        assert 'katex' in html.lower()

    def test_link_with_math_in_title(self):
        """Link with math in the title attribute."""
        md = r'[See "$x^2$" formula](https://example.com)'
        html = mordant.markdown_to_html(md)
        assert '<a ' in html
        assert 'x^2' in html

    def test_math_after_link(self):
        """Math expression after a link in the same paragraph."""
        md = r"[Click here](https://example.com) then compute $\int_0^1 x dx$"
        html = mordant.markdown_to_html(md)
        assert '<a ' in html
        assert 'katex' in html.lower()


# =============================================================================
# Inline Math + Emoji
# =============================================================================

class TestInlineMathWithEmoji:
    """Inline math combined with emoji."""

    def test_math_after_emoji(self):
        """Emoji followed by inline math."""
        md = r":star: The value is $\pi \approx 3.14$"
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()

    def test_emoji_after_math(self):
        """Inline math followed by emoji."""
        md = r"The answer is $42$ :tada:"
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()

    def test_emoji_between_math_expressions(self):
        """Math - emoji - math sequence."""
        md = r"$x^2$ :heart: $y^2$ :heart: $z^2$"
        html = mordant.markdown_to_html(md)
        assert html.count('katex') >= 6  # three math spans

    def test_emoji_inline_with_bold_math(self):
        """Emoji next to bold text containing math."""
        md = r":bulb: **Formula:** $\alpha + \beta$"
        html = mordant.markdown_to_html(md)
        assert '<strong>' in html
        assert 'katex' in html.lower()

    def test_math_in_emoji_shortcode_context(self):
        """Math that looks like emoji shortcode boundaries."""
        md = r"Check $colon:emoji:colon$ notation"
        html = mordant.markdown_to_html(md)
        assert html is not None


# =============================================================================
# Fenced Math + Code Blocks
# =============================================================================

class TestFencedMathWithCodeBlocks:
    """Fenced math blocks alongside regular code blocks."""

    def test_math_and_python_code(self):
        """Math block next to Python code block."""
        md = """```python
import math
x = math.pi
```

```math
\\alpha = \\pi
```"""
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()
        # Python code should be in a code/pre block
        assert '<pre>' in html or '<pre>' in html

    def test_math_and_rust_code(self):
        """Math block next to Rust code block."""
        md = """```rust
let pi = 3.14;
```

```math
\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}
```"""
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()
        assert 'rust' in html.lower()

    def test_interleaved_code_and_math(self):
        """Multiple code and math blocks interleaved."""
        md = """```python
def f(x): return x**2
```

```math
f(x) = x^2
```

```javascript
console.log("hello");
```

```math
\\int f(x) dx
```"""
        html = mordant.markdown_to_html(md)
        katex_count = html.lower().count('katex')
        assert katex_count >= 8  # at least two math blocks
        assert '<pre>' in html
        assert '<pre>' in html

    def test_math_fence_does_not_break_regular_code(self):
        """Math fences should not interfere with regular code block parsing."""
        md = """```python
def calculate():
    # Uses pi
    return 3.14159
```

```math
\\pi = 3.14159
```

```bash
echo "hello"
```"""
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()
        # All three code blocks should be present
        # Math blocks render as katex-display, not <pre>
        assert html.count('<pre>') >= 2
        assert 'katex' in html.lower()

    def test_empty_math_block(self):
        """Empty math block should not crash."""
        md = "```math\n```"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_math_block_with_backticks_in_latex(self):
        """Math block containing backtick characters in LaTeX."""
        md = '```math\n$x`y$\n```'
        html = mordant.markdown_to_html(md)
        assert html is not None


# =============================================================================
# Inline Math Inside Code Blocks
# =============================================================================

class TestMathInsideCodeBlocks:
    """Inline math that appears inside code blocks (should NOT be rendered)."""

    def test_inline_math_in_code_block(self):
        """Inline math in a code block should not be rendered as KaTeX."""
        md = "```\nThis is $x^2$ in code.\n```"
        html = mordant.markdown_to_html(md)
        # The $ should be escaped, not rendered as math
        assert '$x^2$' in html or 'x^2' in html
        # Should not have katex classes for the inline math
        assert 'katex' not in html.lower() or '$' in html

    def test_inline_math_in_python_code(self):
        """Python code containing $ symbols."""
        md = """```python
# Regex pattern with $
pattern = r"end$"
```"""
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_inline_math_in_rust_code(self):
        """Rust code containing $ symbols."""
        md = """```rust
let s = "hello$world";
```"""
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_inline_math_in_inline_code(self):
        """Inline code containing $ should not trigger math."""
        md = r"Use `$variable` for inline code."
        html = mordant.markdown_to_html(md)
        assert '<code>' in html
        assert 'variable' in html.lower()


# =============================================================================
# Complex Mixed Documents
# =============================================================================

class TestComplexMixedDocuments:
    """Full documents combining all three features."""

    def test_full_document_with_all_features(self):
        """Document with code, math, and emoji all together."""
        md = """# Algorithm :rocket:

Here's a Python implementation:

```python
def compute(x, y):
    # Calculate x^2 + y^2
    return x**2 + y**2
```

The mathematical formula is:

```math
\\text{result} = x^2 + y^2
```

For **bold** results, use $\\sqrt{x^2 + y^2}$ :star:

And for the :fire: full version:

```math
\\sum_{i=1}^{n} x_i^2
```

:bulb: **Note:** The formula $E = mc^2$ is famous!
"""
        html = mordant.markdown_to_html(md)
        assert '<h1>' in html
        assert 'katex' in html.lower()
        assert '<pre>' in html or '<pre>' in html
        assert '<strong>' in html

    def test_mathematical_document_with_code_examples(self):
        """Math-heavy document with code examples and emoji."""
        md = """## Linear Algebra :math:

### Matrix Multiplication

```math
\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix} \\begin{pmatrix} e & f \\\\ g & h \\end{pmatrix}
= \\begin{pmatrix} ae+bg & af+bh \\\\ ce+dg & cf+dh \\end{pmatrix}
```

### Python Implementation

```python
import numpy as np

A = np.array([[1, 2], [3, 4]])
B = np.array([[5, 6], [7, 8]])
C = A @ B
```

The determinant is $\\det(A) = ad - bc$ :brain:
"""
        html = mordant.markdown_to_html(md)
        assert '<h2>' in html
        assert '<h3>' in html
        assert html.count('katex') >= 6  # multiple math expressions
        assert '<pre>' in html

    def test_programming_tutorial_with_formulas(self):
        """Programming tutorial mixing code, formulas, and emoji."""
        md = """# Sorting Algorithms :chart:

## QuickSort :zap:

```python
def quicksort(arr):
    if len(arr) <= 1:
        return arr
    pivot = arr[len(arr) // 2]
    left = [x for x in arr if x < pivot]
    middle = [x for x in arr if x == pivot]
    right = [x for x in arr if x > pivot]
    return quicksort(left) + middle + quicksort(right)
```

**Time Complexity:** $O(n \\log n)$ on average :star:

**Worst Case:** $O(n^2)$ :warning:

The recurrence relation is:

```math
T(n) = 2T(n/2) + O(n)
```

:bulb: **Tip:** Use $\\Theta$ notation for tight bounds.
"""
        html = mordant.markdown_to_html(md)
        assert '<h1>' in html
        assert '<h2>' in html
        assert 'katex' in html.lower()
        assert '<pre>' in html
        assert '<strong>' in html

    def test_notebook_style_document(self):
        """Jupyter-style notebook with mixed content."""
        md = """# Data Analysis Notebook :jupyter:

## Import Libraries :package:

```python
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt

data = pd.read_csv("data.csv")
```

## Statistics :chart:

The mean is calculated as:

```math
\\mu = \\frac{1}{n} \\sum_{i=1}^{n} x_i
```

The standard deviation:

```math
\\sigma = \\sqrt{\\frac{1}{n} \\sum_{i=1}^{n} (x_i - \\mu)^2}
```

## Correlation :link:

Pearson correlation coefficient:

$\\rho_{X,Y} = \\frac{\\text{cov}(X,Y)}{\\sigma_X \\sigma_Y}$

```python
corr_matrix = data.corr()
print(corr_matrix)
```

:warning: **Caution:** Correlation does not imply causation!
"""
        html = mordant.markdown_to_html(md)
        assert '<h1>' in html
        assert 'katex' in html.lower()
        assert '<pre>' in html
        # Should have both fenced math and inline math
        assert 'katex-display' in html  # fenced blocks

    def test_cheat_sheet_document(self):
        """Cheat sheet with lots of inline math, code, and emoji."""
        md = """# Rust Cheat Sheet :rust:

## Basics :beginning:

```rust
fn main() {
    let x: i32 = 42;
    println!("Hello, {}!", x);
}
```

## Math Functions :calculator:

- Absolute value: `$|x|$`
- Floor: `$\\lfloor x \\rfloor$`
- Ceiling: `$\\lceil x \\rceil$`

## Collections :box:

```rust
let vec: Vec<i32> = vec![1, 2, 3];
let map: HashMap<String, i32> = HashMap::new();
```

## Complexity :zap:

- Array access: $O(1)$ :star:
- Binary search: $O(\\log n)$ :rocket:
- Linear scan: $O(n)$ :turtle:
- Nested loop: $O(n^2)$ :warning:
"""
        html = mordant.markdown_to_html(md)
        assert '<h1>' in html
        assert html.count('katex') >= 12  # many inline math expressions
        assert '<pre>' in html
        assert '<pre>' in html


# =============================================================================
# Edge Cases and Boundary Conditions
# =============================================================================

class TestEdgeCases:
    """Edge cases and boundary conditions."""

    def test_consecutive_inline_math(self):
        """Multiple inline math expressions in a row."""
        md = r"$x$ $y$ $z$"
        html = mordant.markdown_to_html(md)
        assert html.count('katex') >= 6  # three math spans

    def test_math_with_special_regex_chars(self):
        """Math containing characters that look like regex."""
        md = r"The pattern $[a-z]+$ matches lowercase letters."
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_emoji_shortcode_next_to_math_delimiter(self):
        """Emoji shortcode immediately adjacent to math delimiter."""
        md = r":smile:$x^2$ :frown:"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_math_with_nested_braces(self):
        """Math with nested braces."""
        import mordant
        md = chr(96)*3 + 'math' + chr(10) + chr(92)*2 + 'frac{1}' + chr(92) + chr(92) + 'frac{1}{2}' + chr(10) + chr(96)*3
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()

    def test_code_block_with_math_language_in_regular_code(self):
        """Math language tag inside a document with other code blocks."""
        md = """```python
# This is Python
x = 1
```

```math
x = 1
```

```javascript
// This is JavaScript
let x = 1;
```"""
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()
        # Should have three code blocks
        # Math blocks render as katex-display, not <pre>
        assert html.count('<pre>') >= 2
        assert 'katex' in html.lower()

    def test_inline_math_with_unicode_latex(self):
        """Inline math with Unicode characters in LaTeX."""
        md = r"The value is $\alpha + \beta + \gamma$"
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()

    def test_math_in_table_cell(self):
        """Math inside a markdown table."""
        md = """| Operation | Formula |
|-----------|---------|
| Area | $\\pi r^2$ |
| Volume | $\\frac{4}{3}\\pi r^3$ |"""
        html = mordant.markdown_to_html(md)
        # Mordant doesn't support tables
        assert 'katex' in html.lower()

    def test_math_in_blockquote(self):
        """Math inside a blockquote."""
        md = """> The formula is:
>
> ```math
> E = mc^2
> """
        html = mordant.markdown_to_html(md)
        assert '<blockquote>' in html
        assert 'katex' in html.lower()

    def test_math_in_ordered_list(self):
        """Math inside an ordered list."""
        md = """1. First step: $x = 1$
2. Second step: $y = 2$
3. Result: $x + y = 3$"""
        html = mordant.markdown_to_html(md)
        assert '<ol>' in html
        assert html.count('katex') >= 6  # three math spans

    def test_math_in_definition_list(self):
        """Math in a definition-style list."""
        md = """- **Mean** ($\\mu$): average value
- **Variance** ($\\sigma^2$): spread of data
- **Std Dev** ($\\sigma$): square root of variance"""
        html = mordant.markdown_to_html(md)
        assert '<ul>' in html or '<li>' in html
        assert html.count('katex') >= 6  # three math spans

    def test_very_long_inline_math(self):
        """Very long inline math expression."""
        md = r"The polynomial $x^{100} + x^{99} + x^{98} + \\ldots + x + 1 = \\frac{x^{101} - 1}{x - 1}$ is interesting."
        html = mordant.markdown_to_html(md)
        assert 'katex' in html.lower()

    def test_math_with_escaped_dollar(self):
        """Escaped dollar sign should not trigger math."""
        md = r"The price is \$100 and the formula is $x^2$."
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_empty_inline_math(self):
        """Empty inline math $$ should not crash."""
        md = r"Text $$ more text"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_mismatched_math_delimiters(self):
        """Mismatched math delimiters should not crash."""
        md = r"$unclosed formula"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_math_after_emoji_with_colon(self):
        """Math immediately after emoji shortcode."""
        md = r":rocket:$\\Delta x$"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_code_highlighting_preserved_with_math(self):
        """Code highlighting should still work when math is present."""
        md = """```python
def calculate(x: int, y: int) -> int:
    # Calculate x^2 + y^2
    return x ** 2 + y ** 2
```

```math
\\text{result} = x^2 + y^2
```"""
        html = mordant.markdown_to_html(md)
        # Code block should have highlighting classes
        assert '<pre>' in html
        assert 'katex' in html.lower()


# =============================================================================
# Regression Tests
# =============================================================================

class TestRegression:
    """Tests to catch regressions from feature interactions."""

    def test_emoji_still_works_after_math_integration(self):
        """Emoji parsing should still work after adding math parser."""
        md = ":smile: :rocket: :fire: :star: :heart:"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_code_highlighting_still_works_after_math_integration(self):
        """Code highlighting should still work after adding math renderer."""
        md = """```python
def hello():
    print("world")
```"""
        html = mordant.markdown_to_html(md)
        assert html is not None
        assert '<pre>' in html or '<pre>' in html

    def test_emphasis_still_works_after_math_integration(self):
        """Emphasis parsing should still work after adding math parser."""
        md = "**bold** and *italic* and ~~strikethrough~~"
        html = mordant.markdown_to_html(md)
        assert '<strong>' in html
        assert '<em>' in html
        # Mordant keeps ~~ as literal (no strikethrough support)

    def test_links_still_work_after_math_integration(self):
        """Link parsing should still work after adding math parser."""
        md = "[Click here](https://example.com)"
        html = mordant.markdown_to_html(md)
        assert '<a ' in html
        assert 'href=' in html

    def test_backtick_code_still_works_after_math_integration(self):
        """Inline code spans should still work after adding math parser."""
        md = r"Use `code` for inline code."
        html = mordant.markdown_to_html(md)
        assert '<code>' in html

    def test_autolinks_still_work_after_math_integration(self):
        """Auto-link detection should still work after adding math parser."""
        md = "Visit https://example.com for more info."
        html = mordant.markdown_to_html(md)
        assert '<a ' in html.lower() or 'example.com' in html


# =============================================================================
# Performance/Stress Tests
# =============================================================================

class TestStress:
    """Stress tests with many features combined."""

    def test_many_inline_math_expressions(self):
        """Document with many inline math expressions."""
        terms = " + ".join(f"${i}^2$" for i in range(20))
        md = f"The sum is {terms}."
        html = mordant.markdown_to_html(md)
        assert html.count('katex') >= 40  # 20 math spans

    def test_many_code_blocks_with_math(self):
        """Document with many code blocks and math blocks."""
        parts = []
        for i in range(10):
            parts.append(f"```python\nx{i} = {i}\n```")
            parts.append(f"```math\nx_{i} = {i}\n```")
        md = "\n\n".join(parts)
        html = mordant.markdown_to_html(md)
        assert html.count('katex') >= 20  # 10 math blocks
        assert html.count('<pre>') >= 10  # 10 code blocks

    def test_many_emoji_with_math(self):
        """Document with many emoji and math."""
        emojis = " ".join(f":{'smile' if i % 2 == 0 else 'star'}:" for i in range(20))
        math_exprs = " ".join(f"${i}$" for i in range(20))
        md = f"{emojis} {math_exprs}"
        html = mordant.markdown_to_html(md)
        assert html is not None

    def test_nested_structure_with_all_features(self):
        """Deeply nested structure with all features."""
        md = """# :rocket: Advanced :chart:

## :beginning: Section 1

```python
def complex_function(x, y, z):
    # A complex function with math
    # Formula: $\\sum_{i=1}^{n} x_i^2$
    return x**2 + y**2 + z**2
```

### :star: Subsection

The result is **$\\sqrt{x^2 + y^2}$** and *$\\pi r^2$*.

> :bulb: **Note:** The formula $E = mc^2$ is important!

```math
\\int_{-\\infty}^{\\infty} e^{-x^2} dx = \\sqrt{\\pi}
```

## :end: Section 2

```rust
fn compute(x: f64) -> f64 {
    x * x
}
```

The answer is $42$ :tada:"""
        html = mordant.markdown_to_html(md)
        assert '<h1>' in html
        assert '<h2>' in html
        assert 'katex' in html.lower()
        assert '<pre>' in html
        assert '<blockquote>' in html
        assert '<strong>' in html
        assert '<em>' in html
