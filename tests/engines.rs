//! Integration tests for runtime engine switching.
//!
//! Feature-dependent tests are gated so that `cargo test
//! --no-default-features --features std` keeps working.

use mordant::parser;
use mordant::renderer::html;
use mordant::{
    available_engines, is_engine_available, markdown_to_html_string_with_options, Engine, Engines,
    Result,
};

fn render(source: &str, engines: Engines) -> Result<String> {
    let mut parser_opts = parser::Options::default();
    parser_opts.engines = engines;
    let mut html_opts = html::Options::default();
    html_opts.engines = engines;
    let mut output = String::new();
    markdown_to_html_string_with_options(&mut output, source, parser_opts, html_opts)?;
    Ok(output)
}

#[test]
fn engines_flags_are_queryable_without_any_features() {
    assert!(Engines::all().get(Engine::Emoji));
    assert!(!Engines::none().get(Engine::Emoji));
    assert!(!Engines::default().get(Engine::Highlighter));
    assert!(!Engines::default()
        .without(&[Engine::Emoji])
        .get(Engine::Highlighter));
    assert!(Engines::all()
        .without(&[Engine::Emoji])
        .get(Engine::Highlighter));
    assert_eq!(render("text", Engines::none()).unwrap(), "<p>text</p>\n");
}

#[test]
fn available_engines_matches_engine_flags() {
    for engine in Engine::ALL {
        assert_eq!(
            available_engines().contains(engine),
            is_engine_available(*engine),
            "availability mismatch for {engine}"
        );
    }
}

#[cfg(feature = "emoji")]
#[test]
fn only_enables_selected_engine() {
    let html = render(":smile:", Engines::only(&[Engine::Emoji])).unwrap();
    assert!(html.contains("😄"), "{html}");
    let html = render(":smile:", Engines::only(&[Engine::Math])).unwrap();
    assert!(!html.contains("😄"), "{html}");
}

#[cfg(all(
    feature = "emoji",
    feature = "math",
    feature = "definition-list",
    feature = "fenced-div",
    feature = "link-attributes"
))]
#[test]
fn mixed_features_render_together() {
    let source = concat!(
        "::: note\n",
        "[link](https://example.com){#foo .bar}\n",
        ":::\n\n",
        "Apple\n",
        ": A fruit with :smile:.\n\n",
        "```math\n",
        "x^2\n",
        "```\n"
    );
    let html = render(source, Engines::all_without_highlighter()).unwrap();
    assert!(html.contains("<div class=\"note\">"), "{html}");
    assert!(html.contains("id=\"foo\""), "{html}");
    assert!(html.contains("<dl>"), "{html}");
    assert!(html.contains("😄"), "{html}");
    assert!(html.contains("katex"), "{html}");

    let html = render(
        source,
        Engines::all_without_highlighter().without(&[Engine::Emoji, Engine::LinkAttributes]),
    )
    .unwrap();
    assert!(!html.contains("😄"), "{html}");
    assert!(!html.contains("id=\"foo\""), "{html}");
    assert!(html.contains("<dl>"), "{html}");
    assert!(html.contains("katex"), "{html}");
}

#[test]
fn default_options_do_not_highlight() {
    let html = render("```python\nx = 1\n```", Engines::default()).unwrap();
    assert!(html.contains("<pre><code"), "{html}");
    assert!(!html.contains("style="), "{html}");
}
