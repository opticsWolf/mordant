# Changelog

All notable changes to `mordant`. The Rust crate (`mordant` on crates.io) and
the Python package (`mordant` on PyPI) share the same version number
(lockstep versioning since 0.9.0).

## [0.11.0]

### Added

- Runtime engine switches shared by Rust and Python: `mordant::Engines`
  (also `mordant::extensions::Engines`; `Engine` is re-exported at the crate
  root) with `Engines::all()/none()/only()/without()`, and the Python
  `mordant.Engines` class. All engines ship in the wheel and are enabled by
  default, except syntax highlighting which stays opt-in (`Engines.all()`) to
  preserve 0.10 output. Pass `engines=` to `markdown_to_html()`, `parse()`,
  `lint()`, `fix()` or set it on `ParseOptions`/`RenderOptions`. Rust gains
  `markdown_to_html_string_with_options()`; Rust-only `available_engines()`
  reports the compiled-in engines.
- Structured `RenderOptions`: nested `emoji=`, `diagram=`, `footnote=`, `math=`
  and `highlighter=` (`HighlighterOptions`) settings. Legacy top-level kwargs
  remain accepted as deprecated overrides until 0.12, but an explicit
  `engines=` set is authoritative for whether an engine is enabled.
  `GfmOptions` remains the separate GFM switch.
- Three new core extensions, each behind its own cargo feature and runtime flag:
  `definition-list` (`<dl>/<dt>/<dd>`), `fenced-div` (Pandoc-style `:::` with
  classes/attributes and nesting), and `link-attributes` (`{#id .class ...}`
  after links/images).
- `KATEX_VERSION` is exposed in Rust and Python alongside `KATEX_CSS`.

### Changed

- Upgraded `katex-rs` 0.2.4 → 0.3.0 in both manifests. The accepted range is
  now `>=0.3.0, <0.4.0` (floor 0.3.0, ceiling <0.4.0); lockfiles regenerated.
- Bundled KaTeX CSS updated from 0.16.21 to **0.18.5** to match katex-rs 0.3.0.
  Math markup may differ slightly from 0.10 (new upstream KaTeX release).
- The default cargo feature set now includes all engines; use
  `default-features = false` for the old minimal build.
- Python wheels and the Rust crate are versioned 0.11.0 in lockstep.
- `Engines.without(...)` now starts from the **default** engine set, not
  `Engines::all()`. Use `Engines.all().without(...)` in Rust or
  `mordant.Engines.all_without([...])` in Python for the "all except" case.
  An explicit `engines=` argument now takes precedence over legacy
  `theme=` / `highlighting_theme=` kwargs (which only set values).
- Parsing and rendering of a document use one effective engine set: an engine
  set supplied on either side is inherited by the other. Conflicting explicit
  sets raise `ValueError` in Python instead of silently dropping AST nodes.
- `Engine` and `Engines` are `#[non_exhaustive]` so future engines can be
  added additively; use the constructors and `set()`/`without()` rather than
  struct literals or exhaustive matches. The engine-gated parser-extension
  factory (`parser_extensions()`) is crate-internal, not a public API.
- Attribute URL checks now decode character references and ignore ASCII
  whitespace/control bytes before scheme matching; `allows_unsafe` docs cover
  `style`, `srcdoc` and dangerous URL values from the new extensions.

### Fixed

- Fenced div bodies are now parsed as children of `<div>` (previously the
  body became a sibling and the closing fence produced an empty div). Nested
  divs work without indentation; a closing fence must have at least as many
  colons as the opener and may carry ignored trailing attributes.
- Definition lists no longer drop the first definition line or preceding
  paragraph lines, and no longer swallow the following term. A term must be
  the first line of its paragraph; a colon only starts a definition when
  followed by whitespace (`Term` + `:smile:` stays emoji). Multi-paragraph and
  block-level definitions remain unsupported and are documented as 0.12 work.
- Link attributes survive inline parser splits (`_`, `:`, `*`, backticks) by
  parsing the contiguous source span instead of the first text node.
- Attribute lists can no longer inject a duplicate `href`/`src`, and `style`
  / dangerous URL values from the new extensions require `allows_unsafe=True`.
  The older `ParseOptions(attributes=True)` path keeps its 0.10 behavior;
  unifying the two attribute policies is planned for 0.12.
- Rust default `html::Options`, `parser::Options` and Python defaults now all
  disable syntax highlighting. `RenderOptions(highlighter=...)` enables it
  unless an explicit `engines=` disables it.
- `Engines.without(...)` no longer silently enables highlighting; option
  sub-object mutation (`opts.engines.emoji = False`) now works as expected.
- `lint()`/`fix()` honor the `linter` engine flag; `MarkdownChunker` accepts
  `engines=` and gates extensions accordingly.
- Rust `available_engines()` now reports only the compiled-in engines. Raw
  theme and mode values passed to `HighlighterOptions` are validated.
- Minimal-feature Rust tests and doctests are feature-gated and pass with
  `--no-default-features --features std`; CI covers that configuration. The
  no-std debug writer is now gated to Unix, so enabling
  `no-std-unix-debug` on Windows compiles again. Unsafe blocks touched by
  this release now carry `SAFETY:` comments.
- Python `MarkdownChunker.get_bare_chunks()` propagates list-append errors as
  Python exceptions instead of panicking; a broken chunker intra-doc link is
  fixed.

### Migration

- Rust users who relied on the minimal default build should pin
  `mordant = { version = "0.11", default-features = false, features = ["std"] }`.
- `no-std` remains the parser-core compatibility configuration. The three new
  syntax extensions (`definition-list`, `fenced-div`, `link-attributes`)
  require `std`; a build combining `no-std` with one of them compiles but is a
  hybrid, not a supported embedded configuration. The feature model is planned
  to move to a `std` feature in 0.12.
- Python callers need no changes; no argument means the 0.11 default engine set.
  Pass `engines=mordant.Engines.all()` to additionally enable syntax
  highlighting for every code block.
- Rust code that used `Engines { .. }` literals must switch to
  `Engines::default()/all()/none()/only()` plus `set()`/`without()`; adding an
  engine is now non-breaking. Matches on `Engine` need a wildcard arm.
  `parser_extensions()` is no longer public; custom pipelines should compose
  parser extensions directly.

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
