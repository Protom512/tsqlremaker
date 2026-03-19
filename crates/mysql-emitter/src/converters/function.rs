//! 関数のコンバーター
//!
//! T-SQL の関数を MySQL 関数に変換します。

use crate::{EmitError, MySqlEmitter};
use tsql_parser::common::{CommonExpression, CommonIdentifier};

/// 関数コンバーター
///
/// T-SQL の組込関数を MySQL 関数に変換します。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionConverter;

impl FunctionConverter {
    /// 関数呼び出しを MySQL 関数に変換
    ///
    /// # Arguments
    ///
    /// * `name` - 関数名
    /// * `args` - 引数リスト
    /// * `distinct` - DISTINCT指定があるか
    /// * `emitter` - Emitter（式を文字列化するために使用）
    ///
    /// # Returns
    ///
    /// MySQL 関数呼び出し文字列、またはエラー
    pub fn convert_function(
        name: &CommonIdentifier,
        args: &[CommonExpression],
        distinct: bool,
        emitter: &mut MySqlEmitter,
    ) -> Result<String, EmitError> {
        let func_name = name.name.to_uppercase();

        // 特殊な変換が必要な関数
        match func_name.as_str() {
            "DATEADD" => {
                if args.len() >= 3 {
                    return Self::convert_dateadd(args, emitter);
                }
                return Err(EmitError::UnsupportedFunction {
                    function_name: format!("DATEADD with {} args", args.len()),
                });
            }
            "DATEDIFF" => {
                if args.len() >= 3 {
                    return Self::convert_datediff(args, emitter);
                }
                return Err(EmitError::UnsupportedFunction {
                    function_name: format!("DATEDIFF with {} args", args.len()),
                });
            }
            "GETDATE" | "GETUTCDATE" | "LEN" | "CHARINDEX" | "REPLICATE" | "ISNULL" | "NEWID"
            | "CEILING" | "POWER" => {
                // 単純な名前マッピング
                let mapped_name = Self::map_function_name(&func_name).ok_or_else(|| {
                    EmitError::UnsupportedFunction {
                        function_name: func_name.clone(),
                    }
                })?;

                let args_str = args
                    .iter()
                    .map(|arg| emitter.visit_expression(arg))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");

                return Ok(format!("{}({})", mapped_name, args_str));
            }
            _ => {
                // 名前マッピングを試みる
                if let Some(mapped) = Self::map_function_name(&func_name) {
                    let args_str = args
                        .iter()
                        .map(|arg| emitter.visit_expression(arg))
                        .collect::<Result<Vec<_>, _>>()?
                        .join(", ");

                    return Ok(format!("{}({})", mapped, args_str));
                }
            }
        }

        // デフォルト: 元の関数名を使用（引数があれば）
        let args_str = args
            .iter()
            .map(|arg| emitter.visit_expression(arg))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");

        let distinct_str = if distinct { "DISTINCT " } else { "" };
        Ok(format!("{}{}({})", distinct_str, name.name, args_str))
    }

