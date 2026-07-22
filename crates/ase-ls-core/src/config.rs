//! LSP configuration (#132).
//!
//! User-configurable settings received via `workspace/didChangeConfiguration`.
//! All defaults reproduce the pre-#132 hardcoded behaviour exactly, so an
//! unconfigured server behaves identically to before — configuration only
//! overrides when the client explicitly sends a value.
//!
//! ## Wire format
//!
//! Clients send a JSON object in `DidChangeConfigurationParams.settings`.
//! VSCode namespaces extension settings, so we accept either:
//!
//! ```jsonc
//! // Namespaced (VSCode): preferred
//! { "ase-ls": { "formatting": { "indentWidth": 2 } } }
//!
//! // Raw (other clients): also accepted
//! { "formatting": { "indentWidth": 2 } }
//! ```
//!
//! Field names are `camelCase` to match editor conventions. Unknown fields are
//! ignored, and any per-section deserialisation error falls back to that
//! section's defaults (lenient — a config typo never propagates a parse error
//! to the client or resets unrelated sections).

use lsp_types::DiagnosticSeverity;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Root configuration. Aggregates the three configurable subsystems.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Formatting rules (indent width, keyword casing).
    pub formatting: FormattingConfig,
    /// Diagnostic behaviour (e.g. `SELECT *` severity).
    pub diagnostics: DiagnosticsConfig,
    /// Completion behaviour (snippet emission).
    pub completion: CompletionConfig,
    /// Inlay hint behaviour (#118).
    pub inlay: InlayConfig,
    /// Document link behaviour (#119).
    pub document_link: DocumentLinkConfig,
}

impl Config {
    /// Build a [`Config`] from a `workspace/didChangeConfiguration` payload.
    ///
    /// Tries the `ase-ls` namespace first (VSCode convention), then the raw
    /// value. Each section is parsed independently so a typo in one section
    /// only resets that section, not the others.
    #[must_use]
    pub fn from_value(settings: &serde_json::Value) -> Self {
        let root = settings.get("ase-ls").unwrap_or(settings);
        Self {
            formatting: from_section(root.get("formatting")),
            diagnostics: from_section(root.get("diagnostics")),
            completion: from_section(root.get("completion")),
            inlay: from_section(root.get("inlay")),
            document_link: from_section(root.get("documentLink")),
        }
    }
}

/// Deserialise a section, falling back to `T::default()` on absence or error.
fn from_section<T: DeserializeOwned + Default>(value: Option<&serde_json::Value>) -> T {
    match value {
        Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
        None => T::default(),
    }
}

/// Keyword casing applied during formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KeywordCase {
    /// Upper-case all keywords (pre-#132 behaviour, and the default).
    #[default]
    Upper,
    /// Lower-case all keywords.
    Lower,
    /// Leave keyword casing untouched.
    Preserve,
}

/// Formatting rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FormattingConfig {
    /// Spaces per indent level.
    pub indent_width: u32,
    /// Keyword casing.
    pub keyword_case: KeywordCase,
}

impl Default for FormattingConfig {
    fn default() -> Self {
        Self {
            indent_width: 4,
            keyword_case: KeywordCase::Upper,
        }
    }
}

impl FormattingConfig {
    /// The indent string for one level (e.g. `"    "` for width 4).
    ///
    /// Allocated once per format run; the formatting loop reuses the same
    /// `String` for every indent level via [`str::repeat`].
    #[must_use]
    pub fn indent_unit(&self) -> String {
        " ".repeat(self.indent_width as usize)
    }
}

/// Selectively overridable diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// `DiagnosticSeverity::Error` (1).
    Error,
    /// `DiagnosticSeverity::Warning` (2).
    #[default]
    Warning,
    /// `DiagnosticSeverity::Information` (3).
    Information,
    /// `DiagnosticSeverity::Hint` (4).
    Hint,
}

impl From<Severity> for DiagnosticSeverity {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Information => DiagnosticSeverity::INFORMATION,
            Severity::Hint => DiagnosticSeverity::HINT,
        }
    }
}

/// Diagnostic behaviour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiagnosticsConfig {
    /// Severity for `SELECT *` warnings (default `Warning`).
    pub select_star_severity: Severity,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            select_star_severity: Severity::Warning,
        }
    }
}

