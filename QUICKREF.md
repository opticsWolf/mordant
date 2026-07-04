# Mordant Quick Reference

> **Version:** 0.8.0  
> **Import:** `import mordant`

---

## Install

```bash
pip install mordant
# or from source:
cd mordant-py && cargo build --release
```

---

## Core API

### `list_syntaxes() -> list[str]`

List all available syntax highlighting languages (from syntect-assets).

```python
syntaxes = mordant.list_syntaxes()
print(len(syntaxes))  # ~198 languages
assert "Python" in syntaxes
assert "Rust" in syntaxes
```

---

### `markdown_to_html(source, gfm_opts=None, parse_opts=None, render_opts=None, emoji_parse_opts=None, emoji_render_opts=None, diagram_parse_opts=None, diagram_render_opts=None, highlighting_theme=None, highlighting_mode=None) -> str`

One-call parse + render. GIL is released during the CPU-heavy parse + render phase.

| Argument | Type | Default | Description |
|----------|------|---------|-------------|
| `highlighting_theme` | `str \| None` | `"InspiredGitHub"` | Theme name for code highlighting |
| `highlighting_mode` | `str \| None` | `"Attribute"` | `"Attribute"` (inline `style`) or `"Class"` (CSS `class`) |

```python
import mordant

# Basic
html = mordant.markdown_to_html("# Hello\n\n**World**")
# '<h1>Hello</h1>\n<p><strong>World</strong></p>\n'

# GFM
html = mordant.markdown_to_html("~~strike~~")
# '<p><del>strike</del></p>\n'

# With options
html = mordant.markdown_to_html(
    "Hello\nWorld",
    render_opts=mordant.RenderOptions(hard_wraps=True),
)
# '<p>Hello<br />\nWorld</p>\n'

# With syntax highlighting
html = mordant.markdown_to_html("""```python
def hello():
    print("world")
```""", highlighting_theme="Dracula")
# Code block rendered with Dracula theme (inline style attributes)

# With Class mode
html = mordant.markdown_to_html("""```python
x = 1
```""", highlighting_theme="GitHub", highlighting_mode="Class")
# Code block rendered with CSS class attributes
```

### `parse(source, gfm_opts=None, parse_opts=None, emoji_opts=None, diagram_opts=None) -> Document`

Parse only. Returns a `Document` with full AST access. GIL is released during parsing.

```python
doc = mordant.parse("# Hello\n\n**World**")
print(doc.kind)        # "Document"
print(doc.type)        # "block"
print(doc.source)      # "# Hello\n\n**World**"
print(doc.text)        # "HelloWorld"
print(doc.children)    # [Heading, Paragraph]
print(doc.metadata)    # {}
```

---

## Lint API

### `lint(source, gfm_opts=None, parse_opts=None, emoji_opts=None, diagram_opts=None, lint_opts=None, lint_config=None) -> list[Diagnostic]`

Lint a markdown string and return diagnostics. GIL is released during linting.

```python
import mordant

# Basic lint
diagnostics = mordant.lint("# Hello\n\n### Jump\n")
# [Diagnostic(rule='MD001', name='heading-increment', ...)]

# With options
diagnostics = mordant.lint(
    "# Hello\n\n### Jump\n",
    parse_opts=mordant.ParseOptions(meta_table=True),
    lint_opts=mordant.LintOptions(disable=["MD040"]),
    lint_config=mordant.LintConfig(
        disable=["MD040"],
        params=mordant.RuleParams(heading_style="atx"),
    ),
)

for d in diagnostics:
    print(f"{d.rule}:{d.line} {d.name}: {d.message}")
# MD001:1 heading-increment: Heading incremented by more than 1
```

### `fix(source, gfm_opts=None, parse_opts=None, emoji_opts=None, diagram_opts=None, lint_opts=None, default_language=None, lint_config=None) -> FixResult`

Lint and auto-fix a markdown string. Returns the fixed output and any remaining diagnostics.

```python
result = mordant.fix("# Title  \n\n\nText")
print(result.output)       # Fixed source
print(result.fixed)        # Diagnostics that were auto-corrected
print(result.unfixable)    # Diagnostics that could not be fixed
print(result.remaining)    # Diagnostics remaining after re-linting

# With options
result = mordant.fix(
    "trailing   \n\n\ncontent\n",
    lint_config=mordant.LintConfig(
        params=mordant.RuleParams(default_language="python"),
    ),
)
print(result.output)
# 'trailing\n\ncontent\n'
```

---

## Rule Introspection

### `lint_rules() -> list[RuleMetadata]`

Return metadata for all registered lint rules.

```python
import mordant

for r in mordant.lint_rules():
    print(f"{r.id}: {r.name} - {r.description} (fixable={r.fixable})")
# MD001: heading-increment - Heading levels should increment by one at a time (fixable=False)
# MD009: no-trailing-spaces - Lines should not have trailing spaces (fixable=True)
# ...
```

### RuleMetadata

| Attribute | Type | Description |
|-----------|------|-------------|
| `id` | `str` | Rule ID (e.g., `"MD001"`) |
| `name` | `str` | Rule name (e.g., `"heading-increment"`) |
| `description` | `str` | Human-readable description |
| `fixable` | `bool` | True if the rule has an auto-fix |
| `default_params` | `str` | JSON string of default parameters |

---

## Batch API

### `lint_many(files) -> list[tuple[str, list[Diagnostic]]]`

