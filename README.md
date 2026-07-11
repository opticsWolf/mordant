# Mordant

> **Version:** 0.8.8  
> **Rust:** rushdown v0.18.0 (CommonMark 0.31.2 + GFM)  
> **Python:** 3.9+  
> **Bindings:** PyO3 0.29

A fast CommonMark + GFM Markdown parser and renderer for Python, powered by the [rushdown](https://github.com/yuin/rushdown) Rust library.

- [Architecture](ARCHITECTURE.md) — Full architecture documentation
- [Quick Reference](QUICKREF.md) — Python bindings quick reference

## What's New in 0.8.8

- **Server-side Mermaid rendering** — Mermaid diagrams now render as inline SVG via the `mermaid-rs-renderer` crate (~3ms server-side vs ~2s client-side). No browser/CDN dependency. Three render modes: `server` (default, inline SVG), `client` (legacy, Mermaid.js ESM), `hybrid` (try server, fallback to client)
- **Render mode API** — `PyDiagramHtmlRendererOptions(render_mode="server"|"client"|"hybrid", mermaid_url=...)`
- **Customizable Mermaid themes** — Mermaid color schemes derived from code-highlighting (syntect) themes. `PyDiagramHtmlRendererOptions(theme="Dracula")` themes server-side SVG (via `mermaid-rs-renderer`) and client-side rendering (via `mermaid.initialize` + `themeVariables`). A single `theme=` kwarg on `markdown_to_html` themes both code and diagrams; native mermaid themes (`modern`/`dark`/`forest`/`neutral`) are also supported.

## What's New in 0.8.7

- **Chunker GFM + Diagram parity** — `MarkdownChunker` now uses the same parser extensions as `parse()` and `markdown_to_html()`: GFM tables (`TableAstTransformer`, `TableParagraphTransformer`) and Mermaid diagrams (`DiagramAstTransformer`) are correctly classified as `BlockType::Table` and `BlockType::Diagram` respectively
- **Chunker Diagram block type** — `BlockType::Diagram` added to the chunker's type system; mermaid code blocks yield as `"Diagram"` instead of `"CodeBlock"` or being silently dropped
- **Diagram source position fix** — `DiagramAstTransformer` now copies the original code block's `pos()` to the new `Diagram` node so the chunker can slice raw source correctly

## What's New in 0.8.6

- **Lint engine** — 25 lint rules (MD001, MD003, MD009, MD010, MD012, MD013, MD018–MD022, MD024, MD025, MD026, MD031, MD032, MD034, MD040, MD042, MD045–MD048, MD049, MD050) with diagnostics, fix engine, and configuration
- **Batch API** — `lint_many()` and `fix_many()` for parallel file processing via `rayon`, with GIL release for the entire batch
- **CLI** — `python -m mordant` with `--fix`, `--dry-run`, `--format` (human/json/github), `--config`, `--enable`, `--disable`, `--default-language`, glob/directory recursion
- **Phase 8 accuracy polish** — emoji text in heading comparison (MD024), frontmatter `title:` support (MD025), fragment anchor validation for links (MD042)
- **Document chunking** — `MarkdownChunker` lazy AST-based chunk iterator yielding **bare chunks** (no heading prefix), with `get_chunks()`, `get_all_chunks()`, `get_chunks_with_context()`, `get_bare_chunks()`, `ExtractedChunk` (with `block_type`/`start_offset`/`end_offset`), `get_delimiter()`, `compute_overlap_payloads()`
- **Inline suppression** — `<!-- markdownlint-disable MD001 -->` comments supported
- **VSCode JSON theme support** — Custom themes from `.json` files via `add_custom_theme()` and user directory `~/.mordant/themes/`
- **1198 tests** passing (up from 1161)

## Features

- **Blazing fast.** One of the fastest Markdown parsers for Python — up to 55x faster than python-markdown on large documents.
- **Full AST access.** Parse markdown to a `Document` with complete tree traversal — navigate parent, children, siblings, access all node kinds.
- **CommonMark + GFM.** Fully compliant with CommonMark 0.31.2 and GitHub Flavored Markdown (tables, task lists, strikethrough; autolink disabled by default, enable with `GfmOptions.all()`).
- **YAML frontmatter.** Extract metadata from YAML frontmatter with full type preservation (null, bool, int, float, str, list, dict).
- **Multi-threaded.** Parse and render release the GIL — scale ~4.0x linearly with thread count.
- **Emoji support.** :joy: `:heart:` `:smile:` — shortcode-style emoji rendering with blacklist and custom templates.
- **Math support.** LaTeX math via KaTeX — fenced ```math/```latex blocks, inline `$...$`/`$$...$$` math, standalone `render_math()` function.
- **Mermaid diagrams.** `graph LR`, `sequenceDiagram` — render Mermaid diagrams from code blocks, server-side as inline SVG by default. Customizable color schemes derived from code-highlighting themes (a single `theme=` kwarg themes both code and diagrams).
- **Footnotes.** PHP Markdown Extra style footnotes (`[^1]`, `[^hello]`) with `<sup>` references, `<div class="footnotes">` endnotes, and backlinks.
- **Document chunking.** `MarkdownChunker` — lazy, low-copy AST-based chunk iterator with heading-context propagation, `from_file()` and `from_file_mmap()` constructors.
- **Extensible.** Custom node types, parsers, transformers, and renderers via Rust extensions.

## Install

```bash
pip install mordant
```

Or from source:

```bash
cd mordant-py
cargo build --release
pip install -e .
```

## Quick Start

```python
import mordant

# Parse + render in one call
html = mordant.markdown_to_html("# Hello\n\n**World**")
# '<h1>Hello</h1>\n<p><strong>World</strong></p>\n'

# GFM support (tables, strikethrough, task lists enabled by default)
html = mordant.markdown_to_html("~~deleted~~")
# '<p><del>deleted</del></p>\n'

# Autolink (disabled by default; enable with GfmOptions.all())
html = mordant.markdown_to_html(
    "https://example.com",
    gfm_opts=mordant.GfmOptions.all()
)
# '<p><a href="https://example.com">https://example.com</a></p>\n'

# Full AST access
doc = mordant.parse("# Hello\n\n**World**")
print(doc.kind)        # "Document"
print(doc.children)    # [Heading, Paragraph]
print(doc.text)        # "HelloWorld"

# Emoji support
html = mordant.markdown_to_html("I'm :joy: and :heart:")
# '<p>I'm 😀 and ❤️</p>\n'

# Emoji blacklist
opts = mordant.PyEmojiParserOptions(blacklist="joy")
html = mordant.markdown_to_html(":joy: :heart:", emoji_parse_opts=opts)
# ':joy:' passes through; :heart: renders as ❤️

# Math support
html = mordant.markdown_to_html("""```math
\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}
```""")
# '<span class="katex katex-display">...</span>'

# Standalone math rendering
result = mordant.render_math(r"\alpha + \beta", display=True, output="both")

# Footnotes (always enabled)
html = mordant.markdown_to_html("Text[^1]\n\n[^1]: The footnote.")
# '<p>Text<sup id="fnref:1"><a href="#fn:1" class="footnote-ref">1</a></sup></p>\n<div class="footnotes" role="doc-endnotes">\n<hr>\n<ol><li id="fn:1">The footnote.&#160;<a href="#fnref:1" class="footnote-backref" role="doc-backlink">&#x21a9;&#xfe0e;</a></li></ol></div>'

# Custom footnote options
opts = mordant.PyFootnoteHtmlRendererOptions(
    link_class="my-ref",
    backlink_class="my-back",
    backlink_html="↑ back",
)
html = mordant.markdown_to_html("Text[^1]", footnote_render_opts=opts)

# Mermaid diagrams
html = mordant.markdown_to_html("""```mermaid
graph LR
    A --- B
