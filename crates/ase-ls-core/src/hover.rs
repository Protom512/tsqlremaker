//! Hover 情報の提供
//!
//! T-SQL キーワード、データ型、組み込み関数、変数のホバー情報を提供する。

use crate::{offset_to_position, position_to_offset};
use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Range};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// キーワードドキュメント
static KEYWORD_DOCS: Lazy<HashMap<&str, (&str, &str)>> = Lazy::new(|| {
    let m: Vec<(&str, &str, &str)> = vec![
        (
            "SELECT",
            "Retrieves data from one or more tables",
            "SELECT [DISTINCT] col1, col2 FROM table WHERE condition",
        ),
        (
            "FROM",
            "Specifies the source tables for a query",
            "FROM table1 [AS alias] [JOIN table2 ON ...]",
        ),
        (
            "WHERE",
            "Filters rows based on a condition",
            "WHERE column = value AND column2 > value2",
        ),
        (
            "INSERT",
            "Inserts rows into a table",
            "INSERT INTO table (col1, col2) VALUES (val1, val2)",
        ),
        (
            "INTO",
            "Target specification for INSERT/SELECT INTO",
            "INSERT INTO table ...",
        ),
        (
            "UPDATE",
            "Modifies existing rows in a table",
            "UPDATE table SET col1 = val1 WHERE condition",
        ),
        (
            "DELETE",
            "Removes rows from a table",
            "DELETE FROM table WHERE condition",
        ),
        (
            "CREATE",
            "Creates a new database object",
            "CREATE TABLE | PROCEDURE | VIEW | INDEX ...",
        ),
        (
            "ALTER",
            "Modifies an existing database object",
            "ALTER TABLE ... | ALTER PROCEDURE ...",
        ),
        (
            "DROP",
            "Removes a database object",
            "DROP TABLE | PROCEDURE | VIEW | INDEX ...",
        ),
        (
            "TABLE",
            "Defines or references a table structure",
            "CREATE TABLE name (col1 TYPE, ...)",
        ),
        (
            "INDEX",
            "Creates an index on table columns",
            "CREATE [UNIQUE] [CLUSTERED] INDEX name ON table(col)",
        ),
        (
            "VIEW",
            "Creates a virtual table based on a query",
            "CREATE VIEW name AS SELECT ...",
        ),
        (
            "PROCEDURE",
            "Creates a stored procedure",
            "CREATE PROCEDURE name @param TYPE AS BEGIN ... END",
        ),
        (
            "IF",
            "Conditional execution",
            "IF condition BEGIN ... END ELSE BEGIN ... END",
        ),
        ("ELSE", "Alternative branch for IF", "IF ... ELSE ..."),
        ("BEGIN", "Starts a statement block", "BEGIN ... END"),
        ("END", "Ends a statement block", "BEGIN ... END"),
        ("WHILE", "Loop construct", "WHILE condition BEGIN ... END"),
        ("RETURN", "Exits a procedure or batch", "RETURN [value]"),
        (
            "DECLARE",
            "Declares a local variable",
            "DECLARE @var TYPE [, @var2 TYPE]",
        ),
        (
            "SET",
            "Assigns a value to a variable",
            "SET @var = expression",
        ),
        (
            "JOIN",
            "Combines rows from two tables",
            "[INNER|LEFT|RIGHT|FULL] JOIN table ON condition",
        ),
        (
            "INNER",
            "Specifies an inner join",
            "INNER JOIN table ON condition",
        ),
        (
            "LEFT",
            "Specifies a left outer join",
            "LEFT JOIN table ON condition",
        ),
        (
            "RIGHT",
            "Specifies a right outer join",
            "RIGHT JOIN table ON condition",
        ),
        (
            "ON",
            "Specifies join condition",
            "... JOIN table ON condition",
        ),
        (
            "AND",
            "Logical AND in conditions",
            "condition1 AND condition2",
        ),
        ("OR", "Logical OR in conditions", "condition1 OR condition2"),
        ("NOT", "Logical NOT", "NOT condition"),
        (
            "IN",
            "Checks if value is in a set",
            "column IN (val1, val2, ...)",
        ),
        (
            "EXISTS",
            "Checks if subquery returns rows",
            "EXISTS (SELECT ...)",
        ),
        ("BETWEEN", "Range check", "column BETWEEN val1 AND val2"),
        ("LIKE", "Pattern matching", "column LIKE 'pattern%'"),
        ("IS", "NULL check", "column IS [NOT] NULL"),
        (
            "NULL",
            "Represents missing or unknown data",
            "column IS NULL | column = NULL",
        ),
        (
            "ORDER",
            "Sorts query results",
            "ORDER BY col1 [ASC|DESC], col2",
        ),
        ("BY", "Used with ORDER, GROUP", "ORDER BY | GROUP BY"),
        (
            "GROUP",
            "Groups rows for aggregation",
            "GROUP BY col1, col2",
        ),
        (
            "HAVING",
            "Filters grouped results",
            "GROUP BY ... HAVING condition",
        ),
        (
            "UNION",
            "Combines result sets",
            "SELECT ... UNION [ALL] SELECT ...",
        ),
        (
            "DISTINCT",
            "Removes duplicate rows",
            "SELECT DISTINCT col1, col2 ...",
        ),
        (
            "CASE",
            "Conditional expression",
            "CASE WHEN cond THEN val ELSE val END",
        ),
        (
            "WHEN",
            "Condition branch in CASE",
            "CASE WHEN condition THEN result",
        ),
        ("THEN", "Result in CASE WHEN", "WHEN condition THEN result"),
        ("AS", "Creates an alias", "column AS alias | table AS alias"),
        (
            "PRIMARY",
            "Primary key constraint",
            "PRIMARY KEY (col1, col2)",
        ),
        (
            "FOREIGN",
            "Foreign key constraint",
            "FOREIGN KEY (col) REFERENCES table(col)",
        ),
        ("KEY", "Constraint keyword", "PRIMARY KEY | FOREIGN KEY"),
        (
            "REFERENCES",
            "Referenced table/column",
            "FOREIGN KEY (col) REFERENCES table(col)",
        ),
        ("UNIQUE", "Unique constraint", "UNIQUE (col1, col2)"),
        ("CHECK", "Check constraint", "CHECK (condition)"),
        (
            "DEFAULT",
            "Default value for column",
            "col TYPE DEFAULT value",
        ),
        ("IDENTITY", "Auto-increment column", "col INT IDENTITY(1,1)"),
        (
            "CONSTRAINT",
            "Named constraint",
            "CONSTRAINT name PRIMARY KEY (col)",
        ),
        (
            "TRY",
            "Starts error handling block",
            "BEGIN TRY ... END TRY BEGIN CATCH ... END CATCH",
        ),
        (
            "CATCH",
            "Catches errors in TRY block",
            "BEGIN TRY ... BEGIN CATCH ... END CATCH",
        ),
        (
            "THROW",
            "Raises an error",
            "THROW error_number, 'message', state",
        ),
        (
            "COMMIT",
            "Commits a transaction",
            "COMMIT [TRAN[SACTION] [name]]",
        ),
        (
            "ROLLBACK",
            "Rolls back a transaction",
            "ROLLBACK [TRAN[SACTION] [name]]",
        ),
        (
            "TRANSACTION",
            "Transaction management",
            "BEGIN TRANSACTION | COMMIT | ROLLBACK",
        ),
        (
            "GRANT",
            "Grants permissions",
            "GRANT SELECT ON table TO user",
        ),
        (
            "REVOKE",
            "Removes permissions",
            "REVOKE SELECT ON table FROM user",
        ),
        (
            "EXEC",
            "Executes a stored procedure",
            "EXEC [@ret =] procedure @param = value",
        ),
        (
            "EXECUTE",
            "Executes a stored procedure",
            "EXECUTE [@ret =] procedure @param = value",
        ),
        ("PRINT", "Outputs a message", "PRINT 'message'"),
        ("GO", "Batch separator (ASE/Sybase)", "GO [count]"),
        ("TOP", "Limits rows returned", "SELECT TOP n * FROM table"),
    ];
    m.into_iter().map(|(k, d, s)| (k, (d, s))).collect()
});

