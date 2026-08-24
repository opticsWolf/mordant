//! Markdown linter for mordant (Python bindings).
//!
//! The lint engine lives in the core `mordant` crate (feature `linter`).
//! This module re-exports the engine API and provides the Python-exposed
//! types: `Diagnostic`, `FixResult`, `LintOptions`, `LintConfig` and
//! `RuleMetadata`, plus the rayon-parallelized batch helpers.

#![allow(dead_code)]

pub use mordant_lib::linter::{
    lint_rules as core_lint_rules, parse_suppressions, run_fix, run_fix_with_params, run_lint,
    run_lint_with_params, Edit, FixOp, FixOutcome, RuleParams, RuleSpec, Severity,
    SuppressionDirective, Violation,
};
pub use mordant_lib::linter::LintConfig as PlainLintConfig;

use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

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



/// Plain-Rust lint configuration (Python-exposed wrapper around the core config).
#[pyclass(module = "mordant")]
#[derive(Debug, Clone, Default)]
pub struct LintConfig(pub PlainLintConfig);

impl std::ops::Deref for LintConfig {
    type Target = PlainLintConfig;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl std::ops::DerefMut for LintConfig {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl LintConfig {
    pub fn into_inner(self) -> PlainLintConfig { self.0 }
}

// ---------------------------------------------------------------------------
// LintConfig pymethods
// ---------------------------------------------------------------------------

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

        Ok(LintConfig(PlainLintConfig {
            disable,
            enable,
            suppressions: Vec::new(),
            params,
            _enabled_when_default_false: if default_false && !enabled_rules.is_empty() {
                Some(enabled_rules)
            } else {
                None
            },
        }))
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
        LintConfig(PlainLintConfig {
            disable: self.disable.clone(),
            enable: self.enable.clone(),
            suppressions: Vec::new(),
            params: RuleParams::default(),
            _enabled_when_default_false: None,
        })
    }
}


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
    core_lint_rules()
        .into_iter()
        .map(|s| RuleMetadata {
            id: s.id.to_string(),
            name: s.name.to_string(),
            description: s.description.to_string(),
            fixable: s.fixable,
            default_params: s.default_params.to_string(),
        })
        .collect()
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
            let (arena, root) = super::parse_only(source, None, &parse_cfg);
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
            let (arena, root) = super::parse_only(source, None, &parse_cfg);
            let outcome = run_fix(source, &arena, root, cfg, default_language);
            (name.clone(), outcome)
        })
        .collect()
}

