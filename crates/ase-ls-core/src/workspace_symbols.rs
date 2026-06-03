//! Workspace Symbols provider
//!
//! ワークスペース全体からシンボルを検索する。
//! テーブル、プロシージャ、ビュー、インデックス、変数を
//! クエリ文字列でフィルタリングする。

use crate::analysis::DocumentAnalysis;
use crate::symbol_table::SymbolTableBuilder;
use lsp_types::{Location, SymbolInformation, SymbolKind, Url};

/// Append symbols whose name matches `query_upper` (case-insensitive substring).
#[allow(deprecated)]
fn push_matching(
    results: &mut Vec<SymbolInformation>,
    symbols: impl Iterator<Item = (String, lsp_types::Range, Option<String>)>,
    query_upper: &str,
    uri: &Url,
    kind: SymbolKind,
) {
    for (name, range, container_name) in symbols {
        if name.to_uppercase().contains(query_upper) {
            results.push(SymbolInformation {
                name,
                kind,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: uri.clone(),
                    range,
                },
                container_name,
            });
        }
    }
}

/// Collect all matching symbols from a symbol table into results.
fn collect_symbols(
    table: &crate::symbol_table::SymbolTable,
    query_upper: &str,
    uri: &Url,
) -> Vec<SymbolInformation> {
    let mut results = Vec::new();

    push_matching(
        &mut results,
        table
            .tables
            .values()
            .map(|s| (s.name.clone(), s.range, None)),
        query_upper,
        uri,
        SymbolKind::CLASS,
    );

    push_matching(
        &mut results,
        table
            .procedures
            .values()
            .map(|s| (s.name.clone(), s.range, None)),
        query_upper,
        uri,
        SymbolKind::FUNCTION,
    );

    push_matching(
        &mut results,
        table
            .views
            .values()
            .map(|s| (s.name.clone(), s.range, None)),
        query_upper,
        uri,
        SymbolKind::INTERFACE,
    );

    push_matching(
        &mut results,
        table
            .indexes
            .values()
            .map(|s| (s.name.clone(), s.range, Some(s.table_name.clone()))),
        query_upper,
        uri,
        SymbolKind::PROPERTY,
    );

    push_matching(
        &mut results,
        table
            .variables
            .values()
            .map(|s| (s.name.clone(), s.range, None)),
        query_upper,
        uri,
        SymbolKind::VARIABLE,
    );

    push_matching(
        &mut results,
        table
            .triggers
            .values()
            .map(|s| (s.name.clone(), s.range, Some(s.table_name.clone()))),
        query_upper,
        uri,
        SymbolKind::EVENT,
    );

    results
}

/// DocumentAnalysisからクエリにマッチするシンボルを検索する
pub fn workspace_symbols_with_analysis(
    analysis: &DocumentAnalysis,
    query: &str,
    uri: &Url,
) -> Vec<SymbolInformation> {
    if query.is_empty() {
        return Vec::new();
    }
    collect_symbols(&analysis.symbol_table, &query.to_uppercase(), uri)
}

/// ソースコードからクエリにマッチするシンボルを検索する（ソースから構築）
///
/// 大文字小文字を区別しない部分一致でフィルタリングする。
pub fn workspace_symbols(source: &str, query: &str, uri: &Url) -> Vec<SymbolInformation> {
    if query.is_empty() {
        return Vec::new();
    }
    let table = SymbolTableBuilder::build_tolerant(source);
    collect_symbols(&table, &query.to_uppercase(), uri)
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

    // === workspace_symbols_with_analysis tests ===

    #[test]
    fn test_analysis_based_table_symbol() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let results = workspace_symbols_with_analysis(&analysis, "user", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "users");
        assert_eq!(results[0].kind, SymbolKind::CLASS);
    }

    #[test]
    fn test_analysis_based_procedure_symbol() {
        let source = "CREATE PROCEDURE get_users AS BEGIN SELECT 1 END";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let results = workspace_symbols_with_analysis(&analysis, "get", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "get_users");
        assert_eq!(results[0].kind, SymbolKind::FUNCTION);
    }

    #[test]
    fn test_analysis_based_view_symbol() {
        let source = "CREATE VIEW active_users AS SELECT * FROM users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let results = workspace_symbols_with_analysis(&analysis, "active", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "active_users");
        assert_eq!(results[0].kind, SymbolKind::INTERFACE);
    }

    #[test]
    fn test_analysis_based_index_symbol() {
        let source = "CREATE TABLE t (id INT)\nCREATE INDEX idx_t ON t (id)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let results = workspace_symbols_with_analysis(&analysis, "idx", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "idx_t");
        assert_eq!(results[0].kind, SymbolKind::PROPERTY);
        assert_eq!(
            results[0].container_name,
            Some("t".to_string()),
            "Index should have table name as container"
        );
    }

    #[test]
    fn test_analysis_based_variable_symbol() {
        let source = "DECLARE @total_count INT";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let results = workspace_symbols_with_analysis(&analysis, "total", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "@total_count");
        assert_eq!(results[0].kind, SymbolKind::VARIABLE);
    }

    #[test]
    fn test_analysis_based_empty_query() {
        let source = "CREATE TABLE users (id INT)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let results = workspace_symbols_with_analysis(&analysis, "", &test_uri());
        assert!(results.is_empty());
    }

    #[test]
    fn test_analysis_based_no_match() {
        let source = "CREATE TABLE users (id INT)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let results = workspace_symbols_with_analysis(&analysis, "orders", &test_uri());
        assert!(results.is_empty());
    }

    #[test]
    fn test_analysis_based_trigger_symbol() {
        let source = "CREATE TRIGGER tr_audit ON users FOR INSERT AS BEGIN SELECT 1 END";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let results = workspace_symbols_with_analysis(&analysis, "audit", &test_uri());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "tr_audit");
        assert_eq!(results[0].kind, SymbolKind::EVENT);
        assert_eq!(
            results[0].container_name,
            Some("users".to_string()),
            "Trigger should have table name as container"
        );
    }
}
