//! Code Actions (Quick Fix) provider
//!
//! DDL開発に特化したクイックフィックスを提供する。
//! - SELECT * FROM table → カラム展開
//! - INSERT INTO table → VALUES骨組み生成
//! - BEGIN/END → TRY...CATCH ラッパー

use crate::symbol_table::SymbolTableBuilder;
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Position, Range, TextEdit, WorkspaceEdit,
};
use std::collections::HashMap;

/// カーソル位置のコンテキストに基づいてCode Actionsを生成する
pub fn code_actions(source: &str, range: Range, uri: &lsp_types::Url) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    // カーソル位置の行を取得
    let line_text = get_line_at(source, range.start.line);
    if line_text.is_empty() {
        return actions;
    }

    // シンボルテーブルを構築（現在行より前の部分も試行）
    let symbol_table = build_resilient_symbol_table(source);

    // SELECT * FROM table → カラム展開
    if let Some(action) = try_expand_select_star(&symbol_table, &line_text, range.start, uri) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }

    // INSERT INTO table → VALUES骨組み生成
    if let Some(action) = try_generate_insert_skeleton(&symbol_table, &line_text, range.start, uri)
    {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }

    // BEGIN → TRY...CATCH ラッパー
    if let Some(action) = try_wrap_try_catch(source, &line_text, range.start, uri) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }

    actions
}

/// レジリエントなシンボルテーブル構築
///
/// 完全なパースに失敗した場合、ソースを行ごとに分割して
/// 前方部分だけをパースし、DDL定義を抽出する。
fn build_resilient_symbol_table(source: &str) -> crate::symbol_table::SymbolTable {
    let table = SymbolTableBuilder::build_tolerant(source);
    if !table.tables.is_empty() {
        return table;
    }

    // フォールバック: 前方から徐々に短くしてパースを試行
    let lines: Vec<&str> = source.lines().collect();
    for cut in (1..lines.len()).rev() {
        let partial: String = lines[..cut].join("\n");
        let partial_table = SymbolTableBuilder::build_tolerant(&partial);
        if !partial_table.tables.is_empty() {
            return partial_table;
        }
    }

    table
}

