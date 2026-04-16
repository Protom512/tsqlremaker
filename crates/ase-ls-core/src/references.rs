//! Find References provider
//!
//! カーソル位置のシンボルの全参照箇所を検索する。
//! - 変数: DECLARE + 全使用箇所
//! - テーブル: CREATE TABLE + SELECT/INSERT/UPDATE/DELETE内の参照
//! - プロシージャ: CREATE PROCEDURE + EXEC呼び出し
//! - ビュー: CREATE VIEW + SELECT内の参照

use crate::{offset_to_position, position_to_offset};
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

        let matches = if is_var {
            token.kind == TokenKind::LocalVar && token.text.to_uppercase() == search_name
        } else {
            (token.kind == TokenKind::Ident
                || matches!(
                    token.kind,
                    TokenKind::Select
                        | TokenKind::From
                        | TokenKind::Insert
                        | TokenKind::Update
                        | TokenKind::Delete
                        | TokenKind::Create
                        | TokenKind::Exec
                        | TokenKind::Procedure
                        | TokenKind::Table
                        | TokenKind::View
                        | TokenKind::Index
                )
                || token.kind.is_keyword())
                && token.text.to_uppercase() == search_name
        };

        if matches {
            let (start_line, start_char) = offset_to_position(source, token.span.start);
            let (end_line, end_char) = offset_to_position(source, token.span.end);
            let range = Range {
                start: Position {
                    line: start_line,
                    character: start_char,
                },
                end: Position {
                    line: end_line,
                    character: end_char,
                },
            };

            // 定義箇所の判定: DECLARE内の@var, CREATE直後の識別子
            let is_declaration = is_definition_token(source, &token, is_var);

            if include_declaration || !is_declaration {
                refs.push(range);
            }
        }
    }

    // 重複除去（同一Rangeの排除）
    refs.dedup_by(|a, b| a.start == b.start && a.end == b.end);

    refs
}

/// トークンが定義箇所かどうかを判定する
fn is_definition_token(source: &str, _token: &tsql_lexer::Token<'_>, is_var: bool) -> bool {
    // 簡易ヒューリスティック: 変数の場合は最初の@出現がDECLARE内とみなす
    // より正確にはAST解析が必要だが、トークンベースの簡易判定
    if is_var {
        // LocalVarで、ソース内のテキスト位置がDECLAREキーワードの後にあるか
        // ヒューリスティック: DECLARE 直後の変数は定義
        let before = &source[.._token.span.start as usize];
        let trimmed = before.trim_end();
        // DECLAREの直後にある場合
        if trimmed.to_uppercase().ends_with("DECLARE")
            || trimmed.to_uppercase().ends_with("DECLARE\n")
            || trimmed.ends_with(',')
        {
            return true;
        }
    }
    false
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
    fn test_references_variable_with_declaration() {
        let source = "DECLARE @count INT\nSET @count = 1\nSELECT @count";
        // Cursor on @count in line 1
        let ranges = reference_ranges(
            source,
            Position {
                line: 1,
                character: 5,
            },
            true,
        );
        // Should find: DECLARE @count, SET @count, SELECT @count
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
        // Should find: SET @count, SELECT @count (not DECLARE)
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn test_references_table_name() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM users\nDELETE FROM users";
        // Cursor on "users" in CREATE TABLE
        let ranges = reference_ranges(
            source,
            Position {
                line: 0,
                character: 14,
            },
            true,
        );
        // Should find: CREATE TABLE users, FROM users, DELETE FROM users
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
        // @other is different from @count
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
        // Case-insensitive match: Users == users
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
        let source =
            "CREATE PROCEDURE my_proc AS BEGIN RETURN 1 END\nEXEC my_proc\nEXECUTE my_proc";
        let ranges = reference_ranges(
            source,
            Position {
                line: 0,
                character: 18,
            },
            true,
        );
        // Should find: CREATE PROCEDURE my_proc, EXEC my_proc, EXECUTE my_proc
        assert!(ranges.len() >= 2);
    }
}
