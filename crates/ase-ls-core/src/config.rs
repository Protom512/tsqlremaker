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
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            enable_snippets: true,
        }
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
            },
        };
        let json_str = serde_json::to_string(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&json_str).unwrap();
        assert_eq!(cfg, parsed);
    }
}
