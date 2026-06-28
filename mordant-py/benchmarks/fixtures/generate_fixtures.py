"""Generate benchmark fixtures of varying sizes."""

import os

FIXTURES_DIR = os.path.dirname(__file__) or os.getcwd()


def generate_small():
    """Generate a small markdown document (~40 lines)."""
    content = """---
title: Small Document
author: Benchmark
---

# Hello World

This is a **small** markdown document with `inline code` and *italic text*.

## Features

- Item one
- Item two
- Item three

### Code Example

```python
def hello():
    print("Hello, World!")
```

## Tables

| Name | Age |
|------|-----|
| Alice | 30 |
| Bob | 25 |

---

> A blockquote for good measure.

[Link](https://example.com)
"""
    with open(os.path.join(FIXTURES_DIR, "small.md"), "w") as f:
        f.write(content)
    print(f"Created small.md: {len(content)} chars, {len(content.splitlines())} lines")


def generate_medium():
    """Generate a medium markdown document (~400 lines)."""
    lines = []
    lines.append("---")
    lines.append("title: Medium Document")
    lines.append("description: A medium-sized markdown document for benchmarking")
    lines.append("tags: [benchmark, test, medium]")
    lines.append("---")
    lines.append("")
    lines.append("# Medium Document")
    lines.append("")
    lines.append("This is a medium-sized markdown document used for benchmarking.")
    lines.append("It contains various markdown elements to test parser performance.")
    lines.append("")
    lines.append("## Section 1: Lists")
    lines.append("")
    lines.append("### Unordered List")
    for i in range(30):
        lines.append(f"- Item {i + 1} with some descriptive text to add length")
    lines.append("")
    lines.append("### Ordered List")
    for i in range(20):
        lines.append(f"{i + 1}. Step {i + 1} of the process")
    lines.append("")
    lines.append("## Section 2: Code Blocks")
    lines.append("")
    lines.append("```python")
    lines.append("def fibonacci(n):")
    lines.append("    if n <= 1:")
    lines.append("        return n")
    lines.append("    return fibonacci(n - 1) + fibonacci(n - 2)")
    lines.append("")
    lines.append("for i in range(10):")
    lines.append('    print(fibonacci(i))')
    lines.append("```")
    lines.append("")
    lines.append("```javascript")
    lines.append("const greet = (name) => {")
    lines.append('    console.log(`Hello, ${name}!`);')
    lines.append("};")
    lines.append('greet("World");')
    lines.append("```")
    lines.append("")
    lines.append("## Section 3: Tables")
    lines.append("")
    lines.append("| Column 1 | Column 2 | Column 3 |")
    lines.append("|----------|----------|----------|")
    for i in range(30):
        lines.append(f"| Value {i} | Data {i} | Result {i} |")
    lines.append("")
    lines.append("## Section 4: Blockquotes")
    lines.append("")
    for i in range(10):
        lines.append(f"> Quote paragraph {i + 1} with some meaningful content about programming.")
    lines.append("")
    lines.append("## Section 5: Inline Formatting")
    lines.append("")
    lines.append('This has **bold**, *italic*, ~~strikethrough~~, and `code` text.')
    lines.append("[links](https://example.com)")
    lines.append("and ![images](https://example.com/img.png).")
    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append("## Section 6: Nested Lists")
    lines.append("")
    lines.append("- Parent item 1")
    lines.append("  - Child item 1.1")
    lines.append("    - Grandchild 1.1.1")
    lines.append("    - Grandchild 1.1.2")
    lines.append("  - Child item 1.2")
    lines.append("- Parent item 2")
    lines.append("  - Child item 2.1")
    lines.append("  - Child item 2.2")
    lines.append("    - Grandchild 2.2.1")
    lines.append("    - Grandchild 2.2.2")
    lines.append("    - Grandchild 2.2.3")
    lines.append("    - Grandchild 2.2.4")
    lines.append("")
    lines.append("## Section 7: Math and Special Characters")
    lines.append("")
    lines.append("Math: E = mc^2 and summation notation.")
    lines.append("Special: &amp; &lt; &gt; &copy; &euro;")
    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append("## Section 8: Horizontal Rules and Breaks")
    lines.append("")
    lines.append("---")
    lines.append("* * *")
    lines.append("***")
    lines.append("")
    lines.append("## Section 9: Definition List")
    lines.append("")
    lines.append("Python")
    lines.append(": A high-level programming language")
    lines.append("")
    lines.append("Rust")
    lines.append(": A systems programming language")
    lines.append("")
    lines.append("Markdown")
    lines.append(": A lightweight markup language")
    lines.append("")
    lines.append("---")
    lines.append("")
    lines.append("## Conclusion")
    lines.append("")
    lines.append("This medium document tests various markdown features.")
    lines.append("It includes lists, code blocks, tables, blockquotes, inline formatting, and more.")

    content = "\n".join(lines)
    with open(os.path.join(FIXTURES_DIR, "medium.md"), "w") as f:
        f.write(content)
    print(f"Created medium.md: {len(content)} chars, {len(lines)} lines")


