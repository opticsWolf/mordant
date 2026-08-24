//! YAML frontmatter parser extension for mordant.
//!
//! The implementation moved to the core `mordant` crate (feature `meta`);
//! this module re-exports it so the Python extension keeps the same API.

pub use mordant_lib::meta::*;