    /// 関数名を MySQL 関数名にマッピング
    ///
    /// # Returns
    ///
    /// マッピングされた関数名、または None（マッピングなし）
    fn map_function_name(name: &str) -> Option<&'static str> {
        match name {
            "GETDATE" => Some("NOW"),
            "GETUTCDATE" => Some("UTC_TIMESTAMP"),
            "LEN" => Some("LENGTH"),
            "CHARINDEX" => Some("LOCATE"),
            "REPLICATE" => Some("REPEAT"),
            "ISNULL" => Some("IFNULL"),
            "NEWID" => Some("UUID"),
            "CEILING" => Some("CEIL"),
            "POWER" => Some("POW"),
            _ => None,
        }
    }

    /// DATEADD を DATE_ADD に変換
    ///
    /// T-SQL: DATEADD(part, n, date)
    /// MySQL: DATE_ADD(date, INTERVAL n part)
    fn convert_dateadd(
        args: &[CommonExpression],
        emitter: &mut MySqlEmitter,
    ) -> Result<String, EmitError> {
        // args[0]: datepart, args[1]: number, args[2]: date
        if args.len() < 3 {
            return Err(EmitError::UnsupportedFunction {
                function_name: "DATEADD (requires 3 arguments)".to_string(),
            });
        }

        let datepart = emitter.visit_expression(&args[0])?;
        let number = emitter.visit_expression(&args[1])?;
        let date = emitter.visit_expression(&args[2])?;

        // datepart の文字列を解析
        let part = Self::convert_datepart(&datepart);

        Ok(format!("DATE_ADD({}, INTERVAL {} {})", date, number, part))
    }

    /// DATEDIFF を変換
    ///
    /// T-SQL: DATEDIFF(part, start, end)
    /// MySQL: DATEDIFF(end, start) (注意: 引数順が逆)
    fn convert_datediff(
        args: &[CommonExpression],
        emitter: &mut MySqlEmitter,
    ) -> Result<String, EmitError> {
        if args.len() < 3 {
            return Err(EmitError::UnsupportedFunction {
                function_name: "DATEDIFF (requires 3 arguments)".to_string(),
            });
        }

        let _part = emitter.visit_expression(&args[0])?; // MySQL では使用しない
        let start = emitter.visit_expression(&args[1])?;
        let end = emitter.visit_expression(&args[2])?;

        Ok(format!("DATEDIFF({}, {})", end, start))
    }

    /// T-SQL の datepart を MySQL の interval に変換
    fn convert_datepart(datepart: &str) -> &'static str {
        match datepart.to_uppercase().as_str() {
            "YEAR" | "YYYY" | "YY" => "YEAR",
            "MONTH" | "MM" | "M" => "MONTH",
            "DAY" | "DD" | "D" => "DAY",
            "HOUR" | "HH" => "HOUR",
            "MINUTE" | "MI" | "N" => "MINUTE",
            "SECOND" | "SS" | "S" => "SECOND",
            _ => "DAY", // デフォルト
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsql_parser::common::{CommonExpression, CommonIdentifier, CommonLiteral};

    fn create_identifier(name: &str) -> CommonIdentifier {
        CommonIdentifier {
            name: name.to_string(),
        }
    }

    fn create_literal_str(s: &str) -> CommonExpression {
        CommonExpression::Literal(CommonLiteral::String(s.to_string()))
    }

    fn create_literal_int(n: i64) -> CommonExpression {
        CommonExpression::Literal(CommonLiteral::Integer(n))
    }

    #[test]
    fn test_map_getdate_to_now() {
        assert_eq!(FunctionConverter::map_function_name("GETDATE"), Some("NOW"));
    }

    #[test]
    fn test_map_getutcdate_to_utc_timestamp() {
        assert_eq!(
            FunctionConverter::map_function_name("GETUTCDATE"),
            Some("UTC_TIMESTAMP")
        );
    }

    #[test]
    fn test_map_len_to_length() {
        assert_eq!(FunctionConverter::map_function_name("LEN"), Some("LENGTH"));
    }

    #[test]
    fn test_map_charindex_to_locate() {
        assert_eq!(
            FunctionConverter::map_function_name("CHARINDEX"),
            Some("LOCATE")
        );
    }

    #[test]
    fn test_map_replicate_to_repeat() {
        assert_eq!(
            FunctionConverter::map_function_name("REPLICATE"),
            Some("REPEAT")
        );
    }

    #[test]
    fn test_map_isnull_to_ifnull() {
        assert_eq!(
            FunctionConverter::map_function_name("ISNULL"),
            Some("IFNULL")
        );
    }

    #[test]
    fn test_map_newid_to_uuid() {
        assert_eq!(FunctionConverter::map_function_name("NEWID"), Some("UUID"));
    }

    #[test]
    fn test_map_ceiling_to_ceil() {
        assert_eq!(
            FunctionConverter::map_function_name("CEILING"),
            Some("CEIL")
        );
    }

    #[test]
    fn test_map_power_to_pow() {
        assert_eq!(FunctionConverter::map_function_name("POWER"), Some("POW"));
    }

    #[test]
    fn test_map_unknown_function_returns_none() {
        assert_eq!(FunctionConverter::map_function_name("UNKNOWN_FUNC"), None);
    }

    #[test]
    fn test_convert_datepart_year() {
        assert_eq!(FunctionConverter::convert_datepart("YEAR"), "YEAR");
        assert_eq!(FunctionConverter::convert_datepart("YYYY"), "YEAR");
        assert_eq!(FunctionConverter::convert_datepart("YY"), "YEAR");
    }

    #[test]
    fn test_convert_datepart_month() {
        assert_eq!(FunctionConverter::convert_datepart("MONTH"), "MONTH");
        assert_eq!(FunctionConverter::convert_datepart("MM"), "MONTH");
    }

    #[test]
    fn test_convert_datepart_day() {
        assert_eq!(FunctionConverter::convert_datepart("DAY"), "DAY");
        assert_eq!(FunctionConverter::convert_datepart("DD"), "DAY");
    }

    #[test]
    fn test_convert_datepart_hour() {
        assert_eq!(FunctionConverter::convert_datepart("HOUR"), "HOUR");
        assert_eq!(FunctionConverter::convert_datepart("HH"), "HOUR");
    }

    #[test]
    fn test_convert_datepart_minute() {
        assert_eq!(FunctionConverter::convert_datepart("MINUTE"), "MINUTE");
        assert_eq!(FunctionConverter::convert_datepart("MI"), "MINUTE");
    }

    #[test]
    fn test_convert_datepart_second() {
        assert_eq!(FunctionConverter::convert_datepart("SECOND"), "SECOND");
        assert_eq!(FunctionConverter::convert_datepart("SS"), "SECOND");
    }

    #[test]
    fn test_convert_datepart_default() {
        assert_eq!(FunctionConverter::convert_datepart("UNKNOWN"), "DAY");
    }
}
