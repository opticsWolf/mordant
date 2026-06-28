"""Benchmark suite for mordant vs python-markdown, mistune, and markdown-it-py.

Usage:
    python benchmarks/benchmarks.py              # Run all benchmarks
    python benchmarks/benchmarks.py --fixture small  # Run specific fixture
    python benchmarks/benchmarks.py --fixture medium --repeat 10
    python benchmarks/benchmarks.py --html --output results.json

Results are printed to stdout and optionally saved as JSON.
"""

import argparse
import json
import os
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# Ensure mordant is importable from the parent directory
# ---------------------------------------------------------------------------
ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import mordant

# ---------------------------------------------------------------------------
# Competitor imports (all optional)
# ---------------------------------------------------------------------------
try:
    import markdown as md_markdown
    HAS_MARKDOWN = True
except ImportError:
    HAS_MARKDOWN = False

try:
    import mistune
    HAS_MISTUNE = True
except ImportError:
    HAS_MISTUNE = False

try:
    import markdown_it
    HAS_MARKDOWN_IT = True
except ImportError:
    HAS_MARKDOWN_IT = False

# ---------------------------------------------------------------------------
# Benchmark data classes
# ---------------------------------------------------------------------------

@dataclass
class BenchmarkResult:
    """Single benchmark result."""
    name: str
    library: str
    fixture: str
    iterations: int
    total_seconds: float
    per_second: float
    avg_ms: float
    min_ms: float
    max_ms: float


@dataclass
class BenchmarkSuite:
    """Collection of benchmark results."""
    fixture: str
    repeat: int
    results: list = field(default_factory=list)

    def add(self, r: BenchmarkResult):
        self.results.append(r)

    def to_dict(self) -> dict:
        return {
            "fixture": self.fixture,
            "repeat": self.repeat,
            "results": [r.__dict__ for r in self.results],
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), indent=2)


# ---------------------------------------------------------------------------
# Benchmark runners
# ---------------------------------------------------------------------------

def run_benchmark(
    name: str,
    fixture_content: str,
    repeat: int = 50,
) -> BenchmarkSuite:
    """Run all available markdown libraries against a fixture."""
    suite = BenchmarkSuite(fixture=fixture_content[:50], repeat=repeat)

    # --- mordant (parse + render) ---
    suite.add(_bench_mordant_parse(name, fixture_content, repeat))
    suite.add(_bench_mordant_render(name, fixture_content, repeat))
    suite.add(_bench_mordant_total(name, fixture_content, repeat))

    # --- python-markdown ---
    if HAS_MARKDOWN:
        suite.add(_bench_python_markdown(name, fixture_content, repeat))

    # --- mistune ---
    if HAS_MISTUNE:
        suite.add(_bench_mistune(name, fixture_content, repeat))

    # --- markdown-it-py ---
    if HAS_MARKDOWN_IT:
        suite.add(_bench_markdown_it(name, fixture_content, repeat))

    return suite


def _timeit(fn, repeat: int = 50) -> tuple[float, list[float]]:
    """Run fn `repeat` times, return (total_seconds, individual_times_ms)."""
    times = []
    for _ in range(repeat):
        start = time.perf_counter()
        fn()
        elapsed = time.perf_counter() - start
        times.append(elapsed * 1000)  # ms
    total_s = sum(times) / 1000
    return total_s, times


def _make_result(name: str, library: str, fixture: str,
                 total_s: float, times_ms: list[float]) -> BenchmarkResult:
    return BenchmarkResult(
        name=name,
        library=library,
        fixture=fixture,
        iterations=len(times_ms),
        total_seconds=round(total_s, 4),
        per_second=round(1.0 / (sum(times_ms) / len(times_ms) / 1000), 2),
        avg_ms=round(sum(times_ms) / len(times_ms), 3),
        min_ms=round(min(times_ms), 3),
        max_ms=round(max(times_ms), 3),
    )


# --- mordant benchmarks ---

def _bench_mordant_parse(name: str, content: str, repeat: int) -> BenchmarkResult:
    """Benchmark just parsing (AST construction)."""
    def fn():
        mordant.parse(content)
    total_s, times = _timeit(fn, repeat)
    return _make_result(f"{name}/parse", "mordant", "parse", total_s, times)


def _bench_mordant_render(name: str, content: str, repeat: int) -> BenchmarkResult:
    """Benchmark just rendering (AST -> HTML)."""
    doc = mordant.parse(content)  # pre-parse
    def fn():
        mordant.markdown_to_html(doc.source)  # re-parse+render for fair comparison
    total_s, times = _timeit(fn, repeat)
    return _make_result(f"{name}/render", "mordant", "render", total_s, times)


def _bench_mordant_total(name: str, content: str, repeat: int) -> BenchmarkResult:
    """Benchmark full parse + render."""
    def fn():
        mordant.markdown_to_html(content)
    total_s, times = _timeit(fn, repeat)
    return _make_result(f"{name}/total", "mordant", "total", total_s, times)


# --- python-markdown ---

