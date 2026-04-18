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

/// syntax文字列がカンマ区切りの括弧構文かどうかを判定する
///
/// カンマ区切りではない関数（`CAST(expr AS type)`等）や
/// 括弧なしの関数（`IDENTITY`等）はsnippetプレースホルダー生成に
/// 適さないためfalseを返す。
fn is_comma_separated_syntax(syntax: &str) -> bool {
    if let (Some(open), Some(close)) = (syntax.find('('), syntax.rfind(')')) {
        if open < close {
            let inner = &syntax[open + 1..close];
            return !inner.contains(" AS ") && !inner.contains('\'') && !inner.contains('|');
        }
    }
    false
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
}
