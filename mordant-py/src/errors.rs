//! Python exception types for mordant errors.

use pyo3::prelude::*;

/// Base exception for all mordant errors.
#[pyclass(module = "mordant", skip_from_py_object)]
#[derive(Clone)]
pub struct MordantError {
    message: String,
}

#[pymethods]
impl MordantError {
    #[new]
    fn new(message: String) -> Self {
        MordantError { message }
    }

    #[getter]
    fn message(&self) -> &str {
        &self.message
    }

    fn __str__(&self) -> &str {
        &self.message
    }
}

/// Convert a mordant library error to a Python exception.
#[allow(dead_code)]
pub fn mordant_err_to_pyerr(err: mordant_lib::Error) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(err.to_string())
}