```""")
# '<pre class="mermaid">\ngraph LR\n    A --- B\n</pre>\n<script type="module">...'

# Mermaid with custom URL
opts = mordant.PyDiagramHtmlRendererOptions(mermaid_url="https://cdn.example.com/mermaid.mjs")
html = mordant.markdown_to_html("""```mermaid
graph TD
    A --> B
```""", diagram_render_opts=opts)

# Themed Mermaid diagram — color scheme derived from a code-highlighting theme
opts = mordant.PyDiagramHtmlRendererOptions(render_mode="server", theme="Dracula")
html = mordant.markdown_to_html("""```mermaid
graph TD
    A --> B
```""", diagram_render_opts=opts)
# Server-rendered SVG uses Dracula's palette (background #282a36, pink edges, ...)

# Single `theme=` kwarg themes BOTH code blocks and Mermaid diagrams
html = mordant.markdown_to_html(
    "# Title\n```mermaid\ngraph LR\n A---B\n```\n```python\nx=1\n```",
    theme="Dracula",
)
# Code block and diagram share Dracula's colors

# YAML frontmatter
md = """---
title: My Doc
author: Jane
tags: [rust, markdown]
---

Body
"""
doc = mordant.parse(md)
print(doc.metadata)
# {'title': 'My Doc', 'author': 'Jane', 'tags': ['rust', 'markdown']}

# Document chunking
chunker = mordant.MarkdownChunker("# Section\n\nPara one\n\n## Sub\n\nPara two")
for chunk in chunker:
    print(chunk)
# Para one
# Para two
# (bare chunks — no heading prefix)

# get_chunks() returns ExtractedChunk with metadata
for chunk in chunker.get_chunks():
    print(chunk.block_type, chunk.text, chunk.start_offset, chunk.end_offset)
# Paragraph Para one 9 17
# Paragraph Para two 27 35

# get_all_chunks() includes headings
for chunk in chunker.get_all_chunks():
    print(chunk.block_type, chunk.text)
# Heading # Section
# Paragraph Para one
# Heading ## Sub
# Paragraph Para two

# get_chunks_with_context() adds heading prefix
for chunk in chunker.get_chunks_with_context():
    print(chunk.text)
# # Section\n\nPara one
# ## Sub\n\nPara two

# compute_overlap_payloads() for embedding
payloads = chunker.compute_overlap_payloads(2)
# [{"chunk:0": "Para one"}, {"chunk:1": "one\n\nPara two"}]
```

