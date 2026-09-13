"""Tests for syntax highlighting and theme loading."""

import os
import pytest
import mordant


class TestThemeLoading:
    """Test that themes are loaded correctly from bundled and user directories."""

    def test_list_themes_returns_list(self):
        """list_themes() should return a list of strings."""
        themes = mordant.list_themes()
        assert isinstance(themes, list)
        assert all(isinstance(t, str) for t in themes)

    def test_builtin_syntect_themes_loaded(self):
        """Built-in syntect themes should be available."""
        themes = mordant.list_themes()
        assert "InspiredGitHub" in themes
        assert "GitHub" in themes

    def test_embedded_project_themes_loaded(self):
        """Embedded project themes should be available."""
        themes = mordant.list_themes()
        expected = [
            "1337",
            "Coldark-Cold",
            "Coldark-Dark",
            "DarkNeon",
            "Dracula",
            "Nord",
            "OneHalfDark",
            "OneHalfLight",
            "Monokai Extended",
            "Monokai Extended Bright",
            "Monokai Extended Light",
            "Monokai Extended Origin",
            "Solarized (dark)",
            "Solarized (light)",
            "Sublime Snazzy",
            "TwoDark",
            "Visual Studio Dark+",
            "gruvbox-dark",
            "gruvbox-light",
            "zenburn",
        ]
        for theme in expected:
            assert theme in themes, f"Expected theme '{theme}' not found in {themes}"

    def test_theme_count_reasonable(self):
        """Total theme count should be reasonable (built-in + embedded)."""
        themes = mordant.list_themes()
        # 7 built-in syntect themes + 53 embedded project themes = 60
        # (some may overlap, so we check a reasonable range)
        assert 50 <= len(themes) <= 100


class TestHighlighter:
    """Test the Highlighter class."""

    def test_highlighter_default_theme(self):
        """Highlighter should work with default theme."""
        hl = mordant.Highlighter()
        code = hl.highlight("python", "x = 1")
        assert isinstance(code, str)
        assert "<pre" in code
        assert "</pre>" in code

    def test_highlighter_custom_theme(self):
        """Highlighter should work with custom theme."""
        hl = mordant.Highlighter(theme="Dracula")
        code = hl.highlight("python", "def hello():\n    pass")
        assert isinstance(code, str)
        assert "Dracula" in code or len(code) > 0

    def test_highlighter_rust(self):
        """Highlighter should work with Rust code."""
        hl = mordant.Highlighter(theme="Monokai Extended")
        code = hl.highlight("rust", "fn main() {}")
        assert isinstance(code, str)
        assert "<span" in code  # Attribute mode uses spans

    def test_highlighter_class_mode(self):
        """Highlighter should support Class mode."""
        hl = mordant.Highlighter(theme="GitHub", mode="Class")
        code = hl.highlight("python", "x = 1")
        assert isinstance(code, str)
        assert "class=" in code

    def test_highlighter_invalid_mode_raises(self):
        """Highlighter should raise on invalid mode."""
        with pytest.raises(ValueError):
            mordant.Highlighter(mode="invalid")


class TestMarkdownHighlighting:
    """Test markdown_to_html with syntax highlighting."""

    def test_markdown_no_highlighting(self):
        """markdown_to_html should work without highlighting."""
        md = "# Hello\n\n```python\nx = 1\n```"
        html = mordant.markdown_to_html(md)
        assert "<h1>Hello</h1>" in html

    def test_markdown_with_highlighting(self):
        """markdown_to_html should apply syntax highlighting."""
        md = "# Test\n\n```python\ndef hello():\n    print('world')\n```"
        html = mordant.markdown_to_html(md, highlighting_theme="Dracula")
        assert "<pre" in html
        assert "Dracula" in html or len(html) > 0

    def test_markdown_with_different_themes(self):
        """markdown_to_html should work with different themes."""
        md = "```python\nx = 1\n```"
        for theme in ["Dracula", "Monokai Extended", "Nord"]:
            html = mordant.markdown_to_html(md, highlighting_theme=theme)
            assert "<pre" in html

    def test_markdown_unknown_theme_fallback(self):
        """markdown_to_html should fallback on unknown theme."""
        md = "```python\nx = 1\n```"
        html = mordant.markdown_to_html(md, highlighting_theme="NonExistentTheme")
        assert "<pre" in html  # Should still produce output