Lint multiple files in parallel. Each file is `("filename", source)` tuple. GIL is released for the entire batch.

```python
results = mordant.lint_many([
    ("file1.md", "# Hello\n\n### Jump\n"),
    ("file2.md", "# Hi\n\n## Hello\n"),
])
# [("file1.md", [Diagnostic(...)]), ("file2.md", [])]

for name, diags in results:
    for d in diags:
        print(f"{name}:{d.rule}:{d.line} {d.name}")
```

### `fix_many(files) -> list[tuple[str, FixResult]]`

Fix multiple files in parallel.

```python
results = mordant.fix_many([
    ("file1.md", "trailing   \n"),
    ("file2.md", "more trailing  \n"),
])
# [("file1.md", FixResult(output='trailing\n', remaining=[])), ...]
```

---

## CLI Usage

```bash
# Basic lint
python -m mordant file1.md file2.md

# Fix in place
python -m mordant --fix file.md

# Dry run (show what would be fixed)
python -m mordant --fix --dry-run file.md

# Output format: human (default), json, github
python -m mordant --format json file.md
python -m mordant --format github file.md

# Config file (.markdownlint.json)
python -m mordant --config .markdownlint.json file.md

# Enable/disable specific rules
python -m mordant --enable MD001,MD009 file.md
python -m mordant --disable MD040 file.md

# Default language for code blocks
python -m mordant --fix --default-language python file.md

# Glob patterns
python -m mordant "*.md"

# Directory recursion
python -m mordant ./docs/

# Exit codes
# 0 = no issues found
# 1 = issues found
```

### Output Formats

**Human** (default):
```
file.md:1:1 MD001 [warning] heading-increment: Heading incremented by more than 1
file.md:5:1 MD042 [warning] no-empty-links: Link has an empty destination
```

**JSON**:
```json
[
  {
    "file": "file.md",
    "rule": "MD001",
    "name": "heading-increment",
    "message": "Heading incremented by more than 1",
    "line": 1,
    "severity": "warning",
    "column": 1
  }
]
```

**GitHub Actions**:
```
::warning file=file.md,line=1,col=1::MD001: heading-increment - Heading incremented by more than 1
```

---

## Suppression

Inline suppression comments are supported:

```markdown
<!-- markdownlint-disable MD001 -->
# H1

### H3 jump

<!-- markdownlint-enable MD001 -->
```

Multiple rules:
```markdown
<!-- markdownlint-disable MD001 MD042 -->
<!-- markdownlint-disable-next-line MD009 -->
```

---

## Lint Classes

### Diagnostic

| Attribute | Type | Description |
|-----------|------|-------------|
| `rule` | `str` | Rule ID (e.g., `"MD001"`) |
| `name` | `str` | Rule name (e.g., `"heading-increment"`) |
| `message` | `str` | Human-readable description |
| `line` | `int \| None` | Source line number (1-indexed) |
| `severity` | `str` | `"warning"` or `"error"` |
| `column` | `int \| None` | Byte offset within line |
| `span` | `tuple[int, int] \| None` | `[start_byte, end_byte)` in source |
| `fixable` | `bool` | True if diagnostic has an auto-fix |

### FixResult

| Attribute | Type | Description |
|-----------|------|-------------|
| `output` | `str` | Fixed source text |
| `fixed` | `list[Diagnostic]` | Diagnostics that were auto-corrected |
| `unfixable` | `list[Diagnostic]` | Diagnostics that could not be auto-fixed |
| `remaining` | `list[Diagnostic]` | Diagnostics remaining after re-linting output |

### LintConfig

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `disable` | `list[str]` | `[]` | Rule IDs to disable |
| `enable` | `list[str] \| None` | `None` | If set, ONLY these rules run |
| `suppressions` | `list[SuppressionDirective]` | `[]` | Parsed from inline comments |
| `params` | `RuleParams` | defaults | Per-rule parameters |

### RuleParams

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `heading_style` | `str` | `"consistent"` | MD003: heading style |
| `line_length` | `int` | `80` | MD013: max line length |
| `line_length_ignore_threshold` | `int` | `0` | MD013: ignore lines below this |
| `spaces_per_tab` | `int` | `4` | MD010: spaces per tab |
| `siblings_only` | `bool` | `False` | MD024: only compare sibling headings |
| `default_language` | `str \| None` | `None` | MD040: default language for fixes |

### LintOptions

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `disable` | `list[str]` | `[]` | Rule IDs to disable |
| `enable` | `list[str] \| None` | `None` | If set, only these rules run |

---

## Document API

| Property/Method | Type | Description |
|-----------------|------|-------------|
| `doc.kind` | `str` | Always `"Document"` |
| `doc.type` | `str` | Always `"block"` |
| `doc.source` | `str` | Original markdown source |
| `doc.text` | `str` | All descendant text content (recursive) |
| `doc.children` | `list[Node]` | Direct child nodes |
| `doc.metadata` | `dict` | YAML frontmatter (empty `{}` if none) |
| `doc.walk(mode)` | `Walker` | AST walker: `"depth"` or `"breadth"` |
| `doc.lint(lint_opts=None, lint_config=None)` | `list[Diagnostic]` | Lint the already-parsed document |
| `doc.fix(lint_opts=None, default_language=None, lint_config=None)` | `FixResult` | Lint and auto-fix the document |
| `doc.__repr__()` | `str` | `"<Document source_len=N>"` |

