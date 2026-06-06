//! Hover 情報の提供
//!
//! T-SQL キーワード、データ型、組み込み関数、変数のホバー情報を提供する。
//! 静的ドキュメントデータは [`crate::db_docs`] モジュールに集約されている。

use crate::analysis::DocumentAnalysis;
use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};
use tsql_parser::ast::{Statement, TableReference};
use tsql_token::TokenKind;

/// Hover情報を生成する（DocumentAnalysis利用）
#[must_use]
pub fn hover_with_analysis(analysis: &DocumentAnalysis, position: Position) -> Option<Hover> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    let (token, _idx) = match analysis.find_token_at(offset) {
        Some(t) => t,
        None => {
            tracing::debug!("hover: no token found at offset {offset}");
            return None;
        }
    };
    let kind = token.kind;
    let text = token.text.clone();
    let start = token.span.start as usize;
    let end = token.span.end as usize;

    let content = build_schema_hover(&analysis.symbol_table, &kind, &text)
        .or_else(|| build_column_hover(analysis, offset, &text))
        .or_else(|| build_hover_content(&kind, &text));
    let content = match content {
        Some(c) => c,
        None => {
            tracing::debug!("hover: no documentation found for '{text}' ({kind:?})");
            return None;
        }
    };

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: Some(
            analysis
                .line_index
                .offset_to_range(start as u32, end as u32),
        ),
    })
}

/// Resolve identifier to column info by walking the AST to find the enclosing
/// SELECT's FROM clause tables, then looking up the column in the symbol table.
fn build_column_hover(
    analysis: &DocumentAnalysis,
    offset: usize,
    ident_text: &str,
) -> Option<String> {
    for stmt in &analysis.statements {
        if let Some(result) =
            resolve_column_in_statement(stmt, &analysis.symbol_table, offset, ident_text)
        {
            return Some(result);
        }
    }
    None
}

/// Format a column's hover content as markdown with T-SQL code block.
fn format_column_hover(col: &crate::symbol_table::ColumnSymbol, table_name: &str) -> String {
    let nullable = match col.nullable {
        Some(true) => " NULL",
        Some(false) => " NOT NULL",
        None => "",
    };
    let identity = if col.is_identity { " IDENTITY" } else { "" };
    format!(
        "```tsql\n{} {}{}{}\n```\n\n**Column** of `{}`",
        col.name, col.data_type, nullable, identity, table_name
    )
}

/// Check whether `offset` falls within `[span_start, span_end]`.
/// When `span_end <= span_start` (broken span), uses a fallback window.
fn in_span(offset: usize, span_start: usize, span_end: u32) -> bool {
    let span_end = if span_end as usize > span_start {
        span_end as usize
    } else {
        offset.saturating_add(2000)
    };
    offset >= span_start && offset <= span_end
}

/// Collect table names from a list of TableReference, including JOINed tables.
///
/// Returns borrowed `&str` references to avoid allocation — the caller passes
/// them to `SymbolTableBuilder::find_table` which handles case-insensitivity.
fn collect_table_names(tables: &[TableReference]) -> Vec<&str> {
    let mut names = Vec::new();
    for tr in tables {
        match tr {
            TableReference::Table { name, .. } => {
                names.push(name.name.as_str());
            }
            TableReference::Joined { joins, .. } => {
                for join in joins {
                    names.extend(collect_table_names(std::slice::from_ref(&join.table)));
                }
            }
            TableReference::Subquery { .. } => {}
        }
    }
    names
}