## Document Chunking

Split a document into **bare chunks** — each chunk is the raw block content with no heading prefix. OKF injects heading context at embed time for better embeddings. Headings update a `current_header` context; thematic breaks and other non-body nodes are skipped without resetting context.

```python
import mordant

# Basic chunking — bare chunks (no heading prefix)
chunker = mordant.MarkdownChunker("# Section\n\nPara one\n\n## Sub\n\nPara two")
chunks = list(chunker)
assert len(chunks) == 2
assert chunks[0] == "Para one"          # bare, no heading prefix
assert chunks[1] == "Para two"          # bare, no heading prefix

# current_header still tracks the last heading seen
assert chunker.current_header == "## Sub"

# get_chunks() returns ExtractedChunk with metadata
for chunk in chunker.get_chunks():
    print(chunk.block_type, chunk.text, chunk.start_offset, chunk.end_offset)
# Paragraph Para one 9 17
# Paragraph Para two 27 35

# get_all_chunks() includes headings as separate chunks
for chunk in chunker.get_all_chunks():
    print(chunk.block_type, chunk.text)
# Heading # Section
# Paragraph Para one
# Heading ## Sub
# Paragraph Para two

# get_chunks_with_context() adds heading prefix for display
for chunk in chunker.get_chunks_with_context():
    print(chunk.text)
# # Section\n\nPara one
# ## Sub\n\nPara two

# get_delimiter() for document reconstruction
mordant.MarkdownChunker.get_delimiter("List", "List")           # "\n"
mordant.MarkdownChunker.get_delimiter("Blockquote", "Blockquote")  # "\n> "
mordant.MarkdownChunker.get_delimiter("Paragraph", "CodeBlock")  # "\n\n"

# compute_overlap_payloads() for embedding context continuity
chunker = mordant.MarkdownChunker("# Title\n\nFirst para second para third para.\n\n## Sub\n\nMore text here.")
payloads = chunker.compute_overlap_payloads(2)
# [{"chunk:0": "First para second para third para."},
#  {"chunk:1": "third  para.\n\nMore text here."}]

# from_file reads from disk
chunker = mordant.MarkdownChunker.from_file("/path/to/doc.md")
for chunk in chunker:
    print(chunk)  # bare chunks, no heading prefix

# from_file_mmap for zero-copy large files
chunker = mordant.MarkdownChunker.from_file_mmap("/path/to/large.md")

# Nested headings inside blockquotes never leak as context
chunker = mordant.MarkdownChunker("# Outer\n\n> # Nested\n\n> Quote text.")
chunks = list(chunker)
# current_header is "# Outer" (not "# Nested" which is nested)
assert chunker.current_header == "# Outer"
```

