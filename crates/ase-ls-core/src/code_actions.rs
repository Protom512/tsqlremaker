//! Code Actions (Quick Fix) provider
//!
//! DDL開発に特化したクイックフィックスを提供する。
//! - SELECT * FROM table → カラム展開
//! - INSERT INTO table → VALUES骨組み生成
//! - BEGIN/END → TRY...CATCH ラッパー

use crate::analysis::DocumentAnalysis;
use crate::symbol_table::SymbolTableBuilder;
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, Position, Range, TextEdit, WorkspaceEdit,
};
use std::collections::HashMap;
use tsql_parser::ast::{SelectItem, Statement, TableReference};
use tsql_parser::AstNode;
use tsql_token::TokenKind;

/// Code Actionsを生成する（DocumentAnalysis利用）
pub fn code_actions_with_analysis(
    analysis: &DocumentAnalysis,
    range: Range,
    uri: &lsp_types::Url,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    // AST-based detection: find the statement containing the cursor
    if let Some(action) = try_expand_select_star_ast(analysis, range.start, uri) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }

    if let Some(action) = try_generate_insert_skeleton_ast(analysis, range.start, uri) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    } else {
        // Fallback: incomplete INSERT (no VALUES) isn't parsed as Statement::Insert,
        // so fall back to line-level string matching for INSERT skeleton generation.
        let line_text = analysis.get_line(range.start.line).to_string();
        if let Some(action) =
            try_generate_insert_skeleton(&analysis.symbol_table, &line_text, range.start, uri)
        {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    // TRY...CATCH still uses line-level matching (BEGIN/END detection)
    let line_text = analysis.get_line(range.start.line).to_string();
    if let Some(action) = try_wrap_try_catch(&analysis.source, &line_text, range.start, uri) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }

    actions
}

/// Code Actionsを生成する（ソースから構築）
pub fn code_actions(source: &str, range: Range, uri: &lsp_types::Url) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    // カーソル位置の行を取得
    let line_text = get_line_at(source, range.start.line);
    if line_text.is_empty() {
        return actions;
    }

    // シンボルテーブルを構築（現在行より前の部分も試行）
    let symbol_table = build_fallback_symbol_table(source);

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

/// フォールバック付きシンボルテーブル構築
///
/// 完全なパースに失敗した場合、ソースを行ごとに分割して
/// 前方部分だけをパースし、DDL定義を抽出する。
fn build_fallback_symbol_table(source: &str) -> crate::symbol_table::SymbolTable {
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

/// Find the SELECT statement with Wildcard that the cursor position falls within.
/// Uses token spans instead of statement spans because the parser may produce
/// incorrect end offsets for multi-line statements.
fn find_select_star_for_position(
    analysis: &DocumentAnalysis,
    position: Position,
) -> Option<&Statement> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    let mut found: Option<(usize, &Statement)> = None;

    for (idx, stmt) in analysis.statements.iter().enumerate() {
        let Statement::Select(sel) = stmt else {
            continue;
        };
        if !sel
            .columns
            .iter()
            .any(|item| matches!(item, SelectItem::Wildcard))
        {
            continue;
        }
        let Some(from) = &sel.from else {
            continue;
        };
        let Some(_table_ref) = from.tables.first() else {
            continue;
        };

        let sel_start = sel.span.start as usize;
        if sel_start > offset {
            continue;
        }

        // Upper bound: next statement's start, or end of source
        let upper = analysis
            .statements
            .get(idx + 1)
            .map(|s| {
                let start = s.span().start as usize;
                if start == 0 {
                    analysis.source.len()
                } else {
                    start
                }
            })
            .unwrap_or(analysis.source.len());

        if offset <= upper {
            found = Some((idx, stmt));
            break;
        }
    }

    // Verify the found SELECT has a valid table reference
    if let Some((_, stmt)) = found {
        if let Statement::Select(sel) = stmt {
            if let Some(from) = &sel.from {
                if let Some(table_ref) = from.tables.first() {
                    if matches!(table_ref, TableReference::Table { .. }) {
                        return Some(stmt);
                    }
                }
            }
        }
    }
    None
}