/// Resolve column information for a table reference within a statement at the given offset.
fn resolve_column_in_statement(
    stmt: &Statement,
    symbol_table: &crate::symbol_table::SymbolTable,
    offset: usize,
    upper_ident: &str,
) -> Option<String> {
    match stmt {
        Statement::Select(sel) => {
            if !in_span(offset, sel.span.start as usize, sel.span.end) {
                return None;
            }

            // Collect table names from FROM clause (including JOINs)
            let from = sel.from.as_ref()?;
            let table_names = collect_table_names(&from.tables);

            // Search each table for the column
            for table_name in &table_names {
                if let Some(tbl) =
                    crate::symbol_table::SymbolTableBuilder::find_table(symbol_table, table_name)
                {
                    for col in &tbl.columns {
                        if col.name.eq_ignore_ascii_case(upper_ident) {
                            return Some(format_column_hover(col, &tbl.name));
                        }
                    }
                }
            }
            None
        }
        Statement::Insert(insert) => {
            if !in_span(offset, insert.span.start as usize, insert.span.end) {
                return None;
            }

            // Check inserted columns
            if let Some(tbl) =
                crate::symbol_table::SymbolTableBuilder::find_table(symbol_table, &insert.table.name)
            {
                for col in &tbl.columns {
                    if col.name.eq_ignore_ascii_case(upper_ident) {
                        return Some(format_column_hover(col, &tbl.name));
                    }
                }
            }
            None
        }
        Statement::Update(update) => {
            if !in_span(offset, update.span.start as usize, update.span.end) {
                return None;
            }

            let table_name = match &update.table {
                TableReference::Table { name, .. } => name.name.as_str(),
                _ => return None,
            };
            // Collect tables from both the UPDATE target and FROM clause
            let mut all_tables: Vec<&str> = vec![table_name];
            if let Some(from_clause) = &update.from_clause {
                all_tables.extend(collect_table_names(&from_clause.tables));
            }
            for tbl_name in &all_tables {
                if let Some(tbl) =
                    crate::symbol_table::SymbolTableBuilder::find_table(symbol_table, tbl_name)
                {
                    for col in &tbl.columns {
                        if col.name.eq_ignore_ascii_case(upper_ident) {
                            return Some(format_column_hover(col, &tbl.name));
                        }
                    }
                }
            }
            None
        }
        Statement::Block(block) => block.statements.iter().find_map(|child| {
            resolve_column_in_statement(child, symbol_table, offset, upper_ident)
        }),
        Statement::If(if_stmt) => {
            resolve_column_in_statement(&if_stmt.then_branch, symbol_table, offset, upper_ident)
                .or_else(|| {
                    if_stmt.else_branch.as_ref().and_then(|else_b| {
                        resolve_column_in_statement(else_b, symbol_table, offset, upper_ident)
                    })
                })
        }
        Statement::While(while_stmt) => {
            resolve_column_in_statement(&while_stmt.body, symbol_table, offset, upper_ident)
        }
        Statement::TryCatch(tc) => tc
            .try_block
            .statements
            .iter()
            .chain(tc.catch_block.statements.iter())
            .find_map(|child| {
                resolve_column_in_statement(child, symbol_table, offset, upper_ident)
            }),
        Statement::Create(create) => match &**create {
            tsql_parser::ast::CreateStatement::Procedure(proc) => {
                proc.body.iter().find_map(|child| {
                    resolve_column_in_statement(child, symbol_table, offset, upper_ident)
                })
            }
            tsql_parser::ast::CreateStatement::Trigger(trigger) => {
                trigger.body.iter().find_map(|child| {
                    resolve_column_in_statement(child, symbol_table, offset, upper_ident)
                })
            }
            _ => None,
        },
        _ => None,
    }
}

