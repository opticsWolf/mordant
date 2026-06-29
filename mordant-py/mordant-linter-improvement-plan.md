# Mordant Markdown Linter — Implementation Plan

A phased plan to evolve the Rust-implemented Markdown linter (`linter.rs` + the
`lint` / `fix` bindings) from a working-but-unverified first cut into a correct,
configurable, adoptable tool.

This document is the engineering spec: each phase carries explicit requirements
(stable IDs), a design with code, acceptance criteria, an effort estimate, and
dependencies. Code snippets are illustrative; signatures that touch the rushdown
AST are marked **[API]** because they depend on the real rushdown surface (see
Phase 0).

---

## 0. Current state & premises

**What exists today**

- `linter.rs` — a pure-Rust engine: one DFS extracts a document model
  (`Collected`: headings, links, images, code blocks); rules inspect it.
  Nine rules (MD001, MD024, MD025, MD040, MD042, MD045 from the AST; MD009,
  MD012, MD047 line-based). `Violation` → `Diagnostic` (pyclass). Auto-fix for
  MD009/MD012/MD047 (and MD040 with a supplied language) via a line-keyed
  `FixOp` model and `apply_fixes` / `run_fix`. `FixResult` exposes
  `output` / `fixed` / `unfixable`.
- `lib.rs` — `lint()` and `fix()` pyfunctions (parse + run under `py.detach`).
- `document.rs` — `doc.lint()` / `doc.fix()` methods.
- `test_lint.py` — Python coverage for every rule and the fixer.
- `__init__.py` — full re-export of the compiled module.

**Premises that shape this plan**

- **P1 — Nothing here is compiled yet.** The Rust is written against an
  *inferred* rushdown API (`kind_data()`, `pos()`, `language_str()`, etc.).
  Phase 0 is therefore mandatory and blocks everything else.
- **P2 — Markdown has no serializer in rushdown** (it renders to HTML). All
  fixes are minimal edits to source text, never AST round-trips. This is a
  design invariant, not a limitation to remove.
- **P3 — There are currently two notions of "inside code"**: the AST
  `CodeBlock` nodes, and a separate lexical `fence_mask` / `is_fence_line`
  scan. Unifying them on the AST is a recurring theme (Phase 2).

**Global non-functional requirements**

| ID | Requirement |
|----|-------------|
| NFR-1 | Auto-fixes must never change rendered output for whitespace-only rules. Enforced by an HTML-equivalence oracle (Phase 1). |
| NFR-2 | `lint()` / `fix()` keep releasing the GIL; engine functions stay `Send` and free of `Py*` references. |
| NFR-3 | Fixing is idempotent: `fix(fix(x)) == fix(x)`. |
| NFR-4 | No rule may panic on any input. Fuzz/property inputs included in CI. |
| NFR-5 | Public Python API stays backward-compatible within a minor version; additive changes only. |
| NFR-6 | Every new rule ships with: positive tests, clean-negative tests, and (if fixable) fix + idempotence tests. |

**Global definition of done (per phase):** code + tests written, `cargo test`
and `pytest` green, `cargo clippy` clean, docs/docstrings updated, `CHANGELOG`
entry added.

---

## Phase 0 — Compile & establish a green baseline

**Goal:** turn inferred code into building code, and lock in a passing suite as
the reference point for everything after.

**Requirements**

- R0.1 — `maturin develop` builds the extension against the real rushdown crate.
- R0.2 — Every AST accessor assumed in `linter.rs` is reconciled with the actual
  rushdown API (method names, variant names, return types).
- R0.3 — `pytest tests/` passes, including `test_lint.py`.
- R0.4 — A `CHANGELOG.md` and a CI workflow run build + both test suites.

**Design / steps**

1. Build and triage compiler errors. The likely mismatch points, all isolated
   to `linter.rs` `build()` / `collect_text()`:

   | Assumed in code | Confirm against rushdown |
   |---|---|
   | `arena[nref].kind_data()` returns matchable `KindData` | exact return type & borrow form |
   | `KindData::{Heading,Link,Image,CodeBlock,Text,CodeSpan,RawHtml}` | variant names |
   | `heading.level() -> u8` | name/return |
   | `link.destination_str(src) -> &str` | name/return |
   | `codeblock.language_str(src) -> Option<&str>` | name/return |
   | `node.pos() -> Option<usize>` (0-indexed line) | name/semantics |
   | `node.first_child()/next_sibling() -> Option<NodeRef>` | names |

