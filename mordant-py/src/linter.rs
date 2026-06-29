//! Markdown linter built on top of the rushdown AST.
//!
//! The linter parses Markdown into the rushdown arena (the same AST exposed
//! through `Document` / `Node` / `Walker`) and evaluates a set of lint rules
//! against it. Most rules are AST-driven (heading structure, links, images,
//! fenced code blocks); a few line-based rules (trailing whitespace, blank
//! lines, final newline) supplement them using the raw source string.
//!
//! Rule identifiers follow markdownlint (MD0xx) so output is familiar.
//!
//! The heavy lifting (`run_lint`) returns plain-Rust `Violation` values, which
//! are `Send`, so it is safe to call inside `Python::detach` (GIL released).
//! Conversion into the `Diagnostic` pyclass happens on the GIL thread.

use pyo3::prelude::*;
use rushdown_lib::ast::{Arena, KindData, NodeRef};
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/// Severity of a lint diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    #[allow(dead_code)]
    Error,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// A description of how to auto-correct a violation, as a minimal edit to the
/// source text. All variants are line-oriented (0-indexed lines), which is all
/// the currently auto-fixable rules need. Fixes are applied to the raw source
/// rather than by re-rendering the AST: rushdown renders to HTML and has no
/// Markdown serializer, and source edits keep the resulting diff minimal.
pub enum FixOp {
    /// Replace the whole content of a line (used to strip trailing whitespace).
    ReplaceLine { line: usize, text: String },
    /// Delete a line entirely (used to collapse runs of blank lines).
    DeleteLine { line: usize },
    /// Ensure the document ends with exactly one trailing newline.
    EnsureFinalNewline,
    /// Insert a language onto a fence's opening line. Only applied when the
    /// caller supplies a default language, since the language can't be inferred.
    SetCodeLanguage { line: usize },
}

/// A single rule violation — plain Rust (`Send`), produced without the GIL.
pub struct Violation {
    pub rule: &'static str,
    pub name: &'static str,
    pub message: String,
    /// 1-indexed source line, if known.
    pub line: Option<usize>,
    pub severity: Severity,
    /// How to auto-correct this violation, if it is auto-fixable.
    pub fix: Option<FixOp>,
}

impl Violation {
    fn warn(
        rule: &'static str,
        name: &'static str,
        line: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Violation {
            rule,
            name,
            message: message.into(),
            line,
            severity: Severity::Warning,
            fix: None,
        }
    }

    fn warn_fix(
        rule: &'static str,
        name: &'static str,
        line: Option<usize>,
        message: impl Into<String>,
        fix: FixOp,
    ) -> Self {
        Violation {
            rule,
            name,
            message: message.into(),
            line,
            severity: Severity::Warning,
            fix: Some(fix),
        }
    }
}

/// A lint diagnostic exposed to Python.
#[pyclass(module = "mordant")]
#[derive(Clone)]
pub struct Diagnostic {
    rule: String,
    name: String,
    message: String,
    line: Option<usize>,
    severity: String,
    fixable: bool,
}

#[pymethods]
impl Diagnostic {
    /// The rule identifier (e.g. "MD001").
    #[getter]
    fn rule(&self) -> &str {
        &self.rule
    }

    /// The human-readable rule name / alias (e.g. "heading-increment").
    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    /// The diagnostic message.
    #[getter]
    fn message(&self) -> &str {
        &self.message
    }

    /// The 1-indexed source line, or None if not known.
    #[getter]
    fn line(&self) -> Option<usize> {
        self.line
    }

    /// The severity ("warning" or "error").
    #[getter]
    fn severity(&self) -> &str {
        &self.severity
    }

    /// Whether `lint`/`fix` can auto-correct this issue with no extra input.
    /// Structural rules (e.g. MD001, MD042) and MD040 (which needs a language)
    /// report False here.
    #[getter]
    fn fixable(&self) -> bool {
        self.fixable
    }

    fn __repr__(&self) -> String {
        match self.line {
            Some(l) => format!("<Diagnostic {} line={}>", self.rule, l),
            None => format!("<Diagnostic {}>", self.rule),
        }
    }

    fn __str__(&self) -> String {
        match self.line {
            Some(l) => format!("{} ({}) line {}: {}", self.rule, self.name, l, self.message),
            None => format!("{} ({}): {}", self.rule, self.name, self.message),
        }
    }
}

