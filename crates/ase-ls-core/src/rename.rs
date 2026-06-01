//! Rename provider
//!
//! シンボルの一括リネーム機能を提供する。
//! - 変数（@var）: DECLARE + 全参照箇所をリネーム
//! - テーブル名: CREATE TABLE + 全参照箇所をリネーム
//! - プロシージャ/ビュー/インデックス: 定義 + 参照をリネーム

use crate::analysis::DocumentAnalysis;
use crate::line_index::LineIndex;
use crate::{find_token_at, token_matches_symbol};
use lsp_types::{Position, PrepareRenameResponse, Range, TextEdit, Url, WorkspaceEdit};
use std::collections::HashMap;
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// カーソル位置のシンボルをリネームする（DocumentAnalysis利用）
pub fn rename_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
    new_name: &str,
    uri: &Url,
) -> Option<WorkspaceEdit> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    let (target_kind, target_text) = match analysis.find_token_at(offset) {
        Some((t, _)) => (t.kind, t.text.clone()),
        None => return None,
    };

    let search_upper = target_text.to_uppercase();
    let is_var = target_kind == TokenKind::LocalVar;

    if is_var && !new_name.starts_with('@') {
        return None;
    }
    if new_name.trim().is_empty() {
        return None;
    }

    let mut edits = Vec::new();

    for token in &analysis.tokens {
        if token_matches_symbol(token.kind, &token.text, &search_upper, is_var) {
            let (start_line, start_char) = analysis.line_index.offset_to_position(token.span.start);
            let (end_line, end_char) = analysis.line_index.offset_to_position(token.span.end);
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

    edits.dedup_by(|a, b| a.range.start == b.range.start && a.range.end == b.range.end);
    #[allow(clippy::mutable_key_type)]
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), edits);

    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

/// リネームプレースホルダーを取得する（DocumentAnalysis利用）
pub fn get_rename_placeholder_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
) -> Option<String> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);
    let (token, _) = analysis.find_token_at(offset)?;
    Some(token.text.clone())
}

/// カーソル位置がリネーム可能か検証する（DocumentAnalysis利用）
///
/// リネーム可能なトークン種別のみ許可し、キーワード・文字列・空白は拒否する。
/// prepareRename LSPリクエストのハンドラとして使用する。
pub fn prepare_rename_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
) -> Option<PrepareRenameResponse> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    let (token, _) = analysis.find_token_at(offset)?;

    let is_renamable = matches!(
        token.kind,
        TokenKind::Ident | TokenKind::LocalVar | TokenKind::GlobalVar
    );

    if !is_renamable {
        return None;
    }

    let (start_line, start_char) = analysis.line_index.offset_to_position(token.span.start);
    let (end_line, end_char) = analysis.line_index.offset_to_position(token.span.end);

    Some(PrepareRenameResponse::RangeWithPlaceholder {
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
        placeholder: token.text.clone(),
    })
}

