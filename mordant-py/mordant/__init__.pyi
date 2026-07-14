"""Type stubs for mordant — Mordant Markdown parser for Python."""

from typing import Literal, Sequence

KATEX_CSS: str

# ── Options ────────────────────────────────────────────────────────

class LintOptions:
    def __init__(self, **kwargs) -> None: ...

class ParseOptions:
    def __init__(self, **kwargs) -> None: ...

class RenderOptions:
    def __init__(self, **kwargs) -> None: ...

class GfmOptions:
    def __init__(self, **kwargs) -> None: ...

class ArenaOptions:
    def __init__(self, **kwargs) -> None: ...

class PyEmojiParserOptions:
    def __init__(self, **kwargs) -> None: ...

class PyEmojiHtmlRendererOptions:
    def __init__(self, **kwargs) -> None: ...

class PyDiagramParserOptions:
    def __init__(self, **kwargs) -> None: ...

class PyDiagramHtmlRendererOptions:
    def __init__(self, theme: str | None = ..., render_mode: str | None = ..., mermaid_url: str | None = ...) -> None: ...

class PyFootnoteHtmlRendererOptions:
    def __init__(self, **kwargs) -> None: ...

class PyMathRendererOptions:
    def __init__(self, output: Literal["both", "html", "mathml"] = ...) -> None: ...

class LintConfig:
    def __init__(self, **kwargs) -> None: ...

# ── Enums / Literals ──────────────────────────────────────────────

class GfmFeature:
    ...

class HighlightingMode:
    ...

# ── Core functions ────────────────────────────────────────────────

def markdown_to_html(
    source: str,
    gfm_opts: GfmOptions | None = ...,
    parse_opts: ParseOptions | None = ...,
    render_opts: RenderOptions | None = ...,
    emoji_parse_opts: PyEmojiParserOptions | None = ...,
    emoji_render_opts: PyEmojiHtmlRendererOptions | None = ...,
    diagram_parse_opts: PyDiagramParserOptions | None = ...,
    diagram_render_opts: PyDiagramHtmlRendererOptions | None = ...,
    footnote_render_opts: PyFootnoteHtmlRendererOptions | None = ...,
    highlighting_theme: str | None = ...,
    highlighting_mode: str | None = ...,
    theme: str | None = ...,
    math_renderer_opts: PyMathRendererOptions | None = ...,
) -> str: ...

def parse(source: str, opts: ParseOptions | None = ...) -> Document: ...

def render_math(source: str, opts: PyMathRendererOptions | None = ...) -> str: ...

def lint(source: str, opts: LintOptions | None = ..., config: LintConfig | None = ...) -> list[Diagnostic]: ...

def fix(source: str, opts: LintOptions | None = ..., config: LintConfig | None = ...) -> FixResult: ...

def lint_rules() -> list[RuleMetadata]: ...

def lint_many(sources: Sequence[str], opts: LintOptions | None = ..., config: LintConfig | None = ...) -> list[list[Diagnostic]]: ...

def fix_many(sources: Sequence[str], opts: LintOptions | None = ..., config: LintConfig | None = ...) -> list[FixResult]: ...

# ── Theme / syntax helpers ────────────────────────────────────────

def add_custom_theme(name: str, content: str) -> None: ...

def list_themes() -> list[str]: ...

def list_syntaxes() -> list[str]: ...

# ── AST ───────────────────────────────────────────────────────────

class Document:
    def __init__(self, **kwargs) -> None: ...

class Node:
    def __init__(self, **kwargs) -> None: ...

class Walker:
    def __init__(self, **kwargs) -> None: ...

class Diagnostic:
    def __init__(self, **kwargs) -> None: ...

class FixResult:
    def __init__(self, **kwargs) -> None: ...

class RuleMetadata:
    def __init__(self, **kwargs) -> None: ...

class ExtractedChunk:
    def __init__(self, **kwargs) -> None: ...

# ── Chunker ───────────────────────────────────────────────────────

class MarkdownChunker:
    def __init__(self, **kwargs) -> None: ...

    def chunk(self, source: str, **kwargs) -> list[ExtractedChunk]: ...

# ── Highlighter ───────────────────────────────────────────────────

class Highlighter:
    def __init__(self, theme: str | None = ..., mode: str | None = ...) -> None: ...

    def highlight(self, language: str, source: str) -> str: ...
