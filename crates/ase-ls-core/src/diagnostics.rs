//! Diagnostics 生成
//!
//! パーサーのエラーを LSP Diagnostic に変換する。
//! セマンティック診断（SELECT * 警告等）も提供する。

use crate::analysis::DocumentAnalysis;
use crate::line_index::LineIndex;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use tsql_parser::ast::{SelectItem, Statement};
use tsql_parser::ParseError;
use tsql_token::TokenKind;

/// DocumentAnalysisから診断を生成する（キャッシュ利用）
pub fn diagnose(analysis: &DocumentAnalysis) -> Vec<Diagnostic> {
    let mut diags: Vec<Diagnostic> = analysis
        .parse_errors
        .iter()
        .map(|e| parse_error_to_diagnostic(&analysis.line_index, e))
        .collect();

    diags.extend(semantic_diagnostics(analysis));
    diags
}

/// ASTベースのセマンティック診断を生成する
fn semantic_diagnostics(analysis: &DocumentAnalysis) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for stmt in &analysis.statements {
        collect_select_star_warnings(stmt, analysis, &mut diags);
    }
    diags
}

/// Statementツリーを再帰的に走査し、SELECT * の警告を収集する
fn collect_select_star_warnings(
    stmt: &Statement,
    analysis: &DocumentAnalysis,
    diags: &mut Vec<Diagnostic>,
) {
    match stmt {
        Statement::Select(sel) => {
            for item in &sel.columns {
                if let SelectItem::Wildcard = item {
                    if let Some(diag) = make_star_diagnostic(analysis, &sel.span) {
                        diags.push(diag);
                    }
                }
            }
        }
        Statement::If(if_stmt) => {
            collect_select_star_warnings(&if_stmt.then_branch, analysis, diags);
            if let Some(else_branch) = &if_stmt.else_branch {
                collect_select_star_warnings(else_branch, analysis, diags);
            }
        }
        Statement::While(while_stmt) => {
            collect_select_star_warnings(&while_stmt.body, analysis, diags);
        }
        Statement::Block(block) => {
            for child in &block.statements {
                collect_select_star_warnings(child, analysis, diags);
            }
        }
        Statement::TryCatch(try_catch) => {
            for child in &try_catch.try_block.statements {
                collect_select_star_warnings(child, analysis, diags);
            }
            for child in &try_catch.catch_block.statements {
                collect_select_star_warnings(child, analysis, diags);
            }
        }
        Statement::Create(create) => {
            if let tsql_parser::ast::CreateStatement::Procedure(proc) = &**create {
                for child in &proc.body {
                    collect_select_star_warnings(child, analysis, diags);
                }
            }
        }
        _ => {}
    }
}

/// SELECT * に対応するWARNING Diagnosticを生成する
fn make_star_diagnostic(
    analysis: &DocumentAnalysis,
    select_span: &tsql_token::Span,
) -> Option<Diagnostic> {
    // SELECTキーワードの後の*トークンを探す
    let star_token = find_star_token_after_select(&analysis.tokens, select_span)?;
    let star_u32 = star_token.span.start;
    let end_u32 = star_token.span.end.max(star_u32 + 1);
    let start = analysis.line_index.offset_to_position(star_u32);
    let end = analysis.line_index.offset_to_position(end_u32);

    Some(Diagnostic {
        range: Range {
            start: Position {
                line: start.0,
                character: start.1,
            },
            end: Position {
                line: end.0,
                character: end.1,
            },
        },
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("ase-ls".to_string()),
        message: "SELECT *: consider specifying explicit columns for better performance and maintainability".to_string(),
        ..Diagnostic::default()
    })
}

/// SELECTスパン内で、SELECTキーワードの直後にあるStarトークンを探す
fn find_star_token_after_select<'a>(
    tokens: &'a [crate::analysis::OwnedToken],
    select_span: &tsql_token::Span,
) -> Option<&'a crate::analysis::OwnedToken> {
    let mut found_select = false;
    let mut select_end = select_span.end;
    // Parserの壊れたスパン対策: end=0なら幅を広げて探す
    if select_end == 0 || select_end <= select_span.start {
        // 大量トークンをスキャンしないよう制限（最大50トークン）
        select_end = select_span.start.saturating_add(200);
    }

    for tok in tokens {
        if tok.span.start < select_span.start {
            continue;
        }
        if tok.span.start > select_end {
            break;
        }
        if tok.kind == TokenKind::Select {
            found_select = true;
            continue;
        }
        if found_select && tok.kind == TokenKind::Star {
            return Some(tok);
        }
        // SELECTと*の間の空白等はスキップ、それ以外のトークンが来たら*はない
        if found_select
            && !matches!(
                tok.kind,
                TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
            )
        {
            found_select = false;
        }
    }
    None
}

