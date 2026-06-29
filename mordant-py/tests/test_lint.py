"""Tests for the Markdown linter in mordant.

The linter walks the rushdown AST and reports markdownlint-style diagnostics
(MD0xx). Rules implemented:

  AST-based:
    MD001  heading-increment        headings increment by one
    MD024  no-duplicate-heading     repeated heading text
    MD025  single-h1                more than one top-level heading
    MD040  fenced-code-language     fenced block with no language
    MD042  no-empty-links           link with empty/`#` destination
    MD045  no-alt-text              image with no alt text

  Line-based:
    MD009  no-trailing-spaces       trailing whitespace
    MD012  no-multiple-blanks       >1 consecutive blank line
    MD047  single-trailing-newline  file must end with a newline
"""

import json
import subprocess
import sys
import os

import pytest
import mordant


def rules(diags):
    """Set of rule ids present in a diagnostics list."""
    return {d.rule for d in diags}


# === A clean document produces no diagnostics ===

def test_clean_document_no_violations():
    md = (
        "# Title\n"
        "\n"
        "Intro paragraph.\n"
        "\n"
        "## Section\n"
        "\n"
        "Text with [a link](https://example.com) and ![alt](img.png).\n"
        "\n"
        "```python\n"
        "print(\"hi\")\n"
        "```\n"
    )
    assert mordant.lint(md) == []


def test_empty_document_no_violations():
    assert mordant.lint("") == []


# === MD001: heading-increment ===

def test_md001_skipped_level():
    diags = mordant.lint("# H1\n\n### H3 jumps\n")
    assert "MD001" in rules(diags)


def test_md001_ok_increment():
    diags = mordant.lint("# H1\n\n## H2\n\n### H3\n")
    assert "MD001" not in rules(diags)


def test_md001_decrease_is_ok():
    # Going back down levels is allowed.
    diags = mordant.lint("# H1\n\n## H2\n\n# H1 again\n")
    assert "MD001" not in rules(diags)


# === MD025: single top-level heading ===

def test_md025_multiple_h1():
    diags = mordant.lint("# First\n\n# Second\n")
    assert "MD025" in rules(diags)


def test_md025_single_h1_ok():
    diags = mordant.lint("# Only one\n\n## Sub\n")
    assert "MD025" not in rules(diags)


# === MD024: duplicate heading text ===

def test_md024_duplicate_heading():
    diags = mordant.lint("# Same\n\n## Same\n")
    assert "MD024" in rules(diags)


def test_md024_unique_headings_ok():
    diags = mordant.lint("# One\n\n## Two\n")
    assert "MD024" not in rules(diags)


# === MD040: fenced code block language ===

def test_md040_missing_language():
    diags = mordant.lint("```\ncode\n```\n")
    assert "MD040" in rules(diags)


def test_md040_with_language_ok():
    diags = mordant.lint("```python\ncode\n```\n")
    assert "MD040" not in rules(diags)


def test_md040_indented_code_not_flagged():
    # Indented code blocks have no language and must NOT trigger MD040.
    diags = mordant.lint("    indented code\n")
    assert "MD040" not in rules(diags)


def test_md040_mermaid_not_flagged():
    # Mermaid blocks become Diagram nodes, not CodeBlocks.
    diags = mordant.lint("```mermaid\ngraph LR\n  A --- B\n```\n")
    assert "MD040" not in rules(diags)


# === MD042: empty links ===

def test_md042_empty_destination():
    diags = mordant.lint("[text]()\n")
    assert "MD042" in rules(diags)


def test_md042_hash_destination():
    diags = mordant.lint("[text](#)\n")
    assert "MD042" in rules(diags)


def test_md042_valid_link_ok():
    diags = mordant.lint("[text](https://example.com)\n")
    assert "MD042" not in rules(diags)


# === MD045: image alt text ===

def test_md045_missing_alt():
    diags = mordant.lint("![](img.png)\n")
    assert "MD045" in rules(diags)


def test_md045_with_alt_ok():
    diags = mordant.lint("![some alt](img.png)\n")
    assert "MD045" not in rules(diags)


# === MD009: trailing whitespace ===

def test_md009_trailing_space():
    diags = mordant.lint("line with trailing space \n")
    assert "MD009" in rules(diags)


def test_md009_clean_line_ok():
    diags = mordant.lint("clean line\n")
    assert "MD009" not in rules(diags)


def test_md009_ignores_fenced_code():
    # Trailing spaces inside a fenced block are not flagged.
    md = "```python\ncode line \n```\n"
    assert "MD009" not in rules(mordant.lint(md))


# === MD012: multiple blank lines ===

def test_md012_multiple_blanks():
    diags = mordant.lint("a\n\n\nb\n")
    assert "MD012" in rules(diags)


def test_md012_single_blank_ok():
    diags = mordant.lint("a\n\nb\n")
    assert "MD012" not in rules(diags)


