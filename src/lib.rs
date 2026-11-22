#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod ast;
pub mod context;
pub mod extensions;
pub mod parser;
pub mod renderer;
pub mod test;
pub mod text;
pub mod util;

#[cfg(feature = "meta")]
pub mod meta;

#[cfg(feature = "emoji")]
pub mod emoji;

#[cfg(feature = "footnotes")]
pub mod footnote;

#[cfg(feature = "definition-list")]
pub mod definition_list;

#[cfg(feature = "fenced-div")]
pub mod fenced_div;

#[cfg(feature = "link-attributes")]
pub mod link_attributes;

#[cfg(feature = "linter")]
pub mod linter;

#[cfg(feature = "diagram")]
pub mod mermaid_theme;

#[cfg(feature = "diagram")]
pub mod diagram;

#[cfg(feature = "chunker")]
pub mod chunker;

#[cfg(feature = "math")]
pub mod math;

#[cfg(feature = "highlighter")]
pub mod vscode_theme;

#[cfg(feature = "highlighter")]
pub mod highlighter;

#[cfg(feature = "html-entities")]
mod html_entity;

mod scanner;

mod error;
use alloc::string::String;

pub use error::{Error, Result};

#[doc(inline)]
pub use extensions::{Engine, Engines};

// Brings `apply` into scope for the renderer extensions below. Needed only
// when at least one renderer-side engine can be enabled.
#[cfg(any(
    feature = "emoji",
    feature = "diagram",
    feature = "math",
    feature = "footnotes",
    feature = "definition-list",
    feature = "fenced-div",
    feature = "highlighter"
))]
use renderer::html::RendererExtension;

use crate::{
    parser::{Parser, ParserExtension},
    renderer::{html, TextWrite},
    text::BasicReader,
};

/// Trait for converting Markdown to HTML.
///
/// # Errors
/// Parsing phase will never fail, so the only possible errors are I/O errors during rendering.
pub trait MarkdownToHtml<W: TextWrite = String> {
    /// Converts the given Markdown source to HTML and writes it to the output.
    fn markdown_to_html(&self, out: &mut W, source: &str) -> Result<()>;
}

impl<W: TextWrite, F> MarkdownToHtml<W> for F
where
    F: Fn(&mut W, &str) -> Result<()>,
{
    fn markdown_to_html(&self, out: &mut W, source: &str) -> Result<()> {
        (self)(out, source)
    }
}

/// Creates a function that converts Markdown to HTML using the specified parser and renderer.
///
/// # Arguments
/// - `parser_options`: Options for the Markdown parser.
/// - `renderer_options`: Options for the HTML renderer.
/// - `parser_extension`: Extension for the Markdown parser. If no extensions are needed, use [`crate::parser::NO_EXTENSIONS`].
/// - `renderer_extension`: Extension for the HTML renderer. If no extensions are needed, use [`crate::renderer::html::NO_EXTENSIONS`].
///
/// # Examples
/// ```
/// use core::fmt::Write;
/// use mordant::{
///     new_markdown_to_html,
///     parser::{self, ParserExtension},
///     renderer::html::{self, RendererExtension},
///     Result,
/// };
///
/// let markdown_to_html = new_markdown_to_html(
///     parser::Options::default(),
///     html::Options::default(),
///     parser::gfm_table().and(parser::gfm_task_list_item()),
///     html::NO_EXTENSIONS,
/// );
/// let mut output = String::new();
/// let input = "# Hello, World!\n\nThis is a **Markdown** document.";
/// match markdown_to_html(&mut output, input) {
///     Ok(_) => {
///         println!("HTML output:\n{}", output);
///     }
///     Err(e) => {
///         println!("Error: {:?}", e);
///     }
/// }
/// ```
pub fn new_markdown_to_html<'r, W>(
    parser_options: parser::Options,
    renderer_options: html::Options,
    parser_extension: impl ParserExtension,
    renderer_extension: impl html::RendererExtension<'r, W>,
) -> impl Fn(&mut W, &str) -> Result<()> + 'r
where
    W: TextWrite + 'r,
{
    let parser = Parser::with_extensions(parser_options, parser_extension);
    let renderer = html::Renderer::<'r, W>::with_extensions(renderer_options, renderer_extension);
    move |output: &mut W, source: &str| {
        let mut reader = BasicReader::new(source);
        let (arena, document_ref) = parser.parse(&mut reader);
        renderer.render(output, source, &arena, document_ref)
    }
}

