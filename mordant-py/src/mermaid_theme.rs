//! Derive Mermaid color schemes from syntect (code-highlighting) themes.
//!
//! A syntect `Theme` is a *text-on-canvas* model (background, foreground, and a
//! palette of scope colors). A Mermaid `Theme` is a *diagram* model (node fills,
//! edge colors, pie slices, …) that does not exist in a code theme. This module
//! bridges the two:
//!
//! - canvas/text colors come from `Theme.settings` (`background`/`foreground`/
//!   `line_highlight`/`selection`),
//! - node/edge/pie accents come from the scope palette (`theme.scopes`),
//! - every generated fill gets a contrast-correct text color, so light *and*
//!   dark themes render readably.

use std::collections::HashMap;
use syntect::highlighting::{Color, Theme, ThemeItem};

/// A fully resolved Mermaid color scheme: the `mermaid-rs-renderer` theme for
/// server-side SVG, the `themeVariables` map for client-side mermaid.js, and a
/// dark-mode hint.
pub struct MermaidColorScheme {
    pub rs_theme: mermaid_rs_renderer::theme::Theme,
    pub client_vars: HashMap<String, String>,
    #[allow(dead_code)]
    pub is_dark: bool,
}

/// How a `theme` name resolves.
#[derive(Debug, Clone)]
pub enum MermaidThemeSpec {
    /// A built-in mermaid theme (modern/dark/forest/neutral/default/...). Used
    /// natively — no derivation. `name` is kept for the client `theme:` value.
    Native { theme: mermaid_rs_renderer::theme::Theme, name: String },
    /// A code-highlighting (syntect) theme — derive a custom "base" theme.
    Derived(Theme),
    /// No theme supplied — legacy behavior.
    None,
}

/// Resolve a theme name to a [`MermaidThemeSpec`].
///
/// 1. Built-in mermaid presets via `Theme::from_name` (native, no derivation).
/// 2. Otherwise a syntect code-highlighting theme (derived).
/// 3. Otherwise `None` (legacy fallback).
pub fn resolve_mermaid_theme(name: &str) -> MermaidThemeSpec {
    if let Some(t) = mermaid_rs_renderer::theme::Theme::from_name(name) {
        return MermaidThemeSpec::Native {
            theme: t,
            name: name.to_string(),
        };
    }
    if let Some(t) = crate::highlighter::resolve_theme(name) {
        return MermaidThemeSpec::Derived(t);
    }
    MermaidThemeSpec::None
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

fn to_hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn srgb_to_linear(u: f32) -> f32 {
    let u = u / 255.0;
    if u <= 0.04045 {
        u / 12.92
    } else {
        ((u + 0.055) / 1.055).powf(2.4)
    }
}

/// Relative luminance (0..1) of a color.
fn luminance(c: Color) -> f32 {
    let r = srgb_to_linear(c.r as f32);
    let g = srgb_to_linear(c.g as f32);
    let b = srgb_to_linear(c.b as f32);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Black or white text, whichever is more readable on `c`.
fn contrast_text(c: Color) -> String {
    if luminance(c) < 0.5 {
        "#ffffff".to_string()
    } else {
        "#000000".to_string()
    }
}

fn is_dark(c: Color) -> bool {
    luminance(c) < 0.5
}

/// Euclidean-ish distance between two colors (for "is this an accent?" tests).
fn color_dist(a: Color, b: Color) -> f32 {
    let dr = a.r as f32 - b.r as f32;
    let dg = a.g as f32 - b.g as f32;
    let db = a.b as f32 - b.b as f32;
    (dr * dr + dg * dg + db * db).sqrt()
}

fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).ok()?;
        let g = u8::from_str_radix(&h[2..4], 16).ok()?;
        let b = u8::from_str_radix(&h[4..6], 16).ok()?;
        Some((r, g, b))
    } else {
        None
    }
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let mut h = 0.0;
    let mut s = 0.0;
    if (max - min).abs() > 1e-6 {
        let d = max - min;
        s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h /= 6.0;
    }
    (h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = h - h.floor(); // wrap into 0..1
    let rgb = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        let t = |t: f32| -> f32 {
            let mut t = t - t.floor();
            if t < 0.0 {
                t += 1.0;
            }
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        (t(h + 1.0 / 3.0), t(h), t(h - 1.0 / 3.0))
    };
    (
        (rgb.0 * 255.0).round() as u8,
        (rgb.1 * 255.0).round() as u8,
        (rgb.2 * 255.0).round() as u8,
    )
}

/// Shift a hex color's lightness by `dl` (-1..1). Positive lightens, negative darkens.
fn lighten_darken(hex: &str, dl: f32) -> String {
    if let Some((r, g, b)) = hex_to_rgb(hex) {
        let (h, s, l) = rgb_to_hsl(r, g, b);
        let nl = (l + dl).clamp(0.0, 1.0);
        let (r, g, b) = hsl_to_rgb(h, s, nl);
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    } else {
        hex.to_string()
    }
}