def _bench_python_markdown(name: str, content: str, repeat: int) -> BenchmarkResult:
    def fn():
        md_markdown.markdown(content, extensions=["tables", "fenced_code", "codehilite"])
    total_s, times = _timeit(fn, repeat)
    return _make_result(f"{name}/total", "python-markdown", "total", total_s, times)


# --- mistune ---

def _bench_mistune(name: str, content: str, repeat: int) -> BenchmarkResult:
    
    def fn():
        mistune.markdown(content)
    total_s, times = _timeit(fn, repeat)
    return _make_result(f"{name}/total", "mistune", "total", total_s, times)


# --- markdown-it-py ---

def _bench_markdown_it(name: str, content: str, repeat: int) -> BenchmarkResult:
    md = markdown_it.MarkdownIt("default")
    def fn():
        md.render(content)
    total_s, times = _timeit(fn, repeat)
    return _make_result(f"{name}/total", "markdown-it-py", "total", total_s, times)


# ---------------------------------------------------------------------------
# Fixture loading
# ---------------------------------------------------------------------------

FIXTURES_DIR = Path(__file__).resolve().parent / "fixtures"

FIXTURE_FILES = {
    "small": FIXTURES_DIR / "small.md",
    "medium": FIXTURES_DIR / "medium.md",
    "large": FIXTURES_DIR / "large.md",
    "data": FIXTURES_DIR / "data.md",  # from rushdown repo
}


def load_fixture(name: str) -> Optional[str]:
    path = FIXTURE_FILES.get(name)
    if path is None:
        print(f"Unknown fixture: {name}. Available: {list(FIXTURE_FILES.keys())}")
        return None
    if not path.exists():
        print(f"Fixture not found: {path}")
        return None
    return path.read_text(encoding='utf-8')


# ---------------------------------------------------------------------------
# Output formatting
# ---------------------------------------------------------------------------

def print_results(suite: BenchmarkSuite):
    """Print benchmark results in a formatted table."""
    print(f"\n{'='*70}")
    print(f"  Benchmark: {suite.fixture} ({suite.repeat} iterations)")
    print(f"{'='*70}")

    # Group by library
    libs = {}
    for r in suite.results:
        libs.setdefault(r.library, []).append(r)

    for lib, results in sorted(libs.items()):
        print(f"\n  {lib}:")
        print(f"  {'-'*50}")
        print(f"  {'Test':<25} {'Avg (ms)':>10} {'Min (ms)':>10} {'Max (ms)':>10} {'Total (s)':>10}")
        for r in results:
            print(f"  {r.name:<25} {r.avg_ms:>10.3f} {r.min_ms:>10.3f} {r.max_ms:>10.3f} {r.total_seconds:>10.4f}")


def print_summary(suite: BenchmarkSuite):
    """Print a speed comparison summary."""
    print(f"\n{'='*70}")
    print(f"  Speed Comparison (higher = faster)")
    print(f"{'='*70}")

    # Get total times for each library
    totals = {}
    for r in suite.results:
        if r.name.endswith("/total"):
            totals[r.library] = r.avg_ms

    if not totals:
        return

    # Find fastest
    fastest_lib = min(totals, key=totals.get)
    fastest_ms = totals[fastest_lib]

    print(f"\n  Fastest: {fastest_lib} ({fastest_ms:.3f} ms/iter)")
    print(f"  {'Library':<25} {'Avg (ms)':>10} {'vs Fastest':>12}")
    print(f"  {'-'*47}")
    for lib, avg_ms in sorted(totals.items()):
        ratio = avg_ms / fastest_ms if fastest_ms > 0 else 0
        print(f"  {lib:<25} {avg_ms:>10.3f} {ratio:>11.2f}x")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Mordant benchmark suite")
    parser.add_argument(
        "--fixture", "-f",
        choices=list(FIXTURE_FILES.keys()),
        default=None,
        help="Fixture to benchmark (default: all)",
    )
    parser.add_argument(
        "--repeat", "-n",
        type=int,
        default=50,
        help="Number of iterations per benchmark (default: 50)",
    )
    parser.add_argument(
        "--html",
        action="store_true",
        help="Include HTML output comparison",
    )
    parser.add_argument(
        "--output", "-o",
        type=str,
        default=None,
        help="Save results to JSON file",
    )
    args = parser.parse_args()

    fixtures_to_run = [args.fixture] if args.fixture else list(FIXTURE_FILES.keys())

    all_suites = []
    for fixture_name in fixtures_to_run:
        content = load_fixture(fixture_name)
        if content is None:
            continue

        print(f"\n{'#'*70}")
        print(f"# Fixture: {fixture_name} ({len(content)} chars, {len(content.splitlines())} lines)")
        print(f"{'#'*70}")

        suite = run_benchmark(fixture_name, content, repeat=args.repeat)
        all_suites.append(suite)
        print_results(suite)
        print_summary(suite)

    # Save results
    if args.output:
        output_data = {"repeat": args.repeat, "suites": [s.to_dict() for s in all_suites]}
        with open(args.output, "w") as f:
            json.dump(output_data, f, indent=2)
        print(f"\nResults saved to {args.output}")


if __name__ == "__main__":
    main()
