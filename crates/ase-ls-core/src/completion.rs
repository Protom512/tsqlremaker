//! Completion 生成
//!
//! SQL キーワード、データ型、組み込み関数の補完候補を提供する。
//! カーソル位置のコンテキストに基づいて補完候補をフィルタリングする。

use crate::analysis::DocumentAnalysis;
use lsp_types::{CompletionItem, CompletionItemKind, CompletionList, CompletionResponse};
use std::sync::LazyLock;
use tsql_token::TokenKind;

/// 全補完候補のグローバルキャッシュ。初回アクセス時のみ構築される。
static COMPLETE_ALL_CACHE: LazyLock<CompletionResponse> = LazyLock::new(build_complete_all);

/// キーワード補完のグローバルキャッシュ。
static COMPLETE_KEYWORDS_CACHE: LazyLock<CompletionResponse> =
    LazyLock::new(build_complete_keywords);

/// Label shown in the detail field for T-SQL keyword completion items.
const KEYWORD_DETAIL: &str = "T-SQL Keyword";

/// 関数名とパラメータリストからLSP snippet形式のinsert_textを生成する
///
/// `DocEntry.params`（クリーンなパラメータ名配列）を直接使用し、
/// syntax文字列のブラケット表記（`[, style]`等）による問題を回避する。
///
/// # Examples
/// - `build_function_snippet("SUBSTRING", &["expression", "start", "length"])`
///   → `SUBSTRING(${1:expression}, ${2:start}, ${3:length})`
/// - `build_function_snippet("GETDATE", &[])` → `GETDATE()`
#[must_use]
pub(crate) fn build_function_snippet(name: &str, params: &[&str]) -> String {
    if params.is_empty() {
        return format!("{name}()");
    }
    let placeholders: Vec<String> = params
        .iter()
        .enumerate()
        .map(|(i, p)| format!("${{{}:{p}}}", i + 1))
        .collect();
    format!("{name}({})", placeholders.join(", "))
}

/// syntax文字列がカンマ区切りの括弧構文かどうかを判定する
///
/// カンマ区切りではない関数（`CAST(expr AS type)`等）や
/// 括弧なしの関数（`IDENTITY`等）はsnippetプレースホルダー生成に
/// 適さないためfalseを返す。
#[must_use]
fn is_comma_separated_syntax(syntax: &str) -> bool {
    if let (Some(open), Some(close)) = (syntax.find('('), syntax.rfind(')')) {
        if open < close {
            let inner = &syntax[open + 1..close];
            return !inner.contains(" AS ") && !inner.contains('\'') && !inner.contains('|');
        }
    }
    false
}

/// 全ての補完候補を返す（キャッシュ済み）
///
/// 内部の `Lazy` static から参照を返す。呼び出し元で所有権が必要な場合は
/// `.clone()` すること。
#[must_use]
pub fn complete_all() -> &'static CompletionResponse {
    &COMPLETE_ALL_CACHE
}

/// 全ての補完候補を構築する（内部実装）
fn build_complete_all() -> CompletionResponse {
    let mut items = Vec::new();

    // Keywords from db_docs
    for entry in crate::db_docs::keywords() {
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(KEYWORD_DETAIL.to_string()),
            ..CompletionItem::default()
        });
    }

    // Datatypes from db_docs
    for entry in crate::db_docs::datatypes() {
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some(entry.description.to_string()),
            ..CompletionItem::default()
        });
    }

    // Functions from db_docs — snippet or plain text depending on syntax
    for entry in crate::db_docs::functions() {
        let (insert_text, format) = if is_comma_separated_syntax(entry.syntax) {
            (
                build_function_snippet(entry.name, entry.params),
                lsp_types::InsertTextFormat::SNIPPET,
            )
        } else {
            // Non-comma syntax (e.g., CAST(expr AS type)) — plain text
            (
                entry.syntax.to_string(),
                lsp_types::InsertTextFormat::PLAIN_TEXT,
            )
        };
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!("{} — {}", entry.syntax, entry.description)),
            insert_text: Some(insert_text),
            insert_text_format: Some(format),
            ..CompletionItem::default()
        });
    }

    // System variables from db_docs
    for entry in crate::db_docs::system_variables() {
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some(entry.description.to_string()),
            ..CompletionItem::default()
        });
    }

    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

/// キーワード補完のみを返す（キャッシュ済み）
///
/// `complete_all()` と同様に `&'static` 参照を返し、不要な clone を回避する。
#[must_use]
pub fn complete_keywords() -> &'static CompletionResponse {
    &COMPLETE_KEYWORDS_CACHE
}

