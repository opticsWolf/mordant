# mordant 0.11 — Bug-fix implementation plan

> **Status:** implemented on `dev0.11.0` (not yet committed), including the
> Appendix B Rust-guidelines amendment. D1–D10 were
> resolved as recommended, with these adjustments: Rust `Engines::without` is
> an instance method (`Engines::default().without(...)` /
> `Engines::all().without(...)`); Python adds `Engines.all_without([...])`
> for the instance-style case; D10 kept `linter`/`chunker` flags and made them
> effective. See CHANGELOG.md for the user-visible behavior changes.
> **Source:** review of the `dev0.11.0` working tree (runtime `Engines`, new
> syntax extensions, structured `RenderOptions`, katex-rs 0.3.0 upgrade).
> **Release target:** 0.11.0 if it has not been published yet; otherwise a
> lockstep `0.11.1` Rust + Python patch release.
> **Amendment:** Appendix B records a post-implementation Rust-guidelines
> review. Its actionable findings (GL-01…GL-18, GL-20) are **applied** on
> `dev0.11.0`; GL-19 is an accepted binding deviation. See §B.9 for status and
> §B.8 for the verification matrix.

---

## 0. Executive summary

The 0.11 feature work builds and the existing test suites pass, but several
new paths are effectively untested and are broken in normal use:

| ID | Severity | Area | One-line symptom |
|---|---|---|---|
| BF-01 | Critical | fenced div | Body never becomes a child of `<div>` |
| BF-02 | Critical | definition list | First definition line is dropped when a continuation exists |
| BF-03 | Critical | definition list | Lines before the term are silently deleted |
| BF-04 | Critical | definition list | Next term/paragraph is swallowed into the previous `<dd>` |
| BF-05 | Critical | link attributes | Lists split by inline parsers (`_`, `:`, `*`, backtick) are ignored |
| BF-06 | High (security) | link attributes | Duplicate `href` bypasses `allows_unsafe`; default-on `style` injection |
| BF-07 | Critical (CI) | tests | `tests/engines.rs` and the `Engines` doctest fail without all features |
| BF-08 | High | definition list | `:smile:` at line start is eaten by definition-list parsing |
| BF-09 | High | defaults | Rust `html::Options::default()` highlights; Python default does not |
| BF-10 | High | Python API | `Engines.without()` silently enables highlighting |
| BF-11 | High | Python API | `RenderOptions(highlighter=...)` does not imply highlighting |
| BF-12 | High | precedence | Legacy `theme=` overrides explicit `engines=Engines.none()` |
| BF-13 | High | engines | `linter` / `chunker` flags do nothing |
| BF-14 | High | Python API | Mutating `opts.engines` / `opts.highlighter` in place is a silent no-op |
| BF-15 | High | Python API | Parse/render engine mismatch silently deletes content |
| BF-16 | Medium | API | `available_engines()` ignores compiled features |
| BF-17 | Medium | API | `HighlighterOptions` accepts invalid theme/mode silently |
| BF-18 | Medium | precedence | Nested `diagram=` options are dropped when `theme=` is set |
| BF-19 | Medium | chunker | Chunker ignores new syntax and has no `engines` input |
| BF-20 | Medium | docs/API | Stale defaults, missing node kinds, wrong QUICKREF note |
| BF-21 | Medium | fenced div | Fence length/attributes rules are not enforced |
| BF-22 | Low | definition list | Multi-paragraph/block definitions unsupported (scope/decision) |
| BF-23 | Low | cleanup | Lockfile churn, unused warnings, CI redundancy |

**Blocking recommendation:** do not publish 0.11.0 until BF-01 through BF-07
are fixed and covered by regression tests. BF-08 through BF-15 should also
land in the same release because they are user-visible semantics/documentation
issues introduced by the new API.

---

## 1. How to reproduce the current behavior

All commands assume `D:\User\Documents\Python\mordant` and the built Python
extension (`maturin develop --release` or the test wheel).

```python
import mordant as m

# BF-01
m.markdown_to_html("::: note\nHello.\n:::\n")
# '<div class="note">\n</div>\n<p>Hello.</p>\n<div>\n</div>\n'

# BF-02
m.markdown_to_html("Term\n: a\nb\n")          # <dd>\nb\n</dd>

# BF-03
m.markdown_to_html("intro\nTerm\n: def\n")    # "intro" is gone

# BF-04
m.markdown_to_html("Apple\n: A fruit.\nOrange\n: Citrus fruit.\n")

# BF-05
m.markdown_to_html('[a](https://x){#id .cls target="_blank"}')
m.markdown_to_html("[a](x){.my_class}")
m.markdown_to_html('[a](x){title="a:b"}')

# BF-06
m.markdown_to_html('[a](https://safe){href=javascript&#58;alert(1)}')
m.markdown_to_html('::: {style=color:red}\nx\n:::\n')

# BF-08
m.markdown_to_html("Hello\n:smile: world\n")

# BF-10
m.Engines.without(["emoji"])                  # highlighter becomes enabled

# BF-14
opts = m.ParseOptions(); opts.engines.emoji = False; opts.engines.emoji  # True
```

```text
# BF-07
cargo test --no-default-features --features std --test engines
cargo test --no-default-features --features std --doc
```

---

## 2. Design decisions required before coding

These decisions change public behavior, so record them in `CHANGELOG.md` and
the docs as part of the fix.

| # | Decision | Recommendation |
|---|---|---|
| D1 | Default engine set (highlighting on/off) | Keep highlighting **off** in `Engines::default()`; make every default align with it (BF-09). `Engines::all()` is the explicit opt-in. |
| D2 | `Engines::without` base | **Resolved:** `without` is an instance method. `Engines::default().without(...)` keeps highlighting off; `Engines::all().without(...)` / Python `Engines.all_without([...])` is the “all except” case (BF-10). |
| D3 | Precedence: explicit `engines=` vs legacy kwargs | Explicit `engines=` / `render_opts.engines` wins. Legacy `theme=` / `highlighting_theme=` only turns highlighting on when no explicit engine set was supplied (BF-11/BF-12). |
| D4 | Nested `highlighter=` semantics | Passing `RenderOptions(highlighter=...)` enables the engine unless an explicit `engines=` overrides it (BF-11). |
| D5 | Definition-list syntax strictness | Require whitespace/tab/EOL after the definition colon; require the term to be the first line of the paragraph (BF-03/BF-08). Multi-line terms are out of scope; document. |
| D6 | Definition continuation semantics | Allow lazy continuation lines, but a line whose **next** line is a definition line starts a new term (BF-04). Multi-paragraph/block definitions are out of scope for this patch (BF-22). |
| D7 | Fenced-div closing fence | Prefer Pandoc’s “at least as long as the opening fence” rule; accept and ignore trailing attributes on the closing fence. Document the choice (BF-21). |
| D8 | Attribute security policy | `href`/`src` supplied via attribute lists must be dropped; `style` must require `allows_unsafe` (or be removed from the extension allow-list) for new default-on extensions (BF-06). |
| D9 | Parse/render engine coupling | Renderer inherits the parser’s engine set unless `RenderOptions(engines=...)` or `engines=` is explicit (BF-15). |
| D10 | `linter` / `chunker` flags | **Resolved:** kept both flags and made them effective: `lint`/`fix`/batch variants honor `linter`, and `MarkdownChunker` accepts `engines=` (BF-13/BF-19). |

---

## 3. Critical fixes

### BF-01 — Fenced div body must become children of the div

**Severity:** Critical (feature unusable)
**Files:** `src/fenced_div.rs` (`cont`, lines 148–174; `open`, 90–146)

**Root cause**

`cont()` calls `indent_position(line, line_offset, 4)?`. Content lines with
0–3 columns of indentation make `indent_position` return `None`, which the
parser interprets as “close this block”. The body is then parsed as a sibling
and the real closing fence becomes a new empty `<div>`.

Nested divs fail for a second reason: the outer `cont` consumes four spaces
before the inner div sees the same line.

**Implementation**

1. Rewrite `cont` to behave like a container (`Blockquote`/`ListItem`), not a
   leaf code block:
   - Read `peek_line_bytes`.
   - Return `HAS_CHILDREN` for blank lines.
   - If the line starts with 0–3 spaces and then a colon run matching the
     closing rule, consume the line and return `None`.
   - Otherwise **do not mutate the reader**; return `HAS_CHILDREN` so the
     normal `open_blocks` path parses the content.
2. Do not strip any content indentation in `cont`. CommonMark’s `open_blocks`
   already accepts up to three leading spaces; four or more should become an
   indented code block inside the div.
3. Keep the opening-fence parser responsible for the fence line only.
4. Verify `open` position math under lists/blockquotes. The current
   `segment.start() - segment.padding() + pos` should be replaced with the
   same pattern used by `FencedCodeBlockParser` (`segment.start() +
   segment.padding()` only where required) or validated with tests; avoid
   subtraction that can underflow.
5. Add a test that asserts the body is **inside** the div, not merely present:
   ```rust
   let html = render("::: note\nHello *world*.\n:::\n");
   assert_eq!(
       html,
       "<div class=\"note\">\n<p>Hello <em>world</em>.</p>\n</div>\n"
   );
   ```
6. Add tests for:
   - empty div,
   - unclosed div at EOF,
   - nested `::: outer` / `::: inner` without indentation,
   - div inside a list item and inside a blockquote,
   - body indented 4 spaces (must become a code block inside the div),
   - closing fence indented 4+ spaces (not a close),
   - closing fence with a longer colon run (per D7),
   - closing fence with trailing attributes (per D7),
   - `:::note` (no space) behavior documented.

**Acceptance:** rendered body is a child of `<div>`; no stray empty `<div>`
for normal closing fences; blockquote/list nesting preserved.

---

### BF-02 — Definition lists must not drop the first content line

**Severity:** Critical (data loss)
**Files:** `src/definition_list.rs` (lines 136–172); `src/text.rs` /
`BlockReader` behavior at 1442–1470 and `parse_block` at `src/parser/mod.rs:1432`

**Root cause**

`trim_segment` trims trailing whitespace including the line terminator from
the first definition content line. `BlockReader::advance` sees a segment that
ends before the block’s `last` byte and calls `advance_line()`, so
`parse_block` detects `l != current_l` and skips the text. Continuation lines
are appended untrimmed and therefore work.

Evidence: `Term\n: a\nb\n` yields `<dd>b</dd>`; the AST block source still
contains both `a` and `b`.

**Implementation**

1. Add a line-trim helper that preserves the trailing `\n`/`\r\n`:
   ```rust
   // Keep the terminator; trim leading spaces/tabs and trailing
   // spaces/tabs before the terminator.
   fn trim_line(segment: Segment, source: &str) -> Segment;
   ```
   Or, simpler: trim only the left side and keep `segment.stop()` for the
   first content line, letting `parse_block`’s `trim_right_space` handle the
   display text.
2. Apply the same rule to the term (`dt`) and to every appended line. Terms
   are single-line today, but the rule must be uniform.
3. Use `is_blank` for the empty-content check rather than `is_empty`.
4. Keep the paragraph’s original newline-bearing segments for continuation
   lines; trim only their leading indentation (see BF-04).
