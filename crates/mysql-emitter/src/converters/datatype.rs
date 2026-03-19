//! データ型のコンバーター
//!
//! Common SQL AST のデータ型を MySQL データ型に変換します。

use tsql_parser::common::CommonDataType;

/// DataType コンバーター
///
/// Common SQL AST のデータ型を MySQL データ型文字列に変換します。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct DataTypeConverter;

#[allow(dead_code)]
impl DataTypeConverter {
    /// データ型を MySQL データ型文字列に変換
    ///
    /// # Arguments
    ///
    /// * `data_type` - Common SQL AST のデータ型
    ///
    /// # Returns
    ///
    /// MySQL データ型文字列
    #[must_use]
    pub fn convert(data_type: &CommonDataType) -> String {
        match data_type {
            CommonDataType::TinyInt => "TINYINT".to_string(),
            CommonDataType::SmallInt => "SMALLINT".to_string(),
            CommonDataType::Int => "INT".to_string(),
            CommonDataType::BigInt => "BIGINT".to_string(),
            CommonDataType::Decimal { precision, scale } => {
                Self::format_decimal(*precision, *scale)
            }
            CommonDataType::Numeric { precision, scale } => {
                Self::format_decimal(*precision, *scale)
            }
            CommonDataType::Real => "DOUBLE".to_string(),
            CommonDataType::DoublePrecision => "DOUBLE".to_string(),
            CommonDataType::Float { precision } => {
                if let Some(p) = precision {
                    format!("FLOAT({})", p)
                } else {
                    "FLOAT".to_string()
                }
            }
            CommonDataType::Char { length } => Self::format_char("CHAR", *length),
            CommonDataType::VarChar { length } => Self::format_varchar(*length),
            CommonDataType::Text => "TEXT".to_string(),
            CommonDataType::NChar { length } => Self::format_char("CHAR", *length),
            CommonDataType::NVarChar { length } => Self::format_varchar(*length),
            CommonDataType::Date => "DATE".to_string(),
            CommonDataType::Time { precision } => Self::format_time("TIME", *precision),
            CommonDataType::DateTime { precision } => Self::format_time("DATETIME", *precision),
            CommonDataType::Timestamp { precision } => Self::format_time("TIMESTAMP", *precision),
            CommonDataType::Binary { length } => Self::format_binary("BINARY", *length),
            CommonDataType::VarBinary { length } => Self::format_binary("VARBINARY", *length),
            CommonDataType::Blob => "BLOB".to_string(),
            CommonDataType::Boolean => "TINYINT(1)".to_string(),
            CommonDataType::Uuid => "CHAR(36)".to_string(),
            CommonDataType::Json => "JSON".to_string(),
        }
    }

    /// DECIMAL/NUMERIC 型のパラメータをフォーマット
    fn format_decimal(precision: Option<u8>, scale: Option<u8>) -> String {
        match (precision, scale) {
            (Some(p), Some(s)) => format!("DECIMAL({},{})", p, s),
            (Some(p), None) => format!("DECIMAL({})", p),
            (None, _) => "DECIMAL".to_string(),
        }
    }

    /// CHAR/NCHAR 型のパラメータをフォーマット
    fn format_char(base: &str, length: Option<u64>) -> String {
        match length {
            Some(n) => format!("{}({})", base, n),
            None => base.to_string(),
        }
    }

    /// VARCHAR/NVARCHAR 型のパラメータをフォーマット
    fn format_varchar(length: Option<u64>) -> String {
        match length {
            Some(n) => format!("VARCHAR({})", n),
            None => "VARCHAR(255)".to_string(), // MySQL デフォルト
        }
    }

    /// TIME/DATETIME/TIMESTAMP 型のパラメータをフォーマット
    fn format_time(base: &str, precision: Option<u8>) -> String {
        match precision {
            Some(p) => format!("{}({})", base, p),
            None => base.to_string(),
        }
    }

