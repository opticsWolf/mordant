"""CommonMark 0.31.2 spec test suite.

Runs all 652 test cases from the official CommonMark spec against mordant.
The spec JSON is sourced from rushdown's test fixtures.

Usage:
    python -m pytest tests/test_commonmark_spec.py -v
    python -m pytest tests/test_commonmark_spec.py -v --tb=short
"""

import json
import re
import sys
from pathlib import Path

import pytest

import mordant

# ---------------------------------------------------------------------------
# Load spec fixtures
# ---------------------------------------------------------------------------

RUSHDOWN_ROOT = Path(__file__).resolve().parent.parent.parent
SPEC_PATH = RUSHDOWN_ROOT / "tests" / "fixtures" / "spec.json"


def load_spec_cases():
    """Load all CommonMark spec test cases from spec.json."""
    with open(SPEC_PATH, encoding="utf-8") as f:
        cases = json.load(f)
    return cases


_SPEC_CASES = load_spec_cases()


# ---------------------------------------------------------------------------
# HTML normalization for spec comparison
# ---------------------------------------------------------------------------

def _normalize_html(html: str) -> str:
    """Normalize HTML for spec comparison.

    The CommonMark spec uses XHTML-style self-closing tags (e.g. <hr />),
    while mordant may output HTML-style (<hr>). We normalize for comparison.
    """
    def _fix_void_tag(m):
        tag = m.group(1)
        attrs = m.group(2)
        # Already self-closing, don't double the slash
        if attrs.endswith('/'):
            return m.group(0)
        return f'<{tag}{attrs} />'

    html = re.sub(r'<(hr|br|img|input)([^>]*)>', _fix_void_tag, html)
    return html


# ---------------------------------------------------------------------------
# Test cases
# ---------------------------------------------------------------------------

class TestCommonMarkSpec:
    """Run all 652 CommonMark spec test cases."""

    @pytest.mark.parametrize(
        "case",
        _SPEC_CASES,
        ids=lambda c: f"Example {c['example']} ({c['section']})",
    )
    def test_spec(self, case: dict):
        """Verify mordant's output matches the CommonMark spec."""
        markdown = case["markdown"]
        expected = case["html"]

        # The spec expects raw HTML to pass through in certain contexts.
        # mordant blocks raw HTML by default (allows_unsafe=false).
        # We compare with allows_unsafe=true to match the Rust test baseline.
        html = mordant.markdown_to_html(
            markdown,
            render_opts=mordant.RenderOptions(allows_unsafe=True),
        )

        # Normalize for comparison
        html_norm = _normalize_html(html)
        exp_norm = _normalize_html(expected)

        assert html_norm == exp_norm, (
            f"Example {case['example']} ({case['section']}):\n"
            f"  Input:    {repr(markdown[:80])}\n"
            f"  Expected: {repr(exp_norm[:120])}\n"
            f"  Got:      {repr(html_norm[:120])}"
        )


# ---------------------------------------------------------------------------
# Summary reporter
# ---------------------------------------------------------------------------

def pytest_sessionfinish(session, exitstatus):
    """Print a summary of spec test results."""
    total = len(_SPEC_CASES)
    passed = session.tests_passed if hasattr(session, "tests_passed") else 0
    failed = total - passed

    print()
    print("=" * 70)
    print(f"  CommonMark Spec Test Summary")
    print(f"  Total cases: {total}")
    print(f"  Passed:      {passed}")
    print(f"  Failed:      {failed}")
    print(f"  Status:      {'All pass' if failed == 0 else f'{failed} failures'}")
    print("=" * 70)

    # Per-section breakdown
    section_results = {}
    for item in session.items:
        test_id = item.nodeid
        if "Example" in test_id:
            try:
                ex = int(test_id.split("Example ")[1].split(" ")[0])
            except (ValueError, IndexError):
                continue
            for c in _SPEC_CASES:
                if c["example"] == ex:
                    section_results.setdefault(c["section"], {"pass": 0, "fail": 0})
                    if item.repcall and item.repcall.failed:
                        section_results[c["section"]]["fail"] += 1
                    else:
                        section_results[c["section"]]["pass"] += 1
                    break

    if section_results:
        print()
        print(f"  {'Section':<35} {'Pass':>6} {'Fail':>6} {'Total':>7}")
        print(f"  {'-'*35} {'-'*6} {'-'*6} {'-'*7}")
        for section in sorted(section_results.keys()):
            r = section_results[section]
            total_s = r["pass"] + r["fail"]
            marker = " *" if r["fail"] > 0 else ""
            print(f"  {section:<35} {r['pass']:>6} {r['fail']:>6} {total_s:>7}{marker}")
        print()


# ---------------------------------------------------------------------------
# Manual run
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    filter_section = None
    if len(sys.argv) > 1:
        filter_section = sys.argv[1]

    failed = 0
    passed = 0

    for case in _SPEC_CASES:
        if filter_section and case["section"] != filter_section:
            continue

        markdown = case["markdown"]
        expected = case["html"]
        got = mordant.markdown_to_html(
            markdown,
            render_opts=mordant.RenderOptions(allows_unsafe=True),
        )
        got_norm = _normalize_html(got)
        exp_norm = _normalize_html(expected)

        if got_norm == exp_norm:
            passed += 1
        else:
            failed += 1
            print(f"FAIL Example {case['example']} ({case['section']}):")
            print(f"  Input:    {repr(markdown[:80])}")
            print(f"  Expected: {repr(exp_norm[:120])}")
            print(f"  Got:      {repr(got_norm[:120])}")
            print()

    print(f"\n{'='*70}")
    print(f"  Results: {passed} passed, {failed} failed (of {len(_SPEC_CASES)})")
    if filter_section:
        print(f"  Filtered by section: {filter_section}")
    print("=" * 70)

    sys.exit(1 if failed > 0 else 0)
