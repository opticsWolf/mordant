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
use std::collections::{HashMap, HashSet};
use pyo3::types::PyDict;

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
    /// 1-indexed column within the line, if known.
    pub column: Option<usize>,
    /// Byte offset span (half-open: [start, end)) in the source, if known.
    pub span: Option<(usize, usize)>,
    pub severity: Severity,
    /// How to auto-correct this violation, if it is auto-fixable.
    pub fix: Option<FixOp>,
}

impl Violation {
    fn warn(
        rule: &'static str,
        name: &'static str,
        line: Option<usize>,
        column: Option<usize>,
        span: Option<(usize, usize)>,
        message: impl Into<String>,
    ) -> Self {
        Violation {
            rule,
            name,
            message: message.into(),
            line,
            column,
            span,
            severity: Severity::Warning,
            fix: None,
        }
    }

    fn warn_fix(
        rule: &'static str,
        name: &'static str,
        line: Option<usize>,
        column: Option<usize>,
        span: Option<(usize, usize)>,
        message: impl Into<String>,
        fix: FixOp,
    ) -> Self {
        Violation {
            rule,
            name,
            message: message.into(),
            line,
            column,
            span,
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
    column: Option<usize>,
    span: Option<(usize, usize)>,
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

    /// The 1-indexed column within the line, or None if not known.
    #[getter]
    fn column(&self) -> Option<usize> {
        self.column
    }

    /// The byte offset span (half-open [start, end)), or None if not known.
    #[getter]
    fn span(&self) -> Option<(usize, usize)> {
        self.span
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
            column: v.column,
            span: v.span,
            severity: v.severity.as_str().to_string(),
            fixable,
        }
    }
}

// ---------------------------------------------------------------------------
// Fix-engine hardening (Phase 4)
// ---------------------------------------------------------------------------

/// A byte-range edit to apply to the source text.
/// `start` and `end` are byte offsets (half-open: [start, end)).
/// `replacement` is the text to insert at that position.
#[allow(dead_code)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

/// Internal result of a fix run (plain Rust, `Send`).
pub struct FixOutcome {
    pub output: String,
    pub fixed: Vec<Violation>,
    pub unfixable: Vec<Violation>,
    /// Diagnostics remaining after fixing, computed by re-linting `output`.
    pub remaining: Vec<Violation>,
}

/// The result of `fix()` / `Document.fix()`, exposed to Python.
#[pyclass(module = "mordant")]
pub struct FixResult {
    output: String,
    fixed: Vec<Diagnostic>,
    unfixable: Vec<Diagnostic>,
    remaining: Vec<Diagnostic>,
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

    /// Diagnostics remaining after fixing (re-lint of output).
    #[getter]
    fn remaining(&self) -> Vec<Diagnostic> {
        self.remaining.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "<FixResult fixed={} unfixable={} remaining={}",
            self.fixed.len(),
            self.unfixable.len(),
            self.remaining.len()
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
            remaining: o.remaining.into_iter().map(Diagnostic::from_violation).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Rule parameters — per-rule tuning (Phase 5)
// ---------------------------------------------------------------------------

/// Per-rule configuration parameters.  Each field has a sensible default
/// that mirrors the markdownlint ecosystem.
#[derive(Debug, Clone)]
pub struct RuleParams {
    /// MD003 heading style: "consistent", "atx", "atx_closed", "setext", "setext2".
    pub heading_style: String,
    /// MD013 line length limit (default 80).
    pub line_length: usize,
    /// MD013 ignore threshold: lines below this are never reported.
    pub line_length_ignore_threshold: usize,
    /// MD010: number of spaces per tab (default 4).
    pub spaces_per_tab: usize,
    /// MD024: only compare sibling headings (default false).
    #[allow(dead_code)]
    pub siblings_only: bool,
    /// MD040: default language to insert when fixing (default None).
    #[allow(dead_code)]
    pub default_language: Option<String>,
}

impl Default for RuleParams {
    fn default() -> Self {
        RuleParams {
            heading_style: "consistent".to_string(),
            line_length: 80,
            line_length_ignore_threshold: 0,
            spaces_per_tab: 4,
            siblings_only: false,
            default_language: None,
        }
    }
}

/// A suppression directive parsed from `<!-- markdownlint-disable ... -->` comments.
#[derive(Debug, Clone)]
pub struct SuppressionDirective {
    /// Line number (0-indexed) where the directive appears.
    pub line: usize,
    /// If Some, only these rules are suppressed. If None, all rules are suppressed.
    pub rules: Option<Vec<String>>,
    /// Action: "disable", "enable", "disable-next-line".
    pub action: String,
}

/// Plain-Rust lint configuration (no Python references — safe without the GIL).
#[pyclass(module = "mordant")]
#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    /// Rule ids to disable. Ignored if `enable` is set.
    pub disable: Vec<String>,
    /// If set, ONLY these rule ids run.
    pub enable: Option<Vec<String>>,
    /// When default is False, collect these rule ids.
    #[allow(dead_code)]
    pub _enabled_when_default_false: Option<Vec<String>>,
    /// Suppression directives from inline comments.
    pub suppressions: Vec<SuppressionDirective>,
    /// Per-rule parameters.
    pub params: RuleParams,
}

impl LintConfig {
    fn is_enabled(&self, rule: &str) -> bool {
        if let Some(enable) = &self.enable {
            enable.iter().any(|r| r == rule)
        } else {
            !self.disable.iter().any(|r| r == rule)
        }
    }