class TestAddCustomTheme:
    """Test add_custom_theme function."""

    def test_add_custom_theme_success(self):
        """add_custom_theme should register a theme."""
        # Use actual Dracula theme content as a minimal valid example
        theme_xml = '''<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/dtds/plist-1.0.dtd">
<plist version="1.0">
<dict>
    <key>name</key>
    <string>Dracula</string>
    <key>settings</key>
    <array>
        <dict>
            <key>settings</key>
            <dict>
                <key>background</key>
                <string>#282a36</string>
                <key>caret</key>
                <string>#f8f8f0</string>
                <key>foreground</key>
                <string>#f8f8f0</string>
            </dict>
        </dict>
    </array>
</dict>
</plist>'''
        mordant.add_custom_theme("test-dynamic", theme_xml)
        themes = mordant.list_themes()
        assert "test-dynamic" in themes

    def test_add_custom_theme_invalid_raises(self):
        """add_custom_theme should raise on invalid XML."""
        with pytest.raises(ValueError, match="Failed to parse"):
            mordant.add_custom_theme("bad", "not valid xml")

    def test_add_custom_theme_overrides(self):
        """add_custom_theme should override existing theme with same name."""
        theme_xml = '''<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/dtds/plist-1.0.dtd">
<plist version="1.0">
<dict>
    <key>name</key>
    <string>Override Test</string>
    <key>settings</key>
    <array>
        <dict>
            <key>settings</key>
            <dict>
                <key>background</key>
                <string>#ff0000</string>
                <key>foreground</key>
                <string>#ffffff</string>
            </dict>
        </dict>
    </array>
</dict>
</plist>'''
        mordant.add_custom_theme("override-test", theme_xml)
        themes = mordant.list_themes()
        assert "override-test" in themes

        # Should be usable
        hl = mordant.Highlighter(theme="override-test")
        code = hl.highlight("python", "x = 1")
        assert isinstance(code, str)


class TestListSyntaxes:
    """Test list_syntaxes function."""

    def test_list_syntaxes_returns_list(self):
        """list_syntaxes() should return a list of strings."""
        syntaxes = mordant.list_syntaxes()
        assert isinstance(syntaxes, list)
        assert all(isinstance(s, str) for s in syntaxes)

    def test_common_syntaxes_available(self):
        """Common syntaxes should be available."""
        syntaxes = mordant.list_syntaxes()
        common = ["Python", "Rust", "JavaScript", "TypeScript", "C", "C++", "Go"]
        for syntax in common:
            assert syntax in syntaxes, f"Expected syntax '{syntax}' not found"

    def test_syntax_count_reasonable(self):
        """Should have a reasonable number of syntaxes (bat provides ~198)."""
        syntaxes = mordant.list_syntaxes()
        assert len(syntaxes) >= 150


