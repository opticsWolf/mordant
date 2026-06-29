"""Mordant -- A fast CommonMark + GFM Markdown parser for Python.

Powered by the rushdown Rust library. This package re-exports the public API
of the compiled extension module so that everything is available directly under
the ``mordant`` namespace, e.g.::

    import mordant

    html = mordant.markdown_to_html("# Hello\\n\\n**World**")
    doc = mordant.parse("# Hello")
    diagnostics = mordant.lint("# A\\n\\n### C")
"""

# The compiled extension is built by maturin and placed inside this package as
# the ``mordant.mordant`` submodule (its name comes from `#[pymodule] fn mordant`
# / `[lib] name = "mordant"`). Import its public API via a relative import.
from .mordant import (
    # Functions
    markdown_to_html,
    parse,
    lint,
    # Core AST types
    Document,
    Node,
    Walker,
    # Parse / render options
    ParseOptions,
    RenderOptions,
    GfmOptions,
    ArenaOptions,
    # Emoji extension options
    PyEmojiParserOptions,
    PyEmojiHtmlRendererOptions,
    # Diagram (mermaid) extension options
    PyDiagramParserOptions,
    PyDiagramHtmlRendererOptions,
    # Linter
    LintOptions,
    Diagnostic,
)

__all__ = [
    # Functions
    "markdown_to_html",
    "parse",
    "lint",
    # Core AST types
    "Document",
    "Node",
    "Walker",
    # Parse / render options
    "ParseOptions",
    "RenderOptions",
    "GfmOptions",
    "ArenaOptions",
    # Emoji extension options
    "PyEmojiParserOptions",
    "PyEmojiHtmlRendererOptions",
    # Diagram (mermaid) extension options
    "PyDiagramParserOptions",
    "PyDiagramHtmlRendererOptions",
    # Linter
    "LintOptions",
    "Diagnostic",
]

# Keep the version in lockstep with the installed wheel (built from the
# `version` field in pyproject.toml / Cargo.toml) instead of hard-coding it,
# which is what caused the previous 0.1.0-vs-0.5.0 drift.
try:
    from importlib.metadata import version as _version, PackageNotFoundError

    try:
        __version__ = _version("mordant")
    except PackageNotFoundError:
        # Running from a source checkout that hasn't been installed.
        __version__ = "0.5.0"
finally:
    # Don't leak the import helpers into the public namespace.
    try:
        del _version, PackageNotFoundError
    except NameError:
        pass