/// シンボルテーブルからスキーマ情報のHoverを構築する
fn build_schema_hover(
    symbol_table: &crate::symbol_table::SymbolTable,
    kind: &TokenKind,
    text: &str,
) -> Option<String> {
    use crate::symbol_table::SymbolTableBuilder;

    match kind {
        TokenKind::LocalVar => {
            // 変数の型情報を表示
            if let Some(var) = SymbolTableBuilder::find_variable(symbol_table, text) {
                return Some(format!(
                    "```tsql\n{}: {}\n```\n\n**Variable** — Declared with `DECLARE {} {}`",
                    text, var.data_type, var.name, var.data_type
                ));
            }
            // プロシージャボディ内変数
            for proc in symbol_table.procedures.values() {
                for body_var in &proc.body_variables {
                    if body_var.name.eq_ignore_ascii_case(text) {
                        return Some(format!(
                            "```tsql\n{}: {}\n```\n\n**Variable** in `{}` — `DECLARE {} {}`",
                            text, body_var.data_type, proc.name, body_var.name, body_var.data_type
                        ));
                    }
                }
                for param in &proc.parameters {
                    if param.name.eq_ignore_ascii_case(text) {
                        let output_marker = if param.is_output { " OUTPUT" } else { "" };
                        return Some(format!(
                            "```tsql\n{}: {}{}\n```\n\n**Parameter** of `{}`",
                            text, param.data_type, output_marker, proc.name
                        ));
                    }
                }
            }
            None
        }
        TokenKind::Ident => {
            // テーブルのカラム情報を表示
            if let Some(table) = SymbolTableBuilder::find_table(symbol_table, text) {
                let mut cols = String::new();
                for col in &table.columns {
                    let nullable = match col.nullable {
                        Some(true) => " NULL",
                        Some(false) => " NOT NULL",
                        None => "",
                    };
                    let identity = if col.is_identity { " IDENTITY" } else { "" };
                    cols.push_str(&format!(
                        "\n  `{} {}`{}{}",
                        col.name, col.data_type, nullable, identity
                    ));
                }
                return Some(format!(
                    "```tsql\nCREATE TABLE {} ({}\n)\n```\n\n**Table** — {} column{}",
                    table.name,
                    cols,
                    table.columns.len(),
                    if table.columns.len() != 1 { "s" } else { "" }
                ));
            }
            // プロシージャ情報を表示
            if let Some(proc) = SymbolTableBuilder::find_procedure(symbol_table, text) {
                let mut params = String::new();
                for p in &proc.parameters {
                    let output = if p.is_output { " OUTPUT" } else { "" };
                    params.push_str(&format!("\n  `{} {}{}`", p.name, p.data_type, output));
                }
                return Some(format!(
                    "```tsql\nCREATE PROCEDURE {} ({}\n)\n```\n\n**Procedure** — {} parameter{}",
                    proc.name,
                    params,
                    proc.parameters.len(),
                    if proc.parameters.len() != 1 { "s" } else { "" }
                ));
            }
            // ビュー情報を表示
            if let Some(_view) = SymbolTableBuilder::find_view(symbol_table, text) {
                return Some(format!("**`{}`** — View", text));
            }
            // インデックス情報を表示
            if let Some(idx) = SymbolTableBuilder::find_index(symbol_table, text) {
                let unique = if idx.is_unique { "UNIQUE " } else { "" };
                let cols = idx.columns.join(", ");
                return Some(format!(
                    "```tsql\n{}INDEX {} ON {} ({})\n```\n\n**Index** — {} column{} on `{}`",
                    unique,
                    idx.name,
                    idx.table_name,
                    cols,
                    idx.columns.len(),
                    if idx.columns.len() != 1 { "s" } else { "" },
                    idx.table_name
                ));
            }
            // トリガー情報を表示
            if let Some(trigger) = SymbolTableBuilder::find_trigger(symbol_table, text) {
                let events = trigger.events.join(", ");
                return Some(format!(
                    "```tsql\nCREATE TRIGGER {} ON {} FOR {}\n```\n\n**Trigger** — on `{}`",
                    trigger.name, trigger.table_name, events, trigger.table_name
                ));
            }
            None
        }
        _ => None,
    }
}

