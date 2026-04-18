//! ASE Documentation Data — SAP ASE ドメインデータの単一ソース
//!
//! キーワード、データ型、組み込み関数のドキュメントデータを一箇所に集約する。
//! hover, signature_help, completion の各モジュールはこのデータを参照する。

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// ドキュメントエントリのカテゴリ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocCategory {
    /// SQLキーワード
    Keyword,
    /// データ型
    DataType,
    /// 組み込み関数
    Function,
    /// システム変数
    SystemVariable,
}

/// ASE組み込みドキュメントエントリ
#[derive(Debug, Clone)]
pub struct DocEntry {
    /// 名前（大文字）
    pub name: &'static str,
    /// 説明文
    pub description: &'static str,
    /// 構文例
    pub syntax: &'static str,
    /// パラメータ名リスト
    pub params: &'static [&'static str],
    /// カテゴリ
    pub category: DocCategory,
}

// ---------------------------------------------------------------------------
// Keyword entries
// ---------------------------------------------------------------------------

static KEYWORD_ENTRIES: &[DocEntry] = &[
    DocEntry {
        name: "SELECT",
        description: "Retrieves data from one or more tables",
        syntax: "SELECT [DISTINCT] col1, col2 FROM table WHERE condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "FROM",
        description: "Specifies the source tables for a query",
        syntax: "FROM table1 [AS alias] [JOIN table2 ON ...]",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "WHERE",
        description: "Filters rows based on a condition",
        syntax: "WHERE column = value AND column2 > value2",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "INSERT",
        description: "Inserts rows into a table",
        syntax: "INSERT INTO table (col1, col2) VALUES (val1, val2)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "INTO",
        description: "Target specification for INSERT/SELECT INTO",
        syntax: "INSERT INTO table ...",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "UPDATE",
        description: "Modifies existing rows in a table",
        syntax: "UPDATE table SET col1 = val1 WHERE condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "DELETE",
        description: "Removes rows from a table",
        syntax: "DELETE FROM table WHERE condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "CREATE",
        description: "Creates a new database object",
        syntax: "CREATE TABLE | PROCEDURE | VIEW | INDEX ...",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "ALTER",
        description: "Modifies an existing database object",
        syntax: "ALTER TABLE ... | ALTER PROCEDURE ...",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "DROP",
        description: "Removes a database object",
        syntax: "DROP TABLE | PROCEDURE | VIEW | INDEX ...",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "TABLE",
        description: "Defines or references a table structure",
        syntax: "CREATE TABLE name (col1 TYPE, ...)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "INDEX",
        description: "Creates an index on table columns",
        syntax: "CREATE [UNIQUE] [CLUSTERED] INDEX name ON table(col)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "VIEW",
        description: "Creates a virtual table based on a query",
        syntax: "CREATE VIEW name AS SELECT ...",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "PROCEDURE",
        description: "Creates a stored procedure",
        syntax: "CREATE PROCEDURE name @param TYPE AS BEGIN ... END",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "IF",
        description: "Conditional execution",
        syntax: "IF condition BEGIN ... END ELSE BEGIN ... END",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "ELSE",
        description: "Alternative branch for IF",
        syntax: "IF ... ELSE ...",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "BEGIN",
        description: "Starts a statement block",
        syntax: "BEGIN ... END",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "END",
        description: "Ends a statement block",
        syntax: "BEGIN ... END",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "WHILE",
        description: "Loop construct",
        syntax: "WHILE condition BEGIN ... END",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "RETURN",
        description: "Exits a procedure or batch",
        syntax: "RETURN [value]",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "DECLARE",
        description: "Declares a local variable",
        syntax: "DECLARE @var TYPE [, @var2 TYPE]",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "SET",
        description: "Assigns a value to a variable",
        syntax: "SET @var = expression",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "JOIN",
        description: "Combines rows from two tables",
        syntax: "[INNER|LEFT|RIGHT|FULL] JOIN table ON condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "INNER",
        description: "Specifies an inner join",
        syntax: "INNER JOIN table ON condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "LEFT",
        description: "Specifies a left outer join",
        syntax: "LEFT JOIN table ON condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "RIGHT",
        description: "Specifies a right outer join",
        syntax: "RIGHT JOIN table ON condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "ON",
        description: "Specifies join condition",
        syntax: "... JOIN table ON condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "AND",
        description: "Logical AND in conditions",
        syntax: "condition1 AND condition2",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "OR",
        description: "Logical OR in conditions",
        syntax: "condition1 OR condition2",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "NOT",
        description: "Logical NOT",
        syntax: "NOT condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "IN",
        description: "Checks if value is in a set",
        syntax: "column IN (val1, val2, ...)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "EXISTS",
        description: "Checks if subquery returns rows",
        syntax: "EXISTS (SELECT ...)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "BETWEEN",
        description: "Range check",
        syntax: "column BETWEEN val1 AND val2",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "LIKE",
        description: "Pattern matching",
        syntax: "column LIKE 'pattern%'",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "IS",
        description: "NULL check",
        syntax: "column IS [NOT] NULL",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "NULL",
        description: "Represents missing or unknown data",
        syntax: "column IS NULL | column = NULL",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "ORDER",
        description: "Sorts query results",
        syntax: "ORDER BY col1 [ASC|DESC], col2",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "BY",
        description: "Used with ORDER, GROUP",
        syntax: "ORDER BY | GROUP BY",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "GROUP",
        description: "Groups rows for aggregation",
        syntax: "GROUP BY col1, col2",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "HAVING",
        description: "Filters grouped results",
        syntax: "GROUP BY ... HAVING condition",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "UNION",
        description: "Combines result sets",
        syntax: "SELECT ... UNION [ALL] SELECT ...",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "DISTINCT",
        description: "Removes duplicate rows",
        syntax: "SELECT DISTINCT col1, col2 ...",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "CASE",
        description: "Conditional expression",
        syntax: "CASE WHEN cond THEN val ELSE val END",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "WHEN",
        description: "Condition branch in CASE",
        syntax: "CASE WHEN condition THEN result",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "THEN",
        description: "Result in CASE WHEN",
        syntax: "WHEN condition THEN result",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "AS",
        description: "Creates an alias",
        syntax: "column AS alias | table AS alias",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "PRIMARY",
        description: "Primary key constraint",
        syntax: "PRIMARY KEY (col1, col2)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "FOREIGN",
        description: "Foreign key constraint",
        syntax: "FOREIGN KEY (col) REFERENCES table(col)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "KEY",
        description: "Constraint keyword",
        syntax: "PRIMARY KEY | FOREIGN KEY",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "REFERENCES",
        description: "Referenced table/column",
        syntax: "FOREIGN KEY (col) REFERENCES table(col)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "UNIQUE",
        description: "Unique constraint",
        syntax: "UNIQUE (col1, col2)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "CHECK",
        description: "Check constraint",
        syntax: "CHECK (condition)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "DEFAULT",
        description: "Default value for column",
        syntax: "col TYPE DEFAULT value",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "IDENTITY",
        description: "Auto-increment column",
        syntax: "col INT IDENTITY(1,1)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "CONSTRAINT",
        description: "Named constraint",
        syntax: "CONSTRAINT name PRIMARY KEY (col)",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "TRY",
        description: "Starts error handling block",
        syntax: "BEGIN TRY ... END TRY BEGIN CATCH ... END CATCH",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "CATCH",
        description: "Catches errors in TRY block",
        syntax: "BEGIN TRY ... BEGIN CATCH ... END CATCH",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "THROW",
        description: "Raises an error",
        syntax: "THROW error_number, 'message', state",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "COMMIT",
        description: "Commits a transaction",
        syntax: "COMMIT [TRAN[SACTION] [name]]",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "ROLLBACK",
        description: "Rolls back a transaction",
        syntax: "ROLLBACK [TRAN[SACTION] [name]]",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "TRANSACTION",
        description: "Transaction management",
        syntax: "BEGIN TRANSACTION | COMMIT | ROLLBACK",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "GRANT",
        description: "Grants permissions",
        syntax: "GRANT SELECT ON table TO user",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "REVOKE",
        description: "Removes permissions",
        syntax: "REVOKE SELECT ON table FROM user",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "EXEC",
        description: "Executes a stored procedure",
        syntax: "EXEC [@ret =] procedure @param = value",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "EXECUTE",
        description: "Executes a stored procedure",
        syntax: "EXECUTE [@ret =] procedure @param = value",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "PRINT",
        description: "Outputs a message",
        syntax: "PRINT 'message'",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "GO",
        description: "Batch separator (ASE/Sybase)",
        syntax: "GO [count]",
        params: &[],
        category: DocCategory::Keyword,
    },
    DocEntry {
        name: "TOP",
        description: "Limits rows returned",
        syntax: "SELECT TOP n * FROM table",
        params: &[],
        category: DocCategory::Keyword,
    },
];

