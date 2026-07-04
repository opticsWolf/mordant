"""Emoji extension tests for mordant."""

import mordant


# === Basic Emoji Rendering ===

def test_emoji_basic():
    """Basic emoji rendering: :joy: should render as Unicode emoji."""
    html = mordant.markdown_to_html("I'm :joy:")
    assert "\U0001F602" in html  # 😂


def test_emoji_multiple():
    """Multiple emojis should all render."""
    html = mordant.markdown_to_html(":joy: :heart: :+1:")
    assert "\U0001F602" in html  # 😂
    assert "\u2764\ufe0f" in html  # ❤️
    assert "\U0001F44D" in html  # 👍


def test_emoji_in_paragraph():
    """Emojis in paragraph text should render."""
    html = mordant.markdown_to_html("Hello :joy:")
    assert "\U0001F602" in html  # 😂


# === Emoji Inside Code Spans ===

def test_emoji_inside_code_span():
    """Emojis inside code spans should NOT be parsed."""
    html = mordant.markdown_to_html("I'm `:joy:`")
    assert ":joy:" in html
    assert "\U0001F600" not in html


def test_emoji_inside_code_block():
    """Emojis inside code blocks should NOT be parsed."""
    md = "```\n:joy:\n```"
    html = mordant.markdown_to_html(md)
    assert ":joy:" in html
    assert "\U0001F600" not in html


# === Invalid Shortcodes ===

def test_emoji_invalid_shortcode_passthrough():
    """Unknown shortcodes should pass through unchanged."""
    html = mordant.markdown_to_html("I'm :joyjoy:")
    assert ":joyjoy:" in html
    assert "\U0001F600" not in html


def test_emoji_empty_shortcode():
    """Empty shortcode should pass through."""
    html = mordant.markdown_to_html("I'm ::")
    assert "::" in html


# === Emoji Blacklist ===

def test_emoji_blacklist_single():
    """Blacklisted shortcodes should not be parsed."""
    opts = mordant.PyEmojiParserOptions(blacklist="joy")
    html = mordant.markdown_to_html("I'm :joy:", emoji_parse_opts=opts)
    assert ":joy:" in html
    assert "\U0001F602" not in html


def test_emoji_blacklist_multiple():
    """Multiple blacklisted shortcodes should all be ignored."""
    opts = mordant.PyEmojiParserOptions(blacklist="joy,heart,smile")
    html = mordant.markdown_to_html(":joy: :heart: :smile:", emoji_parse_opts=opts)
    # Blacklisted emojis should pass through as-is
    assert ":joy:" in html or "\U0001F602" not in html
    assert ":heart:" in html or "\u2764\ufe0f" not in html


def test_emoji_blacklist_empty():
    """Empty blacklist should parse all emojis."""
    opts = mordant.PyEmojiParserOptions(blacklist="")
    html = mordant.markdown_to_html("I'm :joy:", emoji_parse_opts=opts)
    assert "\U0001F602" in html  # 😂


def test_emoji_blacklist_whitespace():
    """Blacklist with whitespace should be handled correctly."""
    opts = mordant.PyEmojiParserOptions(blacklist=" joy , heart ")
    html = mordant.markdown_to_html(":joy: :heart: :smile:", emoji_parse_opts=opts)
    # joy and heart should be blacklisted, smile should render
    assert "\U0001F602" not in html  # joy blacklisted
    assert "\u2764\ufe0f" not in html  # heart blacklisted


# === Custom HTML Template ===

def test_emoji_template_default():
    """Default rendering should use Unicode emoji character."""
    html = mordant.markdown_to_html("I'm :joy:")
    assert "\U0001F602" in html


def test_emoji_template_custom():
    """Custom template should render as HTML img tag."""
    template = '<img src="https://example.com/{shortcode}.png" />'
    opts = mordant.PyEmojiHtmlRendererOptions(template=template)
    html = mordant.markdown_to_html("I'm :joy:", emoji_render_opts=opts)
    assert 'src="https://example.com/joy.png"' in html


def test_emoji_template_name():
    """Template with {name} should use emoji name."""
    template = "{name} emoji"
    opts = mordant.PyEmojiHtmlRendererOptions(template=template)
    html = mordant.markdown_to_html("I'm :joy:", emoji_render_opts=opts)
    assert "joy" in html


