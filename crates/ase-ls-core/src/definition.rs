//! Go to Definition provider
//!
//! カーソル位置のシンボルから定義箇所へナビゲーションを提供する。
//! シンボルテーブルを使用して定義箇所を検索する。
//! - 変数参照 → DECLARE文
//! - テーブル参照 → CREATE TABLE定義
//! - プロシージャ参照 → CREATE PROCEDURE定義
//! - ビュー参照 → CREATE VIEW定義
//! - インデックス参照 → CREATE INDEX定義

use crate::analysis::DocumentAnalysis;
use crate::symbol_table::SymbolTable;

use lsp_types::{Position, Range};
use tsql_token::TokenKind;

/// カーソル位置のシンボルの定義箇所を検索する（DocumentAnalysis利用）
pub fn definition_ranges_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
) -> Vec<Range> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    let (target_kind, target_text) = match analysis.find_token_at(offset) {
        Some((t, _)) => (t.kind, t.text.clone()),
        None => return Vec::new(),
    };

    let search_name = target_text.to_uppercase();

    if target_kind == TokenKind::LocalVar {
        find_variable_definition(&analysis.symbol_table, &search_name)
    } else {
        find_object_definition(&analysis.symbol_table, &search_name)
    }
}

/// 変数定義を検索する
fn find_variable_definition(table: &SymbolTable, name: &str) -> Vec<Range> {
    // プロシージャ内変数を含めて検索
    let mut results = Vec::new();

    // トップレベル変数
    if let Some(var) = table.variables.get(name) {
        results.push(var.range);
    }

    // プロシージャボディ内の変数
    for proc in table.procedures.values() {
        for var in &proc.body_variables {
            if var.name.eq_ignore_ascii_case(name) {
                results.push(var.range);
            }
        }
        // パラメータも検索
        for param in &proc.parameters {
            if param.name.eq_ignore_ascii_case(name) {
                results.push(param.range);
            }
        }
    }

    results
}

/// オブジェクト定義（テーブル、プロシージャ、ビュー、インデックス、トリガー）を検索する
fn find_object_definition(table: &SymbolTable, name: &str) -> Vec<Range> {
    let mut results = Vec::new();

    if let Some(tbl) = table.tables.get(name) {
        results.push(tbl.range);
    }
    if let Some(proc) = table.procedures.get(name) {
        results.push(proc.range);
    }
    if let Some(view) = table.views.get(name) {
        results.push(view.range);
    }
    if let Some(idx) = table.indexes.get(name) {
        results.push(idx.range);
    }
    if let Some(trigger) = table.triggers.get(name) {
        results.push(trigger.range);
    }

    results
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_definition_with_analysis_variable() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "DECLARE @count INT\nSET @count = 1\nSELECT @count",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 5,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_definition_with_analysis_table() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE users (id INT)\nSELECT * FROM users",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_definition_with_analysis_empty_source() {
        let analysis = crate::analysis::DocumentAnalysis::new("");
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 0,
            },
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_definition_with_analysis_no_token_at_position() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT  FROM t");
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_definition_with_analysis_procedure() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE PROCEDURE my_proc AS BEGIN RETURN 1 END",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 18,
            },
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_definition_with_analysis_index() {
        let analysis =
            crate::analysis::DocumentAnalysis::new("CREATE INDEX idx_name ON users (id)");
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            },
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_definition_with_analysis_variable_in_while() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "DECLARE @count INT\nWHILE @count < 10 BEGIN\n  SET @count = @count + 1\nEND",
        );
        // Click on @count inside WHILE condition
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 7,
            },
        );
        assert_eq!(ranges.len(), 1);
    }
}
