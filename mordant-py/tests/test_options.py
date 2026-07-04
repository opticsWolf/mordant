"""Tests for options classes and parse function."""

import mordant


# === ParseOptions ===

def test_parse_options_default():
    opts = mordant.ParseOptions()
    assert opts.smart is False


def test_parse_options_full():
    opts = mordant.ParseOptions(
        smart=True,
        attributes=True,
        auto_heading_ids=True,
        escaped_space=True,
        meta_table=True,
    )
    assert opts.smart is True
    assert opts.attributes is True
    assert opts.auto_heading_ids is True
    assert opts.escaped_space is True
    assert opts.meta_table is True


def test_parse_options_setter():
    opts = mordant.ParseOptions()
    opts.attributes = True
    assert opts.attributes is True


# === RenderOptions ===

def test_render_options_default():
    opts = mordant.RenderOptions()
    assert opts.hard_wraps is False
    assert opts.xhtml is False
    assert opts.allows_unsafe is False
    assert opts.escaped_space is False


def test_render_options_custom():
    opts = mordant.RenderOptions(
        hard_wraps=True,
        xhtml=True,
        allows_unsafe=True,
        escaped_space=True,
    )
    assert opts.hard_wraps is True
    assert opts.xhtml is True
    assert opts.allows_unsafe is True
    assert opts.escaped_space is True


# === GfmOptions ===

def test_gfm_options_default():
    opts = mordant.GfmOptions()
    assert opts.has(mordant.GfmFeature.Table)
    assert opts.has(mordant.GfmFeature.Strikethrough)
    assert opts.has(mordant.GfmFeature.TaskList)
    assert not opts.has(mordant.GfmFeature.Linkify)


def test_gfm_options_all():
    opts = mordant.GfmOptions.all()
    assert opts.has(mordant.GfmFeature.Table)
    assert opts.has(mordant.GfmFeature.Strikethrough)
    assert opts.has(mordant.GfmFeature.TaskList)
    assert opts.has(mordant.GfmFeature.Linkify)


def test_gfm_options_custom():
    opts = mordant.GfmOptions(features=[mordant.GfmFeature.Table, mordant.GfmFeature.Strikethrough])
    assert opts.has(mordant.GfmFeature.Table)
    assert opts.has(mordant.GfmFeature.Strikethrough)
    assert not opts.has(mordant.GfmFeature.TaskList)
    assert not opts.has(mordant.GfmFeature.Linkify)


def test_gfm_options_none():
    opts = mordant.GfmOptions.none()
    assert opts.features == []


# === ArenaOptions ===

def test_arena_options_default():
    opts = mordant.ArenaOptions()
    assert opts.initial_size == 1024


def test_arena_options_custom():
    opts = mordant.ArenaOptions(initial_size=2048)
    assert opts.initial_size == 2048


# === Options wired through to parse() ===

def test_parse_with_auto_heading_ids():
    doc = mordant.parse(
        "# Hello World",
        parse_opts=mordant.ParseOptions(auto_heading_ids=True),
    )
    heading = doc.children[0]
    assert heading.kind == "Heading"
    assert "id" in heading.attributes
    assert heading.attributes["id"] == "hello-world"


def test_parse_with_attributes():
    doc = mordant.parse(
        "# Hello {#custom-id}",
        parse_opts=mordant.ParseOptions(attributes=True),
    )
    heading = doc.children[0]
    assert "id" in heading.attributes


def test_markdown_to_html_with_render_options():
    html = mordant.markdown_to_html(
        "a\nb\nc",
        render_opts=mordant.RenderOptions(hard_wraps=True),
    )
    assert "<br>" in html


def test_markdown_to_html_with_parse_opts():
    html = mordant.markdown_to_html(
        "# Hello",
        parse_opts=mordant.ParseOptions(auto_heading_ids=True),
    )
    assert 'id="hello"' in html


def test_parse_with_meta_table():
    doc = mordant.parse(
        "---\ntitle: Test\n---\n\nBody",
        parse_opts=mordant.ParseOptions(meta_table=True),
    )
    assert doc.children[0].kind == "Table"
    assert doc.metadata["title"] == "Test"


# === Document ===

def test_parse_basic():
    doc = mordant.parse("# Hello")
    assert doc.source == "# Hello"
    assert "Document" in repr(doc)


def test_parse_gfm():
    doc = mordant.parse("| A | B |\n|---|---|\n| 1 | 2 |", gfm_opts=mordant.GfmOptions.all())
    assert "| A | B |" in doc.source


def test_parse_empty():
    doc = mordant.parse("")
    assert doc.source == ""
