# Mordant

[![CI](https://github.com/opticsWolf/mordant/actions/workflows/test.yml/badge.svg)](https://github.com/opticsWolf/mordant/actions/workflows/test.yml)
[![License](https://img.shields.io/github/license/opticsWolf/mordant)](https://github.com/opticsWolf/mordant/blob/main/LICENSE)
[![PyPI - Version](https://img.shields.io/pypi/v/mordant)](https://pypi.org/project/mordant/)
[![PyPI - Python Version](https://img.shields.io/pypi/pyversions/mordant)](https://pypi.org/project/mordant/)
[![crates.io](https://img.shields.io/crates/v/mordant)](https://crates.io/crates/mordant)
[![Rust](https://img.shields.io/badge/Rust-1.87+-orange)](https://www.rust-lang.org)

> **Version:** 0.9.0 (Python and Rust crates in lockstep)
> **Python:** 3.9+ · **Bindings:** PyO3 0.29

A fast CommonMark + GFM Markdown parser and renderer for Python, powered by the [`mordant`](https://crates.io/crates/mordant) Rust library — itself built on the [rushdown](https://github.com/yuin/rushdown) engine by Yusuke Inuzuka.

## Features

- **Blazing fast** — one of the fastest Markdown parsers for Python; up to 55x faster than python-markdown on large documents
- **CommonMark 0.31.2 + GFM** — tables, task lists, strikethrough; autolink available via `GfmOptions.all()`
- **Full AST access** — `parse()` returns a `Document`; traverse parents, children and siblings; kind-specific properties on every node
- **YAML frontmatter** — typed metadata extraction (null, bool, int, float, str, list, dict)
- **Emoji shortcodes** — `:joy:` → 😂 with blacklists and custom templates
- **LaTeX math** — fenced ```` ```math ````/```` ```latex ```` blocks, inline `$…$` and `$$…$$`, standalone `render_math()`
- **Mermaid diagrams** — server-side inline SVG by default (~3ms), legacy client-side Mermaid.js mode, hybrid fallback; themeable from code-highlighting themes
- **Footnotes** — PHP Markdown Extra style `[^1]` with backlinks
- **Lint engine** — 25 markdownlint-style rules, auto-fix engine, inline suppressions, `.markdownlint.json` config support
- **Batch linting** — `lint_many()` / `fix_many()` process many files in parallel via rayon
- **CLI** — `python -m mordant` with `--fix`, `--dry-run`, `--format human|json|github`, glob/directory recursion
- **Document chunking** — `MarkdownChunker`: lazy, low-copy chunk iterator with heading context, built for RAG/embedding pipelines
- **Syntax highlighting** — 190+ languages via syntect-assets (bat's syntaxes), VSCode JSON and Sublime `.tmTheme` themes
- **Multi-threaded** — parse, render, lint and fix release the GIL; scales ~4x linearly with thread count

## Install

```bash
pip install mordant
```

Or build from source (requires a Rust toolchain):

```bash
git clone https://github.com/opticsWolf/mordant
cd mordant/mordant-py
pip install -e .
```

Rust users can use the same engine directly: [`cargo add mordant`](https://crates.io/crates/mordant).

## Quick Start

```python
import mordant

html = mordant.markdown_to_html("# Hello\n\n**World**")
# '<h1>Hello</h1>\n<p><strong>World</strong></p>\n'
```

### GFM options

```python
opts = mordant.GfmOptions(features=[
    mordant.GfmFeature.Table,
    mordant.GfmFeature.Strikethrough,
    mordant.GfmFeature.TaskList,
])
html = mordant.markdown_to_html("~~strike~~", gfm_opts=opts)

opts = mordant.GfmOptions.all()   # everything incl. the linkify extension
opts = mordant.GfmOptions.none()
```

## Parse to an AST

```python
doc = mordant.parse("# Hello\n\nSome **bold** text.")

doc.kind        # "Document"
doc.children    # [Heading, Paragraph]

for node in doc.walk("depth"):
    print(node.kind, node.text)

heading = doc.children[0]
heading.content          # rendered inner HTML
heading.level            # kind-specific property (1)

# YAML frontmatter (if present at the top of the document)
print(doc.metadata)      # {'title': 'My Document', 'tags': ['a', 'b']}
```

Frontmatter is controlled through `ParseOptions`, e.g.
`ParseOptions(meta_table=True)` also injects it as a table.

## Emoji

```python
html = mordant.markdown_to_html("I love :heart: and :joy:")
# 'I love ❤️ and 😂'

# Ignore specific shortcodes
html = mordant.markdown_to_html(
    ":joy: stays literal",
    emoji_parse_opts=mordant.EmojiParserOptions(blacklist="joy"),
)

# Custom render template ({emoji}, {shortcode}, {name})
html = mordant.markdown_to_html(
    ":joy:",
    emoji_render_opts=mordant.EmojiHtmlRendererOptions(
        template='<img src="https://cdn.example.com/{shortcode}.png" />'
    ),
)
```

## Math (KaTeX)

```` ```math ```` / ```` ```latex ```` fences and inline `$…$` / `$$…$$` are
rendered automatically. Include `mordant.KATEX_CSS` in your page for styling.

```python
html = mordant.markdown_to_html("Euler: $e^{i\\pi} + 1 = 0$")

# Standalone rendering, independent of any document
markup = mordant.render_math("E = mc^2", display=True, output="both")
```

## Mermaid diagrams

```` ```mermaid ```` blocks render as **inline SVG server-side** by default —
no CDN or JavaScript needed:

```python
html = mordant.markdown_to_html("""```mermaid
graph LR
    A --> B --> C
```""")
# '<div class="mermaid"><svg>...</svg></div>'

# Legacy client-side rendering, or server→client fallback
opts = mordant.DiagramHtmlRendererOptions(render_mode="client")
opts = mordant.DiagramHtmlRendererOptions(render_mode="hybrid")

# One theme for BOTH code highlighting and diagrams
html = mordant.markdown_to_html(src, theme="Dracula")
```

## Linting & fixing

```python
diagnostics = mordant.lint("# Hello\n\n### Jump\n")
for d in diagnostics:
    print(f"{d.rule}:{d.line} {d.name}: {d.message}")
# MD001:1 heading-increment: Heading incremented by more than 1

result = mordant.fix("trailing   \n\n\ntext")
result.output      # 'trailing\n\ntext\n'
result.fixed       # what was auto-corrected
result.unfixable   # what needs manual attention

# Rule catalogue
for meta in mordant.lint_rules():
    print(meta.id, meta.name, meta.fixable)
```

Batch process a whole tree (parallel, GIL released):

```python
results = mordant.lint_many(["a.md", "b.md", "notes/*.md"])
```

Or from the command line:

```bash
python -m mordant check docs/*.md --format github
python -m mordant check docs/ --fix
```

Inline suppressions work too: `<!-- markdownlint-disable MD013 -->`.

## Document chunking

Built for RAG/embedding pipelines — a lazy, low-copy chunk iterator with
heading-context tracking:

```python
chunker = mordant.MarkdownChunker(text)          # or .from_file(path)
# or zero-copy: MarkdownChunker.from_file_mmap(path)

for chunk in chunker:                             # bare str chunks
    embed(chunk)

chunks = chunker.get_chunks_with_context()        # ExtractedChunk objects:
chunks[0].text                                    # "# Title\n\nbody…" (context-prefixed)
chunks[0].block_type                              # "Paragraph", "CodeBlock", …
chunks[0].start_offset, chunks[0].end_offset      # byte offsets in source

# Overlap payloads for embedding models
payloads = chunker.compute_overlap_payloads(overlap_words=50)
```

## Syntax highlighting

```python
hl = mordant.Highlighter(theme="Dracula", mode="Attribute")
html = hl.highlight("python", "def hello():\n    print('hi')")

# In documents
html = mordant.markdown_to_html(src,
    highlighting_theme="Dracula",
    highlighting_mode="Attribute")     # or "Class" for CSS classes

# Custom themes: VSCode JSON or Sublime .tmTheme
mordant.add_custom_theme("my-theme", open("theme.json").read())
mordant.list_themes()
mordant.list_syntaxes()                # ~190 languages
```

## Multi-threading

CPU-heavy work (parse, render, lint, fix, batch operations) runs without the
GIL, so plain `threading` scales across cores:

```python
from concurrent.futures import ThreadPoolExecutor
import mordant

with ThreadPoolExecutor(8) as pool:
    htmls = list(pool.map(mordant.markdown_to_html, documents))
```

## Documentation

- [Quick Reference](https://github.com/opticsWolf/mordant/blob/main/docs/QUICKREF.md) — complete API reference with every option
- [Architecture](https://github.com/opticsWolf/mordant/blob/main/docs/ARCHITECTURE.md) — internals, design decisions, module map
- [README](https://github.com/opticsWolf/mordant#readme) — project overview, Rust crate feature table

## License

MIT
