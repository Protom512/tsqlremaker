//! Workspace Symbols provider
//!
//! ワークスペース全体からシンボルを検索する。
//! テーブル、プロシージャ、ビュー、インデックス、変数を
//! クエリ文字列でフィルタリングする。

use crate::analysis::DocumentAnalysis;
use crate::symbol_store::SymbolStore;
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

/// Cross-file [`SymbolStore`] からクエリにマッチするシンボルを検索する。
///
/// 逆引きマップ (`name → Vec<SymbolEntry>`) を列挙し、各エントリのカテゴリ別
/// `SymbolKind` と `container_name` はストア構築時点で確定済みのため、ここでは
/// クエリの大文字小文字を問わない部分一致フィルタのみを行う。
///
/// # 契約
///
/// - 空クエリ (`""`) → 空の `Vec` （[`workspace_symbols_with_analysis`] と同一）。
/// - 一致判定は `name.to_uppercase().contains(query.to_uppercase())`。ストア側の
///   キーは既に大文字正規化済みなので、クエリ側のみ `.to_uppercase()` する。
///
/// # フォールバック戦略
///
/// 単一ファイルモード (`workspace_folders` なし) ではストアが空になり得るため、
/// 呼び出し元は [`workspace_symbols_with_analysis`] をフォールバックとして併用する
/// こと（estimate 条件: single-file mode の下位互換性）。
#[must_use]
pub fn workspace_symbols_with_store(store: &SymbolStore, query: &str) -> Vec<SymbolInformation> {
    if query.is_empty() {
        return Vec::new();
    }
    let query_upper = query.to_uppercase();
    let mut results = Vec::new();
    for (key, entries) in store.iter() {
        if !key.as_str().contains(query_upper.as_str()) {
            continue;
        }
        for entry in entries {
            // SymbolInformation.deprecated field is #[deprecated] in lsp-types 0.94.
            // tower-lsp 0.20 requires this type — cannot migrate until tower-lsp upgrades.
            #[allow(deprecated)]
            results.push(SymbolInformation {
                // Preserve the original source casing (entry.name), NOT the
                // upper-normalized lookup key, so results read naturally to the
                // user (e.g. "users", not "USERS"). Case-insensitive query
                // matching is already handled by the key.contains() above.
                name: entry.name.clone(),
                kind: entry.kind,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: entry.uri.clone(),
                    range: entry.range,
                },
                container_name: entry.container_name.clone(),
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

    // === workspace_symbols_with_store tests ===

    use crate::symbol_store::{DocumentSource, SymbolStore};

    fn store_with(uri_str: &str, source: &str, origin: DocumentSource) -> SymbolStore {
        let mut store = SymbolStore::new();
        let u = Url::parse(uri_str).unwrap();
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        store.upsert(&u, &analysis, origin);
        store
    }

    #[test]
    fn test_store_based_table_symbol() {
        let store = store_with(
            "file:///test.sql",
            "CREATE TABLE users (id INT)",
            DocumentSource::Open,
        );
        let results = workspace_symbols_with_store(&store, "user");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "users");
        assert_eq!(results[0].kind, SymbolKind::CLASS);
        assert_eq!(results[0].container_name, None);
        assert_eq!(
            results[0].location.uri,
            Url::parse("file:///test.sql").unwrap()
        );
    }

    #[test]
    fn test_store_based_procedure_symbol() {
        let store = store_with(
            "file:///test.sql",
            "CREATE PROCEDURE get_users AS BEGIN SELECT 1 END",
            DocumentSource::Open,
        );
        let results = workspace_symbols_with_store(&store, "get");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "get_users");
        assert_eq!(results[0].kind, SymbolKind::FUNCTION);
    }

    #[test]
    fn test_store_based_view_symbol() {
        let store = store_with(
            "file:///test.sql",
            "CREATE VIEW active_users AS SELECT * FROM users",
            DocumentSource::Open,
        );
        let results = workspace_symbols_with_store(&store, "active");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "active_users");
        assert_eq!(results[0].kind, SymbolKind::INTERFACE);
    }

    #[test]
    fn test_store_based_index_symbol_preserves_container() {
        let store = store_with(
            "file:///test.sql",
            "CREATE TABLE t (id INT)\nCREATE INDEX idx_t ON t (id)",
            DocumentSource::Open,
        );
        let results = workspace_symbols_with_store(&store, "idx");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "idx_t");
        assert_eq!(results[0].kind, SymbolKind::PROPERTY);
        assert_eq!(
            results[0].container_name,
            Some("t".to_string()),
            "Index should carry table name as container"
        );
    }

    #[test]
    fn test_store_based_trigger_symbol_preserves_container() {
        let store = store_with(
            "file:///test.sql",
            "CREATE TRIGGER tr_audit ON users FOR INSERT AS BEGIN SELECT 1 END",
            DocumentSource::Open,
        );
        let results = workspace_symbols_with_store(&store, "audit");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "tr_audit");
        assert_eq!(results[0].kind, SymbolKind::EVENT);
        assert_eq!(
            results[0].container_name,
            Some("users".to_string()),
            "Trigger should carry table name as container"
        );
    }

    #[test]
    fn test_store_based_variable_symbol() {
        let store = store_with(
            "file:///test.sql",
            "DECLARE @total_count INT",
            DocumentSource::Open,
        );
        let results = workspace_symbols_with_store(&store, "total");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "@total_count");
        assert_eq!(results[0].kind, SymbolKind::VARIABLE);
    }

    #[test]
    fn test_store_based_empty_query_returns_empty() {
        let store = store_with(
            "file:///test.sql",
            "CREATE TABLE users (id INT)",
            DocumentSource::Open,
        );
        let results = workspace_symbols_with_store(&store, "");
        assert!(
            results.is_empty(),
            "empty query must always return Vec::new"
        );
    }

    #[test]
    fn test_store_based_no_match_returns_empty() {
        let store = store_with(
            "file:///test.sql",
            "CREATE TABLE users (id INT)",
            DocumentSource::Open,
        );
        let results = workspace_symbols_with_store(&store, "orders");
        assert!(results.is_empty());
    }

    #[test]
    fn test_store_based_case_insensitive_query() {
        let store = store_with(
            "file:///test.sql",
            "CREATE TABLE users (id INT)",
            DocumentSource::Open,
        );
        assert_eq!(workspace_symbols_with_store(&store, "USERS").len(), 1);
        assert_eq!(workspace_symbols_with_store(&store, "Users").len(), 1);
        assert_eq!(workspace_symbols_with_store(&store, "users").len(), 1);
    }

    #[test]
    fn test_store_based_cross_file_aggregation() {
        // Same name defined in two files → both must appear in results.
        let mut store = SymbolStore::new();
        let ua = Url::parse("file:///a.sql").unwrap();
        let ub = Url::parse("file:///b.sql").unwrap();
        let analysis = crate::analysis::DocumentAnalysis::new("CREATE TABLE users (id INT)");
        store.upsert(&ua, &analysis, DocumentSource::Open);
        store.upsert(&ub, &analysis, DocumentSource::Open);

        let results = workspace_symbols_with_store(&store, "user");
        assert_eq!(results.len(), 2);
        let uris: Vec<&Url> = results.iter().map(|r| &r.location.uri).collect();
        assert!(uris.contains(&&ua));
        assert!(uris.contains(&&ub));
    }

    #[test]
    fn test_store_based_empty_store_returns_empty() {
        let store = SymbolStore::new();
        assert!(workspace_symbols_with_store(&store, "anything").is_empty());
    }

    #[test]
    fn test_store_based_partial_match_substring() {
        let store = store_with(
            "file:///test.sql",
            "CREATE TABLE user_orders (id INT)\nCREATE TABLE user_items (id INT)",
            DocumentSource::Open,
        );
        let results = workspace_symbols_with_store(&store, "user");
        assert_eq!(results.len(), 2);
    }
}
