"""Tests for the runtime engine switches and structured render options."""

import pytest

import mordant


class TestEnginesObject:
    def test_defaults(self):
        engines = mordant.Engines()
        assert engines.meta
        assert engines.emoji
        assert engines.footnotes
        assert engines.definition_list
        assert engines.fenced_div
        assert engines.link_attributes
        # Highlighting is opt-in to preserve 0.10 HTML output.
        assert not engines.highlighter

    def test_all_and_none(self):
        assert mordant.Engines.all().highlighter
        assert all(
            not mordant.Engines.none().has(name)
            for name in ["emoji", "math", "highlighter", "fenced_div"]
        )

    def test_only_and_without(self):
        engines = mordant.Engines.only(["emoji", "math"])
        assert engines.has("emoji") and engines.has("math")
        assert not engines.has("footnotes")
        engines = mordant.Engines.without(["emoji"])
        assert not engines.has("emoji") and engines.has("math")

    def test_invalid_name(self):
        with pytest.raises(ValueError):
            mordant.Engines.only(["nope"])

    def test_mutable_flags(self):
        engines = mordant.Engines()
        engines.emoji = False
        assert not engines.has("emoji")


class TestEngineSwitching:
    def test_emoji_can_be_disabled(self):
        source = ":smile:"
        assert "😄" in mordant.markdown_to_html(source)
        html = mordant.markdown_to_html(source, engines=mordant.Engines.none())
        assert "😄" not in html
        assert ":smile:" in html

    def test_definition_list(self):
        source = "Apple\n: A fruit.\n"
        html = mordant.markdown_to_html(source)
        assert "<dl>" in html and "<dt>" in html and "<dd>" in html
        html = mordant.markdown_to_html(source, engines=mordant.Engines.without(["definition_list"]))
        assert "<dl>" not in html
        assert "<p>" in html

    def test_fenced_div(self):
        source = "::: note\nHello.\n:::\n"
        html = mordant.markdown_to_html(source)
        assert '<div class="note">' in html
        html = mordant.markdown_to_html(source, engines=mordant.Engines.only([]))
        assert "<div" not in html

    def test_link_attributes(self):
        source = "[link](https://example.com){#foo .bar}"
        html = mordant.markdown_to_html(source)
        assert 'id="foo"' in html and 'class="bar"' in html
        html = mordant.markdown_to_html(source, engines=mordant.Engines.without(["link_attributes"]))
        assert 'id="foo"' not in html

    def test_parse_honours_engines(self):
        doc = mordant.parse("Apple\n: A fruit.\n", engines=mordant.Engines.none())
        children = doc.children
        assert len(children) == 1
        assert children[0].kind == "Paragraph"

    def test_parse_options_engines(self):
        opts = mordant.ParseOptions(engines=mordant.Engines.without(["emoji"]))
        html = mordant.markdown_to_html(":smile:", parse_opts=opts)
        assert "😄" not in html


class TestStructuredRenderOptions:
    def test_nested_math_options(self):
        opts = mordant.RenderOptions(
            math=mordant.MathRendererOptions(output="mathml")
        )
        html = mordant.markdown_to_html("```math\nx^2\n```", render_opts=opts)
        assert "<math" in html and "<annotation" in html

    def test_nested_footnote_options(self):
        opts = mordant.RenderOptions(
            footnote=mordant.FootnoteHtmlRendererOptions(id_prefix="doc-")
        )
        html = mordant.markdown_to_html("text[^1]\n\n[^1]: note", render_opts=opts)
        assert "doc-fnref" in html or "doc-" in html

    def test_nested_highlighter_options(self):
        opts = mordant.RenderOptions(
            engines=mordant.Engines.all(),
            highlighter=mordant.HighlighterOptions(theme="Dracula", mode="Class"),
        )
        html = mordant.markdown_to_html("```python\nx = 1\n```", render_opts=opts)
        assert "hljs-" in html or "language-python" in html

    def test_legacy_theme_still_enables_highlighting(self):
        html = mordant.markdown_to_html(
            "```python\nx = 1\n```", highlighting_theme="Dracula"
        )
        assert "style=" in html

    def test_engines_argument_overrides_render_options(self):
        opts = mordant.RenderOptions(engines=mordant.Engines.none())
        html = mordant.markdown_to_html(":smile:", render_opts=opts, engines=mordant.Engines.all())
        assert "😄" in html

    def test_katex_version(self):
        assert mordant.KATEX_VERSION == "0.18.5"
        assert mordant.KATEX_VERSION in mordant.KATEX_CSS