    /// BINARY/VARBINARY 型のパラメータをフォーマット
    fn format_binary(base: &str, length: Option<u64>) -> String {
        match length {
            Some(n) => format!("{}({})", base, n),
            None => base.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_tinyint() {
        let result = DataTypeConverter::convert(&CommonDataType::TinyInt);
        assert_eq!(result, "TINYINT");
    }

    #[test]
    fn test_convert_smallint() {
        let result = DataTypeConverter::convert(&CommonDataType::SmallInt);
        assert_eq!(result, "SMALLINT");
    }

    #[test]
    fn test_convert_int() {
        let result = DataTypeConverter::convert(&CommonDataType::Int);
        assert_eq!(result, "INT");
    }

    #[test]
    fn test_convert_bigint() {
        let result = DataTypeConverter::convert(&CommonDataType::BigInt);
        assert_eq!(result, "BIGINT");
    }

    #[test]
    fn test_convert_decimal_with_precision_and_scale() {
        let dt = CommonDataType::Decimal {
            precision: Some(10),
            scale: Some(2),
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL(10,2)");
    }

    #[test]
    fn test_convert_decimal_with_precision_only() {
        let dt = CommonDataType::Decimal {
            precision: Some(10),
            scale: None,
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL(10)");
    }

    #[test]
    fn test_convert_decimal_default() {
        let dt = CommonDataType::Decimal {
            precision: None,
            scale: None,
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL");
    }

    #[test]
    fn test_convert_numeric_to_decimal() {
        let dt = CommonDataType::Numeric {
            precision: Some(8),
            scale: Some(4),
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL(8,4)");
    }

    #[test]
    fn test_convert_real() {
        let result = DataTypeConverter::convert(&CommonDataType::Real);
        assert_eq!(result, "DOUBLE");
    }

    #[test]
    fn test_convert_double_precision() {
        let result = DataTypeConverter::convert(&CommonDataType::DoublePrecision);
        assert_eq!(result, "DOUBLE");
    }

    #[test]
    fn test_convert_float_with_precision() {
        let dt = CommonDataType::Float {
            precision: Some(10),
        };
        assert_eq!(DataTypeConverter::convert(&dt), "FLOAT(10)");
    }

    #[test]
    fn test_convert_float_default() {
        let dt = CommonDataType::Float { precision: None };
        assert_eq!(DataTypeConverter::convert(&dt), "FLOAT");
    }

    #[test]
    fn test_convert_char_with_length() {
        let dt = CommonDataType::Char { length: Some(10) };
        assert_eq!(DataTypeConverter::convert(&dt), "CHAR(10)");
    }

    #[test]
    fn test_convert_char_default() {
        let dt = CommonDataType::Char { length: None };
        assert_eq!(DataTypeConverter::convert(&dt), "CHAR");
    }

    #[test]
    fn test_convert_varchar_with_length() {
        let dt = CommonDataType::VarChar { length: Some(255) };
        assert_eq!(DataTypeConverter::convert(&dt), "VARCHAR(255)");
    }

    #[test]
    fn test_convert_varchar_default() {
        let dt = CommonDataType::VarChar { length: None };
        assert_eq!(DataTypeConverter::convert(&dt), "VARCHAR(255)");
    }

    #[test]
    fn test_convert_text() {
        let result = DataTypeConverter::convert(&CommonDataType::Text);
        assert_eq!(result, "TEXT");
    }

    #[test]
    fn test_convert_nchar_to_char() {
        let dt = CommonDataType::NChar { length: Some(20) };
        assert_eq!(DataTypeConverter::convert(&dt), "CHAR(20)");
    }

    #[test]
    fn test_convert_nvarchar_to_varchar() {
        let dt = CommonDataType::NVarChar { length: Some(100) };
        assert_eq!(DataTypeConverter::convert(&dt), "VARCHAR(100)");
    }

    #[test]
    fn test_convert_date() {
        let result = DataTypeConverter::convert(&CommonDataType::Date);
        assert_eq!(result, "DATE");
    }

    #[test]
    fn test_convert_time_with_precision() {
        let dt = CommonDataType::Time { precision: Some(3) };
        assert_eq!(DataTypeConverter::convert(&dt), "TIME(3)");
    }

    #[test]
    fn test_convert_time_default() {
        let dt = CommonDataType::Time { precision: None };
        assert_eq!(DataTypeConverter::convert(&dt), "TIME");
    }

    #[test]
    fn test_convert_datetime_with_precision() {
        let dt = CommonDataType::DateTime { precision: Some(6) };
        assert_eq!(DataTypeConverter::convert(&dt), "DATETIME(6)");
    }

    #[test]
    fn test_convert_datetime_default() {
        let dt = CommonDataType::DateTime { precision: None };
        assert_eq!(DataTypeConverter::convert(&dt), "DATETIME");
    }

    #[test]
    fn test_convert_timestamp_with_precision() {
        let dt = CommonDataType::Timestamp { precision: Some(3) };
        assert_eq!(DataTypeConverter::convert(&dt), "TIMESTAMP(3)");
    }

    #[test]
    fn test_convert_timestamp_default() {
        let dt = CommonDataType::Timestamp { precision: None };
        assert_eq!(DataTypeConverter::convert(&dt), "TIMESTAMP");
    }

    #[test]
    fn test_convert_binary_with_length() {
        let dt = CommonDataType::Binary { length: Some(16) };
        assert_eq!(DataTypeConverter::convert(&dt), "BINARY(16)");
    }

    #[test]
    fn test_convert_binary_default() {
        let dt = CommonDataType::Binary { length: None };
        assert_eq!(DataTypeConverter::convert(&dt), "BINARY");
    }

    #[test]
    fn test_convert_varbinary_with_length() {
        let dt = CommonDataType::VarBinary { length: Some(256) };
        assert_eq!(DataTypeConverter::convert(&dt), "VARBINARY(256)");
    }

    #[test]
    fn test_convert_varbinary_default() {
        let dt = CommonDataType::VarBinary { length: None };
        assert_eq!(DataTypeConverter::convert(&dt), "VARBINARY");
    }

    #[test]
    fn test_convert_blob() {
        let result = DataTypeConverter::convert(&CommonDataType::Blob);
        assert_eq!(result, "BLOB");
    }

    #[test]
    fn test_convert_boolean_to_tinyint() {
        let result = DataTypeConverter::convert(&CommonDataType::Boolean);
        assert_eq!(result, "TINYINT(1)");
    }

    #[test]
    fn test_convert_uuid_to_char36() {
        let result = DataTypeConverter::convert(&CommonDataType::Uuid);
        assert_eq!(result, "CHAR(36)");
    }

    #[test]
    fn test_convert_json() {
        let result = DataTypeConverter::convert(&CommonDataType::Json);
        assert_eq!(result, "JSON");
    }
}
