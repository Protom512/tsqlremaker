//! 関数のコンバーター
//!
//! T-SQL の関数を SQLite 関数に変換します。
//!
//! このモジュールは [`common_sql::ast::Expression`] / [`common_sql::ast::Identifier`]
//! を入力として扱います。`mysql-emitter::converters::function` (function.rs:23) と
//! 対称な構造体ベースの [`FunctionConverter`] を定義します。
//!
//! ## 設計 (Task 2 — Issue #75)
//!
//! 旧 [`crate::function_mapper`] が持っていた自由関数
//! (`map_function_name` / `map_datepart_to_modifier` / `is_time_datepart` /
//! `is_date_datepart`) を [`FunctionConverter`] の関連関数へ昇格させます。
//! これにより新規関数変換の追加が単一レジストリに集約され、MySQL emitter と
//! アーキテクチャが整合します。
//!
//! 引数の文字列化戦略は呼び出し側からクロージャで注入する設計 (MySQL の
//! `ArgStringifier` パターン) を採用します。これは `SqliteEmitter` 構造体の
//! 実装詳細 (`visit_expression`) との循環依存を回避し、コンバータを単体テスト
//! 可能にするためです (Task 3 で `convert_function` エントリポイント追加時に使用)。

/// SQLite 関数コンバーター
///
/// T-SQL の組込関数を SQLite 関数に変換します。
///
/// この構造体は状態を持たない (zero-field) ため、[`Copy`] / [`Clone`] が可能です。
/// MySQL emitter の `FunctionConverter` (converters/function.rs:23) と対称です。
///
/// # 例
///
/// ```
/// use sqlite_emitter::FunctionConverter;
///
/// assert_eq!(FunctionConverter::map_function_name("LEN"), Some("length"));
/// assert_eq!(FunctionConverter::map_function_name("ISNULL"), Some("ifnull"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionConverter;

impl FunctionConverter {
    /// T-SQL 関数名を SQLite 関数名にマッピングする。
    ///
    /// 大文字小文字を区別しない。マッピングがない場合は `None` を返す。
    ///
    /// # Arguments
    ///
    /// * `name` - T-SQL 関数名（大文字）
    #[must_use]
    pub fn map_function_name(name: &str) -> Option<&'static str> {
        match name {
            // 日付時刻関数
            "GETDATE" | "GETUTCDATE" => Some("datetime('now')"),
            "DATENAME" => Some("strftime"),
            "DATEPART" => Some("strftime"),

            // 文字列関数
            "LEN" => Some("length"),
            "CHARINDEX" => Some("instr"),
            "LEFT" => Some("substr"),
            "RIGHT" => Some("substr"),
            "REPLACE" => Some("replace"),
            "SUBSTRING" => Some("substr"),
            "LTRIM" => Some("ltrim"),
            "RTRIM" => Some("rtrim"),
            "TRIM" => Some("trim"),
            "UPPER" => Some("upper"),
            "LOWER" => Some("lower"),

            // 数学関数
            "ABS" => Some("abs"),
            "CEILING" => Some("ceil"),
            "FLOOR" => Some("floor"),
            "POWER" => Some("pow"),
            "ROUND" => Some("round"),
            "SQRT" => Some("sqrt"),

            // 集計関数（SQLite でも同じ名前、小文字化のみ）
            "COUNT" => Some("count"),
            "SUM" => Some("sum"),
            "AVG" => Some("avg"),
            "MIN" => Some("min"),
            "MAX" => Some("max"),

            // NULL 処理
            "ISNULL" => Some("ifnull"),
            "COALESCE" => Some("coalesce"),

            _ => None,
        }
    }

    /// T-SQL の datepart 文字列を SQLite の修飾子単位に変換する。
    ///
    /// DATEADD 用: 戻り値は `("unit", multiplier)`。
    /// 例: `"week"` → `("days", 7)`, `"quarter"` → `("months", 3)`, `"day"` → `("days", 1)`
    ///
    /// ミリ秒は SQLite でサポートされないため `None` を返す。
    #[must_use]
    pub fn map_datepart_to_modifier(datepart: &str) -> Option<(&'static str, i64)> {
        match datepart {
            "year" | "yyyy" | "yy" => Some(("years", 1)),
            "quarter" | "qq" | "q" => Some(("months", 3)),
            "month" | "mm" | "m" => Some(("months", 1)),
            "dayofyear" | "dy" | "y" => Some(("days", 1)),
            "day" | "dd" | "d" => Some(("days", 1)),
            "week" | "ww" | "wk" => Some(("days", 7)),
            "hour" | "hh" => Some(("hours", 1)),
            "minute" | "mi" | "n" => Some(("minutes", 1)),
            "second" | "ss" | "s" => Some(("seconds", 1)),
            "millisecond" | "ms" => None,
            _ => None,
        }
    }

    /// 時刻を含む datepart かどうかを判定する。
    ///
    /// `true` の場合、SQLite で `datetime()` を使用すべき。
    #[must_use]
    pub fn is_time_datepart(datepart: &str) -> bool {
        matches!(
            datepart,
            "hour" | "hh" | "minute" | "mi" | "n" | "second" | "ss" | "s"
        )
    }

    /// datepart が日付ベースの差分計算でサポートされるかどうか。
    ///
    /// DATEDIFF 用: 時間ベース (hour/minute/second/millisecond) は
    /// julianday では精度が不十分なためエラーとする。
    #[must_use]
    pub fn is_date_datepart(datepart: &str) -> bool {
        matches!(
            datepart,
            "year"
                | "yyyy"
                | "yy"
                | "quarter"
                | "qq"
                | "q"
                | "month"
                | "mm"
                | "m"
                | "dayofyear"
                | "dy"
                | "y"
                | "day"
                | "dd"
                | "d"
                | "week"
                | "ww"
                | "wk"
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::clone_on_copy)]
mod tests {
    use super::*;
    // レガシー自由関数との等価性を検証するため、shim 経由で旧 API を参照する。
    use crate::function_mapper;