2. Capture the confirmed API in a short `docs/ast-notes.md` so future rules
   don't re-derive it.

**Acceptance criteria**

- [ ] `maturin develop` succeeds.
- [ ] `pytest tests/test_lint.py` reports all green.
- [ ] CI runs `cargo build`, `cargo test`, `pytest` on push.

**Effort:** S–M (mostly mechanical renames). **Risk:** Low. **Depends on:** —

---

## Phase 1 — Test & correctness harness

**Goal:** prove the parts we *can* prove without the parser, and install the
oracles that make later refactors safe.

**Requirements**

- R1.1 — Rust unit tests (`#[cfg(test)]`) cover every pure function:
  `apply_fixes`, `set_fence_language`, `fence_mask`, `md009`, `md012`, `md047`.
  These need neither rushdown nor Python and run under plain `cargo test`.
- R1.2 — Property tests (proptest) assert engine invariants NFR-3 and "fixing
  never increases the fixable-violation count."
- R1.3 — HTML-equivalence oracle (NFR-1): for the whitespace subset,
  `markdown_to_html(src) == markdown_to_html(fix(src).output)`.
- R1.4 — A fuzz target over `lint`/`fix` to satisfy NFR-4.

**Design / code**

Pure-function unit tests live next to the engine and bypass the parser entirely:

```rust
// linter.rs (bottom)
#[cfg(test)]
mod tests {
    use super::*;

    fn src(s: &str) -> Source<'_> {
        Source { text: s, lines: s.lines().collect() }
    }

    #[test]
    fn md009_preserves_two_space_hard_break() {
        let s = src("line  \n");           // exactly two = hard break
        let mask = fence_mask(&s.lines);
        let mut v = Vec::new();
        md009(&s, &mask, &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn set_fence_language_keeps_indent_and_marker() {
        assert_eq!(set_fence_language("```", "py"), "```py");
        assert_eq!(set_fence_language("  ~~~~", "py"), "  ~~~~py");
    }

    #[test]
    fn apply_fixes_delete_beats_replace_on_same_line() {
        let fixes = vec![
            Violation::warn_fix("MD009","x",Some(2),"", FixOp::ReplaceLine{line:1,text:"".into()}),
            Violation::warn_fix("MD012","x",Some(2),"", FixOp::DeleteLine{line:1}),
        ];
        assert_eq!(apply_fixes("a\n  \nb\n", &fixes, None), "a\nb\n");
    }
}
```

