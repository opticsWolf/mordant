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
    /// * `language` - Language identifier (e.g. "rust", "python")
    /// * `code` - Source code to highlight
    ///
    /// # Returns
    /// HTML string with syntax highlighting
    fn highlight(&self, language: &str, code: &str) -> PyResult<String> {
        let result = mordant_lib::highlighter::highlight_code(language, code, &self.theme, &self.mode);
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
