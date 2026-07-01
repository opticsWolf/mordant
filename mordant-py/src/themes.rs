//! Embedded themes bundled with the wheel.
//!
//! Theme files are loaded from the package's themes/ directory at runtime
//! by Python. This Rust module provides the add_custom_theme function.

use std::io::Cursor;
use syntect::highlighting::ThemeSet;