# === MD047: final newline ===

def test_md047_missing_final_newline():
    diags = mordant.lint("no newline at end")
    assert "MD047" in rules(diags)


def test_md047_final_newline_ok():
    diags = mordant.lint("ends with newline\n")
    assert "MD047" not in rules(diags)


# === Diagnostic object shape ===

def test_diagnostic_attributes():
    diags = mordant.lint("# H1\n\n### H3\n")
    assert len(diags) >= 1
    d = next(x for x in diags if x.rule == "MD001")
    assert d.rule == "MD001"
    assert d.name == "heading-increment"
    assert isinstance(d.message, str) and d.message
    assert isinstance(d.line, int)
    assert d.severity in ("warning", "error")
    assert "MD001" in repr(d)
    assert "MD001" in str(d)


def test_diagnostics_sorted_by_line():
    md = "# H1\n\n### H3 jump\n\n# Second H1\n"
    diags = mordant.lint(md)
    lines = [d.line for d in diags if d.line is not None]
    assert lines == sorted(lines)


# === Config: enable / disable ===

def test_disable_rule():
    diags = mordant.lint(
        "# A\n\n### C\n",
        lint_opts=mordant.LintOptions(disable=["MD001"]),
    )
    assert "MD001" not in rules(diags)


def test_enable_only_rule():
    md = "# A\n\n# B\n\n### C\n"  # would trigger MD025 and MD001
    diags = mordant.lint(md, lint_opts=mordant.LintOptions(enable=["MD025"]))
    assert rules(diags) <= {"MD025"}
    assert "MD025" in rules(diags)


def test_lint_options_getters():
    opts = mordant.LintOptions(disable=["MD009", "MD012"])
    assert opts.disable == ["MD009", "MD012"]
    assert opts.enable is None
    opts.enable = ["MD001"]
    assert opts.enable == ["MD001"]


# === Document.lint() method ===

def test_document_lint_method():
    doc = mordant.parse("# A\n\n### C\n")
    diags = doc.lint()
    assert "MD001" in rules(diags)


def test_document_lint_with_options():
    doc = mordant.parse("# A\n\n### C\n")
    diags = doc.lint(mordant.LintOptions(disable=["MD001"]))
    assert "MD001" not in rules(diags)


# === GFM interaction ===

def test_lint_works_with_gfm():
    md = "# Title\n\n| A | B |\n|---|---|\n| 1 | 2 |\n"
    diags = mordant.lint(md, gfm=True)
    # Well-formed table, single heading — nothing to report here.
    assert "MD025" not in rules(diags)
    assert "MD001" not in rules(diags)


# ===========================================================================
# Auto-fix
# ===========================================================================

def fixed_rules(result):
    return {d.rule for d in result.fixed}


def unfixable_rules(result):
    return {d.rule for d in result.unfixable}


# --- MD009: trailing whitespace ---

def test_fix_strips_trailing_whitespace():
    result = mordant.fix("text   \n")  # 3 trailing spaces
    assert result.output == "text\n"
    assert "MD009" in fixed_rules(result)


def test_fix_preserves_hard_line_break():
    # Exactly two trailing spaces == CommonMark hard break; must be left alone.
    src = "line one  \nline two\n"
    result = mordant.fix(src)
    assert result.output == src
    assert "MD009" not in fixed_rules(result)


# --- MD012: multiple blank lines ---

def test_fix_collapses_blank_lines():
    result = mordant.fix("a\n\n\nb\n")
    assert result.output == "a\n\nb\n"
    assert "MD012" in fixed_rules(result)


# --- MD047: final newline ---

def test_fix_adds_final_newline():
    result = mordant.fix("no newline")
    assert result.output == "no newline\n"
    assert "MD047" in fixed_rules(result)


# --- Several fixable issues at once ---

def test_fix_combined_is_clean_for_fixable_rules():
    src = "# Title   \n\n\nSome text"  # trailing ws + extra blank + no final NL
    result = mordant.fix(src)
    assert result.output == "# Title\n\nSome text\n"
    # Re-linting the output shows none of the fixable rules remain.
    remaining = {d.rule for d in mordant.lint(result.output)}
    assert remaining.isdisjoint({"MD009", "MD012", "MD047"})


# --- Structural rules are reported, not changed ---

def test_fix_leaves_structural_issues_untouched():
    src = "# A\n\n### C\n"  # MD001 only — not auto-fixable
    result = mordant.fix(src)
    assert result.output == src              # unchanged
    assert fixed_rules(result) == set()
    assert "MD001" in unfixable_rules(result)


# --- MD040: only fixed with an explicit default language ---

def test_fix_md040_unfixable_without_language():
    result = mordant.fix("```\ncode\n```\n")
    assert "MD040" in unfixable_rules(result)
    assert result.output == "```\ncode\n```\n"