---

## Node API

| Property | Type | Description |
|----------|------|-------------|
| `node.kind` | `str` | Node kind: `"Heading"`, `"Paragraph"`, `"Link"`, `"Diagram"`, etc. |
| `node.type` | `str` | `"block"` or `"inline"` |
| `node.text` | `str` | Resolved text content (recursive from all descendants) |
| `node.parent` | `Node \| None` | Parent node |
| `node.children` | `list[Node]` | Direct child nodes |
| `node.next_sibling` | `Node \| None` | Next sibling |
| `node.previous_sibling` | `Node \| None` | Previous sibling |
| `node.has_children` | `bool` | Has child nodes |
| `node.attributes` | `dict` | HTML attributes |
| `node.line` | `int \| None` | Byte offset for Text nodes; line number for others |
| `node.emoji` | `str \| None` | Unicode emoji character for emoji nodes |
| `node.shortcode` | `str \| None` | Shortcode name for emoji nodes (e.g. `"joy"`) |
| `node.name` | `str \| None` | Full name for emoji nodes (e.g. `"grinning face with smiling eyes"`) |
| `node.diagram_type` | `str \| None` | Diagram type for diagram nodes (e.g. `"mermaid"`) |
| `node.diagram_value` | `str` | Diagram source content for diagram nodes |
| `node.__repr__()` | `str` | `"<Node kind=N ref=R>"` |

### Kind-Specific Properties

| Node Kind | Property | Type | Description |
|-----------|----------|------|-------------|
| Heading | `level` | `int \| None` | Heading level (1-6) |
| Link, Image | `destination` | `str \| None` | URL |
| Link, Image | `title` | `str \| None` | Link title |
| CodeBlock | `language` | `str \| None` | Language identifier |
| CodeBlock | `code` | `str` | Code content (empty for non-CodeBlock) |
| List | `is_tight` | `bool \| None` | Tight list (no blank lines) |
| List | `start` | `int \| None` | Starting number (0 for ul) |
| List | `marker` | `str \| None` | Marker char: `"-"`, `"+"`, `"."`, `")"` |
| ListItem | `is_task` | `bool \| None` | Task list item |
| ListItem | `task_status` | `str \| None` | `"active"` or `"completed"` |
| TableCell | `alignment` | `str \| None` | `"left"`, `"center"`, `"right"`, `"none"` |
| Diagram | `diagram_type` | `str \| None` | Always `"mermaid"` |
| Diagram | `diagram_value` | `str` | Diagram source content |

---

## Highlighter Classes

### PyHighlighter

```python
hl = mordant.Highlighter(theme="Dracula", mode="Attribute")
html = hl.highlight("python", "def hello():
    pass")
```

| Constructor | Type | Default | Description |
|-------------|------|---------|-------------|
| `theme` | `str` | `"InspiredGitHub"` | Theme name for highlighting |
| `mode` | `str` | `"Attribute"` | `"Attribute"` (inline `style`) or `"Class"` (CSS `class`) |

| Method | Return Type | Description |
|--------|-------------|-------------|
| `highlight(language, code)` | `str` | Highlight code snippet and return HTML |

### PyHighlightingMode

| Value | Description |
|-------|-------------|
| `Attribute` | Inline `style` attributes (default) |
| `Class` | CSS `class` attributes |

---

## Walker API

```python
doc = mordant.parse("# Hello\n\n**World**")

# Depth-first (DFS) — document first, children pushed in reverse
for node in doc.walk("depth"):
    print(node.kind, node.text)

# Breadth-first (BFS) — document first, children enqueued left-to-right
for node in doc.walk("breadth"):
    print(node.kind, node.text)
```

| Method | Return Type | Description |
|--------|-------------|-------------|
| `__iter__()` | `Walker` | Returns self (iterator protocol) |
| `__next__()` | `Node \| None` | Next node in traversal order |

---

## MarkdownChunker

Lazy, low-copy chunking iterator over the rushdown AST. Yields one chunk (a `str`) at a time. Headings update a "current header" context; each subsequent body block is yielded either standalone or prefixed with that header.

```python
import mordant

# Basic usage — heading context propagation
chunker = mordant.MarkdownChunker("# Section\n\nPara one\n\n## Sub\n\nPara two")
chunks = list(chunker)
assert len(chunks) == 2
assert chunks[0] == "# Section\n\nPara one"
assert chunks[1] == "## Sub\n\nPara two"

# current_header tracks the last heading seen
assert chunker.current_header == "## Sub"

# node_count includes all top-level nodes (headings, paragraphs, thematic breaks, etc.)
assert chunker.node_count == 4  # 2 headings + 2 paragraphs
```

| Constructor / Method | Return Type | Description |
|----------------------|-------------|-------------|
| `MarkdownChunker(text)` | — | Build from Python string. Parses immediately; GIL released during parsing. |
| `MarkdownChunker.from_file(path)` | — | Read `path`, validate UTF-8, own bytes as `String`. Safe path. |
| `MarkdownChunker.from_file_mmap(path)` | — | Zero-copy variant that memory-maps `path`. **Safety invariant:** caller MUST NOT modify/truncate the file while chunker is alive. |
| `__iter__()` | `MarkdownChunker` | Returns self (iterator protocol). |
| `__next__()` | `str \| None` | Advance to next block chunk, or `None` (→ `StopIteration`). |
| `current_header` | `str \| None` | Current heading context (last top-level heading seen), or `None`. |
| `node_count` | `int` | Number of top-level nodes extracted (with a source position). |

