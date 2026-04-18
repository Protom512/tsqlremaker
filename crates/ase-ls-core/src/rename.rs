//! Rename provider
//!
//! シンボルの一括リネーム機能を提供する。
//! - 変数（@var）: DECLARE + 全参照箇所をリネーム
//! - テーブル名: CREATE TABLE + 全参照箇所をリネーム
//! - プロシージャ/ビュー/インデックス: 定義 + 参照をリネーム

use crate::{find_token_at, offset_to_position, position_to_offset, token_matches_symbol};
use lsp_types::{Position, Range, TextEdit, Url, WorkspaceEdit};
use std::collections::HashMap;
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// カーソル位置のシンボルを新しい名前にリネームする
///
/// ソース内の全参照箇所を特定し、WorkspaceEditを返す。
pub fn rename(
    source: &str,
    position: Position,
    new_name: &str,
    uri: &Url,
) -> Option<WorkspaceEdit> {
    let offset = position_to_offset(source, position);

    let (target_kind, target_text) = find_token_at(source, offset)?;

    let search_upper = target_text.to_uppercase();
    let is_var = target_kind == TokenKind::LocalVar;

    // 新しい名前のバリデーション
    if is_var && !new_name.starts_with('@') {
        return None;
    }
    if new_name.is_empty() {
        return None;
    }

    let mut edits = Vec::new();

    for token_result in Lexer::new(source) {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };

        if token_matches_symbol(token.kind, token.text, &search_upper, is_var) {
            let (start_line, start_char) = offset_to_position(source, token.span.start);
            let (end_line, end_char) = offset_to_position(source, token.span.end);
            edits.push(TextEdit {
                range: Range {
                    start: Position {
                        line: start_line,
                        character: start_char,
                    },
                    end: Position {
                        line: end_line,
                        character: end_char,
                    },
                },
                new_text: new_name.to_string(),
            });
        }
    }

    if edits.is_empty() {
        return None;
    }

    // 重複除去（同じ位置の複数TextEditを防止）
    edits.dedup_by(|a, b| a.range.start == b.range.start && a.range.end == b.range.end);

    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

/// リネーム対象のプレースホルダー名を取得する
///
/// カーソル位置のシンボルの現在の名前を返す。
pub fn get_rename_placeholder(source: &str, position: Position) -> Option<String> {
    let offset = position_to_offset(source, position);
    let (_, text) = find_token_at(source, offset)?;
    Some(text)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn test_uri() -> Url {
        Url::parse("file:///test.sql").unwrap()
    }

    #[test]
    fn test_rename_variable() {
        let source = "DECLARE @count INT\nSET @count = 1\nSELECT @count";
        let result = rename(
            source,
            Position {
                line: 1,
                character: 5,
            },
            "@total",
            &test_uri(),
        );
        assert!(result.is_some());
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let text_edits = changes.get(&test_uri()).unwrap();
        assert_eq!(text_edits.len(), 3);
        assert!(text_edits.iter().all(|e| e.new_text == "@total"));
    }

    #[test]
    fn test_rename_table() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM users\nDELETE FROM users";
        let result = rename(
            source,
            Position {
                line: 0,
                character: 14,
            },
            "customers",
            &test_uri(),
        );
        assert!(result.is_some());
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let text_edits = changes.get(&test_uri()).unwrap();
        assert_eq!(text_edits.len(), 3);
        assert!(text_edits.iter().all(|e| e.new_text == "customers"));
    }

    #[test]
    fn test_rename_case_insensitive() {
        let source =
            "CREATE TABLE Users (id INT)\nSELECT * FROM users\nINSERT INTO USERS (id) VALUES (1)";
        let result = rename(
            source,
            Position {
                line: 0,
                character: 14,
            },
            "customers",
            &test_uri(),
        );
        assert!(result.is_some());
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let text_edits = changes.get(&test_uri()).unwrap();
        assert!(text_edits.len() >= 3);
    }

    #[test]
    fn test_rename_variable_without_at_prefix_fails() {
        let source = "DECLARE @count INT\nSET @count = 1";
        let result = rename(
            source,
            Position {
                line: 1,
                character: 5,
            },
            "total",
            &test_uri(),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_rename_empty_name_fails() {
        let source = "CREATE TABLE users (id INT)";
        let result = rename(
            source,
            Position {
                line: 0,
                character: 14,
            },
            "",
            &test_uri(),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_rename_on_whitespace_returns_none() {
        let source = "SELECT  FROM t";
        let result = rename(
            source,
            Position {
                line: 0,
                character: 7,
            },
            "new_name",
            &test_uri(),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_get_rename_placeholder() {
        let source = "SELECT * FROM users";
        let placeholder = get_rename_placeholder(
            source,
            Position {
                line: 0,
                character: 15,
            },
        );
        assert_eq!(placeholder, Some("users".to_string()));
    }

    #[test]
    fn test_rename_procedure() {
        let source = "CREATE PROCEDURE my_proc AS BEGIN RETURN 1 END\nEXEC my_proc";
        let result = rename(
            source,
            Position {
                line: 0,
                character: 18,
            },
            "new_proc",
            &test_uri(),
        );
        assert!(result.is_some());
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let text_edits = changes.get(&test_uri()).unwrap();
        assert!(text_edits.len() >= 2);
    }
}
