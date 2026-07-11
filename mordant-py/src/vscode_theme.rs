//! VSCode JSON theme converter for syntect.
//!
//! Parses Visual Studio Code color theme JSON files and converts them
//! to syntect Theme objects. Supports JSONC (JSON with comments).
//!
//! This module is original and does not copy any GPL-licensed code.

use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;
use syntect::highlighting::{
    Color, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};

// ---------------------------------------------------------------------------
// VSCode theme data structures
// ---------------------------------------------------------------------------

/// A single token color rule from a VSCode theme.
#[derive(Debug, Deserialize)]
pub struct VscodeTokenColor {
    pub scope: Option<VscodeScope>,
    pub settings: VscodeTokenSettings,
}

/// Scope selector(s) from a VSCode theme.
/// Can be a single string or a comma-separated list.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum VscodeScope {
    Single(String),
    Multiple(Vec<String>),
}

impl VscodeScope {
    /// Convert to syntect ScopeSelectors.
    pub fn to_scope_selectors(&self) -> Result<ScopeSelectors, String> {
        match self {
            VscodeScope::Single(s) => ScopeSelectors::from_str(s).map_err(|e| e.to_string()),
            VscodeScope::Multiple(v) => ScopeSelectors::from_str(&v.join(",")).map_err(|e| e.to_string()),
        }
    }
}

/// Settings for a token color rule.
#[derive(Debug, Deserialize)]
pub struct VscodeTokenSettings {
    pub foreground: Option<String>,
    pub background: Option<String>,
    #[serde(rename = "fontStyle")]
    pub font_style: Option<String>,
}

impl Default for VscodeTokenSettings {
    fn default() -> Self {
        Self {
            foreground: None,
            background: None,
            font_style: None,
        }
    }
}

/// A VSCode color theme file structure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VscodeTheme {
    pub name: Option<String>,
    pub author: Option<String>,
    #[serde(rename = "type")]
    pub theme_type: Option<String>,
    #[serde(default)]
    pub colors: HashMap<String, Option<String>>,
    #[serde(rename = "tokenColors")]
    pub token_colors: Vec<VscodeTokenColor>,
}

// ---------------------------------------------------------------------------
// Color name resolution
// ---------------------------------------------------------------------------

/// Resolve a color string to a syntect Color.
/// Handles hex colors (#RRGGBB) and named colors.
pub fn resolve_color(color_str: &str) -> Option<Color> {
    let trimmed = color_str.trim();
    
    // Try hex color first
    if let Ok(color) = hex_to_color(trimmed) {
        return Some(color);
    }
    
    // Try named color lookup
    if let Some(color) = resolve_named_color(trimmed) {
        return Some(color);
    }
    
    None
}

/// Parse a hex color string to a syntect Color.
fn hex_to_color(s: &str) -> Result<Color, ()> {
    let s = s.trim();
    
    // Handle #RRGGBB format
    if s.starts_with('#') && s.len() == 7 {
        let r = u8::from_str_radix(&s[1..3], 16).map_err(|_| ())?;
        let g = u8::from_str_radix(&s[3..5], 16).map_err(|_| ())?;
        let b = u8::from_str_radix(&s[5..7], 16).map_err(|_| ())?;
        return Ok(Color { r, g, b, a: 255 });
    }
    
    // Handle RGB(r, g, b) format
    if s.starts_with("rgb(") && s.ends_with(')') {
        let inner = &s[4..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().map_err(|_| ())?;
            let g = parts[1].parse::<u8>().map_err(|_| ())?;
            let b = parts[2].parse::<u8>().map_err(|_| ())?;
            return Ok(Color { r, g, b, a: 255 });
        }
    }
    
    Err(())
}

