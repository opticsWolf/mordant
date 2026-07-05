//! Code highlighting extension for mordant.
//!
//! Provides syntax highlighting for code blocks using syntect (VS Code's
//! highlighting engine). Supports custom themes loaded at runtime.
//!
//! Uses syntect-assets to provide bat's updated syntaxes and themes.
//! Embedded themes are loaded from the package's themes/ directory by Python.

use pyo3::prelude::*;
use rushdown_lib::ast::{Arena, CodeBlock, NodeRef, WalkStatus};
use rushdown_lib::renderer::{self, html, NodeRenderer, RendererOptions, RenderNode, TextWrite, NodeRendererRegistry, BoxRenderNode};
use rushdown_lib::{as_kind_data, Result};
use std::any::TypeId;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::html::{ClassedHTMLGenerator, ClassStyle, IncludeBackground, styled_line_to_highlighted_html};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use syntect_assets::assets::HighlightingAssets;

use crate::vscode_theme::{parse_vscode_theme_jsonc, is_vscode_json_theme, is_plist_xml_theme};

// ---------------------------------------------------------------------------
// Global resources (lazy-loaded, thread-safe)
// ---------------------------------------------------------------------------

// Use syntect-assets for bat's updated syntaxes/themes
// Wrapped in Mutex because HighlightingAssets contains unsync::OnceCell (not Sync)
static ASSETS: LazyLock<std::sync::Arc<std::sync::Mutex<HighlightingAssets>>> = LazyLock::new(|| {
    let mut assets = HighlightingAssets::from_binary();
    // Set default theme
    assets.set_fallback_theme("Monokai Extended");
    std::sync::Arc::new(std::sync::Mutex::new(assets))
});

// Syntax set from syntect-assets (bat's updated syntaxes)
static SYNTAX_SET: LazyLock<std::sync::Arc<SyntaxSet>> = LazyLock::new(|| {
    let assets = ASSETS.lock().unwrap();
    let ss = match assets.get_syntax_set() {
        Ok(s) => s.clone(),
        Err(_) => SyntaxSet::load_defaults_newlines(),
    };
    std::sync::Arc::new(ss)
});

// Theme set - start with default themes and load custom themes
static THEME_SET: LazyLock<RwLock<ThemeSet>> =
    LazyLock::new(|| RwLock::new(ThemeSet::load_defaults()));

// ---------------------------------------------------------------------------
// Python-exposed enums
// ---------------------------------------------------------------------------

/// The highlighting mode exposed to Python.
#[pyclass(module = "mordant", name = "HighlightingMode", skip_from_py_object)]
pub enum PyHighlightingMode {
    /// Inline style attributes (default).
    Attribute,
    /// CSS class attributes.
    Class,
}

// ---------------------------------------------------------------------------
// Rust internal types
// ---------------------------------------------------------------------------

/// Internal highlighting mode (no PyO3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HighlightingMode {
    #[default]
    Attribute,
    Class,
}

/// Options for the highlighting renderer.
#[derive(Debug, Clone)]
pub struct HighlightingRendererOptions {
    pub theme: String,
    pub mode: HighlightingMode,
}

impl Default for HighlightingRendererOptions {
    fn default() -> Self {
        Self {
            theme: "InspiredGitHub".to_string(),
            mode: HighlightingMode::Attribute,
        }
    }
}

impl RendererOptions for HighlightingRendererOptions {}

// ---------------------------------------------------------------------------
// PyHighlighter class
// ---------------------------------------------------------------------------

/// Python-exposed syntax highlighter.
///
/// # Example
/// ```python
/// hl = mordant.Highlighter(theme="InspiredGitHub", mode="Attribute")
/// html = hl.highlight("rust", "let x = 1;")
/// ```
#[pyclass(module = "mordant", name = "Highlighter", skip_from_py_object)]
pub struct PyHighlighter {
    theme: String,
    mode: HighlightingMode,
}