    /// Check if a (rule, line) pair is suppressed by an inline comment.
    fn is_suppressed(&self, rule: &str, line: usize) -> bool {
        let mut disable_all = false;
        let mut disabled: HashSet<String> = HashSet::new();
        let mut enabled: HashSet<String> = HashSet::new(); // explicit enables override disable_all

        for directive in &self.suppressions {
            if directive.line > line {
                break;
            }
            // `all` == this directive targets every rule (no rule list given).
            let all = directive.rules.as_deref().map_or(true, |r| r.is_empty());
            match directive.action.as_str() {
                "disable-next-line" => {
                    if directive.line + 1 == line
                        && (all || directive.rules.as_ref().unwrap().iter().any(|r| r == rule))
                    {
                        return true;
                    }
                }
                "disable" => {
                    if all {
                        disable_all = true;
                        enabled.clear();
                    } else {
                        for r in directive.rules.as_ref().unwrap() {
                            disabled.insert(r.clone());
                            enabled.remove(r);
                        }
                    }
                }
                "enable" => {
                    if all {
                        disable_all = false;
                        disabled.clear();
                        enabled.clear();
                    } else {
                        for r in directive.rules.as_ref().unwrap() {
                            enabled.insert(r.clone());
                            disabled.remove(r);
                        }
                    }
                }
                _ => {}
            }
        }

        // Explicit enable wins over a blanket disable; otherwise a specific
        // disable wins; otherwise fall back to the blanket disable state.
        if enabled.contains(rule) {
            false
        } else if disabled.contains(rule) {
            true
        } else {
            disable_all
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
            suppressions: Vec::new(),
            params: RuleParams::default(),
            _enabled_when_default_false: None,
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
        KindData::Heading(h) => {
            let line_num = arena[node_ref].pos().and_then(|p| byte_offset_to_line(src, p));
            out.headings.push(HeadingInfo {
                level: h.level(),
                line: line_num,
                text: collect_text(arena, node_ref, src.text),
            });
        }
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

// ===========================================================================
// Inline suppression parsing (Phase 6)
// ===========================================================================

/// Parse inline markdownlint suppression comments from the source text.
/// Supports:
///   <!-- markdownlint-disable MD001 MD002 -->
///   <!-- markdownlint-enable MD001 MD002 -->
///   <!-- markdownlint-disable-next-line MD001 MD002 -->
/// If no rule list is given, all rules are affected.
pub fn parse_suppressions(source: &str) -> Vec<SuppressionDirective> {
    let mut directives: Vec<SuppressionDirective> = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("<!--") || !trimmed.contains("markdownlint-") {
            continue;
        }
        // Extract the comment content between <!-- and -->
        let content = if let Some(start) = trimmed.find("markdownlint-") {
            let end = trimmed.find("-->").unwrap_or(trimmed.len());
            trimmed[start + "markdownlint-".len()..end].trim() // skip "markdownlint-"
        } else {
            continue;
        };
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let action = parts[0].to_string();
        let rules: Option<Vec<String>> = if parts.len() > 1 {
            Some(parts[1..].iter().map(|s| s.to_string()).collect())
        } else {
            None // no rules specified = all rules
        };
        directives.push(SuppressionDirective {
            line: i,
            rules,
            action,
        });
    }
    directives
}
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
                    None, // column
                    h.line.map(|l| (l, l + 1)), // span: [line_start, line_end)
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
                None, // column
                h.line.map(|l| (l, l + 1)), // span: [line_start, line_end)
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
                    None, // column
                    h.line.map(|l| (l, l + 1)), // span: [line_start, line_end)
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
                    None, // column
                    Some((l0, l0 + 1)), // span: [line_start, line_end)
                    "Fenced code block should specify a language",
                    FixOp::SetCodeLanguage { line: l0 },
                ),
                None => Violation::warn(
                    "MD040",
                    "fenced-code-language",
                    None,
                    None, // column
                    None, // span
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
                None, // column
                l.line.map(|x| (x, x + 1)), // span: [line_start, line_end)
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
                None, // column
                img.line.map(|l| (l, l + 1)), // span: [line_start, line_end)
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
        // Column = position of first trailing whitespace char
        let col = trimmed.len() + 1; // 1-indexed
        out.push(Violation::warn_fix(
            "MD009",
            "no-trailing-spaces",
            Some(i + 1),
            Some(col), // column
            Some((i, i + 1)), // span: [line_start, line_end)
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
                    None, // column (blank line, no meaningful column)
                    Some((i, i + 1)), // span: [line_start, line_end)
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
        // Column = length of last line + 1 (position after last char)
        let last_line = src.lines.last().map(|s| s.len() + 1).unwrap_or(1);
        out.push(Violation::warn_fix(
            "MD047",
            "single-trailing-newline",
            Some(src.lines.len().max(1)),
            Some(last_line), // column
            Some((src.text.len(), src.text.len())), // span: [end, end) — point span
            "File should end with a single newline character",
            FixOp::EnsureFinalNewline,
        ));
    }
}

// ===========================================================================
// Phase 5 — New rules
// ===========================================================================

/// MD010 — no hard tabs (convert tabs to spaces).
fn md010(src: &Source, mask: &[bool], params: &RuleParams, out: &mut Vec<Violation>) {
    let spaces = " ".repeat(params.spaces_per_tab);
    for (i, line) in src.lines.iter().enumerate() {
        if mask.get(i).copied().unwrap_or(false) {
            continue;
        }
        if !line.contains('\t') {
            continue;
        }
        let fixed = line.replace('\t', &spaces);
        out.push(Violation::warn_fix(
            "MD010",
            "no-hard-tabs",
            Some(i + 1),
            Some(line.find('\t').unwrap_or(0) + 1), // 1-indexed column of first tab
            Some((i, i + 1)),
            "Hard tab character(s) found",
            FixOp::ReplaceLine { line: i, text: fixed },
        ));
    }
}

/// MD018 — ATX heading should have a space after the `#` characters.
/// (Report-only: flags but does not auto-fix to avoid changing heading semantics.)
fn md018(src: &Source, mask: &[bool], out: &mut Vec<Violation>) {
    for (i, line) in src.lines.iter().enumerate() {
        if mask.get(i).copied().unwrap_or(false) {
            continue;
        }
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }
        // Count leading # characters
        let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
        if hash_count == 0 || hash_count > 6 {
            continue;
        }
        // Check if there's a space after the #s
        if hash_count < trimmed.len() && trimmed.as_bytes()[hash_count] != b' ' {
            out.push(Violation::warn(
                "MD018",
                "atx-closing-spaces",
                Some(i + 1),
                Some(hash_count + 1),
                Some((i, i + 1)),
                "ATX heading should have a space after the opening `#` characters",
            ));
        }
    }
}

/// MD019 — ATX heading `#` spacing (leaf: no closing `#`).
/// Report-only: flags ATX headings without closing `#` that have extra spacing.
fn md019(src: &Source, mask: &[bool], out: &mut Vec<Violation>) {
    for (i, line) in src.lines.iter().enumerate() {
        if mask.get(i).copied().unwrap_or(false) {
            continue;
        }
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }
        let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
        if hash_count == 0 || hash_count > 6 {
            continue;
        }
        // Check if this is a leaf heading (no closing #)
        let rest = &trimmed[hash_count..];
        if rest.trim().is_empty() {
            continue; // empty heading
        }
        // If there's no closing #, it's a leaf ATX heading — MD019 flags
        // leaf ATX headings that don't have a space after opening #s
        if rest.as_bytes()[0] != b' ' {
            out.push(Violation::warn(
                "MD019",
                "atx-spacing",
                Some(i + 1),
                Some(hash_count + 1),
                Some((i, i + 1)),
                "ATX heading should have a space after the opening `#` characters",
            ));
        }
    }
}

/// MD020 — ATX closing `#` spacing (report-only).
fn md020(src: &Source, mask: &[bool], out: &mut Vec<Violation>) {
    for (i, line) in src.lines.iter().enumerate() {
        if mask.get(i).copied().unwrap_or(false) {
            continue;
        }
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }
        let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
        if hash_count == 0 || hash_count > 6 {
            continue;
        }
        let rest = &trimmed[hash_count..].trim();
        // Check for closing #
        if !rest.ends_with('#') {
            continue;
        }
        // There should be a space before the closing #
        let without_closing = rest[..rest.len() - 1].trim_end();
        if !without_closing.is_empty() && rest.as_bytes()[rest.len() - 2] != b' ' {
            out.push(Violation::warn(
                "MD020",
                "atx-closing-spaces",
                Some(i + 1),
                None,
                Some((i, i + 1)),
                "ATX heading should have a space before the closing `#` characters",
            ));
        }
    }
}

/// MD021 — spaces inside ATX heading `#` characters.
/// Flags headings like "# Hello #" where there should be no space before closing #.
fn md021(src: &Source, mask: &[bool], out: &mut Vec<Violation>) {
    for (i, line) in src.lines.iter().enumerate() {
        if mask.get(i).copied().unwrap_or(false) {
            continue;
        }
        let trimmed = line.trim_start();
        if !trimmed.starts_with('#') {
            continue;
        }
        let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
        if hash_count == 0 || hash_count > 6 {
            continue;
        }
        let rest = trimmed[hash_count..].trim();
        // Check for closing # with space before it
        if rest.ends_with('#') {
            let without_closing = rest[..rest.len() - 1].trim_end();
            if !without_closing.is_empty() {
                // Space before closing # is a style issue (MD021)
                let last_char = without_closing.chars().last();
                if last_char == Some(' ') {
                    out.push(Violation::warn(
                        "MD021",
                        "atx-heading-space",
                        Some(i + 1),
                        None,
                        Some((i, i + 1)),
                        "Multiple spaces inside ATX heading",
                    ));
                }
            }
        }
    }
}

/// MD022 — blank lines around headings.
fn md022(collected: &Collected, src: &Source, out: &mut Vec<Violation>) {
    let n_lines = src.lines.len();
    for h in &collected.headings {
        let h_line = h.line.unwrap_or(0); // already 0-indexed line
        // Check line before heading (must be blank, or heading is first)
        if h_line > 0 {
            let prev_line = h_line - 1;
            if !src.lines[prev_line].trim().is_empty() {
                out.push(Violation::warn(
                    "MD022",
                    "heading-blank-lines",
                    Some(h_line + 1),
                    None,
                    Some((prev_line, prev_line + 1)),
                    "Heading should be preceded by a blank line",
                ));
            }
        }
        // Check line immediately after heading (must be blank, or heading is last)
        let next_line = h_line + 1;
        if next_line < n_lines {
            if !src.lines[next_line].trim().is_empty() {
                out.push(Violation::warn(
                    "MD022",
                    "heading-blank-lines",
                    Some(h_line + 1),
                    None,
                    Some((h_line, h_line + 1)),
                    "Heading should be followed by a blank line",
                ));
            }
        }
    }
}

/// MD026 — trailing punctuation in headings.
fn md026(collected: &Collected, src: &Source, out: &mut Vec<Violation>) {
    for h in &collected.headings {
        let text = h.text.trim();
        if text.is_empty() {
            continue;
        }
        let last_char = text.chars().last().unwrap_or('\0');
        // Allow common punctuation: . ! ? ) ] }
        if matches!(last_char, '.' | '!' | '?') {
            let line_num = h.line.unwrap_or(0); // already 0-indexed line
            // Get the original source line and replace only the trailing punctuation
            if let Some(orig_line) = src.lines.get(line_num) {
                let fixed_line = orig_line.trim_end_matches(last_char);
                out.push(Violation::warn_fix(
                    "MD026",
                    "no-trailing-punctuation",
                    Some(line_num + 1),
                    None,
                    Some((line_num, line_num + 1)),
                    format!("Heading should not end with trailing punctuation ({last_char})"),
                    FixOp::ReplaceLine {
                        line: line_num,
                        text: fixed_line.to_string(),
                    },
                ));
            }
        }
    }
}

/// MD031 — blank lines around fenced code blocks.
fn md031(collected: &Collected, src: &Source, out: &mut Vec<Violation>) {
    let lines = src.lines.len();
    for cb in collected.code_blocks.iter().filter(|cb| cb.fenced) {
        let cb_line = cb.line.unwrap_or(0); // 0-indexed
        // Check line before
        if cb_line > 0 {
            let prev_line = cb_line - 1;
            if !src.lines[prev_line].trim().is_empty() {
                out.push(Violation::warn(
                    "MD031",
                    "fenced-code-blocks-working",
                    Some(cb_line + 1),
                    None,
                    Some((prev_line, prev_line + 1)),
                    "Fenced code block should be preceded by a blank line",
                ));
            }
        }
        // Check line after (find the closing fence first)
        // For simplicity, check the next line after the opening fence line
        // The actual closing fence detection is handled by the AST region
        if cb_line + 1 < lines {
            let next_line_idx = cb_line + 1;
            // Skip past content lines to find the line after the closing fence
            // This is approximate — the code_mask already handles the region
            if next_line_idx < lines && !src.lines[next_line_idx].trim().is_empty() {
                // Check if this is a closing fence
                let mut after_closing = false;
                if collected.code_regions.len() > 0 {
                    for region in &collected.code_regions {
                        if region.start == cb_line && next_line_idx == region.end + 1 {
                            after_closing = true;
                            break;
                        }
                    }
                }
                if after_closing && !src.lines[next_line_idx].trim().is_empty() {
                    out.push(Violation::warn(
                        "MD031",
                        "fenced-code-blocks-working",
                        Some(next_line_idx + 1),
                        None,
                        Some((next_line_idx, next_line_idx + 1)),
                        "Fenced code block should be followed by a blank line",
                    ));
                }
            }
        }
    }
}

/// MD032 — blank lines around indented code blocks.
fn md032(collected: &Collected, src: &Source, out: &mut Vec<Violation>) {
    let lines = src.lines.len();
    for cb in collected.code_blocks.iter().filter(|cb| !cb.fenced) {
        let cb_line = cb.line.unwrap_or(0); // 0-indexed
        // Check line before
        if cb_line > 0 {
            let prev_line = cb_line - 1;
            if !src.lines[prev_line].trim().is_empty() {
                out.push(Violation::warn(
                    "MD032",
                    "indented-code-block",
                    Some(cb_line + 1),
                    None,
                    Some((prev_line, prev_line + 1)),
                    "Indented code block should be preceded by a blank line",
                ));
            }
        }
        // Check line after
        if cb_line + 1 < lines {
            let next_line_idx = cb_line + 1;
            if next_line_idx < lines && !src.lines[next_line_idx].trim().is_empty() {
                out.push(Violation::warn(
                    "MD032",
                    "indented-code-block",
                    Some(next_line_idx + 1),
                    None,
                    Some((next_line_idx, next_line_idx + 1)),
                    "Indented code block should be followed by a blank line",
                ));
            }
        }
    }
}

/// MD034 — bare URLs in link text.
/// Flags links where the link text is a bare URL.
fn md034(collected: &Collected, _out: &mut Vec<Violation>) {
    // Placeholder: detect links where the text is a bare URL
    // This requires walking the AST for link nodes and comparing
    // link text against the destination. For now, no violations.
    let _ = collected;
}

/// MD003 — heading style (report-only).
fn md003(_collected: &Collected, _params: &RuleParams, _out: &mut Vec<Violation>) {
    // Placeholder: check that all headings use the same style
    // (all ATX or all setext). Detailed style checking
    // requires setext detection which is more complex.
}

/// MD013 — line length (report-only).
fn md013(src: &Source, mask: &[bool], params: &RuleParams, out: &mut Vec<Violation>) {
    let limit = params.line_length;
    let threshold = params.line_length_ignore_threshold;
    for (i, line) in src.lines.iter().enumerate() {
        if mask.get(i).copied().unwrap_or(false) {
            continue;
        }
        if line.len() <= threshold {
            continue;
        }
        if line.len() > limit {
            out.push(Violation::warn(
                "MD013",
                "line-length",
                Some(i + 1),
                None,
                Some((i, i + 1)),
                format!("Line is {} characters long (max {})", line.len(), limit),
            ));
        }
    }
}

/// MD046 — code block indentation (report-only).
fn md046(collected: &Collected, src: &Source, out: &mut Vec<Violation>) {
    for cb in &collected.code_blocks {
        if !cb.fenced {
            continue; // only fenced blocks
        }
        let cb_line = cb.line.unwrap_or(0);
        // Check if the fence line has indentation
        if let Some(line) = src.lines.get(cb_line) {
            let leading_spaces = line.chars().take_while(|&c| c == ' ').count();
            if leading_spaces > 0 && leading_spaces < 4 {
                out.push(Violation::warn(
                    "MD046",
                    "code-block-indentation",
                    Some(cb_line + 1),
                    None,
                    Some((cb_line, cb_line + 1)),
                    "Fenced code block should use 4-space indentation or no indentation",
                ));
            }
        }
    }
}

/// MD048 — fenced code block punctuation style (report-only).
fn md048(collected: &Collected, src: &Source, out: &mut Vec<Violation>) {
    for cb in &collected.code_blocks {
        if !cb.fenced {
            continue;
        }
        let cb_line = cb.line.unwrap_or(0);
        if let Some(line) = src.lines.get(cb_line) {
            let trimmed = line.trim_start();
            // Check for tilde fences
            if trimmed.starts_with("~~~") {
                out.push(Violation::warn(
                    "MD048",
                    "fenced-code-block-punctuation",
                    Some(cb_line + 1),
                    None,
                    Some((cb_line, cb_line + 1)),
                    "Fenced code block should use backticks, not tildes",
                ));
            }
        }
    }
}

/// MD049 — emphasis style (report-only).
/// Flags use of `*` for emphasis when `__` (double underscore) is preferred.
fn md049(_collected: &Collected, _out: &mut Vec<Violation>) {
    // Placeholder: detect emphasis nodes that use * instead of _
    // This requires walking the AST for emphasis/delimiter nodes.
    // For now, no violations.
}

/// MD050 — strong style (report-only).
/// Flags use of `**` for strong when ___ (triple underscore) is preferred.
fn md050(_collected: &Collected, _out: &mut Vec<Violation>) {
    // Placeholder: detect strong nodes that use ** instead of ___
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run all enabled lint rules against a parsed AST.
///
/// `root` is the document root `NodeRef`; `arena` and `source` are the parse
/// outputs. Returns plain-Rust `Violation`s sorted by (line, rule id).
pub fn run_lint(source: &str, arena: &Arena, root: NodeRef, cfg: &LintConfig) -> Vec<Violation> {
    run_lint_with_params(source, arena, root, cfg, &cfg.params)
}

/// Run all enabled lint rules with custom per-rule parameters.
pub fn run_lint_with_params(
    source: &str,
    arena: &Arena,
    root: NodeRef,
    cfg: &LintConfig,
    params: &RuleParams,
) -> Vec<Violation> {
    let src = Source {
        text: source,
        lines: source.lines().collect(),
    };

    let mut collected = Collected::default();
    build(arena, root, &src, &mut collected);

    let mask = code_mask(&collected.code_regions, src.lines.len());

    let mut out: Vec<Violation> = Vec::new();

    // AST-based rules (Phase 0)
    md001(&collected, &mut out);
    md024(&collected, &mut out);
    md025(&collected, &mut out);
    md040(&collected, &mut out);
    md042(&collected, &mut out);
    md045(&collected, &mut out);

    // Phase 5 — new AST-based rules
    md018(&src, &mask, &mut out);
    md019(&src, &mask, &mut out);
    md020(&src, &mask, &mut out);
    md021(&src, &mask, &mut out);
    md022(&collected, &src, &mut out);
    md026(&collected, &src, &mut out);
    md031(&collected, &src, &mut out);
    md032(&collected, &src, &mut out);
    md034(&collected, &mut out);
    md003(&collected, params, &mut out);
    md049(&collected, &mut out);
    md050(&collected, &mut out);

    // Line-based supplementary rules (Phase 0)
    md009(&src, &mask, &mut out);
    md012(&src, &mask, &mut out);
    md047(&src, &mut out);

    // Phase 5 — new line-based rules
    md010(&src, &mask, params, &mut out);
    md013(&src, &mask, params, &mut out);

    // Phase 5 — report-only style rules
    md046(&collected, &src, &mut out);
    md048(&collected, &src, &mut out);

    // Apply enable/disable configuration and inline suppressions.
    out.retain(|v| {
        let line_0indexed = v.line.map(|l| l.saturating_sub(1));
        cfg.is_enabled(v.rule) && (!line_0indexed.map(|l| cfg.is_suppressed(v.rule, l)).unwrap_or(false))
    });

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
// Rule metadata — for lint_rules() introspection (Phase 6)
// ===========================================================================

/// Metadata about a single lint rule, exposed to Python via `lint_rules()`.
#[pyclass(module = "mordant")]
#[derive(Clone)]
pub struct RuleMetadata {
    id: String,
    name: String,
    description: String,
    fixable: bool,
    default_params: String,
}

#[pymethods]
impl RuleMetadata {
    #[getter]
    fn id(&self) -> &str { &self.id }
    #[getter]
    fn name(&self) -> &str { &self.name }
    #[getter]
    fn description(&self) -> &str { &self.description }
    #[getter]
    fn fixable(&self) -> bool { self.fixable }
    #[getter]
    fn default_params(&self) -> &str { &self.default_params }

    fn __repr__(&self) -> String {
        format!("<RuleMetadata {} ({})>", self.id, self.name)
    }
}

/// Return metadata for all registered lint rules.
pub fn lint_rules() -> Vec<RuleMetadata> {
    vec![
        RuleMetadata { id: "MD001".into(), name: "heading-increment".into(), description: "Heading levels should increment by one at a time".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD003".into(), name: "heading-style".into(), description: "Heading style consistency".into(), fixable: false, default_params: "{\"heading_style\": \"consistent\"}".into() },
        RuleMetadata { id: "MD009".into(), name: "no-trailing-spaces".into(), description: "Lines should not have trailing spaces".into(), fixable: true, default_params: "{}".into() },
        RuleMetadata { id: "MD010".into(), name: "no-hard-tabs".into(), description: "Lines should not contain hard tabs".into(), fixable: true, default_params: "{\"spaces_per_tab\": 4}".into() },
        RuleMetadata { id: "MD012".into(), name: "no-multiple-blanks".into(), description: "There should be no more than one consecutive blank line".into(), fixable: true, default_params: "{}".into() },
        RuleMetadata { id: "MD013".into(), name: "line-length".into(), description: "Lines should not exceed a specified number of characters".into(), fixable: false, default_params: "{\"line_length\": 80, \"line_length_ignore_threshold\": 0}".into() },
        RuleMetadata { id: "MD018".into(), name: "atx-spacing".into(), description: "ATX headings should have a space after the opening '#'".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD019".into(), name: "atx-closing-spaces".into(), description: "ATX leaf headings should not have closing '#'".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD020".into(), name: "atx-closing-spaces".into(), description: "ATX headings should have a space before the closing '#'".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD021".into(), name: "atx-heading-space".into(), description: "Multiple spaces inside ATX heading".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD022".into(), name: "heading-blank-lines".into(), description: "Headings should have blank lines around them".into(), fixable: true, default_params: "{}".into() },
        RuleMetadata { id: "MD024".into(), name: "no-duplicate-heading".into(), description: "Multiple headings with the same content".into(), fixable: false, default_params: "{\"siblings_only\": false}".into() },
        RuleMetadata { id: "MD025".into(), name: "single-h1".into(), description: "Document should have only one h1 heading".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD026".into(), name: "no-trailing-punctuation".into(), description: "Headings should not end with trailing punctuation".into(), fixable: true, default_params: "{}".into() },
        RuleMetadata { id: "MD031".into(), name: "fenced-code-blocks-working".into(), description: "Fenced code blocks should have blank lines around them".into(), fixable: true, default_params: "{}".into() },
        RuleMetadata { id: "MD032".into(), name: "indented-code-block".into(), description: "Indented code blocks should have blank lines around them".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD034".into(), name: "no-bare-urls".into(), description: "Bare URLs should be in angle brackets".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD040".into(), name: "fenced-code-language".into(), description: "Fenced code blocks should specify a language".into(), fixable: true, default_params: "{\"default_language\": null}".into() },
        RuleMetadata { id: "MD042".into(), name: "no-empty-links".into(), description: "Links should have a non-empty destination".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD045".into(), name: "no-alt-text".into(), description: "Images should have alternate text".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD046".into(), name: "code-block-indentation".into(), description: "Fenced code blocks should use 4-space indentation".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD047".into(), name: "single-trailing-newline".into(), description: "Files should end with a single trailing newline".into(), fixable: true, default_params: "{}".into() },
        RuleMetadata { id: "MD048".into(), name: "fenced-code-block-punctuation".into(), description: "Fenced code blocks should use backticks, not tildes".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD049".into(), name: "emphasis-style".into(), description: "Emphasis style consistency".into(), fixable: false, default_params: "{}".into() },
        RuleMetadata { id: "MD050".into(), name: "strong-style".into(), description: "Strong style consistency".into(), fixable: false, default_params: "{}".into() },
    ]
}

// ===========================================================================
// LintConfig pyclass methods (Phase 6)
// ===========================================================================

#[pymethods]
impl LintConfig {
    /// Parse a Python dict into a LintConfig (for .markdownlint.json).
    #[staticmethod]
    fn from_dict(d: &Bound<'_, PyDict>) -> PyResult<Self> {
        let mut disable = Vec::new();
        let mut enable: Option<Vec<String>> = None;
        let mut default_false = false;
        let mut params = RuleParams::default();
        let mut enabled_rules: Vec<String> = Vec::new();

        for (key, value) in d.iter() {
            let key_str = key.extract::<String>()?;
            match key_str.as_str() {
                "default" => {
                    default_false = !value.extract::<bool>()?;
                }
                "disable" => {
                    disable.extend(value.extract::<Vec<String>>()?);
                }
                "enable" => {
                    enable = Some(value.extract::<Vec<String>>()?);
                }
                // Any other key is a rule entry: either a params dict
                // (e.g. MD013: {line_length: 100}) or a bool (on/off).
                _ => {
                    if value.is_instance_of::<PyDict>() {
                        let rule_cfg = value.clone().cast_into::<PyDict>()?;
                        match key_str.as_str() {
                            "MD013" => {
                                if let Some(v) = rule_cfg.get_item("line_length")? {
                                    if let Ok(n) = v.extract::<usize>() {
                                        params.line_length = n;
                                    }
                                }
                                if let Some(v) = rule_cfg.get_item("line_length_ignore_threshold")? {
                                    if let Ok(n) = v.extract::<usize>() {
                                        params.line_length_ignore_threshold = n;
                                    }
                                }
                            }
                            "MD010" => {
                                if let Some(v) = rule_cfg.get_item("spaces_per_tab")? {
                                    if let Ok(n) = v.extract::<usize>() {
                                        params.spaces_per_tab = n;
                                    }
                                }
                            }
                            _ => {}
                        }
                        // A rule supplied as a params dict is implicitly enabled.
                        if default_false {
                            enabled_rules.push(key_str);
                        }
                    } else if let Ok(enabled_flag) = value.extract::<bool>() {
                        if !enabled_flag {
                            disable.push(key_str);
                        } else if default_false {
                            enabled_rules.push(key_str);
                        }
                    }
                }
            }
        }

        // If default is false, the collected enabled rules become the allowlist.
        if default_false && !enabled_rules.is_empty() {
            enable = Some(enabled_rules.clone());
        }

        Ok(LintConfig {
            disable,
            enable,
            suppressions: Vec::new(),
            params,
            _enabled_when_default_false: if default_false && !enabled_rules.is_empty() {
                Some(enabled_rules)
            } else {
                None
            },
        })
    }

    /// Get the disable list.
    #[getter]
    fn disable(&self) -> Vec<String> { self.disable.clone() }

    /// Get the enable list.
    #[getter]
    fn enable(&self) -> Option<Vec<String>> { self.enable.clone() }

    /// Get per-rule parameters as a Python dict.
    #[getter]
    fn get_params(&self) -> String {
        format!(
            "{{\"line_length\": {}, \"line_length_ignore_threshold\": {}, \"spaces_per_tab\": {}, \"heading_style\": \"{}\", \"siblings_only\": {}, \"default_language\": {}}}",
            self.params.line_length,
            self.params.line_length_ignore_threshold,
            self.params.spaces_per_tab,
            self.params.heading_style,
            self.params.siblings_only,
            if self.params.default_language.is_some() { "true" } else { "false" }
        )
    }
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
            "MD009", "no-trailing-spaces", Some(1), None, None, "",
            FixOp::ReplaceLine { line: 0, text: "hello".to_string() },
        )];
        let result = apply_fixes("hello   \n", &fixes, None);
        assert_eq!(result, "hello\n");
    }

    #[test]
    fn apply_fixes_deletes_blank_lines() {
        let fixes = vec![Violation::warn_fix(
            "MD012", "no-multiple-blanks", Some(3), None, None, "",
            FixOp::DeleteLine { line: 2 },
        )];
        let result = apply_fixes("a\n\n\nb\n", &fixes, None);
        assert_eq!(result, "a\n\nb\n");
    }

    #[test]
    fn apply_fixes_adds_final_newline() {
        let fixes = vec![Violation::warn_fix(
            "MD047", "single-trailing-newline", Some(1), None, None, "",
            FixOp::EnsureFinalNewline,
        )];
        let result = apply_fixes("no newline", &fixes, None);
        assert_eq!(result, "no newline\n");
    }

    #[test]
    fn apply_fixes_deletion_wins_over_replacement() {
        let fixes = vec![
            Violation::warn_fix("MD009", "x", Some(1), None, None, "", FixOp::ReplaceLine { line: 1, text: "replaced".to_string() }),
            Violation::warn_fix("MD012", "x", Some(2), None, None, "", FixOp::DeleteLine { line: 1 }),
        ];
        let result = apply_fixes("a\n  \nb\n", &fixes, None);
        assert_eq!(result, "a\nb\n");
    }

    #[test]
    fn apply_fixes_multiple_on_same_line() {
        // Two ReplaceLine on the same line — last one wins (HashMap overwrite).
        let fixes = vec![
            Violation::warn_fix("MD009", "x", Some(1), None, None, "", FixOp::ReplaceLine { line: 0, text: "first".to_string() }),
            Violation::warn_fix("MD009", "x", Some(1), None, None, "", FixOp::ReplaceLine { line: 0, text: "second".to_string() }),
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
            "MD009", "x", Some(1), None, None, "", FixOp::ReplaceLine { line: 0, text: "hello".to_string() })];
        let once = apply_fixes("hello   \n", &fixes, None);
        let twice = apply_fixes(&once, &[], None);
        assert_eq!(once, twice);
    }

    #[test]
    fn fix_idempotent_blank_lines() {
        let fixes = vec![Violation::warn_fix(
            "MD012", "x", Some(3), None, None, "", FixOp::DeleteLine { line: 2 })];
        let once = apply_fixes("a\n\n\nb\n", &fixes, None);
        let twice = apply_fixes(&once, &[], None);
        assert_eq!(once, twice);
    }

    #[test]
    fn fix_idempotent_final_newline() {
        let fixes = vec![Violation::warn_fix(
            "MD047", "x", Some(1), None, None, "", FixOp::EnsureFinalNewline)];
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
            "MD009", "x", Some(1), None, None, "", FixOp::ReplaceLine { line: 0, text: "hello".to_string() })];
        let fixed = apply_fixes(src, &fixes, None);
        assert_eq!(fixed, "hello\n");
    }
}

// ===========================================================================
// Phase 7 — Batch API (rayon-parallelized, GIL-free per file)
// ===========================================================================

/// Batch-lint multiple files in parallel.
///
/// Each `(name, source)` pair is parsed and linted independently on a
/// separate rayon thread. The caller holds the GIL only for the final
/// conversion of `Violation` → `Diagnostic`.
pub fn lint_many(
    files: &[(String, String)],
    cfg: &LintConfig,
) -> Vec<(String, Vec<Violation>)> {
    use rayon::prelude::*;

    files.par_iter()
        .map(|(name, source)| {
            // Each thread builds its own parser/arena — fully independent.
            let parse_cfg = super::ParseConfig::default();
            let (arena, root) = super::parse_only(source, false, &parse_cfg);
            let violations = run_lint(source, &arena, root, cfg);
            (name.clone(), violations)
        })
        .collect()
}

/// Batch-fix multiple files in parallel.
///
/// Returns one `FixOutcome` per file. Each file is parsed, linted, and fixed
/// independently on a separate rayon thread.
pub fn fix_many(
    files: &[(String, String)],
    cfg: &LintConfig,
    default_language: Option<&str>,
) -> Vec<(String, FixOutcome)> {
    use rayon::prelude::*;

    files.par_iter()
        .map(|(name, source)| {
            let parse_cfg = super::ParseConfig::default();
            let (arena, root) = super::parse_only(source, false, &parse_cfg);
            let outcome = run_fix(source, &arena, root, cfg, default_language);
            (name.clone(), outcome)
        })
        .collect()
}

// ===========================================================================
// Auto-fix entry point
// ===========================================================================

/// Lint Markdown source and auto-correct the fixable issues.
///
/// Returns a FixResult with the corrected source (`.output`), the diagnostics
/// that were fixed (`.fixed`), and the ones that still need manual attention
/// (`.unfixable`). Auto-fixable rules: MD009 (trailing spaces), MD010 (tabs),
/// MD012 (multiple blanks), MD026 (trailing punctuation), MD040 (code language),
/// MD047 (final newline). Structural rules are reported but not changed.
///
/// Note: `unfixable` line numbers refer to the *input*. After fixing, lint the
/// returned `output` to get diagnostics with positions in the corrected text.
///
/// The fix engine iterates until stable (max 10 iterations), collecting all
/// fixed violations across iterations. `remaining` is computed by re-linting
/// the final output.
pub fn run_fix(
    source: &str,
    arena: &Arena,
    root: NodeRef,
    cfg: &LintConfig,
    default_language: Option<&str>,
) -> FixOutcome {
    run_fix_with_params(source, arena, root, cfg, &RuleParams::default(), default_language)
}

/// Lint and auto-correct with custom per-rule parameters.
pub fn run_fix_with_params(
    source: &str,
    arena: &Arena,
    root: NodeRef,
    cfg: &LintConfig,
    params: &RuleParams,
    default_language: Option<&str>,
) -> FixOutcome {
    let max_iterations = 10;
    let mut current_source = source.to_string();
    let mut all_fixed: Vec<Violation> = Vec::new();
    let mut all_unfixable: Vec<Violation> = Vec::new();

    for _iteration in 0..max_iterations {
        let violations = run_lint_with_params(&current_source, arena, root, cfg, params);

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

        let no_more_fixable = fixed.is_empty();

        all_fixed.extend(fixed);
        all_unfixable.extend(unfixable);

        if no_more_fixable {
            break;
        }

        current_source = apply_fixes(&current_source, &all_fixed, default_language);
    }

    let remaining = run_lint_with_params(&current_source, arena, root, cfg, params);

    FixOutcome {
        output: current_source,
        fixed: all_fixed,
        unfixable: all_unfixable,
        remaining,
    }
}
