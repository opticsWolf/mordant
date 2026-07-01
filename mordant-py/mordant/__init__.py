"""Mordant - A fast CommonMark + GFM Markdown parser for Python.

Re-exports all public symbols from the compiled Rust extension.
"""

import os
import sys

# Import everything from the compiled module
try:
    from .mordant import (
        markdown_to_html,
        parse,
        lint,
        fix,
        lint_rules,
        lint_many,
        fix_many,
        LintConfig,
        LintOptions,
        ParseOptions,
        RenderOptions,
        GfmOptions,
        ArenaOptions,
        PyEmojiParserOptions,
        PyEmojiHtmlRendererOptions,
        PyDiagramParserOptions,
        PyDiagramHtmlRendererOptions,
        PyHighlighter,
        PyHighlightingMode,
        add_custom_theme,
        list_themes,
        list_syntaxes,
        Document,
        Node,
        Walker,
        Diagnostic,
        FixResult,
        RuleMetadata,
    )
except ImportError:
    # Fallback for development builds where the module name may differ
    try:
        from .mordant import *
    except ImportError:
        import mordant as _mod
        globals().update({k: v for k, v in vars(_mod).items() if not k.startswith("_")})


def _load_embedded_themes():
    """Load embedded themes from the package's themes/ directory."""
    # Get the directory where this module is located
    package_dir = os.path.dirname(os.path.abspath(__file__))
    themes_dir = os.path.join(package_dir, "themes")
    
    if not os.path.isdir(themes_dir):
        return
    
    for f in sorted(os.listdir(themes_dir)):
        if f.endswith(".tmTheme"):
            theme_name = f.replace(".tmTheme", "")
            file_path = os.path.join(themes_dir, f)
            try:
                with open(file_path, "r") as fp:
                    content = fp.read()
                add_custom_theme(theme_name, content)
            except Exception as e:
                print(f"Warning: Could not load theme {theme_name}: {e}")


# Load embedded themes after module import
_load_embedded_themes()

__all__ = [
    "markdown_to_html",
    "parse",
    "lint",
    "fix",
    "lint_rules",
    "lint_many",
    "fix_many",
    "LintConfig",
    "LintOptions",
    "ParseOptions",
    "RenderOptions",
    "GfmOptions",
    "ArenaOptions",
    "PyEmojiParserOptions",
    "PyEmojiHtmlRendererOptions",
    "PyDiagramParserOptions",
    "PyDiagramHtmlRendererOptions",
    "PyHighlighter",
    "PyHighlightingMode",
    "add_custom_theme",
    "list_themes",
    "list_syntaxes",
    "Document",
    "Node",
    "Walker",
    "Diagnostic",
    "FixResult",
    "RuleMetadata",
]