    // ============================================================
    // map_function_name
    // ============================================================

    #[test]
    fn test_map_function_name_string_functions() {
        assert_eq!(FunctionConverter::map_function_name("LEN"), Some("length"));
        assert_eq!(
            FunctionConverter::map_function_name("CHARINDEX"),
            Some("instr")
        );
        assert_eq!(
            FunctionConverter::map_function_name("SUBSTRING"),
            Some("substr")
        );
        assert_eq!(
            FunctionConverter::map_function_name("REPLACE"),
            Some("replace")
        );
    }

    #[test]
    fn test_map_function_name_datetime_functions() {
        assert_eq!(
            FunctionConverter::map_function_name("GETDATE"),
            Some("datetime('now')")
        );
        assert_eq!(
            FunctionConverter::map_function_name("GETUTCDATE"),
            Some("datetime('now')")
        );
    }

    #[test]
    fn test_map_function_name_aggregate_functions() {
        assert_eq!(FunctionConverter::map_function_name("COUNT"), Some("count"));
        assert_eq!(FunctionConverter::map_function_name("SUM"), Some("sum"));
        assert_eq!(FunctionConverter::map_function_name("AVG"), Some("avg"));
        assert_eq!(FunctionConverter::map_function_name("MIN"), Some("min"));
        assert_eq!(FunctionConverter::map_function_name("MAX"), Some("max"));
    }

    #[test]
    fn test_map_function_name_null_handling() {
        assert_eq!(
            FunctionConverter::map_function_name("ISNULL"),
            Some("ifnull")
        );
        assert_eq!(
            FunctionConverter::map_function_name("COALESCE"),
            Some("coalesce")
        );
    }

    #[test]
    fn test_map_function_name_math_functions() {
        assert_eq!(FunctionConverter::map_function_name("ABS"), Some("abs"));
        assert_eq!(
            FunctionConverter::map_function_name("CEILING"),
            Some("ceil")
        );
        assert_eq!(FunctionConverter::map_function_name("FLOOR"), Some("floor"));
        assert_eq!(FunctionConverter::map_function_name("POWER"), Some("pow"));
        assert_eq!(FunctionConverter::map_function_name("ROUND"), Some("round"));
        assert_eq!(FunctionConverter::map_function_name("SQRT"), Some("sqrt"));
    }

    #[test]
    fn test_map_function_name_unknown() {
        assert_eq!(FunctionConverter::map_function_name("UNKNOWN_FUNC"), None);
    }

    // ============================================================
    // map_datepart_to_modifier
    // ============================================================