#[pymethods]
impl PyHighlighter {
    #[new]
    #[pyo3(signature = (theme = "InspiredGitHub", mode = "Attribute"))]
    fn new(theme: &str, mode: &str) -> PyResult<Self> {
        let highlighting_mode = match mode {
            "Attribute" => HighlightingMode::Attribute,
            "Class" => HighlightingMode::Class,
            _ => return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Invalid mode '{}'. Must be 'Attribute' or 'Class'.", mode
            ))),
        };
        Ok(Self {
            theme: theme.to_string(),
            mode: highlighting_mode,
        })
    }

    /// Highlight a code snippet and return HTML.
    ///
    /// # Arguments
    /// * `language` - Language identifier (e.g. "rust", "python")
    /// * `code` - Source code to highlight
    ///
    /// # Returns
    /// HTML string with syntax highlighting
    ///
    /// # Example
    /// ```python
    /// hl = mordant.Highlighter()
    /// html = hl.highlight("rust", "let x = 1;")
    /// ```
    fn highlight(&self, language: &str, code: &str) -> PyResult<String> {
        let result = highlight_code(language, code, &self.theme, &self.mode);
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Standalone highlight function
// ---------------------------------------------------------------------------

/// Detect language from code content when no language is specified.
/// This acts as a robust fallback cascade when extensions or shebangs are missing.
pub fn detect_syntax_from_content(ps: &SyntaxSet, code: &str) -> String {
    // 1. Try first-line detection (shebangs, Emacs modelines, etc.)
    let first_line = code.lines().next().unwrap_or("");
    if let Some(syntax) = ps.find_syntax_by_first_line(first_line) {
        let name = syntax.name.to_lowercase().replace(' ', "-");
        // Normalize syntect names: "bourne", "bourne-again-shell", "bourne-again-shell-(bash)" → "bash"
        if name == "bourne" || name == "bourne-again-shell" || name.starts_with("bourne-again-shell-") || name == "bash" {
            return "bash".to_string();
        }
        // Syntect incorrectly maps `<!DOCTYPE html>` to Svelte via first-line matching.
        // Guard: if syntect says "svelte" but content lacks Svelte markers, skip to heuristics.
        if name == "svelte" {
            let snippet_lower = code.to_lowercase();
            if !(snippet_lower.contains("<script") && snippet_lower.contains("</style>")) {
                // Not actually Svelte, fall through to heuristics
            } else {
                return name;
            }
        } else {
            return name;
        }
    }

    // 2. Try token/name matching on the first line
    let trimmed = first_line.trim();
    if let Some(syntax) = ps.find_syntax_by_token(trimmed) {
        return syntax.name.to_lowercase().replace(' ', "-");
    }

    // 3. Try extension matching (if the first line happens to be a filename)
    if let Some(dot_pos) = trimmed.rfind('.') {
        let ext = &trimmed[dot_pos + 1..];
        if let Some(syntax) = ps.find_syntax_by_extension(ext) {
            return syntax.name.to_lowercase().replace(' ', "-");
        }
    }

    // --- Expanded Content Heuristics ---
    // OPTIMIZATION: Take only the first ~4KB (4096 chars) to prevent massive memory
    // allocations and latency spikes on huge files (e.g., 50MB log files or minified JS).
    let snippet: String = code.chars().take(4096).collect();
    let lower = snippet.to_lowercase();
    let trimmed_lower = lower.trim_start();

    // 1. Unambiguous / Highly Specific Markers & Configs
    if lower.starts_with("diff --git") || (lower.contains("--- ") && lower.contains("+++ ") && lower.contains("@@ ")) { return "diff".to_string(); }
    if lower.contains("<?php") { return "php".to_string(); }
    if lower.contains("<?xml") { return "xml".to_string(); }
    if lower.starts_with("---") && lower.contains(":") && !lower.contains("{") && !lower.contains("(") { return "yaml".to_string(); }
    if (lower.contains("{") && lower.contains("}")) && (lower.contains("\": \"") || lower.contains("\": {") || lower.contains("\": [")) { return "json".to_string(); }

    // 2. Frameworks & Web (Vue/Svelte MUST run before generic HTML — they contain HTML tags)
    if lower.contains("<template") && lower.contains("<script") { return "vue".to_string(); }
    if lower.contains("<script") && lower.contains("</style>") && (lower.contains("export let ") || lower.contains("{")) { return "svelte".to_string(); }
    if lower.contains("<!doctype html") || lower.contains("<html") || lower.contains("<div") || lower.contains("<body") { return "html".to_string(); }
    if lower.contains("type ") && lower.contains(" {") && (lower.contains("query ") || lower.contains("mutation ") || lower.contains("fragment ")) { return "graphql".to_string(); }
    if lower.contains("@media") || lower.contains("margin:") || lower.contains("padding:") || lower.contains("background-color:") || lower.contains("display:") { return "css".to_string(); }

    // 3. Document / Markup
    if lower.contains("\\documentclass") || lower.contains("\\begin{document}") || lower.contains("\\usepackage") { return "latex".to_string(); }
    if trimmed_lower.starts_with("# ") || trimmed_lower.starts_with("## ") || lower.contains("**") || (lower.contains("[") && lower.contains("](")) { return "markdown".to_string(); }
    if lower.contains("h1.") || lower.contains("h2.") || lower.contains("bq.") || lower.contains("|table|") || lower.contains("bc.") { return "textile".to_string(); }
    if lower.contains(".. ") && lower.contains("::") && (lower.contains("toctree") || lower.contains("note::") || lower.contains("warning::")) { return "rst".to_string(); }
    if lower.contains("digraph ") || (lower.contains("graph ") && lower.contains("->") && lower.contains("label=")) || lower.contains("node [") { return "dot".to_string(); }

    // 4. Functional & Data-Oriented (Runs early to protect `def`, `fn`, `let` from JS/Ruby)
    if trimmed_lower.starts_with('(') && (lower.contains("defn ") || lower.contains("ns ") || lower.contains("let [") || lower.contains("println ")) { return "clojure".to_string(); }
    if trimmed_lower.starts_with('(') && (lower.contains("defun ") || lower.contains("lambda ") || lower.contains("let (") || lower.contains("car ") || lower.contains("cdr ")) { return "lisp".to_string(); }
    if lower.contains("-module(") || lower.contains("io:format") || lower.contains("fun(") { return "erlang".to_string(); }
    // Elixir checked before Ruby
    if lower.contains("defmodule ") || (lower.contains("def ") && lower.contains(" do") && lower.contains("end")) { return "elixir".to_string(); }
    // OCaml before Haskell — OCaml uses `module ... = struct` + `;;`
    if lower.contains("module ") && lower.contains("struct") && lower.contains("end") && lower.contains(";;") { return "ocaml".to_string(); }
    // Haskell — must NOT match OCaml (struct/;;)
    if (lower.contains("module ") && lower.contains("where") && lower.contains("::") && !lower.contains("struct")) || lower.contains("main = ") || lower.contains("putstrln") || lower.contains("import data.") { return "haskell".to_string(); }
    if (lower.contains("select ") && lower.contains(" from ")) || lower.contains("insert into ") || lower.contains("create table ") || lower.contains("drop table ") { return "sql".to_string(); }

    // 5. Build Tools & Shell
    if (lower.starts_with("from ") || lower.contains("\nfrom ")) && (lower.contains("cmd ") || lower.contains("run ") || lower.contains("entrypoint ")) { return "dockerfile".to_string(); }
    if lower.contains("cmake_minimum_required") || lower.contains("project(") || lower.contains("add_executable(") { return "cmake".to_string(); }
    if lower.contains(".phony") || lower.contains("$(shell") || lower.contains("all:") || (lower.contains(":") && lower.contains('\t')) || trimmed_lower == "makefile" { return "makefile".to_string(); }
    if lower.contains("write-host") || lower.contains("get-childitem") || lower.contains("param(") || lower.contains("| where-object") { return "powershell".to_string(); }
    if lower.starts_with("@echo off") || lower.contains("setlocal") || lower.contains("errorlevel") { return "bat".to_string(); }
    if lower.contains("echo ") && (lower.contains("fi") || lower.contains("esac") || lower.contains("done") || lower.contains("grep ") || lower.contains("then")) { return "bash".to_string(); }

    // 6. Scientific (Runs before generic JS/Scripting to protect `function`)
    if lower.contains("<- ") && (lower.contains("function") || lower.contains("library(") || lower.contains("data.frame") || lower.contains("c(")) { return "r".to_string(); }
    if lower.contains("function ") && lower.contains("end") && (lower.contains("% ") || lower.contains("disp(") || lower.contains("plot(") || lower.contains("zeros(") || lower.contains("function [")) { return "matlab".to_string(); }
    if lower.contains("function ") && lower.contains("end") && (lower.contains("using ") || lower.contains("println(") || lower.contains("module ")) { return "julia".to_string(); }
    if lower.contains("implicit none") || lower.contains("subroutine ") { return "fortran".to_string(); }

    // 7. Systems & Heavily Typed
    // Zig MUST run before Rust — both use `pub fn` but Zig has `@import`/`!void`
    if lower.contains("const std = @import") || lower.contains("!void") || (lower.contains("pub fn ") && lower.contains("@import")) { return "zig".to_string(); }
    // Rust relaxed to fix `fn` missing strict `impl` blocks
    if lower.contains("fn ") && (lower.contains("impl ") || lower.contains("let ") || lower.contains("pub ") || lower.contains("println!(") || lower.contains("&str") || lower.contains("mut ")) { return "rust".to_string(); }
    if lower.contains("package ") && lower.contains("func ") && lower.contains("import ") && !lower.contains("class ") { return "go".to_string(); }
    // C# MUST run before C/C++ — `using System;` vs `using namespace`
    if lower.contains("using system;") || lower.contains("console.writeline") || (lower.contains("namespace ") && lower.contains("class ")) { return "c#".to_string(); }
    // C++ standalone check (for cases where `#include` isn't the first token)
    if lower.contains("std::") || lower.contains("using namespace") || lower.contains("cout") || lower.contains("iostream") || lower.contains("vector<") || lower.contains("public:") { return "c++".to_string(); }
    if lower.contains("#include <") || lower.contains("#include \"") {
        // C++ strictly separated from C via standard libraries
        if lower.contains("std::") || lower.contains("using namespace") || lower.contains("cout") || lower.contains("iostream") || lower.contains("vector<") || lower.contains("public:") { return "c++".to_string(); }
        if lower.contains("@interface") || lower.contains("@implementation") || lower.contains("nslog") || lower.contains("nsstring") { return "objective-c".to_string(); }
        return "c".to_string();
    }
    if lower.contains("public class ") && (lower.contains("public static void main") || lower.contains("system.out.print")) { return "java".to_string(); }
    if lower.contains("guard let ") || lower.contains("guard var ") || lower.contains("import uikit") || lower.contains("import foundation") { return "swift".to_string(); }
    if lower.contains("fun ") && (lower.contains("val ") || lower.contains("var ") || lower.contains("companion object") || lower.contains("data class")) { return "kotlin".to_string(); }
    if lower.contains("object ") && (lower.contains("case class") || lower.contains("trait ") || lower.contains("def ") || lower.contains("yield") || lower.contains("extends ")) { return "scala".to_string(); }
    if lower.contains("module ") && lower.contains("unittest") && (lower.contains("immutable") || lower.contains("auto ") || lower.contains("writeln")) { return "d".to_string(); }
    if lower.contains("program ") && lower.contains("begin") && lower.contains("end.") && (lower.contains("procedure ") || lower.contains("function ")) { return "pascal".to_string(); }

    // 8. Scripting & Web
    if lower.contains("def ") && lower.contains("end") && (lower.contains("require ") || lower.contains("puts ") || lower.contains("attr_accessor") || lower.contains("do |")) { return "ruby".to_string(); }
    if lower.contains("def ") && lower.contains("println ") && (lower.contains("class ") || lower.contains("import ")) && !lower.contains("end") { return "groovy".to_string(); }
    if lower.contains("def ") && (lower.contains(":") || lower.contains("import ") || lower.contains("print(") || lower.contains("elif ") || lower.contains("if __name__")) { return "python".to_string(); }
    if lower.contains("local ") && lower.contains("function ") && lower.contains("end") { return "lua".to_string(); }
    if lower.contains("use strict;") || lower.contains("my $") || (lower.contains("sub ") && lower.contains("print ")) || lower.contains("=~") { return "perl".to_string(); }
    if lower.contains("import 'package:") || lower.contains("void main()") || lower.contains("widget ") || lower.contains("setstate(") { return "dart".to_string(); }
    if lower.contains("proc ") && lower.contains("set ") && lower.contains("puts ") && lower.contains("$") && lower.contains("expr ") { return "tcl".to_string(); }
    if lower.contains("tell application ") || lower.contains("end tell") || lower.contains("display dialog") { return "applescript".to_string(); }
    // Actionscript relaxed
    if lower.contains("package ") && (lower.contains("import ") || lower.contains("class ")) && (lower.contains("trace(") || lower.contains("var ") || lower.contains("function ")) { return "actionscript".to_string(); }

    // JS/TS checked very last to prevent swallowing MATLAB/Julia/Clojure
    if lower.contains("interface ") || lower.contains("type ") || lower.contains("enum ") || (lower.contains("export ") && lower.contains("class ")) || lower.contains("as const") { return "typescript".to_string(); }
    if lower.contains("function ") || lower.contains("const ") || lower.contains("let ") || lower.contains("console.log") || lower.contains("=>") || lower.contains("require(") || lower.contains("document.getelementbyid") { return "javascript".to_string(); }

    // 9. Regular Expressions (fallback if no other match and heavy metacharacters)
    if !lower.contains(' ') && (lower.contains('^') || lower.contains('$') || lower.contains("\\b") || lower.contains("(?:")) && lower.contains('|') { return "regex".to_string(); }

    // Default fallback if no heuristics match.
    "plaintext".to_string()
}

/// Highlight code using syntect.
fn highlight_code(
    language: &str,
    code: &str,
    theme_name: &str,
    mode: &HighlightingMode,
) -> String {
    let ps = &*SYNTAX_SET;
    let lang: String = if language.is_empty() || language == "plaintext" {
        detect_syntax_from_content(ps, code)
    } else {
        language.to_string()
    };
    let _syntax = ps
        .find_syntax_by_token(&lang)
        .or_else(|| ps.find_syntax_by_extension(&lang))
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let ts = THEME_SET.read().unwrap();
    let theme = ts.themes
        .get(theme_name)
        .unwrap_or_else(|| &ts.themes["InspiredGitHub"]);

    match mode {
        HighlightingMode::Attribute => {
            render_attribute_mode(theme, code, &lang)
        }
        HighlightingMode::Class => {
            render_class_mode(code, &lang)
        }
    }
}

fn render_attribute_mode(
    theme: &syntect::highlighting::Theme,
    code: &str,
    language: &str,
) -> String {
    let ps = &*SYNTAX_SET;
    let lang: String = if language.is_empty() || language == "plaintext" {
        detect_syntax_from_content(ps, code)
    } else {
        language.to_string()
    };
    let syntax = ps
        .find_syntax_by_token(&lang)
        .or_else(|| ps.find_syntax_by_extension(&lang))
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let bg = theme
        .settings
        .background
        .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
        .unwrap_or_else(|| "#ffffff".to_string());

    let mut out = String::new();
    out.push_str(&format!(
        r#"<pre style="background-color: {}; padding: 12px; overflow: auto;"><code class="language-{}">"#,
        bg, lang
    ));

    let mut h = HighlightLines::new(syntax, theme);

    for line in LinesWithEndings::from(code) {
        let regions = h.highlight_line(line, &*SYNTAX_SET).ok();
        if let Some(regions) = regions {
            let html_line = styled_line_to_highlighted_html(
                &regions[..],
                IncludeBackground::No,
            ).ok();
            if let Some(html_line) = html_line {
                out.push_str(&html_line);
            }
        }
    }

    out.push_str("</code></pre>\n");
    out
}

fn render_class_mode(
    code: &str,
    language: &str,
) -> String {
    let ps = &*SYNTAX_SET;
    let lang: String = if language.is_empty() || language == "plaintext" {
        detect_syntax_from_content(ps, code)
    } else {
        language.to_string()
    };
    let syntax = ps
        .find_syntax_by_token(&lang)
        .or_else(|| ps.find_syntax_by_extension(&lang))
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let mut html_gen =
        ClassedHTMLGenerator::new_with_class_style(syntax, &*SYNTAX_SET, ClassStyle::Spaced);

    let mut html = String::new();
    html.push_str(&format!(r#"<pre class="code"><code class="language-{}">"#, lang));
    for line in LinesWithEndings::from(code) {
        html_gen
            .parse_html_for_line_which_includes_newline(line)
            .ok();
    }
    html.push_str(&html_gen.finalize());
    html.push_str("</code></pre>\n");
    html
}

// ---------------------------------------------------------------------------
// Custom theme loading
// ---------------------------------------------------------------------------

/// Load .tmTheme files from user theme directories.
/// Embedded themes are loaded separately by Python after module import.
/// Called automatically on module initialization.
pub fn load_builtin_themes() -> Vec<String> {
    let mut loaded = Vec::new();
    
    // Get user's home directory
    let home_dir = std::env::var("HOME")
        .or(std::env::var("USERPROFILE"))
        .or(std::env::var("APPDATA"))
        .unwrap_or(".".to_string());
    
    // Collect all theme directories to scan
    let mut theme_dirs = Vec::new();
    
    // 1. User's .mordant/themes in home directory
    let user_themes_home = PathBuf::from(&home_dir).join(".mordant").join("themes");
    theme_dirs.push(user_themes_home);
    
    // 2. User's %APPDATA%/mordant/themes on Windows
    let user_themes_appdata = PathBuf::from(&home_dir).join("AppData").join("Roaming").join("mordant").join("themes");
    theme_dirs.push(user_themes_appdata);
    
    for theme_dir in theme_dirs {
        // Create the directory if it doesn't exist
        if !theme_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&theme_dir) {
                eprintln!("Warning: Could not create theme directory {}: {}", theme_dir.display(), e);
                continue;
            }
        }
        
        if theme_dir.exists() && theme_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&theme_dir) {
                for entry in entries {
                    if let Ok(dir_entry) = entry {
                        let file = dir_entry.file_name();
                        let file_str = file.to_string_lossy();
                        let file_path = theme_dir.join(&file);

                        if file_str.ends_with(".tmTheme") {
                            let theme_name = file_str.trim_end_matches(".tmTheme");

                            if let Ok(content) = std::fs::read_to_string(&file_path) {
                                if let Ok(theme) = ThemeSet::load_from_reader(&mut Cursor::new(content)) {
                                    let mut ts = THEME_SET.write().unwrap();
                                    ts.themes.insert(theme_name.to_string(), theme);
                                    loaded.push(theme_name.to_string());
                                }
                            }
                        } else if file_str.ends_with(".json") {
                            let theme_name = file_str.trim_end_matches(".json");

                            if let Ok(content) = std::fs::read_to_string(&file_path) {
                                if is_vscode_json_theme(&content) {
                                    match parse_vscode_theme_jsonc(&content) {
                                        Ok(vscode_theme) => {
                                            match crate::vscode_theme::vscode_theme_to_syntect(&vscode_theme) {
                                                Ok(theme) => {
                                                    let mut ts = THEME_SET.write().unwrap();
                                                    ts.themes.insert(theme_name.to_string(), theme);
                                                    loaded.push(theme_name.to_string());
                                                }
                                                Err(e) => {
                                                    eprintln!("Warning: Could not convert VSCode theme {}: {}", file_str, e);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Warning: Could not parse VSCode JSON theme {}: {}", file_str, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    loaded
}

/// Register a custom syntect theme from JSON or XML content.
///
/// Automatically detects whether the content is a VSCode JSON theme
/// or a plist XML (.tmTheme) file.
///
/// # Arguments
/// * `name` - Theme name (used to reference this theme)
/// * `content` - JSON content (VSCode theme) or XML content (.tmTheme)
///
/// # Example
/// ```python
/// # For a .tmTheme file (plist XML)
/// with open("my_theme.tmTheme", "r") as f:
///     mordant.add_custom_theme("my-theme", f.read())
///
/// # For a VSCode JSON theme
/// with open("my_vscode_theme.json", "r") as f:
///     mordant.add_custom_theme("my-vscode-theme", f.read())
/// ```
#[pyfunction]
pub fn add_custom_theme(name: &str, content: &str) -> PyResult<()> {
    let mut ts = THEME_SET.write().unwrap();
    
    if is_vscode_json_theme(content) {
        // Parse as VSCode JSON theme
        match parse_vscode_theme_jsonc(content) {
            Ok(vscode_theme) => {
                match crate::vscode_theme::vscode_theme_to_syntect(&vscode_theme) {
                    Ok(theme) => {
                        ts.themes.insert(name.to_string(), theme);
                        Ok(())
                    }
                    Err(e) => Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "VSCode theme conversion error: {}", e
                    ))),
                }
            }
            Err(e) => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Failed to parse VSCode JSON theme: {}", e
            ))),
        }
    } else if is_plist_xml_theme(content) {
        // Parse as plist XML (.tmTheme)
        let mut reader = Cursor::new(content.as_bytes());
        let theme = ThemeSet::load_from_reader(&mut reader);
        
        match theme {
            Ok(t) => {
                ts.themes.insert(name.to_string(), t);
                Ok(())
            }
            Err(e) => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "Failed to parse plist XML theme: {}", e
            ))),
        }
    } else {
        // Try plist XML first (fallback)
        let mut reader = Cursor::new(content.as_bytes());
        let theme_result = ThemeSet::load_from_reader(&mut reader);
        
        if let Ok(t) = theme_result {
            ts.themes.insert(name.to_string(), t);
            Ok(())
        } else {
            // Try VSCode JSON as last resort
            match parse_vscode_theme_jsonc(content) {
                Ok(vscode_theme) => {
                    match crate::vscode_theme::vscode_theme_to_syntect(&vscode_theme) {
                        Ok(theme) => {
                            ts.themes.insert(name.to_string(), theme);
                            Ok(())
                        }
                        Err(e) => Err(pyo3::exceptions::PyValueError::new_err(format!(
                            "Theme conversion error: {}", e
                        ))),
                    }
                }
                Err(e) => Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Failed to parse theme: {} (tried both plist XML and VSCode JSON)", e
                ))),
            }
        }
    }
}

