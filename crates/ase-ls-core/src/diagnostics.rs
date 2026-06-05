//! Diagnostics 生成
//!
//! パーサーのエラーを LSP Diagnostic に変換する。
//! セマンティック診断（SELECT * 警告等）も提供する。

use crate::analysis::DocumentAnalysis;
use crate::line_index::LineIndex;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use tsql_parser::ast::{SelectItem, Statement};
use tsql_parser::ParseError;

/// Diagnostic source identifier shared across all diagnostic constructors.
const DIAGNOSTIC_SOURCE: &str = "ase-ls";
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
            // Recurse into subqueries in FROM clause
            if let Some(from_clause) = &sel.from {
                for table_ref in &from_clause.tables {
                    collect_from_table_ref(table_ref, analysis, diags);
                }
            }
        }
        Statement::Insert(insert) => {
            if let tsql_parser::ast::InsertSource::Select(sub_sel) = &insert.source {
                collect_select_star_warnings(&Statement::Select(sub_sel.clone()), analysis, diags);
            }
        }
        Statement::Update(update) => {
            collect_from_table_ref(&update.table, analysis, diags);
            if let Some(from_clause) = &update.from_clause {
                for table_ref in &from_clause.tables {
                    collect_from_table_ref(table_ref, analysis, diags);
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
        Statement::Create(create) => match &**create {
            tsql_parser::ast::CreateStatement::Procedure(proc) => {
                for child in &proc.body {
                    collect_select_star_warnings(child, analysis, diags);
                }
            }
            tsql_parser::ast::CreateStatement::View(view) => {
                collect_select_star_warnings(
                    &Statement::Select(view.query.clone()),
                    analysis,
                    diags,
                );
            }
            tsql_parser::ast::CreateStatement::Trigger(trigger) => {
                for child in &trigger.body {
                    collect_select_star_warnings(child, analysis, diags);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

/// Recurse into TableReference to find subqueries with SELECT *
fn collect_from_table_ref(
    table_ref: &tsql_parser::ast::TableReference,
    analysis: &DocumentAnalysis,
    diags: &mut Vec<Diagnostic>,
) {
    match table_ref {
        tsql_parser::ast::TableReference::Subquery { query, .. } => {
            collect_select_star_warnings(&Statement::Select(query.clone()), analysis, diags);
        }
        tsql_parser::ast::TableReference::Joined { joins, .. } => {
            for join in joins {
                collect_from_table_ref(&join.table, analysis, diags);
            }
        }
        tsql_parser::ast::TableReference::Table { .. } => {}
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

    Some(Diagnostic {
        range: analysis.line_index.offset_to_range(star_u32, end_u32),
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some(DIAGNOSTIC_SOURCE.to_string()),
        message: "SELECT *: consider specifying explicit columns for better performance and maintainability".to_string(),
        ..Diagnostic::default()
    })
}

/// Maximum byte range to scan for * token when parser span is broken
const BROKEN_SPAN_SCAN_LIMIT: u32 = 200;

/// SELECTスパン内で、SELECTキーワードの直後にあるStarトークンを探す
fn find_star_token_after_select<'a>(
    tokens: &'a [crate::analysis::OwnedToken],
    select_span: &tsql_token::Span,
) -> Option<&'a crate::analysis::OwnedToken> {
    let mut found_select = false;
    let mut select_end = select_span.end;
    // Parserの壊れたスパン対策: end が無効/不正なら start から一定バイト幅で探す
    if select_end <= select_span.start {
        // 過剰スキャンを避けるためバイト単位で上限を設ける
        select_end = select_span.start.saturating_add(BROKEN_SPAN_SCAN_LIMIT);
    }

    // Use binary search to find starting token index
    let start_idx = tokens.partition_point(|t| t.span.start < select_span.start);

    for tok in &tokens[start_idx..] {
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
        // Allow commas (for `SELECT col, *`), DISTINCT, ALL, TOP keywords
        if found_select
            && !matches!(
                tok.kind,
                TokenKind::Comma
                    | TokenKind::Distinct
                    | TokenKind::All
                    | TokenKind::Top
                    | TokenKind::Number
                    | TokenKind::Ident
            )
        {
            found_select = false;
        }
    }
    None
}

/// ParseError を LSP Diagnostic に変換する
fn parse_error_to_diagnostic(line_index: &LineIndex, error: &ParseError) -> Diagnostic {
    let range = error_range(line_index, error);
    let message = format!("{error}");

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some(DIAGNOSTIC_SOURCE.to_string()),
        message,
        ..Diagnostic::default()
    }
}

/// ParseError から Range を取得する
fn error_range(line_index: &LineIndex, error: &ParseError) -> Range {
    match error.span() {
        Some(span) => line_index.offset_to_range(span.start, span.end.max(span.start + 1)),
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_sql_no_parse_errors() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT id, name FROM users");
        let diags = diagnose(&analysis);
        let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(parse_errors.is_empty());
    }

    #[test]
    fn test_invalid_sql_has_diagnostics() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELCT * FROM users");
        let diags = diagnose(&analysis);
        let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(!parse_errors.is_empty());
        assert_eq!(parse_errors[0].source.as_deref(), Some("ase-ls"));
    }

    #[test]
    fn test_diagnostic_range() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELCT * FROM users");
        let diags = diagnose(&analysis);
        let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(!parse_errors.is_empty());
        // Error should be at position 0
        assert_eq!(parse_errors[0].range.start.line, 0);
        assert_eq!(parse_errors[0].range.start.character, 0);
    }

    #[test]
    fn test_diagnostic_has_message() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELCT * FROM users");
        let diags = diagnose(&analysis);
        let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(!parse_errors.is_empty());
        assert!(!parse_errors[0].message.is_empty());
    }

    #[test]
    fn test_diagnostic_range_not_default() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELCT * FROM users");
        let diags = diagnose(&analysis);
        let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(!parse_errors.is_empty());
        let range = parse_errors[0].range;
        assert!(
            range.start.line > 0
                || range.start.character > 0
                || range.end.character > range.start.character
        );
    }

    #[test]
    fn test_diagnostic_end_after_start() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELCT * FROM users");
        let diags = diagnose(&analysis);
        let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(!parse_errors.is_empty());
        let range = parse_errors[0].range;
        assert!(range.end.line >= range.start.line);
        if range.end.line == range.start.line {
            assert!(range.end.character > range.start.character);
        }
    }

    #[test]
    fn test_parse_errors_converts_all() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELCT FRO users");
        let diags = diagnose(&analysis);
        let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(!parse_errors.is_empty());
    }

    #[test]
    fn test_valid_complex_sql_no_parse_errors() {
        let source = "CREATE TABLE t (id INT)\nINSERT INTO t (id) VALUES (1)\nSELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let diags = diagnose(&analysis);
        let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(parse_errors.is_empty());
    }

    #[test]
    fn test_diagnose_includes_parse_errors() {
        let sources = ["SELCT * FROM", ""];
        for source in &sources {
            let analysis = crate::analysis::DocumentAnalysis::new(source);
            let diags = diagnose(&analysis);
            // diagnose() should include parse errors
            let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
            if !source.is_empty() {
                assert!(
                    !parse_errors.is_empty() || !diags.is_empty(),
                    "diagnose() should produce diagnostics for invalid source: {source:?}"
                );
            }
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

    #[test]
    fn test_select_star_in_insert_select_warns() {
        let source = "INSERT INTO t2 SELECT * FROM t1";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT * in INSERT...SELECT should produce warning"
        );
    }

    #[test]
    fn test_select_star_id_comma_star_warns() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT id, * FROM users");
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT id, * should warn about *"
        );
    }

    #[test]
    fn test_select_distinct_star_warns() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT DISTINCT * FROM users");
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT DISTINCT * should warn about *"
        );
    }

    #[test]
    fn test_select_top_star_warns() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT TOP 10 * FROM users");
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT TOP 10 * should warn about *"
        );
    }

    #[test]
    fn test_select_distinct_star_with_table_warns() {
        let source = "CREATE TABLE t (id INT)\nSELECT DISTINCT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT DISTINCT * should produce warning"
        );
    }

    #[test]
    fn test_select_star_in_create_view_warns() {
        let source = "CREATE VIEW v AS SELECT * FROM users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT * in CREATE VIEW should produce warning"
        );
    }

    #[test]
    fn test_diagnose_empty_source() {
        let analysis = crate::analysis::DocumentAnalysis::new("");
        let diags = diagnose(&analysis);
        assert!(
            diags.is_empty(),
            "Empty source should produce no diagnostics"
        );
    }

    #[test]
    fn test_diagnose_empty_source_only_parse_errors() {
        let analysis = crate::analysis::DocumentAnalysis::new("");
        let diags = diagnose(&analysis);
        assert!(diags.is_empty());
    }

    #[test]
    fn test_multiple_select_star_multiple_warnings() {
        let source = "SELECT * FROM users\nSELECT * FROM orders";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert_eq!(
            star_warnings.len(),
            2,
            "Two SELECT * statements should produce 2 warnings"
        );
    }

    #[test]
    fn test_parse_error_position_adjusted_to_zero_indexed() {
        // ParseError uses 1-indexed position; diagnostics should convert to 0-indexed
        let analysis = crate::analysis::DocumentAnalysis::new("SELCT * FROM");
        let diags = diagnose(&analysis);
        let parse_errors: Vec<_> = diags.iter().filter(|d| d.severity == Some(DiagnosticSeverity::ERROR)).collect();
        assert!(!parse_errors.is_empty());
        // Position should be 0-indexed (not 1-indexed)
        assert_eq!(parse_errors[0].range.start.line, 0);
    }

    #[test]
    fn test_select_star_inside_trigger_warns() {
        let source =
            "CREATE TRIGGER tr_test ON users FOR INSERT AS\nBEGIN\n    SELECT * FROM users\nEND";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let diags = diagnose(&analysis);
        let star_warnings: Vec<_> = diags
            .iter()
            .filter(|d| d.message.contains("SELECT *"))
            .collect();
        assert!(
            !star_warnings.is_empty(),
            "SELECT * inside CREATE TRIGGER should produce a warning"
        );
    }
}