    #[test]
    fn test_map_datepart_to_modifier_basic() {
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("day"),
            Some(("days", 1))
        );
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("week"),
            Some(("days", 7))
        );
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("quarter"),
            Some(("months", 3))
        );
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("year"),
            Some(("years", 1))
        );
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("month"),
            Some(("months", 1))
        );
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("hour"),
            Some(("hours", 1))
        );
    }

    #[test]
    fn test_map_datepart_to_modifier_abbreviations() {
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("yy"),
            Some(("years", 1))
        );
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("qq"),
            Some(("months", 3))
        );
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("mi"),
            Some(("minutes", 1))
        );
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("ss"),
            Some(("seconds", 1))
        );
    }

    #[test]
    fn test_map_datepart_millisecond_unsupported() {
        assert_eq!(
            FunctionConverter::map_datepart_to_modifier("millisecond"),
            None
        );
        assert_eq!(FunctionConverter::map_datepart_to_modifier("ms"), None);
    }

    #[test]
    fn test_map_datepart_unknown() {
        assert_eq!(FunctionConverter::map_datepart_to_modifier("unknown"), None);
    }

    // ============================================================
    // is_time_datepart
    // ============================================================

    #[test]
    fn test_is_time_datepart_true() {
        assert!(FunctionConverter::is_time_datepart("hour"));
        assert!(FunctionConverter::is_time_datepart("hh"));
        assert!(FunctionConverter::is_time_datepart("minute"));
        assert!(FunctionConverter::is_time_datepart("mi"));
        assert!(FunctionConverter::is_time_datepart("n"));
        assert!(FunctionConverter::is_time_datepart("second"));
        assert!(FunctionConverter::is_time_datepart("ss"));
        assert!(FunctionConverter::is_time_datepart("s"));
    }

    #[test]
    fn test_is_time_datepart_false() {
        assert!(!FunctionConverter::is_time_datepart("day"));
        assert!(!FunctionConverter::is_time_datepart("year"));
        assert!(!FunctionConverter::is_time_datepart("week"));
        assert!(!FunctionConverter::is_time_datepart("millisecond"));
    }

    // ============================================================
    // is_date_datepart
    // ============================================================

    #[test]
    fn test_is_date_datepart_true() {
        assert!(FunctionConverter::is_date_datepart("day"));
        assert!(FunctionConverter::is_date_datepart("year"));
        assert!(FunctionConverter::is_date_datepart("week"));
        assert!(FunctionConverter::is_date_datepart("quarter"));
        assert!(FunctionConverter::is_date_datepart("month"));
        assert!(FunctionConverter::is_date_datepart("dayofyear"));
    }

    #[test]
    fn test_is_date_datepart_false() {
        assert!(!FunctionConverter::is_date_datepart("hour"));
        assert!(!FunctionConverter::is_date_datepart("minute"));
        assert!(!FunctionConverter::is_date_datepart("second"));
        assert!(!FunctionConverter::is_date_datepart("millisecond"));
    }

    // ============================================================
    // 構造体特性 (MySQL FunctionConverter と対称)
    // ============================================================

    #[test]
    fn test_function_converter_is_copy_clone_zero_field() {
        let a = FunctionConverter;
        let b = a; // Copy
        let _c = b.clone(); // Clone
                            // zero-field なので等価
        assert_eq!(a, b);
        // Debug 導出確認 (panic しない)
        let _ = format!("{a:?}");
    }

    // ============================================================
    // レガシー自由関数との等価性 (振る舞い保存の回帰網)
    // T2 は function_mapper.rs を shim として残すため、両者が一致することを保証。
    // ============================================================

    #[test]
    fn test_parity_map_function_name_with_legacy() {
        let cases = [
            "GETDATE",
            "GETUTCDATE",
            "DATENAME",
            "DATEPART",
            "LEN",
            "CHARINDEX",
            "ISNULL",
            "COALESCE",
            "COUNT",
            "CEILING",
            "SQRT",
            "UNKNOWN_FUNC",
            "",
        ];
        for name in cases {
            assert_eq!(
                FunctionConverter::map_function_name(name),
                function_mapper::map_function_name(name),
                "divergence for map_function_name({name:?})"
            );
        }
    }

    #[test]
    fn test_parity_map_datepart_to_modifier_with_legacy() {
        let cases = [
            "year",
            "yyyy",
            "yy",
            "quarter",
            "qq",
            "q",
            "month",
            "mm",
            "m",
            "dayofyear",
            "dy",
            "y",
            "day",
            "dd",
            "d",
            "week",
            "ww",
            "wk",
            "hour",
            "hh",
            "minute",
            "mi",
            "n",
            "second",
            "ss",
            "s",
            "millisecond",
            "ms",
            "unknown",
            "",
        ];
        for dp in cases {
            assert_eq!(
                FunctionConverter::map_datepart_to_modifier(dp),
                function_mapper::map_datepart_to_modifier(dp),
                "divergence for map_datepart_to_modifier({dp:?})"
            );
        }
    }

    #[test]
    fn test_parity_is_time_datepart_with_legacy() {
        let cases = [
            "hour",
            "hh",
            "minute",
            "mi",
            "n",
            "second",
            "ss",
            "s",
            "day",
            "year",
            "millisecond",
            "",
        ];
        for dp in cases {
            assert_eq!(
                FunctionConverter::is_time_datepart(dp),
                function_mapper::is_time_datepart(dp),
                "divergence for is_time_datepart({dp:?})"
            );
        }
    }

    #[test]
    fn test_parity_is_date_datepart_with_legacy() {
        let cases = [
            "year",
            "yyyy",
            "yy",
            "quarter",
            "qq",
            "q",
            "month",
            "mm",
            "m",
            "dayofyear",
            "dy",
            "y",
            "day",
            "dd",
            "d",
            "week",
            "ww",
            "wk",
            "hour",
            "minute",
            "second",
            "millisecond",
            "",
        ];
        for dp in cases {
            assert_eq!(
                FunctionConverter::is_date_datepart(dp),
                function_mapper::is_date_datepart(dp),
                "divergence for is_date_datepart({dp:?})"
            );
        }
    }
}
