//! PostgreSQL データ型マッパー
//!
//! Common SQL DataType を PostgreSQL のデータ型文字列に変換します。

use crate::EmitError;
use common_sql::ast::DataType;

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
    pub fn map(data_type: &DataType) -> Result<String, EmitError> {
        Ok(match data_type {
            // 整数型
            // Note: PostgreSQL doesn't have TINYINT, use SMALLINT instead
            DataType::TinyInt => "SMALLINT".to_string(),
            DataType::SmallInt => "SMALLINT".to_string(),
            DataType::Int => "INTEGER".to_string(),
            DataType::BigInt => "BIGINT".to_string(),

            // 小数型
            // Note: common-sql `DataType` には `Float` バリアントが存在しないため、
            // 従来の FLOAT マッピングは削除された。浮動小数点は Real / DoublePrecision 経由。
            DataType::Decimal { precision, scale } => Self::format_decimal(*precision, *scale),
            DataType::Numeric { precision, scale } => Self::format_decimal(*precision, *scale),
            DataType::Real => "REAL".to_string(),
            DataType::DoublePrecision => "DOUBLE PRECISION".to_string(),

            // 文字列型
            DataType::Char { length } => Self::format_char("CHAR", *length),
            DataType::VarChar { length } => Self::format_varchar(*length),
            DataType::Text => "TEXT".to_string(),
            DataType::NChar { length } => Self::format_char("CHAR", *length),
            DataType::NVarChar { length } => Self::format_varchar(*length),
            // PostgreSQL は Unicode ネイティブ対応のため NTEXT → TEXT に正規化
            DataType::NText => "TEXT".to_string(),

            // 日時型
            DataType::Date => "DATE".to_string(),
            DataType::Time { precision } => Self::format_time("TIME", *precision),
            DataType::DateTime { precision } => Self::format_time("TIMESTAMP", *precision),
            DataType::Timestamp { precision } => Self::format_time("TIMESTAMP", *precision),

            // バイナリ型
            DataType::Binary { length } => Self::format_binary("BYTEA", *length),
            DataType::VarBinary { length } => Self::format_varbinary(*length),
            DataType::Blob => "BYTEA".to_string(),

            // その他
            DataType::Boolean => "BOOLEAN".to_string(),
            DataType::Uuid => "UUID".to_string(),
            DataType::Json => "JSONB".to_string(),
        })
    }

    /// DECIMAL/NUMERIC 型をフォーマット
    fn format_decimal(precision: Option<u8>, scale: Option<u8>) -> String {
        match (precision, scale) {
            (Some(p), Some(s)) => format!("NUMERIC({p},{s})"),
            (Some(p), None) => format!("NUMERIC({p})"),
            (None, _) => "NUMERIC".to_string(),
        }
    }

    /// CHAR/NCHAR 型をフォーマット
    fn format_char(base: &str, length: Option<u64>) -> String {
        match length {
            Some(n) => format!("{base}({n})"),
            None => base.to_string(),
        }
    }

    /// VARCHAR 型をフォーマット
    fn format_varchar(length: Option<u64>) -> String {
        match length {
            Some(n) => format!("VARCHAR({n})"),
            None => "VARCHAR".to_string(),
        }
    }

    /// TIME/TIMESTAMP 型をフォーマット
    fn format_time(base: &str, precision: Option<u8>) -> String {
        match precision {
            Some(p) => format!("{base}({p})"),
            None => base.to_string(),
        }
    }

    /// BINARY 型をフォーマット
    fn format_binary(base: &str, length: Option<u64>) -> String {
        match length {
            Some(n) => format!("{base}({n})"),
            None => base.to_string(),
        }
    }

    /// VARBINARY 型をフォーマット
    fn format_varbinary(length: Option<u64>) -> String {
        match length {
            Some(n) => format!("BYTEA({n})"),
            None => "BYTEA".to_string(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // 整数型のテスト
    #[test]
    fn test_map_tinyint() {
        // PostgreSQLにはTINYINT型がないため、TINYINTはSMALLINTにマップされる
        assert_eq!(DataTypeMapper::map(&DataType::TinyInt).unwrap(), "SMALLINT");
    }

    #[test]
    fn test_map_smallint() {
        assert_eq!(
            DataTypeMapper::map(&DataType::SmallInt).unwrap(),
            "SMALLINT"
        );
    }

    #[test]
    fn test_map_int() {
        assert_eq!(DataTypeMapper::map(&DataType::Int).unwrap(), "INTEGER");
    }

    #[test]
    fn test_map_bigint() {
        assert_eq!(DataTypeMapper::map(&DataType::BigInt).unwrap(), "BIGINT");
    }

    // 小数型のテスト
    #[test]
    fn test_map_decimal_default() {
        assert_eq!(
            DataTypeMapper::map(&DataType::Decimal {
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
            DataTypeMapper::map(&DataType::Decimal {
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
            DataTypeMapper::map(&DataType::Decimal {
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
            DataTypeMapper::map(&DataType::Numeric {
                precision: Some(8),
                scale: Some(0)
            })
            .unwrap(),
            "NUMERIC(8,0)"
        );
    }

    #[test]
    fn test_map_real() {
        assert_eq!(DataTypeMapper::map(&DataType::Real).unwrap(), "REAL");
    }

    #[test]
    fn test_map_double_precision() {
        assert_eq!(
            DataTypeMapper::map(&DataType::DoublePrecision).unwrap(),
            "DOUBLE PRECISION"
        );
    }

    #[test]
    fn test_map_ntext_to_text() {
        // PostgreSQL は Unicode ネイティブ対応のため NTEXT → TEXT に正規化される
        assert_eq!(DataTypeMapper::map(&DataType::NText).unwrap(), "TEXT");
    }

    #[test]
    fn test_map_ntext_distinct_from_blob() {
        // NTEXT は TEXT へ、BLOB は BYTEA へ — 異なる PostgreSQL 型にマップされる
        assert_ne!(
            DataTypeMapper::map(&DataType::NText).unwrap(),
            DataTypeMapper::map(&DataType::Blob).unwrap()
        );
    }

    // 文字列型のテスト
    #[test]
    fn test_map_char_default() {
        assert_eq!(
            DataTypeMapper::map(&DataType::Char { length: None }).unwrap(),
            "CHAR"
        );
    }

    #[test]
    fn test_map_char_with_length() {
        assert_eq!(
            DataTypeMapper::map(&DataType::Char { length: Some(10) }).unwrap(),
            "CHAR(10)"
        );
    }

    #[test]
    fn test_map_varchar_default() {
        assert_eq!(
            DataTypeMapper::map(&DataType::VarChar { length: None }).unwrap(),
            "VARCHAR"
        );
    }

    #[test]
    fn test_map_varchar_with_length() {
        assert_eq!(
            DataTypeMapper::map(&DataType::VarChar { length: Some(255) }).unwrap(),
            "VARCHAR(255)"
        );
    }

    #[test]
    fn test_map_text() {
        assert_eq!(DataTypeMapper::map(&DataType::Text).unwrap(), "TEXT");
    }

    #[test]
    fn test_map_nchar_to_char() {
        assert_eq!(
            DataTypeMapper::map(&DataType::NChar { length: Some(10) }).unwrap(),
            "CHAR(10)"
        );
    }

    #[test]
    fn test_map_nvarchar_to_varchar() {
        assert_eq!(
            DataTypeMapper::map(&DataType::NVarChar { length: Some(255) }).unwrap(),
            "VARCHAR(255)"
        );
    }

    // 日時型のテスト
    #[test]
    fn test_map_date() {
        assert_eq!(DataTypeMapper::map(&DataType::Date).unwrap(), "DATE");
    }

    #[test]
    fn test_map_time_default() {
        assert_eq!(
            DataTypeMapper::map(&DataType::Time { precision: None }).unwrap(),
            "TIME"
        );
    }

    #[test]
    fn test_map_time_with_precision() {
        assert_eq!(
            DataTypeMapper::map(&DataType::Time { precision: Some(6) }).unwrap(),
            "TIME(6)"
        );
    }

    #[test]
    fn test_map_datetime_to_timestamp() {
        assert_eq!(
            DataTypeMapper::map(&DataType::DateTime { precision: None }).unwrap(),
            "TIMESTAMP"
        );
    }

    #[test]
    fn test_map_datetime_with_precision() {
        assert_eq!(
            DataTypeMapper::map(&DataType::DateTime { precision: Some(3) }).unwrap(),
            "TIMESTAMP(3)"
        );
    }

    #[test]
    fn test_map_timestamp_default() {
        assert_eq!(
            DataTypeMapper::map(&DataType::Timestamp { precision: None }).unwrap(),
            "TIMESTAMP"
        );
    }

    #[test]
    fn test_map_timestamp_with_precision() {
        assert_eq!(
            DataTypeMapper::map(&DataType::Timestamp { precision: Some(6) }).unwrap(),
            "TIMESTAMP(6)"
        );
    }

    // バイナリ型のテスト
    #[test]
    fn test_map_binary_default() {
        assert_eq!(
            DataTypeMapper::map(&DataType::Binary { length: None }).unwrap(),
            "BYTEA"
        );
    }

    #[test]
    fn test_map_binary_with_length() {
        assert_eq!(
            DataTypeMapper::map(&DataType::Binary { length: Some(16) }).unwrap(),
            "BYTEA(16)"
        );
    }

    #[test]
    fn test_map_varbinary_default() {
        assert_eq!(
            DataTypeMapper::map(&DataType::VarBinary { length: None }).unwrap(),
            "BYTEA"
        );
    }

    #[test]
    fn test_map_varbinary_with_length() {
        assert_eq!(
            DataTypeMapper::map(&DataType::VarBinary { length: Some(255) }).unwrap(),
            "BYTEA(255)"
        );
    }

    #[test]
    fn test_map_blob_to_bytea() {
        assert_eq!(DataTypeMapper::map(&DataType::Blob).unwrap(), "BYTEA");
    }

    // その他の型のテスト
    #[test]
    fn test_map_boolean() {
        assert_eq!(DataTypeMapper::map(&DataType::Boolean).unwrap(), "BOOLEAN");
    }

    #[test]
    fn test_map_uuid() {
        assert_eq!(DataTypeMapper::map(&DataType::Uuid).unwrap(), "UUID");
    }

    #[test]
    fn test_map_json_to_jsonb() {
        assert_eq!(DataTypeMapper::map(&DataType::Json).unwrap(), "JSONB");
    }
}
