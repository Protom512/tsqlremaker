//! データ型のコンバーター
//!
//! Common SQL AST の [`DataType`] を MySQL のデータ型文字列に変換します。
//!
//! 設計仕様 (`.kiro/specs/mysql-emitter/design.md` の "DataType Mapping" テーブル)
//! に基づき、24 パターンすべてをカバーします。

use common_sql::ast::DataType;

/// データ型コンバーター
///
/// Common SQL の [`DataType`] を MySQL のデータ型文字列へ変換する
/// ステートレスなユニットコンバーターです。
///
/// 全 24 バリアントがサポートされており、[`convert`](Self::convert) は
/// 常に有効な MySQL データ型文字列を返します（`Result` ではありません）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataTypeConverter;

impl DataTypeConverter {
    /// [`DataType`] を MySQL のデータ型文字列に変換します。
    ///
    /// 全 24 バリアントをサポートしています:
    ///
    /// | Common SQL | MySQL |
    /// |------------|-------|
    /// | `TinyInt` | `TINYINT` |
    /// | `SmallInt` | `SMALLINT` |
    /// | `Int` | `INT` |
    /// | `BigInt` | `BIGINT` |
    /// | `Decimal { p, s }` | `DECIMAL(p,s)` |
    /// | `Numeric { p, s }` | `DECIMAL(p,s)` (MySQL では NUMERIC は DECIMAL の別名) |
    /// | `Real` | `DOUBLE` |
    /// | `DoublePrecision` | `DOUBLE` |
    /// | `Char { n }` | `CHAR(n)` |
    /// | `VarChar { n }` | `VARCHAR(n)` |
    /// | `Text` | `TEXT` |
    /// | `NChar { n }` | `CHAR(n)` (NATIONAL CHAR) |
    /// | `NVarChar { n }` | `VARCHAR(n)` |
    /// | `NText` | `LONGTEXT` |
    /// | `Date` | `DATE` |
    /// | `Time { p }` | `TIME(p)` |
    /// | `DateTime { p }` | `DATETIME(p)` |
    /// | `Timestamp { p }` | `TIMESTAMP(p)` |
    /// | `Binary { n }` | `BINARY(n)` |
    /// | `VarBinary { n }` | `VARBINARY(n)` |
    /// | `Blob` | `BLOB` |
    /// | `Boolean` | `TINYINT(1)` |
    /// | `Uuid` | `CHAR(36)` |
    /// | `Json` | `JSON` |
    #[must_use]
    pub fn convert(data_type: &DataType) -> String {
        match data_type {
            // Integer types
            DataType::TinyInt => "TINYINT".to_string(),
            DataType::SmallInt => "SMALLINT".to_string(),
            DataType::Int => "INT".to_string(),
            DataType::BigInt => "BIGINT".to_string(),

            // Decimal / float types
            DataType::Decimal { precision, scale } => {
                format!("DECIMAL{}", Self::format_params(*precision, *scale))
            }
            // MySQL では NUMERIC は DECIMAL の別名のため DECIMAL に正規化
            DataType::Numeric { precision, scale } => {
                format!("DECIMAL{}", Self::format_params(*precision, *scale))
            }
            DataType::Real => "DOUBLE".to_string(),
            DataType::DoublePrecision => "DOUBLE".to_string(),

            // String types
            DataType::Char { length } => format!("CHAR({})", Self::format_length(*length)),
            DataType::VarChar { length } => format!("VARCHAR({})", Self::format_length(*length)),
            DataType::Text => "TEXT".to_string(),
            // MySQL の CHAR は Unicode をネイティブ対応するため NChar -> CHAR に正規化
            DataType::NChar { length } => format!("CHAR({})", Self::format_length(*length)),
            DataType::NVarChar { length } => format!("VARCHAR({})", Self::format_length(*length)),
            DataType::NText => "LONGTEXT".to_string(),

            // Date/time types
            DataType::Date => "DATE".to_string(),
            DataType::Time { precision } => format!("TIME{}", Self::format_precision(*precision)),
            DataType::DateTime { precision } => {
                format!("DATETIME{}", Self::format_precision(*precision))
            }
            DataType::Timestamp { precision } => {
                format!("TIMESTAMP{}", Self::format_precision(*precision))
            }

            // Binary types
            DataType::Binary { length } => format!("BINARY({})", Self::format_length(*length)),
            DataType::VarBinary { length } => {
                format!("VARBINARY({})", Self::format_length(*length))
            }
            DataType::Blob => "BLOB".to_string(),

            // Other
            DataType::Boolean => "TINYINT(1)".to_string(),
            DataType::Uuid => "CHAR(36)".to_string(),
            DataType::Json => "JSON".to_string(),
        }
    }

