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
use crate::line_index::LineIndex;
use crate::symbol_table::{SymbolTable, SymbolTableBuilder};

use crate::find_token_at;
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

/// カーソル位置のシンボルの定義箇所を検索する（ソースから構築）
pub fn definition_ranges(source: &str, position: Position) -> Vec<Range> {
    let line_index = LineIndex::new(source);
    let offset = line_index.position_to_offset(source, position);

    let (target_kind, target_text) = match find_token_at(source, offset) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let symbol_table = SymbolTableBuilder::build_tolerant(source);
    let search_name = target_text.to_uppercase();

    if target_kind == TokenKind::LocalVar {
        find_variable_definition(&symbol_table, &search_name)
    } else {
        find_object_definition(&symbol_table, &search_name)
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
            if var.name.to_uppercase() == name {
                results.push(var.range);
            }
        }
        // パラメータも検索
        for param in &proc.parameters {
            if param.name.to_uppercase() == name {
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
    use crate::line_index::LineIndex as LI;

    #[test]
    fn test_goto_variable_definition() {
        let source = "DECLARE @count INT\nSET @count = 1\nSELECT @count";
        let ranges = definition_ranges(
            source,
            Position {
                line: 1,
                character: 5,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_goto_table_definition() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT * FROM users";
        let ranges = definition_ranges(
            source,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_goto_procedure_definition() {
        let source = "CREATE PROCEDURE my_proc @p1 INT AS BEGIN RETURN @p1 END";
        let ranges = definition_ranges(
            source,
            Position {
                line: 0,
                character: 18,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_goto_no_definition_found() {
        let source = "SELECT * FROM users";
        let ranges = definition_ranges(
            source,
            Position {
                line: 0,
                character: 15,
            },
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_goto_variable_in_procedure_body() {
        let source = "CREATE PROCEDURE test_proc AS BEGIN DECLARE @x INT SET @x = 1 END";
        let set_pos = source.find("SET @x").unwrap() + 5;
        let (line, char) = LI::new(source).offset_to_position(set_pos as u32);
        let ranges = definition_ranges(
            source,
            Position {
                line,
                character: char,
            },
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_goto_view_definition() {
        let source = "CREATE VIEW active_users AS SELECT * FROM users\nSELECT * FROM active_users";
        let ranges = definition_ranges(
            source,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_goto_whitespace_returns_empty() {
        let source = "SELECT  FROM t";
        let ranges = definition_ranges(
            source,
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_goto_index_definition() {
        let source = "CREATE INDEX idx_name ON users (id)\nSELECT * FROM users";
        // Cursor on "idx_name"
        let pos = source.find("idx_name").unwrap();
        let (line, char) = LI::new(source).offset_to_position(pos as u32);
        let ranges = definition_ranges(
            source,
            Position {
                line,
                character: char,
            },
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_goto_parameter_in_procedure() {
        let source = "CREATE PROCEDURE test_proc @p1 INT AS BEGIN RETURN @p1 END";
        // Cursor on @p1 in RETURN statement
        let return_pos = source.find("RETURN @p1").unwrap() + 7;
        let (line, char) = LI::new(source).offset_to_position(return_pos as u32);
        let ranges = definition_ranges(
            source,
            Position {
                line,
                character: char,
            },
        );
        assert_eq!(ranges.len(), 1);
        // Should point to parameter definition
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_goto_case_insensitive_table() {
        let source = "CREATE TABLE MyTable (id INT)\nSELECT * FROM mytable";
        let ranges = definition_ranges(
            source,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_definition_returns_correct_range_location() {
        let source = "DECLARE @x INT\nSET @x = 1";
        let ranges = definition_ranges(
            source,
            Position {
                line: 1,
                character: 5,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
        assert!(ranges[0].start.character < ranges[0].end.character);
    }

    #[test]
    fn test_definition_multiple_procedure_vars() {
        let source =
            "CREATE PROCEDURE p AS BEGIN DECLARE @a INT DECLARE @b INT SET @a = 1 SET @b = 2 END";
        let set_pos = source.find("SET @a").unwrap() + 5;
        let (line, char) = LI::new(source).offset_to_position(set_pos as u32);
        let ranges = definition_ranges(
            source,
            Position {
                line,
                character: char,
            },
        );
        assert_eq!(ranges.len(), 1);
        let def_pos = source.find("DECLARE @a").unwrap();
        let (def_line, _def_char) = LI::new(source).offset_to_position(def_pos as u32);
        assert_eq!(ranges[0].start.line, def_line);
    }

    // --- definition_ranges_with_analysis tests ---

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
}