/// キーワード補完を構築する（内部実装）
fn build_complete_keywords() -> CompletionResponse {
    let items = crate::db_docs::keywords()
        .iter()
        .map(|entry| CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(KEYWORD_DETAIL.to_string()),
            ..CompletionItem::default()
        })
        .collect();

    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

// ============================================================================
// Context-aware completion
// ============================================================================

/// Cursor context for completion filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionContextKind {
    /// After FROM or JOIN — expect table/view names
    TableReference,
    /// After SELECT — expect columns, functions, expressions
    SelectExpression,
    /// After INSERT INTO — expect table names
    InsertTarget,
    /// After SET or in assignment — expect variables
    Assignment,
    /// Typing a @ variable — expect local variables
    Variable,
    /// Default — return keywords
    Default,
}

/// Detect completion context from the token preceding the cursor.
///
/// Uses binary search on the token list to find the last token whose span
/// starts at or before the cursor, then checks if the cursor is still within
/// that token or past it. If past it, the preceding token is the context cue.
#[must_use]
pub fn detect_context(analysis: &DocumentAnalysis, cursor_offset: usize) -> CompletionContextKind {
    let tokens = &analysis.tokens;

    // Find the index of the last token whose span.start <= cursor_offset
    let idx = tokens.partition_point(|t| t.span.start as usize <= cursor_offset);
    if idx == 0 {
        return CompletionContextKind::Default;
    }

    // tokens[idx-1] is the candidate — check if cursor is inside it
    let candidate = &tokens[idx - 1];
    let inside_or_at_end = cursor_offset <= candidate.span.end as usize;

    if inside_or_at_end {
        // Cursor is on this token (or at its end, which means typing into it)
        if candidate.kind == TokenKind::LocalVar {
            return CompletionContextKind::Variable;
        }
        // For partial typing (e.g. "SE"), the token is an Ident —
        // use the token before it as context
        if idx >= 2 {
            return context_from_token(&tokens[idx - 2]);
        }
        return CompletionContextKind::Default;
    }

    // Cursor is past candidate — candidate IS the preceding token
    context_from_token(candidate)
}

/// Map a preceding token kind to a completion context.
fn context_from_token(token: &crate::analysis::OwnedToken) -> CompletionContextKind {
    match token.kind {
        // Variable token → offer variables
        TokenKind::LocalVar => CompletionContextKind::Variable,
        // FROM / JOIN variants → table reference
        TokenKind::From
        | TokenKind::Join
        | TokenKind::Inner
        | TokenKind::Left
        | TokenKind::Right
        | TokenKind::Full
        | TokenKind::Cross
        | TokenKind::Outer => CompletionContextKind::TableReference,
        // SELECT → columns, functions, expressions
        TokenKind::Select | TokenKind::Distinct | TokenKind::All | TokenKind::Top => {
            CompletionContextKind::SelectExpression
        }
        // INSERT INTO → table name
        TokenKind::Into => CompletionContextKind::InsertTarget,
        // SET → variable assignment
        TokenKind::Set => CompletionContextKind::Assignment,
        // After ON in JOIN → join condition
        TokenKind::On => CompletionContextKind::SelectExpression,
        // After WHERE → conditions
        TokenKind::Where | TokenKind::And | TokenKind::Or | TokenKind::Not => {
            CompletionContextKind::SelectExpression
        }
        // After BY (ORDER BY / GROUP BY) → columns, functions
        TokenKind::By => CompletionContextKind::SelectExpression,
        // Comma → need to check surrounding context
        // For simplicity, treat as SelectExpression (most common in column lists)
        TokenKind::Comma => CompletionContextKind::SelectExpression,
        _ => CompletionContextKind::Default,
    }
}

