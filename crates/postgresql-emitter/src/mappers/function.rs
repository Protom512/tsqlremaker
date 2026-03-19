//! PostgreSQL 関数マッパー
//!
//! T-SQL 関数を PostgreSQL 関数にマッピングします。

use crate::mappers::ExpressionEmitter;
use crate::EmitError;
use tsql_parser::common::{CommonExpression, CommonIdentifier};

/// PostgreSQL 関数マッパー
#[derive(Debug, Clone, Copy)]
pub struct FunctionMapper;

impl FunctionMapper {
    /// T-SQL 関数名を PostgreSQL 関数名にマッピング
    ///
    /// # Arguments
    ///
    /// * `name` - T-SQL 関数名（大文字）
    ///
    /// # Returns
    ///
    /// PostgreSQL 関数名、またはマッピングがない場合は None
    pub fn map_function_name(name: &str) -> Option<String> {
        match name {
            // 日時関数
            "GETDATE" => Some("CURRENT_TIMESTAMP".to_string()),
            "GETUTCDATE" => Some("(NOW() AT TIME ZONE 'UTC')".to_string()),

            // 文字列関数
            "LEN" => Some("LENGTH".to_string()),
            "CHARINDEX" => Some("STRPOS".to_string()),
            "LEFT" => Some("SUBSTRING".to_string()),
            "RIGHT" => Some("SUBSTRING".to_string()),
            "REPLICATE" => Some("REPEAT".to_string()),
            "STUFF" => Some("OVERLAY".to_string()),
            "REPLACE" => Some("REPLACE".to_string()),
            "LTRIM" => Some("LTRIM".to_string()),
            "RTRIM" => Some("RTRIM".to_string()),
            "SUBSTRING" => Some("SUBSTRING".to_string()),

            // NULL 関数
            "ISNULL" => Some("COALESCE".to_string()),

            // 数学関数
            "CEILING" => Some("CEIL".to_string()),
            "POWER" => Some("POWER".to_string()),
            "LOG" => Some("LN".to_string()),

            // 日付・時刻関数（DATEADD/DATEDIFF は別途処理）
            "DATEPART" => Some("DATE_PART".to_string()),
            "DATENAME" => Some("TO_CHAR".to_string()),

            // その他
            "NEWID" => Some("UUID_GENERATE_V4".to_string()),
            "GETANSINULL" => Some("NULL".to_string()),

            _ => None,
        }
    }