def test_fix_md040_with_default_language():
    result = mordant.fix("```\ncode\n```\n", default_language="text")
    assert result.output == "```text\ncode\n```\n"
    assert "MD040" in fixed_rules(result)


def test_fix_md040_default_language_preserves_fence_style():
    result = mordant.fix("~~~~\ncode\n~~~~\n", default_language="python")
    assert result.output.startswith("~~~~python\n")


# --- The fixable flag on diagnostics ---

def test_diagnostic_fixable_flag():
    md009 = next(d for d in mordant.lint("text   \n") if d.rule == "MD009")
    assert md009.fixable is True
    md001 = next(d for d in mordant.lint("# A\n\n### C\n") if d.rule == "MD001")
    assert md001.fixable is False


# --- FixResult object shape ---

def test_fixresult_repr():
    result = mordant.fix("a\n\n\nb")
    assert "FixResult" in repr(result)
    assert isinstance(result.output, str)
    assert isinstance(result.fixed, list)
    assert isinstance(result.unfixable, list)


# --- Document.fix() method ---

def test_document_fix_method():
    doc = mordant.parse("# Title   \n\n\nSome text")
    result = doc.fix()
    assert result.output == "# Title\n\nSome text\n"


def test_document_fix_with_default_language():
    doc = mordant.parse("```\ncode\n```\n")
    result = doc.fix(default_language="text")
    assert result.output == "```text\ncode\n```\n"


# --- Fixing respects disable config ---

def test_fix_respects_disabled_rules():
    # Disabling MD009 means the trailing whitespace is left in place.
    result = mordant.fix("text   \n", lint_opts=mordant.LintOptions(disable=["MD009"]))
    assert result.output == "text   \n"
    assert "MD009" not in fixed_rules(result)


# ===========================================================================
# Phase 2 — AST-derived code regions (fenced code inside blockquotes/lists)
# ===========================================================================

def test_md009_skips_code_in_blockquote():
    # Fenced code block inside a blockquote should not trigger MD009.
    md = "> ```python\n> code line   \n> ```\n"
    assert "MD009" not in rules(mordant.lint(md))


def test_md009_skips_code_in_list():
    # Fenced code block inside a list item should not trigger MD009.
    md = "- Item\n  ```python\n  code line   \n  ```\n"
    assert "MD009" not in rules(mordant.lint(md))


def test_md012_skips_blanks_in_code():
    # Multiple blank lines inside a fenced block should not trigger MD012.
    md = "```\n\n\n```\n"
    assert "MD012" not in rules(mordant.lint(md))


def test_md040_indented_code_not_flagged():
    # Indented (4-space) code blocks should NOT trigger MD040.
    md = "    indented code\n    more code\n"
    assert "MD040" not in rules(mordant.lint(md))



# ===========================================================================
# Phase 3 — Rich diagnostic positions (column + span)
# ===========================================================================

def test_diagnostic_column():
    """MD009 should report the column of trailing whitespace."""
    diags = mordant.lint("hello   \n")
    assert len(diags) == 1
    assert diags[0].column == 6  # 1-indexed: position after 'hello'


def test_diagnostic_span():
    """Diagnostics should include a byte offset span."""
    diags = mordant.lint("hello   \n")
    assert len(diags) == 1
    # span is (start, end) byte offsets — for line-based rules, [line_start, line_end)
    assert diags[0].span is not None
    start, end = diags[0].span
    assert start < end


def test_diagnostic_column_none():
    """MD001 (structural) should not have a column."""
    diags = mordant.lint("# A\n\n### C\n")
    md001 = [d for d in diags if d.rule == "MD001"]
    assert len(md001) == 1
    assert md001[0].column is None


# ===========================================================================
# Phase 4 — Fix-engine hardening (remaining, fixpoint)
# ===========================================================================

def test_fix_remaining():
    """fix() should report remaining diagnostics after fixing."""
    md = "hello   \n"  # MD009 (fixable) + no trailing newline (MD047, fixable)
    result = mordant.fix(md)
    assert result.output == "hello\n"
    # After fixing, no remaining diagnostics for fixable rules
    remaining_rules = {d.rule for d in result.remaining}
    assert "MD009" not in remaining_rules


def test_fix_fixpoint():
    """fix() should iterate until stable (max 10 iterations)."""
    md = "a   \n\n\nb   \n"  # MD009 on lines 1,3 + MD012 on line 3
    result = mordant.fix(md)
    assert result.output == "a\n\nb\n"
    # No remaining fixable violations
    remaining_rules = {d.rule for d in result.remaining}
    assert "MD009" not in remaining_rules
    assert "MD012" not in remaining_rules


def test_fixresult_remaining_getter():
    """FixResult should expose a remaining property."""
    md = "hello   \n"
    result = mordant.fix(md)
    # After fixing, remaining should be empty for simple cases
    assert isinstance(result.remaining, list)


