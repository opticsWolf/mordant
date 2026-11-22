//! Link attribute extension for mordant.
//!
//! Consumes an attribute list immediately after a link or image and attaches
//! it to that node:
//!
//! ```markdown
//! [link](https://example.com){#id .class target="_blank"}
//! ![alt](image.png){width=100 .framed}
//! ```
//!
//! The built-in HTML renderer already emits attributes on link and image
//! nodes, so no renderer extension is required.

use crate::ast::{Arena, KindData, NodeRef};
use crate::parser::{
    self, AnyAstTransformer, AstTransformer, Parser, ParserExtension, ParserExtensionFn,
};
use crate::text::{self, Reader, Segment};

#[derive(Debug, Default)]
struct LinkAttributesTransformer;

fn collect_nodes(arena: &Arena, node_ref: NodeRef, out: &mut Vec<NodeRef>) {
    out.push(node_ref);
    let mut child = arena[node_ref].first_child();
    while let Some(child_ref) = child {
        collect_nodes(arena, child_ref, out);
        child = arena[child_ref].next_sibling();
    }
}

/// A text node that was only partially consumed, paired with the source
/// offset where its remaining content starts.
type AttributeRemainder = (NodeRef, usize);

/// Nodes consumed by an attribute list, plus an optional trailing text node
/// whose remaining content starts at the given source offset.
type CollectedAttributeNodes = (Vec<NodeRef>, Option<AttributeRemainder>);

/// Collects the run of following siblings whose source positions lie inside
/// `[start, end)`.
///
/// Inline parsers split attribute lists into several text nodes (at `_`, `:`,
/// `*`, backticks, entities, ...); the list must be parsed from the original
/// source span rather than from the first text node.
///
/// Returns `None` if any affected node has no source position, or if a node
/// straddles `end` without being a source-backed text node. In that case the
/// caller leaves the AST untouched.
fn collect_attribute_nodes(
    arena: &Arena,
    first: NodeRef,
    end: usize,
) -> Option<CollectedAttributeNodes> {
    fn max_text_stop(arena: &Arena, node_ref: NodeRef) -> Option<usize> {
        let mut max = match arena[node_ref].kind_data() {
            KindData::Text(text) => text.index().map(|i| i.stop()),
            _ => None,
        };
        let mut child = arena[node_ref].first_child();
        while let Some(c) = child {
            max = max.max(max_text_stop(arena, c));
            child = arena[c].next_sibling();
        }
        max
    }

    let mut nodes = Vec::new();
    let mut remainder = None;
    let mut current = Some(first);
    while let Some(node_ref) = current {
        let pos = arena[node_ref].pos()?;
        if pos >= end {
            break;
        }
        match arena[node_ref].kind_data() {
            KindData::Text(text) => {
                let index = text.index()?;
                if index.stop() > end {
                    remainder = Some((node_ref, end));
                }
            }
            _ => {
                // Non-text inline nodes (e.g. emphasis) may occur inside a
                // quoted attribute value. They may be removed only if all
                // their text is inside the attribute list.
                if max_text_stop(arena, node_ref).is_some_and(|stop| stop > end) {
                    return None;
                }
            }
        }
        nodes.push(node_ref);
        current = arena[node_ref].next_sibling();
    }
    Some((nodes, remainder))
}

