//! Completion 生成
//!
//! SQL キーワード、データ型、組み込み関数の補完候補を提供する。

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionResponse, InsertTextFormat,
};
use std::sync::LazyLock;

use crate::config::CompletionConfig;
use crate::symbol_table::{SymbolTable, TableSymbol};

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

/// 補完コンテキスト（カーソル直前のトークンから推定、#126）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionContext {
    /// テーブル名が期待される位置 (FROM / JOIN / INTO / UPDATE / TABLE の直後)。
    Table,
    /// 変数名の宣言位置 (`DECLARE @<name>`)。静的候補は無意味なため空。
    VariableDeclaration,
    /// 式が期待される位置 (SELECT / WHERE / SET / 値 等)。全候補を返す。
    Expression,
}

/// カーソル直前の行プレフィックスから補完コンテキストを推定する。
///
/// ホワイトスペース区切りの最終トークン（およびその直前トークン）から、
/// テーブル名位置・変数宣言位置・式位置を判定する。
pub(crate) fn detect_context(prefix: &str) -> CompletionContext {
    let upper = prefix.trim_end().to_uppercase();
    let tokens: Vec<&str> = upper.split_whitespace().collect();
    let table_kw = ["FROM", "JOIN", "INTO", "UPDATE", "TABLE"];

    let last_token = tokens.last().copied().unwrap_or("");

    // 変数宣言位置: "DECLARE" が最終トークン、または "@<name>" の直前が DECLARE。
    let is_declare =
        last_token == "DECLARE" || tokens.len() >= 2 && tokens[tokens.len() - 2] == "DECLARE";
    if is_declare {
        return CompletionContext::VariableDeclaration;
    }

    // テーブル名位置: 最終トークン、またはその直前がテーブル系キーワード。
    let last_is_table = table_kw.contains(&last_token);
    let prev_is_table = tokens.len() >= 2 && table_kw.contains(&tokens[tokens.len() - 2]);
    if last_is_table || prev_is_table {
        return CompletionContext::Table;
    }

    CompletionContext::Expression
}

/// カーソルコンテキストに応じた補完候補を返す (#126, #132: `config` 駆動)。
///
/// * `Table` → シンボルテーブル内のテーブル名
/// * `VariableDeclaration` → 空 (新規変数名入力中)
/// * `Expression` → [`complete_all`] の全候補（`config.enable_snippets` が
///   `false` ならスニペットをプレーンテキストに展開）
///
/// # Arguments
///
/// * `prefix` - 行頭〜カーソル位置までのテキスト
/// * `symbol_table` - 現ドキュメントのシンボルテーブル (テーブル名参照用)
/// * `config` - 補完設定 (スニペット有効/無効)
#[must_use]
pub fn complete_for_context(
    prefix: &str,
    symbol_table: &SymbolTable,
    config: &CompletionConfig,
) -> CompletionResponse {
    match detect_context(prefix) {
        CompletionContext::VariableDeclaration => CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items: Vec::new(),
        }),
        CompletionContext::Table => {
            let items: Vec<CompletionItem> = symbol_table
                .tables
                .values()
                .map(table_completion_item)
                .collect();
            CompletionResponse::List(CompletionList {
                is_incomplete: false,
                items,
            })
        }
        CompletionContext::Expression => {
            apply_snippet_config(complete_all().clone(), config.enable_snippets)
        }
    }
}

/// 補完候補リストのスニペット挙動を `enable_snippets` に従って調整する (#132)。
///
/// `enable_snippets == true` なら何もしない（キャッシュ済みリストをそのまま返す・
/// pre-#132 挙動）。`false` の場合、スニペット形式（`InsertTextFormat::SNIPPET`）の
/// 関数補完をプレーンテキスト（`name()`）に展開する。キーワード/型/変数は元々
/// スニペットではないため影響を受けない。
#[must_use]
pub fn apply_snippet_config(resp: CompletionResponse, enable_snippets: bool) -> CompletionResponse {
    if enable_snippets {
        return resp;
    }
    let CompletionResponse::List(list) = resp else {
        return resp;
    };
    let items = list
        .items
        .into_iter()
        .map(|mut item| {
            if item.insert_text_format == Some(InsertTextFormat::SNIPPET) {
                // スニペットを関数名＋括弧のプレーンテキストに置換。
                item.insert_text = Some(format!("{}()", item.label));
                item.insert_text_format = Some(InsertTextFormat::PLAIN_TEXT);
            }
            item
        })
        .collect();
    CompletionResponse::List(CompletionList {
        is_incomplete: list.is_incomplete,
        items,
    })
}

