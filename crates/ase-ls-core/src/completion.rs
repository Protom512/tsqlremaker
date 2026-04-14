//! Completion 生成
//!
//! SQL キーワード、データ型、組み込み関数の補完候補を提供する。

use lsp_types::{CompletionItem, CompletionItemKind, CompletionList, CompletionResponse};
use once_cell::sync::Lazy;

/// キーワード補完データ
static KEYWORD_COMPLETIONS: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    let keywords = [
        "SELECT",
        "FROM",
        "WHERE",
        "INSERT",
        "INTO",
        "VALUES",
        "UPDATE",
        "SET",
        "DELETE",
        "CREATE",
        "ALTER",
        "DROP",
        "TABLE",
        "INDEX",
        "VIEW",
        "PROCEDURE",
        "FUNCTION",
        "TRIGGER",
        "DATABASE",
        "SCHEMA",
        "IF",
        "ELSE",
        "BEGIN",
        "END",
        "WHILE",
        "RETURN",
        "BREAK",
        "CONTINUE",
        "DECLARE",
        "EXEC",
        "EXECUTE",
        "JOIN",
        "INNER",
        "OUTER",
        "LEFT",
        "RIGHT",
        "FULL",
        "CROSS",
        "ON",
        "AND",
        "OR",
        "NOT",
        "IN",
        "EXISTS",
        "BETWEEN",
        "LIKE",
        "IS",
        "NULL",
        "ORDER",
        "BY",
        "ASC",
        "DESC",
        "GROUP",
        "HAVING",
        "UNION",
        "DISTINCT",
        "ALL",
        "TOP",
        "CASE",
        "WHEN",
        "THEN",
        "AS",
        "PRIMARY",
        "FOREIGN",
        "KEY",
        "REFERENCES",
        "UNIQUE",
        "CHECK",
        "DEFAULT",
        "IDENTITY",
        "CONSTRAINT",
        "COMMIT",
        "ROLLBACK",
        "TRANSACTION",
        "GRANT",
        "REVOKE",
        "DENY",
        "TRY",
        "CATCH",
        "THROW",
        "PRINT",
        "GO",
    ];

    keywords
        .iter()
        .map(|kw| CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("T-SQL Keyword".to_string()),
            ..CompletionItem::default()
        })
        .collect()
});

/// データ型補完データ
static DATATYPE_COMPLETIONS: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    let types = [
        ("INT", "Integer (-2^31 to 2^31-1)"),
        ("INTEGER", "Integer (same as INT)"),
        ("SMALLINT", "Small integer (-32768 to 32767)"),
        ("TINYINT", "Tiny integer (0 to 255)"),
        ("BIGINT", "Big integer (-2^63 to 2^63-1)"),
        ("REAL", "Floating point (4 bytes)"),
        ("DOUBLE", "Double precision (8 bytes)"),
        ("DECIMAL", "Decimal (exact numeric)"),
        ("NUMERIC", "Numeric (same as DECIMAL)"),
        ("MONEY", "Monetary value (8 bytes)"),
        ("SMALLMONEY", "Small monetary (4 bytes)"),
        ("CHAR", "Fixed-length character"),
        ("VARCHAR", "Variable-length character"),
        ("TEXT", "Variable-length text"),
        ("NCHAR", "Fixed-length Unicode character"),
        ("NVARCHAR", "Variable-length Unicode character"),
        ("UNICHAR", "Fixed-length Unicode (ASE specific)"),
        ("UNIVARCHAR", "Variable-length Unicode (ASE specific)"),
        ("BINARY", "Fixed-length binary"),
        ("VARBINARY", "Variable-length binary"),
        ("DATE", "Date value"),
        ("TIME", "Time value"),
        ("DATETIME", "Date and time"),
        ("SMALLDATETIME", "Small date and time"),
        ("TIMESTAMP", "Timestamp"),
        ("BIGDATETIME", "Big date and time (ASE 15.7+)"),
        ("BIT", "Boolean (0 or 1)"),
        ("UNIQUEIDENTIFIER", "GUID/UUID"),
    ];

    types
        .iter()
        .map(|(name, desc)| CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some(desc.to_string()),
            ..CompletionItem::default()
        })
        .collect()
});

