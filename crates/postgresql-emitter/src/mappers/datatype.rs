//! PostgreSQL データ型マッパー
//!
//! Common SQL DataType を PostgreSQL のデータ型文字列に変換します。

use crate::EmitError;
use tsql_parser::common::CommonDataType;

/// PostgreSQL データ型マッパー
#[derive(Debug, Clone, Copy)]
pub struct DataTypeMapper;

impl DataTypeMapper {
    /// Common SQL DataType を PostgreSQL 型文字列に変換
    ///
    /// # Arguments
    ///
    /// * `data_type` - Common SQL データ型
    ///
    /// # Returns
    ///
    /// PostgreSQL データ型文字列
    ///
    /// # Errors
    ///
    /// サポートされていないデータ型の場合はエラーを返す
    pub fn map(data_type: &CommonDataType) -> Result<String, EmitError> {
        Ok(match data_type {
            // 整数型
            CommonDataType::TinyInt => "TINYINT".to_string(),
            CommonDataType::SmallInt => "SMALLINT".to_string(),
            CommonDataType::Int => "INTEGER".to_string(),
            CommonDataType::BigInt => "BIGINT".to_string(),

            // 小数型
            CommonDataType::Decimal { precision, scale } => {
                Self::format_decimal(*precision, *scale)
            }
            CommonDataType::Numeric { precision, scale } => {
                Self::format_decimal(*precision, *scale)
            }
            CommonDataType::Real => "REAL".to_string(),
            CommonDataType::DoublePrecision => "DOUBLE PRECISION".to_string(),
            CommonDataType::Float { precision } => {
                if let Some(p) = precision {
                    format!("FLOAT({})", p)
                } else {
                    "FLOAT".to_string()
                }
            }

            // 文字列型
            CommonDataType::Char { length } => Self::format_char("CHAR", *length),
            CommonDataType::VarChar { length } => Self::format_varchar(*length),
            CommonDataType::Text => "TEXT".to_string(),
            CommonDataType::NChar { length } => Self::format_char("CHAR", *length),
            CommonDataType::NVarChar { length } => Self::format_varchar(*length),

            // 日時型
            CommonDataType::Date => "DATE".to_string(),
            CommonDataType::Time { precision } => Self::format_time("TIME", *precision),
            CommonDataType::DateTime { precision } => Self::format_time("TIMESTAMP", *precision),
            CommonDataType::Timestamp { precision } => Self::format_time("TIMESTAMP", *precision),

            // バイナリ型
            CommonDataType::Binary { length } => Self::format_binary("BYTEA", *length),
            CommonDataType::VarBinary { length } => Self::format_varbinary(*length),
            CommonDataType::Blob => "BYTEA".to_string(),

            // その他
            CommonDataType::Boolean => "BOOLEAN".to_string(),
            CommonDataType::Uuid => "UUID".to_string(),
            CommonDataType::Json => "JSONB".to_string(),
        })
    }

    /// DECIMAL/NUMERIC 型をフォーマット
    fn format_decimal(precision: Option<u8>, scale: Option<u8>) -> String {
        match (precision, scale) {
            (Some(p), Some(s)) => format!("NUMERIC({},{})", p, s),
            (Some(p), None) => format!("NUMERIC({})", p),
            (None, _) => "NUMERIC".to_string(),
        }
    }

    /// CHAR/NCHAR 型をフォーマット
    fn format_char(base: &str, length: Option<u64>) -> String {
        match length {
            Some(n) => format!("{}({})", base, n),
            None => base.to_string(),
        }
    }

    /// VARCHAR 型をフォーマット
    fn format_varchar(length: Option<u64>) -> String {
        match length {
            Some(n) => format!("VARCHAR({})", n),
            None => "VARCHAR".to_string(),
        }
    }

    /// TIME/TIMESTAMP 型をフォーマット
    fn format_time(base: &str, precision: Option<u8>) -> String {
        match precision {
            Some(p) => format!("{}({})", base, p),
            None => base.to_string(),
        }
    }

    /// BINARY 型をフォーマット
    fn format_binary(base: &str, length: Option<u64>) -> String {
        match length {
            Some(n) => format!("{}({})", base, n),
            None => base.to_string(),
        }
    }

    /// VARBINARY 型をフォーマット
    fn format_varbinary(length: Option<u64>) -> String {
        match length {
            Some(n) => format!("BYTEA({})", n),
            None => "BYTEA".to_string(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // 整数型のテスト
    #[test]
    fn test_map_tinyint() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::TinyInt).unwrap(),
            "TINYINT"
        );
    }