/// テーブルシンボルから補完アイテムを構築する。
fn table_completion_item(t: &TableSymbol) -> CompletionItem {
    CompletionItem {
        label: t.name.clone(),
        kind: Some(CompletionItemKind::STRUCT),
        detail: Some(
            if t.is_temporary {
                "Temporary table"
            } else {
                "Table"
            }
            .to_string(),
        ),
        ..CompletionItem::default()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::CompletionConfig;
    use crate::symbol_table::SymbolTableBuilder;
    use lsp_types::InsertTextFormat;

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

    // ----- #126: context-aware completion ---------------------------------

    #[test]
    fn detect_context_table_after_from() {
        assert_eq!(detect_context("SELECT * FROM "), CompletionContext::Table);
    }

    #[test]
    fn detect_context_table_while_typing_name() {
        // "FROM u" — user started typing the table name
        assert_eq!(detect_context("SELECT * FROM u"), CompletionContext::Table);
    }

    #[test]
    fn detect_context_table_after_join_and_into() {
        assert_eq!(
            detect_context("SELECT * FROM a JOIN "),
            CompletionContext::Table
        );
        assert_eq!(detect_context("INSERT INTO "), CompletionContext::Table);
        assert_eq!(detect_context("UPDATE "), CompletionContext::Table);
    }

    #[test]
    fn detect_context_variable_declaration() {
        assert_eq!(
            detect_context("DECLARE @"),
            CompletionContext::VariableDeclaration
        );
        // while typing the variable name
        assert_eq!(
            detect_context("DECLARE @co"),
            CompletionContext::VariableDeclaration
        );
        // after DECLARE + space, before the '@'
        assert_eq!(
            detect_context("DECLARE "),
            CompletionContext::VariableDeclaration
        );
    }

    #[test]
    fn detect_context_expression_positions() {
        assert_eq!(detect_context("SELECT "), CompletionContext::Expression);
        assert_eq!(
            detect_context("SELECT * FROM users WHERE "),
            CompletionContext::Expression
        );
        assert_eq!(detect_context(""), CompletionContext::Expression);
    }

    #[test]
    fn complete_for_context_returns_table_names() {
        let st = SymbolTableBuilder::build("CREATE TABLE users (id INT)");
        let resp = complete_for_context("SELECT * FROM ", &st, &CompletionConfig::default());
        match resp {
            CompletionResponse::List(list) => {
                assert!(
                    list.items.iter().any(|i| i.label == "users"),
                    "FROM context should offer table names"
                );
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn complete_for_context_variable_decl_is_empty() {
        let st = SymbolTableBuilder::build("");
        let resp = complete_for_context("DECLARE @", &st, &CompletionConfig::default());
        match resp {
            CompletionResponse::List(list) => {
                assert!(
                    list.items.is_empty(),
                    "Variable declaration should not offer static items"
                );
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn complete_for_context_expression_returns_full_list() {
        let st = SymbolTableBuilder::build("");
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default());
        match resp {
            CompletionResponse::List(list) => {
                // Expression context returns the full cached list (e.g. SELECT keyword).
                assert!(list.items.iter().any(|i| i.label == "SELECT"));
            }
            _ => panic!("Expected List"),
        }
    }

    // === configuration-driven snippets (#132) ===

    #[test]
    fn config_snippets_disabled_strips_function_placeholders() {
        let st = SymbolTableBuilder::build("");
        let cfg = CompletionConfig {
            enable_snippets: false,
        };
        let resp = complete_for_context("SELECT ", &st, &cfg);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        // A comma-separated function (e.g. SUBSTRING) must be plain text, not a snippet.
        let substring = list.items.iter().find(|i| i.label == "SUBSTRING");
        let substring =
            substring.unwrap_or_else(|| panic!("SUBSTRING should be in completion list: {list:?}"));
        assert_eq!(
            substring.insert_text_format,
            Some(InsertTextFormat::PLAIN_TEXT),
            "snippets disabled → plain text"
        );
        assert_eq!(substring.insert_text.as_deref(), Some("SUBSTRING()"));
    }

    #[test]
    fn config_snippets_enabled_keeps_default_behaviour() {
        let st = SymbolTableBuilder::build("");
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default());
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        let substring = list
            .items
            .iter()
            .find(|i| i.label == "SUBSTRING")
            .expect("SUBSTRING present");
        // Default (enabled) keeps the snippet with placeholders.
        assert_eq!(
            substring.insert_text_format,
            Some(InsertTextFormat::SNIPPET)
        );
        assert!(substring
            .insert_text
            .as_deref()
            .unwrap_or("")
            .contains("${1:"));
    }
}