/// トークンの種類に応じてHover内容を構築する（静的ドキュメント）
///
/// [`crate::db_docs`] からエントリを検索し、マークダウン形式で返す。
fn build_hover_content(kind: &TokenKind, text: &str) -> Option<String> {
    let upper = text.to_uppercase();

    match kind {
        TokenKind::LocalVar => Some(format!(
            "```tsql\n{text}: VARIABLE\n```\n\nLocal variable — Declare with `DECLARE {text} TYPE`"
        )),
        _ => {
            if let Some(entry) = crate::db_docs::lookup(upper.as_str()) {
                Some(format!(
                    "```tsql\n{}\n```\n\n**`{}`** — {}",
                    entry.syntax, upper, entry.description
                ))
            } else if kind.is_keyword() {
                Some(format!("**`{upper}`** — T-SQL Keyword"))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // --- Index and View hover enhancement tests (Phase #57) ---

    #[test]
    fn test_hover_index_shows_table_and_columns_via_source() {
        let source =
            "CREATE TABLE users (id INT, name VARCHAR(50))\nCREATE INDEX idx_name ON users (name)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 13,
            },
        );
        assert!(result.is_some(), "Should return hover for index name");
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(
                    mc.value.contains("Index"),
                    "Should mention Index: {}",
                    mc.value
                );
                assert!(
                    mc.value.contains("users"),
                    "Should mention table: {}",
                    mc.value
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_index_with_analysis() {
        let source =
            "CREATE TABLE users (id INT, name VARCHAR(50))\nCREATE INDEX idx_name ON users (name)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 13, // on "idx_name"
            },
        );
        assert!(result.is_some(), "Should return hover for index name");
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(
                    mc.value.contains("Index"),
                    "Should mention Index: {}",
                    mc.value
                );
                assert!(
                    mc.value.contains("users"),
                    "Should mention table name: {}",
                    mc.value
                );
                assert!(
                    mc.value.contains("name"),
                    "Should mention indexed column: {}",
                    mc.value
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_index_on_referenced_table() {
        let source =
            "CREATE TABLE users (id INT, email VARCHAR(100))\nCREATE INDEX idx_email ON users (email)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 13, // on "idx_email"
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(
                    mc.value.contains("Index"),
                    "Should mention Index: {}",
                    mc.value
                );
                assert!(
                    mc.value.contains("email"),
                    "Should mention indexed column: {}",
                    mc.value
                );
                assert!(
                    mc.value.contains("users"),
                    "Should mention table: {}",
                    mc.value
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_column_from_select() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(50))\nSELECT id, name FROM users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Hover over "id" in SELECT (line 1, char 7)
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 7,
            },
        );
        assert!(result.is_some(), "Should resolve column 'id'");
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(
                    mc.value.contains("Column"),
                    "Should be a column hover: {}",
                    mc.value
                );
                assert!(
                    mc.value.contains("INT"),
                    "Should show data type: {}",
                    mc.value
                );
                assert!(
                    mc.value.contains("users"),
                    "Should mention table: {}",
                    mc.value
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_column_from_where() {
        let source = "CREATE TABLE orders (id INT, total DECIMAL(10,2) NOT NULL)\nSELECT * FROM orders WHERE total > 100";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Hover over "total" in WHERE (line 1, char 30)
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 30,
            },
        );
        assert!(result.is_some(), "Should resolve column 'total'");
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(
                    mc.value.contains("Column"),
                    "Should be a column hover: {}",
                    mc.value
                );
                assert!(
                    mc.value.contains("NOT NULL"),
                    "Should show nullable: {}",
                    mc.value
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_column_identity() {
        let source = "CREATE TABLE t (id INT IDENTITY, val INT)\nSELECT id FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 7,
            },
        );
        assert!(result.is_some(), "Should resolve identity column");
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(
                    mc.value.contains("IDENTITY"),
                    "Should show IDENTITY: {}",
                    mc.value
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_column_in_insert() {
        let source = "CREATE TABLE t (a INT, b INT)\nINSERT INTO t (a) VALUES (1)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Hover over "a" in INSERT column list (line 1, char 15)
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert!(result.is_some(), "Should resolve column in INSERT");
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(
                    mc.value.contains("Column"),
                    "Should be a column hover: {}",
                    mc.value
                );
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_nonexistent_column_returns_none() {
        let source = "CREATE TABLE t (id INT)\nSELECT nonexistent FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 7,
            },
        );
        // "nonexistent" is not a column of t — should not resolve as column
        // (might still show static keyword hover if it matches something)
        if let Some(h) = result {
            if let HoverContents::Markup(mc) = &h.contents {
                assert!(
                    !mc.value.contains("Column"),
                    "Nonexistent column should not resolve as column: {}",
                    mc.value
                );
            }
        }
    }

    #[test]
    fn test_hover_empty_source_returns_none() {
        let analysis = crate::analysis::DocumentAnalysis::new("");
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 0,
            },
        );
        assert!(result.is_none(), "Empty source should return None");
    }

    #[test]
    fn test_hover_builtin_function_getdate() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT GETDATE()");
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 8,
            },
        );
        assert!(result.is_some(), "GETDATE should have hover info");
        if let Some(h) = result {
            if let HoverContents::Markup(mc) = &h.contents {
                assert!(
                    mc.value.contains("GETDATE"),
                    "Should contain GETDATE: {}",
                    mc.value
                );
            }
        }
    }

    #[test]
    fn test_hover_position_beyond_end_returns_none() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT 1");
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 999,
            },
        );
        assert!(
            result.is_none(),
            "Position beyond source end should return None"
        );
    }

    #[test]
    fn test_hover_column_inside_trigger_body() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\n\
                      CREATE TRIGGER tr_test ON users FOR INSERT AS\n\
                      BEGIN\n\
                          SELECT id FROM users\n\
                      END";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Hover over "id" inside the trigger body (line 3, char 18)
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 3,
                character: 18,
            },
        );
        assert!(
            result.is_some(),
            "Hover over column inside trigger body should return hover info"
        );
        let h = result.expect("checked is_some");
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(
                    mc.value.contains("id"),
                    "Hover inside trigger should show column name"
                );
            }
            other => panic!("Expected Markup content, got {other:?}"),
        }
    }

    #[test]
    fn test_hover_trigger_shows_info() {
        let source = "CREATE TRIGGER tr_test ON users FOR INSERT, UPDATE AS BEGIN SELECT 1 END";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Hover over "tr_test" (line 0, char 18)
        let result = hover_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 18,
            },
        );
        assert!(result.is_some(), "Hover on trigger name should return info");
        let h = result.expect("checked is_some");
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("Trigger"), "Should show Trigger label");
                assert!(mc.value.contains("users"), "Should show target table name");
            }
            other => panic!("Expected Markup content, got {other:?}"),
        }
    }
}