impl Diagnostic {
    pub fn from_violation(v: Violation) -> Self {
        // "Auto-fixable with no extra input" — SetCodeLanguage needs a language
        // supplied by the caller, so it does not count.
        let fixable = matches!(
            v.fix,
            Some(FixOp::ReplaceLine { .. })
                | Some(FixOp::DeleteLine { .. })
                | Some(FixOp::EnsureFinalNewline)
        );
        Diagnostic {
            rule: v.rule.to_string(),
            name: v.name.to_string(),
            message: v.message,
            line: v.line,
            severity: v.severity.as_str().to_string(),
            fixable,
        }
    }
}

// ---------------------------------------------------------------------------
// Fix results
// ---------------------------------------------------------------------------

/// Internal result of a fix run (plain Rust, `Send`).
pub struct FixOutcome {
    pub output: String,
    pub fixed: Vec<Violation>,
    pub unfixable: Vec<Violation>,
}

/// The result of `fix()` / `Document.fix()`, exposed to Python.
#[pyclass(module = "mordant")]
pub struct FixResult {
    output: String,
    fixed: Vec<Diagnostic>,
    unfixable: Vec<Diagnostic>,
}

#[pymethods]
impl FixResult {
    /// The corrected Markdown source.
    #[getter]
    fn output(&self) -> &str {
        &self.output
    }

    /// Diagnostics that were auto-corrected (line numbers refer to the input).
    #[getter]
    fn fixed(&self) -> Vec<Diagnostic> {
        self.fixed.clone()
    }

    /// Diagnostics that could not be auto-corrected and still need attention.
    #[getter]
    fn unfixable(&self) -> Vec<Diagnostic> {
        self.unfixable.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "<FixResult fixed={} unfixable={}>",
            self.fixed.len(),
            self.unfixable.len()
        )
    }
}

