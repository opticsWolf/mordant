//! Footnote extension for mordant (Python bindings).
//!
//! The engine lives in the core `mordant` crate (feature `footnotes`);
//! this module re-exports it and adds the Python-exposed options class.

pub use mordant_lib::footnote::{
    footnote_html_renderer_extension, footnote_parser_extension, FootnoteDefinition,
    FootnoteHtmlRendererOptions, FootnoteIdPrefix, FootnoteReference,
};

use pyo3::prelude::*;

/// Python-exposed options for the footnote HTML renderer.
///
/// # Example
/// ```python
/// import mordant
///
/// opts = mordant.PyFootnoteHtmlRendererOptions(
///     link_class="my-footnote-ref",
///     backlink_class="my-backlink",
///     backlink_html="↩",
///     id_prefix="note-",
/// )
/// html = mordant.markdown_to_html(
///     "Text with footnote.[^1]\n\n[^1]: The footnote.",
///     footnote_render_opts=opts,
/// )
/// ```
#[pyclass(module = "mordant", name = "FootnoteHtmlRendererOptions")]
pub struct PyFootnoteHtmlRendererOptions {
    /// CSS class for footnote reference links.
    #[pyo3(get, set)]
    pub link_class: String,

    /// CSS class for footnote backlinks.
    #[pyo3(get, set)]
    pub backlink_class: String,

    /// HTML content for the backlink character.
    #[pyo3(get, set)]
    pub backlink_html: String,

    /// Optional prefix for footnote IDs.
    #[pyo3(get, set)]
    pub id_prefix: Option<String>,
}

#[pymethods]
impl PyFootnoteHtmlRendererOptions {
    #[new]
    #[pyo3(signature = (
        link_class = "footnote-ref".to_string(),
        backlink_class = "footnote-backref".to_string(),
        backlink_html = "&#x21a9;&#xfe0e;".to_string(),
        id_prefix = None,
    ))]
    fn new(
        link_class: String,
        backlink_class: String,
        backlink_html: String,
        id_prefix: Option<String>,
    ) -> Self {
        PyFootnoteHtmlRendererOptions {
            link_class,
            backlink_class,
            backlink_html,
            id_prefix,
        }
    }
}

impl PyFootnoteHtmlRendererOptions {
    pub fn to_mordant(&self) -> FootnoteHtmlRendererOptions {
        let id_prefix = match &self.id_prefix {
            None => FootnoteIdPrefix::None,
            Some(prefix) => FootnoteIdPrefix::Value(prefix.clone()),
        };
        FootnoteHtmlRendererOptions {
            link_class: self.link_class.clone(),
            backlink_class: self.backlink_class.clone(),
            backlink_html: self.backlink_html.clone(),
            id_prefix,
        }
    }
}