// ---------------------------------------------------------------------------
// Data type entries
// ---------------------------------------------------------------------------

static DATATYPE_ENTRIES: &[DocEntry] = &[
    DocEntry {
        name: "INT",
        description: "Integer (-2^31 to 2^31-1, 4 bytes)",
        syntax: "INT",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "INTEGER",
        description: "Integer (same as INT)",
        syntax: "INTEGER",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "SMALLINT",
        description: "Small integer (-32768 to 32767, 2 bytes)",
        syntax: "SMALLINT",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "TINYINT",
        description: "Tiny integer (0 to 255, 1 byte)",
        syntax: "TINYINT",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "BIGINT",
        description: "Big integer (-2^63 to 2^63-1, 8 bytes)",
        syntax: "BIGINT",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "REAL",
        description: "Floating point (4 bytes)",
        syntax: "REAL",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "FLOAT",
        description: "Floating point (8 bytes)",
        syntax: "FLOAT[(n)]",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "DOUBLE",
        description: "Double precision floating point (8 bytes)",
        syntax: "DOUBLE",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "DECIMAL",
        description: "Exact numeric with precision and scale",
        syntax: "DECIMAL(p[, s])",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "NUMERIC",
        description: "Exact numeric (same as DECIMAL)",
        syntax: "NUMERIC(p[, s])",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "MONEY",
        description: "Monetary value (-922 trillion to 922 trillion, 8 bytes)",
        syntax: "MONEY",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "SMALLMONEY",
        description: "Small monetary (-214748 to 214748, 4 bytes)",
        syntax: "SMALLMONEY",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "CHAR",
        description: "Fixed-length character (up to 16384 chars)",
        syntax: "CHAR(n)",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "VARCHAR",
        description: "Variable-length character (up to 16384 chars)",
        syntax: "VARCHAR(n)",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "TEXT",
        description: "Variable-length text (up to 2GB)",
        syntax: "TEXT",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "NCHAR",
        description: "Fixed-length Unicode (UTF-16)",
        syntax: "NCHAR(n)",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "NVARCHAR",
        description: "Variable-length Unicode (UTF-16)",
        syntax: "NVARCHAR(n)",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "UNICHAR",
        description: "Fixed-length Unicode (ASE specific, UTF-16)",
        syntax: "UNICHAR(n)",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "UNIVARCHAR",
        description: "Variable-length Unicode (ASE specific, UTF-16)",
        syntax: "UNIVARCHAR(n)",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "UNITEXT",
        description: "Variable-length Unicode text (ASE specific)",
        syntax: "UNITEXT",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "BINARY",
        description: "Fixed-length binary data",
        syntax: "BINARY(n)",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "VARBINARY",
        description: "Variable-length binary data",
        syntax: "VARBINARY(n)",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "IMAGE",
        description: "Variable-length binary (up to 2GB)",
        syntax: "IMAGE",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "DATE",
        description: "Date value (0001-01-01 to 9999-12-31)",
        syntax: "DATE",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "TIME",
        description: "Time value (00:00:00 to 23:59:59)",
        syntax: "TIME",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "DATETIME",
        description: "Date and time (1753-01-01 to 9999-12-31, 8 bytes)",
        syntax: "DATETIME",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "SMALLDATETIME",
        description: "Small date and time (1900-01-01 to 2079-06-06, 4 bytes)",
        syntax: "SMALLDATETIME",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "BIGDATETIME",
        description: "High-precision datetime (ASE 15.7+, 1/1000 sec)",
        syntax: "BIGDATETIME",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "BIGTIME",
        description: "High-precision time (ASE 15.7+)",
        syntax: "BIGTIME",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "TIMESTAMP",
        description: "Auto-updated binary (unique within database)",
        syntax: "TIMESTAMP",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "BIT",
        description: "Boolean (0 or 1)",
        syntax: "BIT",
        params: &[],
        category: DocCategory::DataType,
    },
    DocEntry {
        name: "UNIQUEIDENTIFIER",
        description: "GUID/UUID (16 bytes)",
        syntax: "UNIQUEIDENTIFIER",
        params: &[],
        category: DocCategory::DataType,
    },
];