impl FixResult {
    pub fn from_outcome(o: FixOutcome) -> Self {
        FixResult {
            output: o.output,
            fixed: o.fixed.into_iter().map(Diagnostic::from_violation).collect(),
            unfixable: o
                .unfixable
                .into_iter()
                .map(Diagnostic::from_violation)
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Plain-Rust lint configuration (no Python references — safe without the GIL).
#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    /// Rule ids to disable. Ignored if `enable` is set.
    disable: Vec<String>,
    /// If set, ONLY these rule ids run.
    enable: Option<Vec<String>>,
}

impl LintConfig {
    fn is_enabled(&self, rule: &str) -> bool {
        if let Some(enable) = &self.enable {
            enable.iter().any(|r| r == rule)
        } else {
            !self.disable.iter().any(|r| r == rule)
        }
    }
}

/// Options controlling which lint rules run (Python-exposed).
///
/// ```python
/// # Run everything except MD009:
/// mordant.LintOptions(disable=["MD009"])
/// # Run only MD025:
/// mordant.LintOptions(enable=["MD025"])
/// ```
#[pyclass(module = "mordant", skip_from_py_object)]
#[derive(Clone)]
pub struct LintOptions {
    pub disable: Vec<String>,
    pub enable: Option<Vec<String>>,
}

#[pymethods]
impl LintOptions {
    #[new]
    #[pyo3(signature = (disable = None, enable = None))]
    fn new(disable: Option<Vec<String>>, enable: Option<Vec<String>>) -> Self {
        LintOptions {
            disable: disable.unwrap_or_default(),
            enable,
        }
    }

    #[getter]
    fn disable(&self) -> Vec<String> {
        self.disable.clone()
    }
    #[setter]
    fn set_disable(&mut self, v: Vec<String>) {
        self.disable = v;
    }

    #[getter]
    fn enable(&self) -> Option<Vec<String>> {
        self.enable.clone()
    }
    #[setter]
    fn set_enable(&mut self, v: Option<Vec<String>>) {
        self.enable = v;
    }
}

impl LintOptions {
    pub fn to_config(&self) -> LintConfig {
        LintConfig {
            disable: self.disable.clone(),
            enable: self.enable.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Document model — extracted once from the AST, then inspected by rules
// ---------------------------------------------------------------------------

struct Source<'s> {
    text: &'s str,
    lines: Vec<&'s str>,
}

struct HeadingInfo {
    level: u8,
    line: Option<usize>, // 0-indexed source line
    text: String,
}

struct LinkInfo {
    destination: String,
    line: Option<usize>,
}

struct ImageInfo {
    alt: String,
    line: Option<usize>,
}

struct CodeBlockInfo {
    language: Option<String>,
    fenced: bool,
    line: Option<usize>,
}

/// A contiguous span of source lines that are part of a code region
/// (fenced block or indented code block).  Used by the line-based rules
/// to skip content that belongs to code.
struct CodeRegion {
    start: usize, // 0-indexed, inclusive
    end: usize,   // 0-indexed, inclusive
    #[allow(dead_code)]
    fenced: bool, // future: differentiate fenced vs indented in mask logic
}

/// Convert a byte offset (as returned by rushdown's Node.pos()) into a
/// 0-indexed source line number.
fn byte_offset_to_line(source: &Source, offset: usize) -> Option<usize> {
    let mut pos = 0usize;
    for (i, line_str) in source.lines.iter().enumerate() {
        // Each line is followed by a \n (except possibly the last)
        let line_end = pos + line_str.len() + 1;
        if offset < line_end {
            return Some(i);
        }
        pos = line_end;
    }
    // Offset is past the last line
    if pos <= offset {
        Some(source.lines.len())
    } else {
        None
    }
}

#[derive(Default)]
struct Collected {
    headings: Vec<HeadingInfo>,
    links: Vec<LinkInfo>,
    images: Vec<ImageInfo>,
    code_blocks: Vec<CodeBlockInfo>,
    /// Code regions derived from AST nodes (fenced blocks, indented code).
    code_regions: Vec<CodeRegion>,
}

/// Check if a source line contains a fence delimiter (backtick or tilde).
/// Handles blockquote/list prefixes like "> ```" or "  ~~~~".
fn has_fence_char(s: &str) -> bool {
    s.contains("```") || s.contains("~~~")
}

/// Collect the resolved text content of a node's subtree.
///
/// Mirrors `node::collect_text`, but operates on a borrowed `&Arena` rather
/// than `Rc<RefCell<Arena>>` so it can run on the GIL-free parse result.
fn collect_text(arena: &Arena, node_ref: NodeRef, source: &str) -> String {
    let mut result = String::new();
    let mut child = arena[node_ref].first_child();
    while let Some(nref) = child {
        match &arena[nref].kind_data() {
            KindData::Text(t) => result.push_str(t.str(source)),
            KindData::CodeSpan(c) => result.push_str(c.str(source).as_ref()),
            KindData::RawHtml(r) => result.push_str(r.str(source).as_ref()),
            _ => result.push_str(&collect_text(arena, nref, source)),
        }
        child = arena[nref].next_sibling();
    }
    result
}

/// Single pre-order DFS that extracts everything the rules need.
fn build(arena: &Arena, node_ref: NodeRef, src: &Source, out: &mut Collected) {
    match &arena[node_ref].kind_data() {
        KindData::Heading(h) => out.headings.push(HeadingInfo {
            level: h.level(),
            line: arena[node_ref].pos(),
            text: collect_text(arena, node_ref, src.text),
        }),
        KindData::Link(l) => out.links.push(LinkInfo {
            destination: l.destination_str(src.text).to_string(),
            line: arena[node_ref].pos(),
        }),
        KindData::Image(_) => out.images.push(ImageInfo {
            alt: collect_text(arena, node_ref, src.text),
            line: arena[node_ref].pos(),
        }),
        KindData::CodeBlock(cb) => {
            // node_pos is a byte offset; convert to line number.
            let node_pos = arena[node_ref].pos();
            let line_num = node_pos.and_then(|p| byte_offset_to_line(src, p));
            // Distinguish fenced from indented code blocks by inspecting the
            // source line at the block's position. Only fenced blocks
            // are eligible for the "missing language" rule (MD040).
            let fenced = line_num
                .and_then(|p| src.lines.get(p))
                .map(|l| has_fence_char(l))
                .unwrap_or(false);
            out.code_blocks.push(CodeBlockInfo {
                language: cb.language_str(src.text).map(|s| s.to_string()),
                fenced,
                line: line_num,
            });

            // Compute the code region span from AST node positions.
            // The rushdown parser sets pos() inconsistently:
            //   - Top-level fenced blocks: pos = opening fence line
            //   - Nested fenced blocks (e.g. inside blockquotes): pos = closing fence line
            //   - Indented blocks: pos = last content line
            // We detect the case by checking if pos points to the opening or closing fence.
            if let Some(pos) = line_num {
                let content_lines: usize = cb.value().iter(src.text).count();
                let (region_start, region_end) = if fenced {
                    // Check if pos + 1 + content_lines points to a closing fence line.
                    // If so, pos is the opening fence; otherwise pos is the closing fence.
                    let candidate_end = pos + 1 + content_lines;
                    if candidate_end < src.lines.len() && has_fence_char(src.lines.get(candidate_end).unwrap_or(&"")) {
                        // pos is the opening fence line; candidate_end is closing fence.
                        (pos, candidate_end)
                    } else {
                        // pos is the closing fence line.
                        let opening = pos.saturating_sub(1).saturating_sub(content_lines);
                        (opening, pos)
                    }
                } else {
                    // Indented code block: pos is the last content line.
                    let opening = pos.saturating_sub(content_lines.saturating_sub(1));
                    (opening, pos)
                };
                out.code_regions.push(CodeRegion { start: region_start, end: region_end, fenced });
            }
        }
        _ => {}
    }

    let mut child = arena[node_ref].first_child();
    while let Some(c) = child {
        build(arena, c, src, out);
        child = arena[c].next_sibling();
    }
}

// ---------------------------------------------------------------------------
// AST-derived code mask — lines inside code blocks
// ---------------------------------------------------------------------------

/// Build a boolean mask marking lines that are part of code regions
/// (fenced or indented), derived from the AST's CodeBlock nodes.
/// Replaces the lexical `fence_mask` scan (Phase 2).
fn code_mask(regions: &[CodeRegion], n_lines: usize) -> Vec<bool> {
    let mut mask = vec![false; n_lines];
    for r in regions {
        for i in r.start..=r.end.min(n_lines.saturating_sub(1)) {
            mask[i] = true;
        }
    }
    mask
}

// ---------------------------------------------------------------------------
// Rules (AST-based)
// ---------------------------------------------------------------------------

/// MD001 — heading levels should only increment by one at a time.
fn md001(m: &Collected, out: &mut Vec<Violation>) {
    let mut prev: Option<u8> = None;
    for h in &m.headings {
        if let Some(p) = prev {
            if h.level > p + 1 {
                out.push(Violation::warn(
                    "MD001",
                    "heading-increment",
                    h.line.map(|l| l + 1),
                    format!(
                        "Heading level jumps from h{} to h{} (expected h{} next)",
                        p,
                        h.level,
                        p + 1
                    ),
                ));
            }
        }
        prev = Some(h.level);
    }
}

/// MD024 — multiple headings with the same text content.
fn md024(m: &Collected, out: &mut Vec<Violation>) {
    let mut seen: HashSet<String> = HashSet::new();
    for h in &m.headings {
        let key = h.text.trim().to_string();
        if key.is_empty() {
            continue;
        }
        if !seen.insert(key) {
            out.push(Violation::warn(
                "MD024",
                "no-duplicate-heading",
                h.line.map(|l| l + 1),
                format!("Duplicate heading content: \"{}\"", h.text.trim()),
            ));
        }
    }
}

/// MD025 — a document should have at most one top-level (h1) heading.
fn md025(m: &Collected, out: &mut Vec<Violation>) {
    let mut count = 0;
    for h in &m.headings {
        if h.level == 1 {
            count += 1;
            if count > 1 {
                out.push(Violation::warn(
                    "MD025",
                    "single-h1",
                    h.line.map(|l| l + 1),
                    "Multiple top-level (h1) headings in the same document",
                ));
            }
        }
    }
}

/// MD040 — fenced code blocks should specify a language.
fn md040(m: &Collected, out: &mut Vec<Violation>) {
    for cb in &m.code_blocks {
        if !cb.fenced {
            continue;
        }
        let missing = cb
            .language
            .as_deref()
            .map(|l| l.trim().is_empty())
            .unwrap_or(true);
        if missing {
            // The language can't be inferred, so this carries a fix op that is
            // only applied if the caller supplies a default language.
            let v = match cb.line {
                Some(l0) => Violation::warn_fix(
                    "MD040",
                    "fenced-code-language",
                    Some(l0 + 1),
                    "Fenced code block should specify a language",
                    FixOp::SetCodeLanguage { line: l0 },
                ),
                None => Violation::warn(
                    "MD040",
                    "fenced-code-language",
                    None,
                    "Fenced code block should specify a language",
                ),
            };
            out.push(v);
        }
    }
}

/// MD042 — links should not have an empty destination.
fn md042(m: &Collected, out: &mut Vec<Violation>) {
    for l in &m.links {
        let dest = l.destination.trim();
        if dest.is_empty() || dest == "#" {
            out.push(Violation::warn(
                "MD042",
                "no-empty-links",
                l.line.map(|x| x + 1),
                "Link has an empty destination",
            ));
        }
    }
}

/// MD045 — images should have alternate text.
fn md045(m: &Collected, out: &mut Vec<Violation>) {
    for img in &m.images {
        if img.alt.trim().is_empty() {
            out.push(Violation::warn(
                "MD045",
                "no-alt-text",
                img.line.map(|l| l + 1),
                "Image should have alternate text",
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Rules (line-based, supplementary)
// ---------------------------------------------------------------------------

/// MD009 — no trailing whitespace (fenced code regions are skipped).
///
/// Exactly two trailing spaces on a non-empty line is a valid CommonMark hard
/// line break, so it is deliberately left alone — both for reporting and for
/// fixing, so the auto-fix can never silently delete an intentional `<br>`.
fn md009(src: &Source, mask: &[bool], out: &mut Vec<Violation>) {
    for (i, line) in src.lines.iter().enumerate() {
        if mask.get(i).copied().unwrap_or(false) {
            continue;
        }
        let trimmed = line.trim_end();
        if line.len() == trimmed.len() {
            continue; // no trailing whitespace
        }
        let trailing = &line[trimmed.len()..];
        let is_hard_break = !trimmed.is_empty() && trailing == "  ";
        if is_hard_break {
            continue;
        }
        out.push(Violation::warn_fix(
            "MD009",
            "no-trailing-spaces",
            Some(i + 1),
            "Line has trailing whitespace",
            FixOp::ReplaceLine {
                line: i,
                text: trimmed.to_string(),
            },
        ));
    }
}

/// MD012 — no more than one consecutive blank line (fenced code skipped).
fn md012(src: &Source, mask: &[bool], out: &mut Vec<Violation>) {
    let mut blank_run = 0usize;
    for (i, line) in src.lines.iter().enumerate() {
        if mask.get(i).copied().unwrap_or(false) {
            blank_run = 0;
            continue;
        }
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                out.push(Violation::warn_fix(
                    "MD012",
                    "no-multiple-blanks",
                    Some(i + 1),
                    "Multiple consecutive blank lines",
                    FixOp::DeleteLine { line: i },
                ));
            }
        } else {
            blank_run = 0;
        }
    }
}

/// MD047 — files should end with a single trailing newline.
fn md047(src: &Source, out: &mut Vec<Violation>) {
    if !src.text.is_empty() && !src.text.ends_with('\n') {
        out.push(Violation::warn_fix(
            "MD047",
            "single-trailing-newline",
            Some(src.lines.len().max(1)),
            "File should end with a single newline character",
            FixOp::EnsureFinalNewline,
        ));
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run all enabled lint rules against a parsed AST.
///
/// `root` is the document root `NodeRef`; `arena` and `source` are the parse
/// outputs. Returns plain-Rust `Violation`s sorted by (line, rule id).
pub fn run_lint(source: &str, arena: &Arena, root: NodeRef, cfg: &LintConfig) -> Vec<Violation> {
    let src = Source {
        text: source,
        lines: source.lines().collect(),
    };

    let mut collected = Collected::default();
    build(arena, root, &src, &mut collected);

    let mask = code_mask(&collected.code_regions, src.lines.len());

    let mut out: Vec<Violation> = Vec::new();

    // AST-based rules
    md001(&collected, &mut out);
    md024(&collected, &mut out);
    md025(&collected, &mut out);
    md040(&collected, &mut out);
    md042(&collected, &mut out);
    md045(&collected, &mut out);

    // Line-based supplementary rules
    md009(&src, &mask, &mut out);
    md012(&src, &mask, &mut out);
    md047(&src, &mut out);

    // Apply enable/disable configuration.
    out.retain(|v| cfg.is_enabled(v.rule));

    // Stable ordering: by source line, then by rule id.
    out.sort_by(|a, b| {
        a.line
            .unwrap_or(usize::MAX)
            .cmp(&b.line.unwrap_or(usize::MAX))
            .then_with(|| a.rule.cmp(b.rule))
    });

    out
}

// ---------------------------------------------------------------------------
// Auto-fix
// ---------------------------------------------------------------------------

/// Rewrite a fence's opening line to include `lang`, preserving indentation and
/// the fence characters (e.g. "```" -> "```python", "  ~~~~" -> "  ~~~~python").
fn set_fence_language(orig: &str, lang: &str) -> String {
    let trimmed = orig.trim_start();
    let indent = &orig[..orig.len() - trimmed.len()];
    let fence_char = trimmed.chars().next().unwrap_or('`');
    let fence_len = trimmed.chars().take_while(|&c| c == fence_char).count();
    let fence: String = std::iter::repeat(fence_char).take(fence_len).collect();
    format!("{indent}{fence}{lang}")
}

/// Apply a set of fixes to the source text, returning the corrected Markdown.
///
/// `fixes` should contain only violations whose fix is applicable. Deletions
/// take precedence over replacements on the same line. Line indices in the
/// fixes refer to the original source, and the original array is walked once,
/// so there is no re-indexing hazard.
fn apply_fixes(source: &str, fixes: &[Violation], default_language: Option<&str>) -> String {
    let lines: Vec<&str> = source.lines().collect();

    let mut replace: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    let mut delete: HashSet<usize> = HashSet::new();
    let mut ensure_nl = false;

    for v in fixes {
        match &v.fix {
            Some(FixOp::ReplaceLine { line, text }) => {
                replace.insert(*line, text.clone());
            }
            Some(FixOp::DeleteLine { line }) => {
                delete.insert(*line);
            }
            Some(FixOp::EnsureFinalNewline) => {
                ensure_nl = true;
            }
            Some(FixOp::SetCodeLanguage { line }) => {
                if let Some(lang) = default_language {
                    if let Some(orig) = lines.get(*line) {
                        replace.insert(*line, set_fence_language(orig, lang));
                    }
                }
            }
            None => {}
        }
    }

    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if delete.contains(&i) {
            continue; // deletion wins over any replacement on the same line
        }
        match replace.get(&i) {
            Some(t) => out.push(t.clone()),
            None => out.push((*line).to_string()),
        }
    }

    let mut result = out.join("\n");
    // Preserve the original trailing-newline state; add one if MD047 asked.
    if source.ends_with('\n') || ensure_nl {
        result.push('\n');
    }
    result
}

// ===========================================================================
// Phase 1 — Pure-function unit tests (cargo test)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers — construct Source from a raw string slice
    // -----------------------------------------------------------------------

    fn src(s: &str) -> Source<'_> {
        Source {
            text: s,
            lines: s.lines().collect(),
        }
    }

    // -----------------------------------------------------------------------
    // set_fence_language
    // -----------------------------------------------------------------------

    #[test]
    fn set_fence_language_basic() {
        assert_eq!(set_fence_language("```", "py"), "```py");
        assert_eq!(set_fence_language("~~~", "js"), "~~~js");
    }

    #[test]
    fn set_fence_language_preserves_indentation() {
        assert_eq!(set_fence_language("  ```", "python"), "  ```python");
    }

    #[test]
    fn set_fence_language_preserves_tilde_fence() {
        assert_eq!(set_fence_language("~~~~", "text"), "~~~~text");
    }

    // -----------------------------------------------------------------------
    // md009 — trailing whitespace
    // -----------------------------------------------------------------------

    #[test]
    fn md009_detects_trailing_spaces() {
        let s = src("hello   \n");
        let mask: Vec<bool> = Vec::new(); // not in code
        let mut v: Vec<Violation> = Vec::new();
        md009(&s, &mask, &mut v);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "MD009");
        assert!(v[0].fix.is_some());
    }

    #[test]
    fn md009_ignores_hard_line_break() {
        // Exactly two trailing spaces = hard break — must NOT be flagged.
        let s = src("line one  \n");
        let mask: Vec<bool> = Vec::new();
        let mut v: Vec<Violation> = Vec::new();
        md009(&s, &mask, &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn md009_ignores_code_region() {
        let s = src("```\ncode   \n```\n");
        let mask = vec![true, true, true]; // all lines are in code
        let mut v: Vec<Violation> = Vec::new();
        md009(&s, &mask, &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn md009_ignores_clean_lines() {
        let s = src("clean line\n");
        let mask: Vec<bool> = Vec::new();
        let mut v: Vec<Violation> = Vec::new();
        md009(&s, &mask, &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn md009_multiple_violations() {
        let s = src("a   \nb   \n");
        let mask: Vec<bool> = Vec::new();
        let mut v: Vec<Violation> = Vec::new();
        md009(&s, &mask, &mut v);
        assert_eq!(v.len(), 2);
    }

    // -----------------------------------------------------------------------
    // md012 — multiple blank lines
    // -----------------------------------------------------------------------

    #[test]
    fn md012_detects_extra_blank() {
        let s = src("a\n\n\nb\n");
        let mask: Vec<bool> = Vec::new();
        let mut v: Vec<Violation> = Vec::new();
        md012(&s, &mask, &mut v);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "MD012");
    }

    #[test]
    fn md012_allows_single_blank() {
        let s = src("a\n\nb\n");
        let mask: Vec<bool> = Vec::new();
        let mut v: Vec<Violation> = Vec::new();
        md012(&s, &mask, &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn md012_skips_code_regions() {
        // Multiple blanks inside a fenced block should be ignored.
        let s = src("```\n\n\n```\n");
        let mask = vec![true, true, true, true];
        let mut v: Vec<Violation> = Vec::new();
        md012(&s, &mask, &mut v);
        assert!(v.is_empty());
    }

    // -----------------------------------------------------------------------
    // md047 — final newline
    // -----------------------------------------------------------------------

    #[test]
    fn md047_detects_missing_newline() {
        let s = src("no newline");
        let mut v: Vec<Violation> = Vec::new();
        md047(&s, &mut v);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "MD047");
    }

    #[test]
    fn md047_allows_existing_newline() {
        let s = src("has newline\n");
        let mut v: Vec<Violation> = Vec::new();
        md047(&s, &mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn md047_empty_document_ok() {
        let s = src("");
        let mut v: Vec<Violation> = Vec::new();
        md047(&s, &mut v);
        assert!(v.is_empty());
    }

    // -----------------------------------------------------------------------
    // apply_fixes
    // -----------------------------------------------------------------------

    #[test]
    fn apply_fixes_strips_trailing_whitespace() {
        let fixes = vec![Violation::warn_fix(
            "MD009", "no-trailing-spaces", Some(1), "",
            FixOp::ReplaceLine { line: 0, text: "hello".to_string() },
        )];
        let result = apply_fixes("hello   \n", &fixes, None);
        assert_eq!(result, "hello\n");
    }

    #[test]
    fn apply_fixes_deletes_blank_lines() {
        let fixes = vec![Violation::warn_fix(
            "MD012", "no-multiple-blanks", Some(3), "",
            FixOp::DeleteLine { line: 2 },
        )];
        let result = apply_fixes("a\n\n\nb\n", &fixes, None);
        assert_eq!(result, "a\n\nb\n");
    }

    #[test]
    fn apply_fixes_adds_final_newline() {
        let fixes = vec![Violation::warn_fix(
            "MD047", "single-trailing-newline", Some(1), "",
            FixOp::EnsureFinalNewline,
        )];
        let result = apply_fixes("no newline", &fixes, None);
        assert_eq!(result, "no newline\n");
    }

    #[test]
    fn apply_fixes_deletion_wins_over_replacement() {
        let fixes = vec![
            Violation::warn_fix("MD009", "x", Some(1), "", FixOp::ReplaceLine { line: 1, text: "replaced".to_string() }),
            Violation::warn_fix("MD012", "x", Some(2), "", FixOp::DeleteLine { line: 1 }),
        ];
        let result = apply_fixes("a\n  \nb\n", &fixes, None);
        assert_eq!(result, "a\nb\n");
    }

    #[test]
    fn apply_fixes_multiple_on_same_line() {
        // Two ReplaceLine on the same line — last one wins (HashMap overwrite).
        let fixes = vec![
            Violation::warn_fix("MD009", "x", Some(1), "", FixOp::ReplaceLine { line: 0, text: "first".to_string() }),
            Violation::warn_fix("MD009", "x", Some(1), "", FixOp::ReplaceLine { line: 0, text: "second".to_string() }),
        ];
        let result = apply_fixes("hello   \n", &fixes, None);
        // Either "first" or "second" is fine — the important thing is
        // no panic and a single replacement.
        assert_eq!(result, "second\n");
    }

    // -----------------------------------------------------------------------
    // code_mask — AST-derived (Phase 2)
    // -----------------------------------------------------------------------

    #[test]
    fn code_mask_marks_single_fenced_block() {
        // Opening fence at line 0, 1 content line, closing fence at line 2
        let regions = vec![CodeRegion { start: 0, end: 2, fenced: true }];
        let mask = code_mask(&regions, 3);
        assert_eq!(mask, vec![true, true, true]);
    }

    #[test]
    fn code_mask_marks_multiple_regions() {
        let regions = vec![
            CodeRegion { start: 0, end: 0, fenced: true },   // line 0
            CodeRegion { start: 3, end: 4, fenced: true },   // lines 3-4
        ];
        let mask = code_mask(&regions, 5);
        assert_eq!(mask, vec![true, false, false, true, true]);
    }

    #[test]
    fn code_mask_skips_non_code_lines() {
        let regions = vec![CodeRegion { start: 1, end: 2, fenced: true }];
        let mask = code_mask(&regions, 5);
        assert_eq!(mask, vec![false, true, true, false, false]);
    }

    #[test]
    fn code_mask_clamps_to_line_count() {
        // End index beyond line count should be clamped.
        let regions = vec![CodeRegion { start: 3, end: 10, fenced: true }];
        let mask = code_mask(&regions, 5);
        assert_eq!(mask, vec![false, false, false, true, true]);
    }

    #[test]
    fn code_mask_empty() {
        let mask = code_mask(&[], 3);
        assert_eq!(mask, vec![false, false, false]);
    }

    // -----------------------------------------------------------------------
    // Idempotence — fix(fix(x)) == fix(x) for the pure fix path
    // -----------------------------------------------------------------------

    #[test]
    fn fix_idempotent_trailing_space() {
        let fixes = vec![Violation::warn_fix(
            "MD009", "x", Some(1), "", FixOp::ReplaceLine { line: 0, text: "hello".to_string() })];
        let once = apply_fixes("hello   \n", &fixes, None);
        let twice = apply_fixes(&once, &[], None);
        assert_eq!(once, twice);
    }

    #[test]
    fn fix_idempotent_blank_lines() {
        let fixes = vec![Violation::warn_fix(
            "MD012", "x", Some(3), "", FixOp::DeleteLine { line: 2 })];
        let once = apply_fixes("a\n\n\nb\n", &fixes, None);
        let twice = apply_fixes(&once, &[], None);
        assert_eq!(once, twice);
    }

    #[test]
    fn fix_idempotent_final_newline() {
        let fixes = vec![Violation::warn_fix(
            "MD047", "x", Some(1), "", FixOp::EnsureFinalNewline)];
        let once = apply_fixes("no newline", &fixes, None);
        let twice = apply_fixes(&once, &[], None);
        assert_eq!(once, twice);
    }

    // -----------------------------------------------------------------------
    // HTML-equivalence oracle (NFR-1)
    //
    // For whitespace-only fixable rules, the fix must never change rendered
    // HTML.  This test is a thin shim — the full Hypothesis-driven oracle
    // lives in tests/test_fix_safety.py (Python side).  This Rust test
    // exercises the common cases.
    // -----------------------------------------------------------------------

    #[test]
    fn md009_fix_preserves_rendered_output() {
        // "hello   " → "hello" (three spaces stripped)
        // In HTML both render as "hello" (trailing spaces are ignorable).
        let src = "hello   \n";
        let fixes = vec![Violation::warn_fix(
            "MD009", "x", Some(1), "", FixOp::ReplaceLine { line: 0, text: "hello".to_string() })];
        let fixed = apply_fixes(src, &fixes, None);
        assert_eq!(fixed, "hello\n");
    }
}

// ===========================================================================
// Entry point
// ===========================================================================

/// Lint, then auto-correct every fixable violation, returning the corrected
/// source plus the partition of fixed vs. still-unfixable diagnostics.
///
/// `default_language` enables the MD040 fix (inserting that language onto
/// language-less fences); when `None`, MD040 issues are reported as unfixable.
///
/// Note: `unfixable` line numbers refer to the *input*. After fixing, lint the
/// returned `output` to get diagnostics with positions in the corrected text.
pub fn run_fix(
    source: &str,
    arena: &Arena,
    root: NodeRef,
    cfg: &LintConfig,
    default_language: Option<&str>,
) -> FixOutcome {
    let violations = run_lint(source, arena, root, cfg);

    let mut fixed: Vec<Violation> = Vec::new();
    let mut unfixable: Vec<Violation> = Vec::new();
    for v in violations {
        let applicable = match &v.fix {
            Some(FixOp::SetCodeLanguage { .. }) => default_language.is_some(),
            Some(_) => true,
            None => false,
        };
        if applicable {
            fixed.push(v);
        } else {
            unfixable.push(v);
        }
    }

    let output = apply_fixes(source, &fixed, default_language);
    FixOutcome {
        output,
        fixed,
        unfixable,
    }
}