5. Add a Rust unit test and a Python test:
   ```python
   assert "<dd>\na\nb\n</dd>" in m.markdown_to_html("Term\n: a\nb\n")
   ```
   Also test CRLF input (`Term\r\n: a\r\nb\r\n`) and a definition whose first
   line is the last line of the document.

**Acceptance:** no definition content is lost, regardless of line count.

---

### BF-03 — Definition lists must not delete paragraphs before the term

**Severity:** Critical (data loss)
**Files:** `src/definition_list.rs:151–167`, `:215`

**Root cause**

The transformer finds *any* colon line after line 0 and uses the preceding
line as the term, then `lines.drain(0..i)` discards every line before it.

**Implementation**

1. Adopt D5: only recognize a definition list when the term is the **first
   line** of the paragraph (`first_def == 1`). Replace the `find_map` over
   `lines.iter().skip(1)` with a check of `lines[1]` only.
2. If `lines[1]` is not a definition line, put the paragraph source back and
   return unchanged. This preserves the entire paragraph.
3. Optionally add a future-compatible helper for multi-line terms, but do not
   enable it in this patch; document the limitation.
4. Regression tests:
   ```python
   m.markdown_to_html("intro\nTerm\n: def\n")  # keeps "intro"
   m.markdown_to_html("Term\ncontinued\n: def\n")  # all three lines preserved
   ```
   The second case must render as a paragraph, not a `<dl>`.

**Acceptance:** no source line before the chosen term is ever dropped.

---

### BF-04 — Definition lists must not swallow the next term or trailing text

**Severity:** Critical (content mis-association)
**Files:** `src/definition_list.rs:174–215`

**Root cause**

The continuation loop treats every non-blank, non-colon line as part of the
current `<dd>`, including the next term. `Apple\n: A fruit.\nOrange\n:
Citrus fruit.` puts `Orange` into the first `<dd>` (and, with BF-02, loses
`A fruit.` entirely).

**Implementation**

1. Track the current `<dd>` node.
2. For each line at index `i`:
   - If `definition_colon(line)` is `Some` → start a new `<dd>`.
   - Else if `i + 1 < lines.len()` and `definition_colon(lines[i + 1])` is
     `Some` → this line is a new `<dt>`; append a term node and continue.
   - Else → append the line as a lazy continuation of the current `<dd>`
     (PHP Markdown Extra allows lazy continuation; keep this behavior).
3. If a non-colon line appears before any `<dd>` exists, leave it in the
   paragraph (defensive).
4. Trim continuation indentation for display. Accept 0–3 leading spaces as
   indentation; preserve 4+ only if block content is implemented later
   (BF-22). For this patch, trimming leading spaces is acceptable and should
   be documented.
5. Keep the “remaining lines” path: if processing stops before consuming all
   paragraph lines, put the remainder back into the paragraph.
6. Tests:
   - two terms in one paragraph without blank lines,
   - multiple `<dd>` for one term,
   - lazy continuation (`Term\n: a\ntrailing`),
   - continuation followed by a new term (`Term\n: a\nOrange\n: b`),
   - a line followed by a colon line inside ordinary prose
     (`intro\nnot a term\n: def`) — should follow the chosen lookahead rule
     and be documented,
   - trailing text after a definition list with a blank line (paragraph
     split) remains a separate paragraph.

**Acceptance:** each term gets its own `<dt>`; no definition body steals the
next term; no unexpected data loss.

---

### BF-05 — Link attributes must survive inline text-node splitting

**Severity:** Critical (feature unreliable)
**Files:** `src/link_attributes.rs:37–95` (`next_sibling` assumption at :51)

**Root cause**

The transformer assumes the text immediately after a link is one `Text` node
containing the complete `{...}` list. Inline parsers split text at emphasis
delimiters (`_`, `*`), emoji colons (`:`), code-span backticks, etc. Only the
first fragment starts with `{`, and `parse_attributes` either fails or stops
early. The rest of the list remains literal markup.

**Implementation**

1. Introduce a helper that collects the **run of adjacent `Text` siblings**
   following the link/image:
   ```rust
   fn collect_adjacent_text(arena: &Arena, first: NodeRef) -> Vec<NodeRef>;
   ```
2. If all nodes in the run have `Text::index()` and their source segments are
   contiguous, build a reader over `Segment::new(first.start, last.stop)` and
   parse the attribute list once.
3. If any node is synthesized (`Text::String`, no index), skip the run
   (documented limitation) rather than partially applying attributes.
4. After parsing:
   - Compute the consumed byte count from the reader position.
   - Remove fully consumed text nodes.
   - For a partially consumed node, replace its value with the remainder
     only if it has a valid source index; otherwise leave it unchanged.
   - Apply all attributes atomically; if parsing fails, change nothing.
5. Preserve the existing behavior for `{}` (decide whether to keep consuming
   empty lists; document).
6. Tests:
   - `{#id .cls target="_blank"}` (quoted value),
   - `{.my_class}`, `{#my_id}`, `{data_x=1}` (underscores),
   - `{title="a:b"}`, `{title="https://x/y"}` (colons),
   - `{title="a*b"}`, `{title="a`b"}` (emphasis/code splits),
   - a list split across multiple text nodes followed by visible text,
   - link inside emphasis, image inside a link, reference links,
   - attributes disabled (`engines=Engines.without(["link_attributes"])`),
   - the same cases with `emoji` disabled and enabled.

**Acceptance:** the full attribute list is applied whenever it is contiguous
in source, regardless of inline-parser segmentation.

---

### BF-06 — Security: no duplicate `href`/`src`; gate style attributes

**Severity:** High (security hardening)
**Files:** `src/link_attributes.rs`, `src/fenced_div.rs`,
`src/renderer/html.rs:733–746`, `build.rs:128–150` (`LINK_ATTRS`),
`src/renderer/html.rs:73–155` (`AttributeFilters`)

**Finding A — duplicate URL attributes**

The built-in link renderer writes the sanitized `href` first, then all node
attributes. `href` is in `LINK_ATTRS`, so `{href=...}` produces a second,
unsanitized `href` even with `allows_unsafe=False`:

```html
<a href="https://safe" href="javascript:alert(1)">a</a>
```

**Fix**

- Drop `href` from link attribute lists and `src` from image attribute lists
  in the `link_attributes` transformer (or filter them in
  `write_attributes!` for those node kinds).
- Add a URL-safety check for any remaining URL-bearing attribute
  (`href`, `src`, `action`, `formaction`, `poster`, `cite`, `data`, etc.) when
  `allows_unsafe` is false.
- Add tests:
  ```python
  out = m.markdown_to_html('[a](https://safe){href=javascript&#58;alert(1)}')
  assert "javascript:" not in out
  assert out.count("href=") == 1
  ```

**Finding B — `style` is allowed by default**

`DEFAULT_ATTRS` includes `style`, so default-on `link-attributes` and
`fenced-div` accept `{style=...}` even with `allows_unsafe=False`. For
untrusted Markdown this is CSS injection/UI redressing.

**Fix options (choose one, document it):**

1. **Recommended:** treat `style` as unsafe for these extension attributes:
   filter it out unless `allows_unsafe` is true. Keep existing heading
   attribute behavior unchanged unless the project decides otherwise.
2. Remove `style` from the default global attribute filter (broader breaking
   change; not recommended for a patch).
3. Keep it and document that `allows_unsafe` does not sanitize style, with a
   security note in README/QUICKREF. This is the least safe option.

Add tests for links, images, and fenced divs, both with
`allows_unsafe=False` and `True`.

**Acceptance:** no duplicate URL attributes; no style attribute injection
from default-on extensions with `allows_unsafe=False`.

---

### BF-07 — Fix feature-gated tests and the `Engines` doctest

**Severity:** Critical (CI breakage)
**Files:** `tests/engines.rs:28–63`, `src/extensions.rs:9`,
`.github/workflows/test.yml`

**Root cause**

`tests/engines.rs` is compiled in every test configuration but asserts emoji,
definition lists, fenced divs, math and KaTeX are available. Minimal-feature
CI builds fail. The `Engines` doctest asserts `is_enabled(Engine::Emoji)` and
fails when the emoji feature is absent.

**Implementation**

1. Gate feature-dependent tests:
   ```rust
   #[cfg(feature = "emoji")]
   #[test]
   fn only_enables_selected_engine() { ... }

   #[cfg(all(feature = "emoji", feature = "math", feature = "definition-list",
             feature = "fenced-div", feature = "link-attributes"))]
   #[test]
   fn mixed_features_render_together() { ... }
   ```
   Keep at least one always-on test that uses `Engines::none()` and
   `Engines::all().get(...)` so the module still compiles and runs in minimal
   builds.
2. Rewrite the `src/extensions.rs` doctest to assert `get()` (flag semantics)
   and gate `is_enabled()` assertions with `#[cfg(feature = "...")]`.
3. Add CI steps:
   ```yaml
   - name: Run Tests (minimal std)
     run: cargo test --no-default-features --features std
   - name: Run Doctests (minimal std)
     run: cargo test --no-default-features --features std --doc
   ```
   Keep the existing no-std test; document the Windows-only `libc::write`
   failure as a local limitation (CI runs Linux).
4. Add a CI assertion that `tests/engines.rs` is skipped, not silently
   ignored, in minimal builds (e.g. a compile-and-run check).

**Acceptance:** `cargo test --no-default-features --features std` and
`--doc` pass; the no-std CI job passes on Linux; default-feature coverage is
unchanged.

---

## 4. High-priority semantic/API fixes

### BF-08 — Definition lists must not eat emoji shortcodes

**Files:** `src/definition_list.rs:112–121` (`definition_colon`)

**Fix**

Require whitespace/tab/EOL after the colon (D5):

```rust
fn definition_colon(line: &[u8]) -> Option<usize> {
    // up to three leading spaces, then ':',
    // then space/tab/CR/LF or end of line
}
```

Tests: `:smile:`, `:joy:`, `::smile::`, `: http://x`, `:\tfoo`, bare `:`,
`:def` (must remain a paragraph, documented), term + definition with 1–3
leading spaces.

**Acceptance:** emoji shortcodes at line start render as emoji; only explicit
`Term` + `: definition` pairs become `<dl>`.

---

### BF-09 — Unify default engine sets

**Files:** `src/renderer/html.rs:41,54`, `src/parser/mod.rs:584`,
`src/extensions.rs:97`, `mordant-py/src/lib.rs:69,102`

**Fix**

1. Change `html::Options::default()` to `engines: Engines::default()`
   (`all_without_highlighter`).
2. Change `ParseConfig::default` / `RenderConfig::default` to
   `Engines::default()`.
3. Update doc comments on `engines` fields and `Engines` to say
   “`Engines::default()`”, not `all()`.
4. Add a Rust test using `markdown_to_html_string_with_options` with default
   options:
   ```rust
   // default: plain <pre><code>
   // Engines::all(): highlighted <pre style=...>
   ```
5. Add a Python test asserting the default and `Engines.all()` behavior.

**Acceptance:** one documented default; Rust and Python produce the same
default HTML for code blocks.

---

### BF-10 — `Engines.without` should keep the default highlighter state

**Files:** `src/extensions.rs:178–185`, `mordant-py/src/options.rs:313`

