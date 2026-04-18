//! Completion 生成
//!
//! SQL キーワード、データ型、組み込み関数の補完候補を提供する。

use lsp_types::{CompletionItem, CompletionItemKind, CompletionList, CompletionResponse};

/// 関数名とパラメータリストからLSP snippet形式のinsert_textを生成する
///
/// `DocEntry.params`（クリーンなパラメータ名配列）を直接使用し、
/// syntax文字列のブラケット表記（`[, style]`等）による問題を回避する。
///
/// # Examples
/// - `build_function_snippet("SUBSTRING", &["expression", "start", "length"])`
///   → `SUBSTRING(${1:expression}, ${2:start}, ${3:length})`
/// - `build_function_snippet("GETDATE", &[])` → `GETDATE()`
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

/// 全ての補完候補を返す（MVP: コンテキスト非依存）
pub fn complete_all() -> CompletionResponse {
    let mut items = Vec::new();

    // Keywords from db_docs
    for entry in crate::db_docs::keywords() {
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("T-SQL Keyword".to_string()),
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

    // Functions from db_docs — snippet format with parameter placeholders
    for entry in crate::db_docs::functions() {
        let snippet = build_function_snippet(entry.name, entry.params);
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!("{} — {}", entry.syntax, entry.description)),
            insert_text: Some(snippet),
            insert_text_format: Some(lsp_types::InsertTextFormat::SNIPPET),
            ..CompletionItem::default()
        });
    }

    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

/// キーワード補完のみを返す
pub fn complete_keywords() -> CompletionResponse {
    let items = crate::db_docs::keywords()
        .iter()
        .map(|entry| CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("T-SQL Keyword".to_string()),
            ..CompletionItem::default()
        })
        .collect();

    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
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
        assert!(!result.contains('['), "No brackets should appear in snippet");
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
}