/// Creates a function that converts Markdown to HTML using the specified parser and renderer,
/// with output written to a `String`.
pub fn new_markdown_to_html_string<'r>(
    parser_options: parser::Options,
    renderer_options: html::Options,
    parser_extension: impl ParserExtension,
    renderer_extension: impl html::RendererExtension<'r, String>,
) -> impl Fn(&mut String, &str) -> Result<()> + 'r {
    new_markdown_to_html::<String>(
        parser_options,
        renderer_options,
        parser_extension,
        renderer_extension,
    )
}

/// Converts Markdown(CommonMark) to HTML using default parser and renderer options.
///
/// # Examples
/// ```
/// use mordant::markdown_to_html_string;
/// let mut output = String::new();
/// let input = "# Hello, World!\n\nThis is a **Markdown** document.";
/// match markdown_to_html_string(&mut output, input) {
///     Ok(_) => {
///         println!("HTML output:\n{}", output);
///     }
///     Err(e) => {
///     println!("Error: {:?}", e);
///     }
///  };
///  ```
pub fn markdown_to_html_string(output: &mut String, source: &str) -> Result<()> {
    let parser = Parser::with_options(parser::Options::default());
    let renderer = html::Renderer::with_options(html::Options::default());
    let mut reader = BasicReader::new(source);
    let (arena, document_ref) = parser.parse(&mut reader);
    renderer.render(output, source, &arena, document_ref)
}

/// Returns the built-in parser extensions for the given engine set.
///
/// This crate-internal factory is shared by the Markdown-to-HTML entry point
/// and the chunker so their engine gates cannot drift. GFM extensions are
/// composed separately by callers.
pub(crate) fn parser_extensions(engines: Engines) -> impl ParserExtension {
    parser::ParserExtensionFn::new(move |p: &mut Parser| {
        #[cfg(not(any(
            feature = "meta",
            feature = "emoji",
            feature = "footnotes",
            feature = "diagram",
            feature = "math",
            feature = "definition-list",
            feature = "fenced-div",
            feature = "link-attributes"
        )))]
        let _ = (p, engines);
        #[cfg(feature = "meta")]
        if engines.is_enabled(Engine::Meta) {
            crate::meta::meta_parser_extension(crate::meta::MetaParserOptions::default()).apply(p);
        }
        #[cfg(feature = "emoji")]
        if engines.is_enabled(Engine::Emoji) {
            crate::emoji::emoji_parser_extension(crate::emoji::EmojiParserOptions::default())
                .apply(p);
        }
        #[cfg(feature = "footnotes")]
        if engines.is_enabled(Engine::Footnotes) {
            crate::footnote::footnote_parser_extension().apply(p);
        }
        #[cfg(feature = "diagram")]
        if engines.is_enabled(Engine::Diagram) {
            crate::diagram::diagram_parser_extension(
                crate::diagram::DiagramParserOptions::default(),
            )
            .apply(p);
        }
        #[cfg(feature = "math")]
        if engines.is_enabled(Engine::Math) {
            crate::math::math_parser_extension(crate::math::MathParserOptions::default()).apply(p);
        }
        #[cfg(feature = "definition-list")]
        if engines.is_enabled(Engine::DefinitionList) {
            crate::definition_list::definition_list_parser_extension().apply(p);
        }
        #[cfg(feature = "fenced-div")]
        if engines.is_enabled(Engine::FencedDiv) {
            crate::fenced_div::fenced_div_parser_extension().apply(p);
        }
        #[cfg(feature = "link-attributes")]
        if engines.is_enabled(Engine::LinkAttributes) {
            crate::link_attributes::link_attributes_parser_extension().apply(p);
        }
    })
}

