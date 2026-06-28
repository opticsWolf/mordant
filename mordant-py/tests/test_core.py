"""Core tests for rushdown Python bindings."""

import mordant


# === Basic CommonMark ===

def test_heading():
    html = mordant.markdown_to_html("# Hello")
    assert "<h1>Hello</h1>" in html


def test_paragraph():
    html = mordant.markdown_to_html("Hello world")
    assert "<p>Hello world</p>" in html


def test_bold():
    html = mordant.markdown_to_html("**bold**")
    assert "<strong>bold</strong>" in html


def test_italic():
    html = mordant.markdown_to_html("*italic*")
    assert "<em>italic</em>" in html


def test_code_span():
    html = mordant.markdown_to_html("`code`")
    assert "<code>code</code>" in html


def test_link():
    html = mordant.markdown_to_html("[text](http://example.com)")
    assert '<a href="http://example.com">text</a>' in html


def test_image():
    html = mordant.markdown_to_html("![alt](http://example.com/img.png)")
    assert '<img src="http://example.com/img.png"' in html


def test_blockquote():
    html = mordant.markdown_to_html("> quoted")
    assert "<blockquote>" in html
    assert "<p>quoted</p>" in html


def test_unordered_list():
    html = mordant.markdown_to_html("- item 1\n- item 2")
    assert "<ul>" in html
    assert "<li>item 1</li>" in html


def test_ordered_list():
    html = mordant.markdown_to_html("1. first\n2. second")
    assert "<ol>" in html
    assert "<li>first</li>" in html


def test_code_block():
    html = mordant.markdown_to_html("```\ncode\n```")
    assert "<pre><code>" in html
    assert "code" in html


def test_thematic_break():
    html = mordant.markdown_to_html("---")
    assert "<hr" in html


def test_empty_input():
    html = mordant.markdown_to_html("")
    assert html == "" or html == "\n"


def test_unicode():
    html = mordant.markdown_to_html("# \u4e2d\u6587\n\n\u6d4b\u8bd5")
    assert "\u4e2d\u6587" in html
    assert "\u6d4b\u8bd5" in html