def test_emoji_template_emoji():
    """Template with {emoji} should use emoji character."""
    template = "Emoji: {emoji}"
    opts = mordant.PyEmojiHtmlRendererOptions(template=template)
    html = mordant.markdown_to_html("I'm :joy:", emoji_render_opts=opts)
    assert "\U0001F602" in html


def test_emoji_template_unknown_placeholder():
    """Template with unknown placeholder should pass through unchanged."""
    template = "{unknown} {shortcode}"
    opts = mordant.PyEmojiHtmlRendererOptions(template=template)
    html = mordant.markdown_to_html("I'm :joy:", emoji_render_opts=opts)
    assert "{unknown}" in html
    assert "joy" in html


# === Emoji Node Access via AST ===

def test_emoji_node_emoji_property():
    """Emoji nodes should expose the emoji property (Unicode char)."""
    doc = mordant.parse(":joy:")
    walker = doc.walk("depth")
    for node in walker:
        emoji_char = node.emoji
        if emoji_char is not None:
            assert emoji_char == "\U0001F602"
            break


def test_emoji_node_shortcode_property():
    """Emoji nodes should expose the shortcode property."""
    doc = mordant.parse(":joy:")
    walker = doc.walk("depth")
    for node in walker:
        shortcode = node.shortcode
        if shortcode is not None:
            assert shortcode == "joy"
            break


def test_emoji_node_name_property():
    """Emoji nodes should expose the name property."""
    doc = mordant.parse(":smile:")
    walker = doc.walk("depth")
    for node in walker:
        name = node.name
        if name is not None:
            # The name is "grinning face with smiling eyes" which contains "smile"
            assert "smile" in name.lower() or "smiling" in name.lower()
            break


def test_emoji_node_properties_non_emoji():
    """Non-emoji nodes should return None for emoji properties."""
    doc = mordant.parse("Hello world")
    walker = doc.walk("depth")
    for node in walker:
        assert node.emoji is None
        assert node.shortcode is None
        assert node.name is None
        break


# === Integration Tests ===

def test_emoji_with_frontmatter():
    """Emojis should work alongside YAML frontmatter."""
    md = "---\ntitle: Test\n---\n\nBody :joy:"
    html = mordant.markdown_to_html(md)
    assert "\U0001F602" in html  # emoji in body


def test_emoji_with_gfm():
    """Emojis should work with GFM extensions."""
    html = mordant.markdown_to_html(":joy: ~~deleted~~", gfm_opts=mordant.GfmOptions.all())
    assert "\U0001F602" in html
    assert "<del>" in html


def test_emoji_with_attributes():
    """Emojis should work with attribute lists."""
    opts = mordant.ParseOptions(attributes=True)
    md = ":joy: {.custom}"
    html = mordant.markdown_to_html(md, parse_opts=opts)
    assert "\U0001F602" in html


def test_emoji_with_auto_heading_ids():
    """Emojis should work with auto heading IDs."""
    opts = mordant.ParseOptions(auto_heading_ids=True)
    md = ":joy: Heading"
    html = mordant.markdown_to_html(md, parse_opts=opts)
    assert "\U0001F602" in html


def test_emoji_empty_string():
    """Empty string should not crash."""
    html = mordant.markdown_to_html("")
    assert "\U0001F602" not in html


def test_emoji_no_colon():
    """Text without colons should pass through."""
    html = mordant.markdown_to_html("Hello world")
    assert "Hello world" in html


def test_emoji_partial_colon():
    """Partial colons should pass through."""
    html = mordant.markdown_to_html(":joy")
    assert ":joy" in html


def test_emoji_reverse_colon():
    """Reverse colons should pass through."""
    html = mordant.markdown_to_html("joy:")
    assert "joy:" in html


def test_emoji_mixed_valid_invalid():
    """Mixed valid and invalid shortcodes."""
    html = mordant.markdown_to_html(":joy: :invalid: :heart:")
    assert "\U0001F602" in html  # valid
    assert ":invalid:" in html  # invalid passes through
    assert "\u2764\ufe0f" in html  # valid