/// Converts Markdown to HTML using the built-in engines selected by the
/// `engines` field of the supplied options.
///
/// All compiled-in engines except syntax highlighting are enabled by default
/// ([`Engines::default()`]); pass [`Engines::all()`] to enable highlighting.
/// Disable an engine by setting the corresponding flag on
/// [`parser::Options::engines`] (parse side) and [`html::Options::engines`]
/// (render side) to `false`. GFM remains a separate opt-in extension; it is
/// not part of `Engines`.
///
/// # Examples
/// ```
/// # #[cfg(feature = "footnotes")] {
/// use mordant::{
///     markdown_to_html_string_with_options, parser, renderer::html, Engines, Engine,
/// };
///
/// let mut output = String::new();
/// let mut parser_opts = parser::Options::default();
/// parser_opts.engines = Engines::only(&[Engine::Footnotes]);
/// let mut html_opts = html::Options::default();
/// html_opts.engines = parser_opts.engines;
/// markdown_to_html_string_with_options(
///     &mut output,
///     "text[^1]\n\n[^1]: note",
///     parser_opts,
///     html_opts,
/// )
/// .unwrap();
/// assert!(output.contains("footnote-ref"));
/// # }
/// ```
///
/// # Errors
///
/// Returns an error if the underlying writer or a renderer callback returns
/// an error.
pub fn markdown_to_html_string_with_options(
    output: &mut String,
    source: &str,
    parser_options: parser::Options,
    renderer_options: html::Options,
) -> Result<()> {
    let parser_engines = parser_options.engines;
    let renderer_engines = renderer_options.engines;

    let parser = Parser::with_extensions(parser_options, parser_extensions(parser_engines));

    let renderer = html::Renderer::with_extensions(
        renderer_options,
        html::renderer_extension(move |r: &mut html::Renderer<'_, String>| {
            #[cfg(not(any(
                feature = "emoji",
                feature = "diagram",
                feature = "math",
                feature = "footnotes",
                feature = "definition-list",
                feature = "fenced-div",
                feature = "highlighter"
            )))]
            let _ = (r, renderer_engines);
            #[cfg(feature = "emoji")]
            if renderer_engines.is_enabled(Engine::Emoji) {
                crate::emoji::emoji_html_renderer_extension(
                    crate::emoji::EmojiHtmlRendererOptions::default(),
                )
                .apply(r);
            }
            #[cfg(feature = "diagram")]
            if renderer_engines.is_enabled(Engine::Diagram) {
                crate::diagram::diagram_html_renderer_extension(
                    crate::diagram::DiagramHtmlRendererOptions::default(),
                )
                .apply(r);
            }
            #[cfg(feature = "math")]
            if renderer_engines.is_enabled(Engine::Math) {
                crate::math::math_html_renderer_extension(
                    crate::math::MathRendererOptions::default(),
                )
                .apply(r);
                crate::math::math_inline_html_renderer_extension(
                    crate::math::MathInlineRendererOptions::default(),
                )
                .apply(r);
            }
            #[cfg(feature = "footnotes")]
            if renderer_engines.is_enabled(Engine::Footnotes) {
                crate::footnote::footnote_html_renderer_extension(
                    crate::footnote::FootnoteHtmlRendererOptions::default(),
                )
                .apply(r);
            }
            #[cfg(feature = "definition-list")]
            if renderer_engines.is_enabled(Engine::DefinitionList) {
                crate::definition_list::definition_list_html_renderer_extension(
                    crate::definition_list::DefinitionListHtmlRendererOptions::default(),
                )
                .apply(r);
            }
            #[cfg(feature = "fenced-div")]
            if renderer_engines.is_enabled(Engine::FencedDiv) {
                crate::fenced_div::fenced_div_html_renderer_extension(
                    crate::fenced_div::FencedDivHtmlRendererOptions::default(),
                )
                .apply(r);
            }
            #[cfg(feature = "highlighter")]
            if renderer_engines.is_enabled(Engine::Highlighter) {
                crate::highlighter::highlighting_html_renderer_extension(
                    crate::highlighter::HighlightingRendererOptions::default(),
                )
                .apply(r);
            }
        }),
    );

    let mut reader = BasicReader::new(source);
    let (arena, document_ref) = parser.parse(&mut reader);
    renderer.render(output, source, &arena, document_ref)
}