/// List all available syntax highlighting themes.
///
/// # Returns
/// List of theme names (built-in + custom themes from .mordant/themes/)
///
/// # Example
/// ```python
/// themes = mordant.list_themes()
/// print(themes)
/// # ['InspiredGitHub', 'GitHub', 'Dracula', 'my-custom-theme', ...]
/// ```
#[pyfunction]
pub fn list_themes() -> Vec<String> {
    let ts = THEME_SET.read().unwrap();
    ts.themes.keys().cloned().collect()
}

/// List all available syntaxes from syntect-assets.
///
/// # Returns
/// List of syntax names (languages supported for highlighting)
///
/// # Example
/// ```python
/// syntaxes = mordant.list_syntaxes()
/// print(syntaxes)
/// # ['Rust', 'Python', 'JavaScript', ...]
/// ```
#[pyfunction]
pub fn list_syntaxes() -> Vec<String> {
    let ps = &*SYNTAX_SET;
    ps.syntaxes().iter()
        .map(|s| s.name.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Renderer integration
// ---------------------------------------------------------------------------

/// HTML renderer that intercepts CodeBlock nodes and applies syntax highlighting.
struct HighlightingHtmlRenderer<W: TextWrite> {
    _phantom: core::marker::PhantomData<W>,
    writer: html::Writer,
    options: HighlightingRendererOptions,
}

impl<W: TextWrite> HighlightingHtmlRenderer<W> {
    fn new(
        html_opts: html::Options,
        options: HighlightingRendererOptions,
    ) -> Self {
        Self {
            _phantom: core::marker::PhantomData,
            writer: html::Writer::with_options(html_opts),
            options,
        }
    }
}

impl<W: TextWrite> RenderNode<W> for HighlightingHtmlRenderer<W> {
    fn render_node<'a>(
        &self,
        w: &mut W,
        source: &'a str,
        arena: &'a Arena,
        node_ref: NodeRef,
        entering: bool,
        _ctx: &mut renderer::Context,
    ) -> Result<WalkStatus> {
        if entering {
            let kd = as_kind_data!(arena, node_ref, CodeBlock);
            let mut code = String::new();
            for line in kd.value().iter(source) {
                code.push_str(&line);
            }
            let lang = kd.language_str(source).unwrap_or("");

            // Apply highlighting
            let html_output = highlight_code(
                lang,
                &code,
                &self.options.theme,
                &self.options.mode,
            );

            w.write_str(&html_output)?;
            return Ok(WalkStatus::Continue);
        }
        Ok(WalkStatus::Continue)
    }
}

impl<'r, W> NodeRenderer<'r, W> for HighlightingHtmlRenderer<W>
where
    W: TextWrite + 'r,
{
    fn register_node_renderer_fn(
        self,
        nrr: &mut impl NodeRendererRegistry<'r, W>,
    ) {
        nrr.register_node_renderer_fn(TypeId::of::<CodeBlock>(), BoxRenderNode::new(self));
    }
}

// ---------------------------------------------------------------------------
// Extension factory
// ---------------------------------------------------------------------------

/// Create a highlighting renderer extension.
pub fn highlighting_html_renderer_extension<'cb, W>(
    options: impl Into<HighlightingRendererOptions>,
) -> impl renderer::html::RendererExtension<'cb, W>
where
    W: TextWrite + 'cb,
{
    renderer::html::RendererExtensionFn::new(move |r: &mut html::Renderer<'cb, W>| {
        let options = options.into();
        r.add_node_renderer(HighlightingHtmlRenderer::new, options);
    })
}
