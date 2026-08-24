//! PyO3 bindings for the math engine (moved to core `mordant::math`).
//!
//! The pure KaTeX engine, parser/renderer extensions and unit tests live
//! behind the `math` feature of the core crate. The highlighter×math
//! interaction tests now live in core `src/highlighter.rs` (core highlighter
//! depends on core math, so that is the only possible home).

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
