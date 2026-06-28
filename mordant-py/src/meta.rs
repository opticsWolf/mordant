//! YAML frontmatter parser extension for rushdown.
//!
//! This module provides a parser extension that extracts YAML frontmatter
//! from markdown documents and stores it as metadata on the Document node.
//!
//! Based on the rushdown-meta crate by Yusuke Inuzuka.

use std::boxed::Box;
use std::format;
use std::rc::Rc;
use std::string::String;
use std::string::ToString;
use std::vec::Vec;
use std::cell::RefCell;
use std::result::Result as CoreResult;

use rushdown_lib::ast::{Arena, Meta, NodeRef};
use rushdown_lib::context::{ContextKey, ContextKeyRegistry, NodeRefValue};
use rushdown_lib::parser::{
    self, AnyAstTransformer, AnyBlockParser, AstTransformer, BlockParser, NoParserOptions,
    Parser, ParserExtension, ParserExtensionFn, PRIORITY_SETTEXT_HEADING,
};
use rushdown_lib::text::Reader;
use rushdown_lib::util::StringMap;

const META_NODE: &str = "mordant-meta-n";

/// Options for the meta parser.
#[derive(Debug, Clone, Default)]
pub struct MetaParserOptions {
    /// Convert the meta data to a table node in the AST.
    pub table: bool,
}

impl parser::ParserOptions for MetaParserOptions {}

// --- Meta value helpers ---