/// データ型ドキュメント
static DATATYPE_DOCS: Lazy<HashMap<&str, (&str, &str)>> = Lazy::new(|| {
    let m: Vec<(&str, &str, &str)> = vec![
        ("INT", "Integer (-2^31 to 2^31-1, 4 bytes)", "INT"),
        ("INTEGER", "Integer (same as INT, 4 bytes)", "INTEGER"),
        (
            "SMALLINT",
            "Small integer (-32768 to 32767, 2 bytes)",
            "SMALLINT",
        ),
        ("TINYINT", "Tiny integer (0 to 255, 1 byte)", "TINYINT"),
        ("BIGINT", "Big integer (-2^63 to 2^63-1, 8 bytes)", "BIGINT"),
        ("REAL", "Floating point (4 bytes, IEEE 754)", "REAL"),
        ("FLOAT", "Floating point (8 bytes, IEEE 754)", "FLOAT[(n)]"),
        ("DOUBLE", "Double precision (8 bytes)", "DOUBLE"),
        (
            "DECIMAL",
            "Exact numeric (precision, scale)",
            "DECIMAL(p[, s])",
        ),
        (
            "NUMERIC",
            "Exact numeric (same as DECIMAL)",
            "NUMERIC(p[, s])",
        ),
        (
            "MONEY",
            "Monetary value (-922 trillion to 922 trillion, 8 bytes)",
            "MONEY",
        ),
        (
            "SMALLMONEY",
            "Small monetary (-214748 to 214748, 4 bytes)",
            "SMALLMONEY",
        ),
        (
            "CHAR",
            "Fixed-length character (up to 16384 chars)",
            "CHAR(n)",
        ),
        (
            "VARCHAR",
            "Variable-length character (up to 16384 chars)",
            "VARCHAR(n)",
        ),
        ("TEXT", "Variable-length text (up to 2GB)", "TEXT"),
        ("NCHAR", "Fixed-length Unicode (UTF-16)", "NCHAR(n)"),
        (
            "NVARCHAR",
            "Variable-length Unicode (UTF-16)",
            "NVARCHAR(n)",
        ),
        (
            "UNICHAR",
            "Fixed-length Unicode (ASE specific, UTF-16)",
            "UNICHAR(n)",
        ),
        (
            "UNIVARCHAR",
            "Variable-length Unicode (ASE specific, UTF-16)",
            "UNIVARCHAR(n)",
        ),
        (
            "UNITEXT",
            "Variable-length Unicode text (ASE specific)",
            "UNITEXT",
        ),
        ("BINARY", "Fixed-length binary data", "BINARY(n)"),
        ("VARBINARY", "Variable-length binary data", "VARBINARY(n)"),
        ("IMAGE", "Variable-length binary (up to 2GB)", "IMAGE"),
        ("DATE", "Date value (0001-01-01 to 9999-12-31)", "DATE"),
        ("TIME", "Time value (00:00:00 to 23:59:59)", "TIME"),
        (
            "DATETIME",
            "Date and time (1753-01-01 to 9999-12-31, 8 bytes)",
            "DATETIME",
        ),
        (
            "SMALLDATETIME",
            "Small date and time (1900-01-01 to 2079-06-06, 4 bytes)",
            "SMALLDATETIME",
        ),
        (
            "BIGDATETIME",
            "High-precision datetime (ASE 15.7+, 1/1000 sec)",
            "BIGDATETIME",
        ),
        ("BIGTIME", "High-precision time (ASE 15.7+)", "BIGTIME"),
        (
            "TIMESTAMP",
            "Auto-updated binary (unique within database)",
            "TIMESTAMP",
        ),
        ("BIT", "Boolean (0 or 1)", "BIT"),
        (
            "UNIQUEIDENTIFIER",
            "GUID/UUID (16 bytes)",
            "UNIQUEIDENTIFIER",
        ),
    ];
    m.into_iter().map(|(k, d, s)| (k, (d, s))).collect()
});

