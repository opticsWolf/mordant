"""GFM (GitHub Flavored Markdown) extension tests."""

import mordant


# === Tables ===

def test_gfm_table_basic():
    md = """| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |"""
    html = mordant.markdown_to_html(md, gfm_opts=mordant.GfmOptions.all())
    assert "<table>" in html
    assert "<thead>" in html
    assert "<th>Header 1</th>" in html
    assert "<th>Header 2</th>" in html
    assert "<tbody>" in html
    assert "<td>Cell 1</td>" in html
    assert "<td>Cell 2</td>" in html


def test_gfm_table_disabled():
    md = """| A | B |
|---|---|
| 1 | 2 |"""
    html = mordant.markdown_to_html(md)
    assert "<table>" not in html


# === Strikethrough ===

def test_gfm_strikethrough():
    html = mordant.markdown_to_html("~~deleted~~", gfm_opts=mordant.GfmOptions.all())
    assert "<del>deleted</del>" in html or "<s>deleted</s>" in html


def test_gfm_strikethrough_disabled():
    html = mordant.markdown_to_html("~~deleted~~")
    assert "<del>" not in html and "<s>" not in html


# === Task Lists ===

def test_gfm_task_list_checked():
    md = "- [x] done"
    html = mordant.markdown_to_html(md, gfm_opts=mordant.GfmOptions.all())
    assert "task-list-item" in html or "checked" in html


def test_gfm_task_list_unchecked():
    md = "- [ ] todo"
    html = mordant.markdown_to_html(md, gfm_opts=mordant.GfmOptions.all())
    assert "<li>" in html


# === Linkify ===

def test_gfm_linkify_url():
    html = mordant.markdown_to_html("Visit https://example.com today", gfm_opts=mordant.GfmOptions.all())
    assert '<a href="https://example.com">' in html


def test_gfm_linkify_disabled():
    html = mordant.markdown_to_html("Visit https://example.com today")
    assert '<a href="https://example.com">' not in html


# === Combined GFM ===

def test_gfm_combined():
    md = """# GFM Test

~~Strikethrough~~ and **bold**.

| Col A | Col B |
|-------|-------|
| x     | y     |

- [x] Task 1
- [ ] Task 2

Visit https://example.com
"""
    html = mordant.markdown_to_html(md, gfm_opts=mordant.GfmOptions.all())
    assert "<table>" in html
    assert "<del>" in html or "<s>" in html
    assert "<strong>bold</strong>" in html
