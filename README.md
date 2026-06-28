# Mordant

> **Version:** 0.4.0  
> **Rust:** rushdown v0.18.0 (CommonMark 0.31.2 + GFM)  
> **Python:** 3.9+  
> **Bindings:** PyO3 0.29

A fast CommonMark + GFM Markdown parser and renderer for Python, powered by the [rushdown](https://github.com/yuin/rushdown) Rust library.

- [Architecture](ARCHITECTURE.md) — Full architecture documentation
- [Quick Reference](QUICKREF.md) — Python bindings quick reference

## Features

- **Blazing fast.** One of the fastest Markdown parsers for Python — up to 30x faster than python-markdown on large documents.
- **Full AST access.** Parse markdown to a `Document` with complete tree traversal — navigate parent, children, siblings, access all node kinds.
- **CommonMark + GFM.** Fully compliant with CommonMark 0.31.2 and GitHub Flavored Markdown (tables, task lists, strikethrough, autolink).
- **YAML frontmatter.** Extract metadata from YAML frontmatter with full type preservation (null, bool, int, float, str, list, dict).
- **Multi-threaded.** Parse and render release the GIL — scale ~3.7x linearly with thread count.
- **Emoji support.** :joy: `:heart:` `:smile:` — shortcode-style emoji rendering with blacklist and custom templates.
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
```

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
# ~3.7x linear scaling vs single-threaded
```

## Performance

### Single-threaded (50 iterations)

| Fixture | mordant | mistune | markdown-it-py | python-markdown |
|---------|---------|---------|----------------|-----------------|
| Small (400B) | **0.240ms** | 0.432ms | 0.477ms | 2.221ms |
| Medium (5.4KB) | **1.044ms** | 2.476ms | 3.963ms | 6.431ms |
| Large (26.7KB) | **3.692ms** | 8.566ms | 16.676ms | 30.917ms |
| Data (202KB) | **22.294ms** | 38.056ms | 66.848ms | 617.221ms |

### Multi-threaded (4 threads, medium fixture)

| Library | 1-thread | 4-threads | Scaling |
|---------|----------|-----------|---------|  
| **mordant** | 958 docs/s | **3,584 docs/s** | **3.74x** |
| python-markdown | 155 docs/s | 228 docs/s | 1.47x |
| mistune | 404 docs/s | 550 docs/s | 1.36x |
| markdown-it-py | 252 docs/s | 290 docs/s | 1.15x |

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

823 tests passing across Core, AST, GFM, Options, YAML Frontmatter, and Emoji.

## License

MIT

## Author

Mordant: Python bindings by [your name]  
Rushdown: Rust core by [Yusuke Inuzuka](https://github.com/yuin)