/// 組み込み関数ドキュメント
static FUNCTION_DOCS: Lazy<HashMap<&str, (&str, &str)>> = Lazy::new(|| {
    let m: Vec<(&str, &str, &str)> = vec![
        // 文字列関数
        (
            "SUBSTRING",
            "Extract substring",
            "SUBSTRING(expression, start, length)",
        ),
        (
            "CHAR_LENGTH",
            "String length in characters",
            "CHAR_LENGTH(expression)",
        ),
        (
            "LEN",
            "String length (trailing spaces not counted)",
            "LEN(expression)",
        ),
        ("UPPER", "Convert to uppercase", "UPPER(expression)"),
        ("LOWER", "Convert to lowercase", "LOWER(expression)"),
        ("LTRIM", "Trim leading spaces", "LTRIM(expression)"),
        ("RTRIM", "Trim trailing spaces", "RTRIM(expression)"),
        (
            "STR_REPLACE",
            "Replace string occurrences",
            "STR_REPLACE(source, pattern, replacement)",
        ),
        (
            "STUFF",
            "Delete and insert at position",
            "STUFF(source, start, length, insert)",
        ),
        (
            "REPLICATE",
            "Repeat string N times",
            "REPLICATE(expression, n)",
        ),
        ("SPACE", "Generate N spaces", "SPACE(n)"),
        ("REVERSE", "Reverse string", "REVERSE(expression)"),
        ("RIGHT", "Right N characters", "RIGHT(expression, n)"),
        ("LEFT", "Left N characters", "LEFT(expression, n)"),
        (
            "CHARINDEX",
            "Find pattern position (1-based)",
            "CHARINDEX(pattern, expression)",
        ),
        (
            "PATINDEX",
            "Pattern index using wildcards (1-based)",
            "PATINDEX('%pattern%', expression)",
        ),
        // 数値関数
        ("ABS", "Absolute value", "ABS(expression)"),
        (
            "CEILING",
            "Smallest integer >= value",
            "CEILING(expression)",
        ),
        ("FLOOR", "Largest integer <= value", "FLOOR(expression)"),
        ("ROUND", "Round to N decimal places", "ROUND(expression, n)"),
        ("SQRT", "Square root", "SQRT(expression)"),
        ("POWER", "Raise to power", "POWER(expression, n)"),
        ("SIGN", "Sign of value (-1, 0, 1)", "SIGN(expression)"),
        // 日付関数
        ("GETDATE", "Current date and time", "GETDATE()"),
        (
            "DATEADD",
            "Add interval to date",
            "DATEADD(unit, number, date)",
        ),
        (
            "DATEDIFF",
            "Difference between dates",
            "DATEDIFF(unit, date1, date2)",
        ),
        (
            "DATEPART",
            "Extract date part as integer",
            "DATEPART(unit, date)",
        ),
        (
            "DATENAME",
            "Extract date part as string",
            "DATENAME(unit, date)",
        ),
        ("DAY", "Day of month (1-31)", "DAY(date)"),
        ("MONTH", "Month number (1-12)", "MONTH(date)"),
        ("YEAR", "Year number", "YEAR(date)"),
        // 変換関数
        (
            "CONVERT",
            "Type conversion with style",
            "CONVERT(type, expression[, style])",
        ),
        ("CAST", "Type conversion", "CAST(expression AS type)"),
        // 集約関数
        ("COUNT", "Count rows", "COUNT([DISTINCT] expression | *)"),
        ("SUM", "Sum values", "SUM([DISTINCT] expression)"),
        ("AVG", "Average value", "AVG([DISTINCT] expression)"),
        ("MIN", "Minimum value", "MIN(expression)"),
        ("MAX", "Maximum value", "MAX(expression)"),
        // システム関数
        (
            "ISNULL",
            "Replace NULL with value",
            "ISNULL(expression, replacement)",
        ),
        (
            "COALESCE",
            "First non-NULL value",
            "COALESCE(expr1, expr2, ...)",
        ),
        (
            "NULLIF",
            "NULL if expressions are equal",
            "NULLIF(expr1, expr2)",
        ),
        ("IDENTITY", "Identity column value", "IDENTITY"),
        (
            "OBJECT_ID",
            "Database object ID",
            "OBJECT_ID('object_name')",
        ),
        (
            "COL_LENGTH",
            "Column length in bytes",
            "COL_LENGTH('table', 'column')",
        ),
        (
            "VALID_NAME",
            "Check if identifier is valid",
            "VALID_NAME('name')",
        ),
    ];
    m.into_iter().map(|(k, d, s)| (k, (d, s))).collect()
});