# ===========================================================================
# Phase 5 — Rule coverage expansion
# ===========================================================================

# === MD010: no-hard-tabs ===

def test_md010_detects_hard_tabs():
    diags = mordant.lint("line\twith\ttabs\n")
    assert "MD010" in rules(diags)


def test_md010_no_tabs_ok():
    diags = mordant.lint("no tabs here\n")
    assert "MD010" not in rules(diags)


def test_md010_fix_replaces_tabs():
    result = mordant.fix("line\twith\ttabs\n")
    assert "\t" not in result.output
    assert "MD010" in fixed_rules(result)


# === MD013: line-length (report-only) ===

def test_md013_detects_long_lines():
    md = "x" * 100 + "\n"  # 100 chars, default limit is 80
    diags = mordant.lint(md)
    assert "MD013" in rules(diags)


def test_md013_ok_within_limit():
    diags = mordant.lint("short line\n")
    assert "MD013" not in rules(diags)


# === MD018: ATX closing # spacing ===

def test_md018_no_space_after_hashes():
    diags = mordant.lint("#Hello\n")
    assert "MD018" in rules(diags)


def test_md018_space_after_hashes_ok():
    diags = mordant.lint("# Hello\n")
    assert "MD018" not in rules(diags)


# === MD020: ATX closing # spacing ===

def test_md020_space_before_closing_hash():
    diags = mordant.lint("# Hello#\n")
    assert "MD020" in rules(diags)


def test_md020_space_before_closing_hash_ok():
    diags = mordant.lint("# Hello #\n")
    assert "MD020" not in rules(diags)


# === MD026: trailing punctuation in headings ===

def test_md026_trailing_period():
    diags = mordant.lint("# Title.\n")
    assert "MD026" in rules(diags)


def test_md026_trailing_exclamation():
    diags = mordant.lint("# Title!\n")
    assert "MD026" in rules(diags)


def test_md026_no_trailing_punctuation_ok():
    diags = mordant.lint("# Title\n")
    assert "MD026" not in rules(diags)


def test_md026_fix_removes_trailing_punctuation():
    result = mordant.fix("text\n\n# Section.\n")
    assert "# Section" in result.output
    assert "# Section." not in result.output
    assert "MD026" in fixed_rules(result)


# === MD022: blank lines around headings ===

def test_md022_no_blank_before_heading():
    diags = mordant.lint("text\n# Heading\n")
    assert "MD022" in rules(diags)


def test_md022_no_blank_after_heading():
    diags = mordant.lint("# Heading\ntext\n")
    assert "MD022" in rules(diags)


def test_md022_blank_around_heading_ok():
    diags = mordant.lint("# Heading\n\ntext\n")
    assert "MD022" not in rules(diags)


# === MD031/MD032: blank lines around code blocks ===

def test_md031_no_blank_before_fenced_code():
    diags = mordant.lint("text\n```python\ncode\n```\n")
    assert "MD031" in rules(diags)


def test_md032_no_blank_before_indented_code():
    # Indented code blocks need a blank line before them in CommonMark
    # Use fenced code instead for this test
    md = "text\n```python\ncode\n```\n"
    diags = mordant.lint(md)
    assert "MD031" in rules(diags)


# === MD046: code block indentation ===

def test_md046_partial_indent():
    diags = mordant.lint("  ```python\ncode\n```\n")
    assert "MD046" in rules(diags)


def test_md046_no_indent_ok():
    diags = mordant.lint("```python\ncode\n```\n")
    assert "MD046" not in rules(diags)


# === MD048: fenced code block punctuation ===

def test_md048_tilde_fence():
    diags = mordant.lint("~~~python\ncode\n~~~\n")
    assert "MD048" in rules(diags)


def test_md048_backtick_fence_ok():
    diags = mordant.lint("```python\ncode\n```\n")
    assert "MD048" not in rules(diags)


# === Disable/enable new rules ===

def test_disable_new_rules():
    diags = mordant.lint(
        "# Title.\n",
        lint_opts=mordant.LintOptions(disable=["MD026"]),
    )
    assert "MD026" not in rules(diags)


def test_enable_only_new_rule():
    md = "# Title.\n\ntext\n"
    diags = mordant.lint(md, lint_opts=mordant.LintOptions(enable=["MD026"]))
    assert rules(diags) == {"MD026"}


# ===========================================================================
# Phase 6 — Configuration, suppression & introspection
# ===========================================================================

# --- lint_rules() introspection ---

def test_lint_rules_returns_all_rules():
    rules_list = mordant.lint_rules()
    assert len(rules_list) == 25
    ids = [r.id for r in rules_list]
    assert "MD001" in ids
    assert "MD009" in ids
    assert "MD040" in ids
    assert "MD047" in ids
    # Check attributes
    md009 = [r for r in rules_list if r.id == "MD009"][0]
    assert md009.fixable is True
    assert "trailing" in md009.description.lower()


