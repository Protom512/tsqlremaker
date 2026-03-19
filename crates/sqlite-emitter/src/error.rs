//! SQLite Emitter のエラー型

use thiserror::Error;

/// SQLite Emitter のエラー型
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EmitError {
    /// サポートされていないステートメントまたは構文
    #[error("Unsupported: {0}")]
    Unsupported(String),

    /// サポートされていないデータ型
    #[error("Unsupported data type: {0:?}")]
    UnsupportedDataType(String),

    /// サポートされていない関数
    #[error("Unsupported function: {0}")]
    UnsupportedFunction(String),

    /// 構文エラー
    #[error("Syntax error: {message}")]
    SyntaxError {
        /// エラーメッセージ
        message: String,
    },
}

/// SQLite Emitter の Result 型エイリアス
#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, EmitError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = EmitError::Unsupported("DECLARE statement".to_string());
        assert_eq!(format!("{}", err), "Unsupported: DECLARE statement");
    }

    #[test]
    fn test_unsupported_data_type() {
        let err = EmitError::UnsupportedDataType("sql_variant".to_string());
        assert_eq!(format!("{}", err), "Unsupported data type: \"sql_variant\"");
    }

    #[test]
    fn test_unsupported_function() {
        let err = EmitError::UnsupportedFunction("TSQL_CUSTOM".to_string());
        assert_eq!(format!("{}", err), "Unsupported function: TSQL_CUSTOM");
    }

    #[test]
    fn test_syntax_error() {
        let err = EmitError::SyntaxError {
            message: "unexpected token".to_string(),
        };
        assert_eq!(format!("{}", err), "Syntax error: unexpected token");
    }

    #[test]
    fn test_error_equality() {
        let err1 = EmitError::Unsupported("test".to_string());
        let err2 = EmitError::Unsupported("test".to_string());
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_error_inequality() {
        let err1 = EmitError::Unsupported("test1".to_string());
        let err2 = EmitError::Unsupported("test2".to_string());
        assert_ne!(err1, err2);
    }
}