/// Returns the set of built-in engines that were compiled into this crate.
pub fn available_engines() -> &'static [Engine] {
    const AVAILABLE: &[Engine] = &[
        #[cfg(feature = "meta")]
        Engine::Meta,
        #[cfg(feature = "emoji")]
        Engine::Emoji,
        #[cfg(feature = "footnotes")]
        Engine::Footnotes,
        #[cfg(feature = "linter")]
        Engine::Linter,
        #[cfg(feature = "diagram")]
        Engine::Diagram,
        #[cfg(feature = "chunker")]
        Engine::Chunker,
        #[cfg(feature = "math")]
        Engine::Math,
        #[cfg(feature = "highlighter")]
        Engine::Highlighter,
        #[cfg(feature = "definition-list")]
        Engine::DefinitionList,
        #[cfg(feature = "fenced-div")]
        Engine::FencedDiv,
        #[cfg(feature = "link-attributes")]
        Engine::LinkAttributes,
    ];
    AVAILABLE
}

/// Returns true if the given engine was compiled into this crate.
pub fn is_engine_available(engine: Engine) -> bool {
    Engines::all().is_enabled(engine)
}

// macros {{{

/// Helper macro to match kind data.
///
/// # Examples
/// ```
/// use mordant::ast::{Arena, NodeRef, KindData, Paragraph};
/// use mordant::matches_kind;
///
/// let mut arena = Arena::new();
/// let para_ref: NodeRef = arena.new_node(Paragraph::new());
/// assert!(matches_kind!(arena, para_ref, Paragraph));
/// assert!(matches_kind!(arena[para_ref], Paragraph));
/// ```
#[macro_export]
macro_rules! matches_kind {
    ($arena:expr, $node_ref:expr, $variant:ident) => {
        matches!(
            $arena[$node_ref].kind_data(),
            $crate::ast::KindData::$variant(_)
        )
    };
    ($node:expr, $variant:ident) => {
        matches!($node.kind_data(), $crate::ast::KindData::$variant(_))
    };
}

/// Helper macro to match extension kind.
///
/// # Examples
/// ```
/// use core::fmt::{self, Write};
/// use mordant::ast::{Arena, NodeRef, NodeType, NodeKind, KindData, PrettyPrint, pp_indent};
/// use mordant::matches_extension_kind;
///
/// #[derive(Debug)]
/// struct Admonition {
///     kind: String,
/// }
///
/// impl NodeKind for Admonition {
///     fn typ(&self) -> NodeType { NodeType::ContainerBlock }
///
///     fn kind_name(&self) -> &'static str { "Admonition" }
/// }
///
/// impl PrettyPrint for Admonition {
///     fn pretty_print(&self, w: &mut dyn Write, _source: &str, level: usize) -> fmt::Result {
///         writeln!(w, "{}kind: {}", pp_indent(level), self.kind)
///     }
/// }
///
/// impl From<Admonition> for KindData {
///     fn from(e: Admonition) -> Self { KindData::Extension(Box::new(e)) }
/// }
///
/// let mut arena = Arena::new();
/// let ext_ref: NodeRef = arena.new_node(Admonition{kind: "note".to_string()});
/// assert!(matches_extension_kind!(arena, ext_ref, Admonition));
/// assert!(matches_extension_kind!(arena[ext_ref], Admonition));
/// ```
///
#[macro_export]
macro_rules! matches_extension_kind {
    ($arena:expr, $ref:expr, $ext_type:ty) => {
        (if let $crate::ast::KindData::Extension(ref d) = $arena[$ref].kind_data() {
            (d.as_ref() as &dyn ::core::any::Any)
                .downcast_ref::<$ext_type>()
                .is_some()
        } else {
            false
        })
    };
    ($node:expr, $ext_type:ty) => {
        (if let $crate::ast::KindData::Extension(ref d) = $node.kind_data() {
            (d.as_ref() as &dyn ::core::any::Any)
                .downcast_ref::<$ext_type>()
                .is_some()
        } else {
            false
        })
    };
}