    #[test]
    fn test_map_smallint() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::SmallInt).unwrap(),
            "SMALLINT"
        );
    }

    #[test]
    fn test_map_int() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Int).unwrap(),
            "INTEGER"
        );
    }

    #[test]
    fn test_map_bigint() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::BigInt).unwrap(),
            "BIGINT"
        );
    }

    // 小数型のテスト
    #[test]
    fn test_map_decimal_default() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Decimal {
                precision: None,
                scale: None
            })
            .unwrap(),
            "NUMERIC"
        );
    }

    #[test]
    fn test_map_decimal_with_precision() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Decimal {
                precision: Some(10),
                scale: None
            })
            .unwrap(),
            "NUMERIC(10)"
        );
    }

    #[test]
    fn test_map_decimal_with_precision_and_scale() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Decimal {
                precision: Some(10),
                scale: Some(2)
            })
            .unwrap(),
            "NUMERIC(10,2)"
        );
    }

    #[test]
    fn test_map_numeric_to_decimal() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Numeric {
                precision: Some(8),
                scale: Some(0)
            })
            .unwrap(),
            "NUMERIC(8,0)"
        );
    }

    #[test]
    fn test_map_real() {
        assert_eq!(DataTypeMapper::map(&CommonDataType::Real).unwrap(), "REAL");
    }

    #[test]
    fn test_map_double_precision() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::DoublePrecision).unwrap(),
            "DOUBLE PRECISION"
        );
    }

    #[test]
    fn test_map_float_default() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Float { precision: None }).unwrap(),
            "FLOAT"
        );
    }

    #[test]
    fn test_map_float_with_precision() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Float {
                precision: Some(53)
            })
            .unwrap(),
            "FLOAT(53)"
        );
    }

    // 文字列型のテスト
    #[test]
    fn test_map_char_default() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Char { length: None }).unwrap(),
            "CHAR"
        );
    }

    #[test]
    fn test_map_char_with_length() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Char { length: Some(10) }).unwrap(),
            "CHAR(10)"
        );
    }

    #[test]
    fn test_map_varchar_default() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::VarChar { length: None }).unwrap(),
            "VARCHAR"
        );
    }

    #[test]
    fn test_map_varchar_with_length() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::VarChar { length: Some(255) }).unwrap(),
            "VARCHAR(255)"
        );
    }

    #[test]
    fn test_map_text() {
        assert_eq!(DataTypeMapper::map(&CommonDataType::Text).unwrap(), "TEXT");
    }

    #[test]
    fn test_map_nchar_to_char() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::NChar { length: Some(10) }).unwrap(),
            "CHAR(10)"
        );
    }

    #[test]
    fn test_map_nvarchar_to_varchar() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::NVarChar { length: Some(255) }).unwrap(),
            "VARCHAR(255)"
        );
    }

    // 日時型のテスト
    #[test]
    fn test_map_date() {
        assert_eq!(DataTypeMapper::map(&CommonDataType::Date).unwrap(), "DATE");
    }

    #[test]
    fn test_map_time_default() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Time { precision: None }).unwrap(),
            "TIME"
        );
    }

    #[test]
    fn test_map_time_with_precision() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Time { precision: Some(6) }).unwrap(),
            "TIME(6)"
        );
    }

    #[test]
    fn test_map_datetime_to_timestamp() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::DateTime { precision: None }).unwrap(),
            "TIMESTAMP"
        );
    }

    #[test]
    fn test_map_datetime_with_precision() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::DateTime { precision: Some(3) }).unwrap(),
            "TIMESTAMP(3)"
        );
    }

    #[test]
    fn test_map_timestamp_default() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Timestamp { precision: None }).unwrap(),
            "TIMESTAMP"
        );
    }

    #[test]
    fn test_map_timestamp_with_precision() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Timestamp { precision: Some(6) }).unwrap(),
            "TIMESTAMP(6)"
        );
    }

    // バイナリ型のテスト
    #[test]
    fn test_map_binary_default() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Binary { length: None }).unwrap(),
            "BYTEA"
        );
    }

    #[test]
    fn test_map_binary_with_length() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Binary { length: Some(16) }).unwrap(),
            "BYTEA(16)"
        );
    }

    #[test]
    fn test_map_varbinary_default() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::VarBinary { length: None }).unwrap(),
            "BYTEA"
        );
    }

    #[test]
    fn test_map_varbinary_with_length() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::VarBinary { length: Some(255) }).unwrap(),
            "BYTEA(255)"
        );
    }

    #[test]
    fn test_map_blob_to_bytea() {
        assert_eq!(DataTypeMapper::map(&CommonDataType::Blob).unwrap(), "BYTEA");
    }

    // その他の型のテスト
    #[test]
    fn test_map_boolean() {
        assert_eq!(
            DataTypeMapper::map(&CommonDataType::Boolean).unwrap(),
            "BOOLEAN"
        );
    }

    #[test]
    fn test_map_uuid() {
        assert_eq!(DataTypeMapper::map(&CommonDataType::Uuid).unwrap(), "UUID");
    }

    #[test]
    fn test_map_json_to_jsonb() {
        assert_eq!(DataTypeMapper::map(&CommonDataType::Json).unwrap(), "JSONB");
    }
}
