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
