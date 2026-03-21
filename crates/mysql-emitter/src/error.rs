//! MySQL Emitter のエラー型

use std::fmt;

/// Emitter のエラー型
///
/// SQL 生成中に発生するエラーを表します。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitError {
    /// サポートされていない文
    UnsupportedStatement {
        /// 文の種類
        statement_type: String,
    },
    /// サポートされていない式
    UnsupportedExpression {
        /// 式の種類
        expression_type: String,
    },
    /// サポートされていないデータ型
    UnsupportedDataType {
        /// データ型
        data_type: String,
    },
    /// サポートされていない関数
    UnsupportedFunction {
        /// 関数名
        function_name: String,
    },
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedStatement { statement_type } => {
                write!(f, "Unsupported statement type: {}", statement_type)
            }
            Self::UnsupportedExpression { expression_type } => {
                write!(f, "Unsupported expression type: {}", expression_type)
            }
            Self::UnsupportedDataType { data_type } => {
                write!(f, "Unsupported data type: {}", data_type)
            }
            Self::UnsupportedFunction { function_name } => {
                write!(f, "Unsupported function: {}", function_name)
            }
        }
    }
}

impl std::error::Error for EmitError {}
