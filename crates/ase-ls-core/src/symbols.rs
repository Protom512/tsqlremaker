//! Document Symbols 生成
//!
//! AST の文から LSP DocumentSymbol / SymbolInformation を生成する。

use crate::offset_to_position;
use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolKind};
use tsql_parser::ast::{Statement, TableReference};

/// ソースコードから Document Symbols を生成する
pub fn document_symbols(source: &str) -> Option<DocumentSymbolResponse> {
    let mut parser = tsql_parser::Parser::new(source);
    let statements = match parser.parse() {
        Ok(s) => s,
        Err(_) => return None,
    };

    let symbols: Vec<DocumentSymbol> = statements
        .iter()
        .filter_map(|stmt| statement_to_symbol(source, stmt))
        .collect();

    if symbols.is_empty() {
        None
    } else {
        Some(DocumentSymbolResponse::Nested(symbols))
    }
}

/// Statement から DocumentSymbol への変換
fn statement_to_symbol(source: &str, stmt: &Statement) -> Option<DocumentSymbol> {
    match stmt {
        Statement::Create(create) => {
            let (name, kind, span) = match create.as_ref() {
                tsql_parser::ast::CreateStatement::Table(td) => {
                    (td.name.name.clone(), SymbolKind::CLASS, td.span)
                }
                tsql_parser::ast::CreateStatement::Procedure(pd) => {
                    (pd.name.name.clone(), SymbolKind::FUNCTION, pd.span)
                }
                tsql_parser::ast::CreateStatement::View(vd) => {
                    (vd.name.name.clone(), SymbolKind::INTERFACE, vd.span)
                }
                tsql_parser::ast::CreateStatement::Index(idx) => {
                    (idx.name.name.clone(), SymbolKind::PROPERTY, idx.span)
                }
            };
            let range = span_to_lsp_range(source, span.start, span.end);
            Some(make_symbol(name, kind, range))
        }
        Statement::Declare(decl) => {
            let names: Vec<&str> = decl
                .variables
                .iter()
                .map(|v| v.name.name.as_str())
                .collect();
            let name = format!("DECLARE {}", names.join(", "));
            let range = span_to_lsp_range(source, decl.span.start, decl.span.end);
            Some(make_symbol(name, SymbolKind::VARIABLE, range))
        }
        Statement::Select(sel) => {
            let range = span_to_lsp_range(source, sel.span.start, sel.span.end);
            Some(make_symbol(
                "SELECT".to_string(),
                SymbolKind::NAMESPACE,
                range,
            ))
        }
        Statement::Insert(ins) => {
            let name = format!("INSERT {}", ins.table.name);
            let range = span_to_lsp_range(source, ins.span.start, ins.span.end);
            Some(make_symbol(name, SymbolKind::NAMESPACE, range))
        }
        Statement::Update(upd) => {
            let table_name = table_ref_name(&upd.table);
            let name = format!("UPDATE {table_name}");
            let range = span_to_lsp_range(source, upd.span.start, upd.span.end);
            Some(make_symbol(name, SymbolKind::NAMESPACE, range))
        }
        Statement::Delete(del) => {
            let name = format!("DELETE FROM {}", del.table.name);
            let range = span_to_lsp_range(source, del.span.start, del.span.end);
            Some(make_symbol(name, SymbolKind::NAMESPACE, range))
        }
        _ => None,
    }
}

/// TableReference から表示名を取得
fn table_ref_name(tr: &TableReference) -> String {
    match tr {
        TableReference::Table { name, .. } => name.name.clone(),
        TableReference::Subquery { alias, .. } => {
            alias.as_ref().map(|a| a.name.clone()).unwrap_or_default()
        }
        TableReference::Joined { .. } => String::new(),
    }
}

/// DocumentSymbol を構築するヘルパー
#[allow(deprecated)]
fn make_symbol(name: String, kind: SymbolKind, range: lsp_types::Range) -> DocumentSymbol {
    DocumentSymbol {
        name,
        kind,
        range,
        selection_range: range,
        children: None,
        detail: None,
        tags: None,
        deprecated: None,
    }
}

/// バイトオフセット範囲から LSP Range を生成
fn span_to_lsp_range(source: &str, start: u32, end: u32) -> lsp_types::Range {
    let (start_line, start_char) = offset_to_position(source, start);
    let (end_line, end_char) = offset_to_position(source, end);
    lsp_types::Range {
        start: lsp_types::Position {
            line: start_line,
            character: start_char,
        },
        end: lsp_types::Position {
            line: end_line,
            character: end_char,
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_create_table_symbol() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))";
        let result = document_symbols(source);
        assert!(result.is_some());
        if let Some(DocumentSymbolResponse::Nested(symbols)) = result {
            assert_eq!(symbols.len(), 1);
            assert_eq!(symbols[0].name, "users");
            assert_eq!(symbols[0].kind, SymbolKind::CLASS);
        }
    }

    #[test]
    fn test_select_symbol() {
        let source = "SELECT * FROM users";
        let result = document_symbols(source);
        assert!(result.is_some());
        if let Some(DocumentSymbolResponse::Nested(symbols)) = result {
            assert_eq!(symbols.len(), 1);
            assert_eq!(symbols[0].name, "SELECT");
        }
    }

    #[test]
    fn test_multiple_symbols() {
        let source = "CREATE TABLE t1 (id INT); SELECT * FROM t1";
        let result = document_symbols(source);
        assert!(result.is_some());
        if let Some(DocumentSymbolResponse::Nested(symbols)) = result {
            assert_eq!(symbols.len(), 2);
        }
    }

    #[test]
    fn test_no_symbols_for_invalid() {
        let source = "INVALID SQL !!!";
        let result = document_symbols(source);
        assert!(
            result.is_none()
                || matches!(result, Some(DocumentSymbolResponse::Nested(s)) if s.is_empty())
        );
    }
}
