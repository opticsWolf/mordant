# Plan: Standalone Syntax Highlighting (Smaller Surface)

> Goal: expose mordant's bundled syntect engine for **independent** use — no Markdown
> pipeline required — while keeping the new API surface **minimal**.

---

## 1. Findings — what already exists

The original gap analysis overestimated the work. Most of the "standalone" story is
already shipped in v0.9.0:

| Capability | Rust core (`src/highlighter.rs`) | Python (`mordant-py`) |
|---|---|---|
| Highlight a snippet → HTML | `highlight_code(lang, code, theme, mode)` | `Highlighter(theme, mode).highlight(lang, code)` |
| Theme registry (global) | `register_custom_theme`, `list_themes`, `resolve_theme` | `add_custom_theme`, `list_themes` |
| Syntax listing | `list_syntaxes()` | `list_syntaxes()` |
| Auto-detection | `detect_syntax_from_content(ps, code)` (public) | ❌ not exposed |
| VSCode JSON / .tmTheme themes | full support | full support (import-time dir scan + `add_custom_theme`) |
| Markdown integration | `highlighting_html_renderer_extension` | `markdown_to_html(..., theme=)` |

The Python `Highlighter` class already highlights arbitrary code without parsing any
Markdown. The engine lives in the core crate; PyO3 is a thin layer — matching the
existing "pure engine in core, bindings in mordant-py" pattern.

### Verified technical facts (syntect 5.3.0, `default-fancy` features)

- `SyntaxDefinition::load_from_str(s, lines_include_newline, fallback_name)` → parses
  `.sublime-syntax` (YAML) from memory. `yaml-load` feature is **enabled** by
  `default-fancy` in our dependency config.
- `SyntaxSet::into_builder()` → round-trips a set back to a builder so syntaxes can be
  added. Docs explicitly support this: *"newly added syntaxes can have references to
  existing syntaxes in the set, but not the other way around"*.
- `.tmLanguage` (plist) syntax files are **not** loadable from str/folder in syntect 5
  (`add_from_folder` only walks `*.sublime-syntax`). Scope custom syntaxes to
  `.sublime-syntax` YAML — same limitation bat lives with.
- `SYNTAX_SET` is only referenced inside `src/highlighter.rs` — changing its container
  is fully contained.

## 2. Real remaining gaps

1. **Custom syntax registration** — no way to add `.sublime-syntax` definitions; the
   global `SyntaxSet` is an immutable `Arc`. ← the genuine gap
2. **Language auto-detection not queryable** — detection runs implicitly inside
   `highlight_code` but callers can't ask *"what language is this snippet?"*.
3. **No wrapper-free output** — `highlight()` always emits
   `<pre style="background-color: …; padding: 12px; …"><code class="language-X">…`.
   Embedding into own HTML (blogs, notebooks, terminals) needs bare spans and the
   theme's background color to build one's own container.
4. **Theme background not queryable** — needed for the custom container above.

Explicitly **deferred** (keeps surface small): theme JSON export/serialization,
per-line token/ANSI output, syntax↔extension mapping tables, a user-facing
`SyntaxHighlighter` builder class.

## 3. Design — reuse the registry, don't add a class

The original proposal sketched a `SyntaxHighlighter` struct owning its own
`SyntaxSet`/`ThemeSet`. That duplicates state, forks the theme registry that Mermaid
derivation already shares, and adds a second object model. Instead: extend the
**existing global registries** (the same pattern custom themes already use) and the
existing `Highlighter` class.

### 3.1 Rust core additions (src/highlighter.rs)

```rust
/// Make a custom .sublime-syntax (YAML) available to all highlighting
/// (markdown pipeline + Highlighter). Returns the registered syntax name
/// (from the YAML `name:` key, or the fallback passed via `name`).
/// New syntaxes may reference built-in syntaxes, not vice versa (syntect rule).
pub fn register_custom_syntax(content: &str, name: Option<&str>) -> Result<String, String>;

/// Detect the language of a snippet (shebang → token → extension → heuristics).
pub fn detect_language(code: &str) -> String;

/// Highlighted token spans only — no <pre>/<code> wrapper.
pub fn highlight_spans(language: &str, code: &str, theme_name: &str, mode: &HighlightingMode) -> String;

/// Theme background as "#rrggbb" (for building your own container).
pub fn theme_background(theme_name: &str) -> Option<String>;
```