**Chunking behaviour:**

| Node Kind | Yielded? | Context Update |
|-----------|----------|----------------|
| Heading | No (not yielded) | Updates `current_header` |
| Paragraph | Yes | Uses current header as prefix |
| CodeBlock | Yes | Uses current header as prefix |
| List | Yes | Uses current header as prefix |
| Table | Yes | Uses current header as prefix |
| Blockquote | Yes | Uses current header as prefix |
| ThematicBreak / HtmlBlock / LinkRefDef | No (skipped) | Does NOT reset heading context |

**Example — from_file:**

```python
chunker = mordant.MarkdownChunker.from_file("/path/to/doc.md")
for chunk in chunker:
    print(chunk)
```

**Example — from_file_mmap (zero-copy):**

```python
chunker = mordant.MarkdownChunker.from_file_mmap("/path/to/large.md")
chunks = list(chunker)
# Zero-copy: the file is memory-mapped, no full read into Python memory
```

**Example — nested headings don't leak:**

```python
# A heading inside a blockquote must never become the context prefix
chunker = mordant.MarkdownChunker("# Outer\n\n> # Nested\n\n> Quote text.")
chunks = list(chunker)
# The paragraph after the blockquote carries "# Outer", not "# Nested"
assert all(not c.startswith("# Nested") for c in chunks)
```

---

## Emoji Extension

### `:joy:`, `:heart:`, `:smile:` etc.

```python
import mordant

# Basic emoji rendering
html = mordant.markdown_to_html("I'm :joy:")
# '<p>I'm 😀</p>\n'

# Multiple emojis
html = mordant.markdown_to_html(":heart: :smile: :joy:")
# '<p>❤️ 😊 😀</p>\n'

# Invalid shortcode passes through
html = mordant.markdown_to_html(":invalid:")
# '<p>:invalid:</p>\n'

# Inside code spans (not parsed)
html = mordant.markdown_to_html("` :joy: `")
# '<p><code> :joy: </code></p>\n'
```

### PyEmojiParserOptions

```python
opts = mordant.PyEmojiParserOptions(
    blacklist=None,       # Comma-separated shortcodes to ignore
)

# Blacklist example
opts = mordant.PyEmojiParserOptions(blacklist="joy,heart")
html = mordant.markdown_to_html(":joy: :heart:", emoji_parse_opts=opts)
# ':joy:' passes through (blacklisted)
# :heart: renders as ❤️ (if not blacklisted)
```

### PyEmojiHtmlRendererOptions

```python
opts = mordant.PyEmojiHtmlRendererOptions(
    template=None,        # Custom template: {emoji}, {shortcode}, {name}
)

# Custom HTML img tag
opts = mordant.PyEmojiHtmlRendererOptions(
    template='<img src="https://cdn.example.com/{shortcode}.png" />'
)
html = mordant.markdown_to_html(":joy:", emoji_render_opts=opts)
# '<img src="https://cdn.example.com/joy.png" />'

# Name-based template
opts = mordant.PyEmojiHtmlRendererOptions(template="{name} emoji")
html = mordant.markdown_to_html(":joy:", emoji_render_opts=opts)
# 'grinning face with smiling eyes emoji'
```

---

## Diagram Extension

### ` ```mermaid ` code blocks

```python
import mordant

# Basic Mermaid diagram
html = mordant.markdown_to_html("""```mermaid
graph LR
    A --- B
```""")
# '<pre class="mermaid">\ngraph LR\n    A --- B\n</pre>\n<script type="module">...'

# Sequence diagram
html = mordant.markdown_to_html("""```mermaid
sequenceDiagram
    Alice->>Bob: Hello Bob
    Bob-->>Alice: Hi Alice
```""")

# Multiple diagrams (single script tag)
html = mordant.markdown_to_html("""```mermaid
graph LR
    A --- B
```

```mermaid
sequenceDiagram
    Alice->>Bob: Hello
```""")
# Two <pre class="mermaid"> blocks, one <script> tag
```

### PyDiagramParserOptions

```python
opts = mordant.PyDiagramParserOptions(
    mermaid_enabled=True,   # Enable/disable Mermaid diagram transformation
)

# Disable diagrams (keeps as regular code block)
opts = mordant.PyDiagramParserOptions(mermaid_enabled=False)
html = mordant.markdown_to_html("```mermaid\ngraph LR\nA --- B\n```", diagram_parse_opts=opts)
# '<pre><code>graph LR\n    A --- B\n</code></pre>\n'
```

### PyDiagramHtmlRendererOptions

```python
opts = mordant.PyDiagramHtmlRendererOptions(
    mermaid_url=None,       # Custom Mermaid.js CDN URL
)

# Custom URL
opts = mordant.PyDiagramHtmlRendererOptions(
    mermaid_url="https://cdn.example.com/mermaid.mjs"
)
html = mordant.markdown_to_html("```mermaid\ngraph LR\nA --- B\n```", diagram_render_opts=opts)
# Script tag uses custom URL
```

### Diagram AST Access

```python
doc = mordant.parse("""```mermaid
graph LR
    A --- B
```""")

# Find diagram nodes
diagram_nodes = [n for n in doc.walk("depth") if n.kind == "Diagram"]
for node in diagram_nodes:
    print(node.diagram_type)   # "mermaid"
    print(node.diagram_value)  # "graph LR\n    A --- B\n"
```