**Fix (per D2)**

```rust
pub fn without(engines: &[Engine]) -> Self {
    let mut result = Self::default(); // not all()
    ...
}
```

Update docs to say “all default engines except the named ones”; add
`Engines::all().without([...])` to the README/QUICKREF for the all-base case.
Python `Engines.without` mirrors the same behavior. Add a test:

```python
assert not m.Engines.without(["emoji"]).highlighter
assert m.Engines.all().without(["emoji"]).highlighter
```

**Acceptance:** disabling one engine never silently enables highlighting.

---

### BF-11 — `RenderOptions(highlighter=...)` must imply highlighting

**Files:** `mordant-py/src/lib.rs:429–445`, `docs/QUICKREF.md:1090`

**Fix (per D4)**

1. When `render_opts.highlighter` is `Some` and no explicit `engines=`
   argument was supplied, set `Engine::Highlighter` to true in
   `render_engines`.
2. If an explicit `engines=` was supplied, it wins (do not silently enable).
3. Apply the same rule to `RenderOptions.engines` defaults if BF-15 changes
   them to inherit.
4. Add tests:
   - `RenderOptions(highlighter=HighlighterOptions(theme="Dracula"))` alone
     highlights,
   - with `engines=Engines.none()` it does not,
   - with `engines=Engines.all()` it does.

**Acceptance:** the documented “implies highlighting” behavior is real, and
explicit engine sets still win.

---

### BF-12 — Precedence: explicit engines vs legacy theme kwargs

**Files:** `mordant-py/src/lib.rs:344–352`, `:429–445`

**Fix (per D3)**

- Track whether the caller supplied an explicit engine set (`engines=` or
  `render_opts.engines`).
- If explicit, do not let `theme=` / `highlighting_theme=` turn the
  highlighter on or off; use the legacy values only for theme/mode.
- If not explicit, preserve 0.10 behavior: legacy theme enables highlighting.
- Apply the same precedence to nested `highlighter=`.
- Add tests for all four combinations:
  `engines=none + theme`, `engines=all + theme`,
  `no engines + theme`, `render_opts.engines=none + theme`.

**Acceptance:** explicit capability configuration is authoritative;
legacy kwargs remain usable for value overrides.

---

### BF-13 — Make `linter` and `chunker` engine flags effective

**Files:** `mordant-py/src/lib.rs:516–600`, `mordant-py/src/options.rs`,
`src/chunker.rs:88–105`, `mordant-py/src/chunker.rs:131`, `src/linter.rs`

**Fix (per D10, recommended option A)**

1. Python `lint()`/`fix()`:
   - If `engines` is supplied and `Engine::Linter` is disabled, return no
     diagnostics (`lint`) or the unchanged source (`fix`).
   - Alternatively raise `ValueError` (“linter engine disabled”) — choose one
     and document; returning empty/unchanged is friendlier.
   - Add `engines=` to `lint_many()`/`fix_many()` or document that those use
     defaults.
2. Chunker:
   - Add `MarkdownChunker::with_engines(text, Engines)` (or a builder) in
     core; use the shared parser-extension factory (see BF-19).
   - Python `MarkdownChunker(text, engines=None)`.
   - Gate the diagram extension on `Engine::Diagram`; add definition list,
     fenced div, link attributes, meta, emoji, math, and footnotes so chunk
     boundaries match `parse()`/default rendering.
3. Rust lint APIs (`run_lint*`) remain AST-based; document that the `linter`
   flag is enforced by the caller, not inside the rule engine.
4. Tests:
   - `lint(..., engines=Engines.none()) == []`,
   - `fix(..., engines=Engines.none()).output == source`,
   - `MarkdownChunker(":::\nx\n:::", engines=Engines.none())` vs default,
   - `Engines.only(["chunker"])` behavior.

**Acceptance:** every engine name in `Engines` has an observable effect, or
is removed/documented as non-toggle-able.

---

### BF-14 — In-place mutation of option sub-objects must work or be impossible

**Files:** `mordant-py/src/options.rs:66,138`, `:340–353`

**Root cause**

PyO3 getters on `Copy` fields (`Engines`) and clone-returning getters
(`HighlighterOptions`) hand back copies. `opts.engines.emoji = False` and
`opts.highlighter.theme = "X"` silently do nothing.

**Fix**

1. Store `Py<Engines>` and `Py<HighlighterOptions>` inside `ParseOptions` /
   `RenderOptions`, so the getter returns the same Python object
   (`clone_ref`). Convert to plain Rust values only when building
   `ParseConfig`/`RenderConfig` (requires passing `Python` into the
   conversion helpers).
2. If that refactor is too large for this patch, the minimum is:
   - remove the in-place-mutation illusion by returning a property that cannot
     be mutated (`Engines` without setters) and add
     `opts.set_engine(name, bool)` / `opts.with_engines(...)`, **or**
   - document loudly that assignment is required and add a test that
     in-place mutation is intentionally unsupported.
3. Add tests that mutation through the getter is reflected in behavior.

**Acceptance:** `opts.engines.emoji = False` either works or fails loudly;
no silent no-op remains.

---

### BF-15 — Parse/render engine mismatch must not silently delete content

**Files:** `mordant-py/src/lib.rs:269–334` (`parse_config_from`),
`:344–445` (`markdown_to_html`), `mordant-py/src/options.rs`

**Fix (per D9, recommended)**

1. Represent “not explicitly set” distinctly:
   - `ParseOptions.engines: Option<Engines>`,
   - `RenderOptions.engines: Option<Engines>`.
   Default `None` means “use `Engines::default()`” on both sides.
2. In `markdown_to_html`, compute the effective engine set once:
   - explicit `engines=` argument, else
   - `render_opts.engines` / `parse_opts.engines`, else
   - `Engines::default()`.
   Use the same set for parser and renderer unless the user explicitly
   configured them differently.
3. If the user does configure them differently and the renderer is missing an
   engine that produced AST nodes, either:
   - raise `ValueError` listing the mismatch, or
   - render the missing nodes as literal source (harder).
   Recommend raising a clear error; add a test.
4. Keep `.pyi` and docs in sync; update the examples that pass `engines=`
   separately to both sides.

**Acceptance:** no silent empty `<p></p>` or vanished footnotes/math/emoji;
mismatches are either impossible or diagnosed.

---

## 5. Medium-priority fixes

### BF-16 — `available_engines()` must respect features

**Files:** `src/lib.rs:333–340`

**Fix**

Build a cfg-gated static list:

```rust
const AVAILABLE: &[Engine] = &[
    #[cfg(feature = "meta")] Engine::Meta,
    #[cfg(feature = "emoji")] Engine::Emoji,
    // ...
];
pub fn available_engines() -> &'static [Engine] { AVAILABLE }
```

Add tests under default and `--no-default-features` builds. If a stable
`&'static [Engine]` is inconvenient, change the return type to
`impl Iterator<Item = Engine>` and document the API change.

---

### BF-17 — Validate `HighlighterOptions`

**Files:** `mordant-py/src/options.rs:340–353`,
`mordant-py/src/lib.rs:432–443`, `src/highlighter.rs:312–315`

**Fix**

1. Validate `mode` at construction: accept `"Attribute"` / `"Class"`
   (decide whether to be case-insensitive); raise `ValueError` otherwise.
2. Validate `theme` against `list_themes()` / the loaded theme set when
   constructing or when rendering; raise `ValueError` instead of silently
   falling back to InspiredGitHub.
3. Keep the legacy kwargs’ silent fallback for compatibility only if
   explicitly documented; ideally route them through the same validation and
   mark the fallback deprecated.
4. Add tests for invalid theme/mode and for both valid modes.

---

### BF-18 — Merge nested diagram options with `theme=`

**Files:** `mordant-py/src/lib.rs:364–395`

**Fix**

- When `theme` is set and `RenderOptions.diagram` is present, clone the nested
  options and fill `theme` only if it is `None`; never construct a new options
  object that drops `mermaid_url`, `diagram_type`, mode, etc.
- Apply legacy `diagram_render_opts` first, then nested, then the convenience
  theme for unset fields (explicit values win).
- Fix legacy `highlighting_mode` so an explicit `"Attribute"` can override a
  nested `mode="Class"` (or document that nested wins).
- Add tests combining `RenderOptions(diagram=...)` + `theme=` and
  `highlighting_mode=` + `HighlighterOptions(mode=...)`.

---

### BF-19 — Chunker: engines and default syntax

**Files:** `src/chunker.rs:88–105`, `mordant-py/src/chunker.rs:131`

**Fix**

1. Factor the built-in parser extension chain into a reusable function, e.g.
   `mordant::extensions::parser_extension(engines)` or
   `mordant::parser_extensions(engines)`.
2. Use it in `markdown_to_html_string_with_options`, `MarkdownChunker`, and
   any other consumer, so the engine gate cannot drift.
3. Add `MarkdownChunker::with_engines(text, engines)`; keep `new(text)` as
   `Engines::default()`.
4. Add Python `MarkdownChunker(text, engines=None)` and `.pyi` signature.
5. Decide how to classify top-level `DefinitionList` / `FencedDiv`:
   - `BlockType::Other` with the whole node text, or
   - recurse into `FencedDiv` children and chunk the inner blocks.
   Document the choice; recursion is a larger change, so `Other` is the
   patch-safe default.
6. Add tests for `:::` and `Term\n: def` under default vs `Engines.none()` and
   for diagram-disabled chunking.

---

### BF-20 — Documentation corrections

**Files:** `README.md`, `mordant-py/README.md`, `docs/ARCHITECTURE.md`,
`docs/QUICKREF.md`, `CHANGELOG.md`, `docs/RELEASE_PLAN_0.11.md`,
`src/lib.rs` doc comments, `mordant-py/src/lib.rs` docstrings

**Fix**

1. Replace every “defaults to `Engines::all()`” with
   “defaults to `Engines::default()` / all engines except highlighting”.
2. QUICKREF:
   - remove “`highlighter=None` implies highlighting” unless BF-11 is done,
   - document `Engines.without` base (BF-10),
   - add `DefinitionList`, `DefinitionTerm`, `DefinitionDescription`,
     `FencedDiv` to the node-kind table,
   - document definition-list single-term/single-paragraph limits (BF-22),
   - document fenced-div closing-fence rule (BF-21),
   - note that `href`/`src` are stripped from attribute lists (BF-06).
3. `markdown_to_html` docstring: document `engines`, nested `RenderOptions`,
   `footnote_render_opts`, and the actual `gfm_opts` name.
4. `ARCHITECTURE.md` §7.0: align defaults, fenced-div nesting claim, and
   definition-list capabilities with reality.
5. `RELEASE_PLAN_0.11.md`: correct “multi-paragraph definition lists” and
   “nested fences” claims, or implement them (BF-22/BF-21).
6. Add a 0.11.1/0.11.0 “Fixed” changelog section with the user-visible
   behavior changes from BF-01..BF-15.

---

### BF-21 — Enforce fenced-div fence rules

**Files:** `src/fenced_div.rs:90–174`

**Fix (per D7)**

1. Closing fence: `colon_run >= opening_fence_length` (Pandoc-compatible) or
   exact (current doc claim); choose one and test.
