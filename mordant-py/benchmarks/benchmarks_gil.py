"""Multi-threaded benchmark to demonstrate GIL release benefit.

This benchmark runs multiple threads simultaneously to show how GIL release
in mordant allows true parallelism, while pure-Python parsers serialize on the GIL.

Usage:
    python benchmarks/benchmarks_gil.py [--threads N]
"""

import argparse
import json
import os
import sys
import threading
import time
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import mordant

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
# Data classes
# ---------------------------------------------------------------------------

@dataclass
class ThreadResult:
    thread_id: int
    library: str
    fixture: str
    iterations: int
    total_seconds: float
    avg_ms: float


@dataclass
class GILBenchmarkSuite:
    fixture: str
    threads: int
    iterations: int
    results: list = field(default_factory=list)

    def add(self, r: ThreadResult):
        self.results.append(r)

    def to_dict(self) -> dict:
        return {
            "fixture": self.fixture,
            "threads": self.threads,
            "iterations": self.iterations,
            "results": [r.__dict__ for r in self.results],
        }


# ---------------------------------------------------------------------------
# Benchmark runners
# ---------------------------------------------------------------------------

def bench_mordant(content: str, iterations: int) -> float:
    """Benchmark mordant parse+render."""
    start = time.perf_counter()
    for _ in range(iterations):
        mordant.markdown_to_html(content)
    return time.perf_counter() - start


def bench_python_markdown(content: str, iterations: int) -> float:
    """Benchmark python-markdown."""
    start = time.perf_counter()
    for _ in range(iterations):
        md_markdown.markdown(content, extensions=["tables", "fenced_code"])
    return time.perf_counter() - start


def bench_mistune(content: str, iterations: int) -> float:
    """Benchmark mistune."""
    start = time.perf_counter()
    for _ in range(iterations):
        mistune.markdown(content)
    return time.perf_counter() - start


def bench_markdown_it(content: str, iterations: int) -> float:
    """Benchmark markdown-it-py."""
    md = markdown_it.MarkdownIt("default")
    start = time.perf_counter()
    for _ in range(iterations):
        md.render(content)
    return time.perf_counter() - start


# ---------------------------------------------------------------------------
# Multi-threaded benchmark
# ---------------------------------------------------------------------------

def run_threaded_bench(
    fixture_content: str,
    num_threads: int,
    iterations: int,
    bench_fn,
    library_name: str,
) -> list:
    """Run a benchmark with multiple threads."""
    thread_results = []
    barrier = threading.Barrier(num_threads)

    def worker(thread_id: int):
        barrier.wait()  # Synchronize all threads to start together
        total_s = bench_fn(fixture_content, iterations)
        thread_results.append(ThreadResult(
            thread_id=thread_id,
            library=library_name,
            fixture=fixture_content[:50],
            iterations=iterations,
            total_seconds=round(total_s, 4),
            avg_ms=round(total_s / iterations * 1000, 3),
        ))

    thread_handles = [threading.Thread(target=worker, args=(i,)) for i in range(num_threads)]
    for t in thread_handles:
        t.start()
    for t in thread_handles:
        t.join()

    return thread_results


# ---------------------------------------------------------------------------
# Fixture loading
# ---------------------------------------------------------------------------

FIXTURES_DIR = Path(__file__).resolve().parent / "fixtures"

FIXTURE_FILES = {
    "small": FIXTURES_DIR / "small.md",
    "medium": FIXTURES_DIR / "medium.md",
    "large": FIXTURES_DIR / "large.md",
}


def load_fixture(name: str) -> Optional[str]:
    path = FIXTURE_FILES.get(name)
    if path is None:
        print(f"Unknown fixture: {name}. Available: {list(FIXTURE_FILES.keys())}")
        return None
    if not path.exists():
        print(f"Fixture not found: {path}")
        return None
    return path.read_text()


# ---------------------------------------------------------------------------
# Output formatting
# ---------------------------------------------------------------------------