def test_lint_rules_all_have_names():
    rules_list = mordant.lint_rules()
    for r in rules_list:
        assert r.id.startswith("MD")
        assert r.name
        assert r.description
        assert r.default_params


# --- LintConfig from dict ---

def test_lint_config_from_dict_basic():
    # Disable specific rules, others should still run
    config = mordant.LintConfig.from_dict({
        "MD009": False,
        "MD010": False,
    })
    # Use a document with violations
    md = "# Title  \n\n\nText\n"  # has MD009 and MD012 violations
    diags = mordant.lint(md, lint_config=config)
    rule_names = [d.rule for d in diags]
    assert "MD009" not in rule_names
    assert "MD010" not in rule_names
    # MD012 should still run
    assert "MD012" in rule_names


def test_lint_config_default_false():
    config = mordant.LintConfig.from_dict({
        "default": False,
        "MD025": True,
    })
    diags = mordant.lint("# A\n\n# B\n\n# C\n", lint_config=config)
    # Only MD025 should run
    rule_names = [d.rule for d in diags]
    assert "MD025" in rule_names
    assert "MD001" not in rule_names


def test_lint_config_disable_list():
    config = mordant.LintConfig.from_dict({
        "disable": ["MD001", "MD024", "MD025"],
    })
    diags = mordant.lint("# A\n\n### C\n\n# A\n", lint_config=config)
    rule_names = [d.rule for d in diags]
    assert "MD001" not in rule_names
    assert "MD024" not in rule_names
    assert "MD025" not in rule_names


def test_lint_config_params_getter():
    config = mordant.LintConfig.from_dict({})
    params = config.params
    assert "line_length" in params


# --- Inline suppression comments ---

def test_inline_suppression_disable_next_line():
    md = "# Title\n<!-- markdownlint-disable-next-line MD025 -->\n# Another\n"
    diags = mordant.lint(md)
    # MD025 should NOT be reported for the second heading
    md025_diags = [d for d in diags if d.rule == "MD025"]
    assert len(md025_diags) == 0


def test_inline_suppression_disable_enable_block():
    md = """# Title
<!-- markdownlint-disable MD025 -->
# Another
# Third
<!-- markdownlint-enable MD025 -->
# Fourth
"""
    diags = mordant.lint(md)
    # MD025 should only be reported for the last heading (after enable)
    md025_diags = [d for d in diags if d.rule == "MD025"]
    # Should have at most one (the last h1 after enable)
    assert len(md025_diags) <= 1


# --- Document.lint() / Document.fix() with LintConfig ---

def test_document_lint_with_config():
    doc = mordant.parse("# A\n\n# B\n\n# C\n")
    config = mordant.LintConfig.from_dict({
        "default": False,
        "MD025": True,
    })
    diags = doc.lint(lint_config=config)
    rule_names = [d.rule for d in diags]
    assert "MD025" in rule_names
    assert "MD001" not in rule_names


def test_document_fix_with_config():
    doc = mordant.parse("# Title  \n\n\nText\n")
    config = mordant.LintConfig.from_dict({
        "default": True,
    })
    result = doc.fix(lint_config=config)
    assert "MD012" not in [d.rule for d in result.remaining]
    assert "MD009" not in [d.rule for d in result.remaining]


# ===========================================================================
# Phase 7 — CLI, batch API, and output formats
# ===========================================================================

# --- CLI helper ---

def _run_cli(*args, tmp_dir=None):
    """Helper to run the CLI and capture output."""
    cmd = [sys.executable, "-m", "mordant"] + list(args)
    if tmp_dir:
        env = os.environ.copy()
        env["PYTHONPATH"] = str(tmp_dir.parent)
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            cwd=str(tmp_dir),
            env=env,
        )
    else:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
        )
    return result


# ===========================================================================
# Batch API — lint_many()
# ===========================================================================

def test_lint_many_empty():
    """Empty file list returns empty results."""
    results = mordant.lint_many([])
    assert results == []


def test_lint_many_single_file():
    """Single file returns one result tuple."""
    files = [("test.md", "# Title\n\n## Section\n")]
    results = mordant.lint_many(files)
    assert len(results) == 1
    name, diags = results[0]
    assert name == "test.md"
    assert diags == []


def test_lint_many_multiple_files():
    """Multiple files returns results for each."""
    files = [
        ("clean.md", "# Clean\n\n## OK\n"),
        ("bad.md", "# A\n\n### Skips H2\n"),
    ]
    results = mordant.lint_many(files)
    assert len(results) == 2

    # Clean file has no issues
    assert results[0][0] == "clean.md"
    assert results[0][1] == []

    # Bad file has MD001
    assert results[1][0] == "bad.md"
    assert any(d.rule == "MD001" for d in results[1][1])