    /// 精度・小数点以下桁数のパラメータ部分を `(p,s)` 形式でフォーマットします。
    ///
    /// - 両方指定: `(p,s)`
    /// - 精度のみ: `(p)`
    /// - どちらもなし: 空文字列（型名にパラメータを付与しない）
    ///
    /// `DECIMAL` / `NUMERIC` の両方で利用します。
    #[must_use]
    fn format_params(precision: Option<u8>, scale: Option<u8>) -> String {
        match (precision, scale) {
            (Some(p), Some(s)) => format!("({p},{s})"),
            (Some(p), None) => format!("({p})"),
            (None, _) => String::new(),
        }
    }

    /// 文字列/バイナリ長のパラメータ部分をフォーマットします。
    ///
    /// 長さが指定されている場合はその数値、未指定の場合は MySQL の
    /// デフォルト長（`1` for `CHAR`/`BINARY`）を用います。
    #[must_use]
    fn format_length(length: Option<u64>) -> String {
        // MySQL のデフォルト: CHAR(1), BINARY(1)。VARCHAR/VARBINARY は
        // 必須長だが、AST 上は Option なので既定値 1 を用いる。
        length.map_or_else(|| "1".to_string(), |n| n.to_string())
    }

    /// 時刻系の精度パラメータ部分をフォーマットします。
    ///
    /// 精度が指定されている場合は `(p)`、未指定の場合は空文字列
    /// （型名にパラメータを付与しない）を返します。
    #[must_use]
    fn format_precision(precision: Option<u8>) -> String {
        match precision {
            Some(p) => format!("({p})"),
            None => String::new(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::DataType;

    // ── Integer types ───────────────────────────────────

    #[test]
    fn tinyint_converts_to_tinyint() {
        assert_eq!(DataTypeConverter::convert(&DataType::TinyInt), "TINYINT");
    }

    #[test]
    fn smallint_converts_to_smallint() {
        assert_eq!(DataTypeConverter::convert(&DataType::SmallInt), "SMALLINT");
    }

    #[test]
    fn int_converts_to_int() {
        assert_eq!(DataTypeConverter::convert(&DataType::Int), "INT");
    }

    #[test]
    fn bigint_converts_to_bigint() {
        assert_eq!(DataTypeConverter::convert(&DataType::BigInt), "BIGINT");
    }

    // ── Decimal / float types ───────────────────────────

    #[test]
    fn decimal_with_precision_and_scale_converts() {
        let dt = DataType::Decimal {
            precision: Some(18),
            scale: Some(4),
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL(18,4)");
    }

    #[test]
    fn decimal_with_precision_only_converts() {
        let dt = DataType::Decimal {
            precision: Some(10),
            scale: None,
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL(10)");
    }

    #[test]
    fn decimal_without_params_converts_bare() {
        let dt = DataType::Decimal {
            precision: None,
            scale: None,
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL");
    }

    #[test]
    fn numeric_normalizes_to_decimal_with_both_params() {
        let dt = DataType::Numeric {
            precision: Some(38),
            scale: Some(10),
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL(38,10)");
    }

    #[test]
    fn numeric_with_precision_only_normalizes_to_decimal() {
        let dt = DataType::Numeric {
            precision: Some(20),
            scale: None,
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL(20)");
    }

    #[test]
    fn real_converts_to_double() {
        assert_eq!(DataTypeConverter::convert(&DataType::Real), "DOUBLE");
    }

    #[test]
    fn double_precision_converts_to_double() {
        assert_eq!(
            DataTypeConverter::convert(&DataType::DoublePrecision),
            "DOUBLE"
        );
    }

    // ── String types ────────────────────────────────────

    #[test]
    fn char_with_length_converts() {
        let dt = DataType::Char { length: Some(10) };
        assert_eq!(DataTypeConverter::convert(&dt), "CHAR(10)");
    }

    #[test]
    fn varchar_with_length_converts() {
        let dt = DataType::VarChar { length: Some(255) };
        assert_eq!(DataTypeConverter::convert(&dt), "VARCHAR(255)");
    }

    #[test]
    fn text_converts_to_text() {
        assert_eq!(DataTypeConverter::convert(&DataType::Text), "TEXT");
    }

    #[test]
    fn nchar_normalizes_to_char() {
        let dt = DataType::NChar { length: Some(50) };
        assert_eq!(DataTypeConverter::convert(&dt), "CHAR(50)");
    }

    #[test]
    fn nvarchar_normalizes_to_varchar() {
        let dt = DataType::NVarChar { length: Some(100) };
        assert_eq!(DataTypeConverter::convert(&dt), "VARCHAR(100)");
    }

    #[test]
    fn ntext_converts_to_longtext() {
        assert_eq!(DataTypeConverter::convert(&DataType::NText), "LONGTEXT");
    }

    // ── Date/time types ─────────────────────────────────

    #[test]
    fn date_converts_to_date() {
        assert_eq!(DataTypeConverter::convert(&DataType::Date), "DATE");
    }

    #[test]
    fn time_with_precision_converts() {
        let dt = DataType::Time { precision: Some(6) };
        assert_eq!(DataTypeConverter::convert(&dt), "TIME(6)");
    }

    #[test]
    fn datetime_with_precision_converts() {
        let dt = DataType::DateTime { precision: Some(3) };
        assert_eq!(DataTypeConverter::convert(&dt), "DATETIME(3)");
    }

    #[test]
    fn timestamp_with_precision_converts() {
        let dt = DataType::Timestamp { precision: Some(6) };
        assert_eq!(DataTypeConverter::convert(&dt), "TIMESTAMP(6)");
    }

    // ── Binary types ────────────────────────────────────

    #[test]
    fn binary_with_length_converts() {
        let dt = DataType::Binary { length: Some(16) };
        assert_eq!(DataTypeConverter::convert(&dt), "BINARY(16)");
    }

    #[test]
    fn varbinary_with_length_converts() {
        let dt = DataType::VarBinary { length: Some(1024) };
        assert_eq!(DataTypeConverter::convert(&dt), "VARBINARY(1024)");
    }

    #[test]
    fn blob_converts_to_blob() {
        assert_eq!(DataTypeConverter::convert(&DataType::Blob), "BLOB");
    }

    // ── Other types ─────────────────────────────────────

    #[test]
    fn boolean_converts_to_tinyint_1() {
        assert_eq!(DataTypeConverter::convert(&DataType::Boolean), "TINYINT(1)");
    }

    #[test]
    fn uuid_converts_to_char_36() {
        assert_eq!(DataTypeConverter::convert(&DataType::Uuid), "CHAR(36)");
    }

    #[test]
    fn json_converts_to_json() {
        assert_eq!(DataTypeConverter::convert(&DataType::Json), "JSON");
    }

    // ── Edge cases: parametrized None / defaults ─────────

    #[test]
    fn char_without_length_uses_default_1() {
        let dt = DataType::Char { length: None };
        assert_eq!(DataTypeConverter::convert(&dt), "CHAR(1)");
    }

    #[test]
    fn binary_without_length_uses_default_1() {
        let dt = DataType::Binary { length: None };
        assert_eq!(DataTypeConverter::convert(&dt), "BINARY(1)");
    }

    #[test]
    fn varchar_without_length_uses_default_1() {
        let dt = DataType::VarChar { length: None };
        assert_eq!(DataTypeConverter::convert(&dt), "VARCHAR(1)");
    }

    #[test]
    fn time_without_precision_is_bare() {
        let dt = DataType::Time { precision: None };
        assert_eq!(DataTypeConverter::convert(&dt), "TIME");
    }

    #[test]
    fn datetime_without_precision_is_bare() {
        let dt = DataType::DateTime { precision: None };
        assert_eq!(DataTypeConverter::convert(&dt), "DATETIME");
    }

    #[test]
    fn timestamp_without_precision_is_bare() {
        let dt = DataType::Timestamp { precision: None };
        assert_eq!(DataTypeConverter::convert(&dt), "TIMESTAMP");
    }

    #[test]
    fn numeric_without_params_normalizes_to_bare_decimal() {
        let dt = DataType::Numeric {
            precision: None,
            scale: None,
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL");
    }

    #[test]
    fn decimal_with_scale_only_omits_scale() {
        // scale without precision is invalid SQL; precision None => bare
        let dt = DataType::Decimal {
            precision: None,
            scale: Some(2),
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL");
    }

    #[test]
    fn zero_length_is_preserved() {
        let dt = DataType::Char { length: Some(0) };
        assert_eq!(DataTypeConverter::convert(&dt), "CHAR(0)");
    }

    #[test]
    fn zero_precision_and_scale_is_preserved() {
        let dt = DataType::Decimal {
            precision: Some(0),
            scale: Some(0),
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL(0,0)");
    }

    #[test]
    fn large_length_is_preserved() {
        let dt = DataType::VarChar {
            length: Some(u64::MAX),
        };
        assert_eq!(
            DataTypeConverter::convert(&dt),
            format!("VARCHAR({})", u64::MAX)
        );
    }

    #[test]
    fn max_u8_precision_is_preserved() {
        let dt = DataType::Decimal {
            precision: Some(u8::MAX),
            scale: Some(u8::MAX),
        };
        assert_eq!(DataTypeConverter::convert(&dt), "DECIMAL(255,255)");
    }

    // ── Exhaustiveness: all 24 variants covered ─────────
    // Guards against a newly added DataType variant slipping through
    // without a converter branch. The count must stay at 24.

    #[test]
    fn all_24_variants_produce_non_empty_output() {
        let all = vec![
            DataType::TinyInt,
            DataType::SmallInt,
            DataType::Int,
            DataType::BigInt,
            DataType::Decimal {
                precision: Some(18),
                scale: Some(4),
            },
            DataType::Numeric {
                precision: Some(10),
                scale: Some(2),
            },
            DataType::Real,
            DataType::DoublePrecision,
            DataType::Char { length: Some(10) },
            DataType::VarChar { length: Some(255) },
            DataType::Text,
            DataType::NChar { length: Some(50) },
            DataType::NVarChar { length: Some(100) },
            DataType::NText,
            DataType::Date,
            DataType::Time { precision: Some(6) },
            DataType::DateTime { precision: Some(3) },
            DataType::Timestamp { precision: Some(6) },
            DataType::Binary { length: Some(16) },
            DataType::VarBinary { length: Some(1024) },
            DataType::Blob,
            DataType::Boolean,
            DataType::Uuid,
            DataType::Json,
        ];
        assert_eq!(all.len(), 24, "DataType variant count changed; update test");
        for dt in &all {
            let out = DataTypeConverter::convert(dt);
            assert!(!out.is_empty(), "empty output for {dt:?}");
            assert!(
                !out.contains("unsupported"),
                "unsupported marker for {dt:?}: {out}"
            );
        }
    }

    // ── Unit struct behavior ────────────────────────────

    #[test]
    fn converter_is_zero_sized_unit_struct() {
        // Unit struct: zero-cost, Copy, equality holds between instances.
        let c1 = DataTypeConverter;
        let c2 = DataTypeConverter;
        assert_eq!(c1, c2);
        assert!(std::mem::size_of::<DataTypeConverter>() == 0);
    }

    #[test]
    fn convert_is_pure_no_state_dependency() {
        // Conversion depends only on the input DataType, not on any state.
        let dt = DataType::BigInt;
        let a = DataTypeConverter::convert(&dt);
        let b = DataTypeConverter::convert(&dt);
        assert_eq!(a, b);
        assert_eq!(a, "BIGINT");
    }
}
