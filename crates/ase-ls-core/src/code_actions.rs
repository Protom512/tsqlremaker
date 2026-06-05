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

    // INSERT INTO table VALUES (...) → add column list
    if let Some(action) = try_add_insert_column_list_ast(analysis, range.start, uri) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    }

    // AST-aware TRY...CATCH: prefer AST path; fall back to string-based
    if let Some(action) = try_wrap_try_catch_ast(analysis, range.start, uri) {
        actions.push(CodeActionOrCommand::CodeAction(action));
    } else {
        let line_text = analysis.get_line(range.start.line).to_string();
        if let Some(action) = try_wrap_try_catch(&analysis.source, &line_text, range.start, uri) {
            actions.push(CodeActionOrCommand::CodeAction(action));
        }
    }

    actions
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

    let edit = make_text_edit(
        uri,
        analysis
            .line_index
            .offset_to_range(star_token.span.start, star_token.span.end),
        expanded,
    );

    Some(make_quickfix(
        format!("Expand SELECT * with columns from {table_name}"),
        edit,
    ))
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
        tsql_parser::InsertSource::Values(_) | tsql_parser::InsertSource::DefaultValues => {}
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
    let edit = make_text_edit(
        uri,
        analysis
            .line_index
            .offset_to_range(ins.span.start, ins.span.end),
        new_text,
    );

    Some(make_quickfix(
        format!("Generate INSERT skeleton for {table_name}"),
        edit,
    ))
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

    Some(make_quickfix(
        format!("Generate INSERT skeleton for {table_name}"),
        edit,
    ))
}

/// INSERT INTO table VALUES (...) → カラムリスト追加
///
/// ASTのInsertStatementでcolumnsが空、sourceがValuesの場合、
/// シンボルテーブルからカラムリストを生成して挿入する。
fn try_add_insert_column_list_ast(
    analysis: &DocumentAnalysis,
    position: Position,
    uri: &lsp_types::Url,
) -> Option<CodeAction> {
    let cursor_offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    for stmt in &analysis.statements {
        if let Some(action) = try_add_insert_columns_in_stmt(stmt, analysis, cursor_offset, uri) {
            return Some(action);
        }
    }
    None
}