/// Helper macro to downcast extension data.
///
/// # Examples
/// ```
/// use core::fmt::{self, Write};
/// use mordant::ast::{Arena, NodeRef, NodeType, NodeKind, KindData, PrettyPrint, pp_indent};
/// use mordant::as_extension_data;
///
/// #[derive(Debug)]
/// struct Admonition {
///     kind: String,
/// }
///
/// impl NodeKind for Admonition {
///     fn typ(&self) -> NodeType { NodeType::ContainerBlock }
///
///     fn kind_name(&self) -> &'static str { "Admonition" }
/// }
///
/// impl PrettyPrint for Admonition {
///     fn pretty_print(&self, w: &mut dyn Write, _source: &str, level: usize) -> fmt::Result {
///         writeln!(w, "{}kind: {}", pp_indent(level), self.kind)
///     }
/// }
///
/// impl From<Admonition> for KindData {
///     fn from(e: Admonition) -> Self { KindData::Extension(Box::new(e)) }
/// }
///
/// let mut arena = Arena::new();
/// let ext_ref: NodeRef = arena.new_node(Admonition{kind: "note".to_string()});
/// let ext_data = as_extension_data!(arena, ext_ref, Admonition);
/// assert_eq!(ext_data.kind, "note");
/// let ext_data = as_extension_data!(arena[ext_ref], Admonition);
/// assert_eq!(ext_data.kind, "note");
/// ```
///
#[macro_export]
macro_rules! as_extension_data {
    ($arena:expr, $ref:expr, $ext_type:ty) => {
        (if let $crate::ast::KindData::Extension(ref d) = $arena[$ref].kind_data() {
            (d.as_ref() as &dyn ::core::any::Any)
                .downcast_ref::<$ext_type>()
                .expect("Failed to downcast extension data")
        } else {
            panic!("Node is not an extension node")
        })
    };
    ($node:expr, $ext_type:ty) => {
        (if let $crate::ast::KindData::Extension(ref d) = $node.kind_data() {
            (d.as_ref() as &dyn ::core::any::Any)
                .downcast_ref::<$ext_type>()
                .expect("Failed to downcast extension data")
        } else {
            panic!("Node is not an extension node")
        })
    };
}

/// Helper macro to downcast mutable extension data.
///
/// See [`as_extension_data!`] for examples.
#[macro_export]
macro_rules! as_extension_data_mut {
    ($arena:expr, $ref:expr, $ext_type:ty) => {
        (if let $crate::ast::KindData::Extension(ref mut d) = $arena[$ref].kind_data_mut() {
            (d.as_mut() as &mut dyn ::core::any::Any)
                .downcast_mut::<$ext_type>()
                .expect("Failed to downcast extension data")
        } else {
            panic!("Node is not an extension node")
        })
    };
    ($node:expr, $ext_type:ty) => {
        (if let $crate::ast::KindData::Extension(ref mut d) = $node.kind_data_mut() {
            (d.as_mut() as &mut dyn ::core::any::Any)
                .downcast_mut::<$ext_type>()
                .expect("Failed to downcast extension data")
        } else {
            panic!("Node is not an extension node")
        })
    };
}

/// Helper macro to work with kind data.
///
/// # Examples
/// ```
/// use mordant::ast::{Arena, NodeRef, KindData, List};
/// use mordant::as_kind_data;
///
/// let mut arena = Arena::new();
/// let para_ref: NodeRef = arena.new_node(List::new(b'-'));
/// let data = as_kind_data!(arena, para_ref, List);
/// assert_eq!(data.marker(), b'-');
/// let data = as_kind_data!(arena[para_ref], List);
/// assert_eq!(data.marker(), b'-');
/// ```
#[macro_export]
macro_rules! as_kind_data {
    ($arena:expr, $node_ref:expr, $variant:ident) => {
        (if let $crate::ast::KindData::$variant(ref d) = $arena[$node_ref].kind_data() {
            d
        } else {
            panic!(
                "Expected kind data variant {} but found {:?}",
                stringify!($variant),
                $arena[$node_ref].kind_data()
            )
        })
    };
    ($node:expr, $variant:ident) => {
        (if let $crate::ast::KindData::$variant(ref d) = $node.kind_data() {
            d
        } else {
            panic!(
                "Expected kind data variant {} but found {:?}",
                stringify!($variant),
                $node.kind_data()
            )
        })
    };
}