2. Accept optional trailing attributes on the closing fence and ignore them
   (Pandoc behavior), or explicitly reject/document them.
3. Non-matching fence lines must not close the div. After BF-01 they will be
   treated as content; decide whether a line of ≥3 colons inside a div opens
   a nested div (likely yes) and test it.
4. Opening fence: decide whether a space is required before a class list
   (`::: note` vs `:::note`); document and test.
5. Add the tests listed in BF-01, plus attribute values with quotes/colons to
   ensure no interaction with inline parsing (block-level parsing should be
   unaffected).

---

### BF-22 — Definition list block content (scope decision)

**Files:** `src/definition_list.rs`

**Current state**

Definitions are inline-only: continuation lines are appended to a `Block` and
inline-parsed; blank lines split the paragraph and end the list; indented
continuation becomes an indented code block outside the `<dl>`. The release
plan claimed multi-paragraph/tight/loose support.

**Recommendation**

- For this patch: implement the single-paragraph behavior correctly
  (BF-02..BF-04), document the limitation clearly, and remove the
  multi-paragraph claim from the plan/docs.
- For a follow-up (0.12): make `<dd>` a container block with child
  paragraphs/lists/code, introduce block parsing for definition content, and
  support loose lists. This is a larger AST/parser change and should not be
  rushed into a patch release.
- Add “known limitation” tests so behavior is explicit rather than accidental.

---

## 6. Low-priority cleanup

### BF-23 — Build, warnings, lockfiles, CI

1. **Lockfile churn:** review `Cargo.lock` / `mordant-py/Cargo.lock` changes
   beyond `katex-rs` (serde, thiserror, bon, darling, phf 0.14, prettyplease,
   syn 3.0.10). If any are not required by katex-rs 0.3.0, revert with a
   targeted `cargo update -p katex-rs` and regenerate both lockfiles.
2. **Minimal-feature warnings:** in `src/lib.rs`, the extension closures leave
   `p`, `r`, `renderer_engines`, and the `HtmlRendererExtension` import unused
   when no engine features are enabled. Gate them with `cfg` or use
   `let _ = ...`; keep `cargo check --no-default-features --features std`
   warning-free for the new code.
3. **Clippy:** fix `clone_on_copy` in `definition_list.rs:348–350` and
   `default_constructed_unit_structs` in `src/lib.rs:306,313`; keep new modules
   clippy-clean.
4. **CI redundancy:** the “all engines” Rust job passes the default feature
   set explicitly. Keep it if it documents intent, but add the
   `--no-default-features --features std` jobs from BF-07, which actually
   cover a different configuration.
5. **`ParseConfig::default` / `RenderConfig::default`:** align with
   `Engines::default()` (BF-09); `RenderConfig::default` is currently unused,
   so either use it or remove it to avoid drift.
6. **Version/release:** if 0.11.0 is unpublished, land fixes on
   `dev0.11.0` and keep the version. If it is already published, create
   `dev0.11.1`, bump `Cargo.toml`, `mordant-py/Cargo.toml`,
   `mordant-py/pyproject.toml`, both lockfiles, README/docs/Pages, and add a
   lockstep changelog entry.

---

## 7. Implementation order and milestones

### Milestone 0 — failing regression tests (0.5 day)

Write the tests from BF-01..BF-07 first against the current code; confirm they
fail. This protects the fixes from regressing and documents the intended
behavior. Do not merge fixes without the tests.

### Milestone 1 — parser correctness (1–2 days)

1. BF-01 fenced div `cont` rewrite.
2. BF-02 definition list line terminators.
3. BF-03 term-at-first-line rule.
4. BF-04 continuation/term lookahead.
5. BF-08 colon whitespace rule.
6. BF-05 link-attribute text-run merging.
7. BF-21 fence rules (if D7 is settled).

Run the Rust unit suites after each module; add Python cases in
`mordant-py/tests/test_engines.py` (or new `test_syntax_extensions.py`).

### Milestone 2 — CI and minimal builds (0.5 day)

BF-07: feature gates, doctest, minimal-feature CI jobs. Verify
`cargo test --no-default-features --features std` and `--doc` pass.

### Milestone 3 — engine/API semantics (1–2 days)

BF-09 defaults, BF-10 `without`, BF-11 nested highlighter, BF-12 precedence,
BF-13 linter/chunker flags, BF-14 mutable option objects, BF-15 inheritance,
BF-16 `available_engines`, BF-17 validation, BF-18 diagram merge, BF-19 chunker.

Update `.pyi` and docstrings in the same commits.

### Milestone 4 — security hardening (0.5 day)

BF-06 URL/style filtering plus tests. Decide whether `style` is filtered
globally or only for extension attributes; document the policy.

### Milestone 5 — docs and release (0.5–1 day)

BF-20 docs, BF-22 limitation notes, BF-23 cleanup, changelog, versioning,
Pages site if a patch release.

---

## 8. Test matrix

### Rust

```text
cargo fmt --all --check
cargo clippy --all-targets --features meta,emoji,footnotes,linter,diagram,chunker,math,highlighter,definition-list,fenced-div,link-attributes -- -D warnings   # new code only
cargo test
cargo test --features meta,emoji,footnotes,linter,diagram,chunker,math,highlighter,definition-list,fenced-div,link-attributes
cargo test --no-default-features --features std
cargo test --no-default-features --features std --doc
cargo test --no-default-features --features no-std-unix-debug,html-entities   # Linux CI
cargo test --no-default-features --features std,definition-list
cargo test --no-default-features --features std,fenced-div
cargo test --no-default-features --features std,link-attributes
```

### Python

```text
maturin build --release --manifest-path mordant-py/Cargo.toml
python -m pytest -q mordant-py/tests
python -m pytest -q mordant-py/tests/test_engines.py
python -m pytest -q mordant-py/tests/test_math.py
```

Add focused tests:

- `test_definition_list.py`: single/multi-line content, two terms, multiple
  definitions, preamble preservation, emoji colon case, CRLF, blockquote/list
  containers, disabled engine.
- `test_fenced_div.py`: body nesting, nested divs, fence lengths, attributes,
  blockquote/list containers, unclosed, disabled engine.
- `test_link_attributes.py`: quoted values, underscores, colons, emphasis,
  images, duplicate `href`, style policy, disabled engine.
- `test_engines.py`: defaults, `without` highlighter state, nested highlighter
  implication, legacy precedence, linter/chunker flags, getter mutation,
  parse/render inheritance, `KATEX_VERSION`.

### Expected regression assertions

```python
# BF-01
assert m.markdown_to_html("::: note\nHello.\n:::\n") == \
    '<div class="note">\n<p>Hello.</p>\n</div>\n'

# BF-02
assert "<dd>\na\nb\n</dd>" in m.markdown_to_html("Term\n: a\nb\n")

# BF-03
assert "intro" in m.markdown_to_html("intro\nTerm\n: def\n")

# BF-04
out = m.markdown_to_html("Apple\n: A fruit.\nOrange\n: Citrus fruit.\n")
assert out.count("<dt>") == 2

# BF-05
assert 'target="_blank"' in m.markdown_to_html('[a](x){#id .cls target="_blank"}')

# BF-08
assert "😄" in m.markdown_to_html("Hello\n:smile: world\n")

# BF-09
assert "style=" not in m.markdown_to_html("```python\nx=1\n```")

# BF-10
assert not m.Engines.without(["emoji"]).highlighter

# BF-11
assert "style=" in m.markdown_to_html(
    "```python\nx=1\n```",
    render_opts=m.RenderOptions(highlighter=m.HighlighterOptions(theme="Dracula")),
)

# BF-14
opts = m.ParseOptions()
opts.engines.emoji = False
assert not opts.engines.emoji
```

---

## 9. Risks and compatibility notes

| Risk | Mitigation |
|---|---|
| Requiring whitespace after `:` changes definition-list syntax for users who wrote `:def`. | Document as a deliberate fix for emoji collision; add a migration note. Most Markdown Extra examples use a space. |
| Changing `Engines::without` to base on defaults changes output for callers who expected `all()`. | Document in CHANGELOG; provide `Engines.all().without(...)`; add tests for both. |
| Making explicit `engines=` win over legacy `theme=` changes behavior. | Document precedence; keep legacy working when no explicit engine set is supplied. |
| Dropping `href`/`src` from attribute lists removes a feature. | Those attributes cannot be safely overridden; document. Users who need custom URLs should use the destination syntax. |
| Filtering `style` may break users who relied on it. | Gate on `allows_unsafe`, document clearly, and mention `AttributeFilters` for custom policies. |
| Inheritance of parse engines into the renderer changes `RenderOptions(engines=None)` behavior. | Default is now the inherited set; explicit `engines=` still overrides. Add tests for all combinations. |
| Full multi-paragraph definition-list support is not in this patch. | Document the limitation; track as a 0.12 feature. Remove the claim from the 0.11 plan. |
| Fenced-div closing-fence rule may differ from other Markdown tools. | Choose Pandoc-compatible “at least as long” and document; test non-matching fences as content. |

---

## 10. Definition of done

- [x] BF-01..BF-07 fixed with regression tests that fail before the fix and
      pass after.
- [x] BF-08..BF-15 fixed (BF-22 limitation documented) with tests.
- [x] BF-16..BF-19 fixed with tests.
- [x] BF-20 docs updated; BF-23 minimal-feature warnings and CI jobs fixed
      (lockfile churn intentionally retained from the katex-rs upgrade).
- [x] `cargo test`, all-engine tests, minimal-std tests and minimal doctests
      pass locally. The Windows no-std debug path was fixed by gating
      `libc::write` to Unix; `cargo check --no-default-features --features
      no-std-unix-debug,html-entities` passes on Windows, and the Linux no-std
      job remains in CI.
- [x] Python wheel builds and the full pytest suite passes (1288 tests).
- [x] No silent content loss in any engine combination (covered by tests).
- [x] Changelog has a “Fixed” section describing the behavior changes.
- [x] Appendix B findings GL-01…GL-18 and GL-20 are applied; GL-19 is accepted
      as a PyO3 binding deviation.
- [x] Version stays 0.11.0; it has not been published yet.

> **Review amendment:** Appendix B records a post-implementation
> Rust-guidelines review with additional follow-ups (GL-01…GL-20). Those are
> now applied (GL-19 is an accepted deviation with no code change); see §B.9
> for status.

---

## Appendix A — Syntect newline/render audit (no defect found)

**Question:** does the syntect integration add trailing newlines or blank
lines between code lines?

**Conclusion:** no. The highlighter preserves the code block text byte for
byte (modulo HTML escaping); the plain and highlighted renderers produce the
same inner `<code>` text.

### What was checked

- `LinesWithEndings::from(code)` + `HighlightLines::highlight_line`
  (`src/highlighter.rs:259–278`). Syntect’s `LinesWithEndings` keeps the
  terminator with the line and does **not** emit an extra empty line for a
  string that ends with `\n`.
- `ClassedHTMLGenerator::parse_html_for_line_which_includes_newline`
  (`src/highlighter.rs:280–289`). This is the non-deprecated API; syntect’s
  deprecated `parse_html_for_line` is the one that adds a newline, and it is
  not used.