fn try_add_insert_columns_in_stmt(
    stmt: &Statement,
    analysis: &DocumentAnalysis,
    cursor_offset: usize,
    uri: &lsp_types::Url,
) -> Option<CodeAction> {
    match stmt {
        Statement::Insert(insert) => {
            let span_end = resolve_insert_stmt_end(&insert.span, &analysis.tokens);
            let start = insert.span.start as usize;
            if cursor_offset < start || cursor_offset > span_end as usize {
                return None;
            }

            if !insert.columns.is_empty() {
                return None;
            }

            if !matches!(&insert.source, tsql_parser::ast::InsertSource::Values(_)) {
                return None;
            }

            let table_name = &insert.table.name;
            let tbl = SymbolTableBuilder::find_table(&analysis.symbol_table, table_name)?;
            if tbl.columns.is_empty() {
                return None;
            }

            let columns: Vec<&str> = tbl
                .columns
                .iter()
                .filter(|c| !c.is_identity)
                .map(|c| c.name.as_str())
                .collect();
            if columns.is_empty() {
                return None;
            }

            let col_list = columns.join(", ");
            let values_start =
                find_values_token_start(&analysis.tokens, insert.span.start, span_end)?;

            let v_start = analysis.line_index.offset_to_position(values_start);

            // VALUES直前に非破壊挿入: テーブル名〜VALUES間のコメントやフォーマットを保持
            let edit = make_text_edit(
                uri,
                Range {
                    start: Position {
                        line: v_start.0,
                        character: v_start.1,
                    },
                    end: Position {
                        line: v_start.0,
                        character: v_start.1,
                    },
                },
                format!("({col_list}) "),
            );

            Some(CodeAction {
                title: format!("Add column list to INSERT for {table_name}"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: None,
                edit: Some(edit),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: None,
            })
        }
        Statement::Block(block) => block
            .statements
            .iter()
            .find_map(|child| try_add_insert_columns_in_stmt(child, analysis, cursor_offset, uri)),
        Statement::If(if_stmt) => {
            try_add_insert_columns_in_stmt(&if_stmt.then_branch, analysis, cursor_offset, uri)
                .or_else(|| {
                    if_stmt.else_branch.as_ref().and_then(|else_b| {
                        try_add_insert_columns_in_stmt(else_b, analysis, cursor_offset, uri)
                    })
                })
        }
        Statement::While(while_stmt) => {
            try_add_insert_columns_in_stmt(&while_stmt.body, analysis, cursor_offset, uri)
        }
        Statement::Create(create) => {
            if let tsql_parser::ast::CreateStatement::Procedure(proc) = &**create {
                proc.body.iter().find_map(|child| {
                    try_add_insert_columns_in_stmt(child, analysis, cursor_offset, uri)
                })
            } else if let tsql_parser::ast::CreateStatement::Trigger(trigger) = &**create {
                trigger.body.iter().find_map(|child| {
                    try_add_insert_columns_in_stmt(child, analysis, cursor_offset, uri)
                })
            } else {
                None
            }
        }
        Statement::TryCatch(try_catch) => try_catch
            .try_block
            .statements
            .iter()
            .chain(try_catch.catch_block.statements.iter())
            .find_map(|child| try_add_insert_columns_in_stmt(child, analysis, cursor_offset, uri)),
        _ => None,
    }
}

/// INSERTスパンの終了位置を解決
fn resolve_insert_stmt_end(span: &tsql_token::Span, tokens: &[crate::analysis::OwnedToken]) -> u32 {
    if span.end > span.start {
        return span.end;
    }
    let start_idx = tokens.partition_point(|t| t.span.end <= span.start);
    for tok in &tokens[start_idx..] {
        if tok.kind == TokenKind::Semicolon {
            return tok.span.end;
        }
    }
    tokens
        .last()
        .map(|t| t.span.end)
        .unwrap_or(span.start + 100)
}

/// VALUESトークンの開始位置を見つける
///
/// `insert_start`〜`insert_end`の範囲内でVALUESトークンを検索する。
/// ステートメント境界を越えたスキャンを防止し、後続ステートメントの
/// VALUESトークンとの誤マッチを防ぐ。
fn find_values_token_start(
    tokens: &[crate::analysis::OwnedToken],
    insert_start: u32,
    insert_end: u32,
) -> Option<u32> {
    let start_idx = tokens.partition_point(|t| t.span.end <= insert_start);
    for tok in &tokens[start_idx..] {
        if tok.span.start > insert_end {
            break;
        }
        if tok.kind == TokenKind::Values {
            return Some(tok.span.start);
        }
    }
    None
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

    Some(make_refactor("Wrap with TRY...CATCH".to_string(), edit))
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

/// AST-aware TRY...CATCH wrapper using Statement::Block spans.
fn try_wrap_try_catch_ast(
    analysis: &DocumentAnalysis,
    position: Position,
    uri: &lsp_types::Url,
) -> Option<CodeAction> {
    // Only trigger when cursor is on the BEGIN line itself
    let line_text = analysis.get_line(position.line);
    if !line_text.trim().eq_ignore_ascii_case("BEGIN") {
        return None;
    }

    let cursor_offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    // Find the Statement::Block that starts at or near the cursor line
    let mut target_block: Option<&tsql_parser::ast::Block> = None;
    for stmt in &analysis.statements {
        if let Some(block) = find_block_at_offset(stmt, cursor_offset, analysis) {
            target_block = Some(block);
            break;
        }
    }

    let block = target_block?;
    let start_offset = block.span.start as usize;
    let end_offset = resolve_block_end(block, analysis)?;

    let (start_line, _start_col) = analysis.line_index.offset_to_position(start_offset as u32);
    let (end_line, _) = analysis.line_index.offset_to_position(end_offset as u32);

    // Get the line text to determine indentation
    let line_text = analysis.get_line(start_line);
    let indent = line_text.len() - line_text.trim_start().len();
    let indent_str = &line_text[..indent];

    // Extract the original block body text (between BEGIN and END lines)
    let original_body: String = if end_line > start_line {
        (start_line + 1..end_line)
            .map(|l| analysis.get_line(l))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        String::new()
    };

    let new_text = format!(
        "{indent_str}BEGIN TRY\n{original_body}\n{indent_str}END TRY\n{indent_str}BEGIN CATCH\n{indent_str}    -- Handle error\n{indent_str}END CATCH"
    );

    let end_line_text = analysis.get_line(end_line);
    let edit = make_text_edit(
        uri,
        Range {
            start: Position {
                line: start_line,
                character: 0,
            },
            end: Position {
                line: end_line,
                character: end_line_text.len() as u32,
            },
        },
        new_text,
    );

    Some(make_refactor("Wrap with TRY...CATCH".to_string(), edit))
}

/// Find the innermost Statement::Block containing the given offset.
fn find_block_at_offset<'a>(
    stmt: &'a Statement,
    offset: usize,
    analysis: &DocumentAnalysis,
) -> Option<&'a tsql_parser::ast::Block> {
    match stmt {
        Statement::Block(block) => {
            let start = block.span.start as usize;
            let end = resolve_block_end(block, analysis)?;
            if offset >= start && offset <= end {
                for child in &block.statements {
                    if let Some(inner) = find_block_at_offset(child, offset, analysis) {
                        return Some(inner);
                    }
                }
                Some(block)
            } else {
                None
            }
        }
        Statement::If(if_stmt) => {
            if let Some(b) = find_block_at_offset(&if_stmt.then_branch, offset, analysis) {
                return Some(b);
            }
            if let Some(else_branch) = &if_stmt.else_branch {
                if let Some(b) = find_block_at_offset(else_branch, offset, analysis) {
                    return Some(b);
                }
            }
            None
        }
        Statement::While(while_stmt) => find_block_at_offset(&while_stmt.body, offset, analysis),
        Statement::TryCatch(try_catch) => try_catch
            .try_block
            .statements
            .iter()
            .chain(try_catch.catch_block.statements.iter())
            .find_map(|child| find_block_at_offset(child, offset, analysis)),
        Statement::Create(create) => {
            let body: &[Statement] = match create.as_ref() {
                tsql_parser::ast::CreateStatement::Procedure(proc) => &proc.body,
                tsql_parser::ast::CreateStatement::Trigger(trigger) => &trigger.body,
                _ => return None,
            };
            body.iter()
                .find_map(|child| find_block_at_offset(child, offset, analysis))
        }
        _ => None,
    }
}