Container change: `SYNTAX_SET: LazyLock<Arc<SyntaxSet>>` →
`LazyLock<RwLock<Arc<SyntaxSet>>>`. Readers deref the `Arc` and highlight against an
immutable set (no lock held while highlighting); registration clones the `Arc`'d set,
`into_builder()`, `add(def)`, `build()`, swap the `Arc`. Registration is rare → the
one-time full-set rebuild (~ms) is acceptable.

Internal cleanup while touching the file: `highlight_code` resolves the language,
then `render_attribute_mode`/`render_class_mode` resolve it **again** (double
detection). Unify into span-producing helpers shared by `highlight_code` (wrapped)
and `highlight_spans` (bare).

### 3.2 Python surface additions (mordant-py)

```python
mordant.add_custom_syntax(content: str, name: str | None = None) -> str
mordant.detect_language(code: str) -> str
mordant.theme_background(name: str) -> str | None

hl = mordant.Highlighter(theme="Dracula")
html = hl.highlight("python", code, bare=True)   # spans only, no <pre>/<code>
bg   = mordant.theme_background("Dracula")        # '#282a36'
```

Total new surface: **3 module functions + 1 optional kwarg**. Existing signatures
unchanged (`highlight()` gains a keyword-only `bare=False`). `list_syntaxes()`
automatically includes user syntaxes after registration (set is swapped in place).
Markdown rendering path untouched.

### 3.3 Feature decoupling (optional, recommended)

`highlighter` currently depends on `math` only because `HighlightingHtmlRenderer`
intercepts ```` ```math ```` fences. Pure-Rust users wanting *just* highlighting pay
for KaTeX. Gate the math import, the `math_options` field, and the math-fence branch
behind `#[cfg(feature = "math")]`, and drop `math` from the `highlighter` feature's
dependency list. `mordant-py` enables both features → zero Python impact. This also
delivers the spirit of the proposed `highlighter-standalone` feature **without adding
a new feature name**.

## 4. Route of action

| Step | Work | Files | Est. |
|---|---|---|---|
| 1 | Core: `SYNTAX_SET` → `RwLock<Arc<SyntaxSet>>`; implement the 4 new functions; unify language resolution / span helpers | `src/highlighter.rs` | 0.5 d |
| 2 | Core tests: register a minimal `.sublime-syntax`, highlight + detect + bare assertions + error case | `src/highlighter.rs` (tests mod) | 0.25 d |
| 3 | PyO3: 3 `#[pyfunction]`s + `bare` kwarg on `PyHighlighter.highlight`; module registration | `mordant-py/src/highlighter.rs`, `lib.rs` | 0.25 d |
| 4 | Optional: cfg-gate math in highlighter, drop `math` dep from feature, verify `cargo check --no-default-features --features highlighter` | `src/highlighter.rs`, `Cargo.toml` | 1–2 h |
| 5 | Python tests: standalone highlight, custom syntax via `add_custom_syntax`, detect, bare mode | `mordant-py/tests/` | 0.25 d |
| 6 | Docs + release: `__init__.pyi` signatures, `__init__.py` exports, QUICKREF section, README "What's New" + version bump **0.10.0** (new public API ⇒ minor) in both `Cargo.toml`s | docs, stubs, manifests | 0.5 d |

**Total: ~2 days core work**, vs ~6 weeks in the original roadmap.

## 5. Risks & notes

- **`into_builder()` round-trip** — documented and supported in syntect 5.3; the one
  caveat (user syntaxes may reference built-ins, not vice versa) is inherent to
  syntect and will be documented.
- **Rebuild cost per registration** — one `SyntaxSet` clone + build per
  `add_custom_syntax` call; rare operation, acceptable. Documented.
- **`lines_include_newline`** — use `true` everywhere (matches the newlines-based
  default set and the existing `LinesWithEndings` feeding).
- **Windows paths** — syntaxes registered from strings are unaffected; if a folder
  loader is added later, use syntect's own path normalization.
- **Backward compatibility** — no removals; `highlight()` default behavior identical;
  markdown pipeline untouched; feature list in README unchanged unless step 4 is
  taken (then: `highlighter` no longer pulls `math`, a strictly additive relaxation).
