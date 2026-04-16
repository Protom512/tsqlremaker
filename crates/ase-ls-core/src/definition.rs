//! Go to Definition provider
//!
//! カーソル位置のシンボルから定義箇所へナビゲーションを提供する。
//! シンボルテーブルを使用して定義箇所を検索する。
//! - 変数参照 → DECLARE文
//! - テーブル参照 → CREATE TABLE定義
//! - プロシージャ参照 → CREATE PROCEDURE定義
//! - ビュー参照 → CREATE VIEW定義
//! - インデックス参照 → CREATE INDEX定義

use crate::symbol_table::{SymbolTable, SymbolTableBuilder};
use crate::position_to_offset;

#[cfg(test)]
use crate::offset_to_position;
use lsp_types::{Position, Range};
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// カーソル位置のシンボルの定義箇所を検索する
///
/// 戻り値は定義箇所のRangeのリスト。空の場合は定義なし。
pub fn definition_ranges(source: &str, position: Position) -> Vec<Range> {
    let offset = position_to_offset(source, position);

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

/// オブジェクト定義（テーブル、プロシージャ、ビュー、インデックス）を検索する
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

    results
}

/// カーソル位置のトークンを特定する
fn find_token_at(source: &str, offset: usize) -> Option<(TokenKind, String)> {
    for token_result in Lexer::new(source) {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };
        let start = token.span.start as usize;
        let end = token.span.end as usize;
        if offset >= start && offset < end {
            return Some((token.kind, token.text.to_string()));
        }
        if start > offset {
            break;
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

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
        let (line, char) = offset_to_position(source, set_pos as u32);
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
        let (line, char) = offset_to_position(source, pos as u32);
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
        let (line, char) = offset_to_position(source, return_pos as u32);
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
}
