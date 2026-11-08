#![cfg(feature = "linter")]

//! Regression tests for the `mordant.fix` native crash (2026-11-08).
//!
//! Root cause: `run_fix_with_params` reused the initial `Arena` (byte offsets
//! into the original source) across fix iterations. After `apply_fixes`
//! changed the source length, the stale offsets were sliced via
//! `get_unchecked` — SIGSEGV in release, UB-check panic in debug.
//! It also applied the cumulative `all_fixed` set (stale line numbers) to the
//! already-fixed source instead of just the current iteration's fixes.
//!
//! The fix re-parses with the caller-supplied parser on every iteration and
//! applies only the current fixes. These tests use a 582-char synthetic that
//! crashed the old engine deterministically (verified via `git stash`) and
//! now fixes cleanly, plus the key invariants: no panic on multi-iteration
//! input, fixpoint stability, and no fixable diagnostics remaining.

use mordant::{parser, text};

fn synth_crash_doc() -> String {
    let mut parts: Vec<String> = Vec::new();
    for i in 0..45 {
        parts.push(format!("para {i}"));
        parts.push(String::new());
        parts.push(String::new());
        parts.push(String::new());
    }
    parts.push("### Corresponding author.".to_string());
    parts.push(String::new());
    parts.push("Some text with unicode: \u{3c3} \u{3bc} \u{3b7} \u{107} \u{10d} \u{2014} end.".to_string());
    parts.push(String::new());
    parts.push("[link](https://example.com)".to_string());
    parts.join("\n") + "\n"
}

fn default_parser() -> parser::Parser {
    parser::Parser::with_extensions(
        parser::Options::default(),
        parser::gfm(parser::GfmOptions::default()),
    )
}

#[test]
fn fix_multiline_shortening_does_not_crash() {
    let source = synth_crash_doc();
    let parser = default_parser();
    let cfg = mordant::linter::LintConfig::default();
    // Would panic (debug) / segfault (release) before the re-parse fix.
    let outcome = mordant::linter::run_fix_with_parser(&source, &parser, &cfg, None);
    // All fixable issues resolved.
    assert!(
        outcome.remaining.iter().all(|v| v.fix.is_none()),
        "remaining should hold no fixable diagnostics, got: {:?}",
        outcome.remaining.iter().map(|v| v.rule).collect::<Vec<_>>()
    );
    // Heading punctuation stripped, blank runs collapsed.
    assert!(outcome.output.contains("### Corresponding author\n"));
    assert!(!outcome.output.contains("\n\n\n"));
}

#[test]
fn legacy_run_fix_is_sound_after_shortening() {
    // The legacy arena-based entry point now re-parses after the first edit
    // instead of reusing the stale AST.
    let source = synth_crash_doc();
    let parser = default_parser();
    let mut reader = text::BasicReader::new(&source);
    let (arena, root) = parser.parse(&mut reader);
    let cfg = mordant::linter::LintConfig::default();
    let outcome = mordant::linter::run_fix(&source, &arena, root, &cfg, None);
    assert!(outcome.remaining.iter().all(|v| v.fix.is_none()));
}

#[test]
fn fix_is_stable_fixpoint() {
    let source = synth_crash_doc();
    let parser = default_parser();
    let cfg = mordant::linter::LintConfig::default();
    let once = mordant::linter::run_fix_with_parser(&source, &parser, &cfg, None);
    let twice = mordant::linter::run_fix_with_parser(&once.output, &parser, &cfg, None);
    assert_eq!(once.output, twice.output, "fix(fix(x)) != fix(x)");
    assert!(twice.fixed.is_empty(), "second fix should find nothing to fix");
}
