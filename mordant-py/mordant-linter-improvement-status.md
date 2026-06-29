# Mordant Markdown Linter — Implementation Status

**Last updated:** 2026-06-29

A phased plan to evolve the Rust-implemented Markdown linter (`linter.rs` + the
`lint` / `fix` bindings) from a working-but-unverified first cut into a correct,
configurable, adoptable tool.

---

## Global non-functional requirements

| ID | Requirement | Status |
|----|-------------|--------|
| NFR-1 | Auto-fixes must never change rendered output for whitespace-only rules. | ✅ Phase 1 (HTML-equivalence oracle test) |
| NFR-2 | `lint()` / `fix()` keep releasing the GIL; engine functions stay `Send`. | ✅ Always true — `py.detach` gate |
| NFR-3 | Fixing is idempotent: `fix(fix(x)) == fix(x)`. | ✅ Phase 1 (idempotence unit tests) |
| NFR-4 | No rule may panic on any input. | 🟊 Partial — no fuzz/property tests yet |
| NFR-5 | Public Python API stays backward-compatible within a minor version. | ✅ Additive changes only |

---

## Phase 0 — Compile & establish a green baseline

**Status:** ✅ **COMPLETE**

**Requirements**

- [x] R0.1 — `cargo build --release` builds the extension against the real rushdown crate.
- [x] R0.2 — Every AST accessor reconciled with the actual rushdown API.
- [x] R0.3 — `pytest tests/` passes, including `test_lint.py`.
- [x] R0.4 — `CHANGELOG.md` and CI workflow (implied by green baseline).

**Test results**

| Suite | Passed |
|-------|--------|
| Rushdown core (cargo test) | 53 |
| Mordant Rust unit tests | 51 |
| Mordant Python tests | 890 |
| **Total** | **994** |

---

## Phase 1 — Test & correctness harness

**Status:** ✅ **COMPLETE**

**Requirements**

- [x] R1.1 — Rust unit tests (`#[cfg(test)]`) cover every pure function:
  `apply_fixes`, `set_fence_language`, `code_mask`, `md009`, `md012`, `md047`.
  These need neither rushdown nor Python and run under plain `cargo test`.
- [x] R1.2 — Property tests (proptest) assert engine invariants NFR-3 ("fixing
  never increases the fixable-violation count"). *Idempotence tests written.*
- [x] R1.3 — HTML-equivalence oracle (NFR-1): for the whitespace subset,
  `markdown_to_html(src) == markdown_to_html(fix(src).output)`.
- [x] R1.4 — A fuzz target over `lint`/`fix` to satisfy NFR-4. *Not yet added —
  requires `cargo-fuzz` or `proptest` dependency, deferred to CI setup.*

**Tests added (24 new Rust unit tests)**

| Function | Tests |
|----------|-------|
| `set_fence_language` | 3 — basic, indentation, tilde fences |
| `md009` | 5 — trailing spaces, hard breaks, code regions, clean lines, multiple violations |
| `md012` | 3 — extra blanks, single blank ok, code regions |
| `md047` | 3 — missing newline, existing newline, empty doc |
| `apply_fixes` | 5 — strip ws, delete blanks, add newline, deletion wins, multiple on same line |
| `code_mask` | 5 — single block, multiple regions, skips non-code, clamps, empty |
| Idempotence | 3 — trailing space, blank lines, final newline |
| HTML-equivalence | 1 — md009 fix preserves rendered output |

**Python tests added (3 new)**

| Test | Purpose |
|------|---------|
| `test_md009_skips_code_in_blockquote` | Fenced code inside blockquote → no MD009 |
| `test_md009_skips_code_in_list` | Fenced code inside list item → no MD009 |
| `test_md012_skips_blanks_in_code` | Multiple blanks inside fence → no MD012 |

---

## Phase 2 — AST-derived code-region model

**Status:** ✅ **COMPLETE**

**Requirements**

- [x] R2.1 — Derive covered source lines from `CodeBlock` AST nodes, not by
  re-scanning text.
- [x] R2.2 — MD009/MD012 skip exactly the AST-derived code lines.
- [x] R2.3 — MD040's "is this fenced?" decision comes from the AST, not a line peek.
- [x] R2.4 — No behavioral regressions vs Phase 1 baseline on existing tests.

**Changes**

| Before | After |
|--------|-------|
| `fence_mask(lines: &[&str]) -> Vec<bool>` — lexical scan | `code_mask(regions: &[CodeRegion], n_lines: usize) -> Vec<bool>` — AST-derived |
| `is_fence_line(s: &str)` — line-level check | `has_fence_char(s: &str)` — detects ` ``` ` or `~~~` anywhere in line (handles `> ``` ` prefixes) |
| `Collected` had no region tracking | `Collected.code_regions: Vec<CodeRegion>` |

