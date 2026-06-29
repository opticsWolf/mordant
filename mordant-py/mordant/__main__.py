"""Mordant CLI — lint and fix Markdown files.

Usage:
    python -m mordant [options] <path> [path ...]

Examples:
    python -m mordant README.md
    python -m mordant --fix docs/**/*.md
    python -m mordant --format json --config .markdownlint.json src/**/*.md
    python -m mordant --enable MD001,MD025 --disable MD013 docs/
"""

import argparse
import glob
import json
import os
import sys
from pathlib import Path

# Import the mordant module (compiled Rust extension)
import mordant


# ===========================================================================
# Output formatters
# ===========================================================================

def format_human(diagnostic, filename):
    """Format a single diagnostic for human-readable output."""
    line = diagnostic.line or "?"
    rule = diagnostic.rule
    name = diagnostic.name
    message = diagnostic.message
    fixable = " [fixable]" if diagnostic.fixable else ""
    return f"{filename}:{line}: {rule} ({name}): {message}{fixable}"


def format_json_result(filename, diagnostics):
    """Format diagnostics as a JSON object for a single file."""
    return {
        "file": filename,
        "diagnostics": [
            {
                "rule": d.rule,
                "name": d.name,
                "message": d.message,
                "line": d.line,
                "column": d.column,
                "span": list(d.span) if d.span else None,
                "severity": d.severity,
                "fixable": d.fixable,
            }
            for d in diagnostics
        ],
    }


def format_github(diagnostic, filename):
    """Format a diagnostic as a GitHub Actions annotation."""
    line = diagnostic.line or 1
    severity = "warning"  # All current rules are warnings
    rule = diagnostic.rule
    message = diagnostic.message
    # GitHub Actions annotation format
    return f"::{severity} file={filename},line={line},title={rule}" + "} " + message


# ===========================================================================
# Config loading
# ===========================================================================

def load_config(config_path):
    """Load a .markdownlint.json config file and return a LintConfig."""
    if config_path is None:
        # Auto-detect: look for .markdownlint.json in CWD
        candidate = Path.cwd() / ".markdownlint.json"
        if candidate.exists():
            config_path = str(candidate)
        else:
            return None

    config_path = Path(config_path)
    if not config_path.exists():
        print(f"Error: config file not found: {config_path}", file=sys.stderr)
        sys.exit(1)

    with open(config_path, "r", encoding="utf-8") as f:
        data = json.load(f)

    return mordant.LintConfig.from_dict(data)


# ===========================================================================
# File collection
# ===========================================================================

def collect_files(paths):
    """Expand path patterns and return list of (filename, source) tuples."""
    files = []
    for pattern in paths:
        expanded = glob.glob(pattern, recursive=True)
        if not expanded:
            # Try as a literal file path
            if os.path.exists(pattern):
                expanded = [pattern]
            else:
                print(f"Warning: no files matching: {pattern}", file=sys.stderr)
                continue

        for filepath in sorted(expanded):
            filepath = os.path.normpath(filepath)
            if os.path.isdir(filepath):
                # Recurse into directories
                for root, _dirs, filenames in os.walk(filepath):
                    for fname in sorted(filenames):
                        full = os.path.join(root, fname)
                        if fname.endswith((".md", ".markdown", ".mdown", ".mkd", ".mkdn", ".mdwn", ".mdtxt", ".mdtext", ".text", ".lit", ".pandoc", ".ron", ".scd", ".work")):
                            files.append(full)
            elif filepath.endswith((".md", ".markdown", ".mdown", ".mkd", ".mkdn", ".mdwn", ".mdtxt", ".mdtext", ".text", ".lit", ".pandoc", ".ron", ".scd", ".work")):
                files.append(filepath)

    # Deduplicate while preserving order
    seen = set()
    unique = []
    for f in files:
        norm = os.path.normpath(f)
        if norm not in seen:
            seen.add(norm)
            unique.append(norm)
    return unique


def read_files(filepaths):
    """Read file contents and return list of (filename, source) tuples."""
    files = []
    for filepath in filepaths:
        try:
            with open(filepath, "r", encoding="utf-8") as f:
                source = f.read()
            files.append((filepath, source))
        except (OSError, UnicodeDecodeError) as e:
            print(f"Warning: could not read {filepath}: {e}", file=sys.stderr)
    return files


# ===========================================================================
# Main CLI
# ===========================================================================

