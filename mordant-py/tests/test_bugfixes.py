"""Regression tests for the 0.11 bug-fix plan (BF-01 .. BF-23).

They were written before the fixes (against the initial 0.11 implementation)
and now serve as the regression suite for the corrected behavior.
"""

import pytest

import mordant


class TestCriticalParserFixes:
    def test_bf01_fenced_div_body_is_a_child(self):
        html = mordant.markdown_to_html("::: note\nHello *world*.\n:::\n")
        assert html == '<div class="note">\n<p>Hello <em>world</em>.</p>\n</div>\n', html

    def test_bf01_nested_divs_without_indentation(self):
        html = mordant.markdown_to_html(
            "::: outer\n::: inner\nnested\n:::\n:::\n"
        )
        assert html.count("<div") == 2, html
        assert html.count("</div>") == 2, html
        assert "<p>nested</p>" in html, html

    def test_bf02_first_definition_line_is_kept(self):
        html = mordant.markdown_to_html("Term\n: a\nb\n")
        assert "<dd>\na\nb" in html, html

    def test_bf02_crlf(self):
        html = mordant.markdown_to_html("Term\r\n: a\r\nb\r\n")
        assert "<dd>\na\nb" in html, html

    def test_bf03_paragraph_before_term_is_kept(self):
        html = mordant.markdown_to_html("intro\nTerm\n: def\n")
        assert "intro" in html, html
        assert "<dl>" not in html, html

    def test_bf04_next_term_is_not_swallowed(self):
        html = mordant.markdown_to_html(
            "Apple\n: A fruit.\nOrange\n: Citrus fruit.\n"
        )
        assert html.count("<dt>") == 2, html
        assert "A fruit." in html and "Citrus fruit." in html, html

    def test_bf05_link_attributes_after_inline_split(self):
        cases = [
            ('[a](x){#id .cls target="_blank"}', 'target="_blank"'),
            ("[a](x){.my_class}", 'class="my_class"'),
            ("[a](x){#my_id}", 'id="my_id"'),
            ('[a](x){title="a_b"}', 'title="a_b"'),
            ('[a](x){title="a:b"}', 'title="a:b"'),
            ('[a](x){title="a*b"}', 'title="a*b"'),
            ('[a](x){title="a`b"}', 'title="a`b"'),
        ]
        for source, expected in cases:
            html = mordant.markdown_to_html(source)
            assert expected in html, (source, html)

    def test_bf08_emoji_shortcode_is_not_a_definition(self):
        html = mordant.markdown_to_html("Hello\n:smile: world\n")
        assert "😄" in html, html
        assert "<dl>" not in html, html


class TestSecurity:
    def test_bf06_duplicate_href_is_dropped(self):
        html = mordant.markdown_to_html(
            '[a](https://safe){href=javascript&#58;alert(1)}'
        )
        assert html.count("href=") == 1, html
        assert "javascript:" not in html, html

    def test_bf06_style_requires_allows_unsafe(self):
        source = "::: {.note style=\"color:red\"}\ntext\n:::\n"
        html = mordant.markdown_to_html(source)
        assert "style=" not in html, html
        html = mordant.markdown_to_html(source, render_opts=mordant.RenderOptions(allows_unsafe=True))
        assert "style=" in html, html


class TestOptionSemantics:
    def test_bf17_highlighter_options_are_validated(self):
        with pytest.raises(ValueError):
            mordant.HighlighterOptions(mode="Bogus")
        with pytest.raises(ValueError):
            mordant.HighlighterOptions(theme="No-Such-Theme-XYZ")
        opts = mordant.HighlighterOptions(theme="Dracula", mode="class")
        assert opts.mode == "Class"

    def test_bf18_nested_diagram_options_survive_theme_kwarg(self):
        source = "```mermaid\ngraph LR\n  A --- B\n```\n"
        opts = mordant.RenderOptions(
            diagram=mordant.DiagramHtmlRendererOptions(render_mode="client", theme="dark")
        )
        html = mordant.markdown_to_html(source, render_opts=opts, theme="Dracula")
        assert '<pre class="mermaid">' in html
        assert "theme: 'dark'" in html
        assert "themeVariables" not in html

    def test_bf18_explicit_highlighting_mode_wins(self):
        opts = mordant.RenderOptions(
            highlighter=mordant.HighlighterOptions(theme="Dracula", mode="Class")
        )
        html = mordant.markdown_to_html(
            "```python\nx=1\n```", render_opts=opts, highlighting_mode="Attribute"
        )
        assert "hljs-" not in html
        assert 'style="color:' in html


