//! Diagnostics 生成
//!
//! パーサーのエラーを LSP Diagnostic に変換する。

use crate::offset_to_position;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use tsql_parser::ParseError;

/// ParseError を LSP Diagnostic に変換する
pub fn parse_errors_to_diagnostics(source: &str, errors: &[ParseError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|e| parse_error_to_diagnostic(source, e))
        .collect()
}

/// 単一の ParseError を Diagnostic に変換する
fn parse_error_to_diagnostic(source: &str, error: &ParseError) -> Diagnostic {
    let range = error_range(source, error);
    let message = format!("{error}");

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("ase-ls".to_string()),
        message,
        ..Diagnostic::default()
    }
}

/// ParseError から Range を取得する
fn error_range(source: &str, error: &ParseError) -> Range {
    match error.span() {
        Some(span) => {
            let start = offset_to_position(source, span.start);
            let end = offset_to_position(source, span.end.max(span.start + 1));
            Range {
                start: Position {
                    line: start.0,
                    character: start.1,
                },
                end: Position {
                    line: end.0,
                    character: end.1,
                },
            }
        }
        None => {
            let pos = error.position();
            Range {
                start: Position {
                    line: pos.line.saturating_sub(1),
                    character: pos.column.saturating_sub(1),
                },
                end: Position {
                    line: pos.line.saturating_sub(1),
                    character: pos.column.saturating_sub(1) + 1,
                },
            }
        }
    }
}

/// ソースコードの完全な診断を実行する
pub fn diagnose_source(source: &str) -> Vec<Diagnostic> {
    let mut parser = tsql_parser::Parser::new(source);
    match parser.parse_with_errors() {
        Ok((_stmts, errors)) => parse_errors_to_diagnostics(source, &errors),
        Err(errs) => parse_errors_to_diagnostics(source, &errs.errors),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_sql_no_diagnostics() {
        let diags = diagnose_source("SELECT * FROM users");
        assert!(diags.is_empty());
    }

    #[test]
    fn test_invalid_sql_has_diagnostics() {
        let diags = diagnose_source("SELCT * FROM users");
        assert!(!diags.is_empty());
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(diags[0].source.as_deref(), Some("ase-ls"));
    }

    #[test]
    fn test_diagnostic_range() {
        let diags = diagnose_source("SELCT * FROM users");
        assert!(!diags.is_empty());
        // Error should be at position 0
        assert_eq!(diags[0].range.start.line, 0);
        assert_eq!(diags[0].range.start.character, 0);
    }

    #[test]
    fn test_diagnostic_has_message() {
        let diags = diagnose_source("SELCT * FROM users");
        assert!(!diags.is_empty());
        assert!(!diags[0].message.is_empty());
    }
}
