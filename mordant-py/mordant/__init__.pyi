"""Type stubs for mordant — Mordant Markdown parser for Python."""

from typing import Literal, Sequence

KATEX_CSS: str
KATEX_VERSION: str

# ── Options ────────────────────────────────────────────────────────

class LintOptions:
    def __init__(self, **kwargs) -> None: ...

class Engines:
    meta: bool
    emoji: bool
    footnotes: bool
    linter: bool
    diagram: bool
    chunker: bool
    math: bool
    highlighter: bool
    definition_list: bool
    fenced_div: bool
    link_attributes: bool

    def __init__(
        self,
        meta: bool = ...,
        emoji: bool = ...,
        footnotes: bool = ...,
        linter: bool = ...,
        diagram: bool = ...,
        chunker: bool = ...,
        math: bool = ...,
        highlighter: bool = ...,
        definition_list: bool = ...,
        fenced_div: bool = ...,
        link_attributes: bool = ...,
    ) -> None: ...

    @staticmethod
    def all() -> "Engines": ...
    @staticmethod
    def none() -> "Engines": ...
    @staticmethod
    def only(names: Sequence[str]) -> "Engines": ...
    @staticmethod
    def without(names: Sequence[str]) -> "Engines": ...
    @staticmethod
    def all_without(names: Sequence[str]) -> "Engines": ...

    def has(self, name: str) -> bool: ...

class ParseOptions:
    engines: Engines
    def __init__(
        self,
        smart: bool = ...,
        attributes: bool = ...,
        auto_heading_ids: bool = ...,
        escaped_space: bool = ...,
        meta_table: bool = ...,
        engines: Engines | None = ...,
    ) -> None: ...

class HighlighterOptions:
    theme: str
    mode: str
    def __init__(self, theme: str = ..., mode: str = ...) -> None: ...

class RenderOptions:
    engines: Engines
    highlighter: HighlighterOptions | None
    def __init__(
        self,
        hard_wraps: bool = ...,
        xhtml: bool = ...,
        allows_unsafe: bool = ...,
        escaped_space: bool = ...,
        engines: Engines | None = ...,
        emoji: EmojiHtmlRendererOptions | None = ...,
        diagram: DiagramHtmlRendererOptions | None = ...,
        footnote: FootnoteHtmlRendererOptions | None = ...,
        math: MathRendererOptions | None = ...,
        highlighter: HighlighterOptions | None = ...,
    ) -> None: ...

class GfmOptions:
    def __init__(self, **kwargs) -> None: ...

class ArenaOptions:
    def __init__(self, **kwargs) -> None: ...

class EmojiParserOptions:
    def __init__(self, **kwargs) -> None: ...

class EmojiHtmlRendererOptions:
    def __init__(self, **kwargs) -> None: ...

class DiagramParserOptions:
    def __init__(self, **kwargs) -> None: ...

class DiagramHtmlRendererOptions:
    def __init__(self, theme: str | None = ..., render_mode: str | None = ..., mermaid_url: str | None = ...) -> None: ...

class FootnoteHtmlRendererOptions:
    def __init__(self, **kwargs) -> None: ...

class MathRendererOptions:
    def __init__(self, output: Literal["both", "html", "mathml"] = ...) -> None: ...

class LintConfig:
    def __init__(self, **kwargs) -> None: ...
    @staticmethod
    def from_dict(d: dict) -> "LintConfig": ...

class RuleParams:
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
    emoji_parse_opts: EmojiParserOptions | None = ...,
    emoji_render_opts: EmojiHtmlRendererOptions | None = ...,
    diagram_parse_opts: DiagramParserOptions | None = ...,
    diagram_render_opts: DiagramHtmlRendererOptions | None = ...,
    footnote_render_opts: FootnoteHtmlRendererOptions | None = ...,
    highlighting_theme: str | None = ...,
    highlighting_mode: str | None = ...,
    theme: str | None = ...,
    math_renderer_opts: MathRendererOptions | None = ...,
    engines: Engines | None = ...,
) -> str: ...

def parse(source: str, gfm_opts: GfmOptions | None = ..., parse_opts: ParseOptions | None = ..., emoji_opts: EmojiParserOptions | None = ..., diagram_opts: DiagramParserOptions | None = ..., engines: Engines | None = ...) -> Document: ...

def render_math(latex: str, display: bool = False, output: str = "both") -> str: ...

def lint(source: str, gfm_opts: GfmOptions | None = ..., parse_opts: ParseOptions | None = ..., emoji_opts: EmojiParserOptions | None = ..., diagram_opts: DiagramParserOptions | None = ..., lint_opts: LintOptions | None = ..., lint_config: LintConfig | None = ..., engines: Engines | None = ...) -> list[Diagnostic]: ...

def fix(source: str, gfm_opts: GfmOptions | None = ..., parse_opts: ParseOptions | None = ..., emoji_opts: EmojiParserOptions | None = ..., diagram_opts: DiagramParserOptions | None = ..., lint_opts: LintOptions | None = ..., default_language: str | None = ..., lint_config: LintConfig | None = ..., engines: Engines | None = ...) -> FixResult: ...

def lint_rules() -> list["RuleMetadata"]: ...

def lint_many(files: Sequence[tuple[str, str]], lint_config: LintConfig | None = ..., engines: Engines | None = ...) -> list[tuple[str, list[Diagnostic]]]: ...

def fix_many(files: Sequence[tuple[str, str]], lint_config: LintConfig | None = ..., default_language: str | None = ..., engines: Engines | None = ...) -> list[tuple[str, FixResult]]: ...

# ── Theme / syntax helpers ────────────────────────────────────────

def add_custom_theme(name: str, content: str) -> None: ...

def add_custom_syntax(content: str, name: str | None = ...) -> str: ...

def list_themes() -> list[str]: ...

def list_syntaxes() -> list[str]: ...

def detect_language(code: str) -> str: ...

def theme_background(name: str) -> str | None: ...

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
    def __init__(self, id: str, name: str, description: str, fixable: bool, default_params: str) -> None: ...

# ── Chunker ───────────────────────────────────────────────────────

class ExtractedChunk:
    def __init__(self, **kwargs) -> None: ...

class MarkdownChunker:
    def __init__(self, text: str, engines: Engines | None = ...) -> None: ...
    @staticmethod
    def from_file(path: str, engines: Engines | None = ...) -> "MarkdownChunker": ...
    @staticmethod
    def from_file_mmap(path: str, engines: Engines | None = ...) -> "MarkdownChunker": ...
    def __iter__(self) -> "MarkdownChunker": ...
    def __next__(self) -> str | None: ...
    @property
    def current_header(self) -> str | None: ...
    @property
    def node_count(self) -> int: ...
    def get_chunks(self) -> list[ExtractedChunk]: ...
    def get_all_chunks(self) -> list[ExtractedChunk]: ...
    def get_chunks_with_context(self) -> list[ExtractedChunk]: ...
    def get_bare_chunks(self) -> list[str]: ...
    @staticmethod
    def get_delimiter(prev: str, curr: str) -> str: ...
    def compute_overlap_payloads(self, overlap_words: int) -> list[dict]: ...

# ── Highlighter ───────────────────────────────────────────────────

class Highlighter:
    def __init__(self, theme: str | None = ..., mode: str | None = ...) -> None: ...

    def highlight(self, language: str, source: str, *, bare: bool = ...) -> str: ...
