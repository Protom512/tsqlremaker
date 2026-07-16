//! Error classification for LSP handler visibility (#139).
//!
//! Splits a handler outcome into one of two classes (see
//! `.kiro/specs/error-handling-visibility/design-error-taxonomy.md` for the
//! full per-handler mapping table):
//!
//! - **Class A — normal no-op** (empty source, document not open, no token at
//!   cursor, unresolvable symbol): the *contract*. Stays silent — no log at
//!   WARN/ERROR, no notification. Logging these would spam on every keystroke.
//! - **Class B — recoverable error** (caught panic, parse errors behind an
//!   empty result, broken span): always logged through `tracing`; only a
//!   caught panic additionally notifies the user via `window/showMessage`.
//!
//! Parse errors (B2) stay **log-only**: they are already published to the
//! editor as diagnostics, so a status-bar notification would double-report.

use crate::panic_recovery::CaughtPanic;
use ase_ls_core::analysis::DocumentAnalysis;

/// A recoverable-error cause that may warrant user-visible feedback.
///
/// Only [`Self::CaughtPanic`] is notified today (`window/showMessage`
/// WARNING). The enum is deliberately non-exhaustive-style so a future
/// "broken-span-yielded-nothing" (B3) cause can be added without touching
/// call sites; parse errors remain log-only ([`log_parse_errors_if_any`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoverableCause {
    /// A panic was caught in the handler's core call (taxonomy B1).
    CaughtPanic,
}

impl RecoverableCause {
    /// Human-facing message shown via `window/showMessage`.
    ///
    /// Kept short and non-alarming: the server recovered, and the detail lives
    /// in the server log for the developer.
    #[must_use]
    pub fn message(self, feature: &str) -> String {
        match self {
            Self::CaughtPanic => {
                format!(
                    "ase-ls: '{feature}' hit an internal error and recovered. See the server log."
                )
            }
        }
    }
}

/// Convert a caught panic into a recoverable cause (taxonomy B1 → notify).
#[must_use]
pub const fn from_panic(_: CaughtPanic) -> RecoverableCause {
    RecoverableCause::CaughtPanic
}

/// Structurally log parse errors (taxonomy B2, **log-only**) when a handler
/// produced no result on a document that failed to parse cleanly.
///
/// Parse errors are already surfaced to the editor as diagnostics, so this
/// emits only a server-side `WARN` trace — it gives a developer the context
/// they need when a feature is unresponsive, without double-notifying the
/// user. No-op when the analysis parsed cleanly (`parse_errors` empty).
///
/// Each error is logged individually with its message, position and span so
/// the trace is greppable and ordered.
pub fn log_parse_errors_if_any(analysis: &DocumentAnalysis, feature: &'static str, uri: &str) {
    if analysis.parse_errors.is_empty() {
        return;
    }
    let count = analysis.parse_errors.len();
    for (index, err) in analysis.parse_errors.iter().enumerate() {
        let position = err.position();
        let span = err.span();
        tracing::warn!(
            feature,
            uri,
            error.index = index,
            error.count = count,
            message = %err,
            line = position.line,
            column = position.column,
            offset = position.offset,
            span.start = span.map(|s| s.start).unwrap_or(0),
            span.end = span.map(|s| s.end).unwrap_or(0),
            "feature produced no result on a document with parse errors",
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn caught_panic_message_names_the_feature() {
        let msg = RecoverableCause::CaughtPanic.message("hover");
        assert!(
            msg.contains("hover"),
            "message should name the feature: {msg}"
        );
        assert!(
            msg.contains("recovered"),
            "message should reassure recovery: {msg}"
        );
    }

    #[test]
    fn from_panic_maps_to_caught_panic() {
        assert_eq!(from_panic(CaughtPanic), RecoverableCause::CaughtPanic);
    }

    #[test]
    fn log_parse_errors_is_silent_for_clean_analysis() {
        // A document that parses cleanly has empty parse_errors → no logging
        // side-effect. The function returns () so we assert it does not panic
        // and that parse_errors is empty on a trivial clean document.
        let analysis = DocumentAnalysis::new("SELECT 1");
        assert!(analysis.parse_errors.is_empty());
        // Should be a no-op (no panic, no return value).
        log_parse_errors_if_any(&analysis, "hover", "file:///test.sql");
    }

    #[test]
    fn log_parse_errors_handles_unparseable_input() {
        // An input that fails to parse has non-empty parse_errors → the helper
        // iterates them. We only assert it does not panic on real errors
        // (BatchError recursion etc.); the trace goes to the tracing layer.
        let analysis = DocumentAnalysis::new("SELECT FROM WHERE ((((");
        // Whether this produces parse errors is parser-dependent; if it does,
        // the helper must walk them without panicking.
        log_parse_errors_if_any(&analysis, "goto_definition", "file:///test.sql");
    }
}
