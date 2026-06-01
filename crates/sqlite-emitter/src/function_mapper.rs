//! SQLite 関数マッパー
//!
//! T-SQL 関数を SQLite 関数にマッピングします。
//! 日付関数 (DATEADD, DATEDIFF) の datepart 変換も含みます。

/// T-SQL 関数名を SQLite 関数名にマッピングする。
///
/// 大文字小文字を区別しない。マッピングがない場合は `None` を返す。
///
/// # Arguments
///
/// * `name` - T-SQL 関数名（大文字）
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
