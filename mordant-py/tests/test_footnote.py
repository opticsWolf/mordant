"""Tests for footnote extension integration."""

import mordant


# ---------------------------------------------------------------------------
# Basic footnote rendering
# ---------------------------------------------------------------------------

def test_footnote_reference_basic():
    """[^1] renders as <sup><a>."""
    md = "Text with footnote.[^1]\n\n[^1]: The footnote.\n"
    html = mordant.markdown_to_html(md)
    assert '<sup id="fnref:1">' in html
    assert 'href="#fn:1"' in html
    assert 'class="footnote-ref"' in html


def test_footnote_definition_basic():
    """[^1]: text renders in footnotes div."""
    md = "Text with footnote.[^1]\n\n[^1]: The footnote.\n"
    html = mordant.markdown_to_html(md)
    assert '<div class="footnotes"' in html
    assert 'role="doc-endnotes"' in html
    assert '<li id="fn:1">' in html


def test_footnote_named():
    """[^named] / [^named]: with named footnotes."""
    md = "Text with named footnote.[^hello]\n\n[^hello]: The named footnote.\n"
    html = mordant.markdown_to_html(md)
    assert 'fnref:1' in html
    assert 'fn:1' in html


def test_footnote_multiple_refs():
    """Multiple [^1] to same [^1]:."""
    md = "First ref [^1] and second ref [^1].\n\n[^1]: Shared footnote.\n"
    html = mordant.markdown_to_html(md)
    # Two superscript refs + two backlinks = 4 fnref occurrences
    assert html.count('fnref:') == 4
    # Both backlinks should appear
    assert html.count('footnote-backref') == 2


def test_footnote_definition_at_end():
    """<div class="footnotes"> at end of document."""
    md = "Text with footnote.[^1]\n\n[^1]: The footnote.\n"
    html = mordant.markdown_to_html(md)
    # footnotes div should be at the end
    assert html.strip().endswith('</div>')
    assert 'class="footnotes"' in html


def test_footnote_no_footnotes():
    """Document without footnotes has no footnotes div."""
    md = "Just some plain text.\n"
    html = mordant.markdown_to_html(md)
    assert 'footnotes' not in html
    assert 'fnref' not in html


def test_footnote_multiline_definition():
    """Multi-paragraph footnote definition."""
    md = "Text with footnote.[^1]\n\n[^1]: First paragraph.\n    \n    Second paragraph.\n"
    html = mordant.markdown_to_html(md)
    assert '<p>First paragraph.</p>' in html
    # Second paragraph has backlink appended inside
    assert 'Second paragraph' in html


# ---------------------------------------------------------------------------
# Custom options
# ---------------------------------------------------------------------------

def test_footnote_options_custom_classes():
    """Custom link_class, backlink_class."""
    md = "Text with footnote.[^1]\n\n[^1]: The footnote.\n"
    opts = mordant.FootnoteHtmlRendererOptions(
        link_class="my-ref",
        backlink_class="my-back",
    )
    html = mordant.markdown_to_html(md, footnote_render_opts=opts)
    assert 'class="my-ref"' in html
    assert 'class="my-back"' in html


def test_footnote_options_custom_backlink():
    """Custom backlink_html."""
    md = "Text with footnote.[^1]\n\n[^1]: The footnote.\n"
    opts = mordant.FootnoteHtmlRendererOptions(
        backlink_html="↑ back",
    )
    html = mordant.markdown_to_html(md, footnote_render_opts=opts)
    assert '↑ back' in html


def test_footnote_options_id_prefix():
    """Custom id_prefix."""
    md = "Text with footnote.[^1]\n\n[^1]: The footnote.\n"
    opts = mordant.FootnoteHtmlRendererOptions(
        id_prefix="note-",
    )
    html = mordant.markdown_to_html(md, footnote_render_opts=opts)
    assert 'id="note-fnref:1"' in html
    assert 'href="#note-fn:1"' in html
    assert 'id="note-fn:1"' in html


# ---------------------------------------------------------------------------
# Node properties
# ---------------------------------------------------------------------------

def test_footnote_node_properties():
    """footnote_label, footnote_index, footnote_references."""
    md = "Ref [^1] and [^hello].\n\n[^1]: First.\n\n[^hello]: Second.\n"
    doc = mordant.parse(md)

    footnote_refs = []
    footnote_defs = []
    for node in doc.walk("depth"):
        if node.kind == "FootnoteReference":
            footnote_refs.append(node)
        elif node.kind == "FootnoteDefinition":
            footnote_defs.append(node)

    assert len(footnote_refs) == 2
    assert len(footnote_defs) == 2

    # Check reference properties
    ref = footnote_refs[0]
    assert ref.footnote_label == "1"
    assert ref.footnote_index == 1

    # Check definition properties
    defn = footnote_defs[0]
    assert defn.footnote_label == "1"
    assert defn.footnote_index == 1
    assert defn.footnote_references is not None
    assert len(defn.footnote_references) == 1


