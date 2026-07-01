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

/// Highlight code using syntect.
fn highlight_code(
    language: &str,
    code: &str,
    theme_name: &str,
    mode: &HighlightingMode,
) -> String {
    let ps = &*SYNTAX_SET;
    let lang = if language.is_empty() { "plaintext" } else { language };
    let _syntax = ps
        .find_syntax_by_token(lang)
        .or_else(|| ps.find_syntax_by_extension(lang))
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let ts = THEME_SET.read().unwrap();
    let theme = ts.themes
        .get(theme_name)
        .unwrap_or_else(|| &ts.themes["InspiredGitHub"]);

    match mode {
        HighlightingMode::Attribute => {
            render_attribute_mode(theme, code, lang)
        }
        HighlightingMode::Class => {
            render_class_mode(code, lang)
        }
    }
}

fn render_attribute_mode(
    theme: &syntect::highlighting::Theme,
    code: &str,
    language: &str,
) -> String {
    let ps = &*SYNTAX_SET;
    let lang = if language.is_empty() { "plaintext" } else { language };
    let syntax = ps
        .find_syntax_by_token(lang)
        .or_else(|| ps.find_syntax_by_extension(lang))
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let bg = theme
        .settings
        .background
        .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b))
        .unwrap_or_else(|| "#ffffff".to_string());

    let mut out = String::new();
    out.push_str(&format!(
        r#"<pre style="background-color: {}; padding: 12px; overflow: auto;"><code class="language-{}">"#,
        bg, language
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
    let syntax = ps.find_syntax_plain_text();

    let mut html_gen =
        ClassedHTMLGenerator::new_with_class_style(syntax, &*SYNTAX_SET, ClassStyle::Spaced);

    let mut html = String::new();
    html.push_str(&format!(r#"<pre class="code"><code class="language-{}">"#, language));
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
                        if file_str.ends_with(".tmTheme") {
                            let theme_name = file_str.trim_end_matches(".tmTheme");
                            let file_path = theme_dir.join(&file);
                            
                            if let Ok(content) = std::fs::read_to_string(&file_path) {
                                if let Ok(theme) = ThemeSet::load_from_reader(&mut Cursor::new(content)) {
                                    let mut ts = THEME_SET.write().unwrap();
                                    ts.themes.insert(theme_name.to_string(), theme);
                                    loaded.push(theme_name.to_string());
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

/// Register a custom syntect theme from XML content.
///
/// # Arguments
/// * `name` - Theme name (used to reference this theme)
/// * `content` - XML content of the .tmTheme file
///
/// # Example
/// ```python
/// with open("my_theme.tmTheme", "r") as f:
///     mordant.add_custom_theme("my-theme", f.read())
/// ```
#[pyfunction]
pub fn add_custom_theme(name: &str, content: &str) -> PyResult<()> {
    let mut reader = Cursor::new(content.as_bytes());
    let theme = ThemeSet::load_from_reader(&mut reader);
    
    match theme {
        Ok(t) => {
            let mut ts = THEME_SET.write().unwrap();
            ts.themes.insert(name.to_string(), t);
            Ok(())
        }
        Err(e) => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "Failed to parse custom theme: {}", e
        ))),
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
            let lang = kd.language_str(source).unwrap_or("plaintext");

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