/// Completion behaviour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CompletionConfig {
    /// Emit function completions as LSP snippets with parameter placeholders.
    ///
    /// When `false`, function completions fall back to plain text (just the
    /// function name followed by `()`). Default `true` (pre-#132 behaviour).
    pub enable_snippets: bool,
    /// Emit column-name completions in cursor contexts that follow a table
    /// reference (#54 context-aware completion).
    ///
    /// When `false`, column suggestions are suppressed regardless of cursor
    /// position. Default `true` — an unconfigured server surfaces columns
    /// (post-#54 intent; users opt out, not in).
    pub enable_column_completion: bool,
    /// Prepend in-scope variable completions (`@var`) to the candidate list
    /// in expression contexts (#54 context-aware completion).
    ///
    /// When `false`, variables are omitted from the completion list. Default
    /// `true` — an unconfigured server surfaces declared variables.
    pub enable_variable_completion: bool,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            enable_snippets: true,
            enable_column_completion: true,
            enable_variable_completion: true,
        }
    }
}

/// Inlay hint behaviour (#118).
///
/// Controls whether `textDocument/inlayHint` emits variable-type and
/// parameter-name annotations. Both default to `true`; an unconfigured server
/// surfaces every supported hint kind (pre-#118 intent — users opt out, not
/// in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct InlayConfig {
    /// Emit type annotations after `DECLARE` variables (e.g. `: INT`).
    pub enable_variable_types: bool,
    /// Emit parameter-name annotations at `EXEC` call sites.
    pub enable_parameter_names: bool,
}

impl Default for InlayConfig {
    fn default() -> Self {
        Self {
            enable_variable_types: true,
            enable_parameter_names: true,
        }
    }
}

/// Document link behaviour (#119).
///
/// Controls whether `textDocument/documentLink` emits clickable links for
/// SQLCMD `:r` include directives. Defaults to `true`; an unconfigured server
/// surfaces every supported link (pre-#119 intent — users opt out, not in).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DocumentLinkConfig {
    /// Emit document links for SQLCMD `:r` file-include directives.
    pub enable: bool,
}

