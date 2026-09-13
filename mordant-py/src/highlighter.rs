//! PyO3 bindings for the code-highlighting engine (moved to core
//! `mordant::highlighter`).
//!
//! The pure engine (syntax/theme registries, highlighters, renderer extension)
//! lives behind the `highlighter` feature of the core crate; this module
//! re-exports it and keeps the Python-exposed surface.

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

pub use mordant_lib::highlighter::*;

/// The highlighting mode exposed to Python.
#[pyclass(module = "mordant", name = "HighlightingMode", skip_from_py_object)]
pub enum PyHighlightingMode {
    /// Inline style attributes (default).
    Attribute,
    /// CSS class attributes.
    Class,
}

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
            _ => return Err(PyValueError::new_err(format!(
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
    /// * `language` - Language identifier (e.g. "rust", "python"). If empty
    ///   or "plaintext", the language is auto-detected from the content.
    /// * `code` - Source code to highlight
    /// * `bare` - If True, return only the highlighted token spans without the
    ///   `<pre>/<code>` wrapper, for embedding into your own container. Pair
    ///   with `mordant.theme_background(theme)` for container styling.
    ///
    /// # Returns
    /// HTML string with syntax highlighting
    #[pyo3(signature = (language, code, bare = false))]
    fn highlight(&self, language: &str, code: &str, bare: bool) -> PyResult<String> {
        let result = if bare {
            mordant_lib::highlighter::highlight_spans(language, code, &self.theme, &self.mode)
        } else {
            mordant_lib::highlighter::highlight_code(language, code, &self.theme, &self.mode)
        };
        Ok(result)
    }
}

/// Register a custom syntect theme from JSON or XML content.
///
/// Automatically detects whether the content is a VSCode JSON theme
/// or a plist XML (.tmTheme) file.
#[pyfunction]
pub fn add_custom_theme(name: &str, content: &str) -> PyResult<()> {
    mordant_lib::highlighter::register_custom_theme(name, content)
        .map_err(|e| PyValueError::new_err(e))
}

/// List all available syntax highlighting themes.
#[pyfunction]
pub fn list_themes() -> Vec<String> {
    mordant_lib::highlighter::list_themes()
}

/// List all available syntaxes from syntect-assets.
#[pyfunction]
pub fn list_syntaxes() -> Vec<String> {
    mordant_lib::highlighter::list_syntaxes()
}

/// Register a custom syntax definition from `.sublime-syntax` (YAML) content.
///
/// The syntax becomes available to all highlighting — both `markdown_to_html`
/// and `Highlighter` — and can be selected by its name or file extensions.
///
/// # Arguments
/// * `content` - `.sublime-syntax` YAML content
/// * `name` - Optional fallback name when the YAML has no `name:` key
///
/// # Returns
/// The registered syntax name (from the YAML `name:` key or the fallback).
///
/// Note: a custom syntax may reference built-in syntaxes, but built-in
/// syntaxes cannot reference a custom one (syntect limitation).
#[pyfunction]
#[pyo3(signature = (content, name = None))]
pub fn add_custom_syntax(py: Python<'_>, content: &str, name: Option<&str>) -> PyResult<String> {
    let result = py.detach(|| mordant_lib::highlighter::register_custom_syntax(content, name));
    result.map_err(PyValueError::new_err)
}

/// Detect the language of a code snippet without highlighting it.
///
/// Tries shebang/first-line matching, then token/extension matching, then
/// content heuristics. Returns the detected language identifier (e.g.
/// "python"), or "plaintext" when nothing matches.
#[pyfunction]
pub fn detect_language(code: &str) -> String {
    mordant_lib::highlighter::detect_language(code)
}

/// Background color of a registered theme as "#rrggbb".
///
/// Useful together with `Highlighter.highlight(..., bare=True)`: highlight
/// bare spans, then build your own container with the theme's background.
/// Returns `None` if no theme with that name is registered.
#[pyfunction]
pub fn theme_background(name: &str) -> Option<String> {
    mordant_lib::highlighter::theme_background(name)
}