- `CodeBlock::value().iter(source)` assembly
  (`src/highlighter.rs:650–653`) is identical to the plain renderer
  (`src/renderer/html.rs:378–380`).
- Both `highlight_code` and `highlight_spans` use the same span builders.

### Empirical checks

1. Compared plain vs highlighted inner `<code>` text for:
   single/multi-line code, blank lines between code lines, trailing blank
   lines, empty code blocks, CRLF, tabs, indented code blocks, fenced code
   inside lists and blockquotes, and `~~~` fences: identical every time.
2. Randomized matrix (300 generated snippets × themes × Attribute/Class
   modes × wrapped/bare) comparing `strip_tags(highlight(code))` to the
   input: 0 mismatches.
3. Randomized Markdown fenced-code matrix (200 documents × themes × modes)
   comparing plain vs highlighted `<code>` text: 0 mismatches.
4. `benches/fixtures/data.md`: 705 code blocks, plain vs highlighted inner
   text identical for all 705.

### Intentional newline behavior (not a bug)

- A code block’s own trailing newline remains inside `<code>`; both plain and
  highlighted renderers then append `</code></pre>\n`. This is consistent
  and matches CommonMark output.
- `Highlighter.highlight(..., bare=True)` returns exactly the input’s line
  endings (including a final newline if supplied); it does not add one.
- Empty code blocks produce an empty `<code>` element, not a stray newline.

### Guardrail

Keep `parse_html_for_line_which_includes_newline` and
`LinesWithEndings`; do not switch to the deprecated `parse_html_for_line`
API, which adds a newline after every line and would produce exactly the
reported symptom. Add the matrix checks above as a regression test if the
highlighter is refactored.

---

## Appendix B — Rust-guidelines review amendment

> **Status:** review complete and remediation applied on `dev0.11.0`
> (still uncommitted). The initial review pass made no source changes; the
> follow-up pass applied GL-01…GL-18 and GL-20. GL-19 is an accepted PyO3
> binding deviation and intentionally has no code change. All items marked
> “blocks 0.11.0” in §B.0 are resolved.
>
> **Guideline source:** Microsoft Pragmatic Rust Guidelines
> (`https://microsoft.github.io/rust-guidelines/agents/all.txt`, MIT).
> Sections loaded for this review: **Libraries / UX**, **Correctness**,
> **Documentation**, **Libraries / Building**, and **Performance**.
>
> **Scope:** the bug-fix work described in §3–§6 and implemented on
> `dev0.11.0`: `src/extensions.rs`, `src/lib.rs`, `src/parser/mod.rs`,
> `src/renderer/html.rs`, `src/definition_list.rs`, `src/fenced_div.rs`,
> `src/link_attributes.rs`, `src/chunker.rs`, `mordant-py/src/*`, and the new
> regression tests.

### B.0 Summary

| ID | Severity | Blocks 0.11.0? | Area | Guideline(s) | Finding |
|---|---|---|---|---|---|
| GL-01 | High (docs correctness) | Yes | Public docs | `M-CANONICAL-DOCS`, `M-FIRST-DOC-SENTENCE` | The `markdown_to_html_string_with_options` doc block is attached to `parser_extensions`; the intended function is undocumented. |
| GL-02 | Medium | Yes | Internal docs | `M-CANONICAL-DOCS`, `M-FIRST-DOC-SENTENCE` | `collect_attribute_nodes`’s doc is attached to `type AttributeRemainder`; two summary sentences, function undocumented. |
| GL-03 | Low | Yes | Hygiene | Dead code / clippy | Dead `HtmlRendererExtension` import hidden with `#[allow(unused_imports)]`. |
| GL-04 | Medium | Yes | Public API docs | `M-CANONICAL-DOCS` | `Engines` has eleven public `bool` fields with no field docs. |
| GL-05 | High (future breaking change) | Yes | API evolution | `M-FEATURES-ADDITIVE` | `Engine` and `Engines` are not `#[non_exhaustive]`; adding an engine later breaks downstream matches/literals. |
| GL-06 | Low | No | API discoverability | `M-BALANCED-MODULES`, `M-DOC-INLINE` | `Engine`/`Engines` are not re-exported at the crate root. |
| GL-07 | Medium | Yes | API surface | `M-BALANCED-MODULES` | `parser_extensions` is public but used only inside the crate; no docs as a supported API. |
| GL-08 | Medium | Yes | Public docs | `M-CANONICAL-DOCS` | `allows_unsafe` under-describes extension attribute policy; `render_attributes_safe` lacks `# Errors`; `render_attributes` silently bypasses the policy. |
| GL-09 | Medium (defense in depth) | No | Security hardening | Correctness/security spirit | `is_dangerous_url` does not trim leading whitespace/C0 or decode character references. |
| GL-10 | Medium | Yes | Security docs | `M-CANONICAL-DOCS` | QUICKREF says `style` requires `allows_unsafe=True` without limiting the claim to the new extensions; the old attributes extension still emits it. |
| GL-11 | Medium | Yes (BF-23) | `unsafe` / no-std | `M-UNSAFE`, `M-OOBE` | Two pre-existing `unsafe` blocks have no safety comments; the no-std debug path does not compile on Windows. |
| GL-12 | Low | No | Python binding | `M-PANIC-IS-STOP` | `list.append(s).unwrap()` in a PyO3 method panics instead of returning `PyResult`. |
| GL-13 | Low | No | Docs | `M-CANONICAL-DOCS` | Broken intra-doc link `TextSource::str` in `src/chunker.rs`. |
| GL-14 | Low | No | Feature model | `M-FEATURES-ADDITIVE` | Pre-existing `no-std` feature and `std`-requiring new features form a hybrid matrix. |
| GL-15 | Low | Yes | Doctests | Test hygiene | The new example is gated on `emoji` **and** `footnotes` though only `footnotes` is used. |
| GL-16 | Info | No | Consistency | — | `available_engines()` duplicates `Engine::ALL`; the lists can drift. |
| GL-17 | Low | No | Performance | `M-MEM-REUSE` (closest) | `unsafe_extension_attribute` allocates a lowercase `String` for every attribute. |
| GL-18 | Info | No | Renderer design | — | Fenced divs and definition lists use `AttributeFilters::paragraph()` because there is no `div`/`dl` filter. |
| GL-19 | Info | No | Python API | `M-INIT-CASCADED` (accepted deviation) | `Engines.__new__` has eleven parameters; acceptable for a PyO3 keyword API. |
| GL-20 | Low | Yes | User docs | `M-CANONICAL-DOCS` | README/CHANGELOG wording: “every engine … on by default” conflicts with highlighting being opt-in; `available_engines()` is Rust-only. |

**Legend:** “Blocks 0.11.0” means the item must be fixed before tagging
`0.11.0`; the release has not been published, so no patch-release fallback is
required yet.

### B.1 Method and evidence

The review combined manual inspection with tooling:

1. Read the complete implementations of the new modules and the changed
   public surfaces, not only the diffs.
2. Ran `cargo clippy --lib` with the all-engine feature list
   (`meta,emoji,footnotes,linter,diagram,chunker,math,highlighter,definition-list,fenced-div,link-attributes`).
   Result: **no warnings in the changed files**; the remaining warnings are
   pre-existing and outside this change set.
3. Ran `cargo doc --no-deps`. Result: ten warnings. All except GL-13 are
   pre-existing diagram-module HTML warnings; GL-13 is the
   `TextSource::str` link. The new public items introduced no new broken
   links.
4. Ran targeted probes against the built wheel (maturin) to verify the
   documented security behavior, including:
   - `::: {onclick="alert(1)"}` and `::: {style="color:red"}` → both dropped;
   - `[a](https://safe){HREF="javascript:alert(1)"}` → dropped;
   - `# hi {style="color:red"}` with `ParseOptions(attributes=True)` and
     `allows_unsafe=False` → **`style` is still emitted** (see GL-10).
5. Checked feature combinations:
   - `cargo check --no-default-features --features no-std,definition-list`
   - `cargo check --no-default-features --features no-std,fenced-div`
   - `cargo check --no-default-features --features no-std,link-attributes`
   All three pass; `--no-default-features` alone fails on `memchr`
   (pre-existing and documented).
6. Re-ran the release gates at review time: `cargo test`, the all-engine
   suite, minimal `std` suite and doctests, and
   `python -m pytest -q mordant-py/tests` (1287 passed).
7. After applying the amendment, re-ran the whole §B.8 matrix: default and
   all-engine Rust suites, minimal `std` tests/doctests (including
   `std,footnotes` doctests), no-std and no-std-debug checks, clippy, rustdoc,
   `maturin develop --release`, and the full Python suite (**1288 passed**).
   `cargo doc --no-deps` now reports only the pre-existing diagram HTML
   warnings and the redundant `homepage` manifest warning.

### B.2 Blocking documentation defects

#### GL-01 — `markdown_to_html_string_with_options` docs are attached to `parser_extensions`

- **Guidelines:** `M-CANONICAL-DOCS` (“Public library items must contain the
  canonical doc sections. The summary sentence must always be present.”);
  `M-FIRST-DOC-SENTENCE` (“the first sentence should not exceed 15 words” and
  is the summary).
- **Location:** `src/lib.rs:184–221` (doc block), `src/lib.rs:222`
  (`pub fn parser_extensions`), `src/lib.rs:267`
  (`pub fn markdown_to_html_string_with_options`).
- **Current state:** the block starts with
  “Converts Markdown to HTML using the built-in engines selected by the
  `engines` field…” and ends with the doctest. The next line in the same
  comment is `/// Returns the built-in parser extensions for the given engine
  set.` and the item that follows is `parser_extensions`. Consequences:
  - `parser_extensions` has **two** summary sentences; the first is false for
    that item.
  - `markdown_to_html_string_with_options` has **no documentation at all**.
  - The rustdoc summary for the new public entry point is missing on
    docs.rs, and the example is presented under the wrong function.
- **Why it matters:** this is the primary new public entry point for the 0.11
  options API. A wrong/missing summary is a documentation correctness bug, not
  a cosmetic nit, and it undermines the `M-CANONICAL-DOCS` contract for a
  library crate.
- **Required change:**
  1. Move the paragraph
     `Returns the built-in parser extensions … cannot drift.` above
     `pub fn parser_extensions`.
  2. Move the complete “Converts Markdown …” block, including the example,
     immediately above `pub fn markdown_to_html_string_with_options`.
  3. Rename `# Example` to `# Examples` while moving it.
  4. Add a canonical `# Errors` section to
     `markdown_to_html_string_with_options`, for example:
     ```rust
     /// # Errors
     ///
     /// Returns an error if the underlying writer or a renderer callback
     /// returns an error.
     ```
     (Keep the wording aligned with the actual `Result` behavior; parser
     extensions can report errors, so do not promise that this function is
     infallible for all inputs.)
  5. Give `parser_extensions` its own `# Examples` if it remains public; see
     GL-07.
- **Acceptance:**
  - `cargo doc --no-deps` shows the “Converts Markdown…” summary under
    `markdown_to_html_string_with_options` and the “Returns the built-in
    parser extensions…” summary under `parser_extensions`.
  - `cargo test --doc` passes under both full features and
    `--no-default-features --features std`.
  - No duplicated summary sentence remains in either doc block.