def generate_large():
    """Generate a large markdown document (~2000 lines)."""
    lines = []
    lines.append("---")
    lines.append("title: Large Document")
    lines.append("description: A large markdown document for stress testing parsers")
    lines.append("tags: [benchmark, stress-test, large]")
    lines.append("---")
    lines.append("")
    lines.append("# Large Document")
    lines.append("")
    lines.append("This is a large markdown document designed to stress-test markdown parsers.")
    lines.append("It contains thousands of lines with various markdown elements.")
    lines.append("")

    # Multiple sections with lists
    for section in range(1, 11):
        lines.append(f"## Section {section}")
        lines.append("")
        lines.append(f"This is section {section} with various content types.")
        lines.append("")

        # Unordered lists
        lines.append("### List Items")
        lines.append("")
        for i in range(20):
            lines.append(f"- Section {section}, item {i + 1} with descriptive content")
        lines.append("")

        # Tables
        lines.append("### Data Table")
        lines.append("")
        lines.append("| Field 1 | Field 2 | Field 3 | Field 4 |")
        lines.append("|---------|---------|---------|---------|")
        for i in range(15):
            lines.append(f"| Val {section}-{i} | Data {i} | Result {i} | Score {i} |")
        lines.append("")

        # Blockquotes
        lines.append("### Quote")
        lines.append("")
        lines.append(f"> This is a quote from section {section}, paragraph 1.")
        lines.append(f"> It contains meaningful content about the topic.")
        lines.append("")

        # Code blocks
        lines.append("### Code Example")
        lines.append("")
        lines.append("```python")
        lines.append(f"def process_section_{section}(data):")
        lines.append("    result = []")
        lines.append("    for item in data:")
        lines.append("        result.append(item * 2)")
        lines.append("    return result")
        lines.append("")
        lines.append(f"# Process section {section}")
        lines.append(f"data_{section} = [1, 2, 3, 4, 5]")
        lines.append(f"output_{section} = process_section_{section}(data_{section})")
        lines.append("```")
        lines.append("")

        # Paragraphs
        lines.append("### Description")
        lines.append("")
        for para in range(5):
            lines.append(f"This is paragraph {para + 1} of section {section}. " * 3)
            lines.append("")

    # Final section
    lines.append("---")
    lines.append("")
    lines.append("## Conclusion")
    lines.append("")
    lines.append("This large document has been parsed and rendered successfully.")
    lines.append("It tests the parser's ability to handle many sections, lists, tables, and code blocks.")

    content = "\n".join(lines)
    with open(os.path.join(FIXTURES_DIR, "large.md"), "w") as f:
        f.write(content)
    print(f"Created large.md: {len(content)} chars, {len(lines)} lines")


if __name__ == "__main__":
    generate_small()
    generate_medium()
    generate_large()
    print("\nAll fixtures generated.")