Invariants as property tests (idempotence shown; uses the real parser, so gate
behind a feature that's on in CI after Phase 0):

```rust
proptest! {
    #[test]
    fn fix_is_idempotent(s in ".{0,400}") {
        let once = fix_str(&s);              // thin test helper: parse+run_fix
        let twice = fix_str(&once);
        prop_assert_eq!(once, twice);
    }
}
```

HTML-equivalence oracle on the Python side (the strongest guarantee we have —
NFR-1):

```python
# tests/test_fix_safety.py
import hypothesis.strategies as st
from hypothesis import given
import mordant

WS_ONLY = mordant.LintOptions(enable=["MD009", "MD012", "MD047"])

@given(st.text(max_size=400))
def test_whitespace_fixes_never_change_rendering(s):
    fixed = mordant.fix(s, lint_opts=WS_ONLY).output
    assert mordant.markdown_to_html(s) == mordant.markdown_to_html(fixed)
```

**Acceptance criteria**

- [ ] `cargo test` runs the pure-function suite with no rushdown feature needed.
- [ ] Idempotence + non-increase properties pass over ≥1000 cases.
- [ ] HTML-equivalence holds for the whitespace subset over the Hypothesis run.
- [ ] Fuzz target runs clean for a fixed time budget in CI.

**Effort:** M. **Risk:** Low. **Depends on:** Phase 0 (for parser-backed props).

---

## Phase 2 — AST-derived code-region model

**Goal:** make the AST the single source of truth for "inside code," retiring
the lexical `fence_mask` / `is_fence_line` re-scan (premise P3). This removes a
class of false positives (fences in blockquotes/list items, tilde vs backtick,
fence-length matching).

**Requirements**

- R2.1 — Derive covered source lines from `CodeBlock` (and inline code where
  relevant) AST nodes, not by re-scanning text.
- R2.2 — MD009/MD012 skip exactly the AST-derived code lines.
- R2.3 — MD040's "is this fenced?" decision comes from the AST, not a line peek.
- R2.4 — No behavioral regressions vs Phase 1 baseline on existing tests.

**Design / code**

Extend the single DFS to record code-block line spans while it already walks the
tree, then materialize a mask:

```rust
struct CodeRegion { start: usize, end: usize, fenced: bool } // 0-indexed, inclusive

// During build(): for each KindData::CodeBlock, compute the span. [API]
// start = node.pos(); line_count = codeblock.value().iter(src).count();
// fenced adds the opening (+ closing) delimiter lines.
fn code_region_for(arena: &Arena, nref: NodeRef, src: &Source) -> Option<CodeRegion> { /* [API] */ }

fn code_mask(regions: &[CodeRegion], n_lines: usize) -> Vec<bool> {
    let mut mask = vec![false; n_lines];
    for r in regions {
        for i in r.start..=r.end.min(n_lines.saturating_sub(1)) { mask[i] = true; }
    }
    mask
}
```

`run_lint` then builds `mask` from `collected.code_regions` and passes it to the
line rules; `fence_mask`/`is_fence_line` are deleted. MD040 reads
`region.fenced` instead of peeking `src.lines[pos]`.

> **Risk to manage:** computing a fenced block's *end* line from content-line
> count + delimiters is the fragile bit. Mitigate by preferring a node end
> offset if rushdown exposes one; otherwise unit-test the span math hard
> (Phase 1 style) across indented/fenced/tilde/nested cases before deleting the
> lexical fallback.

**Acceptance criteria**

- [ ] `fence_mask` and `is_fence_line` are removed.
- [ ] New tests: fence inside blockquote and inside list item are correctly
      treated as code by MD009/MD012.
- [ ] All prior tests still pass.

**Effort:** M. **Risk:** Med (span math). **Depends on:** Phase 0, 1.

---

## Phase 3 — Rich diagnostic positions (column + span)

**Goal:** carry column and byte-span, not just a line. Unlocks editor
underlines, range-based fixes (Phase 4), and an eventual language server.

**Requirements**

- R3.1 — `Violation` and `Diagnostic` gain `column: Option<usize>` (1-indexed)
  and `span: Option<(usize, usize)>` (byte offsets, half-open).
- R3.2 — AST rules populate spans from node offsets where available. **[API]**
- R3.3 — Python `Diagnostic` exposes `.column` and `.span`; additive only
  (NFR-5). Existing `.line` unchanged.

**Design / code**

```rust
pub struct Violation {
    pub rule: &'static str,
    pub name: &'static str,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,        // new
    pub span: Option<(usize, usize)>, // new, byte offsets into source
    pub severity: Severity,
    pub fix: Option<FixOp>,
}
```

Constructors gain optional position info (keep `warn`/`warn_fix` as
line-only shims so existing rules compile unchanged, add `warn_at` for
span-aware rules). `Diagnostic` mirrors the fields with getters.

**Acceptance criteria**

- [ ] `d.column` / `d.span` available in Python; `None` where unknown.
- [ ] At least the link/image/heading rules emit spans.
- [ ] Serialization (Phase 7 JSON) includes the new fields.

**Effort:** M. **Risk:** Med (depends on AST offset availability). **Depends on:** Phase 0.

---

## Phase 4 — Fix-engine hardening

**Goal:** generalize from line-keyed ops to byte-range edits, resolve overlaps
properly, fix-to-stable, and report `remaining` accurately.

**Requirements**

- R4.1 — Introduce a byte-range `Edit { start, end, replacement }` model;
  line ops become a thin layer that lowers to edits.
- R4.2 — Overlapping edits resolved deterministically (earliest start wins;
  conflicts dropped, not misapplied).
- R4.3 — `fix` re-lints to a stable fixpoint, bounded by a max-iteration cap
  (default 10), so future cascading rules are safe.
- R4.4 — `FixResult.remaining` is computed by re-linting `output`, so its
  positions are correct in the corrected text (supersedes input-coordinate
  `unfixable`; keep `unfixable` as a deprecated alias for one minor version).

**Design / code**

```rust
pub struct Edit { pub start: usize, pub end: usize, pub replacement: String } // byte offsets

fn apply_edits(source: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|e| (e.start, e.end));
    let mut out = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for e in edits {
        if e.start < cursor { continue; }            // overlap → drop (R4.2)
        out.push_str(&source[cursor..e.start]);
        out.push_str(&e.replacement);
        cursor = e.end;
    }
    out.push_str(&source[cursor..]);
    out
}
```

Fix-to-stable loop (R4.3), parser-backed so it lives in `lib.rs` alongside
`parse_only`:

```rust
fn run_fix_stable(source, gfm, parse_cfg, lint_cfg, default_language) -> FixOutcome {
    let mut current = source.to_string();
    let mut all_fixed = Vec::new();
    for _ in 0..MAX_FIX_ITERS {
        let (arena, root) = parse_only(&current, gfm, parse_cfg);
        let outcome = run_fix(&current, &arena, root, lint_cfg, default_language);
        if outcome.output == current { break; }       // fixpoint
        all_fixed.extend(outcome.fixed);
        current = outcome.output;
    }
    // remaining = lint(current) (R4.4)
}
```

**Acceptance criteria**

- [ ] Edits expressed as byte ranges; `apply_fixes` lowers line ops to `Edit`s.
- [ ] Overlap test: two edits on the same range → one applied, never corrupted.
- [ ] `remaining` line numbers match positions in `output`.
- [ ] Idempotence (NFR-3) still holds with the loop.

**Effort:** M–L. **Risk:** Med. **Depends on:** Phase 3 (spans).

---

## Phase 5 — Rule coverage expansion

**Goal:** broaden coverage, prioritizing high-frequency, safe-to-fix rules, and
add per-rule parameters.

**Requirements**

- R5.1 — Add rules in priority order:
  - **Auto-fixable:** MD010 (hard tabs→spaces), MD018–MD021 (ATX `#` spacing),
    MD022/MD031/MD032 (blank lines around headings/fences/lists),
    MD026 (trailing heading punctuation), MD034 (bare URL → `<…>`).
  - **Report (style/consistency):** MD003 (heading style), MD013 (line length),
    MD046/MD048 (code/fence style), MD049/MD050 (emphasis/strong style).
- R5.2 — Per-rule parameters (not just on/off): e.g. MD013 `line_length`,
  MD009 `br_spaces`, MD024 `siblings_only`, MD010 `spaces_per_tab`.
- R5.3 — Each rule satisfies NFR-6.

**Design / code**

Rule parameters as a typed, defaulted config carried in `LintConfig`:

```rust
#[derive(Clone)]
pub struct RuleParams {
    pub line_length: usize,     // MD013, default 80
    pub br_spaces: usize,       // MD009, default 2
    pub spaces_per_tab: usize,  // MD010, default 4
    pub siblings_only: bool,    // MD024, default false
}
impl Default for RuleParams { /* 80 / 2 / 4 / false */ }
```

Example new fixable rule (MD010, hard tabs), reusing the established pattern:

```rust
fn md010(src: &Source, mask: &[bool], params: &RuleParams, out: &mut Vec<Violation>) {
    let spaces = " ".repeat(params.spaces_per_tab);
    for (i, line) in src.lines.iter().enumerate() {
        if mask.get(i).copied().unwrap_or(false) { continue; }
        if line.contains('\t') {
            out.push(Violation::warn_fix(
                "MD010", "no-hard-tabs", Some(i + 1), "Hard tab character(s)",
                FixOp::ReplaceLine { line: i, text: line.replace('\t', &spaces) },
            ));
        }
    }
}
```

**Acceptance criteria**

- [ ] Each new rule has positive + clean-negative tests; fixable ones add
      fix + idempotence tests.
- [ ] Parameters are reachable from Python and from config files (Phase 6).
- [ ] HTML-equivalence (NFR-1) holds for every newly fixable whitespace rule.

**Effort:** L (scales with rule count; do in sub-batches). **Risk:** Low–Med.
**Depends on:** Phase 2 (mask), Phase 4 (fix model for the fixable ones).

---

## Phase 6 — Configuration, suppression & introspection

**Status:** ✅ **COMPLETE**

**Goal:** make the linter adoptable in real repos.

**Requirements**

- [x] R6.1 — Load a config file; mirror the `.markdownlint.json` schema to inherit
  an existing ecosystem.
- [x] R6.2 — Honor inline suppression comments:
  `<!-- markdownlint-disable MDxxx -->`, `-enable`, and `-disable-next-line`.
- [x] R6.3 — `mordant.lint_rules()` returns rule metadata
  (id, name, description, fixable, default params) for tooling/`--help`.

**Implementation summary**

Config file loading:
- `.markdownlint.json` auto-detected from CWD
- `enable` / `disable` rule lists (mutually exclusive modes)
- `RuleParams` passed through `LintConfig.params`

Inline suppression:
- `<!-- markdownlint-disable -->` / `<!-- markdownlint-enable -->` (all or specific rules)
- `<!-- markdownlint-disable-line -->` / `<!-- markdownlint-disable-next-line -->` (single line)
- Parsed once per source, applied in filter phase

Introspection:
- `mordant.lint_rules()` returns 25 `RuleMetadata` objects
- Each with `id`, `name`, `description`, `fixable`, `default_params`

**Bug fix:** MD026 and MD022 were incorrectly calling `byte_offset_to_line()` on `h.line` (already a 0-indexed line number). Fixes now target the correct line.

**Acceptance criteria**

- [x] A `.markdownlint.json` in the repo changes lint behavior as specified.
- [x] `disable-next-line` suppresses exactly one line; block disable/enable
      brackets a range.
- [x] `lint_rules()` enumerates all rules with correct `fixable` flags.

**Effort:** M–L. **Risk:** Med (schema breadth — scope to a documented subset).
**Depends on:** Phase 5 (params), Phase 2 (AST comments).

---

## Phase 7 — CLI, batch & output formats

**Goal:** the thing people actually run in CI.

**Requirements**

- R7.1 — `python -m mordant` CLI: globs/paths, `--fix`, `--config`,
  rule selection, proper exit codes (non-zero when unfixed issues remain).
- R7.2 — Formatters: human (default), `--json`, and GitHub Actions annotations.
- R7.3 — Batch API `lint_many(paths)` parallelized in Rust (rayon); `lint`/`fix`
  already release the GIL (NFR-2), so fan-out is near-free.

**Design / code**

```python
# mordant/__main__.py (sketch)
import argparse, glob, sys, mordant

def main():
    ap = argparse.ArgumentParser(prog="mordant")
    ap.add_argument("paths", nargs="+")
    ap.add_argument("--fix", action="store_true")
    ap.add_argument("--config")
    ap.add_argument("--format", choices=["human", "json", "github"], default="human")
    args = ap.parse_args()

    failures = 0
    for path in (f for p in args.paths for f in glob.glob(p, recursive=True)):
        text = open(path, encoding="utf-8").read()
        if args.fix:
            res = mordant.fix(text)
            if res.output != text:
                open(path, "w", encoding="utf-8").write(res.output)
            failures += len(res.unfixable)
        else:
            failures += len(mordant.lint(text))
    sys.exit(1 if failures else 0)
```

```rust
// Rust batch (rayon) exposed as lint_many — releases the GIL per file.
pub fn lint_many(files: Vec<(String, String)>, cfg: &LintConfig)
    -> Vec<(String, Vec<Violation>)> {
    files.par_iter()
        .map(|(name, src)| {
            let (arena, root) = parse_only(src, false, &ParseConfig::default());
            (name.clone(), run_lint(src, &arena, root, cfg))
        })
        .collect()
}
```

**Acceptance criteria**

- [ ] `mordant docs/**/*.md` exits non-zero on issues, zero when clean.
- [ ] `--fix` rewrites files and leaves only unfixable issues.
- [ ] `--json` output validates against a documented schema; `github` format
      produces valid workflow annotations.

**Effort:** M. **Risk:** Low. **Depends on:** Phase 4 (fix), Phase 6 (config).

---

## Phase 8 — Accuracy polish

**Goal:** close known correctness gaps that don't fit a single feature phase.

**Requirements**

- R8.1 — `collect_text` includes emoji/extension text so headings compare
  correctly (today `# Hello :smile:` and `# Hello` can differ in MD024). **[API]**
- R8.2 — MD025 treats a frontmatter `title:` (from the meta extension) as a
  document title when deciding "single H1." **[API]**
- R8.3 — MD042 also flags fragment-only links pointing at missing anchors
  (optional, once heading IDs are collected).

**Acceptance criteria**

- [ ] Heading-text-based rules account for inline extensions.
- [ ] A doc with frontmatter `title` + one `#` heading is handled per config.

**Effort:** S–M. **Risk:** Low. **Depends on:** Phase 0.

---

## Cross-cutting: data-model evolution

The `Violation` / `Diagnostic` / fix model grows monotonically; each step is
additive (NFR-5):

| Field | Introduced | Notes |
|---|---|---|
| `rule, name, message, line, severity` | now | — |
| `fix: Option<FixOp>` | now | line-keyed ops |
| `fixable` (Diagnostic) | now | derived |
| `column, span` | Phase 3 | byte offsets |
| `Edit { start,end,replacement }` | Phase 4 | `FixOp` lowers to this |
| `params: RuleParams` (config) | Phase 5 | per-rule tuning |
| `remaining` (FixResult) | Phase 4 | re-lint of output |

---

## Sequencing & dependency graph

```
Phase 0 ✅ ─┬─ Phase 1 ✅ ─┬─ Phase 2 ✅ ─┐
         │           │           ├─ Phase 5 ✅ ─┬─ Phase 6 ✅ ─ Phase 7
         │           └─ Phase 3 ✅ ─ Phase 4 ✅ ─┘
         └─ Phase 8 (independent; can slot in any time after 0)
```

Recommended order: **0 → 1 → 2 → 3 → 4 → 5 → 6 → 7**, with **8** opportunistic.
Phases 0–1 are prerequisites for trusting anything; 2–4 are the structural core;
5–7 are breadth and adoption.

---

## Risk register

| Risk | Phase | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| Inferred AST API wrong | 0 | High | High | Reconcile first; record in `ast-notes.md` |
| Fenced-block end-line math off | 2 | Med | Med | Prefer node end offsets; heavy unit tests before deleting lexical fallback |
| AST lacks byte offsets | 3 | Med | Med | Fall back to line/column only; degrade gracefully |
| Overlapping edits corrupt output | 4 | Low | High | `apply_edits` drops overlaps; idempotence + HTML oracle in CI |
| Config schema scope creep | 6 | Med | Med | Implement a documented subset of `.markdownlint.json`, reject unknowns loudly |
| Fixes change meaning | all | Low | High | NFR-1 HTML-equivalence oracle gates every fixable rule |

---

## Appendix A — Rule catalog

| ID | Name | Source | Fixable | Status |
|----|------|--------|---------|--------|
| MD001 | heading-increment | AST | no | done |
| MD009 | no-trailing-spaces | line | yes | done |
| MD012 | no-multiple-blanks | line | yes | done |
| MD024 | no-duplicate-heading | AST | no | done |
| MD025 | single-h1 | AST | no | done (Phase 8 refines) |
| MD040 | fenced-code-language | AST | with language | done |
| MD042 | no-empty-links | AST | no | done |
| MD045 | no-alt-text | AST | no | done |
| MD047 | single-trailing-newline | line | yes | done |
| MD010 | no-hard-tabs | line | yes | Phase 5 |
| MD018–MD021 | atx spacing | line/AST | yes | Phase 5 |
| MD022/MD031/MD032 | blanks around blocks | AST | yes | Phase 5 |
| MD026 | no-trailing-punctuation | AST | yes | Phase 5 |
| MD034 | no-bare-urls | AST | yes | Phase 5 |
| MD003 | heading-style | AST | no | Phase 5 |
| MD013 | line-length | line | no | Phase 5 |
| MD046/MD048 | code/fence style | AST | partial | Phase 5 |
| MD049/MD050 | emphasis/strong style | AST | partial | Phase 5 |

## Appendix B — Suggested first PRs

1. **Phase 0 baseline** — build fixes + green `pytest` + CI.
2. **Phase 1 pure tests + HTML oracle** — highest safety-per-line; unblocks fearless refactors.
3. **Phase 2 AST code-mask** — deletes the duplicate fence scanner.