class TestEngineSemantics:
    def test_bf09_default_has_no_highlighting(self):
        html = mordant.markdown_to_html("```python\nx=1\n```")
        assert "style=" not in html, html
        assert "<span" not in html, html
        html = mordant.markdown_to_html(
            "```python\nx=1\n```", engines=mordant.Engines.all()
        )
        assert "style=" in html, html

    def test_bf10_without_keeps_defaults(self):
        assert not mordant.Engines.without(["emoji"]).highlighter
        assert mordant.Engines.all_without(["emoji"]).highlighter

    def test_bf11_nested_highlighter_implies_engine(self):
        opts = mordant.RenderOptions(
            highlighter=mordant.HighlighterOptions(theme="Dracula")
        )
        html = mordant.markdown_to_html("```python\nx=1\n```", render_opts=opts)
        assert "style=" in html, html
        html = mordant.markdown_to_html(
            "```python\nx=1\n```", render_opts=opts, engines=mordant.Engines.none()
        )
        assert "style=" not in html, html

    def test_bf12_explicit_engines_beat_legacy_theme(self):
        html = mordant.markdown_to_html(
            "```python\nx=1\n```", engines=mordant.Engines.none(), theme="Dracula"
        )
        assert "style=" not in html, html
        opts = mordant.RenderOptions(engines=mordant.Engines.none())
        html = mordant.markdown_to_html(
            "```python\nx=1\n```", render_opts=opts, highlighting_theme="Dracula"
        )
        assert "style=" not in html, html
        html = mordant.markdown_to_html("```python\nx=1\n```", theme="Dracula")
        assert "style=" in html, html

    def test_bf13_linter_flag(self):
        source = "# A\n\n### C\n"
        assert mordant.lint(source) != []
        assert mordant.lint(source, engines=mordant.Engines.none()) == []
        result = mordant.fix("# A  \n", engines=mordant.Engines.none())
        assert result.output == "# A  \n"

    def test_bf13_chunker_flag(self):
        source = "::: note\nx\n:::\n"
        default = list(mordant.MarkdownChunker(source))
        disabled = list(
            mordant.MarkdownChunker(source, engines=mordant.Engines.none())
        )
        assert default != disabled, (default, disabled)

    def test_bf14_option_getter_mutation(self):
        opts = mordant.ParseOptions()
        opts.engines.emoji = False
        assert not opts.engines.emoji
        opts2 = mordant.RenderOptions()
        opts2.highlighter = mordant.HighlighterOptions(theme="Dracula")
        opts2.highlighter.theme = "Nord"
        assert opts2.highlighter.theme == "Nord"

    def test_bf14_mutated_render_options_engines_are_used(self):
        opts = mordant.RenderOptions()
        opts.engines.emoji = False
        html = mordant.markdown_to_html(":smile:", render_opts=opts)
        assert "😄" not in html, html
        assert ":smile:" in html

    def test_bf15_mismatched_parse_render_engines_are_diagnosed(self):
        parse_opts = mordant.ParseOptions(engines=mordant.Engines.all())
        render_opts = mordant.RenderOptions(engines=mordant.Engines.none())
        with pytest.raises(ValueError, match="conflicting engine sets"):
            mordant.markdown_to_html(
                ":smile:", parse_opts=parse_opts, render_opts=render_opts
            )
        # An engine set on either options object alone is inherited by the
        # other side, so no content is silently lost.
        assert "😄" not in mordant.markdown_to_html(
            ":smile:", render_opts=render_opts
        )
        # The top-level `engines` argument configures both sides.
        html = mordant.markdown_to_html(
            ":smile:", engines=mordant.Engines.none()
        )
        assert ":smile:" in html and "😄" not in html


class TestGuidelineAmendment:
    def test_gl10_style_policy_is_scoped_to_new_extensions(self):
        unsafe = mordant.RenderOptions(allows_unsafe=False)

        # The older attributes extension keeps its 0.10 behavior.
        old = mordant.markdown_to_html(
            '# hi {style="color:red"}',
            parse_opts=mordant.ParseOptions(attributes=True),
            render_opts=unsafe,
        )
        assert 'style="color:red"' in old, old

        # The new extensions drop style unless explicitly allowed.
        link = mordant.markdown_to_html(
            '[a](https://example.com){style="color:red"}', render_opts=unsafe
        )
        assert "style=" not in link, link
        div = mordant.markdown_to_html(
            '::: {style="color:red"}\nx\n:::\n', render_opts=unsafe
        )
        assert "style=" not in div, div

        allowed = mordant.markdown_to_html(
            '[a](https://example.com){style="color:red"}',
            render_opts=mordant.RenderOptions(allows_unsafe=True),
        )
        assert 'style="color:red"' in allowed, allowed
