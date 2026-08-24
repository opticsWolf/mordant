//! Emoji extension for mordant (Python bindings).
//!
//! The engine lives in the core `mordant` crate (feature `emoji`);
//! this module re-exports it and adds the Python-exposed option classes.

pub use mordant_lib::emoji::{
    emoji_html_renderer_extension, emoji_parser_extension, EmojiData,
    EmojiHtmlRendererOptions, EmojiParserOptions,
};

use pyo3::prelude::*;

/// Options for the emoji parser (Python-exposed).
#[pyclass(module = "mordant", name = "EmojiParserOptions")]
pub struct PyEmojiParserOptions {
    /// A comma-separated list of emoji shortcodes to ignore.
    #[pyo3(get)]
    pub blacklist: Option<String>,
}

impl Default for PyEmojiParserOptions {
    fn default() -> Self {
        PyEmojiParserOptions { blacklist: None }
    }
}

#[pymethods]
impl PyEmojiParserOptions {
    #[new]
    fn new(blacklist: Option<String>) -> Self {
        PyEmojiParserOptions { blacklist }
    }
}

impl PyEmojiParserOptions {
    pub fn to_mordant(&self) -> EmojiParserOptions {
        EmojiParserOptions { blacklist: self.to_blacklist() }
    }

    pub fn to_blacklist(&self) -> Vec<String> {
        self.blacklist.as_ref()
            .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
            .unwrap_or_default()
    }
}

/// Options for the emoji HTML renderer (Python-exposed).
#[pyclass(module = "mordant", name = "EmojiHtmlRendererOptions")]
pub struct PyEmojiHtmlRendererOptions {
    /// A template string for rendering emojis. Supports {emoji}, {shortcode}, {name}.
    #[pyo3(get)]
    pub template: Option<String>,
}

impl Default for PyEmojiHtmlRendererOptions {
    fn default() -> Self {
        PyEmojiHtmlRendererOptions { template: None }
    }
}

#[pymethods]
impl PyEmojiHtmlRendererOptions {
    #[new]
    fn new(template: Option<String>) -> Self {
        PyEmojiHtmlRendererOptions { template }
    }
}

impl PyEmojiHtmlRendererOptions {
    pub fn to_mordant(&self) -> EmojiHtmlRendererOptions {
        EmojiHtmlRendererOptions { template: self.template.clone() }
    }
}