/// Helper macro to work with mutable kind data.
///
/// See [`as_kind_data!`] for examples.
#[macro_export]
macro_rules! as_kind_data_mut {
    ($arena:expr, $node_ref:expr, $variant:ident) => {
        (if let $crate::ast::KindData::$variant(ref mut d) = $arena[$node_ref].kind_data_mut() {
            d
        } else {
            panic!(
                "Expected kind data variant {} but found {:?}",
                stringify!($variant),
                $arena[$node_ref].kind_data()
            )
        })
    };
    ($node:expr, $variant:ident) => {
        (if let $crate::ast::KindData::$variant(ref mut d) = $node.kind_data_mut() {
            d
        } else {
            panic!(
                "Expected kind data variant {} but found {:?}",
                stringify!($variant),
                $node.kind_data()
            )
        })
    };
}

/// Helper macro to work with type data.
///
/// # Examples
/// ```
/// use mordant::ast::{Arena, NodeRef, TypeData, Block, Paragraph};
/// use mordant::as_type_data;
///
/// let mut arena = Arena::new();
/// let para_ref: NodeRef = arena.new_node(Paragraph::new());
/// let data = as_type_data!(arena, para_ref, Block);
/// assert!(data.source().is_empty());
/// let data = as_type_data!(arena[para_ref], Block);
/// assert!(data.source().is_empty());
/// ```
///
#[macro_export]
macro_rules! as_type_data {
    ($arena:expr, $node_ref:expr, $variant:ident) => {
        (if let $crate::ast::TypeData::$variant(ref d) = $arena[$node_ref].type_data() {
            d
        } else {
            panic!(
                "Expected type data variant {} but found {:?}",
                stringify!($variant),
                $arena[$node_ref].type_data()
            )
        })
    };
    ($node:expr, $variant:ident) => {
        (if let $crate::ast::TypeData::$variant(ref d) = $node.type_data() {
            d
        } else {
            panic!(
                "Expected type data variant {} but found {:?}",
                stringify!($variant),
                $node.type_data()
            )
        })
    };
}

/// Helper macro to work with mutable type data.
///
/// See [`as_type_data!`] for examples.
#[macro_export]
macro_rules! as_type_data_mut {
    ($arena:expr, $node_ref:expr, $variant:ident) => {
        (if let $crate::ast::TypeData::$variant(ref mut d) = $arena[$node_ref].type_data_mut() {
            d
        } else {
            panic!(
                "Expected type data variant {} but found {:?}",
                stringify!($variant),
                $arena[$node_ref].type_data()
            )
        })
    };
    ($node:expr, $variant:ident) => {
        (if let $crate::ast::TypeData::$variant(ref mut d) = $node.type_data_mut() {
            d
        } else {
            panic!(
                "Expected type data variant {} but found {:?}",
                stringify!($variant),
                $node.type_data()
            )
        })
    };
}

