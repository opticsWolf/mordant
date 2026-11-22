# mordant 0.11 — release plan

> **Status:** implemented on `dev0.11.0` (uncommitted). The bug-fix plan
> (`docs/BUGFIX_PLAN_0.11.md`) and its Appendix B Rust-guidelines amendment
> have been applied. This document remains the design record for the release.

Lockstep release of the Rust crate (`mordant`) and Python package (`mordant`),
both moving `0.10.1` → `0.11.0`.

## Goals

1. **All engines ship in every default build and the Python wheel; every
   engine except syntax highlighting is on by default and all can be switched
   off.** Same behavior in Rust and Python; one capability model instead of
   the current mix of compile-time-only cargo features (Rust) and always-on
   registration (Python). Highlighting stays opt-in to preserve 0.10 output.
2. **Granular rendering/engine options.** Replace the current pile of top-level
   kwargs with structured, per-engine options, while keeping 0.10 call sites
   working.
3. **Add the three missing rushdown syntax extensions**: definition lists,
   fenced divs, link attributes — implemented in the core using the same
   extension pattern as `meta`, `emoji` and `footnotes`.
4. **Upgrade `katex-rs` 0.2.4 → 0.3.0** (KaTeX 0.18.5), including the bundled
   stylesheet.

---

## 1. Capability model: shipped-by-default, runtime-disablable

### Rust

- Extend `Cargo.toml`:
  - `default = ["std", "html-entities", "meta", "emoji", "footnotes", "linter",
    "diagram", "chunker", "math", "highlighter", "definition-list",
    "fenced-div", "link-attributes"]`.
  - Keep all existing feature names as opt-out levers via
    `default-features = false`; the embedded core is `std`-free, while the
    three new syntax extensions require `std`.
  - Add the three new features as `["std"]` (no new dependencies).
- New public module `src/extensions.rs` (always compiled, re-exported at the
  crate root):

  ```rust
  #[non_exhaustive]
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub enum Engine { /* one variant per engine; GFM is not included */ }

  #[non_exhaustive]
  #[derive(Clone, Copy, PartialEq, Eq)]
  pub struct Engines { /* one bool per engine, incl. the 3 new ones */ }

  impl Engines {
      pub const fn all() -> Self { /* every flag true */ }
      pub const fn none() -> Self { /* every flag false */ }
      pub fn only(...) -> Self { /* convenience */ }
      pub fn without(self, ...) -> Self { /* default/all minus named */ }
  }
  ```

  Effective state for an engine = `cfg!(feature = "...") && flag`. Flags are
  silently inert when the feature was not compiled in; document this.
- Add `engines: Engines` to `parser::Options` and `renderer::html::Options`,
  both defaulting to `Engines::default()` (all except highlighting).
  `new_markdown_to_html*` keeps its
  explicit extension arguments; add
  `markdown_to_html_string_with_options(out, source, parser_opts, render_opts)`
  which composes the built-in extensions from the flags (and keeps GFM as a
  separate, opt-in bundle).
- Gate each `*_parser_extension()` / `*_html_renderer_extension()` registration
  on its flag. Parsing must not even create nodes for a disabled engine.
- `linter` / `chunker` entry points honour the same flags.

### Python

- `mordant-py/Cargo.toml`: keep the hardcoded feature list, append the three new
  features. The wheel therefore contains every engine.
- New `Engines` pyclass mirroring the Rust struct (`all()`, `none()`,
  `only([...])`, `without([...])`, `all_without([...])`, one boolean property
  per engine), default all-on except syntax highlighting.
- Accept it as a single `engines=` argument on `markdown_to_html()`, `parse()`,
  `lint()`, `fix*()` and store it in `ParseConfig`/`RenderConfig`, including the
  copy retained by `Document` so `Document.to_html()` stays consistent.
- `build_parser()` / `build_renderer()` (`mordant-py/src/lib.rs`) conditionally
  chain extensions based on the flags, instead of always chaining all of them.
- Register/export `Engines` in `src/lib.rs`, `mordant/__init__.py` and give
  `__init__.pyi` real signatures for the new option classes (today they are
  mostly `**kwargs`).

### Migration

- Rust users who want the old minimal build use
  `mordant = { version = "0.11", default-features = false, features = ["std"] }`.
- Existing Python calls keep working: no argument means the default engine
  set (`Engines::default()`, highlighting off).

---

## 2. Granular rendering options

### Rust

- Group the existing per-engine renderer settings under the new options structs
  rather than exposing them only as extension-constructor arguments; keep the
  constructors as the low-level API.
- Add `html::Options` fields for the knobs currently spread across renderer
  extensions where that is feasible (e.g. math output format); the high-level
  convenience function applies them when building the extension chain.
- Keep the manually-composed `new_markdown_to_html` API unchanged for advanced
  users.

### Python

- Introduce one structured `RenderOptions` (extend the existing class, do not
  replace it) with nested, optional per-engine settings:

  ```
  RenderOptions(
      hard_wraps=..., xhtml=..., allows_unsafe=..., escaped_space=...,
      engines=Engines(...),
      emoji=EmojiHtmlRendererOptions(...),
      diagram=DiagramHtmlRendererOptions(...),
      footnote=FootnoteHtmlRendererOptions(...),
      math=MathRendererOptions(...),
      highlighter=HighlighterOptions(...),
  )
  ```

- Legacy kwargs (`emoji_render_opts=`, `diagram_render_opts=`,
  `footnote_render_opts=`, `math_renderer_opts=`, `highlighting_theme=`,
  `highlighting_mode=`, `theme=`) remain accepted as overrides; mark them
  deprecated in docstrings, remove in 0.12.