/// Resolve potentially broken span.end using depth-aware forward token scan.
///
/// Returns `Some(end_offset)` when the matching END token is found, `None` otherwise.
fn resolve_span_end_fallback(start_offset: usize, analysis: &DocumentAnalysis) -> Option<usize> {
    let mut depth = 0;
    let mut found_begin = false;
    for t in &analysis.tokens {
        let ts = t.span.start as usize;
        if ts < start_offset {
            continue;
        }
        let te = t.span.end as usize;
        if ts > start_offset + 5000 {
            break;
        }
        if !found_begin && ts <= start_offset && te > start_offset {
            found_begin = true;
            depth = 1;
            continue;
        }
        if found_begin {
            let text = t.text.to_uppercase();
            if text == "BEGIN" {
                depth += 1;
            } else if text == "END" {
                depth -= 1;
                if depth == 0 {
                    return Some(te);
                }
            }
        }
    }
    None
}

/// Resolve the end offset for a Block, using span.end when valid,
/// falling back to child-statement spans, and finally to depth-aware token scan.
fn resolve_block_end(
    block: &tsql_parser::ast::Block,
    analysis: &DocumentAnalysis,
) -> Option<usize> {
    let span = &block.span;
    if span.end > span.start {
        return Some(span.end as usize);
    }
    if let Some(last) = block.statements.last() {
        let s = last.span();
        if s.end > s.start {
            return Some(s.end as usize);
        }
    }
    resolve_span_end_fallback(span.start as usize, analysis)
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

/// Create a quick-fix CodeAction with standard fields.
fn make_quickfix(title: String, edit: WorkspaceEdit) -> CodeAction {
    CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: None,
        edit: Some(edit),
        command: None,
        is_preferred: Some(true),
        disabled: None,
        data: None,
    }
}

