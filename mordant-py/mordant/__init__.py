"""Mordant - A fast CommonMark + GFM Markdown parser for Python.

Re-exports all public symbols from the compiled Rust extension.
"""

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
    "Document",
    "Node",
    "Walker",
    "Diagnostic",
    "FixResult",
    "RuleMetadata",
]