---

## Math Extension (KaTeX)

### ` ```math ` and ` ```latex ` fenced code blocks

```python
import mordant

# Basic math block
html = mordant.markdown_to_html("""```math
\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}
```""")
# Contains <span class="katex katex-display">...</span>

# Using 'latex' language tag (same as 'math')
html = mordant.markdown_to_html("""```latex
E = mc^2
```""")
```

### Standalone `render_math()` function

Render LaTeX independently of the Markdown AST. GIL is released during rendering.

```python
import mordant

# Inline math (default)
result = mordant.render_math("x^2 + y^2")
# '<span class="katex">...</span>'

# Display math
result = mordant.render_math("E = mc^2", display=True)
# '<span class="katex katex-display">...</span>'

# Output formats
result = mordant.render_math("x^2", output="html")     # HTML only
result = mordant.render_math("x^2", output="mathml")   # MathML only
result = mordant.render_math("x^2", output="both")     # HTML + MathML (default)

# Invalid LaTeX produces error span (doesn't crash)
result = mordant.render_math(r"\\nonexistentcommand{}")
# '<span class="katex-error" title="...">...<\/span>'
```

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `latex` | `str` | — | LaTeX expression to render |
| `display` | `bool` | `False` | `True` for display mode (`$$...$$`), `False` for inline (`$...$`) |
| `output` | `str` | `"both"` | Output format: `"both"` (HTML+MathML), `"html"`, or `"mathml"` |

**Output formats:**

| Format | Description | Requires |
|--------|-------------|----------|
| `"both"` (default) | Styled HTML + MathML | KaTeX CSS + web fonts |
| `"html"` | Styled HTML only | KaTeX CSS + web fonts |
| `"mathml"` | Semantic MathML only | MathML-capable browser |

### Inline `$...$` and block `$$...$$` math

```python
import mordant

# Inline math
html = mordant.markdown_to_html("The value of $x^2$ is important.")

# Block math
html = mordant.markdown_to_html("Equation:\n\n$$E = mc^2$$\n\nMore text.")
```

### Math AST Access

```python
doc = mordant.parse("""```math
x^2 + y^2 = z^2
```""")

# Find math nodes
math_nodes = [n for n in doc.walk("depth") if n.kind == "Extension" and hasattr(n, 'latex')]
for node in math_nodes:
    print(node.latex)  # Raw LaTeX source
    print(node.display)  # True for $$...$$, False for $...$
```

---

## Options

### ParseOptions

```python
opts = mordant.ParseOptions(
    smart=False,              # Not yet implemented (no-op)
    attributes=False,         # Parse node attributes
    auto_heading_ids=False,   # Auto-generate heading IDs
    escaped_space=False,      # Treat \ as space escape
    meta_table=False,         # Render metadata as HTML table in AST
)
```

### RenderOptions

```python
opts = mordant.RenderOptions(
    hard_wraps=False,         # Soft line breaks → <br>
    xhtml=False,              # XHTML style (<br />)
    allows_unsafe=False,      # Allow raw HTML / dangerous URLs
    escaped_space=False,      # Don't render backslash-escaped space
)
```

### GfmOptions

```python
import mordant

# Default: tables + strikethrough + task lists (linkify disabled)
opts = mordant.GfmOptions()
opts.has(mordant.GfmFeature.Table)        # True
opts.has(mordant.GfmFeature.Strikethrough) # True
opts.has(mordant.GfmFeature.TaskList)      # True
opts.has(mordant.GfmFeature.Linkify)       # False

# All features (including linkify)
opts = mordant.GfmOptions.all()

# None
opts = mordant.GfmOptions.none()

# Granular feature selection
opts = mordant.GfmOptions(features=[
    mordant.GfmFeature.Table,
    mordant.GfmFeature.Strikethrough,
])
```

| Classmethod | Description |
|-------------|-------------|
| `GfmOptions.all()` | Enable all features (tables, strikethrough, task lists, linkify) |
| `GfmOptions.none()` | Disable all GFM features |
| `GfmOptions(features=[...])` | Enable specific features |

| Attribute/Method | Return Type | Description |
|------------------|-------------|-------------|
| `features` | `list[GfmFeature]` | Enabled GFM feature list |
| `has(feature)` | `bool` | Check if a specific feature is enabled |

### GfmFeature Enum

| Value | Description |
|-------|-------------|
| `GfmFeature.Table` | GFM tables |
| `GfmFeature.Strikethrough` | GFM strikethrough (`~~text~~`) |
| `GfmFeature.TaskList` | GFM task list items (`- [ ]`) |
| `GfmFeature.Linkify` | GFM autolink (auto-convert URLs) |

### ArenaOptions

```python
opts = mordant.ArenaOptions(
    initial_size=1024,        # Initial arena capacity
)
```

> **Note:** `ArenaOptions` is exposed but not yet passed to the parser. The parser always uses the default arena options.

---

## YAML Frontmatter

```python
md = """---
title: My Document
author: Jane
date: 2026-01-15
tags: [rust, markdown]
---

Hello world
"""

doc = mordant.parse(md)
print(doc.metadata)
# {'title': 'My Document', 'author': 'Jane', 'date': '2026-01-15', 'tags': ['rust', 'markdown']}

# Types are preserved
assert isinstance(doc.metadata["tags"], list)
assert isinstance(doc.metadata["date"], str)
```

