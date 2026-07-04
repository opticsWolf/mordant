# Mordant

> **Version:** 0.7.0  
> **Rust:** rushdown v0.18.0 (CommonMark 0.31.2 + GFM)  
> **Python:** 3.9+  
> **Bindings:** PyO3 0.29

A fast CommonMark + GFM Markdown parser and renderer for Python, powered by the [rushdown](https://github.com/yuin/rushdown) Rust library.

- [Architecture](ARCHITECTURE.md) — Full architecture documentation
- [Quick Reference](QUICKREF.md) — Python bindings quick reference

## What's New in 0.7.0

- **Lint engine** — 25 lint rules (MD001, MD003, MD009, MD010, MD012, MD013, MD018–MD022, MD024, MD025, MD026, MD031, MD032, MD034, MD040, MD042, MD045–MD048, MD049, MD050) with diagnostics, fix engine, and configuration
- **Batch API** — `lint_many()` and `fix_many()` for parallel file processing via `rayon`, with GIL release for the entire batch
- **CLI** — `python -m mordant` with `--fix`, `--dry-run`, `--format` (human/json/github), `--config`, `--enable`, `--disable`, `--default-language`, glob/directory recursion
- **Phase 8 accuracy polish** — emoji text in heading comparison (MD024), frontmatter `title:` support (MD025), fragment anchor validation for links (MD042)
- **Document chunking** — `MarkdownChunker` lazy AST-based chunk iterator with heading-context propagation, `from_file()` and `from_file_mmap()` constructors
- **Inline suppression** — `<!-- markdownlint-disable MD001 -->` comments supported
- **VSCode JSON theme support** — Custom themes from `.json` files via `add_custom_theme()` and user directory `~/.mordant/themes/`
- **1040 tests** passing (up from 1003)

## Features

- **Blazing fast.** One of the fastest Markdown parsers for Python — up to 55x faster than python-markdown on large documents.
- **Full AST access.** Parse markdown to a `Document` with complete tree traversal — navigate parent, children, siblings, access all node kinds.
- **CommonMark + GFM.** Fully compliant with CommonMark 0.31.2 and GitHub Flavored Markdown (tables, task lists, strikethrough, autolink).
- **YAML frontmatter.** Extract metadata from YAML frontmatter with full type preservation (null, bool, int, float, str, list, dict).
- **Multi-threaded.** Parse and render release the GIL — scale ~4.0x linearly with thread count.
- **Emoji support.** :joy: `:heart:` `:smile:` — shortcode-style emoji rendering with blacklist and custom templates.
- **Mermaid diagrams.** `graph LR`, `sequenceDiagram` — render Mermaid diagrams from code blocks with client-side JS loading.
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

# GFM support
html = mordant.markdown_to_html("~~deleted~~", gfm=True)
# '<p><del>deleted</del></p>\n'

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
# # Section
#
# Para one
# ## Sub
#
# Para two
```

## Document Chunking

Split a document into heading-scoped chunks — each chunk carries the most recent heading as a prefix. Headings themselves are not yielded; thematic breaks and other non-body nodes are skipped without resetting context.

```python
import mordant

# Basic chunking with heading context
chunker = mordant.MarkdownChunker("# Section\n\nPara one\n\n## Sub\n\nPara two")
chunks = list(chunker)
assert len(chunks) == 2
assert chunks[0] == "# Section\n\nPara one"
assert chunks[1] == "## Sub\n\nPara two"

# current_header tracks the last heading seen
assert chunker.current_header == "## Sub"

# from_file reads from disk
chunker = mordant.MarkdownChunker.from_file("/path/to/doc.md")
for chunk in chunker:
    print(chunk)

# from_file_mmap for zero-copy large files
chunker = mordant.MarkdownChunker.from_file_mmap("/path/to/large.md")

# Nested headings inside blockquotes never leak as context
chunker = mordant.MarkdownChunker("# Outer\n\n> # Nested\n\n> Quote text.")
chunks = list(chunker)
assert all(not c.startswith("# Nested") for c in chunks)
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

# GFM options
gfm_opts = mordant.GfmOptions(
    tables=True,
    strikethrough=True,
    task_lists=True,
    linkify=True,
)

html = mordant.markdown_to_html(
    "Hello\nWorld",
    gfm=True,
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
- **Python bindings:** PyO3 0.29 — `Document`, `Node`, `Walker` classes with shared `Rc<RefCell<Arena>>` memory model
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
- **Parser options:** `PyDiagramParserOptions(mermaid_enabled=False)` — disable diagram transformation to keep code blocks as regular fenced code blocks
- **AST node access:** Diagram nodes expose `diagram_type` ("mermaid") and `diagram_value` (source content) properties via the `Diagram` node kind
- **Multiple diagrams:** Multiple Mermaid blocks in one document all render correctly with a single script tag
- **GFM compatible:** Works alongside other GFM features (tables, task lists, strikethrough, autolink)
- **Frontmatter compatible:** Works alongside YAML frontmatter

See [ARCHITECTURE.md](ARCHITECTURE.md) for full details.

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

1040 Python tests passing (Core, AST, GFM, Options, YAML Frontmatter, Emoji, Mermaid Diagrams, Lint engine, CLI, batch API, Phase 8 accuracy, VSCode theme, Chunker) + 54 Rust tests (Unit tests, AST, CommonMark spec, Extensions, GFM, Options, Doc-tests).

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

Mordant: Python bindings by [your name]  
Rushdown: Rust core by [Yusuke Inuzuka](https://github.com/yuin)