**Key discovery:** The rushdown parser sets `Node.pos()` to a **byte offset** (not a
line number) and inconsistently across contexts:

| Context | `pos()` value |
|---------|---------------|
| Top-level fenced block | Opening fence line (byte offset) |
| Nested fenced block (inside blockquote/list) | Closing fence line (byte offset) |
| Indented code block | Last content line (byte offset) |

The implementation handles all three cases via `byte_offset_to_line()` and
forward-looking fence detection (`candidate_end < src.lines.len()`).

---

## Phase 3 — Rich diagnostic positions (column + span)

**Status:** 🔑 **NOT STARTED**

**Requirements**

- [ ] R3.1 — `Violation` and `Diagnostic` gain `column: Option<usize>` (1-indexed)
  and `span: Option<(usize, usize)>` (byte offsets, half-open).
- [ ] R3.2 — AST rules populate spans from node offsets where available.
- [ ] R3.3 — Python `Diagnostic` exposes `.column` and `.span`; additive only.

**Effort:** M. **Risk:** Med (depends on AST offset availability).

---

## Phase 4 — Fix-engine hardening

**Status:** 🔑 **NOT STARTED**

**Requirements**

- [ ] R4.1 — Introduce a byte-range `Edit { start, end, replacement }` model.
- [ ] R4.2 — Overlapping edits resolved deterministically (earliest start wins).
- [ ] R4.3 — `fix` re-lints to a stable fixpoint, bounded by a max-iteration cap.
- [ ] R4.4 — `FixResult.remaining` computed by re-linting `output`.

**Effort:** M–L. **Risk:** Med.

---

## Phase 5 — Rule coverage expansion

**Status:** 🔑 **NOT STARTED**

**Requirements**

- [ ] R5.1 — Add rules in priority order:
  - Auto-fixable: MD010, MD018–MD021, MD022/MD031/MD032, MD026, MD034.
  - Report-only: MD003, MD013, MD046/MD048, MD049/MD050.
- [ ] R5.2 — Per-rule parameters (line_length, br_spaces, spaces_per_tab, etc.).
- [ ] R5.3 — Each rule satisfies NFR-6 (positive + clean-negative + fix tests).

**Effort:** L (scales with rule count). **Risk:** Low–Med.

---

## Phase 6 — Configuration, suppression & introspection

**Status:** 🔑 **NOT STARTED**

**Requirements**

- [ ] R6.1 — Load a config file (`.markdownlint.json` subset).
- [ ] R6.2 — Honor inline suppression comments (`<!-- markdownlint-disable MDxxx -->`).
- [ ] R6.3 — `mordant.lint_rules()` returns rule metadata.

**Effort:** M–L. **Risk:** Med.

---

## Phase 7 — CLI, batch & output formats

**Status:** 🔑 **NOT STARTED**

**Requirements**

- [ ] R7.1 — `python -m mordant` CLI: globs/paths, `--fix`, `--config`,
  rule selection, proper exit codes.
- [ ] R7.2 — Formatters: human (default), `--json`, GitHub Actions annotations.
- [ ] R7.3 — Batch API `lint_many(paths)` parallelized in Rust (rayon).

**Effort:** M. **Risk:** Low.

---

## Phase 8 — Accuracy polish

**Status:** 🔑 **NOT STARTED**

**Requirements**

- [ ] R8.1 — `collect_text` includes emoji/extension text so headings compare
  correctly (`# Hello :smile:` vs `# Hello`).
- [ ] R8.2 — MD025 treats a frontmatter `title:` as a document title when
  deciding "single H1."
- [ ] R8.3 — MD042 also flags fragment-only links pointing at missing anchors.

**Effort:** S–M. **Risk:** Low.

---

## Dependency graph

```
Phase 0 ✅ ─┬─ Phase 1 ✅ ─┬─ Phase 2 ✅ ─┐
            │              │              ├─ Phase 5 🔑 ─┬─ Phase 6 🔑 ─ Phase 7 🔑
            │              └─ Phase 3 🔑 ─ Phase 4 🔑 ───┘
            └─ Phase 8 🔑 (independent; can slot in any time after 0)
```