#### GL-02 — `collect_attribute_nodes` doc is swallowed by a type alias

- **Guidelines:** `M-CANONICAL-DOCS`, `M-FIRST-DOC-SENTENCE`.
- **Location:** `src/link_attributes.rs:36–47`.
- **Current state:**
  ```rust
  /// Collects the run of following siblings whose source positions lie inside
  /// `[start, end)`. …
  /// caller leaves the AST untouched.
  /// A text node that was only partially consumed, paired with the source offset
  /// where its remaining content starts.
  type AttributeRemainder = (NodeRef, usize);
  type CollectedAttributeNodes = (Vec<NodeRef>, Option<AttributeRemainder>);

  fn collect_attribute_nodes(…) { … }
  ```
  The function documentation is attached to `AttributeRemainder`; the alias
  therefore has two unrelated summary paragraphs, `CollectedAttributeNodes`
  has no documentation, and the function has none.
- **Why it matters:** private docs are not covered by the public-API
  guideline, but this block is now actively misleading during maintenance and
  was introduced by the same edit that fixed the inline-split bug. The
  function is the core of BF-05; future changes need its contract
  (`None` → AST untouched).
- **Required change:** split into three comments:
  ```rust
  /// A text node that was only partially consumed, paired with the source
  /// offset where its remaining content starts.
  type AttributeRemainder = (NodeRef, usize);

  /// Nodes consumed by an attribute list, plus an optional trailing text node
  /// whose remaining content starts at the given source offset.
  type CollectedAttributeNodes = (Vec<NodeRef>, Option<AttributeRemainder>);

  /// Collects the run of following siblings whose source positions lie inside
  /// `[start, end)`.
  ///
  /// Inline parsers split attribute lists into several text nodes (at `_`,
  /// `:`, `*`, backticks, entities, ...); the list must be parsed from the
  /// original source span rather than from the first text node.
  ///
  /// Returns `None` if any affected node has no source position, or if a node
  /// straddles `end` without being a source-backed text node. In that case the
  /// caller leaves the AST untouched.
  fn collect_attribute_nodes(…) { … }
  ```
- **Acceptance:** `cargo doc --document-private-items` (or a plain source
  read) shows one summary per item; no behavior change.

#### GL-03 — dead import hidden by `#[allow(unused_imports)]`

- **Guideline:** general hygiene; clippy flags unused imports when the allow
  is absent.
- **Location:** `src/lib.rs:70–71`.
- **Current state:**
  ```rust
  #[allow(unused_imports)]
  use renderer::html::RendererExtension as HtmlRendererExtension;
  ```
  `grep -rn HtmlRendererExtension src/` returns only this line. The alias is
  never referenced.
- **Why it matters:** suppressing a lint hides real drift. The same pattern
  appears in `parser_extensions`/the renderer callback as
  `#[allow(unused_variables)] let _ = &p;` and `let _ = &engines;`; these are
  no-op guards that make the closure body harder to read.
- **Required change:**
  1. Delete the import and its `#[allow]`.
  2. Replace the two `let _ =` guards with a single cfg-aware use, e.g.:
     ```rust
     #[cfg(not(any(
         feature = "meta",
         feature = "emoji",
         feature = "footnotes",
         feature = "diagram",
         feature = "math",
         feature = "definition-list",
         feature = "fenced-div",
         feature = "link-attributes"
     )))]
     let _ = (p, engines);
     ```
     or leave a short comment explaining why the binding is intentionally
     unused when no engine feature is enabled. Do the same for the renderer
     callback.
- **Acceptance:** `cargo clippy` produces no unused-import or
  unused-variable warnings in `src/lib.rs`; no `#[allow(unused_*)]` remains
  solely to hide dead code.

### B.3 Public API and UX

#### GL-04 — `Engines` public fields are undocumented

- **Guideline:** `M-CANONICAL-DOCS` (“Public library items must contain the
  canonical doc sections. The summary sentence must always be present.”).
- **Location:** `src/extensions.rs:104–115`.
- **Current state:** `pub struct Engines` has a module-level explanation and
  a struct-level summary, but the eleven `pub` fields have no docs. Rustdoc
  renders them as bare `pub meta: bool`, etc.
- **Why it matters:** the fields are the primary Rust construction/mutation
  API for the new feature, and their names (`linter`, `chunker`) do not map
  obviously to parse/render behavior. Without field docs, users cannot tell
  which side an engine affects or that a flag is inert when the cargo feature
  is absent.
- **Required change:** add one-line docs to every field, for example:
  ```rust
  /// Enables YAML front matter (`meta` feature).
  pub meta: bool,
  /// Enables `:shortcode:` emoji parsing (`emoji` feature).
  pub emoji: bool,
  /// Enables PHP Markdown Extra footnotes (`footnotes` feature).
  pub footnotes: bool,
  /// Enables `lint()` / `fix()`; has no parser or renderer side effects
  /// (`linter` feature).
  pub linter: bool,
  /// Enables Mermaid diagram parsing and rendering (`diagram` feature).
  pub diagram: bool,
  /// Enables `MarkdownChunker` engine awareness (`chunker` feature).
  pub chunker: bool,
  /// Enables KaTeX math (`math` feature).
  pub math: bool,
  /// Enables syntax highlighting. Off in `Engines::default()`.
  pub highlighter: bool,
  /// Enables definition lists (`definition-list` feature).
  pub definition_list: bool,
  /// Enables fenced divs (`fenced-div` feature).
  pub fenced_div: bool,
  /// Enables link/image attribute lists (`link-attributes` feature).
  pub link_attributes: bool,
  ```
  Adjust the `linter`/`chunker` wording to match the implemented D10
  semantics exactly.
- **Acceptance:** `cargo doc` displays field docs; optionally enable
  `#![warn(missing_docs)]` locally to confirm no public field is missed.

#### GL-05 — `Engine` and `Engines` should be `#[non_exhaustive]`

- **Guideline:** `M-FEATURES-ADDITIVE` (“Adding enum variants is fine if
  these enums are `#[non_exhaustive]`”). The same reasoning applies to public
  struct fields: adding a field breaks exhaustive struct literals and
  patterns in downstream crates.
- **Location:** `src/extensions.rs:29` (`pub enum Engine`),
  `src/extensions.rs:104` (`pub struct Engines`).
- **Current state:** both are exhaustive. A future `Engine::Typst` (for
  example) would be a breaking change for every downstream `match`, and a
  future field would break `Engines { … }` literals.
- **Why it matters:** 0.11 is the introduction of this API, so marking it now
  costs nothing semantically; postponing it converts an additive change into
  a semver break. The rest of the crate already uses `#[non_exhaustive]`
  (`src/ast.rs` has several).
- **Required change:**
  1. Add `#[non_exhaustive]` to `Engine` and `Engines`.
  2. Document the construction path: `Engines::default()`, `all()`, `none()`,
     `only()`, `without()`, `set()`. Add a sentence to the `Engines` doc:
     “This type is non-exhaustive; construct it with `default()`, `all()`,
     `none()`, or `only()` and change individual flags with `set()`.”
  3. Check the new tests: internal crate tests may still use struct literals
     (allowed inside the defining crate), but external integration tests in
     `tests/` are separate crates and must not use `Engines { … }`. Today
     `tests/engines.rs` only uses constructors, so this is safe.
  4. Add a CHANGELOG migration note for anyone who already wrote
     `Engines { … }` against the pre-release branch.
- **Acceptance:** external code can still use `Engines::all().without(...)`,
  `Engines::only(...)`, and `opts.engines.emoji = false`; `cargo test` and the
  Python wheel still pass. A downstream `match` on `Engine` now requires a
  wildcard arm, which is the intended forward-compatible behavior.
- **Trade-off to record:** `#[non_exhaustive]` also forbids
  `Engines { emoji: false, ..Default::default() }` outside the crate. If the
  team prefers that convenience over future-proofing, document an explicit
  semver policy instead (“adding an engine is a breaking change”) and keep
  the types exhaustive. Do not leave the policy implicit.

#### GL-06 — `Engine` / `Engines` are not re-exported at the crate root

- **Guidelines:** `M-BALANCED-MODULES` (“Essential items users must find in
  order to use a crate should go into its root”); `M-DOC-INLINE` (mark
  re-exports with `#[doc(inline)]`).
- **Location:** `src/lib.rs:6–11` (module list), `src/lib.rs:64–71`
  (private `use`).
- **Current state:** the only path is `mordant::extensions::Engines`; the
  crate root exposes `parser`, `renderer`, etc. but not the new primary type.
  README and docs already use the module path, so this is an ergonomics
  improvement rather than a broken example.
- **Required change:**
  ```rust
  #[doc(inline)]
  pub use extensions::{Engine, Engines};
  ```
  Remove `Engine`/`Engines` from the private `use crate::{…}` import to avoid
  a duplicate-name conflict. Update README/QUICKREF examples to show the
  shorter path, while keeping the module path documented as valid.
- **Acceptance:** `cargo doc` lists both items in the crate root and under
  `extensions`; no name collisions; `cargo test` passes.

#### GL-07 — `parser_extensions` is public but has no supported-API contract

- **Guideline:** `M-BALANCED-MODULES` (module/API surface should reflect
  supported use); `M-SIMPLE-ABSTRACTIONS` (do not expose internals without a
  reason).
- **Location:** `src/lib.rs:222`.
- **Current state (at review time):** the function was `pub`, but the only
  consumers were `src/chunker.rs:104` and `src/lib.rs:276`. `mordant-py` did
  not call it, and no test imported it. It exists because BF-19 asked to
  factor the engine gate into one place.
- **Why it matters:** every `pub` item is semver surface. Right now the
  function has no doc contract beyond the accidental GL-01 text, so external
  users cannot know whether it is stable, whether extensions compose, or how
  it interacts with GFM.
- **Choice: Option B (applied).** `parser_extensions` is now `pub(crate)`;
  custom pipelines compose parser extensions directly, and the public entry
  point remains `markdown_to_html_string_with_options`. ARCHITECTURE and the
  CHANGELOG record this in the 0.11 API notes.
- **Alternatives considered:**
  - **Option A (if custom pipelines are in scope):** keep it
    public, document it as an advanced API with `# Examples` showing
    `Parser::with_extensions(opts, parser_extensions(engines))`, and state
    that GFM is composed separately (`gfm_ext.and(...)`).

Do not leave a helper public-but-undocumented; the applied Option B satisfies
this by narrowing visibility.
- **Acceptance:** exactly one documented public path exists, and the chosen
  visibility is reflected in ARCHITECTURE/QUICKREF.

#### GL-08 — `allows_unsafe` and the attribute-rendering APIs are under-documented

- **Guideline:** `M-CANONICAL-DOCS` (summary, `# Errors`, applicable
  sections).
- **Locations:** `src/renderer/html.rs:32` (`allows_unsafe`),
  `src/renderer/html.rs:1191` (`render_attributes`),
  `src/renderer/html.rs:1205` (`render_attributes_safe`).