// ---------------------------------------------------------------------------
// Function entries (used by hover, signature_help, and completion)
// ---------------------------------------------------------------------------

static FUNCTION_ENTRIES: &[DocEntry] = &[
    // String functions
    DocEntry {
        name: "SUBSTRING",
        description: "Extracts a substring from a string expression",
        syntax: "SUBSTRING(expression, start, length)",
        params: &["expression", "start", "length"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "CHAR_LENGTH",
        description: "Returns the length of a string in characters",
        syntax: "CHAR_LENGTH(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "LEN",
        description: "String length (trailing spaces not counted)",
        syntax: "LEN(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "UPPER",
        description: "Converts a string to uppercase",
        syntax: "UPPER(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "LOWER",
        description: "Converts a string to lowercase",
        syntax: "LOWER(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "LTRIM",
        description: "Removes leading spaces",
        syntax: "LTRIM(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "RTRIM",
        description: "Removes trailing spaces",
        syntax: "RTRIM(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "STR_REPLACE",
        description: "Replaces all occurrences of a pattern in a string",
        syntax: "STR_REPLACE(source, pattern, replacement)",
        params: &["source", "pattern", "replacement"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "STUFF",
        description: "Deletes and inserts characters at a specified position",
        syntax: "STUFF(source, start, length, insert)",
        params: &["source", "start", "length", "insert"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "REPLICATE",
        description: "Repeat string N times",
        syntax: "REPLICATE(expression, n)",
        params: &["expression", "n"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "SPACE",
        description: "Generate N spaces",
        syntax: "SPACE(n)",
        params: &["n"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "REVERSE",
        description: "Reverse string",
        syntax: "REVERSE(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "CHARINDEX",
        description: "Find pattern position (1-based)",
        syntax: "CHARINDEX(pattern, expression)",
        params: &["pattern", "expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "PATINDEX",
        description: "Pattern index using wildcards (1-based)",
        syntax: "PATINDEX('%pattern%', expression)",
        params: &["pattern", "expression"],
        category: DocCategory::Function,
    },
    // Numeric functions
    DocEntry {
        name: "ABS",
        description: "Absolute value",
        syntax: "ABS(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "CEILING",
        description: "Smallest integer >= value",
        syntax: "CEILING(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "FLOOR",
        description: "Largest integer <= value",
        syntax: "FLOOR(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "ROUND",
        description: "Rounds a numeric value to n decimal places",
        syntax: "ROUND(expression, n)",
        params: &["expression", "n"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "SQRT",
        description: "Square root",
        syntax: "SQRT(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "POWER",
        description: "Raise to power",
        syntax: "POWER(expression, n)",
        params: &["expression", "n"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "SIGN",
        description: "Sign of value (-1, 0, 1)",
        syntax: "SIGN(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    // Date functions
    DocEntry {
        name: "GETDATE",
        description: "Current date and time",
        syntax: "GETDATE()",
        params: &[],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "DATEADD",
        description: "Adds an interval to a date",
        syntax: "DATEADD(unit, number, date)",
        params: &["unit", "number", "date"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "DATEDIFF",
        description: "Returns the difference between two dates",
        syntax: "DATEDIFF(unit, date1, date2)",
        params: &["unit", "date1", "date2"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "DATEPART",
        description: "Extracts a part of a date as an integer",
        syntax: "DATEPART(unit, date)",
        params: &["unit", "date"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "DATENAME",
        description: "Extract date part as string",
        syntax: "DATENAME(unit, date)",
        params: &["unit", "date"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "DAY",
        description: "Day of month (1-31)",
        syntax: "DAY(date)",
        params: &["date"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "MONTH",
        description: "Month number (1-12)",
        syntax: "MONTH(date)",
        params: &["date"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "YEAR",
        description: "Year number",
        syntax: "YEAR(date)",
        params: &["date"],
        category: DocCategory::Function,
    },
    // Conversion functions
    DocEntry {
        name: "CONVERT",
        description: "Converts an expression to the specified data type",
        syntax: "CONVERT(type, expression[, style])",
        params: &["type", "expression", "style"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "CAST",
        description: "Converts an expression to the specified data type",
        syntax: "CAST(expression AS type)",
        params: &["expression", "type"],
        category: DocCategory::Function,
    },
    // Aggregate functions
    DocEntry {
        name: "COUNT",
        description: "Returns the number of rows",
        syntax: "COUNT([DISTINCT] expression | *)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "SUM",
        description: "Returns the sum of values",
        syntax: "SUM([DISTINCT] expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "AVG",
        description: "Returns the average of values",
        syntax: "AVG([DISTINCT] expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "MIN",
        description: "Returns the minimum value",
        syntax: "MIN(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "MAX",
        description: "Returns the maximum value",
        syntax: "MAX(expression)",
        params: &["expression"],
        category: DocCategory::Function,
    },
    // System functions
    DocEntry {
        name: "ISNULL",
        description: "Replaces NULL with the specified replacement value",
        syntax: "ISNULL(expression, replacement)",
        params: &["expression", "replacement"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "COALESCE",
        description: "Returns the first non-NULL expression",
        syntax: "COALESCE(expr1, expr2, ...)",
        params: &["expr1", "expr2", "..."],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "NULLIF",
        description: "NULL if expressions are equal",
        syntax: "NULLIF(expr1, expr2)",
        params: &["expr1", "expr2"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "IDENTITY",
        description: "Identity column value",
        syntax: "IDENTITY",
        params: &[],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "OBJECT_ID",
        description: "Database object ID",
        syntax: "OBJECT_ID('object_name')",
        params: &["object_name"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "COL_LENGTH",
        description: "Column length in bytes",
        syntax: "COL_LENGTH('table', 'column')",
        params: &["table", "column"],
        category: DocCategory::Function,
    },
    DocEntry {
        name: "VALID_NAME",
        description: "Check if identifier is valid",
        syntax: "VALID_NAME('name')",
        params: &["name"],
        category: DocCategory::Function,
    },
];

// ---------------------------------------------------------------------------
// Lookup helpers — O(1) by name
// ---------------------------------------------------------------------------

/// 関数エントリの名前で検索できるHashMap（関数優先）
static FUNCTION_LOOKUP: Lazy<HashMap<&'static str, &'static DocEntry>> =
    Lazy::new(|| FUNCTION_ENTRIES.iter().map(|e| (e.name, e)).collect());

/// キーワード・データ型エントリの名前で検索できるHashMap
static OTHER_LOOKUP: Lazy<HashMap<&'static str, &'static DocEntry>> = Lazy::new(|| {
    KEYWORD_ENTRIES
        .iter()
        .chain(DATATYPE_ENTRIES.iter())
        .map(|e| (e.name, e))
        .collect()
});

/// 名前（大文字）でDocEntryを検索する
/// 関数名がキーワードと重複する場合（例: RIGHT）、関数を優先する
pub fn lookup(name: &str) -> Option<&'static DocEntry> {
    // 関数を優先
    FUNCTION_LOOKUP
        .get(name)
        .copied()
        .or_else(|| OTHER_LOOKUP.get(name).copied())
}

/// キーワードエントリのスライスを返す
pub fn keywords() -> &'static [DocEntry] {
    KEYWORD_ENTRIES
}

/// データ型エントリのスライスを返す
pub fn datatypes() -> &'static [DocEntry] {
    DATATYPE_ENTRIES
}

/// 関数エントリのスライスを返す
pub fn functions() -> &'static [DocEntry] {
    FUNCTION_ENTRIES
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_keyword() {
        let entry = lookup("SELECT").unwrap();
        assert_eq!(entry.name, "SELECT");
        assert_eq!(entry.category, DocCategory::Keyword);
        assert!(!entry.description.is_empty());
    }

    #[test]
    fn test_lookup_datatype() {
        let entry = lookup("VARCHAR").unwrap();
        assert_eq!(entry.name, "VARCHAR");
        assert_eq!(entry.category, DocCategory::DataType);
    }

    #[test]
    fn test_lookup_function() {
        let entry = lookup("SUBSTRING").unwrap();
        assert_eq!(entry.name, "SUBSTRING");
        assert_eq!(entry.category, DocCategory::Function);
        assert_eq!(entry.params, &["expression", "start", "length"]);
    }

    #[test]
    fn test_lookup_keyword_right() {
        // RIGHT はキーワード（RIGHT JOIN）として存在
        let entry = lookup("RIGHT").unwrap();
        assert_eq!(entry.name, "RIGHT");
        assert_eq!(entry.category, DocCategory::Keyword);
    }

    #[test]
    fn test_lookup_case_sensitive() {
        // lookup is case-sensitive (uppercase keys)
        assert!(lookup("select").is_none());
        assert!(lookup("SELECT").is_some());
    }

    #[test]
    fn test_lookup_not_found() {
        assert!(lookup("NONEXISTENT").is_none());
    }

    #[test]
    fn test_no_duplicate_names() {
        // 各カテゴリ内で重複がないことを確認
        let mut keyword_names = std::collections::HashSet::new();
        for e in KEYWORD_ENTRIES.iter() {
            assert!(
                !keyword_names.contains(e.name),
                "Duplicate keyword name: {}",
                e.name
            );
            keyword_names.insert(e.name);
        }

        let mut datatype_names = std::collections::HashSet::new();
        for e in DATATYPE_ENTRIES.iter() {
            assert!(
                !datatype_names.contains(e.name),
                "Duplicate datatype name: {}",
                e.name
            );
            datatype_names.insert(e.name);
        }

        let mut function_names = std::collections::HashSet::new();
        for e in FUNCTION_ENTRIES.iter() {
            assert!(
                !function_names.contains(e.name),
                "Duplicate function name: {}",
                e.name
            );
            function_names.insert(e.name);
        }

        // カテゴリ間での重複は許容（例: IDENTITY はキーワードとしても関数としても存在）
        // lookup() は関数を優先して返す
    }

    #[test]
    fn test_all_entries_have_description() {
        for e in KEYWORD_ENTRIES
            .iter()
            .chain(DATATYPE_ENTRIES.iter())
            .chain(FUNCTION_ENTRIES.iter())
        {
            assert!(
                !e.description.is_empty(),
                "Missing description for: {}",
                e.name
            );
            assert!(!e.syntax.is_empty(), "Missing syntax for: {}", e.name);
        }
    }

    #[test]
    fn test_entry_counts() {
        assert!(!KEYWORD_ENTRIES.is_empty());
        assert!(!DATATYPE_ENTRIES.is_empty());
        assert!(!FUNCTION_ENTRIES.is_empty());
        // Spot-check counts are in reasonable range
        assert!(KEYWORD_ENTRIES.len() >= 50);
        assert!(DATATYPE_ENTRIES.len() >= 25);
        assert!(FUNCTION_ENTRIES.len() >= 35);
    }
}
