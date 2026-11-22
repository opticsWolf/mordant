# Mordant

[![CI](https://github.com/opticsWolf/mordant/actions/workflows/test.yml/badge.svg)](https://github.com/opticsWolf/mordant/actions/workflows/test.yml)
[![License](https://img.shields.io/github/license/opticsWolf/mordant)](https://github.com/opticsWolf/mordant/blob/main/LICENSE)
[![PyPI - Version](https://img.shields.io/pypi/v/mordant)](https://pypi.org/project/mordant/)
[![PyPI - Python Version](https://img.shields.io/pypi/pyversions/mordant)](https://pypi.org/project/mordant/)
[![crates.io](https://img.shields.io/crates/v/mordant)](https://crates.io/crates/mordant)
[![docs.rs](https://img.shields.io/docsrs/mordant)](https://docs.rs/mordant)
[![Rust](https://img.shields.io/badge/Rust-1.87+-orange)](https://www.rust-lang.org)
[![Website](https://img.shields.io/badge/website-mordant-blue)](https://opticswolf.github.io/mordant/)

> **Version:** 0.11.0 (Python and Rust crates in lockstep)  
> **Rust crate:** mordant v0.11.0 on [crates.io](https://crates.io/crates/mordant) ([docs.rs](https://docs.rs/mordant)) — CommonMark 0.31.2 + GFM  
> **Python:** 3.9+  
> **Bindings:** PyO3 0.29

A fast CommonMark + GFM Markdown parser and renderer — available as a native Rust library on [crates.io](https://crates.io/crates/mordant), powered by the [rushdown](https://github.com/yuin/rushdown) Rust library by Yusuke Inuzuka, and as Python bindings via `pip install mordant`.

- [Architecture](docs/ARCHITECTURE.md) — Full architecture documentation
- [Quick Reference](docs/QUICKREF.md) — Python bindings quick reference
- [Changelog](CHANGELOG.md) — Release history

## What's New in 0.11.0

- **Runtime engine switches.** Every engine ships in the wheel; all except syntax highlighting are on by default. Pass `engines=mordant.Engines.none()` / `.only([...])` / `.without([...])` (or set `engines=` on `ParseOptions` / `RenderOptions`) in Python, or use `mordant::Engines` (also available as `mordant::extensions::Engines`) in Rust. Syntax highlighting stays opt-in via `Engines.all()` to preserve 0.10 output.
- **Structured render options.** `RenderOptions` now accepts nested `emoji=`, `diagram=`, `footnote=`, `math=` and `highlighter=` settings; the old top-level kwargs still work.
- **New syntax extensions.** Definition lists, fenced divs, and link attributes (`{#id .class}`), each independently disableable.
- **KaTeX upgrade.** `katex-rs 0.3.0` (KaTeX 0.18.5); `mordant.KATEX_VERSION` reports the bundled CSS version. Dependency range is now `>=0.3.0, <0.4.0`.
- See [CHANGELOG.md](CHANGELOG.md) for the full history.

## Features

- **Blazing fast.** One of the fastest Markdown parsers for Python — up to 55x faster than python-markdown on large documents.
- **Full AST access.** Parse markdown to a `Document` with complete tree traversal — navigate parent, children, siblings, access all node kinds.
- **CommonMark + GFM.** Fully compliant with CommonMark 0.31.2 and GitHub Flavored Markdown (tables, task lists, strikethrough; autolink disabled by default, enable with `GfmOptions.all()`).
- **YAML frontmatter.** Extract metadata with full type preservation (null, bool, int, float, str, list, dict).
- **Multi-threaded.** Parse and render release the GIL — scale ~4.0x linearly with thread count. Batch `lint_many()` / `fix_many()` parallelize via rayon.
- **Emoji, math, diagrams, footnotes.** Shortcode emoji (`:joy:`) with blacklists and templates; LaTeX via KaTeX 0.18.5 (fenced blocks, inline math, `render_math()`); Mermaid diagrams as server-side SVG by default; PHP Markdown Extra footnotes.
- **Definition lists, fenced divs, link attributes.** `Term` + `: definition` lists, Pandoc-style `::: divs`, and `{#id .class}` attribute lists on links and images.
- **Safe extension attributes.** The new extensions drop duplicate `href`/`src`, and require `allows_unsafe=True` for `style`, `srcdoc`, or dangerous URL values; URL checks decode entities and ignore ASCII whitespace/control bytes before matching.
- **Lint engine.** 25 markdownlint-style rules with diagnostics, auto-fix engine, inline suppressions, `.markdownlint.json` config, and a CLI (`python -m mordant`).
- **Document chunking.** `MarkdownChunker` — lazy, low-copy AST chunk iterator with heading-context propagation, built for RAG/embedding pipelines.
- **Syntax highlighting.** 190+ languages via syntect, VSCode JSON and Sublime `.tmTheme` themes, custom syntax registration, and a standalone `Highlighter` API.
- **Extensible.** Custom node types, parsers, transformers, and renderers via Rust extensions.

## Install

**Python:**

```bash
pip install mordant
```

**Rust:**

```bash
cargo add mordant
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

# GFM (tables, strikethrough, task lists on by default; linkify opt-in)
html = mordant.markdown_to_html(
    "https://example.com",
    gfm_opts=mordant.GfmOptions.all()
)

# Full AST access
doc = mordant.parse("# Hello\n\n**World**")
print(doc.children)    # [Heading, Paragraph]
print(doc.text)        # "HelloWorld"
print(doc.metadata)    # {} (YAML frontmatter when present)

# Lint + auto-fix
result = mordant.fix("# Title   \n\n\nText\n")
print(result.output)   # "# Title\n\nText\n"

# Chunks for embeddings (bare blocks; context via get_chunks_with_context)
for chunk in mordant.MarkdownChunker("# S\n\nPara one"):
    print(chunk)       # "Para one"
```

See [QUICKREF.md](docs/QUICKREF.md) for the full API: options, emoji/diagram/math/footnote details, chunker methods, lint rules and CLI, themes, AST traversal, and benchmarks.

## Rust Crate

The same engine is available as a native Rust library with no Python dependency:

```toml
# Cargo.toml
dependencies = { mordant = "0.11" }
```

```rust
use mordant::markdown_to_html_string;

let mut html = String::new();
markdown_to_html_string(&mut html, "# Hello\n\n**World**").unwrap();
assert!(html.contains("<h1>Hello</h1>"));
```

The default feature set now includes every engine (with syntax highlighting off by runtime default). Use `default-features = false` with the individual features below for a minimal build, and `mordant::Engines` to switch engines off at runtime:

```rust
use mordant::{Engine, Engines};
use mordant::{markdown_to_html_string_with_options, parser, renderer::html};

let engines = Engines::default().without(&[Engine::Emoji]);
let (mut parser_opts, mut html_opts) = (parser::Options::default(), html::Options::default());
parser_opts.engines = engines;
html_opts.engines = engines;
let mut output = String::new();
markdown_to_html_string_with_options(&mut output, ":smile:", parser_opts, html_opts).unwrap();
```

| Feature | Enables | Extra dependencies |
|---------|---------|--------------------|
| *(default)* | `std`, `html-entities` **and all engines below**; syntax highlighting is off at runtime by default | — |
| `meta` | YAML frontmatter extraction (`document.metadata()`) | `yaml-peg` |
| `emoji` | `:shortcode:` emoji parsing/rendering | `emojis` (always on) |
| `footnotes` | PHP Markdown Extra footnotes | — |
| `linter` | 25 markdownlint-style rules, fix engine, suppressions | requires `emoji` |
| `diagram` | Mermaid diagrams: server SVG / client ESM / hybrid + theme derivation | `mermaid-rs-renderer`, `syntect`, `serde_json` |
| `chunker` | Lazy AST chunk iterator (`MarkdownChunker`), owned + mmap sources | requires `diagram`, adds `memmap2` |
| `math` | KaTeX math: fenced ```` ```math ```` / ```` ```latex ```` blocks, inline `$…$` / `$$…$$` | `katex-rs 0.3.0` (KaTeX 0.18.5) |
| `definition-list` | PHP Markdown Extra definition lists (`<dl>/<dt>/<dd>`) | — |
| `fenced-div` | Pandoc-style `::: divs` with classes and attributes | — |
| `link-attributes` | `{#id .class}` attribute lists on links and images | — |
| `highlighter` | Syntax highlighting via syntect-assets, VSCode theme conversion, custom `.sublime-syntax` registration | adds `syntect`, `syntect-assets`, `jsonc-parser`, `serde`, `serde_json` |
| `no-std`/`alloc` | Embedded use without std (parser core only) | — |

`Engine` and `Engines` are `#[non_exhaustive]`; use the constructors and
`set()`/`without()` rather than struct literals. `parser_extensions()` is
crate-internal; the public entry point is
`markdown_to_html_string_with_options()`.

Supported minimal configurations: `default-features = false, features = ["std"]`
for the std parser/renderer core, or `features = ["no-std"]` for the embedded
parser core. The `no-std` feature is retained for compatibility; the three new
syntax extensions (`definition-list`, `fenced-div`, `link-attributes`) require
`std` and are not part of a pure no-std build. Enabling both `no-std` and a
`std`-requiring extension yields a hybrid build that compiles but is not a
supported embedded configuration.

See [docs.rs/mordant](https://docs.rs/mordant) for the full API documentation.

## Architecture

The Python package wraps the [mordant](https://crates.io/crates/mordant) Rust crate via PyO3 0.29: arena-allocated AST with `Rc<RefCell<Arena>>` / `Rc<str>` sharing across `Document`, `Node`, and `Walker`; GIL released via `Python::detach()`; YAML frontmatter with thematic-break conflict resolution. All engines (lint, diagram, chunker, math, highlighter, meta, emoji, footnotes, definition lists, fenced divs, link attributes) live in the core crate behind cargo features — the Python layer is a thin shim.

See [ARCHITECTURE.md](docs/ARCHITECTURE.md) for the parsing/rendering pipelines, parser priorities, extension system, and per-engine design notes (§6 frontmatter, §7 extensions, §8 linter including the fix-engine re-parse loop, §9 highlighting).

## Performance

Single-threaded (50 iterations) — see [QUICKREF.md](docs/QUICKREF.md#performance-benchmarks) for the multi-threaded table and details:

| Fixture | mordant | mistune | markdown-it-py | python-markdown |
|---------|---------|---------|----------------|-----------------|
| Small (400B) | **0.039ms** | 0.430ms | 0.475ms | 2.301ms |
| Medium (5.4KB) | **0.155ms** | 2.448ms | 3.940ms | 6.455ms |
| Large (26.7KB) | **0.410ms** | 8.611ms | 16.743ms | 31.304ms |
| Data (202KB) | **2.763ms** | 38.152ms | 65.736ms | 621.295ms |

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

1288 Python tests passing (Core, AST, GFM, Options, YAML Frontmatter, Emoji, Mermaid Diagrams, Math, Lint engine, CLI, batch API, Phase 8 accuracy, VSCode theme, Chunker, OKF chunker methods, Extracted Chunk, Mixed Features, Standalone Highlighting) + the full Rust unit/integration/CommonMark spec/doctest suite.

## Theme Loading

Themes load from bundled `mordant/themes/`, `~/.mordant/themes/` (or `%APPDATA%/mordant/themes/` on Windows), and syntect-assets built-ins; register your own via `add_custom_theme(name, content)` (VSCode JSON and Sublime `.tmTheme` supported).

See [QUICKREF.md](docs/QUICKREF.md#theme-loading) for details.

## License

MIT

## Author

- Rust core (`mordant` on crates.io): originally [rushdown](https://github.com/yuin/rushdown) by [Yusuke Inuzuka](https://github.com/yuin), forked and extended as `mordant` by [opticsWolf](https://github.com/opticsWolf) — all engines (meta, emoji, footnotes, linter, diagram, chunker, math, highlighter, definition lists, fenced divs, link attributes) now live in the core crate
- Python bindings: by [opticsWolf](https://github.com/opticsWolf)