/// Find the INSERT statement without complete VALUES that the cursor position falls within.
fn find_insert_for_position(analysis: &DocumentAnalysis, position: Position) -> Option<&Statement> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    for (idx, stmt) in analysis.statements.iter().enumerate() {
        let Statement::Insert(ins) = stmt else {
            continue;
        };

        let insert_start = ins.span.start as usize;
        if insert_start > offset {
            continue;
        }

        // Upper bound: next statement's start, or end of source
        let upper = analysis
            .statements
            .get(idx + 1)
            .map(|s| {
                let start = s.span().start as usize;
                if start == 0 {
                    analysis.source.len()
                } else {
                    start
                }
            })
            .unwrap_or(analysis.source.len());

        if offset <= upper {
            return Some(stmt);
        }
    }
    None
}

/// AST-based SELECT * expansion using Statement spans.
fn try_expand_select_star_ast(
    analysis: &DocumentAnalysis,
    position: Position,
    uri: &lsp_types::Url,
) -> Option<CodeAction> {
    let stmt = find_select_star_for_position(analysis, position)?;

    let Statement::Select(sel) = stmt else {
        return None;
    };

    let from = sel.from.as_ref()?;
    let table_ref = from.tables.first()?;
    let table_name = match table_ref {
        TableReference::Table { name, .. } => name.name.clone(),
        _ => return None,
    };

    let tbl = SymbolTableBuilder::find_table(&analysis.symbol_table, &table_name)?;
    if tbl.columns.is_empty() {
        return None;
    }

    // Find the * token: use SELECT span start as lower bound
    let sel_start = sel.span.start as usize;
    let star_token = analysis
        .tokens
        .iter()
        .find(|t| t.kind == TokenKind::Star && t.span.start as usize >= sel_start)?;

    let columns: Vec<String> = tbl.columns.iter().map(|c| c.name.clone()).collect();
    let expanded = format!("SELECT {}", columns.join(", "));

    let (star_line, star_char_start) = analysis
        .line_index
        .offset_to_position(star_token.span.start);
    let (_, star_char_end) = analysis.line_index.offset_to_position(star_token.span.end);

    let edit = make_text_edit(
        uri,
        Range {
            start: Position {
                line: star_line,
                character: star_char_start,
            },
            end: Position {
                line: star_line,
                character: star_char_end,
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

/// AST-based INSERT skeleton generation using Statement spans.
fn try_generate_insert_skeleton_ast(
    analysis: &DocumentAnalysis,
    position: Position,
    uri: &lsp_types::Url,
) -> Option<CodeAction> {
    let stmt = find_insert_for_position(analysis, position)?;

    let Statement::Insert(ins) = stmt else {
        return None;
    };

    // Skip if already has VALUES or SELECT source
    match &ins.source {
        tsql_parser::InsertSource::Values(v) if !v.is_empty() => return None,
        tsql_parser::InsertSource::Select(_) => return None,
        _ => {}
    }

    let table_name = ins.table.name.clone();
    let tbl = SymbolTableBuilder::find_table(&analysis.symbol_table, &table_name)?;
    if tbl.columns.is_empty() {
        return None;
    }

    let columns: Vec<&str> = tbl
        .columns
        .iter()
        .filter(|c| !c.is_identity)
        .map(|c| c.name.as_str())
        .collect();
    let col_list = columns.join(", ");
    let placeholders = vec!["?"; columns.len()];
    let values_list = placeholders.join(", ");

    let new_text = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table_name, col_list, values_list
    );

    // Replace the entire INSERT statement span
    let (start_line, start_char) = analysis.line_index.offset_to_position(ins.span.start);
    let (end_line, end_char) = analysis.line_index.offset_to_position(ins.span.end);

    let edit = make_text_edit(
        uri,
        Range {
            start: Position {
                line: start_line,
                character: start_char,
            },
            end: Position {
                line: end_line,
                character: end_char,
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
    let placeholders = vec!["?"; columns.len()];
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
    #[allow(clippy::mutable_key_type)]
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

    // === AST-aware tests (RED phase for #68) ===

    fn make_analysis(source: &str) -> DocumentAnalysis {
        DocumentAnalysis::new(source)
    }

    fn find_expand_action(actions: &[CodeActionOrCommand]) -> bool {
        actions.iter().any(
            |a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Expand")),
        )
    }

    #[allow(dead_code)]
    fn find_insert_skeleton_action(actions: &[CodeActionOrCommand]) -> bool {
        actions.iter().any(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("INSERT skeleton"))
        })
    }

    #[test]
    fn test_ast_select_star_multiline_cursor_on_from_line() {
        // SELECT * on line 0, FROM on line 1 — cursor on FROM line
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT *\nFROM users";
        let analysis = make_analysis(source);
        let range = Range {
            start: Position {
                line: 2,
                character: 0,
            },
            end: Position {
                line: 2,
                character: 10,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        assert!(
            find_expand_action(&actions),
            "should detect SELECT * across lines via AST"
        );
    }

    #[test]
    fn test_ast_select_star_multiline_cursor_on_select_line() {
        // SELECT * on line 0, FROM on line 1 — cursor on SELECT line
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT *\nFROM users";
        let analysis = make_analysis(source);
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 9,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        assert!(
            find_expand_action(&actions),
            "should detect multi-line SELECT * with cursor on SELECT line"
        );
    }

    #[test]
    fn test_ast_select_star_three_lines() {
        // SELECT * / FROM / table split across 3 lines
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT\n*\nFROM users";
        let analysis = make_analysis(source);
        let range = Range {
            start: Position {
                line: 3,
                character: 0,
            },
            end: Position {
                line: 3,
                character: 10,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        assert!(
            find_expand_action(&actions),
            "should detect SELECT * across 3 lines via AST"
        );
    }

    #[test]
    fn test_ast_select_star_with_where() {
        // Multi-line SELECT * with WHERE clause
        let source =
            "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT *\nFROM users\nWHERE id = 1";
        let analysis = make_analysis(source);
        let range = Range {
            start: Position {
                line: 3,
                character: 0,
            },
            end: Position {
                line: 3,
                character: 10,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        assert!(
            find_expand_action(&actions),
            "should detect multi-line SELECT * with WHERE"
        );
    }

    #[test]
    fn test_ast_no_expand_for_non_wildcard_select() {
        // SELECT with explicit columns should NOT trigger expand
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT id\nFROM users";
        let analysis = make_analysis(source);
        let range = Range {
            start: Position {
                line: 2,
                character: 0,
            },
            end: Position {
                line: 2,
                character: 10,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        assert!(
            !find_expand_action(&actions),
            "should NOT expand non-wildcard SELECT"
        );
    }

    // === Mutation-resistant tests ===

    /// M1: Verify expanded SELECT * contains actual column names, not placeholder text
    #[test]
    fn test_ast_select_star_expand_columns_content() {
        let source = "CREATE TABLE orders (order_id INT, customer_name VARCHAR(50), total DECIMAL)";
        let source_full = format!("{source}\nSELECT *\nFROM orders");
        let analysis = make_analysis(&source_full);
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 9,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let action = actions
            .iter()
            .find(
                |a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Expand")),
            )
            .expect("should have expand action");
        if let CodeActionOrCommand::CodeAction(ca) = action {
            let edit = ca.edit.as_ref().expect("edit should exist");
            let changes = edit.changes.as_ref().expect("changes should exist");
            let text_edit = changes
                .get(&test_uri())
                .expect("should have changes for URI")
                .first()
                .expect("should have at least one edit");
            assert!(
                text_edit.new_text.contains("order_id"),
                "expanded text must contain order_id, got: {}",
                text_edit.new_text
            );
            assert!(
                text_edit.new_text.contains("customer_name"),
                "expanded text must contain customer_name, got: {}",
                text_edit.new_text
            );
            assert!(
                text_edit.new_text.contains("total"),
                "expanded text must contain total, got: {}",
                text_edit.new_text
            );
            assert!(
                !text_edit.new_text.contains('*'),
                "expanded text should not contain *"
            );
        }
    }

    /// M2: Verify the TextEdit range targets only the * token, not the entire line
    #[test]
    fn test_ast_select_star_edit_range_targets_star_only() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT *\nFROM users";
        let analysis = make_analysis(source);
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 9,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let action = actions
            .iter()
            .find(
                |a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Expand")),
            )
            .expect("should have expand action");
        if let CodeActionOrCommand::CodeAction(ca) = action {
            let edit = ca.edit.as_ref().expect("edit");
            let changes = edit.changes.as_ref().expect("changes");
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert_eq!(
                text_edit.range.start.line, 1,
                "edit should be on line 1 (SELECT line)"
            );
            assert_eq!(
                text_edit.range.start.character, 7,
                "edit should start at column 7 (after 'SELECT ')"
            );
            assert_eq!(
                text_edit.range.end.character, 8,
                "edit should end at column 8 (just the '*')"
            );
        }
    }

    /// M3: Test INSERT skeleton via code_actions_with_analysis (AST path)
    #[test]
    fn test_ast_insert_skeleton_via_analysis() {
        let source = "CREATE TABLE products (id INT, name VARCHAR(100), price DECIMAL)\nINSERT INTO products";
        let analysis = make_analysis(source);
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 19,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let found = find_insert_skeleton_action(&actions);
        assert!(found, "should generate INSERT skeleton via AST path");
        let action = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("INSERT skeleton"))
        }).expect("should have INSERT skeleton action");
        if let CodeActionOrCommand::CodeAction(ca) = action {
            let edit = ca.edit.as_ref().expect("edit");
            let changes = edit.changes.as_ref().expect("changes");
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert!(
                text_edit.new_text.contains("id"),
                "skeleton must contain id column"
            );
            assert!(
                text_edit.new_text.contains("name"),
                "skeleton must contain name column"
            );
            assert!(
                text_edit.new_text.contains("price"),
                "skeleton must contain price column"
            );
            assert!(
                text_edit.new_text.contains("VALUES"),
                "skeleton must contain VALUES"
            );
        }
    }

    /// M4: SELECT * with unknown table should NOT generate action
    #[test]
    fn test_ast_select_star_unknown_table_no_action() {
        let source = "CREATE TABLE users (id INT)\nSELECT *\nFROM nonexistent";
        let analysis = make_analysis(source);
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 9,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        assert!(
            !find_expand_action(&actions),
            "should NOT expand SELECT * for unknown table"
        );
    }

    /// M5: Cursor outside any statement should produce no actions
    #[test]
    fn test_ast_cursor_outside_statement_no_action() {
        let source = "CREATE TABLE users (id INT)\n\nSELECT * FROM users";
        let analysis = make_analysis(source);
        let range = Range {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 0,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        assert!(
            !find_expand_action(&actions),
            "should NOT expand when cursor is on empty line between statements"
        );
    }

    /// M6: INSERT with existing VALUES should NOT generate skeleton via analysis
    #[test]
    fn test_ast_insert_skip_when_values_exists() {
        let source = "CREATE TABLE users (id INT)\nINSERT INTO users VALUES (1)";
        let analysis = make_analysis(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        assert!(
            !find_insert_skeleton_action(&actions),
            "should NOT generate INSERT skeleton when VALUES already exists"
        );
    }
}