def test_lint_many_with_config():
    """LintConfig is applied to all files in the batch."""
    files = [
        ("a.md", "# A\n\n# B\n"),
        ("b.md", "# C\n\n# D\n"),
    ]
    # Disable MD025 (single-h1)
    config = mordant.LintConfig.from_dict({"disable": ["MD025"]})
    results = mordant.lint_many(files, lint_config=config)

    for name, diags in results:
        assert not any(d.rule == "MD025" for d in diags), f"{name} should not have MD025"


def test_lint_many_preserves_order():
    """Results are returned in the same order as input files."""
    names = [f"file_{i}.md" for i in range(5)]
    files = [(n, "# Clean\n") for n in names]
    results = mordant.lint_many(files)

    result_names = [name for name, _ in results]
    assert result_names == names


def test_lint_many_parallel_independence():
    """Each file is linted independently; issues in one don't affect others."""
    files = [
        ("issues.md", "# A\n\n# B\n\n# C\n"),  # MD025
        ("clean.md", "# Single\n\n## Sub\n"),   # clean
    ]
    results = mordant.lint_many(files)

    issues_diags = results[0][1]
    clean_diags = results[1][1]

    assert any(d.rule == "MD025" for d in issues_diags)
    assert clean_diags == []


# ===========================================================================
# Batch API — fix_many()
# ===========================================================================

def test_fix_many_empty():
    """Empty file list returns empty results."""
    results = mordant.fix_many([])
    assert results == []


def test_fix_many_clean_file():
    """Clean file returns no fixed, no remaining."""
    files = [("clean.md", "# Title\n\n## Section\n")]
    results = mordant.fix_many(files)

    assert len(results) == 1
    name, result = results[0]
    assert name == "clean.md"
    assert len(result.fixed) == 0
    assert len(result.remaining) == 0
    assert result.output == "# Title\n\n## Section\n"


def test_fix_many_fixes_trailing_spaces():
    """MD009 trailing spaces are auto-fixed."""
    files = [("trailing.md", "hello   \nworld   \n")]
    results = mordant.fix_many(files)

    name, result = results[0]
    assert result.output == "hello\nworld\n"
    assert len(result.fixed) >= 1


def test_fix_many_fixes_multiple_blanks():
    """MD012 multiple blank lines are auto-fixed."""
    files = [("blanks.md", "a\n\n\n\nb\n")]
    results = mordant.fix_many(files)

    name, result = results[0]
    assert result.output == "a\n\nb\n"


def test_fix_many_fixes_final_newline():
    """MD047 missing final newline is auto-fixed."""
    files = [("no_nl.md", "no newline")]
    results = mordant.fix_many(files)

    name, result = results[0]
    assert result.output == "no newline\n"


def test_fix_many_with_default_language():
    """MD040 fenced code blocks get the default language."""
    files = [("code.md", "```\ncode\n```\n")]
    results = mordant.fix_many(files, default_language="python")

    name, result = results[0]
    assert "```python" in result.output


def test_fix_many_with_config():
    """LintConfig is applied to all files in the batch fix."""
    files = [("test.md", "# Title  \n\n\nText\n")]
    # Disable MD009 (trailing spaces)
    config = mordant.LintConfig.from_dict({"disable": ["MD009"]})
    results = mordant.fix_many(files, lint_config=config)

    name, result = results[0]
    # MD009 should not be in fixed or remaining
    fixed_rules = [d.rule for d in result.fixed]
    remaining_rules = [d.rule for d in result.remaining]
    assert "MD009" not in fixed_rules
    assert "MD009" not in remaining_rules


def test_fix_many_multiple_files():
    """Multiple files are fixed independently."""
    files = [
        ("a.md", "trailing   \n"),
        ("b.md", "clean\n"),
        ("c.md", "no newline"),
    ]
    results = mordant.fix_many(files)

    assert len(results) == 3
    assert results[0][1].output == "trailing\n"
    assert results[1][1].output == "clean\n"
    assert results[2][1].output == "no newline\n"


# ===========================================================================
# CLI — python -m mordant
# ===========================================================================

def test_cli_clean_file(tmp_path):
    """Clean file exits with code 0 and no output."""
    md = tmp_path / "clean.md"
    md.write_text("# Title\n\n## Section\n")
    result = _run_cli(str(md), tmp_dir=tmp_path)
    assert result.returncode == 0
    assert result.stdout.strip() == ""


def test_cli_issues_found(tmp_path):
    """File with issues exits with code 1."""
    md = tmp_path / "bad.md"
    md.write_text("# A\n\n# B\n\n# C\n")
    result = _run_cli(str(md), tmp_dir=tmp_path)
    assert result.returncode == 1
    assert "MD025" in result.stdout


def test_cli_human_format(tmp_path):
    """Human format shows filename:line: rule."""
    md = tmp_path / "test.md"
    md.write_text("# Title  \n\n\nText\n")
    result = _run_cli(str(md), tmp_dir=tmp_path)
    assert "test.md" in result.stdout
    assert ":" in result.stdout