/// Named color lookup table.
/// Maps common color names to their hex values.
fn resolve_named_color(name: &str) -> Option<Color> {
    let normalized = name.trim().to_lowercase();
    
    match normalized.as_str() {
        // Basic colors
        "black" => Some(Color { r: 0, g: 0, b: 0, a: 255 }),
        "white" => Some(Color { r: 255, g: 255, b: 255, a: 255 }),
        "red" => Some(Color { r: 255, g: 0, b: 0, a: 255 }),
        "green" => Some(Color { r: 0, g: 128, b: 0, a: 255 }),
        "blue" => Some(Color { r: 0, g: 0, b: 255, a: 255 }),
        "yellow" => Some(Color { r: 255, g: 255, b: 0, a: 255 }),
        "cyan" => Some(Color { r: 0, g: 255, b: 255, a: 255 }),
        "magenta" => Some(Color { r: 255, g: 0, b: 255, a: 255 }),
        "orange" => Some(Color { r: 255, g: 165, b: 0, a: 255 }),
        "pink" => Some(Color { r: 255, g: 192, b: 192, a: 255 }),
        "purple" => Some(Color { r: 128, g: 0, b: 128, a: 255 }),
        "brown" => Some(Color { r: 165, g: 42, b: 42, a: 255 }),
        "gray" | "grey" => Some(Color { r: 128, g: 128, b: 128, a: 255 }),
        "silver" => Some(Color { r: 192, g: 192, b: 192, a: 255 }),
        "teal" => Some(Color { r: 0, g: 128, b: 128, a: 255 }),
        "aqua" => Some(Color { r: 0, g: 255, b: 255, a: 255 }),
        "fuchsia" => Some(Color { r: 255, g: 0, b: 255, a: 255 }),
        "lime" => Some(Color { r: 0, g: 255, b: 0, a: 255 }),
        "olive" => Some(Color { r: 128, g: 128, b: 0, a: 255 }),
        "indigo" => Some(Color { r: 75, g: 75, b: 238, a: 255 }),
        "violet" => Some(Color { r: 238, g: 130, b: 238, a: 255 }),
        "coral" => Some(Color { r: 255, g: 127, b: 80, a: 255 }),
        "salmon" => Some(Color { r: 250, g: 128, b: 128, a: 255 }),
        "khaki" => Some(Color { r: 188, g: 143, b: 78, a: 255 }),
        "tan" => Some(Color { r: 210, g: 180, b: 140, a: 255 }),
        "beige" => Some(Color { r: 245, g: 245, b: 220, a: 255 }),
        "cream" => Some(Color { r: 255, g: 253, b: 215, a: 255 }),
        "ivory" => Some(Color { r: 255, g: 255, b: 240, a: 255 }),
        "linen" => Some(Color { r: 250, g: 240, b: 230, a: 255 }),
        "lace" => Some(Color { r: 255, g: 250, b: 240, a: 255 }),
        "snow" => Some(Color { r: 255, g: 250, b: 250, a: 255 }),
        "mint" => Some(Color { r: 189, g: 245, b: 239, a: 255 }),
        "sage" => Some(Color { r: 165, g: 188, b: 165, a: 255 }),
        "gold" => Some(Color { r: 255, g: 215, b: 0, a: 255 }),
        "bronze" => Some(Color { r: 205, g: 127, b: 53, a: 255 }),
        "copper" => Some(Color { r: 184, g: 115, b: 67, a: 255 }),
        "brass" => Some(Color { r: 184, g: 152, b: 32, a: 255 }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Theme conversion
// ---------------------------------------------------------------------------

/// Convert a VSCode theme to a syntect Theme.
pub fn vscode_theme_to_syntect(vscode: &VscodeTheme) -> Result<Theme, String> {
    let mut settings = ThemeSettings::default();
    
    // Map editor colors to theme settings
    for (key, value) in &vscode.colors {
        if value.is_none() {
            continue;
        }
        let color_val = value.as_ref().unwrap();
        let color = resolve_color(color_val);
        
        match key.as_str() {
            "editor.background" => settings.background = color,
            "editor.foreground" => {
                settings.foreground = color;
                settings.caret = color;
            }
            "foreground" => settings.foreground = color,
            "editorCursor.background" => settings.caret = color,
            "editor.lineHighlightBackground" => settings.line_highlight = color,
            "list.highlightForeground" => {
                settings.find_highlight_foreground = color;
                settings.accent = color;
            }
            "editorGutter.background" => settings.gutter = color,
            "editorLineNumber.foreground" => settings.gutter_foreground = color,
            "editor.selectionBackground" => settings.selection = color,
            "list.inactiveSelectionBackground" => settings.inactive_selection = color,
            "list.inactiveSelectionForeground" => settings.inactive_selection_foreground = color,
            "editor.findMatchBackground" | "peekViewEditor.matchHighlightBorder" => {
                settings.highlight = color;
                settings.find_highlight = color;
            }
            "editorIndentGuide.background" => settings.guide = color,
            "breadcrumb.activeSelectionForeground" => settings.active_guide = color,
            "breadcrumb.foreground" => settings.stack_guide = color,
            "selection.background" => {
                settings.tags_foreground = color;
                settings.brackets_foreground = color;
            }
            "widget.shadow" | "scrollbar.shadow" => settings.shadow = color,
            _ => {}
        }
    }
    
    // Convert token colors to theme items
    let mut scopes: Vec<ThemeItem> = Vec::new();
    for token_color in &vscode.token_colors {
        let scope = if let Some(s) = &token_color.scope {
            s.to_scope_selectors()?
        } else {
            ScopeSelectors::from_str("*").map_err(|e| e.to_string())?
        };
        
        let style = StyleModifier {
            foreground: token_color.settings.foreground.as_ref().and_then(|s| resolve_color(s)),
            background: token_color.settings.background.as_ref().and_then(|s| resolve_color(s)),
            font_style: token_color.settings.font_style.as_ref().map(|s| parse_font_style(s)).unwrap_or(None),
        };
        
        scopes.push(ThemeItem { scope, style });
    }
    
    Ok(Theme {
        name: vscode.name.clone(),
        author: vscode.author.clone(),
        scopes,
        settings,
    })
}

/// Parse font style string to FontStyle.
///
/// Must match syntect 5.x's `FontStyle` bit layout:
/// `BOLD = 1`, `UNDERLINE = 2`, `ITALIC = 4`.
/// (syntect 5.3.0 has no `strikethrough` bit.)
fn parse_font_style(s: &str) -> Option<FontStyle> {
    let s = s.trim().to_lowercase();
    let mut flags: u32 = 0;

    for part in s.split(',') {
        let part = part.trim();
        match part {
            "bold" => flags |= 1 << 0,                                  // BOLD = 1
            "underline" | "underlined" => flags |= 1 << 1,             // UNDERLINE = 2
            "italic" => flags |= 1 << 2,                               // ITALIC = 4
            _ => {}
        }
    }
    
    if flags == 0 {
        None
    } else {
        FontStyle::from_bits((flags & 0xFF) as u8)
    }
}

// ---------------------------------------------------------------------------
// JSONC parsing
// ---------------------------------------------------------------------------

/// Parse a VSCode theme from a JSONC string (JSON with comments).
pub fn parse_vscode_theme_jsonc(jsonc_str: &str) -> Result<VscodeTheme, String> {
    // Use jsonc-parser's serde support to parse directly to serde_json::Value
    let serde_value = jsonc_parser::parse_to_serde_value(jsonc_str, &jsonc_parser::ParseOptions::default())
        .map_err(|e| format!("JSONC parse error: {}", e))?;
    
    serde_json::from_value(serde_value)
        .map_err(|e| format!("Theme parse error: {}", e))
}

/// Detect if a string contains a VSCode JSON theme (vs plist XML).
pub fn is_vscode_json_theme(content: &str) -> bool {
    let trimmed = content.trim();
    // VSCode themes start with { and contain "tokenColors"
    trimmed.starts_with('{') && trimmed.contains("\"tokenColors\"")
}

/// Detect if a string contains a plist XML theme.
pub fn is_plist_xml_theme(content: &str) -> bool {
    let trimmed = content.trim();
    trimmed.starts_with("<?xml") || trimmed.starts_with("<plist")
}