impl AstTransformer for LinkAttributesTransformer {
    fn transform(
        &self,
        arena: &mut Arena,
        doc_ref: NodeRef,
        reader: &mut text::BasicReader,
        _context: &mut parser::Context,
    ) {
        let source = reader.source();
        let mut nodes = Vec::new();
        collect_nodes(arena, doc_ref, &mut nodes);

        for node_ref in nodes {
            let is_link = matches!(arena[node_ref].kind_data(), KindData::Link(_));
            let is_image = matches!(arena[node_ref].kind_data(), KindData::Image(_));
            if !is_link && !is_image {
                continue;
            }
            let Some(next_ref) = arena[node_ref].next_sibling() else {
                continue;
            };
            let KindData::Text(text) = arena[next_ref].kind_data() else {
                continue;
            };
            if !text.str(source).starts_with('{') {
                continue;
            }
            let Some(index) = text.index() else {
                continue;
            };
            let start = index.start();
            if !source.is_char_boundary(start) {
                continue;
            }

            // Parse the whole list from the source, not from the (possibly
            // split) text node.
            let mut attr_reader = text::BasicReader::new(source);
            attr_reader.set_position(0, Segment::new(start, source.len()));
            let Some(attrs) = parser::parse_attributes(&mut attr_reader) else {
                continue;
            };
            let end = attr_reader.position().1.start();
            if end <= start || !source.is_char_boundary(end) {
                continue;
            }
            let Some((consumed, remainder)) = collect_attribute_nodes(arena, next_ref, end) else {
                continue;
            };

            // Apply all attributes atomically. `href`/`src` are written by the
            // built-in link/image renderer and cannot be overridden here:
            // doing so would emit a second, unsanitized URL attribute.
            for (key, value) in attrs {
                if (is_link && key == "href") || (is_image && key == "src") {
                    continue;
                }
                arena[node_ref].attributes_mut().insert(key, value);
            }

            if let Some((node_ref, pos)) = remainder {
                if let KindData::Text(text) = arena[node_ref].kind_data_mut() {
                    if let Some(idx) = text.index() {
                        let stop = idx.stop();
                        text.set(text::Index::new(pos, stop));
                    }
                }
            }
            for node_ref in consumed {
                if remainder.is_some_and(|(r, _)| r == node_ref) {
                    continue;
                }
                node_ref.remove(arena);
            }
        }
    }
}

impl From<LinkAttributesTransformer> for AnyAstTransformer {
    fn from(t: LinkAttributesTransformer) -> Self {
        AnyAstTransformer::Extension(Box::new(t))
    }
}

/// Returns a parser extension that attaches trailing attribute lists to links
/// and images.
pub fn link_attributes_parser_extension() -> impl ParserExtension {
    ParserExtensionFn::new(|p: &mut Parser| {
        p.add_ast_transformer(|| LinkAttributesTransformer, parser::NoParserOptions, 1000);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::KindData;
    use crate::parser::Parser;

    #[test]
    fn attributes_are_attached_to_links() {
        let source = "[link](https://example.com){#foo .bar}";
        let parser = Parser::with_extensions(
            crate::parser::Options::default(),
            link_attributes_parser_extension(),
        );
        let (arena, doc) = parser.parse(&mut text::BasicReader::new(source));

        let mut refs = Vec::new();
        collect_nodes(&arena, doc, &mut refs);
        let link = refs
            .iter()
            .copied()
            .find(|r| matches!(arena[*r].kind_data(), KindData::Link(_)))
            .expect("link node");
        let attrs = arena[link].attributes();
        assert_eq!(
            attrs
                .get("id")
                .map(|v| v.str(source).into_owned())
                .as_deref(),
            Some("foo")
        );
        assert_eq!(
            attrs
                .get("class")
                .map(|v| v.str(source).into_owned())
                .as_deref(),
            Some("bar")
        );
        assert!(!refs.iter().any(|r| {
            matches!(arena[*r].kind_data(), KindData::Text(t) if t.str(source).starts_with('{'))
        }));
    }

    #[test]
    fn html_renderer_emits_link_attributes() {
        use crate::extensions::{Engine, Engines};
        let engines = Engines::only(&[Engine::LinkAttributes]);
        let mut parser_opts = crate::parser::Options::default();
        parser_opts.engines = engines;
        let mut html_opts = crate::renderer::html::Options::default();
        html_opts.engines = engines;
        let mut output = String::new();
        crate::markdown_to_html_string_with_options(
            &mut output,
            "[link](https://example.com){#foo .bar}",
            parser_opts,
            html_opts,
        )
        .unwrap();
        assert!(output.contains("id=\"foo\""), "{output}");
        assert!(output.contains("class=\"bar\""), "{output}");
        assert!(!output.contains('{'), "{output}");
    }

    #[test]
    fn leaf_like_attributes_are_ignored() {
        let source = "plain {text}";
        let parser = Parser::with_extensions(
            crate::parser::Options::default(),
            link_attributes_parser_extension(),
        );
        let (arena, doc) = parser.parse(&mut text::BasicReader::new(source));
        let mut refs = Vec::new();
        collect_nodes(&arena, doc, &mut refs);
        assert!(refs.iter().all(|r| arena[*r].attributes().is_empty()));
    }
}