def test_footnote_node_non_footnote():
    """footnote properties return None for non-footnote nodes."""
    md = "# Heading\n"
    doc = mordant.parse(md)
    heading = doc.children[0]
    assert heading.kind == "Heading"
    assert heading.footnote_label is None
    assert heading.footnote_index is None
    assert heading.footnote_references is None


# ---------------------------------------------------------------------------
# Footnotes with other extensions
# ---------------------------------------------------------------------------

def test_footnote_with_gfm():
    """Footnotes + GFM tables/strikethrough/task lists."""
    md = "Text with footnote.[^1]\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n[^1]: GFM table footnote.\n"
    html = mordant.markdown_to_html(md, gfm_opts=mordant.GfmOptions.all())
    assert '<table>' in html
    assert 'fnref:1' in html


def test_footnote_with_emoji():
    """Footnotes + emoji."""
    md = "Text with footnote :joy:.[^1]\n\n[^1]: Emoji footnote.\n"
    html = mordant.markdown_to_html(md)
    assert 'fnref:1' in html


def test_footnote_with_math():
    """Footnotes + math blocks."""
    md = "Text with footnote.[^1]\n\n$$E = mc^2$$\n\n[^1]: Math footnote.\n"
    html = mordant.markdown_to_html(md)
    assert 'fnref:1' in html


def test_footnote_with_diagram():
    """Footnotes + Mermaid diagrams."""
    md = "Text with footnote.[^1]\n\n```mermaid\ngraph TD; A-->B;\n```\n\n[^1]: Diagram footnote.\n"
    html = mordant.markdown_to_html(md)
    assert 'fnref:1' in html


def test_footnote_with_frontmatter():
    """Footnotes + YAML frontmatter."""
    md = "---\ntitle: Test\n---\n\nText with footnote.[^1]\n\n[^1]: Frontmatter footnote.\n"
    html = mordant.markdown_to_html(md)
    assert 'fnref:1' in html


def test_footnote_with_highlighting():
    """Footnotes + code highlighting."""
    md = "Text with footnote.[^1]\n\n```python\nprint('hello')\n```\n\n[^1]: Highlighting footnote.\n"
    html = mordant.markdown_to_html(md, highlighting_theme="InspiredGitHub")
    assert 'fnref:1' in html


# ---------------------------------------------------------------------------
# Edge cases
# ---------------------------------------------------------------------------

def test_footnote_empty_label():
    """[]: not parsed as footnote."""
    md = "Text with []: not a footnote.\n"
    html = mordant.markdown_to_html(md)
    assert 'footnotes' not in html


def test_footnote_invalid_ref_no_def():
    """[^unknown] without definition passes through."""
    md = "Text with [^unknown].\n"
    html = mordant.markdown_to_html(md)
    assert 'fnref' not in html
    assert 'footnotes' not in html


def test_footnote_multiple_definitions():
    """Multiple different footnotes."""
    md = "First [^a] and second [^b] and third [^c].\n\n[^a]: Footnote A.\n[^b]: Footnote B.\n[^c]: Footnote C.\n"
    html = mordant.markdown_to_html(md)
    # 3 superscript refs + 3 backlinks = 6 fnref occurrences
    assert html.count('fnref:') == 6
    assert html.count('footnote-backref') == 3


def test_footnote_unreferenced_definition():
    """Definition without reference should not appear in footnotes div."""
    md = "Text only.\n\n[^unused]: This footnote is never referenced.\n"
    html = mordant.markdown_to_html(md)
    assert 'footnotes' not in html


def test_footnote_xhtml():
    """XHTML mode produces self-closing hr."""
    md = "Text with footnote.[^1]\n\n[^1]: The footnote.\n"
    render_opts = mordant.RenderOptions(xhtml=True)
    html = mordant.markdown_to_html(md, render_opts=render_opts)
    assert '<hr />' in html or '<br />' in html  # xhtml flag affects self-closing tags


# ---------------------------------------------------------------------------
# Default options
# ---------------------------------------------------------------------------

def test_footnote_default_options():
    """Default FootnoteHtmlRendererOptions values."""
    opts = mordant.FootnoteHtmlRendererOptions()
    assert opts.link_class == "footnote-ref"
    assert opts.backlink_class == "footnote-backref"
    assert opts.backlink_html == "&#x21a9;&#xfe0e;"
    assert opts.id_prefix is None


def test_footnote_setter_options():
    """Setting options via setters."""
    opts = mordant.FootnoteHtmlRendererOptions()
    opts.link_class = "custom-ref"
    opts.backlink_class = "custom-back"
    opts.backlink_html = "↩"
    opts.id_prefix = "my-"

    md = "Text with footnote.[^1]\n\n[^1]: The footnote.\n"
    html = mordant.markdown_to_html(md, footnote_render_opts=opts)
    assert 'class="custom-ref"' in html
    assert 'class="custom-back"' in html
    assert '↩' in html
    assert 'id="my-fnref:1"' in html