/// SAP ASE 組み込み関数補完データ
static FUNCTION_COMPLETIONS: Lazy<Vec<CompletionItem>> = Lazy::new(|| {
    let functions = [
        // 文字列関数
        (
            "SUBSTRING",
            "SUBSTRING(expr, start, length)",
            "Extract substring",
        ),
        ("CHAR_LENGTH", "CHAR_LENGTH(expr)", "String length"),
        ("LEN", "LEN(expr)", "String length"),
        ("UPPER", "UPPER(expr)", "Convert to uppercase"),
        ("LOWER", "LOWER(expr)", "Convert to lowercase"),
        ("LTRIM", "LTRIM(expr)", "Trim leading spaces"),
        ("RTRIM", "RTRIM(expr)", "Trim trailing spaces"),
        (
            "STR_REPLACE",
            "STR_REPLACE(src, pat, repl)",
            "Replace string",
        ),
        ("STUFF", "STUFF(src, start, len, ins)", "Delete and insert"),
        ("REPLICATE", "REPLICATE(expr, n)", "Repeat string"),
        ("SPACE", "SPACE(n)", "Generate spaces"),
        ("REVERSE", "REVERSE(expr)", "Reverse string"),
        ("RIGHT", "RIGHT(expr, n)", "Right substring"),
        ("LEFT", "LEFT(expr, n)", "Left substring"),
        ("CHARINDEX", "CHARINDEX(pat, expr)", "Find pattern position"),
        ("PATINDEX", "PATINDEX(pat, expr)", "Pattern index"),
        // 数値関数
        ("ABS", "ABS(expr)", "Absolute value"),
        ("CEILING", "CEILING(expr)", "Smallest integer >= value"),
        ("FLOOR", "FLOOR(expr)", "Largest integer <= value"),
        ("ROUND", "ROUND(expr, n)", "Round to n decimal places"),
        ("SQRT", "SQRT(expr)", "Square root"),
        ("POWER", "POWER(expr, n)", "Power"),
        ("SIGN", "SIGN(expr)", "Sign of value"),
        // 日付関数
        ("GETDATE", "GETDATE()", "Current datetime"),
        ("DATEADD", "DATEADD(unit, n, date)", "Add to date"),
        ("DATEDIFF", "DATEDIFF(unit, d1, d2)", "Date difference"),
        ("DATEPART", "DATEPART(unit, date)", "Extract date part"),
        ("DATENAME", "DATENAME(unit, date)", "Date part name"),
        ("DAY", "DAY(date)", "Day of month"),
        ("MONTH", "MONTH(date)", "Month number"),
        ("YEAR", "YEAR(date)", "Year"),
        // 変換関数
        ("CONVERT", "CONVERT(type, expr)", "Type conversion"),
        ("CAST", "CAST(expr AS type)", "Type conversion"),
        // 集約関数
        ("COUNT", "COUNT(expr)", "Count rows"),
        ("SUM", "SUM(expr)", "Sum values"),
        ("AVG", "AVG(expr)", "Average value"),
        ("MIN", "MIN(expr)", "Minimum value"),
        ("MAX", "MAX(expr)", "Maximum value"),
        // システム関数
        ("ISNULL", "ISNULL(expr, replacement)", "Replace NULL"),
        ("COALESCE", "COALESCE(e1, e2, ...)", "First non-NULL"),
        ("NULLIF", "NULLIF(e1, e2)", "NULL if equal"),
        ("IDENTITY", "IDENTITY", "Identity value"),
        ("@@IDENTITY", "@@IDENTITY", "Last identity value"),
        ("@@ROWCOUNT", "@@ROWCOUNT", "Rows affected"),
        ("@@ERROR", "@@ERROR", "Last error number"),
        ("@@TRANCOUNT", "@@TRANCOUNT", "Transaction count"),
        ("@@VERSION", "@@VERSION", "Server version"),
    ];

    functions
        .iter()
        .map(|(name, sig, desc)| CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!("{sig} — {desc}")),
            insert_text: Some(sig.to_string()),
            insert_text_format: Some(lsp_types::InsertTextFormat::PLAIN_TEXT),
            ..CompletionItem::default()
        })
        .collect()
});

/// 全ての補完候補を返す（MVP: コンテキスト非依存）
pub fn complete_all() -> CompletionResponse {
    let mut items = Vec::new();
    items.extend(KEYWORD_COMPLETIONS.iter().cloned());
    items.extend(DATATYPE_COMPLETIONS.iter().cloned());
    items.extend(FUNCTION_COMPLETIONS.iter().cloned());

    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

/// キーワード補完のみを返す
pub fn complete_keywords() -> CompletionResponse {
    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items: KEYWORD_COMPLETIONS.iter().cloned().collect(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

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