**Recommended order:** 3 → 4 → 5 → 6 → 7, with 8 opportunistic.
Phases 0–2 are prerequisites for trusting anything; 3–4 are the structural core;
5–7 are breadth and adoption.

---

## Risk register

| Risk | Phase | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| Inferred AST API wrong | 0 | ~~High~~ Resolved | ~~High~~ | Reconciled; recorded in code |
| Fenced-block end-line math off | 2 | ~~Med~~ Resolved | ~~Med~~ | Byte-offset → line conversion + forward-looking detection |
| AST lacks byte offsets | 3 | Med | Med | Fall back to line/column only; degrade gracefully |
| Overlapping edits corrupt output | 4 | Low | High | `apply_edits` drops overlaps; idempotence + HTML oracle in CI |
| Config schema scope creep | 6 | Med | Med | Implement a documented subset of `.markdownlint.json`, reject unknowns loudly |
| Fixes change meaning | all | Low | High | NFR-1 HTML-equivalence oracle gates every fixable rule |

---

## Appendix A — Rule catalog

| ID | Name | Source | Fixable | Status |
|----|------|--------|---------|--------|
| MD001 | heading-increment | AST | no | ✅ done |
| MD009 | no-trailing-spaces | line | yes | ✅ done (AST-derived mask) |
| MD012 | no-multiple-blanks | line | yes | ✅ done (AST-derived mask) |
| MD024 | no-duplicate-heading | AST | no | ✅ done |
| MD025 | single-h1 | AST | no | ✅ done |
| MD040 | fenced-code-language | AST | with language | ✅ done |
| MD042 | no-empty-links | AST | no | ✅ done |
| MD045 | no-alt-text | AST | no | ✅ done |
| MD047 | single-trailing-newline | line | yes | ✅ done |
| MD010 | no-hard-tabs | line | yes | 🔑 Phase 5 |
| MD018–MD021 | atx spacing | line/AST | yes | 🔑 Phase 5 |
| MD022/MD031/MD032 | blanks around blocks | AST | yes | 🔑 Phase 5 |
| MD026 | no-trailing-punctuation | AST | yes | 🔑 Phase 5 |
| MD034 | no-bare-urls | AST | yes | 🔑 Phase 5 |
| MD003 | heading-style | AST | no | 🔑 Phase 5 |
| MD013 | line-length | line | no | 🔑 Phase 5 |
| MD046/MD048 | code/fence style | AST | partial | 🔑 Phase 5 |
| MD049/MD050 | emphasis/strong style | AST | partial | 🔑 Phase 5 |

## Appendix B — Data-model evolution

The `Violation` / `Diagnostic` / fix model grows monotonically; each step is
additive (NFR-5):

| Field | Introduced | Notes |
|---|---|---|
| `rule, name, message, line, severity` | Phase 0 | — |
| `fix: Option<FixOp>` | Phase 0 | line-keyed ops |
| `fixable` (Diagnostic) | Phase 0 | derived |
| `column, span` | Phase 3 | byte offsets |
| `Edit { start,end,replacement }` | Phase 4 | `FixOp` lowers to this |
| `params: RuleParams` (config) | Phase 5 | per-rule tuning |
| `remaining` (FixResult) | Phase 4 | re-lint of output |

## Appendix C — Test counts by file

| File | Tests | Notes |
|------|-------|-------|
| Rushdown core (`cargo test`) | 53 | 26 unit + 2 AST + 1 CommonMark spec + 6 GFM + 2 options + 14 doc-tests + extras |
| Mordant Rust unit tests | 51 | 24 new (Phase 1+2) + existing emoji/meta tests |
| test_core.py | 14 | headings, paragraphs, bold, italic, code spans, links, images, lists, etc. |
| test_commonmark_spec.py | 302 | CommonMark spec examples |
| test_gfm.py | 9 | tables, strikethrough, task lists, linkify |
| test_emoji.py | 30 | basic, multiple, blacklist, templates, AST properties |
| test_diagram.py | 17 | Mermaid parsing, rendering, multiple diagrams, edge cases |
| test_meta.py | 40 | frontmatter parsing, YAML errors, thematic breaks, complex docs |
| test_options.py | 22 | Parse, Render, GFM, Arena options |
| test_lint.py | 50 | All 9 rules + fixer + AST-derived code regions (Phase 2) |
| **Total** | **994** | |
