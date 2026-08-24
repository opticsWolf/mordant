# Mordant

[![CI](https://github.com/opticsWolf/mordant/actions/workflows/test.yml/badge.svg)](https://github.com/opticsWolf/mordant/actions/workflows/test.yml)
[![License](https://img.shields.io/github/license/opticsWolf/mordant)](https://github.com/opticsWolf/mordant/blob/main/LICENSE)
[![PyPI - Version](https://img.shields.io/pypi/v/mordant)](https://pypi.org/project/mordant/)
[![PyPI - Python Version](https://img.shields.io/pypi/pyversions/mordant)](https://pypi.org/project/mordant/)
[![crates.io](https://img.shields.io/crates/v/mordant)](https://crates.io/crates/mordant)
[![Rust](https://img.shields.io/badge/Rust-1.87+-orange)](https://www.rust-lang.org)

> **Version:** 0.9.0 (Python and Rust crates in lockstep)
> **Python:** 3.9+ · **Bindings:** PyO3 0.29

A fast CommonMark + GFM Markdown parser and renderer for Python, powered by the [`mordant`](https://crates.io/crates/mordant) Rust library (itself based on [rushdown](https://github.com/yuin/rushdown)).

This is the **Python bindings** package. The full documentation lives in the [repository](https://github.com/opticsWolf/mordant):

- [README](https://github.com/opticsWolf/mordant#readme) — features, install, quick start
- [Quick Reference](https://github.com/opticsWolf/mordant/blob/main/docs/QUICKREF.md) — complete Python API reference
- [Architecture](https://github.com/opticsWolf/mordant/blob/main/docs/ARCHITECTURE.md) — internals and design

## Features

- **Blazing fast** — one of the fastest Markdown parsers for Python
- **CommonMark 0.31.2 + GFM** — tables, task lists, strikethrough
- **Full AST access** — `parse()` returns a `Document` with tree traversal (`Node`, `Walker`)
- **YAML frontmatter** — typed metadata extraction
- **Emoji** — `:shortcode:` rendering with blacklists and custom templates
- **Math** — LaTeX via KaTeX (fenced ```` ```math ```` blocks, inline `$…$`/`$$…$$`, standalone `render_math()`)
- **Mermaid diagrams** — server-side SVG by default, client/hybrid modes, themeable
- **Footnotes** — PHP Markdown Extra style
- **Lint engine** — 25 markdownlint-style rules, auto-fix, batch API, CLI (`python -m mordant`)
- **Document chunking** — `MarkdownChunker` for RAG/embedding pipelines
- **Syntax highlighting** — 190+ languages, VSCode JSON / Sublime `.tmTheme` themes
- **Multi-threaded** — GIL released during parse/render

## Install

```bash
pip install mordant
```

## Quick Start

```python
import mordant

html = mordant.markdown_to_html("# Hello\n\n**World**")
# '<h1>Hello</h1>\n<p><strong>World</strong></p>\n'

doc = mordant.parse("# Title\n\nSome text.")
print(doc.metadata)      # YAML frontmatter
for node in doc.walk():  # AST traversal
    ...

result = mordant.lint("# Heading\n### Skipped level\n")
```

See the [Quick Reference](https://github.com/opticsWolf/mordant/blob/main/docs/QUICKREF.md) for the full API.

## License

MIT