/// ParseError を LSP Diagnostic に変換する
pub fn parse_errors_to_diagnostics(source: &str, errors: &[ParseError]) -> Vec<Diagnostic> {
    let line_index = LineIndex::new(source);
    errors
        .iter()
        .map(|e| parse_error_to_diagnostic(&line_index, e))
        .collect()
}

/// 単一の ParseError を Diagnostic に変換する
fn parse_error_to_diagnostic(line_index: &LineIndex, error: &ParseError) -> Diagnostic {
    let range = error_range(line_index, error);
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
fn error_range(line_index: &LineIndex, error: &ParseError) -> Range {
    match error.span() {
        Some(span) => {
            let start = line_index.offset_to_position(span.start);
            let end = line_index.offset_to_position(span.end.max(span.start + 1));
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

    #[test]
    fn test_diagnostic_range_not_default() {
        let diags = diagnose_source("SELCT * FROM users");
        assert!(!diags.is_empty());
        let range = diags[0].range;
        assert!(
            range.start.line > 0
                || range.start.character > 0
                || range.end.character > range.start.character
        );
    }

    #[test]
    fn test_diagnostic_end_after_start() {
        let diags = diagnose_source("SELCT * FROM users");
        assert!(!diags.is_empty());
        let range = diags[0].range;
        assert!(range.end.line >= range.start.line);
        if range.end.line == range.start.line {
            assert!(range.end.character > range.start.character);
        }
    }

    #[test]
    fn test_parse_errors_to_diagnostics_converts_all() {
        let errors = diagnose_source("SELCT FRO users");
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_valid_complex_sql_no_diagnostics() {
        let source = "CREATE TABLE t (id INT)\nINSERT INTO t (id) VALUES (1)\nSELECT * FROM t";
        let diags = diagnose_source(source);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_diagnose_includes_parse_errors() {
        let sources = ["SELCT * FROM", ""];
        for source in &sources {
            let analysis = crate::analysis::DocumentAnalysis::new(source);
            let from_analysis = diagnose(&analysis);
            let from_source = diagnose_source(source);
            // parse errors should be included in diagnose()
            assert!(
                from_analysis.len() >= from_source.len(),
                "diagnose() should include at least as many diagnostics as diagnose_source() for source: {source:?}"
            );
        }
    }

    #[test]
    fn test_diagnose_includes_semantic_warnings() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT * FROM users");
        let diags = diagnose(&analysis);
        // parse errors (0) + semantic warnings (1) = 1
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::WARNING));
    }

    // === Semantic diagnostics: SELECT * warning ===

    #[test]
    fn test_select_star_warns() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT * FROM users");
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT * should produce a semantic warning"
        );
    }

    #[test]
    fn test_select_columns_no_warning() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT id, name FROM users");
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            star_warnings.is_empty(),
            "Explicit columns should not produce SELECT * warning"
        );
    }

    #[test]
    fn test_select_star_warning_points_to_star() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT * FROM users");
        let diags = diagnose(&analysis);
        let star_diag = diags.iter().find(|d| d.message.contains("SELECT *"));
        assert!(star_diag.is_some());
        let d = star_diag.unwrap();
        // * is at character 7 (after "SELECT ")
        assert_eq!(d.range.start.character, 7);
        assert_eq!(d.range.end.character, 8);
        assert_eq!(d.severity, Some(DiagnosticSeverity::WARNING));
    }

    #[test]
    fn test_select_count_star_no_warning() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT COUNT(*) FROM users");
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            star_warnings.is_empty(),
            "COUNT(*) should not produce SELECT * warning"
        );
    }

    #[test]
    fn test_select_star_with_extra_columns_warns() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT *, id FROM users");
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT *, id should still warn about *"
        );
    }

    #[test]
    fn test_select_star_inside_if_block_warns() {
        let source = "IF 1 = 1\nBEGIN\n    SELECT * FROM users\nEND";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT * inside IF/BLOCK should produce warning"
        );
    }

    #[test]
    fn test_select_star_inside_while_warns() {
        let source = "WHILE 1 = 1\nBEGIN\n    SELECT * FROM t\nEND";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT * inside WHILE should produce warning"
        );
    }

    #[test]
    fn test_select_star_inside_procedure_warns() {
        let source = "CREATE PROCEDURE myproc AS BEGIN SELECT * FROM users END";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT * inside PROCEDURE should produce warning"
        );
    }
}