    /// 関数呼び出しを PostgreSQL 関数に変換
    ///
    /// # Arguments
    ///
    /// * `name` - 関数名
    /// * `args` - 引数リスト
    ///
    /// # Returns
    ///
    /// PostgreSQL 関数呼び出しの文字列
    ///
    /// # Errors
    ///
    /// サポートされていない関数の場合はエラーを返す
    pub fn map_function_call(name: &str, args: &[CommonExpression]) -> Result<String, EmitError> {
        match name {
            // DATEADD: DATEADD(part, n, date) → date + INTERVAL 'n part'
            "DATEADD" => Self::convert_dateadd(args),

            // DATEDIFF: DATEDIFF(part, start, end) → DATE_PART('part', end - start)
            "DATEDIFF" => Self::convert_datediff(args),

            // GETDATE: GETDATE() → CURRENT_TIMESTAMP
            "GETDATE" => Ok("CURRENT_TIMESTAMP".to_string()),

            // GETUTCDATE: GETUTCDATE() → (NOW() AT TIME ZONE 'UTC')
            "GETUTCDATE" => Ok("(NOW() AT TIME ZONE 'UTC')".to_string()),

            // LEN: LEN(s) → LENGTH(s)
            "LEN" if args.len() == 1 => {
                Ok(format!("LENGTH({})", ExpressionEmitter::emit(&args[0])))
            }

            // SUBSTRING: SUBSTRING(s, start, len) → SUBSTRING(s FROM start FOR len)
            "SUBSTRING" if args.len() == 3 => Ok(format!(
                "SUBSTRING({} FROM {} FOR {})",
                ExpressionEmitter::emit(&args[0]),
                ExpressionEmitter::emit(&args[1]),
                ExpressionEmitter::emit(&args[2])
            )),

            // CHARINDEX: CHARINDEX(find, text) → STRPOS(text, find)
            "CHARINDEX" if args.len() >= 2 => Ok(format!(
                "STRPOS({}, {})",
                ExpressionEmitter::emit(&args[1]),
                ExpressionEmitter::emit(&args[0])
            )),

            // ISNULL: ISNULL(expr, alt) → COALESCE(expr, alt)
            "ISNULL" if args.len() == 2 => Ok(format!(
                "COALESCE({}, {})",
                ExpressionEmitter::emit(&args[0]),
                ExpressionEmitter::emit(&args[1])
            )),

            // CEILING: CEILING(x) → CEIL(x)
            "CEILING" if args.len() == 1 => {
                Ok(format!("CEIL({})", ExpressionEmitter::emit(&args[0])))
            }

            // NEWID: NEWID() → UUID_GENERATE_V4() (uuid-ossp 拡張が必要)
            "NEWID" if args.is_empty() => Ok("UUID_GENERATE_V4()".to_string()),

            _ => {
                // マッピングがあれば使用
                if let Some(mapped) = Self::map_function_name(name) {
                    Ok(format!(
                        "{}({})",
                        mapped,
                        args.iter()
                            .map(ExpressionEmitter::emit)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                } else {
                    Err(EmitError::UnsupportedFunction(name.to_string()))
                }
            }
        }
    }

    /// DATEADD を変換
    fn convert_dateadd(args: &[CommonExpression]) -> Result<String, EmitError> {
        if args.len() != 3 {
            return Err(EmitError::SyntaxError {
                message: "DATEADD requires exactly 3 arguments".to_string(),
            });
        }

        // DATEADD(part, n, date) → date + INTERVAL 'n part'
        let part = match &args[0] {
            CommonExpression::Identifier(CommonIdentifier { name }) => name,
            _ => {
                return Err(EmitError::SyntaxError {
                    message: "DATEADD first argument must be an identifier".to_string(),
                })
            }
        };

        let n = ExpressionEmitter::emit(&args[1]);
        let date = ExpressionEmitter::emit(&args[2]);

        let postgres_part = match part.as_str() {
            "YEAR" | "YY" | "YYYY" => "years",
            "MONTH" | "MM" | "M" => "months",
            "DAY" | "DD" | "D" => "days",
            "HOUR" | "HH" => "hours",
            "MINUTE" | "MI" | "N" => "mins",
            "SECOND" | "SS" | "S" => "secs",
            _ => part,
        };

        Ok(format!("{} + INTERVAL '{} {}'", date, n, postgres_part))
    }

    /// DATEDIFF を変換
    fn convert_datediff(args: &[CommonExpression]) -> Result<String, EmitError> {
        if args.len() != 3 {
            return Err(EmitError::SyntaxError {
                message: "DATEDIFF requires exactly 3 arguments".to_string(),
            });
        }

        // DATEDIFF(part, start, end) → DATE_PART('part', end - start)
        let part = match &args[0] {
            CommonExpression::Identifier(CommonIdentifier { name }) => name,
            _ => {
                return Err(EmitError::SyntaxError {
                    message: "DATEDIFF first argument must be an identifier".to_string(),
                })
            }
        };

        let start = ExpressionEmitter::emit(&args[1]);
        let end = ExpressionEmitter::emit(&args[2]);

        let postgres_part = match part.as_str() {
            "YEAR" | "YY" | "YYYY" => "year",
            "MONTH" | "MM" | "M" => "month",
            "DAY" | "DD" | "D" => "day",
            "HOUR" | "HH" => "hour",
            "MINUTE" | "MI" | "N" => "minute",
            "SECOND" | "SS" | "S" => "second",
            _ => part,
        };

        Ok(format!(
            "DATE_PART('{}', {} - {})",
            postgres_part, end, start
        ))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::panic)]

    use super::*;
    use tsql_parser::common::CommonLiteral;

    #[test]
    fn test_map_getdate() {
        assert_eq!(
            FunctionMapper::map_function_name("GETDATE"),
            Some("CURRENT_TIMESTAMP".to_string())
        );
    }

    #[test]
    fn test_map_getutcdate() {
        assert_eq!(
            FunctionMapper::map_function_name("GETUTCDATE"),
            Some("(NOW() AT TIME ZONE 'UTC')".to_string())
        );
    }

    #[test]
    fn test_map_len() {
        assert_eq!(
            FunctionMapper::map_function_name("LEN"),
            Some("LENGTH".to_string())
        );
    }

    #[test]
    fn test_map_isnull() {
        assert_eq!(
            FunctionMapper::map_function_name("ISNULL"),
            Some("COALESCE".to_string())
        );
    }

    #[test]
    fn test_map_ceiling() {
        assert_eq!(
            FunctionMapper::map_function_name("CEILING"),
            Some("CEIL".to_string())
        );
    }

    #[test]
    fn test_map_newid() {
        assert_eq!(
            FunctionMapper::map_function_name("NEWID"),
            Some("UUID_GENERATE_V4".to_string())
        );
    }

    #[test]
    fn test_map_unknown_function_returns_none() {
        assert!(FunctionMapper::map_function_name("UNKNOWN_FUNCTION").is_none());
    }

    #[test]
    fn test_convert_getdate() {
        let result = FunctionMapper::map_function_call("GETDATE", &[]).unwrap();
        assert_eq!(result, "CURRENT_TIMESTAMP");
    }

    #[test]
    fn test_convert_getutcdate() {
        let result = FunctionMapper::map_function_call("GETUTCDATE", &[]).unwrap();
        assert_eq!(result, "(NOW() AT TIME ZONE 'UTC')");
    }

    #[test]
    fn test_convert_len() {
        let args = vec![CommonExpression::Identifier(CommonIdentifier {
            name: "column_name".to_string(),
        })];
        let result = FunctionMapper::map_function_call("LEN", &args).unwrap();
        assert_eq!(result, "LENGTH(column_name)");
    }

    #[test]
    fn test_convert_isnull() {
        let args = vec![
            CommonExpression::Identifier(CommonIdentifier {
                name: "expr".to_string(),
            }),
            CommonExpression::Literal(CommonLiteral::Integer(0)),
        ];
        let result = FunctionMapper::map_function_call("ISNULL", &args).unwrap();
        assert_eq!(result, "COALESCE(expr, 0)");
    }

    #[test]
    fn test_convert_ceiling() {
        let args = vec![CommonExpression::Identifier(CommonIdentifier {
            name: "value".to_string(),
        })];
        let result = FunctionMapper::map_function_call("CEILING", &args).unwrap();
        assert_eq!(result, "CEIL(\"value\")");
    }

    #[test]
    fn test_convert_dateadd_day() {
        let args = vec![
            CommonExpression::Identifier(CommonIdentifier {
                name: "DAY".to_string(),
            }),
            CommonExpression::Literal(CommonLiteral::Integer(7)),
            CommonExpression::Identifier(CommonIdentifier {
                name: "current_date".to_string(),
            }),
        ];
        let result = FunctionMapper::map_function_call("DATEADD", &args).unwrap();
        assert_eq!(result, "\"current_date\" + INTERVAL '7 days'");
    }

    #[test]
    fn test_convert_dateadd_month() {
        let args = vec![
            CommonExpression::Identifier(CommonIdentifier {
                name: "MONTH".to_string(),
            }),
            CommonExpression::Literal(CommonLiteral::Integer(3)),
            CommonExpression::Identifier(CommonIdentifier {
                name: "start_date".to_string(),
            }),
        ];
        let result = FunctionMapper::map_function_call("DATEADD", &args).unwrap();
        assert_eq!(result, "start_date + INTERVAL '3 months'");
    }

    #[test]
    fn test_convert_dateadd_year() {
        let args = vec![
            CommonExpression::Identifier(CommonIdentifier {
                name: "YEAR".to_string(),
            }),
            CommonExpression::Literal(CommonLiteral::Integer(1)),
            CommonExpression::Identifier(CommonIdentifier {
                name: "hire_date".to_string(),
            }),
        ];
        let result = FunctionMapper::map_function_call("DATEADD", &args).unwrap();
        assert_eq!(result, "hire_date + INTERVAL '1 years'");
    }

    #[test]
    fn test_convert_datediff_day() {
        let args = vec![
            CommonExpression::Identifier(CommonIdentifier {
                name: "DAY".to_string(),
            }),
            CommonExpression::Identifier(CommonIdentifier {
                name: "start_date".to_string(),
            }),
            CommonExpression::Identifier(CommonIdentifier {
                name: "end_date".to_string(),
            }),
        ];
        let result = FunctionMapper::map_function_call("DATEDIFF", &args).unwrap();
        assert_eq!(result, "DATE_PART('day', end_date - start_date)");
    }

    #[test]
    fn test_convert_datediff_month() {
        let args = vec![
            CommonExpression::Identifier(CommonIdentifier {
                name: "MONTH".to_string(),
            }),
            CommonExpression::Identifier(CommonIdentifier {
                name: "start_date".to_string(),
            }),
            CommonExpression::Identifier(CommonIdentifier {
                name: "end_date".to_string(),
            }),
        ];
        let result = FunctionMapper::map_function_call("DATEDIFF", &args).unwrap();
        assert_eq!(result, "DATE_PART('month', end_date - start_date)");
    }

    #[test]
    fn test_convert_unsupported_function() {
        let args = vec![CommonExpression::Literal(CommonLiteral::Integer(1))];
        let result = FunctionMapper::map_function_call("TSQL_CUSTOM", &args);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmitError::UnsupportedFunction(msg) => assert_eq!(msg, "TSQL_CUSTOM"),
            _ => panic!("Expected UnsupportedFunction error"),
        }
    }

    #[test]
    fn test_convert_dateadd_wrong_args() {
        let args = vec![
            CommonExpression::Identifier(CommonIdentifier {
                name: "DAY".to_string(),
            }),
            CommonExpression::Literal(CommonLiteral::Integer(7)),
        ];
        let result = FunctionMapper::map_function_call("DATEADD", &args);
        assert!(result.is_err());
    }
}