impl Default for DocumentLinkConfig {
    fn default() -> Self {
        Self { enable: true }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_reproduce_pre_132_behaviour() {
        let cfg = Config::default();
        assert_eq!(cfg.formatting.indent_width, 4);
        assert_eq!(cfg.formatting.keyword_case, KeywordCase::Upper);
        assert_eq!(cfg.diagnostics.select_star_severity, Severity::Warning);
        assert!(cfg.completion.enable_snippets);
        assert_eq!(cfg.formatting.indent_unit(), "    ");
    }

    #[test]
    fn from_value_missing_sections_uses_defaults() {
        let cfg = Config::from_value(&json!({}));
        assert_eq!(cfg.formatting.indent_width, 4);
        assert_eq!(cfg.diagnostics.select_star_severity, Severity::Warning);
        assert!(cfg.completion.enable_snippets);
    }

    #[test]
    fn from_value_namespaced_overrides() {
        let cfg = Config::from_value(&json!({
            "ase-ls": {
                "formatting": { "indentWidth": 2 },
                "diagnostics": { "selectStarSeverity": "hint" },
                "completion": { "enableSnippets": false }
            }
        }));
        assert_eq!(cfg.formatting.indent_width, 2);
        assert_eq!(cfg.formatting.indent_unit(), "  ");
        assert_eq!(cfg.diagnostics.select_star_severity, Severity::Hint);
        assert!(!cfg.completion.enable_snippets);
    }

    #[test]
    fn from_value_raw_namespace_overrides() {
        // No "ase-ls" wrapper — raw root is accepted.
        let cfg = Config::from_value(&json!({
            "formatting": { "keywordCase": "preserve" }
        }));
        assert_eq!(cfg.formatting.keyword_case, KeywordCase::Preserve);
    }

    #[test]
    fn from_value_invalid_section_falls_back_to_default_for_that_section_only() {
        // keywordCase typo invalidates the formatting section only;
        // diagnostics/completion overrides survive.
        let cfg = Config::from_value(&json!({
            "ase-ls": {
                "formatting": { "keywordCase": "uper" },
                "diagnostics": { "selectStarSeverity": "error" }
            }
        }));
        assert_eq!(
            cfg.formatting.keyword_case,
            KeywordCase::Upper,
            "invalid formatting resets to default"
        );
        assert_eq!(
            cfg.diagnostics.select_star_severity,
            Severity::Error,
            "valid diagnostics override survives"
        );
    }

    #[test]
    fn from_value_non_object_settings_uses_all_defaults() {
        // A non-object payload (e.g. a bare string) cannot contain sections.
        let cfg = Config::from_value(&json!("not an object"));
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn severity_maps_to_lsp_diagnostic_severity() {
        assert_eq!(
            DiagnosticSeverity::from(Severity::Error),
            DiagnosticSeverity::ERROR
        );
        assert_eq!(
            DiagnosticSeverity::from(Severity::Warning),
            DiagnosticSeverity::WARNING
        );
        assert_eq!(
            DiagnosticSeverity::from(Severity::Information),
            DiagnosticSeverity::INFORMATION
        );
        assert_eq!(
            DiagnosticSeverity::from(Severity::Hint),
            DiagnosticSeverity::HINT
        );
    }

    #[test]
    fn indent_unit_respects_width() {
        fn fmt_with_width(width: u32) -> FormattingConfig {
            FormattingConfig {
                indent_width: width,
                ..Default::default()
            }
        }
        assert_eq!(fmt_with_width(0).indent_unit(), "");
        assert_eq!(fmt_with_width(2).indent_unit(), "  ");
        assert_eq!(fmt_with_width(8).indent_unit(), "        ");
    }

    #[test]
    fn config_round_trips_through_serde() {
        let cfg = Config {
            formatting: FormattingConfig {
                indent_width: 2,
                keyword_case: KeywordCase::Lower,
            },
            diagnostics: DiagnosticsConfig {
                select_star_severity: Severity::Hint,
            },
            completion: CompletionConfig {
                enable_snippets: false,
                enable_column_completion: false,
                enable_variable_completion: false,
            },
            inlay: InlayConfig::default(),
            document_link: DocumentLinkConfig::default(),
        };
        let json_str = serde_json::to_string(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&json_str).unwrap();
        assert_eq!(cfg, parsed);
    }

    // ------------------------------------------------------------------
    // CompletionConfig context knobs (#54)
    // ------------------------------------------------------------------

    #[test]
    fn completion_defaults_preserve_new_behaviour() {
        let comp = CompletionConfig::default();
        assert!(comp.enable_snippets, "pre-existing knob unchanged");
        assert!(
            comp.enable_column_completion,
            "new knob defaults to true (preserve post-#54 behaviour)"
        );
        assert!(
            comp.enable_variable_completion,
            "new knob defaults to true (preserve post-#54 behaviour)"
        );
    }

    #[test]
    fn from_value_missing_completion_section_uses_new_defaults() {
        let cfg = Config::from_value(&json!({}));
        assert!(cfg.completion.enable_snippets);
        assert!(cfg.completion.enable_column_completion);
        assert!(cfg.completion.enable_variable_completion);
    }

    #[test]
    fn from_value_completion_context_knobs_namespaced_override() {
        let cfg = Config::from_value(&json!({
            "ase-ls": {
                "completion": {
                    "enableColumnCompletion": false,
                    "enableVariableCompletion": false
                }
            }
        }));
        assert!(
            cfg.completion.enable_snippets,
            "unspecified enable_snippets keeps its default"
        );
        assert!(
            !cfg.completion.enable_column_completion,
            "camelCase override should be honoured"
        );
        assert!(
            !cfg.completion.enable_variable_completion,
            "camelCase override should be honoured"
        );
    }

    #[test]
    fn from_value_completion_context_knobs_raw_namespace_override() {
        // No "ase-ls" wrapper — raw root is accepted.
        let cfg = Config::from_value(&json!({
            "completion": { "enableColumnCompletion": false }
        }));
        assert!(!cfg.completion.enable_column_completion);
        assert!(
            cfg.completion.enable_variable_completion,
            "unspecified field keeps its default"
        );
    }

    #[test]
    fn from_value_invalid_completion_section_falls_back_to_default_only() {
        // enableColumnCompletion has a non-boolean value → completion section
        // resets; a valid inlay override in the same payload must survive.
        let cfg = Config::from_value(&json!({
            "ase-ls": {
                "completion": { "enableColumnCompletion": "yes" },
                "inlay": { "enableVariableTypes": false }
            }
        }));
        assert!(
            cfg.completion.enable_snippets,
            "invalid completion section resets to default (true)"
        );
        assert!(
            cfg.completion.enable_column_completion,
            "invalid completion section resets to default (true)"
        );
        assert!(
            cfg.completion.enable_variable_completion,
            "invalid completion section resets to default (true)"
        );
        assert!(
            !cfg.inlay.enable_variable_types,
            "valid inlay override survives completion failure"
        );
    }

    #[test]
    fn completion_context_knobs_round_trip_through_serde() {
        let comp = CompletionConfig {
            enable_snippets: true,
            enable_column_completion: false,
            enable_variable_completion: false,
        };
        let json_str = serde_json::to_string(&comp).unwrap();
        let parsed: CompletionConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(comp, parsed);

        // Confirm wire format is camelCase for the new fields.
        assert!(
            json_str.contains("enableColumnCompletion"),
            "expected camelCase field in serialized output, got: {json_str}"
        );
        assert!(
            json_str.contains("enableVariableCompletion"),
            "expected camelCase field in serialized output, got: {json_str}"
        );
    }

    // ------------------------------------------------------------------
    // InlayConfig (#118)
    // ------------------------------------------------------------------

    #[test]
    fn inlay_defaults_are_true() {
        let inlay = InlayConfig::default();
        assert!(inlay.enable_variable_types);
        assert!(inlay.enable_parameter_names);
    }

    #[test]
    fn from_value_missing_inlay_section_uses_defaults() {
        let cfg = Config::from_value(&json!({}));
        assert!(cfg.inlay.enable_variable_types);
        assert!(cfg.inlay.enable_parameter_names);
    }

    #[test]
    fn from_value_inlay_namespaced_override() {
        let cfg = Config::from_value(&json!({
            "ase-ls": {
                "inlay": { "enableVariableTypes": false }
            }
        }));
        assert!(
            !cfg.inlay.enable_variable_types,
            "camelCase override should be honoured"
        );
        assert!(
            cfg.inlay.enable_parameter_names,
            "unspecified fields keep their defaults"
        );
    }

    #[test]
    fn from_value_inlay_raw_namespace_override() {
        let cfg = Config::from_value(&json!({
            "inlay": { "enableParameterNames": false }
        }));
        assert!(cfg.inlay.enable_variable_types);
        assert!(!cfg.inlay.enable_parameter_names);
    }

    #[test]
    fn from_value_invalid_inlay_section_falls_back_to_default_only() {
        // enableVariableTypes has a non-boolean value → inlay section resets;
        // a valid completion override in the same payload must survive.
        let cfg = Config::from_value(&json!({
            "ase-ls": {
                "inlay": { "enableVariableTypes": "yes" },
                "completion": { "enableSnippets": false }
            }
        }));
        assert!(
            cfg.inlay.enable_variable_types,
            "invalid inlay section resets to default (true)"
        );
        assert!(
            cfg.inlay.enable_parameter_names,
            "invalid inlay section resets to default (true)"
        );
        assert!(
            !cfg.completion.enable_snippets,
            "valid completion override survives inlay failure"
        );
    }

    #[test]
    fn inlay_config_round_trips_through_serde() {
        let inlay = InlayConfig {
            enable_variable_types: false,
            enable_parameter_names: false,
        };
        let json_str = serde_json::to_string(&inlay).unwrap();
        let parsed: InlayConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(inlay, parsed);

        // Confirm wire format is camelCase.
        assert!(
            json_str.contains("enableVariableTypes"),
            "expected camelCase field in serialized output, got: {json_str}"
        );
    }

    // ------------------------------------------------------------------
    // DocumentLinkConfig (#119)
    // ------------------------------------------------------------------

    #[test]
    fn document_link_defaults_are_true() {
        let document_link = DocumentLinkConfig::default();
        assert!(document_link.enable);
    }

    #[test]
    fn from_value_missing_document_link_section_uses_defaults() {
        let cfg = Config::from_value(&json!({}));
        assert!(cfg.document_link.enable);
    }

    #[test]
    fn from_value_document_link_namespaced_override() {
        let cfg = Config::from_value(&json!({
            "ase-ls": {
                "documentLink": { "enable": false }
            }
        }));
        assert!(
            !cfg.document_link.enable,
            "namespaced override should be honoured"
        );
    }

    #[test]
    fn from_value_document_link_raw_namespace_override() {
        // No "ase-ls" wrapper — raw root is accepted.
        let cfg = Config::from_value(&json!({
            "documentLink": { "enable": false }
        }));
        assert!(!cfg.document_link.enable);
    }

    #[test]
    fn from_value_invalid_document_link_section_falls_back_to_default_only() {
        // enable has a non-boolean value → documentLink section resets;
        // a valid completion override in the same payload must survive.
        let cfg = Config::from_value(&json!({
            "ase-ls": {
                "documentLink": { "enable": "yes" },
                "completion": { "enableSnippets": false }
            }
        }));
        assert!(
            cfg.document_link.enable,
            "invalid documentLink section resets to default (true)"
        );
        assert!(
            !cfg.completion.enable_snippets,
            "valid completion override survives documentLink failure"
        );
    }

    #[test]
    fn document_link_config_round_trips_through_serde() {
        let document_link = DocumentLinkConfig { enable: false };
        let json_str = serde_json::to_string(&document_link).unwrap();
        let parsed: DocumentLinkConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(document_link, parsed);

        // Confirm wire format is camelCase.
        assert!(
            json_str.contains("enable"),
            "expected enable field in serialized output, got: {json_str}"
        );
    }
}
