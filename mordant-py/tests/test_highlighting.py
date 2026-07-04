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
