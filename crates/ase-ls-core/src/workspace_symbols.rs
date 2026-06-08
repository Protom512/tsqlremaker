//! Workspace Symbols provider
//!
//! ワークスペース全体からシンボルを検索する。
//! テーブル、プロシージャ、ビュー、インデックス、変数を
//! クエリ文字列でフィルタリングする。

use crate::analysis::DocumentAnalysis;
use lsp_types::{Location, SymbolInformation, SymbolKind, Url};

/// Collect all matching symbols from a symbol table into results.
/// Uses pre-normalized upper keys from the HashMap to avoid per-symbol `.to_uppercase()` allocation.
// SymbolInformation.deprecated field is #[deprecated] in lsp-types 0.94.
// tower-lsp 0.20 requires this type — cannot migrate until tower-lsp upgrades.
#[allow(deprecated)]
fn collect_symbols(
    table: &crate::symbol_table::SymbolTable,
    query_upper: &str,
    uri: &Url,
) -> Vec<SymbolInformation> {
    let mut results = Vec::new();

    // Macro to avoid repeating the push pattern for each symbol category
    macro_rules! match_category {
        ($map:expr, $kind:expr, $container:expr) => {
            for (key, sym) in &$map {
                if key.as_str().contains(query_upper) {
                    results.push(SymbolInformation {
                        name: sym.name.clone(),
                        kind: $kind,
                        tags: None,
                        deprecated: None,
                        location: Location {
                            uri: uri.clone(),
                            range: sym.range,
                        },
                        container_name: $container(sym),
                    });
                }
            }
        };
    }

    match_category!(table.tables, SymbolKind::CLASS, |_| None);
    match_category!(table.procedures, SymbolKind::FUNCTION, |_| None);
    match_category!(table.views, SymbolKind::INTERFACE, |_| None);
    match_category!(
        table.indexes,
        SymbolKind::PROPERTY,
        |s: &crate::symbol_table::IndexSymbol| Some(s.table_name.clone())
    );
    match_category!(table.variables, SymbolKind::VARIABLE, |_| None);
    match_category!(
        table.triggers,
        SymbolKind::EVENT,
        |s: &crate::symbol_table::TriggerSymbol| Some(s.table_name.clone())
    );

    results
}

/// DocumentAnalysisからクエリにマッチするシンボルを検索する
#[must_use]
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn test_uri() -> Url {
        Url::parse("file:///test.sql").unwrap()
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
