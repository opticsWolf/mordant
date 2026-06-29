# Mordant Architecture

> **Version:** 0.6.0  
> **Rust:** rushdown v0.18.0 (CommonMark 0.31.2 + GFM)  
> **Bindings:** PyO3 0.29 (Python 3.9+)  
> **Tests:** 973 Python (806 core + 29 emoji + 17 diagram + 121 lint + 12 phase8) + 54 Rust (26 unit + 2 AST + 1 CommonMark spec + 1 extension + 1 extra + 6 GFM + 2 options + 1 override renderer + 14 doc-tests)

---

## 1. Overview

Mordant is a fast CommonMark + GFM Markdown parser and renderer for Python, powered by the [rushdown](https://github.com/yuin/rushdown) Rust library. It provides:

- **Single-call parse + render:** `markdown_to_html("# Hello")`
- **AST access:** `parse("# Hello")` returns a `Document` with full tree traversal
- **YAML frontmatter:** Metadata extraction via `yaml-peg`
- **GFM support:** Tables, task lists, strikethrough, autolink
- **Emoji support:** Shortcode-based emoji rendering (`:joy:`, `:heart:`, etc.)
- **Diagram support:** Mermaid diagram rendering from code blocks
- **Lint engine:** 25 rules (MD001, MD009, MD012, MD024, MD025, MD040, MD042, MD045, MD047, MD010, MD018–MD021, MD030, MD031, MD032, MD033, MD034, MD036, MD037, MD038, MD039, MD041, MD043, MD044, MD046, MD048, MD049) with diagnostics, fix engine, config, and suppression
- **Batch API:** `lint_many()` / `fix_many()` for parallel file processing via `rayon`
- **CLI:** `python -m mordant` with `--fix`, `--dry-run`, `--format` (human/json/github), `--config`, `--enable`, `--disable`
- **GIL release:** Parse, render, and batch lint/fix operations run without the GIL for multi-threaded parallelism

---

## 2. Repository Layout

```
mordant/                          # Rushdown Rust crate (unchanged upstream)
├── src/                          # Core parser/renderer (27,801 lines)
│   ├── lib.rs                    # Public API: markdown_to_html_string, new_markdown_to_html
│   ├── ast.rs                    # Arena, NodeRef, KindData (24 node kinds)
│   ├── parser/                   # Block + inline parsers, extensions
│   ├── renderer/                 # HTML renderer, BuiltinNodesRenderer
│   ├── text.rs                   # Index, Value, Lines, BasicReader
│   ├── context.rs                # Type-safe KV store for parser/renderer
│   ├── scanner/                  # re2c-generated scanners (HTML, URLs, etc.)
│   └── error.rs                  # Error types

mordant-py/                       # PyO3 Python bindings
├── Cargo.toml                    # pyo3 0.29, rushdown (path dep), yaml-peg 1.0.9, emojis 0.8.0, rayon 1.10, serde, serde_json
├── src/
│   ├── lib.rs                    # Module entry, markdown_to_html(), parse(), lint(), fix(), lint_many(), fix_many(), GIL detach
│   ├── document.rs               # Document wrapper (Arena + source + root_ref)
│   ├── node.rs                   # Node wrapper, kind-specific properties (incl. emoji/diagram props)
│   ├── walker.rs                 # DFS/BFS AST walker
│   ├── options.rs                # ParseOptions, RenderOptions, GfmOptions, ArenaOptions
│   ├── errors.rs                 # RushdownError Python exception type
│   ├── meta.rs                   # YAML frontmatter parser extension
│   ├── emoji.rs                  # Emoji shortcode inline parser + HTML renderer + unit tests
│   ├── diagram.rs                # Mermaid diagram AST transformer + HTML renderer + post-render hook
│   ├── linter.rs                 # Lint engine: 25 rules (MD001, MD009, MD012, MD024, MD025, MD040, MD042, MD045, MD047+), diagnostics, fix engine, config, suppression, batch API
│   └── fix_engine.rs             # Fix engine: FixOp, FixResult, fixpoint checking
├── mordant/
│   ├── __init__.py               # Python re-exports: lint, fix, lint_many, fix_many, Diagnostic, FixResult, FixOp, RuleParams, PyRuleConfig, PyEnable, PyDisable, PySuppression, PyLintResult
│   └── __main__.py               # CLI: argparse, formatters (human/json/github), config loading, glob expansion, exit codes
├── tests/
│   ├── test_core.py              # 14 tests: basic CommonMark rendering
│   ├── test_ast.py               # 61 tests: Document, Node, Walker, metadata
│   ├── test_gfm.py               # 9 tests: GFM extensions
│   ├── test_options.py           # 17 tests: options propagation
│   ├── test_meta.py              # 41 tests: YAML frontmatter + thematic break conflict
│   ├── test_emoji.py             # 29 tests: emoji rendering, blacklist, templates, AST access
│   ├── test_diagram.py           # 17 tests: Mermaid diagram rendering, options, AST access
│   ├── test_commonmark_spec.py   # 652 spec cases: full CommonMark 0.31.2 spec
│   └── test_lint.py              # 133 tests: 25 rules + fixer + config + CLI + batch API + Phase 8 emoji/frontmatter/fragment anchors
└── benchmarks/                   # Performance benchmarks vs. python-markdown, mistune, markdown-it-py

pyproject/                        # Python package project (setuptools/pip install)
```

---

## 3. Rust Core (rushdown) Architecture

### 3.1. Parsing Pipeline

```
Markdown String
    │
    ▼
┌──────────────┐
│ BasicReader   │  ──►  text::Reader<'a> trait
│ (line/pos)    │       • peek_byte(), peek_line_bytes()
└──────────────┘       • advance(), advance_line()
    │
    ▼
┌──────────────┐
│  Parser       │  ──►  parser::Parser
│  (blocks)     │       • parse(reader) → (Arena, NodeRef)
│               │       • add_block_parser() / add_inline_parser()
│  Phase 1:     │       • add_ast_transformer() (post-block/inline)
│  Block        │       • add_paragraph_transformer() (para → list/table)
│  Parsing      │
└──────────────┘
    │
    ▼
┌──────────────┐
│  Parser       │  ──►  parser::Parser::parse_block()
│  (inline)     │       • Walk each block's source lines
│               │       • Run inline parsers (code spans, links, etc.)
│  Phase 2:     │       • process_delimiters() for emphasis/strong
│  Inline       │
│  Parsing      │
└──────────────┘
    │
    ▼
┌──────────────┐
│  AST          │  ──►  ast::Arena + ast::NodeRef(root)
│  Transformers │       • Link reference resolution
│               │       • Paragraph→List/Table transforms
│               │       • Diagram code block → Diagram node
└──────────────┘
```

### 3.2. Rendering Pipeline

```
Arena + NodeRef(root)
    │
    ▼
┌──────────────┐
│  Renderer     │  ──►  renderer::html::Renderer<'r, W>
│  (AST walk)   │       • render(writer, source, arena, node_ref)
│               │       • WalkStatus: Continue / Stop / SkipChildren
└──────────────┘
    │
    ▼
┌──────────────┐
│  Builtin      │  ──►  BuiltinNodesRenderer<W>
│  Node Render  │       • render_paragraph() → "<p>...</p>"
│  (each kind)  │       • render_heading() → "<h1>...</h1>"
│               │       • render_link() → "<a href=...>...</a>"
│  24+ kinds    │       • render_image() → "<img ...>"
│               │       • render_code_block() → "<pre><code>...</code></pre>"
│               │       • render_table() → "<table><thead>...</thead>..."
│               │       • render_strikethrough() → "<del>...</del>"
│               │       • render_diagram() → "<pre class=\"mermaid\">...</pre>"
└──────────────┘
    │
    ▼
┌──────────────┐
│  Post-Render  │  ──►  DiagramPostRenderHook (injects Mermaid.js ESM script)
│  Hook         │       • Only runs if diagrams were rendered
└──────────────┘
    │
    ▼
W: TextWrite (String by default)
```

### 3.3. Key Design Patterns

1. **Arena-based allocation** — All AST nodes live in an `Arena` (vector of `Option<Node>`), accessed by `NodeRef` (cell + id). Nodes are never freed individually; the arena is dropped after rendering.

2. **Source-indexed strings** — Text content is stored as `text::Index` (byte offsets into source) or `text::Value::String`. This avoids copying and enables fast access via `index.str(source)`.

3. **Trait-based extension** — Parsers and renderers are plugged in via `ParserExtension` / `RendererExtension` traits, enabling custom AST kinds, parsers, and renderers without modifying core code.

4. **Priority-based parser dispatch** — Block parsers are indexed by first byte (0–255) and priority. Inline parsers similarly indexed. This enables O(1) lookup for common triggers.

5. **NodeKindRegistry** — Dynamic registration of custom node kinds via `NodeKindRegistry::register<T>()`, returning a `NodeKindId` used for runtime type checking and downcasting.

6. **Context key-value store** — `Context` holds type-safe KV pairs (`ContextKey<T>`) for passing data between parser phases, hooks, and renderers (e.g., tight-list detection, custom ID generation, diagram presence tracking).

### 3.4. AST Node Kinds (25 total: 23 built-in + 2 extension)

| Kind | Type | Key Fields |
|------|------|------------|
| Document | block | `meta: Metadata` (YAML frontmatter) |
| Paragraph | block | source lines |
| Heading | block | `level: u8`, `heading_kind` (Atx/Setext) |
| ThematicBreak | block | — |
| CodeBlock | block | `info: Option<Value>`, `value: Lines`, `code_block_kind` |
| Blockquote | block | source lines |
| List | block | `marker: u8`, `is_tight: bool`, `start: u32`, `list_kind`, `marker_width` |
| ListItem | block | `offset: usize`, `task: Option<Task>` |
| HtmlBlock | block | `html_block_kind`, `value: Lines` |
| Text | inline | `value: Value`, `qualifiers: TextQualifier` |
| CodeSpan | inline | `value: CodeSpanValue` |
| Emphasis | inline | — |
| Strong | inline | — |
| Link | inline | `destination: Value`, `title: Option<MultilineValue>`, `link_kind`, `link_reference` |
| Image | inline | `destination: Value`, `title: Option<MultilineValue>`, `image_kind` |
| RawHtml | inline | `value: MultilineValue`, `raw_html_kind` |
| LinkReferenceDefinition | block | `label`, `destination`, `title` |
| Table | block | — |
| TableHeader | block | — |
| TableBody | block | — |
| TableRow | block | — |
| TableCell | block | `alignment: TableCellAlignment` |
| Strikethrough | inline | — |
| Diagram | block | `diagram_type: DiagramType`, `value: Lines` |
| Extension | any | `Box<dyn ExtensionData>` |

### 3.5. Parser Options

| Option | Default | Description |
|--------|---------|-------------|
| `attributes` | false | Parse node attributes |
| `auto_heading_ids` | false | Auto-generate heading IDs |
| `without_default_parsers` | false | Disable default parsers |
| `arena` | `ArenaOptions` | Arena allocation settings (`initial_size: 1024`) |
| `escaped_space` | false | Treat `\` as space escape |
| `id_generator` | None | Custom node ID generator (`GenerateNodeId`) |

### 3.6. GFM Parser Options

| Option | Default | Description |
|--------|---------|-------------|
| `linkify` | `LinkifyOptions` | GFM autolink configuration |
| `linkify.allowed_protocols` | `["http","https","ftp","mailto"]` | Allowed URL protocols |
| `linkify.url_scanner` | default | URL detection function |
| `linkify.www_scanner` | default | www-prefixed URL detection |
| `linkify.email_scanner` | default | Email detection |

### 3.7. HTML Renderer Options

| Option | Default | Description |
|--------|---------|-------------|
| `hard_wraps` | false | Soft line breaks → `<br>` |
| `xhtml` | false | XHTML style (`<br />`) |
| `allows_unsafe` | false | Allow raw HTML / dangerous URLs |
| `escaped_space` | false | Don't render backslash-escaped space |
| `attribute_filters` | `Option<Rc<AttributeFilters>>` | Filters for rendering node attributes (per-kind `AsciiWordSet`) |

### 3.8. Parser Priority Constants

| Constant | Value | Parser |
|----------|-------|--------|
| `PRIORITY_SETTEXT_HEADING` | 100 | SetextHeadingParser |
| `PRIORITY_THEMATIC_BREAK` | 200 | ThematicBreakParser |
| `PRIORITY_LIST` | 300 | ListParser |
| `PRIORITY_LIST_ITEM` | 400 | ListItemParser |
| `PRIORITY_INDENTED_CODE_BLOCK` | 500 | IndentedCodeBlockParser |
| `PRIORITY_ATX_HEADING` | 600 | AtxHeadingParser |
| `PRIORITY_FENCED_CODE_BLOCK` | 700 | FencedCodeBlockParser |
| `PRIORITY_BLOCKQUOTE` | 800 | BlockquoteParser |
| `PRIORITY_HTML_BLOCK` | 900 | HtmlBlockParser |
| `PRIORITY_PARAGRAPH` | 1000 | ParagraphParser |
| `PRIORITY_CODE_SPAN` | 100 | CodeSpanParser |
| `PRIORITY_LINK` | 200 | LinkParser |
| `PRIORITY_AUTO_LINK` | 300 | AutoLinkParser |
| `PRIORITY_RAW_HTML` | 400 | RawHtmlParser |
| `PRIORITY_EMPHASIS` | 500 | EmphasisParser |

### 3.9. GFM Extension Functions

| Function | Description |
|----------|-------------|
| `gfm(options: GfmOptions)` | Full GFM (tables + linkify + strikethrough + task lists) |
| `gfm_table()` | GFM tables only |
| `gfm_linkify(options: LinkifyOptions)` | GFM autolink only |
| `gfm_strikethrough()` | GFM strikethrough only |
| `gfm_task_list_item()` | GFM task list items only |

### 3.10. Parser Struct

| Method | Description |
|--------|-------------|
| `Parser::with_options(opts)` | Create parser with options |
| `Parser::with_extensions(opts, ext)` | Create parser with extension |
| `add_block_parser<T, O, R, F>(f, copt, priority)` | Register a block parser |
| `add_inline_parser<T, O, R, F>(f, copt, priority)` | Register an inline parser |
| `add_ast_transformer<T, O, R, F>(f, copt, priority)` | Register an AST transformer |
| `add_paragraph_transformer<T, O, R, F>(f, copt, priority)` | Register a paragraph transformer |
| `parse(reader)` | Parse → `(Arena, NodeRef)` |

---

## 4. Python Bindings Architecture

### 4.1. Module Structure

```
mordant-py/src/
├── lib.rs          # PyO3 module entry, core API, GIL detach logic
├── document.rs     # Document wrapper (Arena + source + root_ref)
├── node.rs         # Node wrapper, kind-specific properties (incl. emoji/diagram props)
├── walker.rs       # DFS/BFS AST walker
├── options.rs      # ParseOptions, RenderOptions, GfmOptions, ArenaOptions
├── errors.rs       # RushdownError Python exception
├── meta.rs         # YAML frontmatter parser extension
├── emoji.rs        # Emoji shortcode inline parser + HTML renderer + unit tests
└── diagram.rs      # Mermaid diagram AST transformer + HTML renderer + post-render hook
```

### 4.2. Module Registration

The `mordant` module (via `#[pymodule]`) registers:

| Class | Source |
|-------|--------|
| `ParseOptions` | `options.rs` |
| `RenderOptions` | `options.rs` |
| `GfmOptions` | `options.rs` |
| `ArenaOptions` | `options.rs` |
| `PyEmojiParserOptions` | `emoji.rs` |
| `PyEmojiHtmlRendererOptions` | `emoji.rs` |
| `PyDiagramParserOptions` | `diagram.rs` |
| `PyDiagramHtmlRendererOptions` | `diagram.rs` |
| `Document` | `document.rs` |
| `Node` | `node.rs` |
| `Walker` | `walker.rs` |

| Function | Source |
|----------|--------|
| `markdown_to_html(source, gfm, parse_opts, render_opts, emoji_parse_opts, emoji_render_opts, diagram_parse_opts, diagram_render_opts)` | `lib.rs` |
| `parse(source, gfm, parse_opts, emoji_opts, diagram_opts)` | `lib.rs` |

### 4.3. GIL Management

Parse and render operations release the GIL via `Python::detach()`:

```rust
// In lib.rs — markdown_to_html()
py.detach(move || {
    parse_and_render(source, gfm, &parse_cfg, &render_cfg)
}).map_err(|e| pyo3::exceptions::PyValueError::new_err(e))

// In lib.rs — parse()
let (arena, root_ref) = py.detach(move || {
    parse_only(source, gfm, &parse_cfg)
});
```

This enables true parallelism across threads: mordant scales ~4.0x linearly with thread count, while pure-Python parsers show ~1.1x (GIL-bound).

### 4.4. Internal Build Functions (lib.rs)

| Function | Description |
|----------|-------------|
| `build_parser(gfm, parse_cfg)` | Constructs `rushdown::parser::Parser` with options + meta + emoji + diagram + GFM extensions |
| `build_renderer(render_cfg)` | Constructs `rushdown::renderer::html::Renderer` with render options + emoji + diagram extensions |
| `parse_and_render(source, gfm, parse_cfg, render_cfg)` | Parse + render to HTML string (runs without GIL) |
| `parse_only(source, gfm, parse_cfg)` | Parse only, returns `(Arena, NodeRef)` (runs without GIL) |

### 4.5. Memory Model

```
Document (Python object)
├── arena: Rc<RefCell<Arena>>    # Shared arena via Rc for Node/Walker sharing
├── source: String               # Owned source string (keeps source-indexed text valid)
└── root_ref: NodeRef            # Root of AST tree

Node (Python object)
├── arena: Rc<RefCell<Arena>>    # Shared reference to same arena
├── node_ref: NodeRef            # Pointer into arena
└── source: String               # Shared source string

Walker (Python object)
├── arena: Rc<RefCell<Arena>>    # Shared reference to same arena
├── source: String               # Shared source string
├── mode: String                 # "depth" or "breadth"
├── stack: Vec<NodeRef>          # DFS stack
└── queue: Vec<NodeRef>          # BFS queue
```

When `Document` is garbage-collected, the `Rc` reference count drops to 0, freeing the Arena and all AST nodes.

### 4.6. Plain-Rust Config Structs

Options are converted to plain-Rust structs before GIL detach:

```rust
#[derive(Clone)]
struct ParseConfig {
    attributes: bool,
    auto_heading_ids: bool,
    escaped_space: bool,
    meta_table: bool,
    emoji_options: EmojiParserOptions,
    diagram_options: DiagramParserOptions,
}

#[derive(Clone)]
struct RenderConfig {
    hard_wraps: bool,
    xhtml: bool,
    allows_unsafe: bool,
    escaped_space: bool,
    emoji_options: EmojiHtmlRendererOptions,
    diagram_options: DiagramHtmlRendererOptions,
}
```

These are `Send` and have no Python references, so they are safe to use inside `py.detach()`.

---

## 5. Python Binding Classes

### 5.1. ParseOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `smart` | bool | false | Smart parsing mode |
| `attributes` | bool | false | Parse node attributes |
| `auto_heading_ids` | bool | false | Auto-generate heading IDs |
| `escaped_space` | bool | false | Treat `\` as space escape |
| `meta_table` | bool | false | Enable YAML frontmatter (`meta.rs`) |

### 5.2. RenderOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `hard_wraps` | bool | false | Soft line breaks → `<br>` |
| `xhtml` | bool | false | XHTML style (`<br />`) |
| `allows_unsafe` | bool | false | Allow raw HTML / dangerous URLs |
| `escaped_space` | bool | false | Don't render backslash-escaped space |

### 5.3. GfmOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `tables` | bool | true | Enable GFM tables |
| `strikethrough` | bool | true | Enable GFM strikethrough |
| `task_lists` | bool | true | Enable GFM task list items |
| `linkify` | bool | true | Enable GFM autolink |

### 5.4. ArenaOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `initial_size` | usize | 1024 | Initial arena capacity |

### 5.5. Document

| Attribute/Method | Return Type | Description |
|------------------|-------------|-------------|
| `source` | str | Raw markdown source string |
| `kind` | str | Always `"Document"` |
| `type` | str | Always `"block"` |
| `children` | list[Node] | Direct child nodes |
| `metadata` | dict | YAML frontmatter as dict (raises `ValueError` on parse error) |
| `text` | str | Concatenated text of all children |
| `walk(mode)` | Walker | Create DFS/BFS walker (`"depth"` or `"breadth"`) |
| `__repr__()` | str | `"<Document source_len=N>"` |

### 5.6. Node

| Attribute/Method | Return Type | Description |
|------------------|-------------|-------------|
| `kind` | str | Node kind name (e.g. `"Heading"`, `"Paragraph"`, `"Text"`, `"Diagram"`) |
| `type` | str | `"block"` or `"inline"` |
| `parent` | Node\|None | Parent node, or None for document root |
| `children` | list[Node] | Child nodes |
| `next_sibling` | Node\|None | Next sibling, or None |
| `previous_sibling` | Node\|None | Previous sibling, or None |
| `has_children` | bool | True if node has children |
| `text` | str | Resolved text content (recursive) |
| `attributes` | dict | HTML attributes as dict |
| `level` | int\|None | Heading level (1-6) for Heading nodes |
| `destination` | str\|None | Link/image destination URL |
| `title` | str\|None | Link/image title |
| `language` | str\|None | Code block language |
| `code` | str | Code block content |
| `alignment` | str\|None | Table cell alignment (`"left"`, `"center"`, `"right"`, `"none"`) |
| `is_tight` | bool\|None | List tightness (no blank lines between items) |
| `start` | int\|None | Ordered list starting number (0 for unordered) |
| `marker` | str\|None | List marker character (`'-'`, `'+'`, `'.'`, `')'`) |
| `is_task` | bool\|None | Whether list item is a task list item |
| `task_status` | str\|None | Task status (`"active"` or `"completed"`) |
| `line` | int\|None | Source line number (0-indexed) |
| `emoji` | str\|None | Unicode emoji character for emoji nodes |
| `shortcode` | str\|None | Shortcode name for emoji nodes (e.g. `"joy"`) |
| `name` | str\|None | Full name for emoji nodes (e.g. `"grinning face with smiling eyes"`) |
| `diagram_type` | str\|None | Diagram type for diagram nodes (e.g. `"mermaid"`) |
| `diagram_value` | str | Diagram source content for diagram nodes |
| `__repr__()` | str | `"<Node kind=N ref=R>"` |

### 5.7. Walker

| Method | Return Type | Description |
|--------|-------------|-------------|
| `__iter__()` | Walker | Returns self (iterator protocol) |
| `__next__()` | Node\|None | Next node in traversal order |

### 5.8. PyEmojiParserOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `blacklist` | str\|None | None | Comma-separated shortcodes to ignore (e.g. `"joy,heart"`) |

```python
opts = mordant.PyEmojiParserOptions(blacklist="joy,heart")
html = mordant.markdown_to_html(":joy: :heart:", emoji_parse_opts=opts)
# ':joy:' passes through as-is (blacklisted)
# :heart: renders as ❤️
```

### 5.9. PyEmojiHtmlRendererOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `template` | str\|None | None | Custom template: `{emoji}`, `{shortcode}`, `{name}` |

```python
opts = mordant.PyEmojiHtmlRendererOptions(template='<img src="https://cdn.example.com/{shortcode}.png" />')
html = mordant.markdown_to_html(":joy:", emoji_render_opts=opts)
# '<img src="https://cdn.example.com/joy.png" />'
```

### 5.10. PyDiagramParserOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mermaid_enabled` | bool | true | Enable/disable Mermaid diagram transformation |

```python
opts = mordant.PyDiagramParserOptions(mermaid_enabled=False)
html = mordant.markdown_to_html("```mermaid\ngraph LR\nA --- B\n```", diagram_parse_opts=opts)
# Renders as regular <pre><code>...</code></pre> (not a diagram)
```

### 5.11. PyDiagramHtmlRendererOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mermaid_url` | str\|None | `https://cdn.jsdelivr.net/npm/mermaid@latest/dist/mermaid.esm.min.mjs` | URL to Mermaid.js ESM module |

```python
opts = mordant.PyDiagramHtmlRendererOptions(mermaid_url="https://cdn.example.com/mermaid.mjs")
html = mordant.markdown_to_html("```mermaid\ngraph LR\nA --- B\n```", diagram_render_opts=opts)
# Script tag uses custom URL
```

### 5.12. RushdownError

| Attribute/Method | Return Type | Description |
|------------------|-------------|-------------|
| `message` | str | Error message |
| `__str__()` | str | Same as message |

---

## 6. YAML Frontmatter (meta.rs)

### 6.1. Parser Design

The meta parser is a rushdown `BlockParser` extension with priority `PRIORITY_SETTEXT_HEADING - 100`:

```
Trigger: first byte `-`
Priority: ~0 (before setext heading at priority ~100)
```

**Critical: Thematic break conflict resolution**

The meta parser uses lookahead in `open()` to distinguish `---` (thematic break) from `---\nkey: value` (frontmatter):

| Input | Result |
|-------|--------|
| `---` alone | Thematic break (not consumed) |
| `-----` (5 dashes) | Thematic break |
| `---\n` + empty/whitespace | Thematic break |
| `---\n` + plain text (no colon) | Thematic break + setext heading |
| `---\n` + YAML-like content | Frontmatter consumed |

YAML-like content is detected by checking if the first line after `---` contains a colon, starts with `- ` (list), or starts with `|` / `>` (block scalar). Also checks for `---` and `...` document markers.

### 6.2. YAML Parsing

Uses `yaml-peg` v1.0.9 (PEG-based YAML subset):

| YAML Type | Rust Meta Type | Python Type |
|-----------|----------------|-------------|
| `null` | `Meta::Null` | `None` |
| `true`/`false` | `Meta::Bool` | `bool` |
| `42` | `Meta::Int` | `int` |
| `3.14` | `Meta::Float` | `float` |
| `"hello"` | `Meta::String` | `str` |
| `[a, b]` | `Meta::Sequence` | `list` |
| `{k: v}` | `Meta::Mapping` | `dict` |

**Limitations:** No YAML anchors/aliases support. Parse errors are inserted as HTML comments in the AST; Python raises `ValueError` on `doc.metadata` access.

### 6.3. MetaParserOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `table` | bool | false | Render metadata as HTML table in AST |

### 6.4. Meta Parser Architecture

```
MetaParser (BlockParser)
├── trigger() → b"-"
├── open() → Option<(NodeRef, State)>
│   └── Validates frontmatter, creates CodeBlock node, stores in Context
├── cont() → Option<State>
│   └── Accumulates source lines until closing ---
└── close() → ()

MetaAstTransformer (AstTransformer)
├── transform()
│   └── Extracts YAML from CodeBlock, parses with yaml-peg
│   └── Inserts metadata into Document node
│   └── Optionally renders as HTML table node
└── render_meta_as_table()
    └── Creates Table → TableHeader → TableRow → TableCell tree
```

### 6.5. Meta Parser Tests (41 tests in `test_meta.py`)

- Simple frontmatter, no frontmatter, thematic break not consumed
- Five dashes not consumed, nested mapping, sequence
- All scalar types, empty frontmatter, dash in string
- Thematic break with blank line, multiple keys
- Original rushdown-meta test cases, table option

---

## 7. Extension System

### 7.1. Custom Block Parser

```rust
pub trait BlockParser {
    fn open(&self, arena: &mut Arena, parent: NodeRef, reader: &mut BasicReader, ctx: &mut Context)
        -> Option<(NodeRef, State)>;
    fn cont(&self, arena: &mut Arena, node: NodeRef, reader: &mut BasicReader, ctx: &mut Context)
        -> Option<State>;
    fn close(&self, arena: &mut Arena, node: NodeRef, reader: &mut BasicReader, ctx: &mut Context);
    fn trigger(&self) -> &[u8];
    fn can_accept_indented_line(&self) -> bool;
    fn can_interrupt_paragraph(&self) -> bool;
}
```

### 7.2. Custom Inline Parser

```rust
pub trait InlineParser {
    fn trigger(&self) -> &[u8];
    fn parse(&self, arena: &mut Arena, parent_ref: NodeRef, reader: &mut BlockReader, ctx: &mut Context)
        -> Option<NodeRef>;
    fn close_block(&self, arena: &mut Arena, node: NodeRef, reader: &mut BlockReader, ctx: &mut Context);
}
```

### 7.3. Custom AST Transformer

```rust
pub trait AstTransformer {
    fn transform(&self, arena: &mut Arena, doc_ref: NodeRef, reader: &mut BasicReader, ctx: &mut Context);
}
```

### 7.4. Custom Paragraph Transformer

```rust
pub trait ParagraphTransformer {
    fn transform(&self, arena: &mut Arena, paragraph_ref: NodeRef, ctx: &mut Context);
}
```

### 7.5. Custom Node Renderer

```rust
pub trait NodeRenderer<'r, W: TextWrite> {
    fn register_node_renderer_fn(self, nrr: &mut impl NodeRendererRegistry<'r, W>);
}
```

### 7.6. Extension Wrappers

| Type | Wraps |
|------|-------|
| `AnyBlockParser` | `Box<dyn BlockParser>` |
| `AnyInlineParser` | `Box<dyn InlineParser>` |
| `AnyAstTransformer` | `Box<dyn AstTransformer>` |
| `AnyParagraphTransformer` | `Box<dyn ParagraphTransformer>` |

### 7.7. Parser Extension System

```rust
pub trait ParserExtension {
    fn apply(self, parser: &mut Parser);
    fn and<R>(self, other: R) -> ChainedParserExtension<Self, R>;
}

pub struct ParserExtensionFn<T: FnOnce(&mut Parser)>;
pub struct ChainedParserExtension<T, U>;
```

### 7.8. Renderer Extension System

```rust
pub trait RendererExtension<'r, W: TextWrite> {
    fn apply(self, renderer: &mut Renderer<'r, W>);
    fn and<R>(self, other: R) -> ChainedRendererExtension<Self, R>;
}

pub struct EmptyRendererExtension;
pub const NO_EXTENSIONS: EmptyRendererExtension;
pub struct ChainedRendererExtension<T, U>;
pub struct RendererExtensionFn<T>;
pub const fn renderer_extension<'r, W, T>(f: T) -> RendererExtensionFn<T>
    where T: FnOnce(&mut Renderer<'r, W>)
```

### 7.9. Extension Factory Helpers

```rust
pub fn parser_extension<T: FnOnce(&mut Parser)>(f: T) -> ParserExtensionFn<T>
pub fn gfm(options: GfmOptions) -> ...          // full GFM
pub fn gfm_table() -> ...                       // GFM tables only
pub fn gfm_strikethrough() -> ...               // GFM strikethrough only
pub fn gfm_task_list_item() -> ...              // GFM task lists only
pub fn gfm_linkify(options: LinkifyOptions) -> ...  // GFM autolink
pub fn paragraph_renderer(opts: ParagraphRendererOptions) -> impl RendererExtension<'r, W>
```

---

## 7.10. Emoji Extension (rushdown-emoji)

The emoji extension provides shortcode-based emoji rendering (`:joy:`, `:heart:`, etc.) via an inline parser and HTML renderer.

### 7.10.1. EmojiParserOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `blacklist` | Vec\<String\> | `[]` | Shortcodes to skip during parsing |

### 7.10.2. EmojiHtmlRendererOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `template` | Option\<String\> | `None` | Custom render template: `{emoji}`, `{shortcode}`, `{name}` |

### 7.10.3. EmojiInlineParser

The emoji inline parser is a rushdown `InlineParser` that triggers on `:` and parses emoji shortcodes:

```rust
struct EmojiInlineParser {
    options: EmojiParserOptions,
}

impl EmojiInlineParser {
    fn trigger(&self) -> &[u8]      // ":"
    fn parse(&self, ...) -> Option<NodeRef>
    fn close_block(&self, ...) -> ()
    fn is_blacklisted(&self, shortcode: &str) -> bool
}
```

**Parsing flow:**
1. Trigger on `:` character
2. Look ahead for `shortcode:` pattern
3. Check blacklist — if blacklisted, pass through as-is
4. Look up shortcode in `emojis` crate database (v0.8.0)
5. If found, create `Extension` node with `EmojiData`
6. If not found, pass through as literal `:shortcode:`

### 7.10.4. EmojiData (Extension Node)

Emojis are stored as `Extension` AST nodes containing `EmojiData`:

| Field | Type | Description |
|-------|------|-------------|
| `emoji` | String | Unicode emoji character (e.g. `"😀"`) |
| `shortcode` | String | Shortcode name (e.g. `"joy"`) |
| `name` | String | Full name (e.g. `"grinning face with smiling eyes"`) |

### 7.10.5. EmojiHtmlRenderer

The emoji HTML renderer converts `EmojiData` nodes to HTML:

| Template | Output |
|----------|--------|
| `None` (default) | Unicode character: `<p>😀</p>` |
| `"<img src=\"{shortcode}.png\">" ` | `<img src="joy.png">` |
| `"{name} emoji"` | `grinning face with smiling eyes emoji` |

### 7.10.6. Emoji Extension Registration

```rust
// In lib.rs — build_parser()
let emoji_ext = emoji_parser_extension(parse_cfg.emoji_options.clone());
let parser_ext = meta_ext.and(emoji_ext);

// In lib.rs — build_renderer()
let emoji_ext = emoji_html_renderer_extension(render_cfg.emoji_options.clone());
```

### 7.10.7. Emoji Extension Tests

**Rust unit tests** (in `mordant-py/src/emoji.rs`):
- `test_emoji_basic` — Basic emoji rendering
- `test_emoji_not_exists` — Invalid shortcode passes through
- `test_emoji_blacklist` — Blacklist prevents parsing
- `test_emoji_render_unicode` — Unicode rendering
- `test_emoji_render_template` — Custom HTML template
- `test_emoji_render_template_name` — Template with {name}
- `test_emoji_inside_code_span` — Emojis in code spans not parsed
- `test_emoji_multiple` — Multiple emojis
- `test_emoji_emoji_data` — Emoji node data access

**Python tests** (in `mordant-py/tests/test_emoji.py`):
- Basic rendering, multiple emojis, paragraph text
- Code span/block protection
- Invalid shortcodes, empty shortcodes
- Blacklist (single, multiple, empty, whitespace)
- Custom HTML templates (default, custom, name, emoji, unknown)
- AST node access (emoji, shortcode, name properties)
- Integration with frontmatter, GFM, attributes, auto heading IDs
- Edge cases (empty string, no colon, partial colon, reverse colon, mixed)

---

## 7.11. Diagram Extension (rushdown-diagram)

The diagram extension provides Mermaid diagram rendering from fenced code blocks via an AST transformer and HTML renderer.

### 7.11.1. DiagramParserOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mermaid` | `MermaidParserOptions` | `enabled: true` | Mermaid diagram parsing configuration |

### 7.11.2. DiagramHtmlRendererOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mermaid` | `MermaidHtmlRenderingOptions` | `Client(...)` | Mermaid rendering configuration |

### 7.11.3. DiagramAstTransformer

The diagram extension uses an AST transformer (priority 100) that runs after block/inline parsing to convert Mermaid code blocks into `Diagram` nodes:

```rust
struct DiagramAstTransformer {
    options: DiagramParserOptions,
}

impl AstTransformer for DiagramAstTransformer {
    fn transform(&self, arena: &mut Arena, doc_ref: NodeRef, reader: &mut BasicReader, ctx: &mut Context) {
        // 1. Recursively walk AST to find CodeBlock nodes with language == "mermaid"
        // 2. For each match, create a Diagram node with the code block's value
        // 3. Replace the CodeBlock in its parent with the Diagram node
    }
}
```

**Transformation flow:**
1. `collect_mermaid_blocks()` recursively walks the AST
2. Matches `CodeBlock` nodes where `language_str(source) == "mermaid"`
3. Creates `Diagram` node with `DiagramType::Mermaid`
4. Copies the code block's `Lines` value to the diagram node
5. Replaces the `CodeBlock` in its parent with the `Diagram` node

### 7.11.4. Diagram Node

Diagrams are stored as `Extension` AST nodes containing `Diagram`:

| Field | Type | Description |
|-------|------|-------------|
| `diagram_type` | `DiagramType` | `DiagramType::Mermaid` |
| `value` | `Lines` | Diagram source content |

### 7.11.5. DiagramHtmlRenderer

The diagram HTML renderer converts `Diagram` nodes to HTML:

```html
<pre class="mermaid">
graph LR
    A --- B
</pre>
```

### 7.11.6. DiagramPostRenderHook

After all rendering completes, the post-render hook checks if any diagrams were rendered. If so, it injects a single Mermaid.js ESM script tag:

```html
<script type="module">
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@latest/dist/mermaid.esm.min.mjs';
</script>
```

The hook uses a `ContextKey<BoolValue>` to track whether diagrams were rendered, ensuring only one script tag is injected regardless of how many diagrams are in the document.

### 7.11.7. Diagram Extension Registration

```rust
// In lib.rs — build_parser()
let diagram_ext = diagram_parser_extension(parse_cfg.diagram_options.clone());
let parser_ext = meta_ext.and(emoji_ext).and(diagram_ext);

// In lib.rs — build_renderer()
let diagram_ext = diagram_html_renderer_extension(render_cfg.diagram_options.clone());
rushdown_lib::renderer::html::Renderer::with_extensions(opts, emoji_ext.and(diagram_ext))
```

### 7.11.8. Diagram Extension Tests (17 tests in `test_diagram.py`)

- Basic rendering: `<pre class="mermaid">`, script injection, content preservation
- Options: `mermaid_enabled=False`, custom `mermaid_url`, default URL
- AST access: `Diagram` node kind, `diagram_type`, `diagram_value`
- Multiple diagrams: multiple `<pre>` blocks, single script tag
- Mixed content: diagrams with headings, paragraphs, lists
- Edge cases: empty block, special HTML chars, GFM mode, frontmatter integration

---

## 8. Error Handling

### 8.1. Rust Error Types

| Error | Description |
|-------|-------------|
| `Error::InvalidNodeRef` | Invalid node reference |
| `Error::InvalidNodeOperation` | Invalid node operation |
| `Error::Io` | I/O error with optional source |
| `CallbackError<E>` | Wraps `Internal(Error)` or `Callback(E)` |

### 8.2. Python Exception Mapping

| Rust Error | Python Exception |
|------------|------------------|
| `rushdown::Error::InvalidNodeRef` | `ValueError` |
| `rushdown::Error::InvalidNodeOperation` | `ValueError` |
| `rushdown::Error::Io` | `ValueError` |
| YAML parse error (in AST) | `ValueError` on `doc.metadata` access |

### 8.3. RushdownError Class

```python
class RushdownError(Exception):
    """Base exception for all rushdown errors."""
    def __init__(self, message: str)
    @property
    def message(self) -> str
    def __str__(self) -> str
```

### 8.4. Error Conversion Helper

```rust
pub fn rushdown_err_to_pyerr(err: rushdown_lib::Error) -> PyErr
```

---

## 9. Context System

### 9.1. Context Struct

```rust
pub struct Context {
    // Internal vector of AnyValue
}

impl Context {
    fn new() -> Self
    fn initialize(registry: &ContextKeyRegistry)
    fn insert<T: AnyValueSpec>(key: ContextKey<T>, value: T::Item)
    fn get<T: AnyValueSpec>(key: ContextKey<T>) -> Option<&T::Item>
    fn get_mut<T: AnyValueSpec>(key: ContextKey<T>) -> Option<&mut T::Item>
    fn remove<T: AnyValueSpec>(key: ContextKey<T>) -> Option<T::Item>
}
```

### 9.2. Context Key Types

| Type | Purpose |
|------|---------|
| `UsizeValue` | Store usize values |
| `StringValue` | Store String values |
| `ObjectValue` | Store boxed trait objects |
| `NodeRefValue` | Store NodeRef values (used by meta parser) |
| `IntegerValue` | Store integer values |
| `NumberValue` | Store number values |
| `BoolValue` | Store boolean values (used by diagram to track diagram presence) |

### 9.3. ContextKey Registry

```rust
pub struct ContextKeyRegistry {
    // Internal registry for unique key IDs
}

impl ContextKeyRegistry {
    fn create<T: AnyValueSpec>() -> ContextKey<T>
    fn get_or_create<T: AnyValueSpec>(key: &str) -> ContextKey<T>
}
```

---

## 10. Build & Distribution

### 10.1. Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| `rushdown` | 0.18.0 (path dep) | Core parser/renderer |
| `pyo3` | 0.29 | Python bindings |
| `yaml-peg` | 1.0.9 | YAML frontmatter parsing |
| `emojis` | 0.8.0 | Emoji shortcode database (1,500+ emojis) |

### 10.2. Build Commands

```bash
# Build Python extension
cd mordant-py
cargo build --release

# Run tests
python -m pytest tests/ -v

# Run benchmarks
cd benchmarks
python benchmarks.py              # All fixtures, 50 iterations
python benchmarks.py -f medium -n 100  # Specific fixture, custom count
python benchmarks.py -o results.json  # Save JSON
```

### 10.3. Benchmarks (single-threaded, 50 iterations)

| Fixture | mordant | mistune | markdown-it-py | python-markdown |
|---------|---------|---------|----------------|-----------------|
| Small (400B) | **0.039ms** | 0.430ms | 0.475ms | 2.301ms |
| Medium (5.4KB) | **0.155ms** | 2.448ms | 3.940ms | 6.455ms |
| Large (26.7KB) | **0.410ms** | 8.611ms | 16.743ms | 31.304ms |
| Data (202KB) | **2.763ms** | 38.152ms | 65.736ms | 621.295ms |

### 10.4. Multi-threaded Scaling (4 threads, medium fixture)

| Library | 1-thread | 4-threads | Scaling |
|---------|----------|-----------|---------|
| **mordant** | ~1,000 docs/s | ~4,000 docs/s | **4.0x** |
| python-markdown | ~59 docs/s | ~257 docs/s | 4.35x |
| mistune | ~133 docs/s | ~542 docs/s | 4.07x |
| markdown-it-py | ~83 docs/s | ~337 docs/s | 4.06x |

---

## 11. Comparison with Existing Libraries

| Library | AST Access | GFM | Speed | Extensibility |
|---------|------------|-----|-------|---------------|
| **mordant** | Full AST | ✅ | ⭐⭐⭐⭐⭐ | ✅ (Rust extensions) |
| python-markdown | Token list | Partial | ⭐⭐ | ✅ (extensions) |
| mistune | AST | ✅ | ⭐⭐⭐⭐ | Partial |
| markdown-it-py | AST | ✅ | ⭐⭐⭐⭐⭐ | ✅ |
| CommonMark (pure) | AST | ❌ | ⭐⭐ | ✅ |

---

## 12. File Reference

| File | Lines | Purpose |
|------|-------|---------|
| **Rust Core** | | |
| `src/lib.rs` | 594 | Public API entry points |
| `src/ast.rs` | 3,281 | AST types: Node, NodeRef, Arena, KindData (25 variants) |
| `src/parser/mod.rs` | 2,660 | Parser struct, options, extensions, GFM |
| `src/parser/attribute.rs` | ~100 | Attribute parser |
| `src/parser/paragraph.rs` | 87 | Paragraph parser |
| `src/parser/blockquote.rs` | ~100 | Blockquote parser |
| `src/parser/code_block.rs` | ~200 | Fenced + indented code block parsers |
| `src/parser/heading.rs` | 369 | ATX + Setext heading parsers |
| `src/parser/thematic_break.rs` | 82 | Thematic break parser |
| `src/parser/list.rs` | 451 | List + list item parsers |
| `src/parser/html_block.rs` | 124 | HTML block parser |
| `src/parser/table.rs` | 369 | Table parser + AST transformer |
| `src/parser/linkify.rs` | 217 | GFM autolink parser |
| `src/parser/code_span.rs` | ~100 | Code span parser |
| `src/parser/raw_html.rs` | 45 | Raw HTML parser |
| `src/parser/delimiter.rs` | 189 | Delimiter processing for emphasis |
| `src/parser/emphasis.rs` | 56 | Emphasis parser |
| `src/parser/link.rs` | 438 | Link parser |
| `src/parser/link_ref.rs` | 190 | Link reference resolution |
| `src/parser/auto_link.rs` | ~100 | Auto-link parser |
| `src/parser/strikethrough.rs` | 62 | Strikethrough parser |
| `src/parser/task_list_item.rs` | 61 | Task list item paragraph transformer |
| `src/parser/*.rs` | ~5,500 | Individual block/inline parsers (9 block + 6 inline + helpers) |
| `src/renderer/mod.rs` | 1,453 | Renderer base, NodeKindRegistry, hooks, Context |
| `src/renderer/html.rs` | 1,464 | HTML renderer, BuiltinNodesRenderer (24+ render methods), Writer, SafeStr |
| `src/text.rs` | 1,707 | Index, Value, MultilineValue, Lines, Segment, Reader, BasicReader, BlockReader |
| `src/context.rs` | 606 | Context key-value store, key types |
| `src/scanner/mod.rs` | 603 | re2c-generated HTML/URL scanners |
| `src/scanner/scanner_gen.rs` | 8,996 | re2c-generated scanner code |
| `src/util.rs` | 2,205 | StringMap, TinyVec, escape functions, AsciiWordSet, Prioritized |
| `src/error.rs` | 200 | Error types, CallbackError |
| `build.rs` | 217 | Build-time code generation (entities, attributes, tags) |
| **Python Bindings** | | |
| `mordant-py/src/lib.rs` | ~500 | PyO3 module, core API, lint/fix/batch API, GIL detach |
| `mordant-py/src/document.rs` | 183 | Document wrapper, metadata, walk |
| `mordant-py/src/node.rs` | ~380 | Node wrapper, kind-specific properties (incl. emoji/diagram props) |
| `mordant-py/src/walker.rs` | 105 | AST walker (DFS/BFS) |
| `mordant-py/src/options.rs` | 143 | ParseOptions, RenderOptions, GfmOptions, ArenaOptions |
| `mordant-py/src/errors.rs` | 33 | Python exception types |
| `mordant-py/src/meta.rs` | 655 | YAML frontmatter parser + unit tests |
| `mordant-py/src/emoji.rs` | 572 | Emoji shortcode inline parser + HTML renderer + unit tests |
| `mordant-py/src/diagram.rs` | ~350 | Mermaid diagram AST transformer + HTML renderer + post-render hook |
| `mordant-py/src/linter.rs` | ~1,800 | Lint engine: 25 rules, diagnostics, fix engine, config, suppression, batch API |
| `mordant-py/src/fix_engine.rs` | ~200 | Fix engine: FixOp, FixResult, fixpoint checking |
| `mordant-py/mordant/__init__.py` | ~100 | Python re-exports: lint, fix, lint_many, fix_many, Diagnostic, FixResult, etc. |
| `mordant-py/mordant/__main__.py` | ~300 | CLI: argparse, formatters (human/json/github), config loading, glob expansion |
| **Tests** | | |
| `mordant-py/tests/test_core.py` | 14 | Basic CommonMark rendering |
| `mordant-py/tests/test_ast.py` | 61 | Document, Node, Walker, metadata |
| `mordant-py/tests/test_gfm.py` | 9 | GFM extensions |
| `mordant-py/tests/test_options.py` | 17 | Options propagation |
| `mordant-py/tests/test_meta.py` | 41 | YAML frontmatter + thematic break conflict |
| `mordant-py/tests/test_emoji.py` | 29 | Emoji rendering, blacklist, templates, AST access |
| `mordant-py/tests/test_diagram.py` | 17 | Mermaid diagram rendering, options, AST access |
| `mordant-py/tests/test_commonmark_spec.py` | 652 | Full CommonMark 0.31.2 spec |
| `mordant-py/tests/test_lint.py` | 133 | 25 rules + fixer + config + CLI + batch API + Phase 8 emoji/frontmatter/fragment anchors |
| `mordant-py/tests/test_lint.py` | 133 | 25 rules + fixer + config + CLI + batch API + Phase 8 emoji/frontmatter/fragment anchors |

---

## 13. Linter Module

The linter module provides a 25-rule Markdown linting engine with diagnostics, fix engine, configuration, suppression, and batch processing. It operates on the parsed AST to detect issues and produce actionable diagnostics.

### 13.1. Architecture Overview

```
Source String
    │
    ▼
┌──────────────┐
│  Rushdown     │  ──►  (Arena, NodeRef)
│  Parser       │       Parse-only (no render)
└──────────────┘
    │
    ▼
┌──────────────┐
│  AST          │  ──►  Collected struct (headings, links, code_blocks, etc.)
│  Traversal    │       Single DFS walk collecting rule-relevant data
│  (build())    │
└──────────────┘
    │
    ▼
┌──────────────┐
│  Rule         │  ──►  25 lint rules, each a function(Collected, &mut Vec<Violation>)
│  Engine       │       MD001 (heading-increment), MD009 (no-trailing-spaces),
│               │       MD012 (no-multiple-blanks), MD024 (no-duplicate-heading),
│               │       MD025 (single-h1), MD040 (fenced-code-language),
│               │       MD042 (no-empty-links), MD045 (no-alt-text),
│               │       MD047 (single-trailing-newline), MD010 (no-hard-tabs),
│               │       MD018/MD019/MD020/MD021 (atx/setext spacing),
│               │       MD030/MD031 (proper list spacing), MD032 (bq-spaces),
│               │       MD033/MD034 (no-unbalanced), MD036/MD037/MD038/MD039,
│               │       MD041 (no-repeated), MD043 (no-html), MD044 (no-dup-id),
│               │       MD046 (no-inline-html), MD048 (no-hard-tabs-alt), MD049
└──────────────┘
    │
    ▼
┌──────────────┐
│  Diagnostics  │  ──►  Diagnostic { rule, name, message, line, severity, column, span, fix }
│  Output       │       Fixable flag derived from fix field
└──────────────┘
    │
    ▼
┌──────────────┐
│  Fix Engine   │  ──►  apply_fixes(source, diagnostics) → FixResult
│               │       Applies FixOp (Insert, Delete, Replace) on source lines
│               │       Re-lints to verify fixpoint (no new violations)
└──────────────┘
```

### 13.2. Diagnostic Model

```rust
struct Diagnostic {
    rule: String,         // e.g., "MD001"
    name: String,         // e.g., "heading-increment"
    message: String,      // Human-readable description
    line: Option<usize>,  // Source line number (1-indexed)
    severity: Severity,   // Severity::Warning | Severity::Error | Severity::Info
    column: Option<usize>,// Byte offset within line
    span: Option<(usize, usize)>, // [start_byte, end_byte) in source
    fix: Option<FixOp>,   // Auto-fix operation (None if not fixable)
}
```

### 13.3. Fix Model

```rust
struct FixOp {
    line: usize,          // 1-indexed source line
    column: Option<usize>,// Byte offset within line
    replacement: String,  // Text to insert/replace
    kind: FixKind,        // Insert | Delete | Replace
}

struct FixResult {
    output: String,       // Fixed source text
    remaining: Vec<Diagnostic>, // Diagnostics that could not be auto-fixed
}
```

### 13.4. Configuration System

Rules are configured via `LintConfig` / `RuleParams`:

```rust
struct RuleParams {
    heading_style: String,           // MD003: "consistent" | "atx" | "setext"
    line_length: usize,              // MD013: max line length (default 80)
    line_length_ignore_threshold: usize,  // MD013: ignore lines below this
    spaces_per_tab: usize,           // MD010: spaces per tab (default 4)
    siblings_only: bool,             // MD024: only compare sibling headings
    default_language: Option<String>, // MD040: default language for fixes
}

struct LintConfig {
    disable: Vec<String>,            // Rule IDs to disable
    enable: Option<Vec<String>>,     // If set, only these rules run
    _enabled_when_default_false: Option<Vec<String>>,
    suppressions: Vec<SuppressionDirective>,  // Parsed from inline comments
    params: RuleParams,              // Per-rule parameters
}
```

### 13.5. Suppression System

Inline suppression comments are parsed at lint time:

```markdown
<!-- markdownlint-disable MD001 -->
# H1

### H3 jump

<!-- markdownlint-enable MD001 -->

<!-- markdownlint-disable-next-line MD009 -->
```

The `SuppressionDirective` struct captures line number, affected rules (or all if unspecified), and action (`disable`, `enable`, `disable-next-line`).

### 13.6. Batch API

Uses `rayon` for thread-parallel processing of multiple files:

```rust
fn lint_many(files: Vec<(String, String)>, cfg: &LintConfig) -> Vec<(String, Vec<Violation>)>
fn fix_many(files: Vec<(String, String)>, cfg: &LintConfig, default_language: Option<&str>) -> Vec<(String, FixOutcome)>
```

- Each file is processed independently on a separate rayon thread
- GIL is released via `py.detach()` for the entire batch operation
- Returns `[(filename, diagnostics), ...]` or `[(filename, FixOutcome), ...]`

### 13.7. Collected Data Structure

Single DFS walk collects all rule-relevant data:

```rust
struct Collected {
    headings: Vec<HeadingInfo>,   // level, line, text
    links: Vec<LinkInfo>,         // destination, line
    images: Vec<ImageInfo>,       // alt, line
    code_blocks: Vec<CodeBlockInfo>, // language, fenced, line
    code_regions: Vec<CodeRegion>,   // start, end, fenced — for line-based rules
    // Phase 8 additions:
    frontmatter_title: Option<String>,  // Extracted from YAML frontmatter
    heading_anchors: Vec<String>,       // Canonical slug anchors for fragment validation
}
```

### 13.8. Phase 8 Accuracy Polish

- **R8.1**: `collect_text()` handles `KindData::Extension` nodes by downcasting to `EmojiData` — emoji shortcode headings now correctly compared by MD024
- **R8.2**: `build()` extracts `title` from YAML frontmatter metadata — MD025 respects frontmatter title
- **R8.3**: Canonical heading anchors (lowercase, hyphenated, no special chars) generated during AST traversal — MD042 validates `#fragment` links against known anchors

### 13.9. Rule Catalog

| ID | Name | Source | Fixable | Description |
|----|------|--------|---------|-------------|
| MD001 | heading-increment | AST | no | Headings increment by 1 |
| MD003 | heading-style | AST | no | Heading style consistency |
| MD009 | no-trailing-spaces | line | yes | No trailing whitespace |
| MD010 | no-hard-tabs | line | yes | No hard tabs (spaces preferred) |
| MD012 | no-multiple-blanks | line | yes | No multiple blank lines |
| MD013 | line-length | line | no | Lines should not exceed max length |
| MD018 | atx-spacing | line | no | ATX heading space after # |
| MD019 | atx-closing-spaces | line | no | ATX leaf headings no closing # |
| MD020 | atx-spacing | line | no | ATX heading space before closing # |
| MD021 | atx-heading-space | line | no | Multiple spaces inside ATX heading |
| MD022 | heading-blank-lines | AST | yes | Headings should have blank lines around them |
| MD024 | no-duplicate-heading | AST | no | No duplicate headings |
| MD025 | single-h1 | AST | no | Single H1 per document |
| MD026 | no-trailing-punctuation | AST | yes | Headings should not end with trailing punctuation |
| MD031 | fenced-code-blocks-working | AST | yes | Fenced code blocks should have blank lines around them |
| MD032 | indented-code-block | AST | no | Indented code blocks should have blank lines around them |
| MD034 | no-bare-urls | AST | no | Bare URLs should be in angle brackets |
| MD040 | fenced-code-language | AST | yes | Fenced code blocks should specify a language |
| MD042 | no-empty-links | AST | no | Links should have a non-empty destination |
| MD045 | no-alt-text | AST | no | Images should have alternate text |
| MD046 | code-block-indentation | AST | no | Fenced code blocks should use 4-space indentation |
| MD047 | single-trailing-newline | line | yes | Files should end with a single trailing newline |
| MD048 | fenced-code-block-punctuation | AST | no | Fenced code blocks should use backticks, not tildes |
| MD049 | emphasis-style | AST | no | Emphasis style consistency |
| MD050 | strong-style | AST | no | Strong style consistency |

### 13.10. Python Bindings — Linter Classes

#### Diagnostic

| Attribute | Type | Description |
|-----------|------|-------------|
| `rule` | `str` | Rule ID (e.g., `"MD001"`) |
| `name` | `str` | Rule name (e.g., `"heading-increment"`) |
| `message` | `str` | Human-readable description |
| `line` | `int \| None` | Source line number (1-indexed) |
| `severity` | `str` | `"warning"` or `"error"` |
| `column` | `int \| None` | Byte offset within line |
| `span` | `tuple[int, int] \| None` | `[start_byte, end_byte)` in source |
| `fixable` | `bool` | True if diagnostic has an auto-fix (ReplaceLine, DeleteLine, EnsureFinalNewline) |

#### FixResult

| Attribute | Type | Description |
|-----------|------|-------------|
| `output` | `str` | Fixed source text |
| `fixed` | `list[Diagnostic]` | Diagnostics that were auto-corrected |
| `unfixable` | `list[Diagnostic]` | Diagnostics that could not be auto-fixed |
| `remaining` | `list[Diagnostic]` | Diagnostics remaining after re-linting output |

#### LintConfig

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `disable` | `list[str]` | `[]` | Rule IDs to disable |
| `enable` | `list[str] \| None` | `None` | If set, ONLY these rules run |
| `suppressions` | `list[SuppressionDirective]` | `[]` | Parsed from inline comments |
| `params` | `RuleParams` | defaults | Per-rule parameters |

#### RuleParams

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `heading_style` | `str` | `"consistent"` | MD003: heading style |
| `line_length` | `int` | `80` | MD013: max line length |
| `line_length_ignore_threshold` | `int` | `0` | MD013: ignore lines below this |
| `spaces_per_tab` | `int` | `4` | MD010: spaces per tab |
| `siblings_only` | `bool` | `False` | MD024: only compare sibling headings |
| `default_language` | `str \| None` | `None` | MD040: default language for fixes |

#### LintOptions

| Attribute | Type | Default | Description |
|-----------|------|---------|-------------|
| `disable` | `list[str]` | `[]` | Rule IDs to disable |
| `enable` | `list[str] \| None` | `None` | If set, only these rules run |

#### FixOp (internal, not exposed to Python)

| Variant | Fields | Description |
|---------|--------|-------------|
| `ReplaceLine` | `line: int, text: str` | Replace entire line content |
| `DeleteLine` | `line: int` | Delete entire line |
| `EnsureFinalNewline` | — | Ensure document ends with single newline |
| `SetCodeLanguage` | `line: int` | Insert language on fence line |

#### Severity (internal, not exposed to Python)

| Value | Description |
|-------|-------------|
| `Warning` | Warning severity |
| `Error` | Error severity |

#### SuppressionDirective (internal)

| Attribute | Type | Description |
|-----------|------|-------------|
| `line` | `int` | 0-indexed line number |
| `rules` | `list[str] \| None` | Affected rules (None = all) |
| `action` | `str` | `"disable"`, `"enable"`, `"disable-next-line"` |

### 13.11. Python API

```python
import mordant

# Lint a single document
diagnostics = mordant.lint("# Hello\n\n### Jump\n")
# [Diagnostic(rule='MD001', name='heading-increment', ...)]

# Lint with options
diagnostics = mordant.lint(
    "# Hello\n\n### Jump\n",
    parse_opts=mordant.ParseOptions(meta_table=True),
    lint_opts=mordant.LintOptions(disable=["MD040"]),
    lint_config=mordant.LintConfig(
        disable=["MD040"],
        enable=None,
        params=mordant.RuleParams(heading_style="atx", spaces_per_tab=4),
    ),
)

# Fix a single document
result = mordant.fix("# Title  \n\n\nText")
print(result.output)       # Fixed source
print(result.fixed)        # Diagnostics that were auto-corrected
print(result.unfixable)    # Diagnostics that could not be fixed
print(result.remaining)    # Diagnostics remaining after re-linting

# Fix with options
result = mordant.fix(
    "content",
    parse_opts=mordant.ParseOptions(meta_table=True),
    lint_opts=mordant.LintOptions(disable=["MD040"]),
    default_language="python",
    lint_config=mordant.LintConfig(
        disable=["MD040"],
        params=mordant.RuleParams(default_language="python"),
    ),
)

# Batch lint (parallel)
results = mordant.lint_many([
    ("file1.md", "# Hello\n"),
    ("file2.md", "# Hi\n"),
])
# [("file1.md", [Diagnostic(...)]), ("file2.md", [])]

# Batch fix (parallel)
results = mordant.fix_many([
    ("file1.md", "trailing   \n"),
    ("file2.md", "more trailing  \n"),
])
# [("file1.md", FixResult(output='trailing\n', fixed=[...], unfixable=[], remaining=[])), ...]
```

### 13.12. CLI Usage

```bash
# Basic lint
python -m mordant file1.md file2.md

# Fix in place
python -m mordant --fix file.md

# Dry run (show what would be fixed)
python -m mordant --fix --dry-run file.md

# Output format
python -m mordant --format human file.md    # Default
python -m mordant --format json file.md     # JSON array
python -m mordant --format github file.md   # GitHub Actions annotations

# Config file
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

### 13.13. Output Formats

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

## Appendix A: Macro Reference (Rust Core)

| Macro | Purpose |
|-------|---------|
| `matches_kind!(arena, ref, Kind)` | Check if node is a given kind |
| `as_kind_data!(arena, ref, Kind)` | Downcast to kind-specific data |
| `as_kind_data_mut!(arena, ref, Kind)` | Mutable downcast |
| `as_type_data!(arena, ref, Block)` | Downcast to type-specific data |
| `as_type_data_mut!(arena, ref, Block)` | Mutable type downcast |
| `matches_extension_kind!(arena, ref, T)` | Check extension kind |
| `as_extension_data!(arena, ref, T)` | Downcast extension data |
| `as_extension_data_mut!(arena, ref, T)` | Mutable extension downcast |
| `md_ast!(arena, Root => { children })` | Build AST programmatically |
| `node_path!(arena, start, method1, method2)` | Traverse node path |

---

## Appendix B: Built-in Block Parsers

| Parser | Priority | Trigger |
|--------|----------|---------|
| `HtmlBlockParser` | 900 | HTML tags |
| `BlockquoteParser` | 800 | `>` |
| `FencedCodeBlockParser` | 700 | ` ``` ` |
| `AtxHeadingParser` | 600 | `#`–`######` |
| `IndentedCodeBlockParser` | 500 | 4+ spaces indent |
| `ListParser` | 300 | `-`, `+`, `*`, `1.`, etc. |
| `ListItemParser` | 400 | Nested list items |
| `ThematicBreakParser` | 200 | `---`, `***`, `___` |
| `SetextHeadingParser` | 100 | `===` / `---` underlines |
| `ParagraphParser` | 1000 | Default fallback |

---

## Appendix C: Built-in Inline Parsers

| Parser | Priority | Trigger |
|--------|----------|---------|
| `CodeSpanParser` | 100 | `` ` `` |
| `LinkParser` | 200 | `[`, `]`, `(`, `)` |
| `AutoLinkParser` | 300 | URLs, emails |
| `RawHtmlParser` | 400 | `<`, `!` |
| `EmphasisParser` | 500 | `*`, `_` |

---

## Appendix D: Node Type System

```
Node
├── type_data: TypeData (Block | Inline)
│   └── Block: source lines, parent/child/sibling relations
│   └── Inline: child nodes (for rich inline like Link, Emphasis)
└── kind_data: KindData (25 variants)
    └── Document, Paragraph, Heading, ThematicBreak, CodeBlock
    └── Blockquote, List, ListItem, HtmlBlock
    └── Text, CodeSpan, Emphasis, Strong, Link, Image, RawHtml
    └── LinkReferenceDefinition, Table, TableHeader, TableBody, TableRow, TableCell
    └── Strikethrough, Diagram, Extension
```

---

## Appendix E: Text Module Types

| Type | Description |
|------|-------------|
| `Value` | Text value enum (Index, String, etc.) |
| `Index` | Byte offset pair (start, end) into source |
| `MultilineValue` | Multi-line text value (Empty, Segments, String) |
| `Lines` | Collection of lines (Empty, Segments, String) |
| `Segment` | Source segment with byte offset |
| `Block` | Array of `Segment` |
| `BlockExt` | Extension trait for `Block` |
| `Reader<'a>` | Reader trait |
| `BasicReader<'a>` | Basic line-based reader |
| `BlockReader<'a>` | Block-level reader for inline parsing |
| `EOS` | End-of-string marker (0xff) |

---

## Appendix F: Util Module Types

| Type | Description |
|------|-------------|
| `StringMap<V>` | String-keyed map (hashbrown or std) |
| `TinyVec<T>` | Small-vector optimization |
| `Prioritized<T>` | Priority-value pair |
| `AsciiWordSet` | Set of ASCII words for attribute filtering |
| `AsciiWordSetOptions` | Options for word set construction |
| `CowByteBuffer<'a>` | Cow-based byte buffer |
| `EscapeUrlOptions` | URL escaping configuration |
| `SafeStr` | Trait for safe string escaping |
| `escape_html()` | HTML entity escaping |
| `escape_url()` | URL escaping |
| `look_up_html5_entity_by_name()` | HTML5 entity lookup |
| `resolve_entity_references()` | Entity reference resolution |
| `resolve_numeric_references()` | Numeric reference resolution |
| `is_space()`, `is_punct()` | Character classification |
| `trim_left()`, `trim_right()` | Trim functions |
| `unescape_puncts()` | Punctuation unescaping |
| `to_link_reference()` | Link reference normalization |
| `collapse_spaces()` | Space collapsing |

---

## Appendix G: Scanner Module

| File | Description |
|------|-------------|
| `scanner/mod.rs` | Scanner trait and public API |
| `scanner/scanner_gen.rs` | re2c-generated HTML tag, URL, entity scanners |

Key scanners: HTML tag detection, URL detection, entity reference detection.
