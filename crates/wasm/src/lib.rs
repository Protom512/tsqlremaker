//! T-SQL Remaker WebAssembly Bindings
//!
//! This crate provides JavaScript/WASM bindings for the T-SQL lexer and parser.

#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

#[cfg(feature = "wasm")]
mod ast_js;
#[cfg(feature = "wasm")]
mod token_js;

#[cfg(feature = "wasm")]
use std::panic;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

// Export WASM-friendly types
#[cfg(feature = "wasm")]
pub use ast_js::{JsConversionResult, JsParseError, JsParseResult, JsStatement};
#[cfg(feature = "wasm")]
pub use token_js::{JsPosition, JsSpan, JsToken, JsTokenKind};

use tsql_lexer::Lexer;

/// Target SQL dialect for conversion
#[cfg(feature = "wasm")]
#[derive(Debug, Clone, Copy)]
#[wasm_bindgen]
pub enum TargetDialect {
    /// MySQL / MariaDB
    MySQL,
    /// PostgreSQL
    PostgreSQL,
    /// SQLite
    SQLite,
}

/// Initialize the panic hook for better error messages in the browser
#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn start() {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
}

/// Tokenize SQL input
///
/// # Arguments
///
/// * `input` - SQL source code to tokenize
///
/// # Returns
///
/// Array of tokens, or throws an error on failure
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = tokenize)]
pub fn tokenize_js(input: &str) -> Result<JsValue, JsValue> {
    let mut lexer = Lexer::new(input);
    let tokens: Result<Vec<_>, _> = lexer.by_ref().collect();

    match tokens {
        Ok(tokens) => {
            let js_tokens: Vec<JsToken> = tokens.into_iter().map(JsToken::from).collect();
            Ok(serde_wasm_bindgen::to_value(&js_tokens)?)
        }
        Err(e) => Err(JsError::new(&e.to_string()).into()),
    }
}

/// Parse SQL input into statements
///
/// # Arguments
///
/// * `input` - SQL source code to parse
///
/// # Returns
///
/// Parse result containing statements or error information
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = parse)]
pub fn parse_js(input: &str) -> JsValue {
    match parse(input) {
        Ok(statements) => {
            let js_statements: Vec<JsStatement> = statements
                .into_iter()
                .filter_map(|s| JsStatement::try_from(s).ok())
                .collect();
            serde_wasm_bindgen::to_value(&JsParseResult::Success(js_statements))
                .unwrap_or_else(|_| JsValue::from_str("{\"error\":\"serialization_error\"}"))
        }
        Err(e) => {
            let error = JsParseError::from(e);
            serde_wasm_bindgen::to_value(&JsParseResult::Error(error))
                .unwrap_or_else(|_| JsValue::from_str("{\"error\":\"serialization_error\"}"))
        }
    }
}

/// Parse a single SQL statement
///
/// # Arguments
///
/// * `input` - SQL source code to parse (single statement)
///
/// # Returns
///
/// Parse result containing the statement or error information
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = parseOne)]
pub fn parse_one_js(input: &str) -> JsValue {
    match parse_one(input) {
        Ok(stmt) => {
            if let Ok(js_stmt) = JsStatement::try_from(stmt) {
                serde_wasm_bindgen::to_value(&JsParseResult::SuccessSingle(js_stmt))
                    .unwrap_or_else(|_| JsValue::from_str("{\"error\":\"serialization_error\"}"))
            } else {
                serde_wasm_bindgen::to_value(&JsParseResult::Error(JsParseError {
                    message: "Statement type not supported".to_string(),
                    line: 0,
                    column: 0,
                    offset: 0,
                }))
                .unwrap_or_else(|_| JsValue::from_str("{\"error\":\"serialization_error\"}"))
            }
        }
        Err(e) => {
            let error = JsParseError::from(e);
            serde_wasm_bindgen::to_value(&JsParseResult::Error(error))
                .unwrap_or_else(|_| JsValue::from_str("{\"error\":\"serialization_error\"}"))
        }
    }
}

