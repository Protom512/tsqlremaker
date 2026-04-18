//! Workspace Symbols provider
//!
//! ワークスペース全体からシンボルを検索する。
//! テーブル、プロシージャ、ビュー、インデックス、変数を
//! クエリ文字列でフィルタリングする。

use crate::symbol_table::SymbolTableBuilder;
use lsp_types::{Location, SymbolInformation, SymbolKind, Url};

/// ソースコードからクエリにマッチするシンボルを検索する
///
/// 大文字小文字を区別しない部分一致でフィルタリングする。
#[allow(deprecated)]
pub fn workspace_symbols(source: &str, query: &str, uri: &Url) -> Vec<SymbolInformation> {
    if query.is_empty() {
        return Vec::new();
    }

    let table = SymbolTableBuilder::build_tolerant(source);
    let query_upper = query.to_uppercase();
    let mut results = Vec::new();

    // テーブルシンボル
    for sym in table.tables.values() {
        if sym.name.to_uppercase().contains(&query_upper) {
            results.push(SymbolInformation {
                name: sym.name.clone(),
                kind: SymbolKind::CLASS,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: sym.range,
                },
                container_name: None,
            });
        }
    }

    // プロシージャシンボル
    for sym in table.procedures.values() {
        if sym.name.to_uppercase().contains(&query_upper) {
            results.push(SymbolInformation {
                name: sym.name.clone(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: sym.range,
                },
                container_name: None,
            });
        }
    }

    // ビューシンボル
    for sym in table.views.values() {
        if sym.name.to_uppercase().contains(&query_upper) {
            results.push(SymbolInformation {
                name: sym.name.clone(),
                kind: SymbolKind::INTERFACE,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: sym.range,
                },
                container_name: None,
            });
        }
    }

    // インデックスシンボル
    for sym in table.indexes.values() {
        if sym.name.to_uppercase().contains(&query_upper) {
            results.push(SymbolInformation {
                name: sym.name.clone(),
                kind: SymbolKind::PROPERTY,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: sym.range,
                },
                container_name: Some(sym.table_name.clone()),
            });
        }
    }

    // 変数シンボル
    for sym in table.variables.values() {
        let display_name = sym.name.clone();
        if display_name.to_uppercase().contains(&query_upper) {
            results.push(SymbolInformation {
                name: display_name,
                kind: SymbolKind::VARIABLE,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range: sym.range,
                },
                container_name: None,
            });
        }
    }

    results
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

    fn matches_query(symbol_name: &str, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        symbol_name.to_uppercase().contains(&query.to_uppercase())
    }

    #[test]
    fn test_workspace_symbols_table() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))";
        let results = workspace_symbols(source, "user", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "users");
        assert_eq!(results[0].kind, SymbolKind::CLASS);
    }

    #[test]
    fn test_workspace_symbols_case_insensitive() {
        let source = "CREATE TABLE MyTable (id INT)";
        let results = workspace_symbols(source, "mytable", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "MyTable");
    }

    #[test]
    fn test_workspace_symbols_multiple_types() {
        let source = "\
            CREATE TABLE users (id INT)\n\
            CREATE PROCEDURE get_users AS BEGIN SELECT * FROM users END\n\
            CREATE VIEW active_users AS SELECT * FROM users\n\
            CREATE INDEX idx_users ON users (id)";
        let results = workspace_symbols(source, "user", &test_uri());
        // users table, get_users proc, active_users view, idx_users index
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_workspace_symbols_procedure() {
        let source = "CREATE PROCEDURE calculate_total @amount INT AS BEGIN RETURN @amount END";
        let results = workspace_symbols(source, "calc", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "calculate_total");
        assert_eq!(results[0].kind, SymbolKind::FUNCTION);
    }

    #[test]
    fn test_workspace_symbols_variable() {
        let source = "DECLARE @total_count INT\nDECLARE @user_name VARCHAR(50)";
        let results = workspace_symbols(source, "total", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "@total_count");
        assert_eq!(results[0].kind, SymbolKind::VARIABLE);
    }

    #[test]
    fn test_workspace_symbols_empty_query() {
        let source = "CREATE TABLE users (id INT)";
        let results = workspace_symbols(source, "", &test_uri());
        assert!(results.is_empty());
    }

    #[test]
    fn test_workspace_symbols_no_match() {
        let source = "CREATE TABLE users (id INT)";
        let results = workspace_symbols(source, "orders", &test_uri());
        assert!(results.is_empty());
    }

    #[test]
    fn test_workspace_symbols_index_with_container() {
        let source = "CREATE INDEX idx_name ON users (id)";
        let results = workspace_symbols(source, "idx", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].container_name, Some("users".to_string()));
    }

    #[test]
    fn test_matches_query() {
        assert!(matches_query("users", "user"));
        assert!(matches_query("USERS", "user"));
        assert!(matches_query("get_users", "user"));
        assert!(!matches_query("orders", "user"));
        assert!(matches_query("anything", ""));
    }
}