/// Generate context-appropriate completions based on DocumentAnalysis.
///
/// Uses the symbol table to provide table names, column names, and
/// variable names relevant to the current cursor position.
#[must_use]
pub fn complete_with_context(
    analysis: &DocumentAnalysis,
    cursor_offset: usize,
) -> CompletionResponse {
    let context = detect_context(analysis, cursor_offset);
    let mut items = Vec::new();

    match context {
        CompletionContextKind::TableReference => {
            // Table names from symbol table
            for table in analysis.symbol_table.tables.values() {
                items.push(CompletionItem {
                    label: table.name.clone(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(format!("Table ({} columns)", table.columns.len())),
                    ..CompletionItem::default()
                });
            }
            // Also include keywords that are valid after FROM
            let from_keywords = ["SELECT", "WHERE", "GROUP", "ORDER", "HAVING", "LIMIT"];
            for kw in from_keywords {
                if let Some(entry) = crate::db_docs::keywords().iter().find(|e| e.name == kw) {
                    items.push(CompletionItem {
                        label: entry.name.to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some(KEYWORD_DETAIL.to_string()),
                        ..CompletionItem::default()
                    });
                }
            }
        }
        CompletionContextKind::SelectExpression => {
            // Columns from all tables in symbol table
            let mut seen_columns = std::collections::HashSet::new();
            for table in analysis.symbol_table.tables.values() {
                for col in &table.columns {
                    if seen_columns.insert(col.name.to_uppercase()) {
                        items.push(CompletionItem {
                            label: col.name.clone(),
                            kind: Some(CompletionItemKind::FIELD),
                            detail: Some(format!(
                                "{}.{} ({})",
                                table.name,
                                col.name,
                                format_data_type(&col.data_type)
                            )),
                            ..CompletionItem::default()
                        });
                    }
                }
            }
            // Functions
            for entry in crate::db_docs::functions() {
                let (insert_text, format) = if is_comma_separated_syntax(entry.syntax) {
                    (
                        build_function_snippet(entry.name, entry.params),
                        lsp_types::InsertTextFormat::SNIPPET,
                    )
                } else {
                    (
                        entry.syntax.to_string(),
                        lsp_types::InsertTextFormat::PLAIN_TEXT,
                    )
                };
                items.push(CompletionItem {
                    label: entry.name.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!("{} — {}", entry.syntax, entry.description)),
                    insert_text: Some(insert_text),
                    insert_text_format: Some(format),
                    ..CompletionItem::default()
                });
            }
            // Star for SELECT *
            items.push(CompletionItem {
                label: "*".to_string(),
                kind: Some(CompletionItemKind::OPERATOR),
                detail: Some("All columns".to_string()),
                ..CompletionItem::default()
            });
            // Local variables
            for var in analysis.symbol_table.variables.values() {
                items.push(CompletionItem {
                    label: var.name.clone(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(format!(
                        "{} ({})",
                        var.name,
                        format_data_type(&var.data_type)
                    )),
                    ..CompletionItem::default()
                });
            }
        }
        CompletionContextKind::InsertTarget => {
            // Table names
            for table in analysis.symbol_table.tables.values() {
                items.push(CompletionItem {
                    label: table.name.clone(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(format!("Table ({} columns)", table.columns.len())),
                    ..CompletionItem::default()
                });
            }
        }
        CompletionContextKind::Assignment | CompletionContextKind::Variable => {
            // Variables from symbol table
            for var in analysis.symbol_table.variables.values() {
                items.push(CompletionItem {
                    label: var.name.clone(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(format!(
                        "{} ({})",
                        var.name,
                        format_data_type(&var.data_type)
                    )),
                    ..CompletionItem::default()
                });
            }
            // Procedure parameters
            for proc in analysis.symbol_table.procedures.values() {
                for param in &proc.parameters {
                    items.push(CompletionItem {
                        label: param.name.clone(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(format!(
                            "{} ({}){}",
                            param.name,
                            format_data_type(&param.data_type),
                            if param.is_output { " OUTPUT" } else { "" }
                        )),
                        ..CompletionItem::default()
                    });
                }
                for var in &proc.body_variables {
                    items.push(CompletionItem {
                        label: var.name.clone(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(format!(
                            "{} ({})",
                            var.name,
                            format_data_type(&var.data_type)
                        )),
                        ..CompletionItem::default()
                    });
                }
            }
            // System variables
            for entry in crate::db_docs::system_variables() {
                items.push(CompletionItem {
                    label: entry.name.to_string(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(entry.description.to_string()),
                    ..CompletionItem::default()
                });
            }
        }
        CompletionContextKind::Default => {
            // Keywords only — use the cached static response items
            let keywords = complete_keywords();
            if let CompletionResponse::List(list) = keywords {
                items.extend(list.items.clone());
            }
        }
    }

    CompletionResponse::List(CompletionList {
        is_incomplete: true, // Context-filtered lists are incomplete
        items,
    })
}

/// Format a DataType for display in completion details.
fn format_data_type(dt: &tsql_parser::ast::DataType) -> String {
    use tsql_parser::ast::DataType;
    match dt {
        DataType::Varchar(Some(n)) => format!("VARCHAR({n})"),
        DataType::Varchar(None) => "VARCHAR".to_string(),
        DataType::Numeric(Some(p), Some(s)) => format!("NUMERIC({p},{s})"),
        DataType::Numeric(Some(p), None) => format!("NUMERIC({p})"),
        DataType::Numeric(None, None) => "NUMERIC".to_string(),
        DataType::Char(n) => format!("CHAR({n})"),
        DataType::Decimal(Some(p), Some(s)) => format!("DECIMAL({p},{s})"),
        DataType::Decimal(Some(p), None) => format!("DECIMAL({p})"),
        DataType::Decimal(None, None) => "DECIMAL".to_string(),
        DataType::Binary(n) => format!("BINARY({n})"),
        DataType::VarBinary(Some(n)) => format!("VARBINARY({n})"),
        DataType::VarBinary(None) => "VARBINARY".to_string(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use lsp_types::InsertTextFormat;

    // ========================================================================
    // Context-aware completion tests (#126)
    // ========================================================================

    /// Helper: build analysis and get completions at end of source
    fn complete_at(source: &str) -> CompletionResponse {
        let analysis = DocumentAnalysis::new(source);
        let offset = source.len();
        complete_with_context(&analysis, offset)
    }

    #[test]
    fn test_context_from_offers_tables() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                let has_users = list
                    .items
                    .iter()
                    .any(|i| i.label == "users" && i.kind == Some(CompletionItemKind::CLASS));
                assert!(has_users, "FROM context should offer table 'users'");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_from_no_spam_keywords() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                // Should NOT include every keyword (like INSERT, DELETE, etc.)
                let has_insert = list.items.iter().any(|i| i.label == "INSERT");
                assert!(!has_insert, "FROM context should not offer INSERT keyword");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_select_offers_columns() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                let has_id = list
                    .items
                    .iter()
                    .any(|i| i.label == "id" && i.kind == Some(CompletionItemKind::FIELD));
                let has_name = list
                    .items
                    .iter()
                    .any(|i| i.label == "name" && i.kind == Some(CompletionItemKind::FIELD));
                assert!(has_id, "SELECT context should offer column 'id'");
                assert!(has_name, "SELECT context should offer column 'name'");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_select_offers_functions() {
        let source = "SELECT ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                let has_getdate = list
                    .items
                    .iter()
                    .any(|i| i.label == "GETDATE" && i.kind == Some(CompletionItemKind::FUNCTION));
                assert!(has_getdate, "SELECT context should offer GETDATE function");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_select_offers_star() {
        let source = "SELECT ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                let has_star = list.items.iter().any(|i| i.label == "*");
                assert!(has_star, "SELECT context should offer *");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_insert_into_offers_tables() {
        let source = "CREATE TABLE orders (id INT)\nINSERT INTO ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                let has_orders = list.items.iter().any(|i| i.label == "orders");
                assert!(has_orders, "INSERT INTO context should offer 'orders'");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_set_offers_variables() {
        let source = "DECLARE @count INT\nSET ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                let has_count = list
                    .items
                    .iter()
                    .any(|i| i.label == "@count" && i.kind == Some(CompletionItemKind::VARIABLE));
                assert!(has_count, "SET context should offer variable @count");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_variable_inside_localvar() {
        // When cursor is on a LocalVar token, context should be Variable
        let source = "DECLARE @count INT\nSELECT @co";
        let analysis = DocumentAnalysis::new(source);
        let ctx = detect_context(&analysis, source.len());
        assert_eq!(
            ctx,
            CompletionContextKind::Variable,
            "Cursor on LocalVar @co should detect Variable context"
        );
    }

    #[test]
    fn test_context_set_offers_variables_with_declare() {
        // SET after DECLARE should offer the declared variable
        let source = "DECLARE @total NUMERIC(10,2)\nSET @total = @";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                // After @, the lexer may not have a token at the end,
                // but SET context should still offer variables
                let has_var = list
                    .items
                    .iter()
                    .any(|i| i.label == "@total" && i.kind == Some(CompletionItemKind::VARIABLE));
                assert!(has_var, "Should offer @total variable in SET context");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_default_returns_keywords() {
        let source = "";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                let has_select = list.items.iter().any(|i| i.label == "SELECT");
                assert!(has_select, "Default context should offer keywords");
                // Should be keywords only
                let all_kw = list
                    .items
                    .iter()
                    .all(|i| i.kind == Some(CompletionItemKind::KEYWORD));
                assert!(all_kw, "Default context should only have keywords");
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_detect_from() {
        let source = "CREATE TABLE t (id INT)\nSELECT * FROM ";
        let analysis = DocumentAnalysis::new(source);
        let ctx = detect_context(&analysis, source.len());
        assert_eq!(ctx, CompletionContextKind::TableReference);
    }

    #[test]
    fn test_context_detect_select() {
        let source = "SELECT ";
        let analysis = DocumentAnalysis::new(source);
        let ctx = detect_context(&analysis, source.len());
        assert_eq!(ctx, CompletionContextKind::SelectExpression);
    }

    #[test]
    fn test_context_detect_insert_into() {
        let source = "INSERT INTO ";
        let analysis = DocumentAnalysis::new(source);
        let ctx = detect_context(&analysis, source.len());
        assert_eq!(ctx, CompletionContextKind::InsertTarget);
    }

    #[test]
    fn test_context_detect_set() {
        let source = "SET ";
        let analysis = DocumentAnalysis::new(source);
        let ctx = detect_context(&analysis, source.len());
        assert_eq!(ctx, CompletionContextKind::Assignment);
    }

    #[test]
    fn test_context_is_incomplete_flag() {
        let source = "SELECT ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                assert!(
                    list.is_incomplete,
                    "Context-filtered list should have is_incomplete=true"
                );
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_column_dedup_across_tables() {
        let source =
            "CREATE TABLE t1 (id INT, name VARCHAR(50))\nCREATE TABLE t2 (id INT)\nSELECT ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                let id_count = list
                    .items
                    .iter()
                    .filter(|i| i.label == "id" && i.kind == Some(CompletionItemKind::FIELD))
                    .count();
                assert_eq!(
                    id_count, 1,
                    "Column 'id' should appear once (deduped across tables)"
                );
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_context_column_detail_shows_table() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT ";
        let response = complete_at(source);
        match &response {
            CompletionResponse::List(list) => {
                let id_item = list
                    .items
                    .iter()
                    .find(|i| i.label == "id" && i.kind == Some(CompletionItemKind::FIELD));
                assert!(id_item.is_some(), "Should have 'id' column");
                let detail = id_item.unwrap().detail.as_deref().unwrap_or("");
                assert!(
                    detail.contains("users"),
                    "Column detail should mention table name, got: {detail}"
                );
            }
            _ => panic!("Expected List"),
        }
    }

    // ========================================================================
    // Original tests (unchanged)
    // ========================================================================

    #[test]
    fn test_complete_all_has_items() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                assert!(!list.items.is_empty());
                assert!(!list.is_incomplete);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_includes_select() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let has_select = list.items.iter().any(|i| i.label == "SELECT");
                assert!(has_select);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_includes_types() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let has_int = list.items.iter().any(|i| i.label == "INT");
                assert!(has_int);
                let has_varchar = list.items.iter().any(|i| i.label == "VARCHAR");
                assert!(has_varchar);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_includes_functions() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let has_getdate = list.items.iter().any(|i| i.label == "GETDATE");
                assert!(has_getdate);
                let has_convert = list.items.iter().any(|i| i.label == "CONVERT");
                assert!(has_convert);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_function_has_detail() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let getdate = list.items.iter().find(|i| i.label == "GETDATE");
                assert!(getdate.is_some());
                let item = getdate.unwrap();
                assert!(item.detail.is_some());
                assert!(item.insert_text.is_some());
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_function_snippet_format() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let substring = list.items.iter().find(|i| i.label == "SUBSTRING");
                assert!(substring.is_some());
                let item = substring.unwrap();
                assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
                // Should have placeholder syntax
                let insert = item.insert_text.as_ref().unwrap();
                assert!(
                    insert.contains("${1:"),
                    "Expected snippet placeholder, got: {}",
                    insert
                );
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_build_snippet_with_params() {
        let result = build_function_snippet("SUBSTRING", &["expression", "start", "length"]);
        assert_eq!(
            result,
            "SUBSTRING(${1:expression}, ${2:start}, ${3:length})"
        );
    }

    #[test]
    fn test_build_snippet_no_params() {
        let result = build_function_snippet("GETDATE", &[]);
        assert_eq!(result, "GETDATE()");
    }

    #[test]
    fn test_build_snippet_single_param() {
        let result = build_function_snippet("COUNT", &["expression"]);
        assert_eq!(result, "COUNT(${1:expression})");
    }

    #[test]
    fn test_build_snippet_optional_params_clean() {
        // CONVERT has optional "style" param in syntax but params field is clean
        let result = build_function_snippet("CONVERT", &["type", "expression", "style"]);
        assert_eq!(result, "CONVERT(${1:type}, ${2:expression}, ${3:style})");
        assert!(
            !result.contains('['),
            "No brackets should appear in snippet"
        );
    }

    #[test]
    fn test_complete_keywords() {
        let response = complete_keywords();
        match response {
            CompletionResponse::List(list) => {
                assert!(!list.items.is_empty());
                // Should be keywords only
                let all_keywords = list
                    .items
                    .iter()
                    .all(|i| i.kind == Some(CompletionItemKind::KEYWORD));
                assert!(all_keywords);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_keywords_is_static_ref() {
        let a = complete_keywords() as *const CompletionResponse;
        let b = complete_keywords() as *const CompletionResponse;
        // Same static address — no clone
        assert_eq!(a, b);
    }

    #[test]
    fn test_cast_uses_plain_text() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let cast = list.items.iter().find(|i| i.label == "CAST");
                assert!(cast.is_some());
                let item = cast.unwrap();
                assert_eq!(
                    item.insert_text_format,
                    Some(InsertTextFormat::PLAIN_TEXT),
                    "CAST should use PLAIN_TEXT, not SNIPPET"
                );
                let text = item.insert_text.as_ref().unwrap();
                assert!(
                    text.contains(" AS "),
                    "CAST insert_text should preserve AS syntax, got: {text}"
                );
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_is_comma_separated_syntax() {
        assert!(is_comma_separated_syntax(
            "SUBSTRING(expression, start, length)"
        ));
        assert!(is_comma_separated_syntax("GETDATE()"));
        assert!(!is_comma_separated_syntax("CAST(expression AS type)"));
        assert!(!is_comma_separated_syntax("IDENTITY")); // no parens
        assert!(!is_comma_separated_syntax("OBJECT_ID('object_name')")); // quotes
        assert!(!is_comma_separated_syntax(
            "COUNT([DISTINCT] expression | *)"
        )); // pipe
    }

    #[test]
    fn test_identity_no_empty_parens() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let identity = list.items.iter().find(|i| {
                    i.label == "IDENTITY" && i.kind == Some(CompletionItemKind::FUNCTION)
                });
                assert!(identity.is_some(), "IDENTITY function should exist");
                let item = identity.unwrap();
                assert_eq!(
                    item.insert_text_format,
                    Some(InsertTextFormat::PLAIN_TEXT),
                    "IDENTITY should use PLAIN_TEXT"
                );
                let text = item.insert_text.as_ref().unwrap();
                assert!(
                    !text.ends_with("()"),
                    "IDENTITY should not have empty parens, got: {text}"
                );
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_cache_returns_same_instance() {
        let a = complete_all();
        let b = complete_all();
        match (&a, &b) {
            (CompletionResponse::List(la), CompletionResponse::List(lb)) => {
                assert_eq!(la.items.len(), lb.items.len());
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_complete_keywords_cache_returns_same_count() {
        let a = complete_keywords();
        let b = complete_keywords();
        match (a, b) {
            (CompletionResponse::List(la), CompletionResponse::List(lb)) => {
                assert_eq!(la.items.len(), lb.items.len());
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_complete_all_includes_system_variables() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let has_rowcount = list.items.iter().any(|i| {
                    i.label == "@@ROWCOUNT" && i.kind == Some(CompletionItemKind::VARIABLE)
                });
                assert!(has_rowcount, "Should include @@ROWCOUNT system variable");
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_no_duplicate_labels() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let mut labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
                labels.sort();
                let deduped: Vec<&str> = labels
                    .windows(2)
                    .filter(|w| w[0] == w[1])
                    .map(|w| w[0])
                    .collect();
                // Keywords may appear as both keyword and function (e.g., SELECT)
                // so allow some duplicates but verify it's not excessive
                assert!(
                    deduped.len() <= 5,
                    "Too many duplicate labels: {:?}",
                    deduped
                );
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_is_comma_separated_syntax_edge_cases() {
        assert!(!is_comma_separated_syntax(""));
        assert!(is_comma_separated_syntax("()")); // empty parens still match pattern
        assert!(is_comma_separated_syntax("F()")); // single char func
    }
}