/// Helper macro to construct an AST.
///
/// # Examples
/// ```rust
/// use mordant::md_ast;
/// use mordant::ast::*;
/// use mordant::renderer::html;
///
/// let mut arena = Arena::new();
/// let doc = md_ast!(&mut arena, Document::new() => {
///     Blockquote::new() => {
///         Paragraph::new(); { |node: &mut Node| {
///             node.attributes_mut().insert("class", "paragraph".into());
///         } } => {
///             Text::new("Hello, World!")
///         },
///         Paragraph::new() => {
///             Text::new("This is a test.")
///         }
///     }
/// });
/// let renderer = html::Renderer::with_options(html::Options::default());
/// let mut output = String::new();
/// renderer.render(&mut output, "", &arena, doc).expect("Failed to render HTML");
/// assert_eq!(output, "<blockquote>\n<p class=\"paragraph\">Hello, World!</p>\n<p>This is a test.</p>\n</blockquote>\n");
/// ```
#[macro_export]
macro_rules! md_ast {
    ($arena:expr, $root:expr => { $($children:tt)* }) => {{
        let __root = $arena.new_node($root);
        md_ast!(@children $arena, __root, { $($children)* });
        __root
    }};

    // make a node, optional post hook written as: ;{ <expr> }
    (@mk $arena:expr, $spec:expr) => {{
        $arena.new_node($spec)
    }};
    (@mk $arena:expr, $spec:expr, @{ $post:expr }) => {{
        let __n = $arena.new_node($spec);
        ($post)(&mut $arena[__n]);
        __n
    }};

    (@children $arena:expr, $parent:ident, { }) => {};

    // child with grandchildren
    (@children $arena:expr, $parent:ident, {
        $child:expr $( ;{ $post:expr } )? => { $($grand:tt)* } $(, $($rest:tt)*)?
    }) => {{
        let __child = md_ast!(@mk $arena, $child $(, @{ $post })?);
        $parent.append_child($arena, __child);

        md_ast!(@children $arena, __child, { $($grand)* });
        md_ast!(@children $arena, $parent, { $($($rest)*)? });
    }};

    // leaf child
    (@children $arena:expr, $parent:ident, {
        $child:expr $( ;{ $post:expr } )? $(, $($rest:tt)*)?
    }) => {{
        let __child = md_ast!(@mk $arena, $child $(, ;{ $post })?);
        $parent.append_child($arena, __child);

        md_ast!(@children $arena, $parent, { $($($rest)*)? });
    }};
}

/// Helper macro to traverse the AST by calling methods on nodes.
///
/// # Examples
/// ```rust
/// use mordant::md_ast;
/// use mordant::node_path;
/// use mordant::as_kind_data;
/// use mordant::ast::*;
/// use mordant::renderer::html;
///
/// let mut arena = Arena::new();
/// let doc = md_ast!(&mut arena, Document::new() => {
///     Blockquote::new() => {
///         Paragraph::new(); { |node: &mut Node| {
///             node.attributes_mut().insert("class", "paragraph".into());
///         } } => {
///             Text::new("Hello, World!")
///         },
///         Paragraph::new() => {
///             Text::new("This is a test.")
///         }
///     }
/// });
/// let text_opt = node_path!(arena, doc, first_child, first_child, first_child);
/// assert_eq!(as_kind_data!(arena, text_opt.unwrap(), Text).str(""), "Hello, World!");
/// ```
#[macro_export]
macro_rules! node_path {
    ($arena:expr, $start:expr $(, $method:ident )* ) => {{
        let mut node_opt = Some($start);
        $(
            node_opt = node_opt.and_then(|n| $arena[n].$method());
        )*
        node_opt
    }};
}

// }}} macros

// debug stuff {{{

#[cfg(not(feature = "std"))]
pub mod debug {
    #[cfg(all(feature = "no-std-unix-debug", unix))]
    extern crate libc;

    use core::fmt::{self, Write};

    #[allow(dead_code)]
    pub struct Stdout;

    impl Write for Stdout {
        fn write_str(&mut self, _s: &str) -> fmt::Result {
            #[cfg(all(feature = "no-std-unix-debug", unix))]
            {
                // SAFETY: `_s` is a valid `&str`, so its pointer is valid for
                // `_s.len()` bytes. `libc::write` only reads that range; fd 1
                // is stdout. Partial writes are ignored because this is a
                // debug-only path.
                unsafe {
                    libc::write(1, _s.as_ptr().cast(), _s.len());
                }
            }
            Ok(())
        }
    }

    #[macro_export]
    macro_rules! print {
        ($($arg:tt)*) => {{
            use core::fmt::Write;
            let mut out = $crate::debug::Stdout;
            core::write!(&mut out, $($arg)*).ok();
        }};
    }

    #[macro_export]
    macro_rules! println {
        ($($arg:tt)*) => {{
            $crate::print!("{}\n", format_args!($($arg)*));
        }};
    }
}
// }}}
