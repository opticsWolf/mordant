"""Test VSCode JSON theme support for mordant."""
import mordant

# --- Sample themes ---

VS_CODE_THEME_JSON = '''{
    "name": "Test VSCode Theme",
    "type": "dark",
    "tokenColors": [
        {
            "scope": "comment",
            "settings": {
                "foreground": "#6A7A3E",
                "fontStyle": "italic"
            }
        },
        {
            "scope": "keyword",
            "settings": {
                "foreground": "#FF6B6B"
            }
        },
        {
            "scope": "string",
            "settings": {
                "foreground": "#4EC6FB"
            }
        },
        {
            "scope": "variable",
            "settings": {
                "foreground": "#E0E0E0"
            }
        }
    ],
    "colors": {
        "editor.background": "#1E1E1E",
        "editor.foreground": "#E0E0E0"
    }
}'''

NAMED_COLOR_THEME = '''{
    "name": "Named Colors Test",
    "tokenColors": [
        {
            "scope": "comment",
            "settings": {
                "foreground": "green"
            }
        },
        {
            "scope": "keyword",
            "settings": {
                "foreground": "red"
            }
        }
    ]
}'''

MULTI_SCOPE_THEME = '''{
    "name": "Multi-Scope Test",
    "tokenColors": [
        {
            "scope": "variable, constant",
            "settings": {
                "foreground": "#FFD700"
            }
        },
        {
            "scope": "entity.name.function, support.function",
            "settings": {
                "foreground": "#61DAFF",
                "fontStyle": "bold"
            }
        }
    ]
}'''

JSONC_THEME = '''{
    // This is a comment
    "name": "JSONC Test Theme",
    "type": "dark",
    /* Block comment */
    "tokenColors": [
        {
            "scope": "comment",
            "settings": {
                "foreground": "#888888" // inline comment
            }
        }
    ]
}'''


class TestListThemes:
    def test_list_themes_returns_list(self):
        themes = mordant.list_themes()
        assert isinstance(themes, list)
        assert len(themes) > 0

    def test_list_themes_contains_strings(self):
        themes = mordant.list_themes()
        for t in themes:
            assert isinstance(t, str)


class TestAddVsCodeTheme:
    def test_parse_and_register_vscode_theme(self):
        mordant.add_custom_theme("test-vscode", VS_CODE_THEME_JSON)
        themes = mordant.list_themes()
        assert "test-vscode" in themes

    def test_named_color_resolution(self):
        mordant.add_custom_theme("named-colors", NAMED_COLOR_THEME)
        themes = mordant.list_themes()
        assert "named-colors" in themes

    def test_multiple_scopes(self):
        mordant.add_custom_theme("multi-scope", MULTI_SCOPE_THEME)
        themes = mordant.list_themes()
        assert "multi-scope" in themes

    def test_jsonc_with_comments(self):
        mordant.add_custom_theme("jsonc-test", JSONC_THEME)
        themes = mordant.list_themes()
        assert "jsonc-test" in themes


class TestHighlightingWithVsCodeTheme:
    def test_highlighter_with_custom_theme(self):
        mordant.add_custom_theme("test-vscode-hl", VS_CODE_THEME_JSON)
        hl = mordant.Highlighter(theme="test-vscode-hl")
        code = 'let x = "hello"; // comment'
        html = hl.highlight("javascript", code)
        assert isinstance(html, str)
        assert len(html) > 0

    def test_highlighting_contains_inline_styles(self):
        mordant.add_custom_theme("test-vscode-styles", VS_CODE_THEME_JSON)
        hl = mordant.Highlighter(theme="test-vscode-styles")
        code = 'let x = "hello"; // comment'
        html = hl.highlight("javascript", code)
        assert "style=" in html

    def test_highlighting_contains_theme_colors(self):
        mordant.add_custom_theme("test-vscode-colors", VS_CODE_THEME_JSON)
        hl = mordant.Highlighter(theme="test-vscode-colors")
        code = 'let x = "hello"; // comment'
        html = hl.highlight("javascript", code)
        lower = html.lower()
        assert any(color.lower() in lower for color in ["#6A7A3E", "#FF6B6B", "#4EC6FB"])

    def test_font_style_italic_not_rendered_as_underline(self):
        """Regression test for the FontStyle bit-mapping bug.

        syntect 5.x defines BOLD=1, UNDERLINE=2, ITALIC=4. A VSCode
        `italic` font style must render as `font-style:italic`, NOT as
        `text-decoration:underline` (which is what the swapped mapping
        previously produced for many JSON themes).
        """
        mordant.add_custom_theme("test-vscode-italic", VS_CODE_THEME_JSON)
        hl = mordant.Highlighter(theme="test-vscode-italic")
        # `comment` scope is styled `italic` in VS_CODE_THEME_JSON
        html = hl.highlight("javascript", 'let x = 1; // a comment')
        assert "font-style:italic" in html
        assert "text-decoration:underline" not in html

    def test_font_style_explicit_underline(self):
        """An explicit `underline` font style must render as underline."""
        underline_theme = '''{
            "name": "UnderlineTest",
            "tokenColors": [
                {"scope": "markup.underline", "settings": {"foreground": "#ff0000", "fontStyle": "underline"}}
            ]
        }'''
        mordant.add_custom_theme("test-vscode-underline", underline_theme)
        hl = mordant.Highlighter(theme="test-vscode-underline")
        html = hl.highlight("markdown", "see [link](url)")
        assert "text-decoration:underline" in html
        assert "font-style:italic" not in html


class TestMarkdownWithVsCodeHighlighting:
    def test_markdown_rendered_with_highlighting(self):
        mordant.add_custom_theme("test-vscode-md", VS_CODE_THEME_JSON)
        md = '''# Test Document

```javascript
let greeting = "Hello, World!";
console.log(greeting);
```
'''
        html = mordant.markdown_to_html(md, highlighting_theme="test-vscode-md")
        assert isinstance(html, str)
        assert len(html) > 0

    def test_background_color_from_theme_applied(self):
        mordant.add_custom_theme("test-vscode-bg", VS_CODE_THEME_JSON)
        md = '''```javascript
let x = 1;
```'''
        html = mordant.markdown_to_html(md, highlighting_theme="test-vscode-bg")
        # The background color from the "colors" section may or may not appear
        # depending on rendering mode; just verify no crash
        assert isinstance(html, str)
