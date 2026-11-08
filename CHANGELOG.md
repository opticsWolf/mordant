# Changelog

All notable changes to `mordant`. The Rust crate (`mordant` on crates.io) and
the Python package (`mordant` on PyPI) share the same version number
(lockstep versioning since 0.9.0).

## [0.10.1]

### Fixed

- **Fix-engine soundness (native crash).** `mordant.fix` no longer reuses the
  pre-fix AST across iterations. The `Arena` holds byte offsets into the
  source it was parsed from; after `apply_fixes` changed the source length,
  the stale offsets were read via `get_unchecked` — SIGSEGV in release builds
  (exit 139) or `str.rs` out-of-bounds panics (exit 127) on documents with
  many fixes and multi-byte UTF-8 content (e.g. PDF-converted Markdown).
  Every iteration now re-parses the current source before linting, and only
  the current iteration's fixes are applied (line numbers refer to the source
  they were reported against). The final `remaining` lint also re-parses.
- Fixed the same stale-AST reuse in the legacy `run_fix` / `run_fix_with_params`
  entry points (first iteration uses the passed AST, later iterations and
  `remaining` re-parse with a default parser).

### Added

- New core API: `run_fix_with_parser` / `run_fix_with_parser_and_params`
  (take the caller-supplied `Parser` so re-parses match the original parse
  options, including GFM features). Python `fix()`, `fix_many()`, and
  `Document.fix()` all use it; `Document` now retains its parse config to
  rebuild the identical parser.
- Regression tests: `tests/lint_fix.rs` (multi-iteration shortening,
  legacy entry point, fixpoint stability) and
  `test_fix_reparses_after_shortening` in `mordant-py/tests/test_lint.py`.

### Docs

- README condensed (details moved to `docs/QUICKREF.md` and
  `docs/ARCHITECTURE.md`); fix-engine re-parse behavior documented in both.
- GitHub Pages site version bump.

## [0.10.0]

- **Standalone highlighting API** — the bundled syntect engine is now usable
  directly, without the Markdown pipeline:
  - `Highlighter.highlight(lang, code, bare=True)` returns only the
    highlighted token spans (no `<pre>/<code>` wrapper) for embedding into
    your own HTML; pair with `theme_background(name)` for the theme's
    background color.
  - `add_custom_syntax(content, name=None)` registers custom
    `.sublime-syntax` (YAML) definitions at runtime; they become available to
    `markdown_to_html`, `Highlighter`, and `list_syntaxes()`.
  - `detect_language(code)` exposes content-based language auto-detection
    (shebang → token → extension → heuristics).
  - Rust parity: `register_custom_syntax`, `detect_language`,
    `highlight_spans`, `theme_background` in `mordant::highlighter`.
- **Feature decoupling** — the `highlighter` cargo feature no longer pulls in
  `math` (KaTeX); the math-fence interception in the highlighting renderer is
  now compiled only when both features are enabled.
- 141 core unit tests plus the full CommonMark spec suite; Python bindings
  add 1247 integration tests.

## [0.9.0]

- **Rust crate on crates.io** — the full engine published as `mordant`
  ([docs.rs](https://docs.rs/mordant)). All engines live in the core crate
  behind granular cargo features; the Python package is a thin binding layer
  over it.
- **Feature parity** — every engine previously exclusive to the Python
  bindings is now available to Rust users: `meta` (YAML frontmatter),
  `emoji`, `footnotes`, `linter` (25 MD rules + fix engine), `diagram`
  (Mermaid server/client/hybrid rendering + theme derivation), `chunker`
  (AST chunk iterator with mmap support), `math` (KaTeX), `highlighter`
  (syntect highlighting + VSCode theme conversion).
- **Lockstep versioning** — the Rust crate and Python package share the same
  version number.
- 134 core unit tests plus the full CommonMark spec suite; Python bindings
  add 1233 integration tests.

## [0.8.11]

- `RuleMetadata` export fixed — `from mordant import RuleMetadata` works.
- `LintConfig.from_dict` — CLI `--config` works; parses
  `.markdownlint.json` into a config object.
- Stub file corrected — accurate signatures for `parse`, `render_math`,
  `lint`, `fix`, `lint_many`, `fix_many` and correct option class names.
- `MarkdownChunker` API documented — actual methods (`get_chunks`,
  `get_all_chunks`, `get_chunks_with_context`, `get_bare_chunks`,
  `compute_overlap_payloads`) instead of the non-existent `chunk`.

## [0.8.10]

- **Server-side Mermaid rendering** — diagrams render as inline SVG via
  `mermaid-rs-renderer` (~3 ms server-side vs ~2 s client-side). Three render
  modes: `server` (default, inline SVG), `client` (legacy Mermaid.js ESM),
  `hybrid` (try server, fallback to client).
- **Render mode API** — `DiagramHtmlRendererOptions(render_mode="server" |
  "client" | "hybrid", mermaid_url=...)`.
- **Customizable Mermaid themes** — color schemes derived from
  code-highlighting themes; a single `theme=` kwarg themes both code and
  diagrams; native mermaid themes (`modern` / `dark` / `forest` / `neutral`)
  also supported.

## [0.8.9]

- Math rendering fix (see commit `3618d9f`).

## [0.8.7]

- **Chunker GFM + Diagram parity** — `MarkdownChunker` uses the same parser
  extensions as `parse()` / `markdown_to_html()`; GFM tables and Mermaid
  diagrams classify as `BlockType::Table` / `BlockType::Diagram`.
- **Chunker Diagram block type** — mermaid code blocks yield `"Diagram"`.
- **Diagram source position fix** — `DiagramAstTransformer` copies the
  original code block's `pos()` so the chunker slices raw source correctly.

## [0.8.6]

- **Lint engine** — 25 lint rules (MD001–MD050) with diagnostics, fix engine,
  and configuration.
- **Batch API** — `lint_many()` / `fix_many()` for parallel file processing
  via `rayon`, with GIL release for the entire batch.
- **CLI** — `python -m mordant` with `--fix`, `--dry-run`, `--format`
  (human/json/github), `--config`, `--enable`, `--disable`,
  `--default-language`, glob/directory recursion.
- **Phase 8 accuracy polish** — emoji text in heading comparison (MD024),
  frontmatter `title:` support (MD025), fragment anchor validation (MD042).
- **Document chunking** — `MarkdownChunker` lazy AST-based chunk iterator
  yielding bare chunks, with `get_chunks()`, `get_all_chunks()`,
  `get_chunks_with_context()`, `get_bare_chunks()`, `ExtractedChunk`,
  `get_delimiter()`, `compute_overlap_payloads()`.
- **Inline suppression** — `<!-- markdownlint-disable MD001 -->` comments.
- **VSCode JSON theme support** — custom themes via `add_custom_theme()`
  and `~/.mordant/themes/`.
- 1297 tests passing (up from 1161).