Supported types: `null`, `bool`, `int`, `float`, `str`, `list`, `dict`.  
**Not supported:** YAML anchors/aliases.

---

## Thematic Break vs Frontmatter

The meta parser uses lookahead to distinguish `---` (thematic break) from `---\nkey: value` (frontmatter):

```python
# Thematic break (not frontmatter)
doc = mordant.parse("---")
assert doc.metadata == {}
assert any(n.kind == "ThematicBreak" for n in doc.children)

# Frontmatter
doc = mordant.parse("---\ntitle: Test\n---\n\nBody")
assert doc.metadata["title"] == "Test"

# Five dashes is thematic break
doc = mordant.parse("-----")
assert doc.metadata == {}

# ---\n + empty/whitespace is thematic break
doc = mordant.parse("---\n\nBody")
assert doc.metadata == {}

# ---\n + plain text (no colon) is thematic break + setext heading
doc = mordant.parse("---\nFoo\n---")
assert doc.metadata == {}

# YAML-like content is frontmatter (contains colon, starts with "- ", "|", ">")
doc = mordant.parse("---\ntitle: Test\n---\n\nBody")
assert "title" in doc.metadata

doc = mordant.parse("---\n- item\n---\n\nBody")
assert True  # Starts with "- " → frontmatter

doc = mordant.parse("---\n| block scalar\n---\n\nBody")
assert True  # Starts with "|" → frontmatter
```

---

## Theme Loading

### Available Themes

```python
import mordant

# List all available themes (built-in + embedded + user)
themes = mordant.list_themes()
print(f"Total themes: {len(themes)}")
for t in sorted(themes)[:10]:
    print(f"  - {t}")
```

### Theme Sources