/// カーソル位置のシンボルを新しい名前にリネームする（ソースから構築）
///
/// ソース内の全参照箇所を特定し、WorkspaceEditを返す。
pub fn rename(
    source: &str,
    position: Position,
    new_name: &str,
    uri: &Url,
) -> Option<WorkspaceEdit> {
    let line_index = LineIndex::new(source);
    let offset = line_index.position_to_offset(source, position);

    let (target_kind, target_text) = find_token_at(source, offset)?;

    let search_upper = target_text.to_uppercase();
    let is_var = target_kind == TokenKind::LocalVar;

    // 新しい名前のバリデーション
    if is_var && !new_name.starts_with('@') {
        return None;
    }
    if new_name.trim().is_empty() {
        return None;
    }

    let mut edits = Vec::new();

    for token_result in Lexer::new(source) {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };

        if token_matches_symbol(token.kind, token.text, &search_upper, is_var) {
            let (start_line, start_char) = line_index.offset_to_position(token.span.start);
            let (end_line, end_char) = line_index.offset_to_position(token.span.end);
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
    #[allow(clippy::mutable_key_type)]
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
    let line_index = LineIndex::new(source);
    let offset = line_index.position_to_offset(source, position);
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
    fn test_rename_whitespace_only_fails() {
        let source = "CREATE TABLE users (id INT)";
        let result = rename(
            source,
            Position {
                line: 0,
                character: 14,
            },
            "   ",
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

    #[test]
    fn test_rename_dedup_same_position() {
        // Ensure overlapping positions are deduped
        let source = "DECLARE @x INT\nSET @x = @x + 1\nSELECT @x";
        let result = rename(
            source,
            Position {
                line: 1,
                character: 5,
            },
            "@y",
            &test_uri(),
        );
        assert!(result.is_some());
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let text_edits = changes.get(&test_uri()).unwrap();
        // No duplicate ranges
        for i in 0..text_edits.len() {
            for j in (i + 1)..text_edits.len() {
                assert!(
                    text_edits[i].range.start != text_edits[j].range.start
                        || text_edits[i].range.end != text_edits[j].range.end
                );
            }
        }
    }

    #[test]
    fn test_rename_edits_all_match_new_name() {
        let source = "DECLARE @val INT\nSET @val = 1\nSELECT @val";
        let result = rename(
            source,
            Position {
                line: 1,
                character: 5,
            },
            "@new_val",
            &test_uri(),
        );
        assert!(result.is_some());
        let edit = result.unwrap();
        let changes = edit.changes.unwrap();
        let text_edits = changes.get(&test_uri()).unwrap();
        assert!(text_edits.iter().all(|e| e.new_text == "@new_val"));
        assert_eq!(text_edits.len(), 3);
    }

    #[test]
    fn test_rename_edit_ranges_are_valid() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM users";
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
        for te in text_edits {
            assert!(
                te.range.end.character > te.range.start.character
                    || te.range.end.line > te.range.start.line
            );
        }
    }

    // --- prepare_rename tests ---

    #[test]
    fn test_prepare_rename_on_identifier() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        let result = prepare_rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 15,
            },
        );
        assert!(result.is_some());
        match result {
            Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. }) => {
                assert_eq!(placeholder, "users");
            }
            _ => panic!("Expected RangeWithPlaceholder"),
        }
    }

    #[test]
    fn test_prepare_rename_on_variable() {
        let analysis = DocumentAnalysis::new("DECLARE @count INT\nSET @count = 1");
        let result = prepare_rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 9,
            },
        );
        assert!(result.is_some());
        match result {
            Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. }) => {
                assert_eq!(placeholder, "@count");
            }
            _ => panic!("Expected RangeWithPlaceholder"),
        }
    }

    #[test]
    fn test_prepare_rename_on_keyword_rejected() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        let result = prepare_rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 2,
            },
        );
        assert!(result.is_none(), "Keywords should not be renamable");
    }

    #[test]
    fn test_prepare_rename_on_whitespace_rejected() {
        let analysis = DocumentAnalysis::new("SELECT  FROM t");
        let result = prepare_rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(result.is_none(), "Whitespace should not be renamable");
    }

    #[test]
    fn test_prepare_rename_on_string_rejected() {
        let analysis = DocumentAnalysis::new("SELECT 'hello' FROM t");
        let result = prepare_rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 9,
            },
        );
        assert!(result.is_none(), "String literals should not be renamable");
    }

    #[test]
    fn test_prepare_rename_returns_valid_range() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        let result = prepare_rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 15,
            },
        );
        match result {
            Some(PrepareRenameResponse::RangeWithPlaceholder { range, .. }) => {
                assert_eq!(range.start.line, 0);
                assert!(range.start.character < range.end.character);
            }
            _ => panic!("Expected RangeWithPlaceholder"),
        }
    }

    // --- rename_with_analysis edge cases ---

    #[test]
    fn test_rename_with_analysis_variable() {
        let source = "DECLARE @count INT\nSET @count = 1\nSELECT @count";
        let analysis = DocumentAnalysis::new(source);
        let result = rename_with_analysis(
            &analysis,
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
    fn test_rename_with_analysis_table() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM users";
        let analysis = DocumentAnalysis::new(source);
        let result = rename_with_analysis(
            &analysis,
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
        assert!(text_edits.len() >= 2);
    }

    #[test]
    fn test_rename_with_analysis_empty_source() {
        let analysis = DocumentAnalysis::new("");
        let result = rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 0,
            },
            "new_name",
            &test_uri(),
        );
        assert!(result.is_none(), "Empty source should not allow rename");
    }

    #[test]
    fn test_rename_with_analysis_position_beyond_end() {
        let analysis = DocumentAnalysis::new("SELECT 1");
        let result = rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 999,
            },
            "new_name",
            &test_uri(),
        );
        assert!(
            result.is_none(),
            "Position beyond source end should not allow rename"
        );
    }

    #[test]
    fn test_rename_with_analysis_var_without_at_prefix() {
        let analysis = DocumentAnalysis::new("DECLARE @count INT\nSET @count = 1");
        let result = rename_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 5,
            },
            "total",
            &test_uri(),
        );
        assert!(
            result.is_none(),
            "Variable rename without @ prefix should be rejected"
        );
    }

    #[test]
    fn test_get_rename_placeholder_with_analysis() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        let placeholder = get_rename_placeholder_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 15,
            },
        );
        assert_eq!(placeholder, Some("users".to_string()));
    }

    #[test]
    fn test_get_rename_placeholder_with_analysis_no_token() {
        let analysis = DocumentAnalysis::new("SELECT  FROM t");
        let placeholder = get_rename_placeholder_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(placeholder.is_none());
    }
}
