//! Regression tests for the 0.11 bug-fix plan (core crate).

#![cfg(all(
    feature = "definition-list",
    feature = "fenced-div",
    feature = "link-attributes"
))]

use mordant::extensions::{Engine, Engines};
use mordant::markdown_to_html_string_with_options;
use mordant::parser;
use mordant::renderer::html;

fn render(source: &str) -> String {
    let engines = Engines::all_without_highlighter();
    let mut parser_opts = parser::Options::default();
    parser_opts.engines = engines;
    let mut html_opts = html::Options::default();
    html_opts.engines = engines;
    let mut output = String::new();
    markdown_to_html_string_with_options(&mut output, source, parser_opts, html_opts).unwrap();
    output
}

#[test]
fn bf01_fenced_div_body_is_a_child() {
    assert_eq!(
        render("::: note\nHello *world*.\n:::\n"),
        "<div class=\"note\">\n<p>Hello <em>world</em>.</p>\n</div>\n"
    );
    let html = render("::: outer\n::: inner\nnested\n:::\n:::\n");
    assert_eq!(html.matches("<div").count(), 2, "{html}");
    assert!(html.contains("<p>nested</p>"), "{html}");
    let html = render("::: note\ntext\n\n:::\n");
    assert!(html.ends_with("</div>\n"), "{html}");
    assert!(!html.contains("<div>\n</div>"), "{html}");
}

#[test]
fn bf01_fenced_div_edge_cases() {
    assert_eq!(render("::: note\n:::\n"), "<div class=\"note\">\n</div>\n");
    assert_eq!(
        render("::: note\nbody\n"),
        "<div class=\"note\">\n<p>body</p>\n</div>\n"
    );
    let html = render("- item\n\n  ::: note\n  body\n  :::\n");
    assert!(html.contains("<div class=\"note\">"), "{html}");
    assert!(html.contains("<p>body</p>"), "{html}");
    let html = render("> ::: note\n> body\n> :::\n");
    assert!(html.contains("<div class=\"note\">"), "{html}");
    assert!(html.contains("<p>body</p>"), "{html}");
    let html = render("::: note\n    code\n:::\n");
    assert!(html.contains("<pre><code>code"), "{html}");
    let html = render("::: note\nbody\n    :::\n:::\n");
    assert_eq!(html.matches("<div").count(), 1, "{html}");
    assert!(html.contains(":::"), "{html}");
    let html = render("::: note\nbody\n::: {#end}\n");
    assert_eq!(html.matches("<div").count(), 1, "{html}");
    assert!(!html.contains("id=\"end\""), "{html}");
}

#[test]
fn bf01_fenced_div_fence_rules() {
    let html = render("::: note\n::::\nnot a close\n:::\n:::\n");
    assert!(html.contains("<p>not a close</p>"), "{html}");
    let html = render("::: note\ntext\n::::\n");
    assert!(html.contains("text"), "{html}");
    assert_eq!(html.matches("<div").count(), 1, "{html}");
}

#[test]
fn bf02_bf03_bf04_definition_lists() {
    let html = render("Term\n: a\nb\n");
    assert!(html.contains("<dd>\na\nb"), "{html}");
    let html = render("intro\nTerm\n: def\n");
    assert!(html.contains("intro"), "{html}");
    assert!(!html.contains("<dl>"), "{html}");
    let html = render("Apple\n: A fruit.\nOrange\n: Citrus fruit.\n");
    assert_eq!(html.matches("<dt>").count(), 2, "{html}");
    assert!(html.contains("A fruit."), "{html}");
    assert!(html.contains("Citrus fruit."), "{html}");
    let html = render("Term\r\n: a\r\nb\r\n");
    assert!(html.contains("<dd>\na\nb"), "{html}");
    let html = render("Term\n: one\n: two\n");
    assert_eq!(html.matches("<dd>").count(), 2, "{html}");
    let html = render("Term\n: a\ntrailing\n");
    assert!(html.contains("trailing"), "{html}");
    let html = render("Term\ncontinued\n: def\n");
    assert!(html.contains("continued"), "{html}");
    assert!(!html.contains("<dl>"), "{html}");
}

#[test]
fn bf08_emoji_colons_are_not_definitions() {
    for source in [
        "Hello\n:smile: world\n",
        "Hello\n:joy: world\n",
        "Hello\n::smile:: world\n",
        "Hello\n:def\n",
    ] {
        let html = render(source);
        assert!(!html.contains("<dl>"), "{source}: {html}");
    }
    let html = render("Term\n:  spaced\n");
    assert!(html.contains("spaced"), "{html}");
    let html = render("Term\n:\tfoo\n");
    assert!(html.contains("foo"), "{html}");
}

#[test]
fn bf05_link_attributes_split_by_inline_parsers() {
    for (source, expected) in [
        ("[a](x){#id .cls target=\"_blank\"}", "target=\"_blank\""),
        ("[a](x){.my_class}", "class=\"my_class\""),
        ("[a](x){title=\"a:b\"}", "title=\"a:b\""),
        ("[a](x){title=\"a*b\"}", "title=\"a*b\""),
        ("[a](x){title=\"a`b\"}", "title=\"a`b\""),
    ] {
        let html = render(source);
        assert!(html.contains(expected), "{source} -> {html}");
    }
}

#[test]
fn bf06_security() {
    let html = render("[a](https://safe){href=javascript&#58;alert(1)}");
    assert_eq!(html.matches("href=").count(), 1, "{html}");
    assert!(!html.contains("javascript:"), "{html}");
    let html = render("::: {style=color:red}\nx\n:::\n");
    assert!(!html.contains("style="), "{html}");
}

#[test]
fn bf16_available_engines_respects_features() {
    let available = mordant::available_engines();
    assert!(available.contains(&Engine::DefinitionList));
    assert!(available.contains(&Engine::FencedDiv));
    assert!(available.contains(&Engine::LinkAttributes));
    for engine in mordant::extensions::Engine::ALL {
        assert_eq!(
            available.contains(engine),
            mordant::is_engine_available(*engine),
            "{engine}"
        );
    }
}