/// SELECT * FROM table → カラム展開クイックフィックス
fn try_expand_select_star(
    symbol_table: &crate::symbol_table::SymbolTable,
    line_text: &str,
    position: Position,
    uri: &lsp_types::Url,
) -> Option<CodeAction> {
    let upper = line_text.to_uppercase();

    // SELECT * FROM パターンを検索
    let star_pos = upper.find("SELECT *")?;
    let from_pos = upper.find("FROM")?;
    if from_pos < star_pos + 8 {
        return None;
    }

    // テーブル名を抽出
    let after_from = line_text[from_pos + 4..].trim();
    let table_name = after_from
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(';')
        .trim_end_matches(',');

    if table_name.is_empty() {
        return None;
    }

    // シンボルテーブルからテーブルのカラムを検索
    let tbl = SymbolTableBuilder::find_table(symbol_table, table_name)?;

    if tbl.columns.is_empty() {
        return None;
    }

    // カラム展開テキストを生成
    let columns: Vec<String> = tbl.columns.iter().map(|c| c.name.clone()).collect();
    let expanded = format!("SELECT {}", columns.join(", "));

    // * の位置を特定
    let star_start = star_pos + "SELECT ".len();
    let star_end = star_start + 1;

    let edit = make_text_edit(
        uri,
        Range {
            start: Position {
                line: position.line,
                character: star_start as u32,
            },
            end: Position {
                line: position.line,
                character: star_end as u32,
            },
        },
        expanded,
    );

    Some(CodeAction {
        title: format!("Expand SELECT * with columns from {table_name}"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(edit),
        command: None,
        is_preferred: Some(true),
        disabled: None,
        data: None,
    })
}

/// INSERT INTO table → VALUES骨組み生成クイックフィックス
fn try_generate_insert_skeleton(
    symbol_table: &crate::symbol_table::SymbolTable,
    line_text: &str,
    position: Position,
    uri: &lsp_types::Url,
) -> Option<CodeAction> {
    let upper = line_text.to_uppercase();
    let insert_pos = upper.find("INSERT INTO")?;

    // テーブル名を抽出
    let after_insert = line_text[insert_pos + 11..].trim();
    let table_name = after_insert
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(';')
        .trim_end_matches(',');

    if table_name.is_empty() {
        return None;
    }

    // 既にカラムリストやVALUESがある場合はスキップ
    if upper.contains("VALUES") || upper.contains("SELECT") {
        return None;
    }

    // シンボルテーブルからテーブルのカラムを検索
    let tbl = SymbolTableBuilder::find_table(symbol_table, table_name)?;

    if tbl.columns.is_empty() {
        return None;
    }

    // INSERT骨組みを生成（IDENTITYカラムは除外）
    let columns: Vec<&str> = tbl
        .columns
        .iter()
        .filter(|c| !c.is_identity)
        .map(|c| c.name.as_str())
        .collect();
    let col_list = columns.join(", ");
    let placeholders: Vec<&str> = tbl
        .columns
        .iter()
        .filter(|c| !c.is_identity)
        .map(|_| "?")
        .collect();
    let values_list = placeholders.join(", ");

    let new_text = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table_name, col_list, values_list
    );

    let edit = make_text_edit(
        uri,
        Range {
            start: Position {
                line: position.line,
                character: 0,
            },
            end: Position {
                line: position.line,
                character: line_text.len() as u32,
            },
        },
        new_text,
    );

    Some(CodeAction {
        title: format!("Generate INSERT skeleton for {table_name}"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(edit),
        command: None,
        is_preferred: Some(true),
        disabled: None,
        data: None,
    })
}

/// BEGIN → TRY...CATCH ラッパー
///
/// カーソル位置のBEGINに対応するENDを見つけ、全体をTRY...CATCHでラップする。
/// 対応するENDが見つからない場合はNoneを返す。
fn try_wrap_try_catch(
    source: &str,
    line_text: &str,
    position: Position,
    uri: &lsp_types::Url,
) -> Option<CodeAction> {
    let trimmed = line_text.trim();
    if !trimmed.eq_ignore_ascii_case("BEGIN") {
        return None;
    }

    // 対応するENDを見つける（ネストしたBEGIN...ENDを正しく追跡）
    let end_line = find_matching_end(source, position.line)?;

    let indent = line_text.len() - line_text.trim_start().len();
    let indent_str = &line_text[..indent];

    // BEGIN...END全体をTRY...CATCHでラップするテキストを生成
    let begin_text = format!("{indent_str}BEGIN TRY\n{indent_str}    BEGIN");
    let end_text = format!("{indent_str}    END\n{indent_str}END TRY\n{indent_str}BEGIN CATCH\n{indent_str}    -- Handle error\n{indent_str}END CATCH");

    let new_text = format!("{begin_text}\n{end_text}");

    let edit = make_text_edit(
        uri,
        Range {
            start: Position {
                line: position.line,
                character: 0,
            },
            end: Position {
                line: end_line,
                character: source.lines().nth(end_line as usize)?.len() as u32,
            },
        },
        new_text,
    );

    Some(CodeAction {
        title: "Wrap with TRY...CATCH".to_string(),
        kind: Some(CodeActionKind::REFACTOR),
        diagnostics: None,
        edit: Some(edit),
        command: None,
        is_preferred: None,
        disabled: None,
        data: None,
    })
}

/// 指定行のBEGINに対応するEND行を見つける
fn find_matching_end(source: &str, begin_line: u32) -> Option<u32> {
    let lines: Vec<&str> = source.lines().collect();
    let mut depth = 1u32;
    let start = (begin_line + 1) as usize;

    for (line_idx, line) in lines.iter().enumerate().skip(start) {
        let trimmed = line.trim().to_uppercase();
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        for word in &words {
            if *word == "BEGIN" {
                depth += 1;
            } else if *word == "END" {
                depth -= 1;
                if depth == 0 {
                    return Some(line_idx as u32);
                }
            }
        }
    }

    None
}

/// 指定行のテキストを取得する
fn get_line_at(source: &str, line: u32) -> String {
    source.lines().nth(line as usize).unwrap_or("").to_string()
}

/// WorkspaceEdit を生成するヘルパー
fn make_text_edit(uri: &lsp_types::Url, range: Range, new_text: String) -> WorkspaceEdit {
    let mut changes = HashMap::new();
    changes.insert(uri.clone(), vec![TextEdit { range, new_text }]);
    WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use lsp_types::Url;

    fn test_uri() -> Url {
        Url::parse("file:///test.sql").unwrap()
    }

    #[test]
    fn test_expand_select_star() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT * FROM users";
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 20,
            },
        };
        let actions = code_actions(source, range, &test_uri());
        let expand_action = actions.iter().find(
            |a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Expand")),
        );
        assert!(expand_action.is_some());
        if let CodeActionOrCommand::CodeAction(ca) = expand_action.unwrap() {
            assert_eq!(ca.kind, Some(CodeActionKind::QUICKFIX));
        }
    }

    #[test]
    fn test_expand_select_star_columns() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100), email VARCHAR(200))\nSELECT * FROM users";
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 20,
            },
        };
        let actions = code_actions(source, range, &test_uri());
        let expand = actions.iter().find(
            |a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Expand")),
        );
        assert!(expand.is_some());
        if let CodeActionOrCommand::CodeAction(ca) = expand.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert!(text_edit.new_text.contains("id"));
            assert!(text_edit.new_text.contains("name"));
            assert!(text_edit.new_text.contains("email"));
        }
    }

    #[test]
    fn test_generate_insert_skeleton() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nINSERT INTO users";
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 16,
            },
        };
        let actions = code_actions(source, range, &test_uri());
        let insert_action = actions.iter().find(
            |a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("INSERT")),
        );
        assert!(insert_action.is_some());
        if let CodeActionOrCommand::CodeAction(ca) = insert_action.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert!(text_edit.new_text.contains("(id, name)"));
            assert!(text_edit.new_text.contains("VALUES (?, ?)"));
        }
    }

    #[test]
    fn test_wrap_try_catch() {
        let source = "CREATE PROCEDURE test_proc AS\nBEGIN\n    SELECT 1\nEND";
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 5,
            },
        };
        let actions = code_actions(source, range, &test_uri());
        let try_catch = actions
            .iter()
            .find(|a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("TRY")));
        assert!(try_catch.is_some());
        if let CodeActionOrCommand::CodeAction(ca) = try_catch.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert!(text_edit.new_text.contains("BEGIN TRY"));
            assert!(text_edit.new_text.contains("BEGIN CATCH"));
            // Range covers BEGIN (line 1) through END (line 3)
            assert_eq!(text_edit.range.start.line, 1);
            assert_eq!(text_edit.range.end.line, 3);
        }
    }

    #[test]
    fn test_wrap_try_catch_nested_begin_end() {
        let source = "BEGIN\n    BEGIN\n        SELECT 1\n    END\nEND";
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 5,
            },
        };
        let actions = code_actions(source, range, &test_uri());
        let try_catch = actions
            .iter()
            .find(|a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("TRY")));
        assert!(try_catch.is_some());
        if let CodeActionOrCommand::CodeAction(ca) = try_catch.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            // Should match outer BEGIN (line 0) with outer END (line 4)
            assert_eq!(text_edit.range.start.line, 0);
            assert_eq!(text_edit.range.end.line, 4);
        }
    }

    #[test]
    fn test_wrap_try_catch_no_matching_end() {
        let source = "BEGIN\n    SELECT 1";
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 5,
            },
        };
        let actions = code_actions(source, range, &test_uri());
        let try_catch = actions
            .iter()
            .find(|a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("TRY")));
        assert!(try_catch.is_none());
    }

    #[test]
    fn test_no_action_on_regular_line() {
        let source = "SELECT id, name FROM users";
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 10,
            },
        };
        let actions = code_actions(source, range, &test_uri());
        assert!(actions.is_empty());
    }

    #[test]
    fn test_insert_skip_when_values_exists() {
        let source = "CREATE TABLE users (id INT)\nINSERT INTO users VALUES (1)";
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 16,
            },
        };
        let actions = code_actions(source, range, &test_uri());
        let insert_action = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("INSERT skeleton"))
        });
        assert!(insert_action.is_none());
    }
}