- **Current state:**
  - `allows_unsafe` says only “Allows raw HTML and unsafe links.” BF-06 made
    it also control `style`, `srcdoc`, and dangerous URL values in the new
    extension paths.
  - `render_attributes_safe` has a two-line summary and one behavior
    sentence, but no `# Errors` even though it returns `Result<()>`.
  - `render_attributes` hardcodes the inner `allows_unsafe = true`; its doc
    (“Renders attributes to the given writer”) does not say it bypasses the
    extension safety policy. A caller could reasonably assume the opposite.
- **Required change:**
  1. Update the field doc to:
     “Allows raw HTML, unsafe link destinations, and unsafe attributes
     (`style`, `srcdoc`, and dangerous URL values) emitted by syntax
     extensions.”
  2. Add to `render_attributes`:
     “This entry point preserves the pre-0.11 behavior and does **not**
     apply the extension safety policy; use [`render_attributes_safe`] for
     attributes produced by link-attribute lists, fenced divs, or definition
     lists.”
  3. Add `# Errors` to `render_attributes_safe` (writer failure), and shorten
     its first sentence to under 15 words:
     “Renders extension attributes using the 0.11 safety policy.”
  4. State explicitly that `data-*`/`aria-*` bypass the name filter (this is
     true today) so users are not surprised.
- **Acceptance:** rustdoc shows complete sections; a unit test exercises a
  writer error if one is practical (otherwise document why not).

### B.4 Correctness, security, and bindings

#### GL-09 — `is_dangerous_url` does not normalize before matching

- **Location:** `src/renderer/html.rs:1331`.
- **Current behavior:** the function prefix-matches raw bytes against
  `javascript:`, `vbscript:`, `file:`, and `data:`; the only exception is
  `data:image/{png,gif,jpeg,webp}`.
- **Gap:** it does not strip leading/trailing ASCII whitespace or C0 control
  characters, and it does not decode character references. HTML URL parsing
  strips leading/trailing ASCII whitespace and decodes entities in attribute
  values before resolving the URL, so a value such as
  `formaction=" javascript:alert(1)"` or `href="java&#x73;cript:alert(1)"`
  can evade a raw-prefix check.
- **Current practical exposure:** low. The default attribute filters only
  allow `href`/`src` (which the link-attributes transformer drops or the
  renderer writes from the sanitized destination) and non-script URL
  attributes (`ping`, `srcset`, `usemap`). `formaction` is not in the default
  link/image whitelist. The gap becomes relevant if a user supplies a custom
  `AttributeFilters`, or if `render_attributes_safe` is reused elsewhere.
- **Required change (defense in depth):**
  1. Trim leading/trailing bytes `<= 0x20` before matching.
  2. Decode numeric/named character references before matching; reuse
     `crate::util::resolve_entity_references` (already public) or call it
     from the renderer module.
  3. Keep the `data:image/*` allow-list behavior; add explicit tests for
     `data:image/svg+xml` (must be dangerous) and `data:image/png` (allowed).
  4. Document that attributes are compared after HTML entity decoding.
- **Tests to add (unit):**
  ```rust
  assert!(is_dangerous_url(b" javascript:alert(1)"));
  assert!(is_dangerous_url(b"java&#x73;cript:alert(1)"));
  assert!(is_dangerous_url(b"java\0script:alert(1)"));
  assert!(is_dangerous_url(b"data:image/svg+xml;base64,..."));
  assert!(!is_dangerous_url(b"data:image/png;base64,..."));
  ```
  Also keep the existing link-destination tests (`[a]( javascript:…)` → empty
  `href`).
- **Acceptance:** the new unit tests pass; no behavior change for ordinary
  safe URLs; QUICKREF documents that URL checks are normalized.

#### GL-10 — the “`style` requires `allows_unsafe`” statement is over-broad in user docs

- **Guideline:** `M-CANONICAL-DOCS` (documentation must match behavior).
- **Location:** `docs/QUICKREF.md` (the “Limits / syntax rules” bullet added
  for BF-06); `CHANGELOG.md` has the correctly scoped “from the new
  extensions” wording.
- **Verified behavior:** with `ParseOptions(attributes=True)` and
  `RenderOptions(allows_unsafe=False)`:
  ```text
  # hi {style="color:red"}
  → <h1 style="color:red">hi</h1>
  ```
  The pre-existing attributes extension still emits `style`; the new
  link-attributes/fenced-div/definition-list paths drop it. BF-06 only
  promised the latter, so this is not a regression — but the current
  QUICKREF sentence reads as a blanket guarantee.
- **Required change:** scope the sentence explicitly:
  “`style`, `srcdoc`, and dangerous URL values supplied through the new
  link-attribute, fenced-div, and definition-list extensions are dropped
  unless `RenderOptions(allows_unsafe=True)`. The older
  `ParseOptions(attributes=True)` extension keeps its 0.10 behavior.”
  Add a note that unifying the two attribute paths is 0.12 work.
- **Acceptance:** the doc sentence and CHANGELOG agree; a test asserts both
  behaviors (new extension drops style; old attributes extension keeps it)
  so a future unification cannot silently change one without the other.

#### GL-11 — pre-existing `unsafe` lacks safety comments; no-std debug path fails on Windows

- **Guideline:** `M-UNSAFE` (“Any use of `unsafe` must be accompanied by
  plain-text reasoning outlining its safety. This applies to both calling
  `unsafe` methods, as well as providing `_unchecked` ones.”).
- **Locations:**
  - `src/parser/attribute.rs:163` —
    `result.push_str(unsafe { core::str::from_utf8_unchecked(&resolved) });`
  - `src/lib.rs:810` — `libc::write(1, s.as_ptr() as *const _, s.len());`
- **Current state:** neither block has a `// SAFETY:` comment. The no-std
  block is gated only on `feature = "no-std-unix-debug"`, not on `unix`, so
  the Windows build sees the Unix `libc::write` signature and fails with
  `expected u32, found usize` (the BF-23 local limitation).
- **Why it matters:** BF-23 claims minimal-feature CI coverage; an uncommented
  `unsafe` block and a feature name that says “unix” but compiles on Windows
  are exactly the kind of defect that coverage is meant to catch.
- **Required change:**
  1. Add a `// SAFETY:` comment to the `from_utf8_unchecked` call explaining
     why `resolved` is valid UTF-8: the input is a slice of the UTF-8
     `source`, and the entity/numeric-reference replacements are written with
     `char::encode_utf8` (`src/util.rs:1087–1103`), so the concatenated bytes
     remain valid UTF-8.
  2. Restrict the libc import and call to `#[cfg(all(feature =
     "no-std-unix-debug", unix))]`; on non-Unix, provide the no-op `Ok(())`
     path. If Windows no-std debugging is desired later, add a dedicated
     `windows` implementation rather than reusing the Unix signature.
  3. Add a `// SAFETY:` comment for the libc write (valid pointer for
     `s.len()` bytes, fd 1) and document that partial writes are ignored
     because this is a debug-only path.
- **Acceptance:** `cargo check --no-default-features --features
  no-std-unix-debug,html-entities` passes on Windows and Linux;
  `grep -n unsafe src/lib.rs src/parser/attribute.rs` shows a `SAFETY:`
  comment for every block.

#### GL-12 — PyO3 method panics instead of returning `PyResult`

- **Guideline:** `M-PANIC-IS-STOP` (“It is not valid to … use panics to
  communicate (errors) upstream”); library code should return `Result` for
  recoverable failures.
- **Location:** `mordant-py/src/chunker.rs:219`.
- **Current state:**
  ```rust
  fn get_bare_chunks<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyList> {
      let list = PyList::empty(py);
      for s in self.inner.get_bare_chunks() {
          list.append(s).unwrap();
      }
      list
  }
  ```
  `PyList::append` returns `PyResult<()>`; the `unwrap` turns a Python
  allocation/runtime error into a Rust panic (which aborts the interpreter or
  surfaces as `PanicException`).
- **Why it matters:** this is pre-existing, but BF-19 modified the file and
  its constructors. The guideline applies to the binding as much as to the
  core crate.
- **Required change:**
  ```rust
  fn get_bare_chunks<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
      let list = PyList::empty(py);
      for s in self.inner.get_bare_chunks() {
          list.append(s)?;
      }
      Ok(list)
  }
  ```
  Python callers still receive a list; only the error path changes.
- **Acceptance:** `cargo check --manifest-path mordant-py/Cargo.toml`;
  `pytest -q mordant-py/tests` passes; a test that exercises
  `get_bare_chunks()` still returns a list.

#### GL-13 — broken intra-doc link in the chunker

- **Guideline:** `M-CANONICAL-DOCS` (docs must resolve).
- **Location:** `src/chunker.rs:187`.
- **Current state:** `/// # Safety invariant (for [`TextSource::str`])` —
  `TextSource` has no `str` item; `cargo doc` warns “unresolved link to
  `TextSource::str`”.
- **Required change:** use `[`TextSource`]` or
  `[`TextSource::str_checked`]`, whichever matches the invariant being
  described; remove the broken path.
- **Acceptance:** `cargo doc --no-deps` no longer reports this warning.

#### GL-14 — inherited `no-std` feature-model debt

- **Guideline:** `M-FEATURES-ADDITIVE` (“You must not introduce a `no-std`
  feature, use a `std` feature instead”).
- **Location:** `Cargo.toml` (`std`, `alloc`, `no-std`, `no-std-unix-debug`
  features); all new syntax features use `["std"]`.
- **Current state:** the crate has a pre-existing `no-std` feature.
  `definition-list`, `fenced-div`, and `link-attributes` all pull `std`, so
  `--no-default-features --features no-std,definition-list` compiles as a
  hybrid (std and no-std both enabled) rather than a coherent no-std build.
  `--no-default-features` alone fails on `memchr`.
- **Why it matters:** the guideline exists to prevent exactly this
  feature-unification ambiguity; new features should not deepen the debt.
- **Required change:** do not fix in 0.11 unless embedded support is a
  release goal. Document the matrix in ARCHITECTURE/README:
  - supported minimal: `default-features = false, features = ["std"]`;
  - supported embedded: `features = ["no-std"]` (parser core only);
  - new syntax extensions require `std` and are not available in `no-std`.
  For 0.12, invert the model to a `std` feature and make `no-std` a
  compatibility alias, or split the crate.
- **Acceptance:** docs state the supported combinations; CI continues to test
  the two supported rows, not every hybrid.

#### GL-15 — the new doctest is over-gated

- **Location:** `src/lib.rs` (the `# Example` inside GL-01’s block).
- **Current state:** `# #[cfg(all(feature = "emoji", feature = "footnotes"))]`
  guards an example that uses only `Engine::Footnotes` and footnote syntax.
  With only `footnotes` enabled, the example body is compiled out and proves
  nothing.
- **Required change:** change the gate to `#[cfg(feature = "footnotes")]`.
  If the example is intentionally meant to demonstrate multiple engines, add a
  second example instead of over-gating this one.
- **Acceptance:** `cargo test --no-default-features --features std,footnotes
  --doc` runs the example (not an empty body).

### B.5 Additional observations

These are not guideline violations that block the release, but they are worth
recording while the code is fresh.

#### GL-16 — `available_engines()` duplicates `Engine::ALL`

