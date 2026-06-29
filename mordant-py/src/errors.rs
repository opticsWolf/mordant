//! Python exception types for rushdown errors.

use pyo3::prelude::*;

/// Base exception for all rushdown errors.
#[pyclass(module = "mordant", skip_from_py_object)]
#[derive(Clone)]
pub struct RushdownError {
    message: String,
}

#[pymethods]
impl RushdownError {
    #[new]
    fn new(message: String) -> Self {
        RushdownError { message }
    }

    #[getter]
    fn message(&self) -> &str {
        &self.message
    }

    fn __str__(&self) -> &str {
        &self.message
    }
}

/// Convert a rushdown library error to a Python exception.
#[allow(dead_code)]
pub fn rushdown_err_to_pyerr(err: rushdown_lib::Error) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(err.to_string())
}
