//! Find References provider
//!
//! カーソル位置のシンボルの全参照箇所を検索する。
//! - 変数: DECLARE + 全使用箇所
//! - テーブル: CREATE TABLE + SELECT/INSERT/UPDATE/DELETE内の参照
//! - プロシージャ: CREATE PROCEDURE + EXEC呼び出し
//! - ビュー: CREATE VIEW + SELECT内の参照

use crate::{find_token_at, offset_to_position, position_to_offset, token_matches_symbol};
use lsp_types::{Position, Range};
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// カーソル位置のシンボルの全参照箇所を検索する
///
/// `include_declaration` が true の場合は定義箇所も含める。
pub fn reference_ranges(source: &str, position: Position, include_declaration: bool) -> Vec<Range> {
    let offset = position_to_offset(source, position);

    let (target_kind, target_text) = match find_token_at(source, offset) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let search_name = target_text.to_uppercase();
    let is_var = target_kind == TokenKind::LocalVar;

    let mut refs = Vec::new();

    for token_result in Lexer::new(source) {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };

        if token_matches_symbol(token.kind, token.text, &search_name, is_var) {
            let range = token_span_to_range(source, &token);

            // 定義箇所の判定
            let is_declaration =
                !include_declaration && is_definition_token(source, &token, is_var);

            if include_declaration || !is_declaration {
                refs.push(range);
            }
        }
    }

    // 重複除去
    refs.dedup_by(|a, b| a.start == b.start && a.end == b.end);

    refs
}

/// トークンが定義箇所かどうかを判定する
fn is_definition_token(source: &str, token: &tsql_lexer::Token<'_>, is_var: bool) -> bool {
    let before = &source[..token.span.start as usize];
    let trimmed = before.trim_end();
    let upper = trimmed.to_uppercase();

    if is_var {
        // 変数定義: DECLARE @var
        if upper.ends_with("DECLARE") || upper.ends_with("DECLARE\n") || trimmed.ends_with(',') {
            return true;
        }
    } else {
        // テーブル/プロシージャ/ビュー/インデックス定義: CREATE [OBJECT] name
        if upper.ends_with("CREATE TABLE")
            || upper.ends_with("CREATE TABLE\n")
            || upper.ends_with("CREATE PROCEDURE")
            || upper.ends_with("CREATE PROCEDURE\n")
            || upper.ends_with("CREATE VIEW")
            || upper.ends_with("CREATE VIEW\n")
            || upper.ends_with("CREATE INDEX")
            || upper.ends_with("CREATE INDEX\n")
        {
            return true;
        }
    }
    false
}

/// トークンのSpanからLSP Rangeを生成
fn token_span_to_range(source: &str, token: &tsql_lexer::Token<'_>) -> Range {
    let (start_line, start_char) = offset_to_position(source, token.span.start);
    let (end_line, end_char) = offset_to_position(source, token.span.end);
    Range {
        start: Position {
            line: start_line,
            character: start_char,
        },
        end: Position {
            line: end_line,
            character: end_char,
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_references_variable_with_declaration() {
        let source = "DECLARE @count INT\nSET @count = 1\nSELECT @count";
        let ranges = reference_ranges(
            source,
            Position {
                line: 1,
                character: 5,
            },
            true,
        );
        assert_eq!(ranges.len(), 3);
    }

    #[test]
    fn test_references_variable_without_declaration() {
        let source = "DECLARE @count INT\nSET @count = 1\nSELECT @count";
        let ranges = reference_ranges(
            source,
            Position {
                line: 1,
                character: 5,
            },
            false,
        );
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn test_references_table_name() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM users\nDELETE FROM users";
        let ranges = reference_ranges(
            source,
            Position {
                line: 0,
                character: 14,
            },
            true,
        );
        assert!(ranges.len() >= 2);
    }

    #[test]
    fn test_references_no_match() {
        let source = "DECLARE @count INT\nSET @other = 1";
        let ranges = reference_ranges(
            source,
            Position {
                line: 1,
                character: 5,
            },
            true,
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_references_case_insensitive_table() {
        let source = "CREATE TABLE Users (id INT)\nSELECT * FROM users";
        let ranges = reference_ranges(
            source,
            Position {
                line: 0,
                character: 14,
            },
            true,
        );
        assert!(ranges.len() >= 2);
    }

    #[test]
    fn test_references_empty_on_whitespace() {
        let source = "SELECT  FROM t";
        let ranges = reference_ranges(
            source,
            Position {
                line: 0,
                character: 7,
            },
            true,
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_references_procedure() {
        let source = "CREATE PROCEDURE my_proc AS BEGIN RETURN 1 END\nEXEC my_proc";
        let ranges = reference_ranges(
            source,
            Position {
                line: 0,
                character: 18,
            },
            true,
        );
        assert!(ranges.len() >= 2);
    }

    #[test]
    fn test_references_table_in_insert() {
        let source = "CREATE TABLE orders (id INT)\nINSERT INTO orders (id) VALUES (1)\nSELECT * FROM orders";
        let ranges = reference_ranges(
            source,
            Position {
                line: 0,
                character: 14,
            },
            true,
        );
        // CREATE TABLE, INSERT INTO, SELECT FROM
        assert!(ranges.len() >= 3);
    }

    #[test]
    fn test_references_view() {
        let source = "CREATE VIEW active_users AS SELECT * FROM users\nSELECT * FROM active_users";
        let ranges = reference_ranges(
            source,
            Position {
                line: 1,
                character: 15,
            },
            true,
        );
        assert!(ranges.len() >= 2);
    }

    #[test]
    fn test_references_exclude_table_definition() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM users";
        let ranges = reference_ranges(
            source,
            Position {
                line: 0,
                character: 14,
            },
            false, // exclude declaration
        );
        // Should NOT include the CREATE TABLE line (it's a definition)
        // Should include the SELECT FROM line (it's a reference)
        assert!(!ranges.is_empty());
        // None of the ranges should be on line 0 (definition excluded)
        for range in &ranges {
            assert_ne!(range.start.line, 0, "Definition should be excluded");
        }
    }
}