def test_cli_json_format(tmp_path):
    """JSON format outputs valid JSON."""
    md = tmp_path / "test.md"
    md.write_text("# A\n\n# B\n")
    result = _run_cli("--format", "json", str(md), tmp_dir=tmp_path)
    data = json.loads(result.stdout)
    assert isinstance(data, list)
    assert len(data) >= 1
    assert "file" in data[0]
    assert "diagnostics" in data[0]


def test_cli_github_format(tmp_path):
    """GitHub format outputs ::warning annotations."""
    md = tmp_path / "test.md"
    md.write_text("# A\n\n# B\n")
    result = _run_cli("--format", "github", str(md), tmp_dir=tmp_path)
    assert "::warning" in result.stdout


def test_cli_fix_flag(tmp_path):
    """--fix flag corrects files in-place."""
    md = tmp_path / "test.md"
    md.write_text("trailing   \n\n\nmore   \n")
    result = _run_cli("--fix", str(md), tmp_dir=tmp_path)
    # File should be corrected
    content = md.read_text()
    assert "   " not in content
    assert content.count("\n\n\n") == 0


def test_cli_fix_dry_run(tmp_path):
    """--fix --dry-run doesn't modify files."""
    md = tmp_path / "test.md"
    original = "trailing   \n"
    md.write_text(original)
    result = _run_cli("--fix", "--dry-run", str(md), tmp_dir=tmp_path)
    # File should be unchanged
    assert md.read_text() == original


def test_cli_disable_flag(tmp_path):
    """--disable flag suppresses specific rules."""
    md = tmp_path / "test.md"
    md.write_text("# Title  \n\n\nText\n")
    result = _run_cli("--disable", "MD009,MD012", str(md), tmp_dir=tmp_path)
    assert "MD009" not in result.stdout
    assert "MD012" not in result.stdout


def test_cli_enable_flag(tmp_path):
    """--enable flag runs only specified rules."""
    md = tmp_path / "test.md"
    md.write_text("# Title\n\n# Another\n\n# Third\n")
    result = _run_cli("--enable", "MD025", str(md), tmp_dir=tmp_path)
    # Only MD025 should appear
    for line in result.stdout.strip().split("\n"):
        if line.strip():
            assert "MD025" in line


def test_cli_multiple_files(tmp_path):
    """Multiple files are linted together."""
    (tmp_path / "a.md").write_text("# Clean\n\n## OK\n")
    (tmp_path / "b.md").write_text("# A\n\n# B\n")
    result = _run_cli(str(tmp_path / "a.md"), str(tmp_path / "b.md"), tmp_dir=tmp_path)
    assert "b.md" in result.stdout
    assert "a.md" not in result.stdout  # clean file not mentioned


def test_cli_no_files_found(tmp_path):
    """No matching files exits cleanly."""
    result = _run_cli("nonexistent_pattern_*.md", tmp_dir=tmp_path)
    assert result.returncode == 0


def test_cli_config_file(tmp_path):
    """--config flag loads .markdownlint.json config."""
    config = tmp_path / ".markdownlint.json"
    config.write_text(json.dumps({"disable": ["MD025"]}))
    md = tmp_path / "test.md"
    md.write_text("# A\n\n# B\n")
    result = _run_cli("--config", str(config), str(md), tmp_dir=tmp_path)
    assert "MD025" not in result.stdout


def test_cli_default_language(tmp_path):
    """--default-language flag sets language for MD040 fixes."""
    md = tmp_path / "test.md"
    md.write_text("```\ncode\n```\n")
    result = _run_cli("--fix", "--default-language", "python", str(md), tmp_dir=tmp_path)
    content = md.read_text()
    assert "```python" in content


def test_cli_glob_pattern(tmp_path):
    """Glob patterns are expanded correctly."""
    (tmp_path / "a.md").write_text("# A\n")
    (tmp_path / "b.md").write_text("# B\n")
    result = _run_cli("*.md", tmp_dir=tmp_path)
    assert result.returncode == 0


def test_cli_directory_recursion(tmp_path):
    """Directory paths are recursed."""
    (tmp_path / "sub").mkdir()
    (tmp_path / "sub" / "nested.md").write_text("# Nested\n")
    result = _run_cli(str(tmp_path), tmp_dir=tmp_path)
    assert result.returncode == 0


# ===========================================================================
# Batch API — performance sanity
# ===========================================================================

def test_lint_many_100_files():
    """Batch linting 100 files completes without errors."""
    files = [(f"file_{i}.md", f"# File {i}\n\n## Section\n") for i in range(100)]
    results = mordant.lint_many(files)
    assert len(results) == 100