/// Create a refactor CodeAction with standard fields.
fn make_refactor(title: String, edit: WorkspaceEdit) -> CodeAction {
    CodeAction {
        title,
        kind: Some(CodeActionKind::REFACTOR),
        diagnostics: None,
        edit: Some(edit),
        command: None,
        is_preferred: None,
        disabled: None,
        data: None,
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

    // === AST-aware tests (RED phase for #68) ===

    fn make_analysis(source: &str) -> DocumentAnalysis {
        DocumentAnalysis::new(source)
    }

    fn find_expand_action(actions: &[CodeActionOrCommand]) -> bool {
        actions.iter().any(
            |a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Expand")),
        )
    }

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

    // === AST-aware TRY...CATCH tests ===

    fn find_try_catch_action(actions: &[CodeActionOrCommand]) -> Option<&CodeAction> {
        actions.iter().find_map(|a| match a {
            CodeActionOrCommand::CodeAction(ca) if ca.title.contains("TRY") => Some(ca),
            _ => None,
        })
    }

    #[test]
    fn test_ast_try_catch_wraps_block_at_cursor() {
        let source = "BEGIN\n    SELECT 1\nEND";
        let analysis = make_analysis(source);
        let actions = code_actions_with_analysis(
            &analysis,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 5,
                },
            },
            &test_uri(),
        );
        let action = find_try_catch_action(&actions);
        assert!(
            action.is_some(),
            "Should offer TRY...CATCH wrap for BEGIN block"
        );
        let ca = action.unwrap();
        let edit = ca
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .get(&test_uri())
            .unwrap()
            .first()
            .unwrap();
        assert!(edit.new_text.contains("BEGIN TRY"));
        assert!(edit.new_text.contains("BEGIN CATCH"));
        assert!(edit.new_text.contains("SELECT 1"));
        assert_eq!(edit.range.start.line, 0);
        assert_eq!(edit.range.end.line, 2);
    }

    #[test]
    fn test_ast_try_catch_nested_inner_block() {
        let source = "BEGIN\n    BEGIN\n        SELECT 1\n    END\nEND";
        let analysis = make_analysis(source);
        let actions = code_actions_with_analysis(
            &analysis,
            Range {
                start: Position {
                    line: 1,
                    character: 4,
                },
                end: Position {
                    line: 1,
                    character: 9,
                },
            },
            &test_uri(),
        );
        let action = find_try_catch_action(&actions);
        assert!(action.is_some(), "Should offer wrap for inner BEGIN");
        let ca = action.unwrap();
        let edit = ca
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .get(&test_uri())
            .unwrap()
            .first()
            .unwrap();
        assert_eq!(edit.range.start.line, 1);
        assert_eq!(edit.range.end.line, 3);
    }

    #[test]
    fn test_ast_try_catch_no_trigger_on_body_line() {
        // Cursor on a body line (SELECT 1) inside BEGIN...END → should NOT offer wrap.
        let source = "BEGIN\n    SELECT 1\nEND";
        let analysis = make_analysis(source);
        let actions = code_actions_with_analysis(
            &analysis,
            Range {
                start: Position {
                    line: 1,
                    character: 4,
                },
                end: Position {
                    line: 1,
                    character: 12,
                },
            },
            &test_uri(),
        );
        assert!(
            find_try_catch_action(&actions).is_none(),
            "TRY...CATCH wrap should NOT be offered when cursor is on a body line"
        );
    }

    #[test]
    fn test_ast_try_catch_no_trigger_on_end_line() {
        // Cursor on the END line → should NOT offer wrap.
        let source = "BEGIN\n    SELECT 1\nEND";
        let analysis = make_analysis(source);
        let actions = code_actions_with_analysis(
            &analysis,
            Range {
                start: Position {
                    line: 2,
                    character: 0,
                },
                end: Position {
                    line: 2,
                    character: 3,
                },
            },
            &test_uri(),
        );
        assert!(
            find_try_catch_action(&actions).is_none(),
            "TRY...CATCH wrap should NOT be offered when cursor is on END line"
        );
    }

    #[test]
    fn test_ast_try_catch_preserves_indentation() {
        let source = "CREATE PROCEDURE p AS\nBEGIN\n    SELECT 1\nEND";
        let analysis = make_analysis(source);
        let actions = code_actions_with_analysis(
            &analysis,
            Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 5,
                },
            },
            &test_uri(),
        );
        let action = find_try_catch_action(&actions);
        assert!(action.is_some());
        let ca = action.unwrap();
        let edit = ca
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .get(&test_uri())
            .unwrap()
            .first()
            .unwrap();
        assert!(edit.new_text.contains("BEGIN TRY"));
        assert!(edit.new_text.contains("SELECT 1"));
        assert!(edit.new_text.contains("END TRY"));
        assert!(edit.new_text.contains("BEGIN CATCH"));
    }

    #[test]
    fn test_ast_try_catch_outer_block_at_cursor() {
        let source = "BEGIN\n    BEGIN\n        SELECT 1\n    END\nEND";
        let analysis = make_analysis(source);
        let actions = code_actions_with_analysis(
            &analysis,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 5,
                },
            },
            &test_uri(),
        );
        let ca = find_try_catch_action(&actions).expect("should have TRY action");
        let edit = ca
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .get(&test_uri())
            .unwrap()
            .first()
            .unwrap();
        assert_eq!(edit.range.start.line, 0);
        assert_eq!(edit.range.end.line, 4);
    }

    #[test]
    fn test_ast_try_catch_action_kind_is_refactor() {
        let source = "BEGIN\n    SELECT 1\nEND";
        let analysis = make_analysis(source);
        let actions = code_actions_with_analysis(
            &analysis,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 5,
                },
            },
            &test_uri(),
        );
        let ca = find_try_catch_action(&actions).expect("should have TRY action");
        assert_eq!(
            ca.kind,
            Some(CodeActionKind::REFACTOR),
            "TRY...CATCH wrap must be REFACTOR, not QUICKFIX"
        );
    }

    #[test]
    fn test_ast_try_catch_inside_trigger_body() {
        let source = "CREATE TRIGGER tr_test ON users FOR INSERT AS\n\
                      BEGIN\n\
                          SELECT 1\n\
                      END";
        let analysis = make_analysis(source);
        let actions = code_actions_with_analysis(
            &analysis,
            Range {
                start: Position {
                    line: 1,
                    character: 0,
                },
                end: Position {
                    line: 1,
                    character: 5,
                },
            },
            &test_uri(),
        );
        let action = find_try_catch_action(&actions);
        assert!(
            action.is_some(),
            "TRY...CATCH wrap should be offered for BEGIN block inside CREATE TRIGGER"
        );
    }

    // === INSERT column list code action ===

    #[test]
    fn test_insert_add_column_list() {
        let source = "CREATE TABLE t (id INT, name VARCHAR(100))\nINSERT INTO t VALUES (1, 'test')";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(add_cols.is_some(), "Should offer to add column list");
        if let CodeActionOrCommand::CodeAction(ca) = add_cols.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert!(
                text_edit.new_text.contains("id, name"),
                "new_text: {}",
                text_edit.new_text
            );
            assert!(
                !text_edit.new_text.contains("VALUES"),
                "new_text should not duplicate VALUES keyword"
            );
        }
    }

    #[test]
    fn test_insert_skip_when_columns_exist() {
        let source =
            "CREATE TABLE t (id INT, name VARCHAR(100))\nINSERT INTO t (id, name) VALUES (1, 'test')";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_none(),
            "Should not offer when columns already listed"
        );
    }

    #[test]
    fn test_insert_column_list_skips_identity() {
        let source =
            "CREATE TABLE t (id INT IDENTITY, name VARCHAR(100))\nINSERT INTO t VALUES ('test')";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(add_cols.is_some());
        if let CodeActionOrCommand::CodeAction(ca) = add_cols.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert!(
                !text_edit.new_text.contains("id"),
                "IDENTITY should be excluded"
            );
            assert!(text_edit.new_text.contains("name"));
        }
    }

    #[test]
    fn test_insert_column_list_unknown_table_skip() {
        let source = "INSERT INTO unknown VALUES (1)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(add_cols.is_none());
    }

    #[test]
    fn test_insert_column_list_preserves_values() {
        let source = "CREATE TABLE t (a INT, b INT, c INT)\nINSERT INTO t VALUES (1, 2, 3)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(add_cols.is_some());
        if let CodeActionOrCommand::CodeAction(ca) = add_cols.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            let new = &text_edit.new_text;
            assert!(new.contains("a, b, c"), "Should contain column list");
            assert!(!new.contains("VALUES"), "Should not duplicate VALUES");
        }
    }

    /// レビュー指摘: 後続ステートメントのVALUESとの誤マッチ防止
    /// INSERT INTO t SELECT * FROM other  -- VALUESなし
    /// INSERT INTO t2 VALUES (...)        -- 後続のVALUES
    /// カーソルが1つ目のINSERTにある場合、2つ目のVALUESにマッチしてはならない
    #[test]
    fn test_insert_column_list_no_cross_statement_values_match() {
        let source = concat!(
            "CREATE TABLE t (id INT, name VARCHAR(100))\n",
            "CREATE TABLE t2 (id INT, name VARCHAR(100))\n",
            "INSERT INTO t SELECT * FROM t2\n",
            "INSERT INTO t2 VALUES (1, 'test')\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // カーソルを3行目(INSERT INTO t...)に置く
        let range = Range {
            start: Position {
                line: 2,
                character: 0,
            },
            end: Position {
                line: 2,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        // INSERT INTO t SELECT は VALUES ではないので、action は出ない
        // また、後続ステートメントのVALUESに誤マッチしてactionが出てもNG
        assert!(
            add_cols.is_none(),
            "Should not offer column list for INSERT...SELECT or cross-statement VALUES"
        );
    }

    /// レビュー指摘: 非破壊挿入 - テーブル名とVALUES間のコメントを保持する
    #[test]
    fn test_insert_column_list_preserves_comments_between_table_and_values() {
        let source = concat!(
            "CREATE TABLE t (id INT, name VARCHAR(100))\n",
            "INSERT INTO t -- important comment\n",
            "VALUES (1, 'test')\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(add_cols.is_some(), "Should offer column list");
        if let CodeActionOrCommand::CodeAction(ca) = add_cols.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            // 編集範囲がVALUESキーワードの直前であることを確認
            // (テーブル名直後〜VALUES間のコメントは置換対象ではない)
            assert!(
                text_edit.range.start.line >= 2,
                "Edit should be at or near VALUES line, got line {}",
                text_edit.range.start.line
            );
            assert!(
                text_edit.new_text.contains("id, name"),
                "new_text should contain column list"
            );
            // 非破壊: start == end (ゼロ幅挿入)
            assert_eq!(
                text_edit.range.start, text_edit.range.end,
                "Edit should be a zero-width insertion"
            );
        }
    }

    /// INSERT INTO t VALUES (...) の正常系でゼロ幅挿入を確認
    #[test]
    fn test_insert_column_list_is_zero_width_insertion() {
        let source = "CREATE TABLE t (a INT, b INT)\nINSERT INTO t VALUES (1, 2)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(add_cols.is_some());
        if let CodeActionOrCommand::CodeAction(ca) = add_cols.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            // ゼロ幅挿入: start == end
            assert_eq!(
                text_edit.range.start, text_edit.range.end,
                "Should be a non-destructive zero-width insertion at VALUES position"
            );
            assert!(
                text_edit.new_text.starts_with('('),
                "new_text should start with '('"
            );
        }
    }

    // === INSERT column list inside compound statements ===

    /// INSERT inside a WHILE loop should trigger column list action
    #[test]
    fn test_insert_column_list_inside_while() {
        let source = concat!(
            "CREATE TABLE t (id INT, name VARCHAR(100))\n",
            "WHILE 1 = 1\n",
            "BEGIN\n",
            "    INSERT INTO t VALUES (1, 'test')\n",
            "END\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 3,
                character: 4,
            },
            end: Position {
                line: 3,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_some(),
            "Should offer column list for INSERT inside WHILE"
        );
        if let CodeActionOrCommand::CodeAction(ca) = add_cols.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert!(
                text_edit.new_text.contains("id, name"),
                "new_text: {}",
                text_edit.new_text
            );
        }
    }

    /// INSERT inside an IF block should trigger column list action
    #[test]
    fn test_insert_column_list_inside_if() {
        let source = concat!(
            "CREATE TABLE t (a INT, b INT)\n",
            "IF 1 = 1\n",
            "BEGIN\n",
            "    INSERT INTO t VALUES (1, 2)\n",
            "END\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 3,
                character: 4,
            },
            end: Position {
                line: 3,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_some(),
            "Should offer column list for INSERT inside IF"
        );
    }

    /// INSERT inside IF's ELSE branch should trigger column list action
    #[test]
    fn test_insert_column_list_inside_if_else() {
        let source = concat!(
            "CREATE TABLE t (a INT, b INT)\n",
            "IF 1 = 1\n",
            "BEGIN\n",
            "    SELECT 1\n",
            "END\n",
            "ELSE\n",
            "BEGIN\n",
            "    INSERT INTO t VALUES (1, 2)\n",
            "END\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 7,
                character: 4,
            },
            end: Position {
                line: 7,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_some(),
            "Should offer column list for INSERT inside ELSE branch"
        );
    }

    /// INSERT inside a CREATE TRIGGER body should trigger column list action
    #[test]
    fn test_insert_column_list_inside_trigger() {
        let source = concat!(
            "CREATE TABLE t (id INT, name VARCHAR(100))\n",
            "CREATE TRIGGER trg ON t FOR INSERT AS\n",
            "BEGIN\n",
            "    INSERT INTO t VALUES (1, 'test')\n",
            "END\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 3,
                character: 4,
            },
            end: Position {
                line: 3,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_some(),
            "Should offer column list for INSERT inside TRIGGER"
        );
    }

    /// INSERT inside TRY block should trigger column list action
    #[test]
    fn test_insert_column_list_inside_try() {
        let source = concat!(
            "CREATE TABLE t (a INT, b INT)\n",
            "BEGIN TRY\n",
            "    INSERT INTO t VALUES (1, 2)\n",
            "END TRY\n",
            "BEGIN CATCH\n",
            "    SELECT 'error'\n",
            "END CATCH\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 2,
                character: 4,
            },
            end: Position {
                line: 2,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_some(),
            "Should offer column list for INSERT inside TRY"
        );
    }

    /// INSERT inside CATCH block should trigger column list action
    #[test]
    fn test_insert_column_list_inside_catch() {
        let source = concat!(
            "CREATE TABLE log_table (err_msg VARCHAR(255))\n",
            "BEGIN TRY\n",
            "    SELECT 1\n",
            "END TRY\n",
            "BEGIN CATCH\n",
            "    INSERT INTO log_table VALUES ('error')\n",
            "END CATCH\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 5,
                character: 4,
            },
            end: Position {
                line: 5,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_some(),
            "Should offer column list for INSERT inside CATCH"
        );
    }

    /// INSERT inside a stored procedure should trigger column list action
    #[test]
    fn test_insert_column_list_inside_procedure() {
        let source = concat!(
            "CREATE TABLE t (a INT, b INT)\n",
            "CREATE PROCEDURE my_proc AS\n",
            "BEGIN\n",
            "    INSERT INTO t VALUES (1, 2)\n",
            "END\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 3,
                character: 4,
            },
            end: Position {
                line: 3,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_some(),
            "Should offer column list for INSERT inside procedure"
        );
    }

    /// When all columns are IDENTITY, column list should not be offered
    #[test]
    fn test_insert_column_list_all_identity_no_action() {
        let source = "CREATE TABLE t (id INT IDENTITY)\nINSERT INTO t VALUES (DEFAULT)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_none(),
            "Should not offer column list when all columns are IDENTITY"
        );
    }

    // === Coverage gap tests ===

    /// INSERT skeleton generation via code_actions_with_analysis
    #[test]
    fn test_analysis_insert_skeleton() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nINSERT INTO users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let insert_action = actions.iter().find(
            |a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("INSERT")),
        );
        assert!(
            insert_action.is_some(),
            "code_actions_with_analysis should offer INSERT skeleton"
        );
    }

    /// code_actions_with_analysis returns empty for empty line
    #[test]
    fn test_analysis_empty_line_no_actions() {
        let source = "CREATE TABLE t (a INT)\n\nSELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
            actions.is_empty(),
            "Empty line should produce no actions"
        );
    }

    /// Symbol table fallback: DDL before parse errors
    /// When the full source fails to produce tables via build_tolerant,
    /// the fallback progressively shortens the source to find DDL definitions.
    #[test]
    fn test_fallback_symbol_table_with_parse_errors() {
        // Use SELECT * which triggers expand
        let source = concat!(
            "CREATE TABLE t (a INT, b INT)\n",
            "GO\n",
            "SELECT * FROM t\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 2,
                character: 0,
            },
            end: Position {
                line: 2,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        // The expand should find the table and offer SELECT * expansion
        let expand_action = actions.iter().find(
            |a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Expand")),
        );
        assert!(
            expand_action.is_some(),
            "Symbol table should find table definition and offer SELECT * expansion"
        );
    }

    /// `resolve_insert_stmt_end` with broken span (span.end == 0):
    /// Should fall back to semicolon-based scan
    #[test]
    fn test_insert_column_list_with_semicolon_terminated() {
        let source = "CREATE TABLE t (a INT, b INT)\nINSERT INTO t VALUES (1, 2);\nSELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        // Should still work with semicolon-terminated statements
        assert!(
            add_cols.is_some(),
            "Should offer column list for semicolon-terminated INSERT"
        );
    }

    /// `resolve_insert_stmt_end` fallback: no semicolon, uses last token
    #[test]
    fn test_insert_column_list_no_semicolon_uses_last_token() {
        let source = "CREATE TABLE t (a INT, b INT)\nINSERT INTO t VALUES (1, 2)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_some(),
            "Should offer column list even without semicolon"
        );
    }

    /// Cursor outside INSERT span should not trigger column list action
    #[test]
    fn test_insert_column_list_cursor_before_insert() {
        let source = "CREATE TABLE t (a INT, b INT)\nINSERT INTO t VALUES (1, 2)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Cursor on CREATE TABLE line
        let range = Range {
            start: Position {
                line: 0,
                character: 5,
            },
            end: Position {
                line: 0,
                character: 10,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_none(),
            "Should not offer column list when cursor is before INSERT"
        );
    }

    /// DocumentAnalysis::get_line utility: out-of-range line returns empty string
    #[test]
    fn test_get_line_out_of_range() {
        let analysis = crate::analysis::DocumentAnalysis::new("hello\nworld");
        assert_eq!(analysis.get_line(0), "hello");
        assert_eq!(analysis.get_line(1), "world");
        assert_eq!(analysis.get_line(5), "");
        let empty_analysis = crate::analysis::DocumentAnalysis::new("");
        assert_eq!(empty_analysis.get_line(0), "");
    }

    /// `find_values_token_start`: no VALUES token in range returns None
    /// This covers the early-break path when tokens exceed insert_end
    #[test]
    fn test_insert_skip_when_no_values_source() {
        // INSERT ... SELECT has no VALUES keyword
        let source = "CREATE TABLE t (a INT, b INT)\nINSERT INTO t SELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(
            add_cols.is_none(),
            "Should not offer column list for INSERT...SELECT (no VALUES)"
        );
    }

    /// TRY...CATCH wrap via code_actions_with_analysis:
    /// BEGIN on current line should trigger TRY...CATCH wrap
    #[test]
    fn test_analysis_try_catch() {
        let source = "BEGIN\n    SELECT 1\nEND";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
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
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let try_action = actions
            .iter()
            .find(|a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("TRY")));
        assert!(
            try_action.is_some(),
            "code_actions_with_analysis should offer TRY...CATCH for BEGIN"
        );
    }

    // === Multi-INSERT regression tests (CodeRabbit review) ===

    /// Two INSERTs inside WHILE: cursor on second INSERT should offer column list
    #[test]
    fn test_two_inserts_inside_while_targets_second() {
        let source = concat!(
            "CREATE TABLE t (a INT, b INT)\n",
            "WHILE 1 = 1\n",
            "BEGIN\n",
            "    INSERT INTO t VALUES (0, 0)\n",
            "    INSERT INTO t VALUES (1, 1)\n",
            "END\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 4,
                character: 4,
            },
            end: Position {
                line: 4,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        // Verify the action is offered for the second INSERT
        assert!(
            add_cols.is_some(),
            "Should offer column list for second INSERT"
        );
        if let CodeActionOrCommand::CodeAction(ca) = add_cols.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert!(
                text_edit.new_text.contains("a, b"),
                "new_text should contain column list"
            );
        }
    }

    /// Two INSERTs at top level: action is offered for whichever INSERT the cursor hits.
    /// NOTE: parser broken spans (span.end=0) may cause the first INSERT's resolved
    /// end to extend past the second INSERT, so cursor matching may hit either one.
    #[test]
    fn test_two_top_level_inserts_offers_action() {
        let source = concat!(
            "CREATE TABLE t (x INT, y INT)\n",
            "INSERT INTO t VALUES (10, 20)\n",
            "INSERT INTO t VALUES (30, 40)\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 2,
                character: 0,
            },
            end: Position {
                line: 2,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        // Action should be offered; which INSERT it targets depends on span resolution
        assert!(
            add_cols.is_some(),
            "Should offer column list for one of the INSERTs"
        );
    }

    /// Two INSERTs inside a procedure: cursor on second should target second
    #[test]
    fn test_two_inserts_inside_procedure_targets_second() {
        let source = concat!(
            "CREATE TABLE t (id INT, val VARCHAR(50))\n",
            "CREATE PROCEDURE p AS\n",
            "BEGIN\n",
            "    INSERT INTO t VALUES (1, 'a')\n",
            "    INSERT INTO t VALUES (2, 'b')\n",
            "END\n",
        );
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = Range {
            start: Position {
                line: 4,
                character: 4,
            },
            end: Position {
                line: 4,
                character: 20,
            },
        };
        let actions = code_actions_with_analysis(&analysis, range, &test_uri());
        let add_cols = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.contains("Add column list"))
        });
        assert!(add_cols.is_some());
        if let CodeActionOrCommand::CodeAction(ca) = add_cols.unwrap() {
            let edit = ca.edit.as_ref().unwrap();
            let changes = edit.changes.as_ref().unwrap();
            let text_edit = changes.get(&test_uri()).unwrap().first().unwrap();
            assert!(
                text_edit.new_text.contains("id, val"),
                "new_text: {}",
                text_edit.new_text,
            );
        }
    }

    // === Direct unit tests for fallback helpers ===

    /// `resolve_insert_stmt_end`: broken span (end == 0) falls back to semicolon
    #[test]
    fn test_resolve_insert_stmt_end_broken_span_uses_semicolon() {
        use crate::analysis::OwnedToken;
        use tsql_token::{Span, TokenKind};

        let tokens = vec![
            OwnedToken {
                kind: TokenKind::Insert,
                text: "INSERT".into(),
                span: Span::new(0, 6),
            },
            OwnedToken {
                kind: TokenKind::Ident,
                text: "t".into(),
                span: Span::new(12, 13),
            },
            OwnedToken {
                kind: TokenKind::Values,
                text: "VALUES".into(),
                span: Span::new(14, 20),
            },
            OwnedToken {
                kind: TokenKind::LParen,
                text: "(".into(),
                span: Span::new(21, 22),
            },
            OwnedToken {
                kind: TokenKind::Number,
                text: "1".into(),
                span: Span::new(22, 23),
            },
            OwnedToken {
                kind: TokenKind::RParen,
                text: ")".into(),
                span: Span::new(23, 24),
            },
            OwnedToken {
                kind: TokenKind::Semicolon,
                text: ";".into(),
                span: Span::new(24, 25),
            },
            OwnedToken {
                kind: TokenKind::Select,
                text: "SELECT".into(),
                span: Span::new(26, 32),
            },
        ];
        // Broken span: start=0, end=0 (parser didn't set end)
        let broken_span = Span::new(0, 0);
        let result = resolve_insert_stmt_end(&broken_span, &tokens);
        // Should find semicolon at position 25
        assert_eq!(result, 25, "Should fall back to semicolon boundary");
    }

    /// `resolve_insert_stmt_end`: valid span returns span.end directly
    #[test]
    fn test_resolve_insert_stmt_end_valid_span() {
        use crate::analysis::OwnedToken;

        let tokens: Vec<OwnedToken> = Vec::new();
        let valid_span = tsql_token::Span::new(10, 30);
        let result = resolve_insert_stmt_end(&valid_span, &tokens);
        assert_eq!(result, 30, "Should return span.end when valid");
    }

    /// `resolve_insert_stmt_end`: broken span with no semicolon uses last token
    #[test]
    fn test_resolve_insert_stmt_end_no_semicolon_uses_last_token() {
        use crate::analysis::OwnedToken;
        use tsql_token::{Span, TokenKind};

        let tokens = vec![
            OwnedToken {
                kind: TokenKind::Insert,
                text: "INSERT".into(),
                span: Span::new(0, 6),
            },
            OwnedToken {
                kind: TokenKind::Ident,
                text: "t".into(),
                span: Span::new(12, 13),
            },
            OwnedToken {
                kind: TokenKind::Values,
                text: "VALUES".into(),
                span: Span::new(14, 20),
            },
            OwnedToken {
                kind: TokenKind::Number,
                text: "1".into(),
                span: Span::new(22, 23),
            },
        ];
        let broken_span = Span::new(0, 0);
        let result = resolve_insert_stmt_end(&broken_span, &tokens);
        assert_eq!(result, 23, "Should fall back to last token's span.end");
    }

    /// `find_values_token_start`: respects insert_end boundary
    #[test]
    fn test_find_values_respects_end_boundary() {
        use crate::analysis::OwnedToken;
        use tsql_token::{Span, TokenKind};

        let tokens = vec![
            OwnedToken {
                kind: TokenKind::Insert,
                text: "INSERT".into(),
                span: Span::new(0, 6),
            },
            OwnedToken {
                kind: TokenKind::Values,
                text: "VALUES".into(),
                span: Span::new(14, 20),
            },
            OwnedToken {
                kind: TokenKind::Insert,
                text: "INSERT".into(),
                span: Span::new(30, 36),
            },
            OwnedToken {
                kind: TokenKind::Values,
                text: "VALUES".into(),
                span: Span::new(44, 50),
            },
        ];
        // First INSERT: start=0, end=29 → should find VALUES at 14, NOT at 44
        let result = find_values_token_start(&tokens, 0, 29);
        assert_eq!(result, Some(14), "Should find first VALUES within boundary");
        // Second INSERT: start=30, end=60 → should find VALUES at 44
        let result2 = find_values_token_start(&tokens, 30, 60);
        assert_eq!(
            result2,
            Some(44),
            "Should find second VALUES within boundary"
        );
    }

    /// SymbolTableBuilder::build_tolerant: when it succeeds, returns tables
    #[test]
    fn test_symbol_table_build_tolerant_works() {
        let source = "CREATE TABLE fb_t (a INT, b INT)\nSELECT * FROM fb_t";
        let table = crate::symbol_table::SymbolTableBuilder::build_tolerant(source);
        assert!(
            !table.tables.is_empty(),
            "build_tolerant should find at least one table from valid DDL"
        );
    }

    /// SymbolTableBuilder::build_tolerant: verifies it handles GO-separated batches
    #[test]
    fn test_symbol_table_build_tolerant_go_batches() {
        let source = "CREATE TABLE trunc_t (x INT)\nGO\nSELECT * FROM trunc_t\nGO";
        let table = crate::symbol_table::SymbolTableBuilder::build_tolerant(source);
        assert!(
            !table.tables.is_empty(),
            "Should find table (build_tolerant handles GO batches)"
        );
    }
}