fn format_meta_value(meta: &Meta) -> String {
    match meta {
        Meta::Null => "null".to_string(),
        Meta::Bool(b) => b.to_string(),
        Meta::Int(i) => i.to_string(),
        Meta::Float(f) => f.to_string(),
        Meta::String(s) => s.clone(),
        Meta::Sequence(seq) => {
            let items: Vec<String> = seq.iter().map(format_meta_value).collect();
            format!("[{}]", items.join(", "))
        }
        Meta::Mapping(map) => {
            let items: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}: {}", k, format_meta_value(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

// --- YAML parsing via yaml-peg ---

fn to_meta<R: yaml_peg::repr::Repr>(node: &yaml_peg::Node<R>) -> Meta {
    match node.yaml() {
        yaml_peg::Yaml::Null => Meta::Null,
        yaml_peg::Yaml::Bool(b) => Meta::Bool(*b),
        yaml_peg::Yaml::Int(s) => Meta::Int(s.parse().unwrap_or(0)),
        yaml_peg::Yaml::Float(s) => Meta::Float(s.parse().unwrap_or(0.0)),
        yaml_peg::Yaml::Str(s) => Meta::String(s.clone()),
        yaml_peg::Yaml::Seq(seq) => Meta::Sequence(seq.iter().map(|n| to_meta(n)).collect()),
        yaml_peg::Yaml::Map(map) => {
            let mut result = StringMap::with_capacity(map.len());
            for (k, v) in map.iter() {
                if let yaml_peg::Yaml::Str(key) = k.yaml() {
                    result.insert(key.clone(), to_meta(v));
                }
            }
            Meta::Mapping(result)
        }
        yaml_peg::Yaml::Alias(_) => Meta::Null, // Aliases not supported
    }
}

fn parse_yaml(input: &str) -> CoreResult<Meta, String> {
    let doc = yaml_peg::parser::parse::<yaml_peg::repr::RcRepr>(input)
        .map_err(|e| format!("YAML parsing error: {:?}", e))?;
    if !doc.is_empty() {
        Ok(to_meta(&doc[0]))
    } else {
        Err("YAML document is empty".to_string())
    }
}

// --- Block parser ---

#[derive(Debug)]
struct MetaParser {
    meta_node: ContextKey<NodeRefValue>,
}

impl MetaParser {
    pub fn new(reg: Rc<RefCell<ContextKeyRegistry>>) -> Self {
        let meta_node = reg.borrow_mut().get_or_create::<NodeRefValue>(META_NODE);
        Self { meta_node }
    }
}

impl BlockParser for MetaParser {
    fn trigger(&self) -> &[u8] {
        b"-"
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut rushdown_lib::text::BasicReader,
        ctx: &mut parser::Context,
    ) -> Option<(NodeRef, parser::State)> {
        let (line, _) = reader.position();
        if line != 0 {
            return None;
        }
        let (line_bytes, seg) = reader.peek_line_bytes()?;
        if !line_bytes.starts_with(b"---") {
            return None;
        }

        // Valid frontmatter must be exactly `---` followed by a newline.
        // `-----` (5 dashes) is a thematic break, not frontmatter.
        // `---` without a newline is also not frontmatter.
        let has_newline = line_bytes.iter().any(|&b| b == b'\n');
        if !has_newline {
            return None; // No newline after `---` - not frontmatter
        }

        // Check what comes after the newline. The line after `---` must have
        // actual content (not empty/whitespace-only, not another `---`).
        // Additionally, the content should look like YAML (contain colons,
        // list markers, or block scalars) to avoid consuming setext headings
        // like `---\nFoo\n---` which should be thematic break + heading.
        let source = reader.source();
        let after_line_pos = seg.stop();
        if after_line_pos >= source.len() {
            return None; // `---\n` at EOF - thematic break
        }
        let remaining = &source[after_line_pos..];

        // Check the first line of remaining content
        if let Some(first_nl) = remaining.find('\n') {
            let first_line = &remaining[..first_nl];
            // If the first line is empty/whitespace-only or is just `---`, it's a thematic break
            if first_line.trim().is_empty() || first_line.trim() == "---" {
                return None;
            }
            // Only claim as frontmatter if the content looks like YAML:
            // - Contains a colon (key: value)
            // - Starts with "- " (list item)
            // - Starts with "|" or ">" (block scalar)
            let trimmed = first_line.trim();
            let looks_like_yaml = trimmed.contains(':') 
                || trimmed.starts_with("- ") 
                || trimmed.starts_with("|") 
                || trimmed.starts_with(">")
                || trimmed.starts_with("---")
                || trimmed.starts_with("...");
            if !looks_like_yaml {
                return None; // Looks like plain text, not YAML - let it be a setext heading
            }
        } else {
            // No more newlines - remaining is a single line
            if remaining.trim().is_empty() || remaining.trim() == "---" {
                return None;
            }
            // Single line without newline - check if it looks like YAML
            let trimmed = remaining.trim();
            let looks_like_yaml = trimmed.contains(':') 
                || trimmed.starts_with("- ") 
                || trimmed.starts_with("|") 
                || trimmed.starts_with(">")
                || trimmed.starts_with("---")
                || trimmed.starts_with("...");
            if !looks_like_yaml {
                return None;
            }
        }

        reader.advance_to_eol();
        let node_ref = arena.new_node(rushdown_lib::ast::CodeBlock::new(
            rushdown_lib::ast::CodeBlockKind::Fenced,
            None,
        ));
        ctx.insert(self.meta_node, node_ref);
        Some((node_ref, parser::State::NO_CHILDREN))
    }

    fn cont(
        &self,
        arena: &mut Arena,
        node_ref: NodeRef,
        reader: &mut rushdown_lib::text::BasicReader,
        _ctx: &mut parser::Context,
    ) -> Option<parser::State> {
        let (line_bytes, seg) = reader.peek_line_bytes()?;
        if line_bytes.starts_with(b"---") {
            reader.advance_to_eol();
            return None;
        }
        rushdown_lib::as_type_data_mut!(arena, node_ref, Block).append_source_line(seg);
        Some(parser::State::NO_CHILDREN)
    }

    fn close(
        &self,
        _arena: &mut Arena,
        _node_ref: NodeRef,
        _reader: &mut rushdown_lib::text::BasicReader,
        _ctx: &mut parser::Context,
    ) {
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
}

impl From<MetaParser> for AnyBlockParser {
    fn from(p: MetaParser) -> Self {
        AnyBlockParser::Extension(Box::new(p))
    }
}

// --- AST transformer ---

#[derive(Debug)]
struct MetaAstTransformer {
    meta_node: ContextKey<NodeRefValue>,
    options: MetaParserOptions,
}

impl MetaAstTransformer {
    pub fn new(reg: Rc<RefCell<ContextKeyRegistry>>, options: MetaParserOptions) -> Self {
        let meta_node = reg.borrow_mut().get_or_create::<NodeRefValue>(META_NODE);
        Self { meta_node, options }
    }
}

impl AstTransformer for MetaAstTransformer {
    fn transform(
        &self,
        arena: &mut Arena,
        doc_ref: NodeRef,
        reader: &mut rushdown_lib::text::BasicReader,
        ctx: &mut parser::Context,
    ) {
        let Some(meta_ref) = ctx.get(self.meta_node) else {
            return;
        };
        let mut yaml_data = String::new();
        let source = reader.source();

        for line in
            rushdown_lib::as_type_data!(arena, *meta_ref, Block).source()
        {
            yaml_data.push_str(&line.str(source));
        }
        meta_ref.delete(arena);

        // If no YAML content was captured (e.g., thematic break `---` was
        // incorrectly consumed), silently skip without inserting an error.
        if yaml_data.trim().is_empty() {
            return;
        }

        match parse_yaml(&yaml_data) {
            Ok(Meta::Mapping(map)) => {
                let m = map.clone();
                for (key, value) in map {
                    rushdown_lib::as_kind_data_mut!(arena, doc_ref, Document)
                        .metadata_mut()
                        .insert(key, value);
                }
                if self.options.table {
                    render_meta_as_table(arena, doc_ref, m);
                }
            }
            Ok(_other) => {
                // YAML parsed but wasn't a mapping (e.g., a bare list)
                let mut error_data =
                    rushdown_lib::ast::HtmlBlock::new(rushdown_lib::ast::HtmlBlockKind::Kind2);
                error_data.set_value(
                    "<!-- YAML metadata must be a mapping -->\n".to_string(),
                );
                let error_ref = arena.new_node(error_data);
                if let Some(first) = arena[doc_ref].first_child() {
                    doc_ref.insert_before(arena, first, error_ref);
                } else {
                    doc_ref.append_child(arena, error_ref);
                }
            }
            Err(e) => {
                let mut error_data =
                    rushdown_lib::ast::HtmlBlock::new(rushdown_lib::ast::HtmlBlockKind::Kind2);
                error_data.set_value(
                    format!("<!-- Error parsing YAML metadata: {} -->\n", e).to_string(),
                );
                let error_ref = arena.new_node(error_data);
                if let Some(first) = arena[doc_ref].first_child() {
                    doc_ref.insert_before(arena, first, error_ref);
                } else {
                    doc_ref.append_child(arena, error_ref);
                }
            }
        }
    }
}

impl From<MetaAstTransformer> for AnyAstTransformer {
    fn from(t: MetaAstTransformer) -> Self {
        AnyAstTransformer::Extension(Box::new(t))
    }
}

/// Render metadata as an HTML table node in the AST.
fn render_meta_as_table(arena: &mut Arena, doc_ref: NodeRef, map: StringMap<Meta>) {
    use rushdown_lib::ast::{Table, TableBody, TableCell, TableHeader, TableRow};

    let table_ref = arena.new_node(Table::new());
    let header_ref = arena.new_node(TableHeader::new());
    let header_row_ref = arena.new_node(TableRow::new());

    for (key, _) in map.iter() {
        let cell_ref = arena.new_node(TableCell::default());
        let text_ref = arena.new_node(rushdown_lib::ast::Text::new(key.clone()));
        cell_ref.append_child(arena, text_ref);
        header_row_ref.append_child(arena, cell_ref);
    }
    header_ref.append_child(arena, header_row_ref);
    table_ref.append_child(arena, header_ref);

    let body_ref = arena.new_node(TableBody::new());
    table_ref.append_child(arena, body_ref);
    let body_row_ref = arena.new_node(TableRow::new());

    for (_, value) in map {
        let cell_ref = arena.new_node(TableCell::default());
        let text_ref = arena.new_node(rushdown_lib::ast::Text::new(format_meta_value(&value)));
        cell_ref.append_child(arena, text_ref);
        body_row_ref.append_child(arena, cell_ref);
    }
    body_ref.append_child(arena, body_row_ref);

    if let Some(first) = arena[doc_ref].first_child() {
        doc_ref.insert_before(arena, first, table_ref);
    } else {
        doc_ref.append_child(arena, table_ref);
    }
}

// --- Extension factory ---

/// Returns a parser extension that parses YAML frontmatter.
pub fn meta_parser_extension(options: impl Into<MetaParserOptions>) -> impl ParserExtension {
    ParserExtensionFn::new(|p: &mut Parser| {
        p.add_block_parser(
            MetaParser::new,
            NoParserOptions,
            PRIORITY_SETTEXT_HEADING - 100,
        );
        p.add_ast_transformer(
            MetaAstTransformer::new,
            options.into(),
            0,
        );
    })
}

// --- Unit Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_with_meta(source: &str) -> (Arena, NodeRef) {
        let ext = meta_parser_extension(MetaParserOptions::default());
        let parser = Parser::with_extensions(
            parser::Options::default(),
            ext,
        );
        let mut reader = rushdown_lib::text::BasicReader::new(source);
        parser.parse(&mut reader)
    }

    #[test]
    fn test_simple_frontmatter() {
        let (arena, doc_ref) = parse_with_meta("---\ntitle: Test\n---\n\nBody");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(!meta.is_empty(), "Metadata should not be empty");
            assert!(meta.contains_key("title"), "Should have 'title' key");
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_no_frontmatter() {
        let (arena, doc_ref) = parse_with_meta("No frontmatter here");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(meta.is_empty(), "Metadata should be empty");
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_thematic_break_not_consumed() {
        // A bare `---` should be parsed as a thematic break, not frontmatter
        let (arena, doc_ref) = parse_with_meta("---");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(meta.is_empty(), "Metadata should be empty for thematic break");
        } else {
            panic!("Expected Document node");
        }
        // Verify thematic break exists in children
        let mut child = arena[doc_ref].first_child();
        let mut found_hr = false;
        while let Some(nref) = child {
            if matches!(arena[nref].kind_data(), rushdown_lib::ast::KindData::ThematicBreak(_)) {
                found_hr = true;
            }
            child = arena[nref].next_sibling();
        }
        assert!(found_hr, "Should have a ThematicBreak child");
    }

    #[test]
    fn test_five_dashes_not_consumed() {
        // Five dashes is a thematic break, not frontmatter
        let (arena, doc_ref) = parse_with_meta("-----");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(meta.is_empty(), "Metadata should be empty for five dashes");
        } else {
            panic!("Expected Document node");
        }
        // Verify thematic break exists
        let mut child = arena[doc_ref].first_child();
        let mut found_hr = false;
        while let Some(nref) = child {
            if matches!(arena[nref].kind_data(), rushdown_lib::ast::KindData::ThematicBreak(_)) {
                found_hr = true;
            }
            child = arena[nref].next_sibling();
        }
        assert!(found_hr, "Should have a ThematicBreak child for five dashes");
    }

    #[test]
    fn test_nested_mapping() {
        let source = "---\nauthor:\n  name: Jane\n  age: 30\n---\n\nBody";
        let (arena, doc_ref) = parse_with_meta(source);
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(meta.contains_key("author"), "Should have 'author' key");
            if let Some(Meta::Mapping(inner)) = meta.get("author") {
                assert_eq!(inner.get("name"), Some(&Meta::String("Jane".to_string())));
                assert_eq!(inner.get("age"), Some(&Meta::Int(30)));
            } else {
                panic!("Author should be a Mapping");
            }
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_sequence() {
        let source = "---\ntags:\n  - rust\n  - markdown\n---\n\nBody";
        let (arena, doc_ref) = parse_with_meta(source);
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(meta.contains_key("tags"), "Should have 'tags' key");
            if let Some(Meta::Sequence(items)) = meta.get("tags") {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], Meta::String("rust".to_string()));
                assert_eq!(items[1], Meta::String("markdown".to_string()));
            } else {
                panic!("Tags should be a Sequence");
            }
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_all_scalar_types() {
        let source = "---\nstr_val: hello\nint_val: 42\nfloat_val: 3.14\nbool_val: true\nnull_val: null\n---\n\nBody";
        let (arena, doc_ref) = parse_with_meta(source);
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert_eq!(meta.get("str_val"), Some(&Meta::String("hello".to_string())));
            assert_eq!(meta.get("int_val"), Some(&Meta::Int(42)));
            assert_eq!(meta.get("float_val"), Some(&Meta::Float(3.14)));
            assert_eq!(meta.get("bool_val"), Some(&Meta::Bool(true)));
            assert_eq!(meta.get("null_val"), Some(&Meta::Null));
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_empty_frontmatter() {
        // Empty frontmatter should not crash
        let (arena, doc_ref) = parse_with_meta("---\n---\n\nBody");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(meta.is_empty(), "Empty frontmatter should produce empty metadata");
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_frontmatter_with_dash_in_string() {
        // YAML string containing --- should not confuse the parser
        let source = "---\nbody: |\n  text with --- inside\n---\n\nBody";
        let (arena, doc_ref) = parse_with_meta(source);
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(meta.contains_key("body"), "Should have 'body' key");
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_thematic_break_with_blank_line() {
        // ---\n\nHello should be thematic break, not frontmatter
        let (arena, doc_ref) = parse_with_meta("---\n\nHello");
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(meta.is_empty(), "Should not parse as frontmatter");
        } else {
            panic!("Expected Document node");
        }
        // Verify thematic break exists
        let mut child = arena[doc_ref].first_child();
        let mut found_hr = false;
        while let Some(nref) = child {
            if matches!(arena[nref].kind_data(), rushdown_lib::ast::KindData::ThematicBreak(_)) {
                found_hr = true;
            }
            child = arena[nref].next_sibling();
        }
        assert!(found_hr, "Should have a ThematicBreak child");
    }

    #[test]
    fn test_multiple_frontmatter_keys() {
        let source = "---\ntitle: Doc\nauthor: Jane\ndate: 2024-01-15\ntags:\n  - a\n  - b\n---\n\nBody";
        let (arena, doc_ref) = parse_with_meta(source);
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert!(meta.contains_key("title"));
            assert!(meta.contains_key("author"));
            assert!(meta.contains_key("date"));
            assert!(meta.contains_key("tags"));
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_original_test_meta_full_frontmatter() {
        // Original rushdown-meta test_meta: full frontmatter with nested structures
        let source = "---\ntitle: YAML Frontmatter\ndate: 2026-03-11\ntags: [Rust, Markdown<>]\nauthor:\n  name: yuin\n---\naaa\n";
        let (arena, doc_ref) = parse_with_meta(source);
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert_eq!(meta.get("title"), Some(&Meta::String("YAML Frontmatter".to_string())));
            assert_eq!(meta.get("date"), Some(&Meta::String("2026-03-11".to_string())));
            if let Some(Meta::Sequence(tags)) = meta.get("tags") {
                assert_eq!(tags.len(), 2);
                assert_eq!(tags[0], Meta::String("Rust".to_string()));
                assert_eq!(tags[1], Meta::String("Markdown<>".to_string()));
            } else {
                panic!("Tags should be a Sequence");
            }
            if let Some(Meta::Mapping(author)) = meta.get("author") {
                assert_eq!(author.get("name"), Some(&Meta::String("yuin".to_string())));
            } else {
                panic!("Author should be a Mapping");
            }
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_original_test_ok_simple() {
        // Original rushdown-meta test_ok: simple frontmatter
        let source = "---\ntitle: YAML Frontmatter\n---\naaa\n";
        let (arena, doc_ref) = parse_with_meta(source);
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert_eq!(meta.get("title"), Some(&Meta::String("YAML Frontmatter".to_string())));
        } else {
            panic!("Expected Document node");
        }
    }

    #[test]
    fn test_table_option() {
        // Verify the table option renders metadata as an HTML table
        let ext = meta_parser_extension(MetaParserOptions { table: true });
        let parser = Parser::with_extensions(
            parser::Options::default(),
            ext,
        );
        let source = "---\ntitle: Test\n---\nBody\n";
        let mut reader = rushdown_lib::text::BasicReader::new(source);
        let (arena, doc_ref) = parser.parse(&mut reader);
        let kd = &arena[doc_ref].kind_data();
        if let rushdown_lib::ast::KindData::Document(doc) = kd {
            let meta = doc.metadata();
            assert_eq!(meta.get("title"), Some(&Meta::String("Test".to_string())));
            // With table option, a Table node should be inserted as first child
            let first_child = arena[doc_ref].first_child();
            assert!(first_child.is_some(), "Should have a first child (table)");
            if let Some(table_ref) = first_child {
                assert!(matches!(
                    arena[table_ref].kind_data(),
                    rushdown_lib::ast::KindData::Table(_)
                ), "First child should be a Table node");
            }
        } else {
            panic!("Expected Document node");
        }
    }
}