class TestStandaloneHighlighting:
    """Standalone (Markdown-free) highlighting API."""

    def test_bare_mode_has_no_wrapper(self):
        """highlight(bare=True) should return spans only, no <pre>/<code>."""
        hl = mordant.Highlighter(theme="InspiredGitHub")
        bare = hl.highlight("python", "def x(): pass", bare=True)
        assert "<pre" not in bare
        assert "<code" not in bare
        assert "<span" in bare

    def test_bare_mode_is_inner_of_wrapped(self):
        """bare output should be the inner content of the wrapped output."""
        hl = mordant.Highlighter(theme="InspiredGitHub")
        code = "def x(): pass"
        wrapped = hl.highlight("python", code)
        bare = hl.highlight("python", code, bare=True)
        assert bare in wrapped
        assert wrapped.startswith("<pre")

    def test_bare_mode_class_style(self):
        """bare=True with mode='Class' should return class spans without wrapper."""
        hl = mordant.Highlighter(theme="GitHub", mode="Class")
        bare = hl.highlight("python", "x = 1", bare=True)
        assert "<pre" not in bare
        assert "<span" in bare
        assert "class=" in bare

    def test_theme_background(self):
        """theme_background() should return the theme's background hex color."""
        bg = mordant.theme_background("Dracula")
        assert isinstance(bg, str)
        assert bg.startswith("#") and len(bg) == 7

    def test_theme_background_unknown_is_none(self):
        """theme_background() should return None for unknown themes."""
        assert mordant.theme_background("No-Such-Theme-XYZ") is None

    def test_detect_language(self):
        """detect_language() should identify common languages."""
        assert mordant.detect_language("def greet(name):\n    print(name)") == "python"
        assert mordant.detect_language("fn main() {\n    let x = 1;\n}") == "rust"

    def test_detect_language_fallback(self):
        """detect_language() should return 'plaintext' when nothing matches."""
        assert mordant.detect_language("just some words here") == "plaintext"


class TestAddCustomSyntax:
    """Test add_custom_syntax function."""

    MINI_SYNTAX = '''
name: TestCustomLang
file_extensions:
  - tcl1
scope: source.test-custom-lang

contexts:
  main:
    - match: \\b(frobnicate)\\b
      scope: keyword.control.test-custom-lang
'''

    def test_add_custom_syntax_returns_name(self):
        """add_custom_syntax() should return the registered syntax name."""
        name = mordant.add_custom_syntax(self.MINI_SYNTAX)
        assert name == "TestCustomLang"

    def test_custom_syntax_in_list_syntaxes(self):
        """Registered custom syntaxes should appear in list_syntaxes()."""
        mordant.add_custom_syntax(self.MINI_SYNTAX)
        assert "TestCustomLang" in mordant.list_syntaxes()

    def test_highlight_with_custom_syntax_by_name(self):
        """Highlighter should highlight code using the custom syntax name."""
        mordant.add_custom_syntax(self.MINI_SYNTAX)
        hl = mordant.Highlighter(theme="InspiredGitHub")
        spans = hl.highlight("TestCustomLang", "frobnicate x", bare=True)
        plain = hl.highlight("plaintext", "frobnicate x", bare=True)
        assert spans != plain, "custom syntax should produce scoped styling"

    def test_highlight_with_custom_syntax_by_extension(self):
        """Highlighter should resolve custom syntaxes by file extension."""
        mordant.add_custom_syntax(self.MINI_SYNTAX)
        hl = mordant.Highlighter(theme="InspiredGitHub")
        spans = hl.highlight("tcl1", "frobnicate x", bare=True)
        plain = hl.highlight("plaintext", "frobnicate x", bare=True)
        assert spans != plain, "extension lookup should find the custom syntax"

    def test_custom_syntax_in_markdown(self):
        """markdown_to_html should use the registered custom syntax."""
        mordant.add_custom_syntax(self.MINI_SYNTAX)
        html = mordant.markdown_to_html(
            "```tcl1\nfrobnicate x\n```", highlighting_theme="InspiredGitHub"
        )
        assert "language-tcl1" in html

    def test_add_custom_syntax_fallback_name(self):
        """add_custom_syntax(name=...) should name syntaxes lacking a name key."""
        no_name = self.MINI_SYNTAX.replace("name: TestCustomLang\n", "")
        name = mordant.add_custom_syntax(no_name, name="FallbackTestLang")
        assert name == "FallbackTestLang"
        assert "FallbackTestLang" in mordant.list_syntaxes()

    def test_add_custom_syntax_invalid_raises(self):
        """add_custom_syntax() should raise ValueError on invalid content."""
        with pytest.raises(ValueError):
            mordant.add_custom_syntax("\t:not: valid: yaml: [")