def main():
    ap = argparse.ArgumentParser(
        prog="mordant",
        description="Fast Markdown linter powered by Rust.",
    )
    ap.add_argument(
        "paths",
        nargs="+",
        help="Files or glob patterns to lint (e.g. '*.md' 'docs/**')",
    )
    ap.add_argument(
        "--fix",
        action="store_true",
        help="Auto-fix fixable issues and write corrected files in-place",
    )
    ap.add_argument(
        "--config",
        default=None,
        help="Path to .markdownlint.json config file (auto-detected if not given)",
    )
    ap.add_argument(
        "--format",
        choices=["human", "json", "github"],
        default="human",
        help="Output format (default: human)",
    )
    ap.add_argument(
        "--enable",
        default=None,
        help="Comma-separated list of rules to enable (e.g. 'MD001,MD025')",
    )
    ap.add_argument(
        "--disable",
        default=None,
        help="Comma-separated list of rules to disable (e.g. 'MD013,MD025')",
    )
    ap.add_argument(
        "--default-language",
        default=None,
        help="Default language to insert for MD040 fixes (e.g. 'python')",
    )
    ap.add_argument(
        "--dry-run",
        action="store_true",
        help="With --fix: show what would be fixed without writing files",
    )

    args = ap.parse_args()

    # --- Build config ---
    lint_config = load_config(args.config)

    # Apply --enable / --disable overrides
    if args.enable or args.disable:
        if lint_config is None:
            lint_config = mordant.LintConfig.from_dict({})

        if args.enable:
            rules = [r.strip() for r in args.enable.split(",")]
            lint_config = mordant.LintConfig.from_dict({"enable": rules})
            # If we also have --disable, merge it
            if args.disable:
                disabled = [r.strip() for r in args.disable.split(",")]
                # Remove disabled rules from enable list
                lint_config = mordant.LintConfig.from_dict({
                    "enable": [r for r in rules if r not in disabled]
                })
        elif args.disable:
            disabled = [r.strip() for r in args.disable.split(",")]
            lint_config = mordant.LintConfig.from_dict({"disable": disabled})

    # --- Collect files ---
    filepaths = collect_files(args.paths)
    if not filepaths:
        print("No Markdown files found.", file=sys.stderr)
        sys.exit(0)

    # --- Read file contents ---
    files = read_files(filepaths)
    if not files:
        print("No files could be read.", file=sys.stderr)
        sys.exit(1)

    # --- Run lint/fix ---
    if args.fix:
        # Batch fix
        results = mordant.fix_many(
            files,
            lint_config=lint_config,
            default_language=args.default_language,
        )

        total_fixed = 0
        total_unfixable = 0
        json_output = []

        for (filename, _source), (result_name, result) in zip(files, results):
            fixed_count = len(result.fixed)
            remaining_count = len(result.remaining)
            total_fixed += fixed_count
            total_unfixable += remaining_count

            if args.format == "json":
                json_output.append({
                    "file": filename,
                    "fixed": fixed_count,
                    "remaining": [
                        {
                            "rule": d.rule,
                            "name": d.name,
                            "message": d.message,
                            "line": d.line,
                            "column": d.column,
                            "severity": d.severity,
                            "fixable": d.fixable,
                        }
                        for d in result.remaining
                    ],
                })

            if args.format == "human":
                if fixed_count > 0:
                    print(f"Fixed {fixed_count} issue(s) in {filename}")
                if remaining_count > 0:
                    for d in result.remaining:
                        print(format_human(d, filename))

            elif args.format == "github":
                for d in result.remaining:
                    print(format_github(d, filename))

            # Write corrected file (unless dry-run)
            if not args.dry_run and result.output != _source:
                try:
                    with open(filename, "w", encoding="utf-8") as f:
                        f.write(result.output)
                except OSError as e:
                    print(f"Error writing {filename}: {e}", file=sys.stderr)

        if args.format == "json":
            print(json.dumps(json_output, indent=2))

        # Exit code: non-zero if unfixable issues remain
        sys.exit(1 if total_unfixable > 0 else 0)

    else:
        # Batch lint
        results = mordant.lint_many(files, lint_config=lint_config)

        total_issues = 0
        json_output = []

        for (filename, _source), (result_name, diagnostics) in zip(files, results):
            if not diagnostics:
                continue

            total_issues += len(diagnostics)

            if args.format == "human":
                for d in diagnostics:
                    print(format_human(d, filename))

            elif args.format == "json":
                json_output.append(format_json_result(filename, diagnostics))

            elif args.format == "github":
                for d in diagnostics:
                    print(format_github(d, filename))

        if args.format == "json":
            print(json.dumps(json_output, indent=2))

        # Exit code: non-zero if issues found
        sys.exit(1 if total_issues > 0 else 0)


if __name__ == "__main__":
    main()