/// Hover情報を生成する
///
/// カーソル位置のトークンを特定し、対応するドキュメントを返す。
pub fn hover(source: &str, position: Position) -> Option<Hover> {
    let offset = position_to_offset(source, position);
    let lexer = Lexer::new(source);

    let mut hovered_token = None;
    for token_result in lexer {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };
        let start = token.span.start as usize;
        let end = token.span.end as usize;
        if offset >= start && offset < end {
            hovered_token = Some((token.kind, token.text.to_string(), start, end));
            break;
        }
        if start > offset {
            break;
        }
    }

    let (kind, text, start, end) = hovered_token?;
    let content = build_hover_content(&kind, &text)?;

    let (start_line, start_char) = offset_to_position(source, start as u32);
    let (end_line, end_char) = offset_to_position(source, end as u32);

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: Some(Range {
            start: Position {
                line: start_line,
                character: start_char,
            },
            end: Position {
                line: end_line,
                character: end_char,
            },
        }),
    })
}

/// トークンの種類に応じてHover内容を構築する
fn build_hover_content(kind: &TokenKind, text: &str) -> Option<String> {
    let upper = text.to_uppercase();

    match kind {
        TokenKind::LocalVar => {
            let var_name = text.trim_start_matches('@');
            Some(format!(
                "```tsql\n{text}: VARIABLE\n```\n\nLocal variable — Declare with `DECLARE @{var_name} TYPE`"
            ))
        }
        _ => {
            if let Some((desc, syntax)) = KEYWORD_DOCS
                .get(upper.as_str())
                .or_else(|| DATATYPE_DOCS.get(upper.as_str()))
                .or_else(|| FUNCTION_DOCS.get(upper.as_str()))
            {
                Some(format!(
                    "```tsql\n{syntax}\n```\n\n**`{}`** — {desc}",
                    upper
                ))
            } else if kind.is_keyword() {
                Some(format!("**`{upper}`** — T-SQL Keyword"))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_keyword_select() {
        let result = hover(
            "SELECT * FROM t",
            Position {
                line: 0,
                character: 2,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("SELECT"));
                assert!(mc.value.contains("Retrieves data"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_keyword_from() {
        let result = hover(
            "SELECT * FROM t",
            Position {
                line: 0,
                character: 10,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("FROM"));
                assert!(mc.value.contains("source tables"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_datatype_varchar() {
        let result = hover(
            "CREATE TABLE t (col VARCHAR(100))",
            Position {
                line: 0,
                character: 25,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("VARCHAR"));
                assert!(mc.value.contains("Variable-length"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_function_getdate() {
        let result = hover(
            "SELECT GETDATE()",
            Position {
                line: 0,
                character: 9,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("GETDATE"));
                assert!(mc.value.contains("Current"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_variable() {
        let result = hover(
            "SELECT @var",
            Position {
                line: 0,
                character: 8,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("@var"));
                assert!(mc.value.contains("variable"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_whitespace_returns_none() {
        let result = hover(
            "SELECT  FROM t",
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_hover_has_range() {
        let result = hover(
            "SELECT * FROM t",
            Position {
                line: 0,
                character: 2,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        assert!(h.range.is_some());
        let range = h.range.unwrap();
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.character, 6);
    }
}