- **Location:** `src/lib.rs:347–374`, `src/extensions.rs:44–56`.
- **Observation:** `AVAILABLE` re-lists every variant behind `#[cfg]`, while
  `Engine::ALL` lists them unconditionally. Adding an engine requires editing
  both, and a mismatch would make `available_engines()` disagree with
  `is_engine_available()`.
- **Suggested test (cheap, no API change):**
  ```rust
  #[test]
  fn available_matches_engine_flags() {
      for engine in Engine::ALL {
          assert_eq!(
              available_engines().contains(engine),
              is_engine_available(*engine),
              "availability mismatch for {engine}"
          );
      }
  }
  ```
- **Acceptance:** the test passes with every feature subset.

#### GL-17 — avoid a per-attribute allocation in the unsafe-attribute check

- **Location:** `src/renderer/html.rs:1243–1256`.
- **Observation:** `key.to_ascii_lowercase().as_str()` allocates a `String`
  for every rendered attribute, even though the key set is tiny. Attributes
  are not usually a hot path, but the fix is trivial:
  ```rust
  const URL_ATTRS: &[&str] = &[
      "href", "src", "action", "formaction", "poster", "cite", "data",
      "xlink:href", "background", "lowsrc",
  ];
  fn is_url_attribute(key: &str) -> bool {
      URL_ATTRS.iter().any(|a| key.eq_ignore_ascii_case(a))
  }
  ```
  The closest guideline is `M-MEM-REUSE` (avoid repeated allocations in
  loops); treat this as optional cleanup rather than a blocker.
- **Acceptance:** `cargo clippy` clean; behavior unchanged.

#### GL-18 — div/dl attribute filters reuse the paragraph filter

- **Locations:** `src/fenced_div.rs:266–278`,
  `src/definition_list.rs:305–318`.
- **Observation:** both renderers call
  `attribute_filters.paragraph()` because `AttributeFilters` has no `div` or
  `definition_list` field. The default paragraph filter is the global
  attribute set, so the default behavior is sensible, but a user who
  customizes paragraph filters (for example, to restrict `style`) will
  silently affect fenced divs and definition lists too.
- **Suggested change (0.12):** add `div` / `definition_list` fields to
  `AttributeFilters`, defaulting to the global set. For 0.11, document the
  reuse in the renderer docs so the behavior is not surprising.
- **Acceptance:** documented now; follow-up tracked.

#### GL-19 — Python `Engines` constructor has eleven parameters

- **Location:** `mordant-py/src/options.rs:287–330`.
- **Observation:** `M-INIT-CASCADED` recommends helper types for 4+
  parameters. For a PyO3 constructor whose callers use keyword arguments,
  this is an accepted binding-layer deviation; the Python signature is the
  API, not the Rust function. Keep as-is unless the Rust helper is needed for
  maintenance.
- **Action:** none; recorded so a future reviewer does not “fix” it into an
  awkward builder that Python users cannot see.

#### GL-20 — README/CHANGELOG wording drift

- **Locations:** `README.md:25`, `CHANGELOG.md` (“`available_engines()` now
  reports only the compiled-in engines.”).
- **Observations:**
  - README says “Every engine ships in the wheel and is on by default” and
    then “Syntax highlighting stays opt-in”. The first clause is false for
    highlighting; rewrite as “Every engine ships in the wheel; all except
    syntax highlighting are on by default.”
  - `available_engines()` exists only in the Rust crate; the Python binding
    does not expose it. Say “Rust `available_engines()`” so Python users do
    not search for a missing function.
- **Acceptance:** the two files match the implemented default engine set and
  the actual API surface.

### B.6 What already complies

The following were checked and **do not** need changes; they are recorded so a
future review does not re-litigate them.

- **Module documentation (`M-MODULE-DOCS`).** `definition_list`, `fenced_div`,
  `link_attributes`, and `extensions` all start with `//!` documentation, a
  short first sentence, syntax samples, and behavior limits (single-paragraph
  definitions; closing-fence rule; inline-split parsing).
- **First doc sentences (`M-FIRST-DOC-SENTENCE`).** The new public items
  mostly have one-line summaries under 15 words; GL-01/GL-02 are the
  exceptions.
- **Inherent functionality (`M-ESSENTIAL-FN-INHERENT`).** `Engines` exposes
  `all`/`none`/`only`/`without`/`set` as inherent methods; no essential
  behavior lives only in a trait.
- **Construction (`M-INIT-BUILDER`).** `Engines::only` and `without` provide
  the combinatorial shortcuts; the remaining construction is a flat flag
  struct, not a nested generic API.
- **Panic policy (`M-PANIC-IS-STOP`, `M-PANIC-ON-BUG`).** No `unwrap`,
  `expect`, `panic!`, or `unreachable!` appears in the new production parser
  or renderer paths; the only hits are in `#[cfg(test)]` modules, where panic
  messages are not required. GL-12 is a pre-existing binding exception.
- **Unsafe policy (`M-UNSAFE`).** No new `unsafe` was introduced by BF-01..
  BF-23; GL-11 covers inherited blocks.
- **Feature additivity (`M-FEATURES-ADDITIVE`).** The new features gate
  modules and registrations rather than disabling public items; the tested
  combinations (`std`, all engines, each new feature alone, and
  `no-std` + each new feature) compile. GL-05/GL-14 are the remaining
  evolution/debt items.
- **Out-of-the-box build (`M-OOBE`).** The default feature set builds on
  Windows and Linux; CI covers both. The only platform-specific failure is
  the pre-existing Windows no-std debug path (GL-11).
- **No wrappers in public APIs (`M-AVOID-WRAPPERS`,
  `M-SIMPLE-ABSTRACTIONS`).** The new public surface is a flat `Engines`
  value plus extension factories returning `impl Trait`; no `Rc`/`Arc`/
  `RefCell` was added to a signature. The existing `Rc<AttributeFilters>` in
  `Options` is pre-existing and justified by cheap cloning.
- **Parameter consistency (`M-PARAMETER-CONSISTENCY`).**
  `markdown_to_html_string_with_options(output, source, parser_opts,
  renderer_opts)` follows the existing `markdown_to_html_string(output,
  source)` order.

### B.7 Remediation order and milestone mapping

Do **not** mix these with behavior changes; apply them as review follow-ups so
the regression story stays clear.

| Milestone | Items | Rationale |
|---|---|---|
| **B0 — before committing the bug-fix branch** | GL-01, GL-02, GL-03, GL-08, GL-10, GL-15, GL-20 | Pure docs/hygiene; zero risk; the current doc block is actively wrong. |
| **B1 — before tagging 0.11.0** | GL-04, GL-05, GL-06, GL-07, GL-11 | Public API contract, future-proofing, and the no-std/unsafe cleanup that BF-23 claims. |
| **B2 — originally 0.11.x patch or 0.12** | GL-09, GL-12, GL-13, GL-14, GL-16, GL-17, GL-18, GL-19 | Defense-in-depth and debt; none changes the default output. |

**Applied status:** all B0, B1 and B2 items were applied in one follow-up pass
on `dev0.11.0` before committing. GL-19 was reviewed and left unchanged by
design. The commit split below remains the recommended history if the team
prefers smaller commits; the working tree currently contains them together.

Recommended commit split if applied:

1. `docs: fix 0.11 rustdoc attachments and extension docs` (GL-01..GL-04,
   GL-08, GL-10, GL-15, GL-20).
2. `api: mark Engine/Engines non-exhaustive and re-export at root` (GL-05,
   GL-06, GL-07).
3. `safety: document unsafe blocks and gate no-std debug to unix` (GL-11).
4. `py: propagate PyList append errors` (GL-12).
5. `docs: fix chunker intra-doc link` (GL-13).
6. `test: add engine availability consistency check` (GL-16).
7. Optional cleanup: GL-17/GL-18.

### B.8 Verification matrix after applying the amendment

Run all of the following; every row must be green before the release is
tagged.

| Command | Expected |
|---|---|
| `cargo fmt --all --check` | clean for changed files (repo-wide rustfmt churn is pre-existing) |
| `cargo clippy --lib --features meta,emoji,footnotes,linter,diagram,chunker,math,highlighter,definition-list,fenced-div,link-attributes` | no warnings in changed files |
| `cargo test` | all default tests + doctests pass |
| `cargo test --features meta,emoji,footnotes,linter,diagram,chunker,math,highlighter,definition-list,fenced-div,link-attributes` | all-engine suite passes |
| `cargo test --no-default-features --features std` | minimal suite passes |
| `cargo test --no-default-features --features std --doc` | minimal doctests pass |
| `cargo test --no-default-features --features std,footnotes --doc` | the GL-15 example actually runs (not an empty body) |
| `cargo check --no-default-features --features no-std,definition-list` (and `fenced-div`, `link-attributes`) | compile (documented hybrid; GL-14) |
| `cargo check --no-default-features --features no-std-unix-debug,html-entities` on Windows and Linux | passes after GL-11 |
| `cargo doc --no-deps` | no new warnings; GL-13 gone; GL-01 summaries under correct items |
| `maturin develop --manifest-path mordant-py/Cargo.toml --release` | wheel builds |
| `python -m pytest -q mordant-py/tests` | full suite passes (1288 after the GL-10 regression test) |

Additional targeted checks:

```bash
# GL-01/GL-07: summaries are attached to the right functions
cargo doc --no-deps --open   # inspect the two items manually

# GL-09: URL normalization
cargo test --lib is_dangerous_url

# GL-10: scoped docs behavior
python - <<'PY'
import mordant
po = mordant.ParseOptions(attributes=True)
ro = mordant.RenderOptions(allows_unsafe=False)
html = mordant.markdown_to_html('# hi {style="color:red"}', parse_opts=po, render_opts=ro)
assert 'style="color:red"' in html  # old extension keeps 0.10 behavior
html = mordant.markdown_to_html('[a](https://e.x){style="color:red"}', render_opts=ro)
assert "style=" not in html          # new extension drops it
PY
```

### B.9 Amendment definition of done

- [x] GL-01/GL-02/GL-03 applied; every new public item has a correct
      one-line summary and canonical sections.
- [x] GL-04/GL-05/GL-06/GL-07 applied; `Engine`/`Engines` are documented,
      future-proofed, and discoverable; the visibility of
      `parser_extensions` is an explicit decision.
- [x] GL-08/GL-10/GL-20 applied; `allows_unsafe` docs, QUICKREF, README, and
      CHANGELOG describe the same policy.
- [x] GL-11 applied; every `unsafe` block touched by this amendment has a
      `SAFETY:` comment and the no-std debug path compiles on Windows.
      (Pre-existing unsafe blocks elsewhere in the crate are out of scope.)
- [x] GL-12/GL-13 applied; `cargo doc` has no broken link and PyO3 methods do
      not panic on append errors.
- [x] GL-15 applied; the doctest runs with `footnotes` alone.
- [x] GL-16 regression test added; GL-17/GL-18/GL-19 recorded (applied or
      tracked).
- [x] GL-19 reviewed and accepted as a PyO3 keyword-API deviation; no code
      change required.
- [x] The full matrix in §B.8 passes; no behavior changes beyond those
      already documented in the CHANGELOG.
- [x] If any amendment changes public API (GL-05/GL-06/GL-07), add a
      CHANGELOG “Changed” entry and update the migration notes.