/// Get version information
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = getVersion)]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Convert T-SQL to target dialect
///
/// # Arguments
///
/// * `_input` - T-SQL source code to convert
/// * `dialect` - Target dialect (mysql, postgresql, sqlite)
///
/// # Returns
///
/// Convert T-SQL to target dialect SQL
///
/// # Arguments
///
/// * `input` - T-SQL SQL string
/// * `dialect` - Target dialect (MySQL or PostgreSQL)
///
/// # Returns
///
/// Conversion result containing the converted SQL or error information
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = convertTo)]
pub fn convert_to(input: &str, dialect: TargetDialect) -> JsValue {
    use ast_js::JsConversionResult;
    // 直接コンバータ (Issue #163): 旧2段階チェーン (to_common_ast + convert) を
    // 単一の to_common_sql に統合。レガシー bridge への依存を除去。
    use tsql_parser::ast::to_common_sql::to_common_sql;

    // Parser で T-SQL をパース
    let result = tsql_parser::parse(input);

    let stmts = match result {
        Ok(stmts) => stmts,
        Err(e) => {
            let error_result = JsConversionResult::Error {
                message: format!("Parse error: {}", e),
            };
            return serde_wasm_bindgen::to_value(&error_result)
                .unwrap_or_else(|_| JsValue::from_str(r#"{"error":"serialization_error"}"#));
        }
    };

    // 直接 common_sql::ast::Statement へ変換 (非対応/DialectSpecific は None で除外)
    let common_stmts: Vec<_> = stmts.iter().filter_map(to_common_sql).collect();

    if common_stmts.is_empty() && !stmts.is_empty() {
        // パースは成功したが、Common AST に変換できない文が含まれる
        let error_result = JsConversionResult::Error {
            message: "Statement contains unsupported features for conversion".to_string(),
        };
        return serde_wasm_bindgen::to_value(&error_result)
            .unwrap_or_else(|_| JsValue::from_str(r#"{"error":"serialization_error"}"#));
    }

    // PostgreSQL Emitter で出力
    match dialect {
        TargetDialect::PostgreSQL => {
            use postgresql_emitter::{EmissionConfig, PostgreSqlEmitter};

            let mut emitter = PostgreSqlEmitter::new(EmissionConfig::default());
            let mut results = Vec::new();

            for stmt in common_stmts {
                match emitter.emit(&stmt) {
                    Ok(sql) => results.push(sql),
                    Err(e) => {
                        let error_result = JsConversionResult::Error {
                            message: format!("Emit error: {}", e),
                        };
                        return serde_wasm_bindgen::to_value(&error_result).unwrap_or_else(|_| {
                            JsValue::from_str(r#"{"error":"serialization_error"}"#)
                        });
                    }
                }
            }

            let success_result = JsConversionResult::Success {
                sql: results.join(";\n"),
            };
            serde_wasm_bindgen::to_value(&success_result)
                .unwrap_or_else(|_| JsValue::from_str(r#"{"error":"serialization_error"}"#))
        }
        TargetDialect::MySQL => {
            use mysql_emitter::{EmitterConfig, MySqlEmitter};

            let mut emitter = MySqlEmitter::new(EmitterConfig::default());
            let mut results = Vec::new();

            for stmt in common_stmts {
                match emitter.emit(&stmt) {
                    Ok(sql) => results.push(sql),
                    Err(e) => {
                        let error_result = JsConversionResult::Error {
                            message: format!("Emit error: {}", e),
                        };
                        return serde_wasm_bindgen::to_value(&error_result).unwrap_or_else(|_| {
                            JsValue::from_str(r#"{"error":"serialization_error"}"#)
                        });
                    }
                }
            }

            let success_result = JsConversionResult::Success {
                sql: results.join(";\n"),
            };
            serde_wasm_bindgen::to_value(&success_result)
                .unwrap_or_else(|_| JsValue::from_str(r#"{"error":"serialization_error"}"#))
        }
        TargetDialect::SQLite => {
            use sqlite_emitter::{EmitterConfig, SqliteEmitter};

            let mut emitter = SqliteEmitter::new(EmitterConfig::default());
            let mut results = Vec::new();

            for stmt in common_stmts {
                match emitter.emit(&stmt) {
                    Ok(sql) => results.push(sql),
                    Err(e) => {
                        let error_result = JsConversionResult::Error {
                            message: format!("Emit error: {}", e),
                        };
                        return serde_wasm_bindgen::to_value(&error_result).unwrap_or_else(|_| {
                            JsValue::from_str(r#"{"error":"serialization_error"}"#)
                        });
                    }
                }
            }

            let success_result = JsConversionResult::Success {
                sql: results.join(";\n"),
            };
            serde_wasm_bindgen::to_value(&success_result)
                .unwrap_or_else(|_| JsValue::from_str(r#"{"error":"serialization_error"}"#))
        }
    }
}

/// Get supported target dialects
///
/// # Returns
///
/// Array of supported dialect names with their status
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = getSupportedDialects)]
pub fn get_supported_dialects() -> JsValue {
    let dialects = vec![
        ("postgresql", "PostgreSQL - Available"),
        ("mysql", "MySQL / MariaDB - Available"),
        ("sqlite", "SQLite - Available"),
    ];

    serde_wasm_bindgen::to_value(&dialects).unwrap_or_else(|_| JsValue::from_str("[]"))
}

// Non-WASM API for testing
#[cfg(not(feature = "wasm"))]
pub use tsql_parser::{parse, parse_one};

// Import parser functions for WASM
#[cfg(feature = "wasm")]
use tsql_parser::{parse, parse_one};

/// Tokenize SQL input (non-WASM API for testing)
///
/// # Arguments
///
/// * `input` - SQL source code to tokenize
///
/// # Returns
///
/// Tokens or a lexer error
#[cfg(not(feature = "wasm"))]
pub fn tokenize(input: &str) -> Result<Vec<tsql_lexer::Token<'_>>, tsql_lexer::LexError> {
    let mut lexer = Lexer::new(input);
    lexer.by_ref().collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[cfg(not(feature = "wasm"))]
    #[test]
    fn test_tokenize() {
        let input = "SELECT * FROM users";
        let result = tokenize(input);
        assert!(result.is_ok());
        let tokens = result.unwrap();
        assert!(!tokens.is_empty());
    }

    #[cfg(not(feature = "wasm"))]
    #[test]
    fn test_parse() {
        let input = "SELECT * FROM users";
        let result = parse(input);
        assert!(result.is_ok());
    }

    #[cfg(not(feature = "wasm"))]
    #[test]
    fn test_parse_one() {
        let input = "SELECT 1";
        let result = parse_one(input);
        assert!(result.is_ok());
    }

    // T6 (#158): convert_to の DialectSpecific パススルー契約の根拠を native で検証。
    // T3 の意図通り、T-SQL 制御構文 (DECLARE / IF) は to_common_sql で
    // `Some(DialectSpecific { .. })` に変換され、None-filter でドロップされない。
    // これにより convert_to は DialectSpecific 文を emitter まで届け、emitter が
    // Unsupported を返すことで "Emit error" を発行する (サイレントな空成功は起きない)。
    // 一方、変換先のない DDL (CREATE TABLE) や BatchSeparator は None になり、
    // filter_map 後に空になれば convert_to は "unsupported features" エラーを返す。
    #[cfg(not(feature = "wasm"))]
    #[test]
    fn test_to_common_sql_control_flow_passes_through_as_dialect_specific() {
        use tsql_parser::ast::to_common_sql::to_common_sql;

        // DECLARE @v INT → パース成功、to_common_sql は DialectSpecific を返す (T3)
        let stmts = tsql_parser::parse("DECLARE @v INT").unwrap_or_default();
        assert_eq!(stmts.len(), 1, "DECLARE should parse to one statement");
        let common = to_common_sql(&stmts[0]);
        assert!(
            common.is_some(),
            "DECLARE must pass through as Some (DialectSpecific), not None — \
             otherwise convert_to silently drops it"
        );

        // IF ... → パース成功、to_common_sql は DialectSpecific を返す (T3)
        let stmts = tsql_parser::parse("IF 1 = 1 SELECT 1").unwrap_or_default();
        assert_eq!(stmts.len(), 1, "IF should parse to one statement");
        assert!(
            to_common_sql(&stmts[0]).is_some(),
            "IF must pass through as Some (DialectSpecific), not None"
        );
    }

    // T6 (#158): CREATE TABLE は T2.3 で変換先 CreateTable variant を持つようになった。
    // §0.5 parity 反転: 変換可能な DDL は Some(CreateTable) となり None-filter でドロップ
    // されない。CREATE VIEW / PROCEDURE 等、依然として変換先なしの DDL は None のままで、
    // これが convert_to の "Statement contains unsupported features" エラー経路の根拠。
    #[cfg(not(feature = "wasm"))]
    #[test]
    fn test_to_common_sql_create_table_now_maps_to_some() {
        use tsql_parser::ast::to_common_sql::to_common_sql;

        // CREATE TABLE は T2.3 で CreateTable variant に変換される → Some
        let stmts = tsql_parser::parse("CREATE TABLE t (id INT)").unwrap_or_default();
        if let Some(stmt) = stmts.first() {
            assert!(
                to_common_sql(stmt).is_some(),
                "CREATE TABLE should now map to Some(CreateTable) after T2.3"
            );
        }

        // CREATE VIEW は依然として変換先なし → None (エラー経路の根拠は維持)
        let stmts = tsql_parser::parse("CREATE VIEW v AS SELECT 1 AS x").unwrap_or_default();
        if let Some(stmt) = stmts.first() {
            assert!(
                to_common_sql(stmt).is_none(),
                "CREATE VIEW should still map to None (no destination variant)"
            );
        }
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_get_version() {
        let version = get_version();
        assert!(!version.is_empty());
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_convert_to_postgresql_simple_select() {
        let input = "SELECT * FROM users";
        let result = convert_to(input, TargetDialect::PostgreSQL);

        // JSONをパースして結果を検証
        let result_str = result.as_string().unwrap();
        assert!(result_str.contains(r#""status":"success""#) || result_str.contains("Success"));
        assert!(result_str.contains("SELECT"));
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_convert_to_postgresql_with_where() {
        let input = "SELECT id, name FROM users WHERE id = 1";
        let result = convert_to(input, TargetDialect::PostgreSQL);

        let result_str = result.as_string().unwrap();
        assert!(result_str.contains("WHERE"));
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_convert_to_mysql_simple_select() {
        let input = "SELECT * FROM users";
        let result = convert_to(input, TargetDialect::MySQL);

        // JSONをパースして結果を検証
        let result_str = result.as_string().unwrap();
        assert!(result_str.contains(r#""status":"success""#) || result_str.contains("Success"));
        assert!(result_str.contains("SELECT"));
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_convert_to_mysql_with_where() {
        let input = "SELECT id, name FROM users WHERE id = 1";
        let result = convert_to(input, TargetDialect::MySQL);

        let result_str = result.as_string().unwrap();
        assert!(result_str.contains("WHERE"));
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_convert_to_invalid_sql() {
        let input = "INVALID SQL HERE";
        let result = convert_to(input, TargetDialect::PostgreSQL);

        let result_str = result.as_string().unwrap();
        // パースエラーが返るはず
        assert!(result_str.contains("Error") || result_str.contains("error"));
    }

    // T6 (#158): DialectSpecific パススルー検証。
    // DECLARE 等、to_common_sql (T3) が None に変換する T-SQL 制御構文を
    // PostgreSQL 変換にかけた際、None-filter で暗黙にドロップして空の成功結果を
    // 返してはならない。convert_to は "Statement contains unsupported features"
    // エラーを返さなければならない (サイレントな情報消失の防止)。
    #[cfg(feature = "wasm")]
    #[test]
    fn test_convert_to_postgresql_dialect_specific_not_silently_dropped() {
        // DECLARE @v INT はパース成功するが to_common_sql は None を返す
        // (T3 pinned lossy mapping: DialectSpecific → None)。
        let input = "DECLARE @v INT";
        let result = convert_to(input, TargetDialect::PostgreSQL);

        let result_str = result.as_string().unwrap();
        // 成功ステータスでなく、unsupported features エラーであること
        assert!(
            !result_str.contains(r#""status":"success""#) && !result_str.contains("Success"),
            "DialectSpecific 文がサイレントにドロップされ成功結果になりました: {result_str}"
        );
        assert!(
            result_str.contains("unsupported features"),
            "unsupported features エラーが返るべきです: {result_str}"
        );
    }

    // T6 (#158): 同一パススルー契約の SQLite 方言検証。
    // PostgreSQL 以外の方言でも、None-filter によるサイレントドロップが起きないこと。
    #[cfg(feature = "wasm")]
    #[test]
    fn test_convert_to_sqlite_dialect_specific_not_silently_dropped() {
        let input = "IF 1 = 1 SELECT 1";
        let result = convert_to(input, TargetDialect::SQLite);

        let result_str = result.as_string().unwrap();
        assert!(
            !result_str.contains(r#""status":"success""#) && !result_str.contains("Success"),
            "DialectSpecific 文がサイレントにドロップされ成功結果になりました: {result_str}"
        );
        assert!(
            result_str.contains("unsupported features"),
            "unsupported features エラーが返るべきです: {result_str}"
        );
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_convert_to_sqlite_simple_select() {
        let input = "SELECT * FROM users";
        let result = convert_to(input, TargetDialect::SQLite);

        // JSONをパースして結果を検証
        let result_str = result.as_string().unwrap();
        assert!(result_str.contains(r#""status":"success""#) || result_str.contains("Success"));
        assert!(result_str.contains("SELECT"));
    }

    #[cfg(feature = "wasm")]
    #[test]
    fn test_convert_to_sqlite_with_where() {
        let input = "SELECT id, name FROM users WHERE id = 1";
        let result = convert_to(input, TargetDialect::SQLite);

        let result_str = result.as_string().unwrap();
        assert!(result_str.contains("WHERE"));
    }
}
