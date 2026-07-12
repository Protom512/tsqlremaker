//! Rename provider
//!
//! シンボルの一括リネーム機能を提供する。
//! - 変数（@var）: DECLARE + 全参照箇所をリネーム
//! - テーブル名: CREATE TABLE + 全参照箇所をリネーム
//! - プロシージャ/ビュー/インデックス: 定義 + 参照をリネーム

use crate::analysis::DocumentAnalysis;
use crate::token_matches_symbol;
use lsp_types::{Position, PrepareRenameResponse, TextEdit, Url, WorkspaceEdit};
use std::collections::HashMap;
use tsql_token::TokenKind;

/// カーソル位置のシンボルをリネームする（DocumentAnalysis利用）
#[must_use]
pub fn rename_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
    new_name: &str,
    uri: &Url,
) -> Option<WorkspaceEdit> {
    let (target_kind, target_text) = match analysis.find_token_at_position(position) {
        Some((t, _)) => (t.kind, &*t.text),
        None => return None,
    };

    let is_var = target_kind == TokenKind::LocalVar;
    let is_temp_table = matches!(
        target_kind,
        TokenKind::TempTable | TokenKind::GlobalTempTable
    );

    if is_var && !new_name.starts_with('@') {
        return None;
    }
    // 一時テーブル識別子は '#' (ローカル) / '##' (グローバル) プレフィックスが必須。
    // 変数の '@' ガードと同様、プレフィックスを欠く new_name は却下する。
    // これにより "#temp" → "renamed" のようなプレフィックス喪失リネームを防ぐ。
    if is_temp_table && !new_name.starts_with('#') {
        return None;
    }
    if new_name.trim().is_empty() {
        return None;
    }

    let mut edits = Vec::new();

    for token in &analysis.tokens {
        if token_matches_symbol(token.kind, &token.text, target_text, is_var) {
            edits.push(TextEdit {
                range: analysis
                    .line_index
                    .offset_to_range(token.span.start, token.span.end),
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
#[must_use]
pub fn get_rename_placeholder_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
) -> Option<String> {
    let (token, _) = analysis.find_token_at_position(position)?;
    Some(token.text.to_string())
}

/// カーソル位置がリネーム可能か検証する（DocumentAnalysis利用）
///
/// リネーム可能なトークン種別のみ許可し、キーワード・文字列・空白は拒否する。
/// prepareRename LSPリクエストのハンドラとして使用する。
#[must_use]
pub fn prepare_rename_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
) -> Option<PrepareRenameResponse> {
    let (token, _) = analysis.find_token_at_position(position)?;

    let is_renamable = matches!(
        token.kind,
        TokenKind::Ident | TokenKind::LocalVar | TokenKind::GlobalVar
    );

    if !is_renamable {
        return None;
    }

    Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: analysis
            .line_index
            .offset_to_range(token.span.start, token.span.end),
        placeholder: token.text.to_string(),
    })
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

    #[test]
    fn test_rename_with_analysis_table_case_insensitive() {
        // CREATE TABLE with mixed case, reference in different case
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE Users (id INT)\nSELECT * FROM users\n",
        );
        let result = rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            }, // "Users"
            "customers",
            &test_uri(),
        );
        let ws_edit = result.expect("should produce WorkspaceEdit");
        let changes = ws_edit.changes.expect("should have changes");
        let edits = changes.values().next().unwrap();
        // Should rename both "Users" and "users"
        assert!(
            edits.len() >= 2,
            "Expected at least 2 edits for case-insensitive table rename, got {}",
            edits.len()
        );
    }

    #[test]
    fn test_rename_with_analysis_variable_multiple_references() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "DECLARE @count INT\nSET @count = 1\nSELECT @count\n",
        );
        let result = rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 9,
            }, // @count in DECLARE
            "@total",
            &test_uri(),
        );
        let ws_edit = result.expect("should produce WorkspaceEdit");
        let changes = ws_edit.changes.expect("should have changes");
        let edits = changes.values().next().unwrap();
        assert_eq!(edits.len(), 3, "Should rename all 3 occurrences of @count");
    }

    // --- #temp / ##global temporary table rename (Issue #140) ---
    //
    // token_matches_symbol (lib.rs) が非変数ブランチで TempTable /
    // GlobalTempTable を許容する前提。一時テーブルの lexer text は '#' /
    // '##' プレフィックスを含む (lexer.rs:294) ため、search_upper も同形状に
    // なり eq_ignore_ascii_case で定義↔使用がマッチする。

    /// UC-1: ローカル一時テーブル (#temp) のリネームが定義+使用の両方に
    /// TextEdit を生成することを検証する回帰テスト。
    ///
    /// new_name は '#' プレフィックスを保持する (#renamed)。変数リネームの
    /// @プレフィックス検証 (rename.rs:29) と同様に、一時テーブルでも
    /// プレフィックス保持の整合性が取れることを確認する。
    #[test]
    fn test_rename_temp_table_local_definition_and_usage() {
        // CREATE TABLE #temp (id INT)   ← 定義 (offset ~13 に #temp)
        // SELECT * FROM #temp           ← 使用  (offset ~31 に #temp)
        let source = "CREATE TABLE #temp (id INT)\nSELECT * FROM #temp";
        let analysis = crate::analysis::DocumentAnalysis::new(source);

        // カーソルを実行行の #temp 上に置く (SELECT * FROM #temp)
        // "SELECT * FROM " = 14 chars → offset 29+len("\n")=30 から '#temp' 開始
        let result = rename_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 14,
            },
            "#renamed",
            &test_uri(),
        );

        let ws_edit = result.expect("#temp rename should produce a WorkspaceEdit");
        let changes = ws_edit.changes.expect("should have changes");
        let edits = changes
            .get(&test_uri())
            .expect("should have edits for test uri");

        // 定義 (CREATE TABLE #temp) と 使用 (FROM #temp) の両方に TextEdit が
        // 生成されること。token_matches_symbol が TempTable を許容すれば自動的に
        // 両オカレンスがマッチする。
        assert!(
            edits.len() >= 2,
            "Expected at least 2 edits (definition + usage) for #temp rename, got {}",
            edits.len()
        );
        // 全 TextEdit が '#' プレフィックスを保持する new_name であること
        assert!(
            edits.iter().all(|e| e.new_text == "#renamed"),
            "All edits should carry the '#'-prefixed new_name"
        );
    }

    /// UC-2: グローバル一時テーブル (##global) のリネームが定義+使用の両方に
    /// TextEdit を生成することを検証する。
    #[test]
    fn test_rename_temp_table_global_definition_and_usage() {
        let source = "CREATE TABLE ##global (id INT)\nSELECT * FROM ##global";
        let analysis = crate::analysis::DocumentAnalysis::new(source);

        let result = rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            }, // 定義側 ##global 上
            "##renamed",
            &test_uri(),
        );

        let ws_edit = result.expect("##global rename should produce a WorkspaceEdit");
        let changes = ws_edit.changes.expect("should have changes");
        let edits = changes
            .get(&test_uri())
            .expect("should have edits for test uri");

        assert!(
            edits.len() >= 2,
            "Expected at least 2 edits (definition + usage) for ##global rename, got {}",
            edits.len()
        );
        assert!(
            edits.iter().all(|e| e.new_text == "##renamed"),
            "All edits should carry the '##'-prefixed new_name"
        );
    }

    /// UC-3 (エッジケース): new_name が '#' プレフィックスを欠く場合、
    /// 一時テーブルのリネームは拒否されること。
    ///
    /// 変数の「@なしリネーム拒否」(test_rename_with_analysis_var_without_at_prefix)
    /// と対称な振る舞い。#temp は本来 '#' プレフィックスが意味を持つため、
    /// プレフィックス欠落は一貫性違反として却下する。これは現在の実装では
    /// rename.rs:29 の is_var ガードのみがプレフィックスを検査し、一時テーブル
    /// ブランチには同様のガードが存在しない——すなわち非スコープではなく、
    /// 現状は # なし new_name でも受け入れてしまう。本テストは仕様を文書化し、
    /// ガード追加または非スコープ明記のいずれかで解決すべき振る舞いを固定する。
    #[test]
    fn test_rename_temp_table_new_name_without_hash_prefix() {
        let source = "CREATE TABLE #temp (id INT)\nSELECT * FROM #temp";
        let analysis = crate::analysis::DocumentAnalysis::new(source);

        let result = rename_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 14,
            },
            "renamed", // ← '#' プレフィックス欠落
            &test_uri(),
        );

        // 一時テーブル識別子は '#' プレフィックスが必須。変数(@)と同様に、
        // プレフィックスを欠く new_name は却下されるべき。
        assert!(
            result.is_none(),
            "Rename of #temp with a '#'-less new_name should be rejected \
             to preserve temporary-table prefix integrity"
        );
    }

    /// カーソルが定義側 (#temp in CREATE TABLE) にあっても、使用側を含めて
    /// 両方に TextEdit が生成されること（定義起点でも参照が追跡されること）。
    #[test]
    fn test_rename_temp_table_cursor_on_definition_covers_usage() {
        let source = "CREATE TABLE #temp (id INT)\nSELECT * FROM #temp";
        let analysis = crate::analysis::DocumentAnalysis::new(source);

        let result = rename_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            }, // 定義側 #temp 上
            "#renamed",
            &test_uri(),
        );

        let ws_edit = result.expect("#temp rename from definition should succeed");
        let changes = ws_edit.changes.expect("should have changes");
        let edits = changes
            .get(&test_uri())
            .expect("should have edits for test uri");
        assert!(
            edits.len() >= 2,
            "Rename from definition should still cover usage, got {} edits",
            edits.len()
        );
    }
}
