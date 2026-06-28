# Mordant Quick Reference

> **Version:** 0.5.0  
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

### `markdown_to_html(source, gfm=False, parse_opts=None, render_opts=None, emoji_parse_opts=None, emoji_render_opts=None, diagram_parse_opts=None, diagram_render_opts=None) -> str`

One-call parse + render. GIL is released during the CPU-heavy parse + render phase.

```python
import mordant

# Basic
html = mordant.markdown_to_html("# Hello\n\n**World**")
# '<h1>Hello</h1>\n<p><strong>World</strong></p>\n'

# GFM
html = mordant.markdown_to_html("~~strike~~", gfm=True)
# '<p><del>strike</del></p>\n'

# With options
html = mordant.markdown_to_html(
    "Hello\nWorld",
    render_opts=mordant.RenderOptions(hard_wraps=True),
)
# '<p>Hello<br />\nWorld</p>\n'
```

### `parse(source, gfm=False, parse_opts=None, emoji_opts=None, diagram_opts=None) -> Document`

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
opts = mordant.GfmOptions(
    tables=True,              # GFM tables
    strikethrough=True,       # ~~strikethrough~~
    task_lists=True,          # - [ ] task items
    linkify=True,             # Auto-link URLs
)
```

> **Note:** `GfmOptions` is exposed but not yet wired to the parser. When `gfm=True` is passed to `parse()` or `markdown_to_html()`, the parser always uses default GFM settings.

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

## GFM Examples

```python
# Tables
html = mordant.markdown_to_html(
    "| A | B |\n|---|---|\n| 1 | 2 |",
    gfm=True,
)

# Task lists
md = "- [ ] todo\n- [x] done"
html = mordant.markdown_to_html(md, gfm=True)

# Strikethrough
html = mordant.markdown_to_html("~~deleted~~", gfm=True)

# Autolink
html = mordant.markdown_to_html("https://example.com", gfm=True)
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
    html = mordant.markdown_to_html(md, gfm=True)
    return html

docs = [open(f).read() for f in file_list]
with ThreadPoolExecutor(max_workers=4) as pool:
    results = list(pool.map(parse_and_render, docs))
# ~4.0x linear scaling vs single-threaded
```

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
           ├── source: String               ← keeps source-indexed text valid
           └── root_ref: NodeRef            ← root of AST tree

Node ──────┬── arena: Rc<RefCell<Arena>>   ← same arena as Document
           ├── node_ref: NodeRef            ← pointer into arena
           └── source: String               ← same source as Document

Walker ────┬── arena: Rc<RefCell<Arena>>   ← same arena as Document
           ├── source: String               ← same source as Document
           ├── mode: "depth" | "breadth"
           ├── stack: Vec<NodeRef>          ← DFS stack
           └── queue: Vec<NodeRef>          ← BFS queue
```

When `Document` is garbage-collected, the `Rc` reference count drops to 0, freeing the Arena and all AST nodes. Share `Document` between `Node` and `Walker` objects to keep the AST alive.

---

## GIL Release

Parse and render operations release the GIL via `Python::detach()`:

```python
# These calls release the GIL internally:
mordant.markdown_to_html(source, gfm=True)   # GIL released during parse + render
mordant.parse(source)                         # GIL released during parse
```

This enables true multi-threaded parallelism. Use `ThreadPoolExecutor` or `threading` for concurrent processing:

```python
from concurrent.futures import ThreadPoolExecutor

with ThreadPoolExecutor(max_workers=4) as pool:
    results = list(pool.map(mordant.markdown_to_html, markdown_docs))
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
