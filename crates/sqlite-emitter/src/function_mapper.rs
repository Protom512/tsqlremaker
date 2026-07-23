//! SQLite 関数マッパー (レガシー互換 shim)
//!
//! T-SQL 関数を SQLite 関数にマッピングします。
//! 日付関数 (DATEADD, DATEDIFF) の datepart 変換も含みます。
//!
//! ## Issue #75 / Task 2 — 構造体ベースコンバータへの移行
//!
//! 旧実装の自由関数は [`crate::converters::FunctionConverter`] の関連関数へ
//! 昇格されました。本モジュールは後方互換性のため、各自由関数を
//! [`FunctionConverter`] の対応メソッドへの thin delegation として保持します。
//! これにより `lib.rs` の既存呼び出し元 (Task 3 で委譲予定) と、既存の
//! `function_mapper` 単体テストを壊さずに移行を進められます。
//!
//! 新規コードは [`crate::converters::FunctionConverter`] を直接使用してください。

use crate::converters::FunctionConverter;

/// T-SQL 関数名を SQLite 関数名にマッピングする。
///
/// 大文字小文字を区別しない。マッピングがない場合は `None` を返す。
///
/// # Arguments
///
/// * `name` - T-SQL 関数名（大文字）
///
/// # Deprecation
///
/// 後方互換のため残置されています。新規コードは
/// [`FunctionConverter::map_function_name`] を使用してください。
#[must_use]
pub fn map_function_name(name: &str) -> Option<&'static str> {
    FunctionConverter::map_function_name(name)
}

/// T-SQL の datepart 文字列を SQLite の修飾子単位に変換する。
///
/// DATEADD 用: 戻り値は `("unit", multiplier)`。
/// 例: `"week"` → `("days", 7)`, `"quarter"` → `("months", 3)`, `"day"` → `("days", 1)`
///
/// ミリ秒は SQLite でサポートされないため `None` を返す。
///
/// # Deprecation
///
/// 後方互換のため残置されています。新規コードは
/// [`FunctionConverter::map_datepart_to_modifier`] を使用してください。
#[must_use]
pub fn map_datepart_to_modifier(datepart: &str) -> Option<(&'static str, i64)> {
    FunctionConverter::map_datepart_to_modifier(datepart)
}

/// 時刻を含む datepart かどうかを判定する。
///
/// `true` の場合、SQLite で `datetime()` を使用すべき。
///
/// # Deprecation
///
/// 後方互換のため残置されています。新規コードは
/// [`FunctionConverter::is_time_datepart`] を使用してください。
#[must_use]
pub fn is_time_datepart(datepart: &str) -> bool {
    FunctionConverter::is_time_datepart(datepart)
}

/// datepart が日付ベースの差分計算でサポートされるかどうか。
///
/// DATEDIFF 用: 時間ベース (hour/minute/second/millisecond) は
/// julianday では精度が不十分なためエラーとする。
///
/// # Deprecation
///
/// 後方互換のため残置されています。新規コードは
/// [`FunctionConverter::is_date_datepart`] を使用してください。
#[must_use]
pub fn is_date_datepart(datepart: &str) -> bool {
    FunctionConverter::is_date_datepart(datepart)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_map_function_name_string_functions() {
        assert_eq!(map_function_name("LEN"), Some("length"));
        assert_eq!(map_function_name("CHARINDEX"), Some("instr"));
        assert_eq!(map_function_name("SUBSTRING"), Some("substr"));
        assert_eq!(map_function_name("REPLACE"), Some("replace"));
    }

    #[test]
    fn test_map_function_name_datetime_functions() {
        assert_eq!(map_function_name("GETDATE"), Some("datetime('now')"));
        assert_eq!(map_function_name("GETUTCDATE"), Some("datetime('now')"));
    }

    #[test]
    fn test_map_function_name_aggregate_functions() {
        assert_eq!(map_function_name("COUNT"), Some("count"));
        assert_eq!(map_function_name("SUM"), Some("sum"));
        assert_eq!(map_function_name("AVG"), Some("avg"));
    }

    #[test]
    fn test_map_function_name_null_handling() {
        assert_eq!(map_function_name("ISNULL"), Some("ifnull"));
        assert_eq!(map_function_name("COALESCE"), Some("coalesce"));
    }

    #[test]
    fn test_map_function_name_unknown() {
        assert_eq!(map_function_name("UNKNOWN_FUNC"), None);
    }

    #[test]
    fn test_map_datepart_to_modifier() {
        assert_eq!(map_datepart_to_modifier("day"), Some(("days", 1)));
        assert_eq!(map_datepart_to_modifier("week"), Some(("days", 7)));
        assert_eq!(map_datepart_to_modifier("quarter"), Some(("months", 3)));
        assert_eq!(map_datepart_to_modifier("year"), Some(("years", 1)));
    }

    #[test]
    fn test_map_datepart_millisecond_unsupported() {
        assert_eq!(map_datepart_to_modifier("millisecond"), None);
    }

    #[test]
    fn test_map_datepart_unknown() {
        assert_eq!(map_datepart_to_modifier("unknown"), None);
    }

    #[test]
    fn test_is_time_datepart() {
        assert!(is_time_datepart("hour"));
        assert!(is_time_datepart("hh"));
        assert!(is_time_datepart("minute"));
        assert!(is_time_datepart("second"));
        assert!(!is_time_datepart("day"));
        assert!(!is_time_datepart("year"));
    }

    #[test]
    fn test_is_date_datepart() {
        assert!(is_date_datepart("day"));
        assert!(is_date_datepart("year"));
        assert!(is_date_datepart("week"));
        assert!(!is_date_datepart("hour"));
        assert!(!is_date_datepart("millisecond"));
    }
}
