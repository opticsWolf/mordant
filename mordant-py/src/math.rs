//! PyO3 bindings for the math engine (moved to core `mordant::math`).
//!
//! The pure KaTeX engine, parser/renderer extensions and unit tests live
//! behind the `math` feature of the core crate; this module re-exports them
//! and keeps the Python-exposed surface (`render_math`, options classes) plus
//! the highlighter-dependent regression tests.

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyString;

pub use mordant_lib::math::*;

/// Render LaTeX math to markup (Python-exposed wrapper over the core engine).
#[pyfunction]
#[pyo3(signature = (latex, display = false, output = "both"))]
pub fn render_math(
    py: Python<'_>,
    latex: &str,
    display: bool,
    output: &str,
) -> PyResult<Py<PyString>> {
    let markup = mordant_lib::math::render_math(latex, display, output)
        .map_err(|e| PyValueError::new_err(e))?;
    Ok(PyString::new(py, &markup).unbind())
}

/// Python-exposed math renderer options.
#[pyclass(module = "mordant", name = "MathRendererOptions")]
#[derive(Clone)]
pub struct PyMathRendererOptions {
    #[pyo3(get, set)]
    pub output: String,
}

#[pymethods]
impl PyMathRendererOptions {
    #[new]
    #[pyo3(signature = (output = "both"))]
    pub fn new(output: &str) -> Self {
        PyMathRendererOptions {
            output: output.to_string(),
        }
    }
}

impl PyMathRendererOptions {
    pub fn to_mordant(&self) -> MathRendererOptions {
        MathRendererOptions {
            output: output_from_str(&self.output).unwrap_or(katex::OutputFormat::HtmlAndMathml),
        }
    }
}

// ---------------------------------------------------------------------------
// Regression tests that need the binding-side highlighter. The pure-math
// tests live in core src/math.rs.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod highlighter_interaction_tests {
    use super::*;
    use mordant_lib::renderer::html;
    use mordant_lib::parser;

    use crate::highlighter::{highlighting_html_renderer_extension, HighlightingRendererOptions};

    /// Render with the math extensions AND the code highlighter active.
    /// Mirrors md_viewer, which always passes a highlighting theme.
    fn render_math_highlighted(source: &str) -> String {
        let parser_ext = math_parser_extension(MathParserOptions::default());
        let renderer_ext = math_html_renderer_extension(MathRendererOptions::default())
            .and(math_inline_html_renderer_extension(MathInlineRendererOptions::default()))
            .and(highlighting_html_renderer_extension(
                HighlightingRendererOptions::default(),
            ));
        let html_opts = html::Options::default();
        let mut result = String::new();
        let f = mordant_lib::new_markdown_to_html(
            parser::Options::default(),
            html_opts,
            parser_ext,
            renderer_ext,
        );
        f(&mut result, source).unwrap();
        result
    }

    // Bug A: fenced math/latex must become KaTeX even when a theme is active.
    #[test]
    fn fenced_math_with_highlighting_bug_a() {
        let h = render_math_highlighted("```math\nE = mc^2\n```");
        assert!(h.contains("katex"), "fenced math under a theme should render KaTeX: {h}");
        assert!(h.contains("katex-display"), "fenced math is display mode: {h}");
        assert!(!h.contains("language-math"), "should not fall through to highlighting: {h}");
    }

    #[test]
    fn latex_fence_with_highlighting_bug_a() {
        let h = render_math_highlighted("```latex\nE = mc^2\n```");
        assert!(h.contains("katex"), "fenced latex under a theme should render KaTeX: {h}");
        assert!(!h.contains("language-latex"), "should not fall through to highlighting: {h}");
    }

    #[test]
    fn other_code_blocks_still_highlighted() {
        let h = render_math_highlighted("```python\nx = 1\n```");
        assert!(h.contains("language-python"), "non-math code should still highlight: {h}");
        assert!(!h.contains("katex"), "python block must not be treated as math: {h}");
    }

    #[test]
    fn multiline_display_math_with_highlighting() {
        let h = render_math_highlighted("$$\nE = mc^2\n$$");
        assert!(h.contains("katex"), "multi-line $$ under a theme should render: {h}");
        assert!(h.contains("E = mc"), "formula content preserved: {h}");
    }
}