/// Fill an `[String; N]` by cycling `accents`; if empty, generate a hue ramp.
fn fill_array<const N: usize>(accents: &[String], dark: bool) -> [String; N] {
    let mut out: [String; N] = std::array::from_fn(|_| String::new());
    if accents.is_empty() {
        for (i, slot) in out.iter_mut().enumerate() {
            let hue = i as f32 / N as f32;
            let (r, g, b) = hsl_to_rgb(hue, 0.65, if dark { 0.6 } else { 0.5 });
            *slot = format!("#{:02x}{:02x}{:02x}", r, g, b);
        }
    } else {
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = accents[i % accents.len()].clone();
        }
    }
    out
}

/// First color in `entries` whose scope text contains any of `keys`.
fn find_color(entries: &[(String, Color)], keys: &[&str]) -> Option<Color> {
    entries
        .iter()
        .find(|(t, _)| {
            let tl = t.to_lowercase();
            keys.iter().any(|k| tl.contains(k))
        })
        .map(|(_, c)| *c)
}

fn pick_hex(opt: Option<Color>, fallback: String) -> String {
    match opt {
        Some(c) => to_hex(c),
        None => fallback,
    }
}

// ---------------------------------------------------------------------------
// Derivation
// ---------------------------------------------------------------------------

/// Derive a Mermaid color scheme from a syntect code-highlighting theme.
///
/// See `MERMAID_THEME_PLAN.md` §4 for the full mapping. In short:
/// - canvas/text from `settings.background/foreground/line_highlight/selection`,
/// - node fills = `line_highlight`/`selection` mid-tone (or a shade of bg),
/// - edges/borders/pie/git = the vivid scope palette,
/// - every fill gets contrast-correct text.
pub fn derive_mermaid_theme(syn: &Theme) -> MermaidColorScheme {
    let s = &syn.settings;
    let bg_color = s
        .background
        .unwrap_or(Color { r: 0xff, g: 0xff, b: 0xff, a: 0xff });
    let fg_color = s
        .foreground
        .unwrap_or(Color { r: 0x00, g: 0x00, b: 0x00, a: 0xff });
    let bg = to_hex(bg_color);
    let fg = to_hex(fg_color);
    let dark = is_dark(bg_color);

    // Node fill: prefer a mid-tone (current-line / selection); else shade the bg.
    let primary_fill_color = s.line_highlight.or(s.selection).unwrap_or(bg_color);
    let primary_fill = to_hex(primary_fill_color);
    let primary_text = contrast_text(primary_fill_color);

    // Collect (scope-text, color) pairs from the theme's scope palette.
    let mut entries: Vec<(String, Color)> = syn
        .scopes
        .iter()
        .filter_map(|item: &ThemeItem| {
            item.style.foreground.map(|c| {
                // Best-effort scope name. `ScopeSelectors` wraps a `Vec<ScopeSelector>`;
                // each may resolve to a single `Scope` (multi-scope selectors yield
                // None and are still used as generic accents below).
                let name = item
                    .scope
                    .selectors
                    .iter()
                    .filter_map(|sel| sel.extract_single_scope())
                    .map(|sc| sc.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                (name, c)
            })
        })
        .collect();
    // De-duplicate by (scope-text, hex) keeping first occurrence.
    let mut seen = std::collections::HashSet::new();
    entries.retain(|(t, c)| seen.insert((t.clone(), to_hex(*c))));

    // Semantic accent roles (prefer when present).
    let keyword = find_color(&entries, &["keyword", "storage"]);
    let string_c = find_color(&entries, &["string"]);
    let function_c = find_color(&entries, &["entity.name.function"]);
    let type_c = find_color(
        &entries,
        &[
            "entity.name.type",
            "entity.name.class",
            "support.type",
            "storage.type",
        ],
    );
    let constant_c = find_color(&entries, &["constant"]);

    // Vivid accent list (exclude colors too close to bg/fg), sorted by hue.
    let mut accents: Vec<(Color, String)> = entries
        .iter()
        .filter(|(_, c)| color_dist(*c, bg_color) > 40.0 && color_dist(*c, fg_color) > 40.0)
        .map(|(_, c)| (*c, to_hex(*c)))
        .collect();
    accents.sort_by(|a, b| {
        let ha = rgb_to_hsl(a.0.r, a.0.g, a.0.b).0;
        let hb = rgb_to_hsl(b.0.r, b.0.g, b.0.b).0;
        ha.partial_cmp(&hb).unwrap()
    });
    let accent_hex: Vec<String> = accents.iter().map(|(_, h)| h.clone()).collect();
    let first_accent = accents.first().map(|(c, _)| *c);
    let second_accent = accents.get(1).map(|(c, _)| *c);
    let third_accent = accents.get(2).map(|(c, _)| *c);

    // Assign roles with sensible fallbacks (accent color → shaded fill → fg).
    let line_hex = pick_hex(
        keyword.or(function_c).or(type_c).or(first_accent),
        fg.clone(),
    );
    let border_hex = pick_hex(keyword.or(type_c).or(second_accent), fg.clone());
    let secondary_hex = pick_hex(
        string_c.or(function_c).or(first_accent),
        lighten_darken(&primary_fill, 0.12),
    );
    let tertiary_hex = pick_hex(
        type_c.or(constant_c).or(third_accent),
        lighten_darken(&primary_fill, -0.12),
    );

    // Cluster / edge-label backgrounds.
    let cluster_bg = s
        .line_highlight
        .or(s.selection)
        .map(to_hex)
        .unwrap_or_else(|| lighten_darken(&bg, if dark { 0.08 } else { -0.05 }));
    let cluster_border = to_hex(fg_color);
    let edge_label_bg = s
        .selection
        .or(s.line_highlight)
        .map(to_hex)
        .unwrap_or_else(|| lighten_darken(&bg, if dark { 0.05 } else { -0.03 }));

    // Build the server-side theme (start from `modern` and override).
    let mut t = mermaid_rs_renderer::theme::Theme::modern();
    t.background = bg.clone();
    t.text_color = fg.clone();
    t.primary_color = primary_fill.clone();
    t.primary_text_color = primary_text.clone();
    t.primary_border_color = border_hex.clone();
    t.line_color = line_hex.clone();
    t.secondary_color = secondary_hex.clone();
    t.tertiary_color = tertiary_hex.clone();
    t.cluster_background = cluster_bg.clone();
    t.cluster_border = cluster_border.clone();
    t.edge_label_background = edge_label_bg.clone();
    t.sequence_actor_fill = primary_fill.clone();
    t.sequence_actor_border = border_hex.clone();
    t.sequence_actor_line = line_hex.clone();
    t.sequence_note_fill = secondary_hex.clone();
    t.sequence_note_border = border_hex.clone();
    t.sequence_activation_fill = cluster_bg.clone();
    t.sequence_activation_border = border_hex.clone();
    t.pie_colors = fill_array::<12>(&accent_hex, dark);
    t.git_colors = fill_array::<8>(&accent_hex, dark);
    t.pie_title_text_color = fg.clone();
    t.pie_section_text_color = fg.clone();
    t.pie_legend_text_color = fg.clone();

    // Client-side themeVariables (mermaid.js `base` + themeVariables).
    let mut vars = HashMap::new();
    vars.insert("background".to_string(), bg.clone());
    vars.insert("textColor".to_string(), fg.clone());
    vars.insert("primaryColor".to_string(), primary_fill.clone());
    vars.insert("primaryTextColor".to_string(), primary_text.clone());
    vars.insert("primaryBorderColor".to_string(), border_hex.clone());
    vars.insert("lineColor".to_string(), line_hex.clone());
    vars.insert("secondaryColor".to_string(), secondary_hex.clone());
    vars.insert("tertiaryColor".to_string(), tertiary_hex.clone());
    vars.insert("clusterBkg".to_string(), cluster_bg.clone());
    vars.insert("clusterBorder".to_string(), cluster_border.clone());
    vars.insert("edgeLabelBackground".to_string(), edge_label_bg.clone());

    MermaidColorScheme {
        rs_theme: t,
        client_vars: vars,
        is_dark: dark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_scheme_structure_and_contrast() {
        // Iterate all registered (default) syntect themes — no filesystem needed.
        let themes = crate::highlighter::list_themes();
        assert!(!themes.is_empty());
        for name in &themes {
            let Some(t) = crate::highlighter::resolve_theme(name) else {
                continue;
            };
            let scheme = derive_mermaid_theme(&t);
            // Structural invariants for every derived theme.
            assert_eq!(scheme.rs_theme.pie_colors.len(), 12);
            assert_eq!(scheme.rs_theme.git_colors.len(), 8);
            assert!(scheme.rs_theme.background.starts_with('#'));
            assert!(scheme.client_vars.contains_key("primaryColor"));
            assert!(scheme.client_vars.contains_key("lineColor"));
            assert!(scheme.client_vars.contains_key("background"));
            // Node text must be contrast-correct vs the node fill (readable in
            // both light and dark themes).
            let fill = hex_to_rgb(&scheme.rs_theme.primary_color)
                .map(|(r, g, b)| Color { r, g, b, a: 255 })
                .unwrap();
            assert_eq!(scheme.rs_theme.primary_text_color, contrast_text(fill));
        }
    }

    #[test]
    fn native_names_resolve() {
        assert!(matches!(
            resolve_mermaid_theme("dark"),
            MermaidThemeSpec::Native { .. }
        ));
        assert!(matches!(
            resolve_mermaid_theme("forest"),
            MermaidThemeSpec::Native { .. }
        ));
    }

    #[test]
    fn unknown_name_is_none() {
        assert!(matches!(
            resolve_mermaid_theme("no-such-theme-xyz"),
            MermaidThemeSpec::None
        ));
    }
}
