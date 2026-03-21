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
    use tsql_parser::ToCommonAst;

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

    // Common SQL AST に変換
    let common_stmts: Vec<_> = stmts
        .iter()
        .filter_map(|stmt| stmt.to_common_ast())
        .collect();

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