- Keep `GfmOptions` as the GFM-specific switch (`all()` / `none()` / explicit
  feature list); do not fold it into `Engines`.
- **Decision (resolved):** highlighting stays opt-in in the default engine set
  via `Engines::all_without_highlighter()` (the `Default` implementation).
  `Engines::all()` enables it with the `InspiredGitHub` default theme, and the
  legacy `highlighting_theme=` / `theme=` kwargs still turn it on. This keeps
  CommonMark output byte-identical to 0.10 unless requested.

---

## 3. New syntax extensions (core modules)

Each follows the `meta` / `emoji` / `footnote` pattern: custom AST kind(s),
`*_parser_extension()`, `*_html_renderer_extension()` where needed, feature
gate, Rust unit tests, Python binding wiring, Python tests, docs.

| Feature | Module | Parser mechanism | Renderer |
|---|---|---|---|
| `definition-list` | `src/definition_list.rs` | paragraph transformer for PHP Markdown Extra `Term` + `: definition` (single paragraph; multi-paragraph/block content deferred to 0.12) | `<dl>/<dt>/<dd>` |
| `fenced-div` | `src/fenced_div.rs` | `BlockParser` on `:::` runs; nested fences; `:::note` / `::: {.note #id}` via `src/parser/attribute.rs` | `<div>` with filtered attributes |
| `link-attributes` | `src/link_attributes.rs` | `AstTransformer`: consume a trailing `{#id .class key="val"}` text node after links/images | none — `renderer/html.rs:739,790` already emits node attributes |

Upstream syntax references: [definition lists](https://github.com/yuin/rushdown-definition-list),
[fenced divs](https://github.com/growler/rushdown-fenced-div),
[link attributes](https://github.com/yuin/rushdown-link-attribute).

Ordering in the extension chains matters; register new block parsers before
generic fences/lists and document the chosen order.

---

## 4. katex-rs 0.3.0 upgrade

Facts established for the plan:

- Before the upgrade: `katex-rs 0.2.4` (released 2026-03-07), caret range
  `>=0.2.4, <0.3.0`.
- Upgrade target: `katex-rs 0.3.0` (released 2026-09-05) — a
  **semver-breaking bump** outside the old range. Release notes: KaTeX
  dependency update, MathML CI tests, dependency updates, fix for issue #31.
- 0.3.0 tracks **KaTeX 0.18.5** (upstream commit `49904aa2…`); the bundled
  `KATEX_CSS` was upgraded from KaTeX **0.16.21** (matching 0.2.4) to 0.18.5.
- The API we use in `src/math.rs` (`render_to_string`, `KatexContext`,
  `Settings`, `OutputFormat`, `StrictMode`, `StrictSetting`) is still present in
  0.3.0, so the upgrade is expected to be source-compatible — must be verified.

Work items:

1. Change the dependency to `katex = { package = "katex-rs", version = "0.3.0", ... }`
   in **both** `Cargo.toml` and `mordant-py/Cargo.toml` (floor 0.3.0, ceiling
   `<0.4.0`; or spell out `>=0.3.0, <0.4.0`).
2. Regenerate `Cargo.lock` and `mordant-py/Cargo.lock` (the binding lockfile
   recorded mordant 0.10.0 before the upgrade).
3. Replace the embedded minified CSS with KaTeX 0.18.5's `katex.min.css`; verify
   font-family/file names and add any new fonts to the docs. Introduce a
   `KATEX_VERSION` constant and derive the CSS version comment from it.
4. Update assertions: `src/math.rs` and `mordant-py/tests/test_math.py`
   required `0.16.21` before the upgrade; they now assert `0.18.5`.
5. Run math + mixed-feature + highlighter test suites; diff representative
   rendered HTML/MathML against 0.2.4 output and record intentional changes in
   the CHANGELOG (0.18 adds/changes some markup and CSS classes).
6. Declare the new floor/ceiling in the README feature table and CHANGELOG.

---

## 5. Versioning, docs, CI

- Bump version in `Cargo.toml`, `mordant-py/Cargo.toml`,
  `mordant-py/pyproject.toml`, docs headers (`README.md`, `docs/ARCHITECTURE.md`,
  `docs/QUICKREF.md`), Pages site version, and both lockfiles.
- CHANGELOG entry for 0.11.0: engine flags, structured options, three new
  extensions, katex-rs/KaTeX upgrade, migration notes, deprecations.
- CI: add jobs for the full engine feature list (not `--all-features`, which
  also enables `pprof`/`no-std`), `--no-default-features --features std`, and
  Python smoke tests with `Engines.none()` / `Engines.only([...])`.
- Acceptance: CommonMark spec suite unchanged; all existing Rust/Python tests
  pass; new features each have Rust + Python + mixed-feature coverage; disabling
  an engine removes its syntax/markup from output.

## Decisions made during implementation

1. Highlighting remains opt-in in the default engine set; `Engines.all()`
   enables it. See §2.
2. GFM stays a separate option object (`GfmOptions`); it is not part of
   `Engines`.
3. Cargo `default` is the full engine set; the runtime default leaves syntax
   highlighting off. Minimal builds use `default-features = false, features =
   ["std"]`; the `no-std` feature covers the parser core only and is retained
   as a compatibility alias. The three new syntax extensions require `std`.

## Implementation status

Implemented on `dev0.11.0` (uncommitted): runtime engine switches and
structured options (Rust + Python), the three new extensions, the katex-rs
0.3.0 / KaTeX 0.18.5 upgrade, version bumps, tests and docs. The bug-fix plan
and the Rust-guidelines amendment in `docs/BUGFIX_PLAN_0.11.md` were applied
in the same working tree, including the security, `no-std`, doctest, and
public-API follow-ups.