| Source | Location | Format |
|--------|----------|--------|
| **Embedded** | `mordant/themes/` (package) | `.tmTheme` + `.json` |
| **User** | `~/.mordant/themes/` | `.tmTheme` + `.json` |
| **AppData** | `%APPDATA%/mordant/themes/` (Windows) | `.tmTheme` + `.json` |
| **Built-in** | `syntect-assets` (bat's themes) | Syntect format |

### Custom Themes

```python
# Add a VSCode JSON theme
vscode_theme = '''{
    "name": "My Theme",
    "type": "dark",
    "tokenColors": [
        {"scope": "comment", "settings": {"foreground": "#888888"}},
        {"scope": "keyword", "settings": {"foreground": "#FF6B6B"}}
    ]
}'''

mordant.add_custom_theme("my-vscode", vscode_theme)

# Verify it's loaded
assert "my-vscode" in mordant.list_themes()

# Use it for highlighting
hl = mordant.Highlighter(theme="my-vscode")
html = hl.highlight("python", "def hello(): pass")

# Use it for markdown rendering
html = mordant.markdown_to_html("# Hello", highlighting_theme="my-vscode")
```

### User Theme Directory

Place `.json` or `.tmTheme` files in `~/.mordant/themes/` (or `%APPDATA%/mordant/themes/` on Windows) to have them auto-loaded at import time:

```bash
# Create user theme directory
mkdir -p ~/.mordant/themes

# Place a VSCode JSON theme
# ~/.mordant/themes/my-dark.json
```

JSON themes are parsed through the same VSCode theme conversion pipeline (`parse_vscode_theme_jsonc` → `vscode_theme_to_syntect`). Failed loads print a warning to stderr but don't crash.

---

## AST Traversal Examples

```python
doc = mordant.parse("# Hello\n\n**World**")

# Navigate up
heading = doc.children[0]
parent = heading.parent        # Document
grandparent = parent.parent    # None

# Navigate down
para = doc.children[1]
strong = para.children[0]      # Strong node

# Siblings
heading = doc.children[0]
para = heading.next_sibling    # Paragraph

# Sibling chain
for child in doc.children:
    print(child.kind, child.text)
    # Heading Hello
    # Paragraph World

# Walk all nodes with depth tracking
def walk_with_depth(doc):
    """Yield (node, depth) for all nodes in depth-first order."""
    stack = [(doc, 0)]
    while stack:
        node, depth = stack.pop()
        yield node, depth
        # Push children in reverse so first child is processed first
        for child in reversed(node.children):
            stack.append((child, depth + 1))

for node, depth in walk_with_depth(doc):
    print("  " * depth + node.kind, node.text)

# Find nodes by kind
def find_all(doc, kind):
    return [n for n in doc.walk("depth") if n.kind == kind]

headings = find_all(doc, "Heading")
links = find_all(doc, "Link")
diagrams = find_all(doc, "Diagram")

# Access emoji node properties
emoji_nodes = find_all(doc, "Extension")
for node in emoji_nodes:
    if node.emoji:
        print(f"Emoji: {node.emoji} ({node.shortcode}) - {node.name}")

# Access diagram node properties
for node in diagrams:
    print(f"Diagram: {node.diagram_type}\n{node.diagram_value}")
```

---

## Error Handling

```python
import mordant

# YAML parse error — raised on metadata access
try:
    doc = mordant.parse("---\ninvalid: yaml: [broken")
    meta = doc.metadata  # Raises ValueError
except ValueError as e:
    print(e)  # YAML parsing error message

# Using RushdownError directly
from mordant import RushdownError
try:
    err = RushdownError("custom error")
    print(err.message)  # "custom error"
    print(str(err))     # "custom error"
except Exception as e:
    print(e)
```

---

## Rule Catalog

| ID | Name | Description | Fixable |
|----|------|-------------|---------|
| MD001 | heading-increment | Headings increment by 1 | no |
| MD003 | heading-style | Heading style consistency | no |
| MD009 | no-trailing-spaces | No trailing whitespace | yes |
| MD010 | no-hard-tabs | No hard tabs (spaces preferred) | yes |
| MD012 | no-multiple-blanks | No multiple blank lines | yes |
| MD013 | line-length | Lines should not exceed max length | no |
| MD018 | atx-spacing | ATX heading space after # | no |
| MD019 | atx-closing-spaces | ATX leaf headings no closing # | no |
| MD020 | atx-spacing | ATX heading space before closing # | no |
| MD021 | atx-heading-space | Multiple spaces inside ATX heading | no |
| MD022 | heading-blank-lines | Headings should have blank lines around them | yes |
| MD024 | no-duplicate-heading | No duplicate headings | no |
| MD025 | single-h1 | Single H1 per document | no |
| MD026 | no-trailing-punctuation | Headings should not end with trailing punctuation | yes |
| MD031 | fenced-code-blocks-working | Fenced code blocks should have blank lines around them | yes |
| MD032 | indented-code-block | Indented code blocks should have blank lines around them | no |
| MD034 | no-bare-urls | Bare URLs should be in angle brackets | no |
| MD040 | fenced-code-language | Fenced code blocks should specify a language | yes |
| MD042 | no-empty-links | Links should have a non-empty destination | no |
| MD045 | no-alt-text | Images should have alt text | no |
| MD046 | code-block-indentation | Fenced code blocks should use 4-space indentation | no |
| MD047 | single-trailing-newline | Files should end with a single trailing newline | yes |
| MD048 | fenced-code-block-punctuation | Fenced code blocks should use backticks, not tildes | no |
| MD049 | emphasis-style | Emphasis style consistency | no |
| MD050 | strong-style | Strong style consistency | no |

---

## GFM Examples

```python
import mordant

# Tables (enabled by default)
html = mordant.markdown_to_html(
    "| A | B |\n|---|---|\n| 1 | 2 |"
)

# Task lists (enabled by default)
md = "- [ ] todo\n- [x] done"
html = mordant.markdown_to_html(md)

# Strikethrough (enabled by default)
html = mordant.markdown_to_html("~~deleted~~")

# Autolink (disabled by default; enable with GfmOptions.all())
html = mordant.markdown_to_html(
    "https://example.com",
    gfm_opts=mordant.GfmOptions.all()
)
# '<p><a href="https://example.com">https://example.com</a></p>\n'
```

---

## Multi-threaded Usage

```python
import threading
from concurrent.futures import ThreadPoolExecutor
import mordant

def parse_and_render(md):
    # GIL is released during parse + render
    html = mordant.markdown_to_html(md)
    return html

docs = [open(f).read() for f in file_list]
with ThreadPoolExecutor(max_workers=4) as pool:
    results = list(pool.map(parse_and_render, docs))
# ~4.0x linear scaling vs single-threaded
```

> **Tip:** For batch linting/fixing, prefer the built-in `lint_many()` / `fix_many()` API — it handles parallelism internally via `rayon` and releases the GIL for the entire batch, which is simpler and faster than manual threading.

---

## Node Kind Reference

| Kind | Type | Example |
|------|------|---------|
| Document | block | Root node |
| Paragraph | block | `Hello world` |
| Heading | block | `# Title` |
| ThematicBreak | block | `---` |
| CodeBlock | block | ` ```python ... ``` ` |
| Blockquote | block | `> quoted` |
| List | block | `- item` |
| ListItem | block | `- [x] done` |
| HtmlBlock | block | `<div>...</div>` |
| Text | inline | Plain text |
| CodeSpan | inline | `` `code` `` |
| Emphasis | inline | `*italic*` |
| Strong | inline | `**bold**` |
| Link | inline | `[text](url)` |
| Image | inline | `![alt](url)` |
| RawHtml | inline | `<span>` |
| LinkReferenceDefinition | block | `[ref]: url` |
| Table | block | `| A | B |` |
| TableHeader | block | Header row |
| TableBody | block | Body rows |
| TableRow | block | `<tr>` |
| TableCell | block | `<td>` |
| Strikethrough | inline | `~~text~~` |
| Diagram | block | ` ```mermaid ... ``` ` |
| Extension | any | Custom nodes |

---

## Parser Dispatch Priority (Block)

| Priority | Parser | Trigger |
|----------|--------|---------|
| 900 | HtmlBlockParser | HTML tags |
| 800 | BlockquoteParser | `>` |
| 700 | FencedCodeBlockParser | ` ``` ` |
| 600 | AtxHeadingParser | `#` |
| 500 | IndentedCodeBlockParser | 4+ spaces |
| 400 | ListItemParser | Nested items |
| 300 | ListParser | `-`, `+`, `*`, `1.` |
| 200 | ThematicBreakParser | `---`, `***`, `___` |
| 100 | SetextHeadingParser | `===`, `---` underline |
| 1000 | ParagraphParser | Default fallback |

---

## Inline Parser Dispatch Priority

| Priority | Parser | Trigger |
|----------|--------|---------|
| 100 | CodeSpanParser | `` ` `` |
| 200 | LinkParser | `[`, `]`, `(`, `)` |
| 300 | AutoLinkParser | URLs, emails |
| 400 | RawHtmlParser | `<`, `!` |
| 500 | EmphasisParser | `*`, `_` |

---

## Performance Benchmarks

### Single-threaded (50 iterations)

| Fixture | mordant | mistune | markdown-it-py | python-markdown |
|---------|---------|---------|----------------|-----------------|
| Small (400B) | **0.039ms** | 0.430ms | 0.475ms | 2.301ms |
| Medium (5.4KB) | **0.155ms** | 2.448ms | 3.940ms | 6.455ms |
| Large (26.7KB) | **0.410ms** | 8.611ms | 16.743ms | 31.304ms |
| Data (202KB) | **2.763ms** | 38.152ms | 65.736ms | 621.295ms |

### Multi-threaded (4 threads, medium fixture)

| Library | 1-thread | 4-threads | Scaling |
|---------|----------|-----------|---------|
| **mordant** | ~1,000 docs/s | ~4,000 docs/s | **4.0x** |
| python-markdown | ~59 docs/s | ~257 docs/s | 4.35x |
| mistune | ~133 docs/s | ~542 docs/s | 4.07x |
| markdown-it-py | ~83 docs/s | ~337 docs/s | 4.06x |

---

## Memory Model

```
Document ──┬── arena: Rc<RefCell<Arena>>   ← shared with Node/Walker
           ├── source: Rc<str>              ← shared source (refcount bump, no deep copy)
           └── root_ref: NodeRef            ← root of AST tree

Node ──────┬── arena: Rc<RefCell<Arena>>   ← same arena as Document
           ├── node_ref: NodeRef            ← pointer into arena
           └── source: Rc<str>              ← shared source (refcount bump on navigation)

Walker ────┬── arena: Rc<RefCell<Arena>>   ← same arena as Document
           ├── source: Rc<str>              ← shared source (refcount bump)
           ├── mode: "depth" | "breadth"
           ├── stack: Vec<NodeRef>          ← DFS stack
           └── queue: Vec<NodeRef>          ← BFS queue
```

`source` is shared via `Rc<str>` across all three classes. Every `Node` created during navigation or walking bumps the refcount instead of deep-copying the source. When `Document` is garbage-collected, the `Rc` reference count drops to 0, freeing the Arena and all AST nodes. Share `Document` between `Node` and `Walker` objects to keep the AST alive.

---

## GIL Release

Parse, render, lint, and fix operations release the GIL via `Python::detach()`:

```python
# These calls release the GIL internally:
mordant.markdown_to_html(source)   # GIL released during parse + render
mordant.parse(source)                          # GIL released during parse
mordant.lint(source)                           # GIL released during linting
mordant.fix(source)                            # GIL released during linting + fixing
```

This enables true multi-threaded parallelism. Use `ThreadPoolExecutor` or `threading` for concurrent processing:

```python
from concurrent.futures import ThreadPoolExecutor

with ThreadPoolExecutor(max_workers=4) as pool:
    results = list(pool.map(mordant.markdown_to_html, markdown_docs))
```

Or use the built-in batch API for parallel file processing:

```python
# Batch lint — GIL released for entire batch
results = mordant.lint_many([
    ("file1.md", open("file1.md").read()),
    ("file2.md", open("file2.md").read()),
])

# Batch fix — GIL released for entire batch
results = mordant.fix_many([
    ("file1.md", open("file1.md").read()),
    ("file2.md", open("file2.md").read()),
])
```

---

## Common Patterns

### Extract all links

```python
def extract_links(doc):
    links = []
    for node in doc.walk("depth"):
        if node.kind == "Link" and node.destination:
            links.append((node.text, node.destination, node.title))
    return links

links = extract_links(mordant.parse("[Click](https://example.com)"))
# [('Click', 'https://example.com', None)]
```

### Extract all headings

```python
def extract_headings(doc):
    headings = []
    for node in doc.walk("depth"):
        if node.kind == "Heading" and node.level:
            headings.append((node.level, node.text))
    return headings

headings = extract_headings(mordant.parse("# Title\n## Subtitle"))
# [(1, 'Title'), (2, 'Subtitle')]
```

### Extract code blocks

```python
def extract_code_blocks(doc):
    blocks = []
    for node in doc.walk("depth"):
        if node.kind == "CodeBlock":
            blocks.append({
                "language": node.language,
                "code": node.code,
            })
    return blocks
```

### Extract diagrams

```python
def extract_diagrams(doc):
    diagrams = []
    for node in doc.walk("depth"):
        if node.kind == "Diagram":
            diagrams.append({
                "type": node.diagram_type,
                "value": node.diagram_value,
            })
    return diagrams

diagrams = extract_diagrams(mordant.parse("```mermaid\ngraph LR\nA --- B\n```"))
# [{'type': 'mermaid', 'value': 'graph LR\n    A --- B\n'}]
```

### Walk with indentation tracking

```python
def walk_tree(doc, max_depth=None):
    """Walk AST tree with indentation tracking."""
    stack = [(doc, 0)]
    while stack:
        node, depth = stack.pop()
        if max_depth is not None and depth > max_depth:
            continue
        indent = "  " * depth
        print(f"{indent}{node.kind}: {node.text[:50]}")
        for child in reversed(node.children):
            stack.append((child, depth + 1))

walk_tree(mordant.parse("# Title\n\n**Bold** and *italic*"))
```