def print_threaded_results(suite: GILBenchmarkSuite):
    """Print multi-threaded benchmark results."""
    print(f"\n{'='*70}")
    print(f"  Multi-threaded Benchmark: {suite.fixture} ({suite.threads} threads, {suite.iterations} iterations/thread)")
    print(f"{'='*70}")

    # Group by library
    libs = {}
    for r in suite.results:
        libs.setdefault(r.library, []).append(r)

    for lib, results in sorted(libs.items()):
        print(f"\n  {lib}:")
        print(f"  {'-'*60}")
        print(f"  {'Thread':<10} {'Total (s)':>12} {'Avg (ms)':>12} {'Rate (docs/s)':>15}")
        total = sum(r.total_seconds for r in results)
        for r in results:
            rate = r.iterations / r.total_seconds if r.total_seconds > 0 else 0
            print(f"  {r.thread_id:<10} {r.total_seconds:>12.4f} {r.avg_ms:>12.3f} {rate:>15.1f}")
        print(f"  {'-'*10} {'-'*12} {'-'*12} {'-'*15}")
        total_docs = sum(r.iterations for r in results)
        overall_rate = total_docs / total if total > 0 else 0
        print(f"  {'TOTAL':<10} {total:>12.4f} {'':>12} {overall_rate:>15.1f}")


def print_concurrency_analysis(suites: list):
    """Analyze concurrency scaling across libraries."""
    print(f"\n{'='*70}")
    print(f"  Concurrency Scaling Analysis")
    print(f"{'='*70}")

    # Get single-thread and multi-thread results
    single = {}
    multi = {}
    for suite in suites:
        for r in suite.results:
            if r.thread_id == 0:
                single[(r.library, suite.fixture)] = r.avg_ms
        if suite.threads > 1:
            for r in suite.results:
                multi[(r.library, suite.fixture)] = r.avg_ms

    for lib in sorted(set(k[0] for k in single.keys())):
        print(f"\n  {lib}:")
        for fixture in ["small", "medium", "large"]:
            key = (lib, fixture)
            if key in single:
                print(f"    {fixture}: {single[key]:.3f} ms/iter")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Mordant GIL release benchmark")
    parser.add_argument(
        "--fixture", "-f",
        choices=list(FIXTURE_FILES.keys()),
        default=None,
        help="Fixture to benchmark (default: all)",
    )
    parser.add_argument(
        "--threads", "-t",
        type=int,
        default=4,
        help="Number of threads (default: 4)",
    )
    parser.add_argument(
        "--iterations", "-n",
        type=int,
        default=20,
        help="Iterations per thread (default: 20)",
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
        print(f"# Threads: {args.threads}, Iterations/thread: {args.iterations}")
        print(f"{'#'*70}")

        suite = GILBenchmarkSuite(fixture=fixture_name, threads=args.threads, iterations=args.iterations)

        # mordant
        suite.results.extend(run_threaded_bench(
            content, args.threads, args.iterations,
            bench_mordant, "mordant"
        ))

        # python-markdown
        if HAS_MARKDOWN:
            suite.results.extend(run_threaded_bench(
                content, args.threads, args.iterations,
                bench_python_markdown, "python-markdown"
            ))

        # mistune
        if HAS_MISTUNE:
            suite.results.extend(run_threaded_bench(
                content, args.threads, args.iterations,
                bench_mistune, "mistune"
            ))

        # markdown-it-py
        if HAS_MARKDOWN_IT:
            suite.results.extend(run_threaded_bench(
                content, args.threads, args.iterations,
                bench_markdown_it, "markdown-it-py"
            ))

        all_suites.append(suite)
        print_threaded_results(suite)

    print_concurrency_analysis(all_suites)

    if args.output:
        output_data = {
            "threads": args.threads,
            "iterations": args.iterations,
            "suites": [s.to_dict() for s in all_suites],
        }
        with open(args.output, "w") as f:
            json.dump(output_data, f, indent=2)
        print(f"\nResults saved to {args.output}")


if __name__ == "__main__":
    main()