See [QUICKREF.md](QUICKREF.md#markdownchunker) for full API reference.

## AST Traversal

```python
doc = mordant.parse("# Title\n\n**Bold** and *italic*")

# Navigate tree
heading = doc.children[0]
print(heading.level)       # 1
print(heading.text)        # "Title"

# Walk all nodes
for node in doc.walk("depth"):
    print(f"{node.kind}: {node.text}")

# Find by kind
links = [n for n in doc.walk("depth") if n.kind == "Link"]
```

## Options

```python
# Parse options
parse_opts = mordant.ParseOptions(
    attributes=False,
    auto_heading_ids=False,
    escaped_space=False,
    meta_table=False,
)

# Render options
render_opts = mordant.RenderOptions(
    hard_wraps=False,
    xhtml=False,
    allows_unsafe=False,
    escaped_space=False,
)

# GFM options (default: tables + strikethrough + task lists; linkify disabled)
import mordant

gfm_opts = mordant.GfmOptions()
# Enable all features including linkify
gfm_opts = mordant.GfmOptions.all()
# Granular feature selection
gfm_opts = mordant.GfmOptions(features=[
    mordant.GfmFeature.Table,
    mordant.GfmFeature.Strikethrough,
])

html = mordant.markdown_to_html(
    "Hello\nWorld",
    gfm_opts=gfm_opts,
    parse_opts=parse_opts,
    render_opts=render_opts,
)
```

## Multi-threaded Usage

```python
from concurrent.futures import ThreadPoolExecutor
import mordant

# GIL is released during parse + render — safe for concurrent use
with ThreadPoolExecutor(max_workers=4) as pool:
    results = list(pool.map(mordant.markdown_to_html, markdown_docs))
# ~4.0x linear scaling vs single-threaded
```

## Performance

### Single-threaded (50 iterations)

| Fixture | mordant | mistune | markdown-it-py | python-markdown |
|---------|---------|---------|----------------|-----------------|
| Small (400B) | **0.039ms** | 0.430ms | 0.475ms | 2.301ms |
| Medium (5.4KB) | **0.155ms** | 2.448ms | 3.940ms | 6.455ms |
| Large (26.7KB) | **0.410ms** | 8.611ms | 16.743ms | 31.304ms |
| Data (202KB) | **2.763ms** | 38.152ms | 65.736ms | 621.295ms |

### Multi-threaded (4 threads, medium fixture)

| Library | 1-thread | 4-threads | Scaling |
|---------|----------|-----------|---------|  
| **mordant** | ~1,000 docs/s | ~4,000 docs/s | **4.0x** |
| python-markdown | ~59 docs/s | ~257 docs/s | 4.35x |
| mistune | ~133 docs/s | ~542 docs/s | 4.07x |
| markdown-it-py | ~83 docs/s | ~337 docs/s | 4.06x |

## Node Kind Reference

| Kind | Type | Example |
|------|------|---------|
| Document | block | Root node |
| Paragraph | block | `Hello world` |
| Heading | block | `# Title` |
| ThematicBreak | block | `---` |
| CodeBlock | block | ` ```python ... ``` ` |
| Blockquote | block | `> quoted` |
| List | block | `- item` |
| ListItem | block | `- [x] done` |
| HtmlBlock | block | `<div>...</div>` |
| Text | inline | Plain text |
| CodeSpan | inline | `` `code` `` |
| Emphasis | inline | `*italic*` |
| Strong | inline | `**bold**` |
| Link | inline | `[text](url)` |
| Image | inline | `![alt](url)` |
| RawHtml | inline | `<span>` |
| LinkReferenceDefinition | block | `[ref]: url` |
| Table | block | `| A | B |` |
| TableHeader | block | Header row |
| TableBody | block | Body rows |
| TableRow | block | `<tr>` |
| TableCell | block | `<td>` |
| Strikethrough | inline | `~~text~~` |
| Diagram | block | ` ```mermaid ... ``` ` |
| FootnoteReference | inline | `[^1]`, `[^hello]` |
| FootnoteDefinition | block | `[^1]:`, `[^hello]:` |
| Extension | any | Custom nodes |

## Thematic Break vs Frontmatter

The meta parser uses lookahead to distinguish `---` (thematic break) from frontmatter:

```python
# Thematic break
mordant.parse("---").metadata == {}

# Frontmatter
mordant.parse("---\ntitle: Test\n---").metadata["title"] == "Test"

# Five dashes is thematic break
mordant.parse("-----").metadata == {}
```

## Error Handling

```python
import mordant

try:
    doc = mordant.parse("---\ninvalid: yaml: [broken")
    doc.metadata  # Raises ValueError on access
except ValueError as e:
    print(e)  # YAML parsing error message
```

## Architecture

Mordant wraps the [rushdown](https://github.com/yuin/rushdown) Rust library (CommonMark 0.31.2 + GFM) via PyO3 bindings:

- **Rust core:** rushdown v0.18.0 — arena-allocated AST, priority-based parser dispatch, HTML renderer
- **Python bindings:** PyO3 0.29 — `Document`, `Node`, `Walker` classes with shared `Rc<RefCell<Arena>>` and `Rc<str>` source memory model (refcount bump on node creation instead of deep source copy)
- **GIL release:** Parse and render release the GIL via `Python::detach()` for multi-threaded parallelism
- **Frontmatter:** YAML parsing via `yaml-peg` with thematic break conflict resolution

### rushdown-meta

YAML frontmatter support is provided by [rushdown-meta](https://crates.io/crates/rushdown-meta), which has been directly incorporated into mordant. The original rushdown-meta crate is available in `extensions/rushdown-meta-main/`.

Key features of the integrated meta parser:

- **Thematic break conflict resolution:** `---` alone is a thematic break; `---\n` + YAML-like content is frontmatter
- **Full YAML subset:** null, bool, int, float, str, list, dict (via `yaml-peg`)
- **AST table rendering:** Optional `meta_table` option renders metadata as an HTML table in the AST
- **Error handling:** YAML parse errors are inserted as HTML comments in the AST; Python raises `ValueError` on `doc.metadata` access

See [ARCHITECTURE.md §6](ARCHITECTURE.md#6-yaml-frontmatter-meta-rs) for full details.

### rushdown-emoji

Emoji shortcode support (`:joy:`, `:heart:`, `:smile:`, etc.) is provided by [rushdown-emoji](https://crates.io/crates/rushdown-emoji), which has been directly incorporated into mordant. The original rushdown-emoji crate is available in `extensions/rushdown-emoji-main/`.

Key features of the integrated emoji extension:

- **Shortcode parsing:** `:joy:` → 😀, `:heart:` → ❤️, 1,500+ emojis from the `emojis` crate (v0.8.0)
- **Blacklist support:** `PyEmojiParserOptions(blacklist="joy,heart")` — blacklisted shortcodes pass through as literal text
- **Custom HTML templates:** `PyEmojiHtmlRendererOptions(template='<img src="{shortcode}.png" />')` — render emojis as `<img>` tags or any custom format
- **Template placeholders:** `{emoji}` (Unicode char), `{shortcode}` (e.g. `"joy"`), `{name}` (e.g. `"grinning face with smiling eyes"`)
- **Code span protection:** Emojis inside `` `code` `` are not parsed — `:joy:` stays literal in code spans
- **AST node access:** Emoji nodes expose `emoji`, `shortcode`, and `name` properties via the `Extension` node kind
- **Error handling:** Unknown shortcodes pass through as-is (`:invalid:` → `:invalid:`)

See [ARCHITECTURE.md §7.10](ARCHITECTURE.md#710-emoji-extension-rushdown-emoji) for full details.

### rushdown-diagram

Diagram support is provided by [rushdown-diagram](https://crates.io/crates/rushdown-diagram), which has been directly incorporated into mordant. The original rushdown-diagram crate is available in `extensions/rushdown-diagram-main/`.

rushdown-diagram supports two diagram formats:

- **MermaidJS** — client-side rendering via the Mermaid.js ESM module
- **PlantUML** — server-side rendering (requires a `plantuml` command)

Mordant currently implements Mermaid support only. Key features:

- **Code block detection:** ```` ```mermaid ```` code blocks are automatically detected and converted to diagram nodes via an AST transformer
- **Client-side rendering:** Diagrams render as `<pre class="mermaid">` with automatic Mermaid.js ESM script injection (single script tag for all diagrams)
- **Custom Mermaid URL:** `PyDiagramHtmlRendererOptions(mermaid_url="https://cdn.example.com/mermaid.mjs")` — use a custom Mermaid.js CDN or local file
- **Customizable themes:** `PyDiagramHtmlRendererOptions(theme="<name>")` derives Mermaid colors from a code-highlighting (syntect) theme — server-side SVG via `render_with_options`, client-side via `mermaid.initialize` + `themeVariables`. Built-in mermaid themes (`modern`/`dark`/`forest`/`neutral`) are used natively. A single `theme=` kwarg on `markdown_to_html` themes both code and diagrams; explicit per-param args override it.
- **Parser options:** `PyDiagramParserOptions(mermaid_enabled=False)` — disable diagram transformation to keep code blocks as regular fenced code blocks
- **AST node access:** Diagram nodes expose `diagram_type` ("mermaid") and `diagram_value` (source content) properties via the `Diagram` node kind
- **Multiple diagrams:** Multiple Mermaid blocks in one document all render correctly with a single script tag
- **GFM compatible:** Works alongside other GFM features (tables, task lists, strikethrough; autolink disabled by default, enable with `GfmOptions.all()`)
- **Frontmatter compatible:** Works alongside YAML frontmatter

See [ARCHITECTURE.md](ARCHITECTURE.md) for full details.

### rushdown-math

Math support is provided by the pure-Rust `katex-rs` crate, incorporated directly into mordant.

Key features:

- **Fenced math blocks:** ```` ```math ```` and ```` ```latex ```` code blocks render to KaTeX markup
- **Inline math:** `$...$` for inline, `$$...$$` for display mode
- **Standalone `render_math()`:** `mordant.render_math(r"\alpha + \beta", display=True, output="both")` — renders LaTeX independently of the Markdown AST
- **Output formats:** `"both"` (HTML+MathML, default), `"html"`, or `"mathml"`
- **Error handling:** Invalid LaTeX produces an error span (`<span class="katex-error">...`) instead of crashing
- **Caching:** Rendered markup is memoized on `(display, output, latex)` for repeated formulas
- **GIL released:** Math rendering runs with the GIL released for multi-threaded parallelism

See [ARCHITECTURE.md §7.12](ARCHITECTURE.md#712-math-extension-katex) for full details.

### rushdown-footnote

Footnote support is provided by [rushdown-footnote](https://github.com/yuin/rushdown-footnote), which has been directly incorporated into mordant. Footnotes are **always enabled** — no parser options to disable them.

**Syntax (PHP Markdown Extra):**

```markdown
Text with a footnote.[^1]
Text with a named footnote.[^hello]

[^1]: The footnote.

[^hello]: The named footnote.
```

**Output:**

```html
<p>Text with a footnote.<sup id="fnref:1"><a href="#fn:1" class="footnote-ref">1</a></sup></p>
<div class="footnotes" role="doc-endnotes">
<hr>
<ol>
<li id="fn:1">The footnote.&#160;<a href="#fnref:1" class="footnote-backref" role="doc-backlink">&#x21a9;&#xfe0e;</a></li>
</ol>
</div>
```

Key features:

- **Inline references:** `[^1]`, `[^hello]` — rendered as `<sup><a href="#fn:N">N</a></sup>`
- **Block definitions:** `[^1]:` followed by content — rendered in `<div class="footnotes">` at end of document
- **Named footnotes:** `[^hello]` — label preserved in ID
- **Multiple refs:** Multiple `[^1]` to same `[^1]:` — each gets a superscript ref, definition rendered once
- **Backlinks:** Each definition has a backlink anchor (`&#x21a9;&#xfe0e;`) to return to the reference
- **Accessibility:** `role="doc-endnotes"`, `role="doc-noteref"`, `role="doc-backlink"` ARIA attributes
- **Custom options:** `PyFootnoteHtmlRendererOptions` for custom CSS classes, backlink HTML, and ID prefixes
- **AST node access:** `node.footnote_label`, `node.footnote_index`, `node.footnote_references` properties
- **No parser options:** Footnotes are always enabled (matches math extension pattern)

See [ARCHITECTURE.md §7.14](ARCHITECTURE.md#714-footnote-extension-rushdown-footnote) for full details.

## Benchmarks

Run benchmarks:

```bash
cd mordant-py
python benchmarks.py              # All fixtures, 50 iterations
python benchmarks.py -f medium -n 100  # Specific fixture, custom count
python benchmarks.py -o results.json  # Save JSON
```

## Tests

```bash
cd mordant-py
python -m pytest tests/ -v
```

1198 Python tests passing (Core, AST, GFM, Options, YAML Frontmatter, Emoji, Mermaid Diagrams, Math, Lint engine, CLI, batch API, Phase 8 accuracy, VSCode theme, Chunker, OKF chunker methods, Mixed Features) + 51 Rust tests (Unit tests, AST, CommonMark spec, Extensions, GFM, Options, Doc-tests).

## Theme Loading

Themes are loaded from multiple sources:

- **Embedded themes** — Bundled in `mordant/themes/`, loaded at import time
- **User themes** — Place `.json` or `.tmTheme` files in `~/.mordant/themes/` (or `%APPDATA%/mordant/themes/` on Windows) for auto-loading
- **Built-in themes** — Loaded from `syntect-assets` (bat's updated themes)
- **Custom themes** — Use `add_custom_theme(name, content)` to register themes from JSON or XML content

Both VSCode JSON and Sublime `.tmTheme` formats are supported. VSCode JSON themes are automatically converted to the syntect format via the `parse_vscode_theme_jsonc` → `vscode_theme_to_syntect` pipeline, allowing you to use any VSCode theme file directly.

See [QUICKREF.md](QUICKREF.md#theme-loading) for details.

## License

MIT

## Author

Mordant: Python bindings by [opticsWolf](https://github.com/opticsWolf)
Rushdown: Rust core by [Yusuke Inuzuka](https://github.com/yuin)