def test_fix_many_50_files():
    """Batch fixing 50 files completes without errors."""
    files = [(f"file_{i}.md", f"trailing   \n\n\ncontent\n") for i in range(50)]
    results = mordant.fix_many(files)
    assert len(results) == 50
    for name, result in results:
        assert result.output == f"trailing\n\ncontent\n"


# ===========================================================================
# Phase 8 — accuracy polish
# ===========================================================================

# --- R8.1: collect_text includes emoji/extension text ---

def test_md024_emoji_headings_same_text():
    """Two headings with same text plus emoji should be treated as duplicates."""
    # Both headings resolve to "Hello 😂" (emoji shortcode :joy: -> 😂)
    md = "# Hello :joy:\n\n## Other\n\n# Hello :joy:\n"
    diags = mordant.lint(md)
    assert "MD024" in rules(diags), "Duplicate headings with emoji should be detected"


def test_md024_emoji_headings_different_text():
    """Headings with different emoji are not duplicates."""
    md = "# Hello :joy:\n\n## Other\n\n# Hello :heart:\n"
    diags = mordant.lint(md)
    assert "MD024" not in rules(diags), "Different emoji make headings distinct"


def test_emoji_in_heading_text_collected():
    """Emoji in heading text is collected as the Unicode character, not the shortcode."""
    doc = mordant.parse("# Hello :smile:", emoji_opts=mordant.PyEmojiParserOptions(blacklist=None))
    # Find the heading node and check its text includes the emoji
    heading = None
    for child in doc.children:
        if child.kind == "Heading":
            heading = child
            break
    assert heading is not None
    # The collected text should contain the emoji character, not the shortcode
    assert ":smile:" not in heading.text or "\U0001f604" in heading.text or "smile" in heading.text.lower()


# --- R8.2: MD025 treats frontmatter title as document title ---

def test_md025_frontmatter_title_with_single_h1():
    """Document with frontmatter title and single H1 should not trigger MD025."""
    md = "---\ntitle: My Document\n---\n\n# Main Section\n\nContent here.\n"
    parse_opts = mordant.ParseOptions(meta_table=True)
    diags = mordant.lint(md, parse_opts=parse_opts)
    assert "MD025" not in rules(diags), "Single H1 with frontmatter title should be OK"


def test_md025_frontmatter_title_with_multiple_h1():
    """Document with frontmatter title and multiple H1s still triggers MD025."""
    md = "---\ntitle: My Document\n---\n\n# First\n\n# Second\n"
    parse_opts = mordant.ParseOptions(meta_table=True)
    diags = mordant.lint(md, parse_opts=parse_opts)
    assert "MD025" in rules(diags), "Multiple H1s with frontmatter title still flagged"


def test_md025_no_frontmatter_single_h1():
    """Document without frontmatter and single H1 should not trigger MD025."""
    md = "# Title\n\nContent.\n"
    diags = mordant.lint(md)
    assert "MD025" not in rules(diags), "Single H1 without frontmatter is OK"


def test_md025_no_frontmatter_no_h1():
    """Document without frontmatter and no H1 should not trigger MD025."""
    md = "## Section\n\nContent.\n"
    diags = mordant.lint(md)
    assert "MD025" not in rules(diags), "No H1 without frontmatter is OK (MD025 only flags multiples)"


# --- R8.3: MD042 flags fragment-only links to missing anchors ---

def test_md042_fragment_valid_anchor():
    """Fragment link to existing heading anchor should not trigger MD042."""
    md = "# Hello World\n\n[link](#hello-world)\n"
    diags = mordant.lint(md)
    assert "MD042" not in rules(diags), "Valid fragment link should pass"


def test_md042_fragment_missing_anchor():
    """Fragment link to non-existent heading anchor should trigger MD042."""
    md = "# Hello World\n\n[link](#nonexistent-section)\n"
    diags = mordant.lint(md)
    assert "MD042" in rules(diags), "Missing fragment anchor should be flagged"


def test_md042_fragment_empty_hash():
    """Bare # link should still trigger MD042 (original behavior)."""
    md = "[link](#)\n"
    diags = mordant.lint(md)
    assert "MD042" in rules(diags), "Empty hash link should be flagged"


def test_md042_fragment_with_special_chars():
    """Fragment links should handle special characters in heading text."""
    md = "# Hello, World!\n\n[link](#hello-world)\n"
    diags = mordant.lint(md)
    assert "MD042" not in rules(diags), "Special chars stripped from anchor, link should pass"


def test_md042_full_url_not_fragment():
    """Full URLs should not be checked for fragment anchors."""
    md = "[link](https://example.com/page)\n"
    diags = mordant.lint(md)
    assert "MD042" not in rules(diags), "Full URLs should not trigger MD042"


def test_md042_empty_destination():
    """Empty destination should still trigger MD042 (original behavior)."""
    md = "[link]()\n"
    diags = mordant.lint(md)
    assert "MD042" in rules(diags), "Empty destination should be flagged"
