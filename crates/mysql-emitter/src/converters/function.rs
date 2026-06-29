//! 関数のコンバーター
//!
//! T-SQL の関数を MySQL 関数に変換します。
//!
//! このモジュールは [`common_sql::ast::Expression`] / [`common_sql::ast::Identifier`]
//! を入力として扱い、レガシー `tsql_parser::common::*` 型には依存しません。
//! 引数の文字列化は呼び出し側からクロージャで注入する設計により、
//! `MySqlEmitter` 構造体 (Task 3.1) の実装詳細との循環依存を回避しています。

use crate::EmitError;
use common_sql::ast::{Expression, Identifier, Literal};

/// 引数式を MySQL SQL 文字列へ変換するクロージャの型。
///
/// `FunctionConverter` は引数の文字列化戦略をこのクロージャ経由で受け取ることで、
/// `MySqlEmitter::visit_expression` (Task 3.3) の実装に依存せず単体テスト可能です。
pub type ArgStringifier<'a> = &'a mut dyn FnMut(&Expression) -> Result<String, EmitError>;

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
    /// * `name` - 関数名 ([`Identifier`])
    /// * `args` - 引数リスト ([`Expression`])
    /// * `distinct` - DISTINCT 指定があるか
    /// * `stringify` - 引数式を MySQL SQL 文字列へ変換するクロージャ
    ///
    /// # Returns
    ///
    /// MySQL 関数呼び出し文字列、またはエラー
    ///
    /// # Errors
    ///
    /// 引数不足の DATEADD/DATEDIFF、またはマッピング不可能な関数名の場合
    /// [`EmitError::UnsupportedFunction`] を返します。
    pub fn convert_function(
        name: &Identifier,
        args: &[Expression],
        distinct: bool,
        stringify: ArgStringifier,
    ) -> Result<String, EmitError> {
        let func_name = name.value().to_uppercase();

        // 特殊な変換が必要な関数 (引数順逆転 / INTERVAL 再構成 / 引数破棄)
        match func_name.as_str() {
            "DATEADD" => {
                if args.len() >= 3 {
                    return Self::convert_dateadd(args, stringify);
                }
                return Err(EmitError::UnsupportedFunction {
                    function_name: format!("DATEADD with {} args", args.len()),
                });
            }
            "DATEDIFF" => {
                if args.len() >= 3 {
                    return Self::convert_datediff(args, stringify);
                }
                return Err(EmitError::UnsupportedFunction {
                    function_name: format!("DATEDIFF with {} args", args.len()),
                });
            }
            // 引数を破棄する関数: RAND(seed) -> RAND()
            "RAND" => return Ok("RAND()".to_string()),
            // STUFF -> INSERT (MySQL 8.0+ のみ。簡易変換: 引数順はそのまま INSERT(...) として展開)
            "STUFF" => return Self::convert_stuff(args, stringify),
            // PATINDEX -> LOCATE (簡易変換: そのまま LOCATE にマッピング)
            "PATINDEX" => {
                let mapped = "LOCATE";
                let args_str = Self::join_args(args, distinct, stringify)?;
                return Ok(format!("{mapped}({args_str})"));
            }
            _ => {}
        }

        // 単純な名前マッピングが存在すれば適用
        if let Some(mapped_name) = Self::map_function_name(&func_name) {
            let args_str = Self::join_args(args, distinct, stringify)?;
            return Ok(format!("{mapped_name}({args_str})"));
        }

        // デフォルト: 元の関数名を使用（DISTINCT は括弧内に展開）
        let args_str = Self::join_args(args, distinct, stringify)?;
        Ok(format!("{}({args_str})", name.value()))
    }

    /// [`MySqlEmitter`] に結合した関数変換エントリポイント。
    ///
    /// これは [`Self::convert_function`] の thin adapter であり、引数の文字列化を
    /// [`MySqlEmitter::emit_expression`] に委譲します。`emitter` 型は本 crate 内の
    /// [`crate::MySqlEmitter`] であるため、循環参照はモジュール境界内で閉じます。
    ///
    /// # Arguments
    ///
    /// * `name` - 関数名（大文字化して解決されるため元のケースは不問）
    /// * `args` - 引数リスト ([`Expression`])
    /// * `distinct` - DISTINCT 指定があるか
    /// * `emitter` - 引数文字列化に用いる Emitter
    ///
    /// # Errors
    ///
    /// 引数不足の DATEADD/DATEDIFF/STUFF の場合 [`EmitError::UnsupportedFunction`]
    /// を返します。
    pub fn convert_common_function(
        name: &str,
        args: &[Expression],
        distinct: bool,
        emitter: &mut crate::MySqlEmitter,
    ) -> Result<String, EmitError> {
        let ident = Identifier::new(name.to_string());
        Self::convert_function(&ident, args, distinct, &mut |arg: &Expression| {
            emitter.emit_expression(arg)
        })
    }

    /// 関数名を MySQL 関数名にマッピング
    ///
    /// 名前が変わるが引数構造はそのままの関数のマッピングテーブルです。
    /// DATEADD / DATEDIFF / RAND / STUFF / PATINDEX は引数構造が変わるため
    /// ここではなく [`Self::convert_function`] の match で個別処理されます。
    ///
    /// # Returns
    ///
    /// マッピングされた関数名、または `None`（マッピングなし）
    #[must_use]
    pub fn map_function_name(name: &str) -> Option<&'static str> {
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
            // 以下は名前不变だが明示的に MySQL 互換として保証する関数群
            "SUBSTRING" => Some("SUBSTRING"),
            "LEFT" => Some("LEFT"),
            "RIGHT" => Some("RIGHT"),
            "LTRIM" => Some("LTRIM"),
            "RTRIM" => Some("RTRIM"),
            "REPLACE" => Some("REPLACE"),
            "COALESCE" => Some("COALESCE"),
            "ABS" => Some("ABS"),
            "FLOOR" => Some("FLOOR"),
            "ROUND" => Some("ROUND"),
            "SQRT" => Some("SQRT"),
            _ => None,
        }
    }

    /// DATEADD を DATE_ADD に変換
    ///
    /// T-SQL: `DATEADD(part, n, date)`
    /// MySQL: `DATE_ADD(date, INTERVAL n part)`
    fn convert_dateadd(
        args: &[Expression],
        stringify: ArgStringifier,
    ) -> Result<String, EmitError> {
        // args[0]: datepart, args[1]: number, args[2]: date
        if args.len() < 3 {
            return Err(EmitError::UnsupportedFunction {
                function_name: "DATEADD (requires 3 arguments)".to_string(),
            });
        }

        // datepart はクォート付与前の raw 値で判定する (文字列リテラル/識別子)。
        // それ以外は文字列化値からクォートを除去してフォールバックする
        // (convert_datepart が未知値を既定の DAY へ正規化)。
        let datepart = match &args[0] {
            Expression::Literal(Literal::String(s)) => s.clone(),
            Expression::Identifier(id) => id.value().to_string(),
            other => {
                let s = stringify(other)?;
                s.trim_matches(|c| c == '\'' || c == '`').to_string()
            }
        };
        let number = stringify(&args[1])?;
        let date = stringify(&args[2])?;

        let part = Self::convert_datepart(&datepart);

        Ok(format!("DATE_ADD({date}, INTERVAL {number} {part})"))
    }

    /// DATEDIFF を変換
    ///
    /// T-SQL: `DATEDIFF(part, start, end)`
    /// MySQL: `DATEDIFF(end, start)`（注意: 引数順が逆。datepart は破棄）
    fn convert_datediff(
        args: &[Expression],
        stringify: ArgStringifier,
    ) -> Result<String, EmitError> {
        if args.len() < 3 {
            return Err(EmitError::UnsupportedFunction {
                function_name: "DATEDIFF (requires 3 arguments)".to_string(),
            });
        }

        // MySQL は日付差分のみ (day 単位) を返すため datepart は使用しない
        let _part = stringify(&args[0])?;
        let start = stringify(&args[1])?;
        let end = stringify(&args[2])?;

        Ok(format!("DATEDIFF({end}, {start})"))
    }

    /// STUFF を INSERT (MySQL) に変換
    ///
    /// T-SQL: `STUFF(s, start, len, insert)`
    /// MySQL: `INSERT(s, start, len, insert)`（関数名のみ変更、引数順は同一）
    fn convert_stuff(args: &[Expression], stringify: ArgStringifier) -> Result<String, EmitError> {
        if args.len() != 4 {
            return Err(EmitError::UnsupportedFunction {
                function_name: format!("STUFF with {} args (requires 4)", args.len()),
            });
        }
        let args_str = Self::join_args(args, false, stringify)?;
        Ok(format!("INSERT({args_str})"))
    }

    /// 引数リストをカンマ区切りで結合
    ///
    /// `distinct` が真の場合、最初の引数の前に `DISTINCT ` を付与します
    /// (MySQL の集計関数構文 `AGG(DISTINCT arg)` に合致)。
    fn join_args(
        args: &[Expression],
        distinct: bool,
        stringify: ArgStringifier,
    ) -> Result<String, EmitError> {
        let mut parts: Vec<String> = Vec::with_capacity(args.len());
        for (i, arg) in args.iter().enumerate() {
            let mut s = stringify(arg)?;
            if distinct && i == 0 {
                s = format!("DISTINCT {s}");
            }
            parts.push(s);
        }
        Ok(parts.join(", "))
    }

    /// T-SQL の datepart を MySQL の interval 単位に変換
    #[must_use]
    pub fn convert_datepart(datepart: &str) -> &'static str {
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
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::{Identifier, Literal};

    // テスト用の文字列化クロージャ: 引数を再帰的に MySQL SQL へ変換する最小実装。
    // MySqlEmitter::visit_expression (Task 3.3) のスタンドイン。
    fn stringify_expr(expr: &Expression) -> Result<String, EmitError> {
        match expr {
            Expression::Literal(lit) => match lit {
                Literal::Integer(n) => Ok(n.to_string()),
                Literal::Float(s) => Ok(s.clone()),
                Literal::String(s) => Ok(format!("'{}'", s.replace('\'', "''"))),
                Literal::Boolean(b) => Ok(if *b { "TRUE" } else { "FALSE" }.to_string()),
                Literal::Null => Ok("NULL".to_string()),
            },
            Expression::Identifier(ident) => Ok(format!("`{}`", ident.value())),
            Expression::QualifiedIdentifier { table, column } => {
                Ok(format!("`{}`.`{}`", table.value(), column.value()))
            }
            // それ以外は関数コンバータ自身に再委譲（関数の入れ子テスト用）
            Expression::Function {
                name,
                args,
                distinct,
            } => FunctionConverter::convert_function(name, args, *distinct, &mut |e| {
                stringify_expr(e)
            }),
            _ => Err(EmitError::UnsupportedExpression {
                expression_type: format!("{expr:?}"),
            }),
        }
    }

    fn make_stringifier() -> impl FnMut(&Expression) -> Result<String, EmitError> {
        |e: &Expression| stringify_expr(e)
    }

    fn ident(name: &str) -> Identifier {
        Identifier::new(name.to_string())
    }

    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn str_expr(s: &str) -> Expression {
        Expression::Literal(Literal::String(s.to_string()))
    }

    fn id_expr(name: &str) -> Expression {
        Expression::Identifier(ident(name))
    }

    // ===== map_function_name: 名前マッピングテーブル =====

    #[test]
    fn map_getdate_to_now() {
        assert_eq!(FunctionConverter::map_function_name("GETDATE"), Some("NOW"));
    }

    #[test]
    fn map_getutcdate_to_utc_timestamp() {
        assert_eq!(
            FunctionConverter::map_function_name("GETUTCDATE"),
            Some("UTC_TIMESTAMP")
        );
    }

    #[test]
    fn map_len_to_length() {
        assert_eq!(FunctionConverter::map_function_name("LEN"), Some("LENGTH"));
    }

    #[test]
    fn map_charindex_to_locate() {
        assert_eq!(
            FunctionConverter::map_function_name("CHARINDEX"),
            Some("LOCATE")
        );
    }

    #[test]
    fn map_replicate_to_repeat() {
        assert_eq!(
            FunctionConverter::map_function_name("REPLICATE"),
            Some("REPEAT")
        );
    }

    #[test]
    fn map_isnull_to_ifnull() {
        assert_eq!(
            FunctionConverter::map_function_name("ISNULL"),
            Some("IFNULL")
        );
    }

    #[test]
    fn map_newid_to_uuid() {
        assert_eq!(FunctionConverter::map_function_name("NEWID"), Some("UUID"));
    }

    #[test]
    fn map_ceiling_to_ceil() {
        assert_eq!(
            FunctionConverter::map_function_name("CEILING"),
            Some("CEIL")
        );
    }

    #[test]
    fn map_power_to_pow() {
        assert_eq!(FunctionConverter::map_function_name("POWER"), Some("POW"));
    }

    #[test]
    fn map_unknown_function_returns_none() {
        assert_eq!(FunctionConverter::map_function_name("UNKNOWN_FUNC"), None);
    }

    // ===== convert_function: 名前変更パターン (reqs 1,2,7,13,16,18,20,23,26) =====

    #[test]
    fn convert_getdate_to_now() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(&ident("GETDATE"), &[], false, &mut s);
        assert_eq!(result.unwrap(), "NOW()");
    }

    #[test]
    fn convert_getdate_case_insensitive() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(&ident("getdate"), &[], false, &mut s);
        assert_eq!(result.unwrap(), "NOW()");
    }

    #[test]
    fn convert_getutcdate_to_utc_timestamp() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(&ident("GETUTCDATE"), &[], false, &mut s);
        assert_eq!(result.unwrap(), "UTC_TIMESTAMP()");
    }

    #[test]
    fn convert_len_to_length() {
        let mut s = make_stringifier();
        let result =
            FunctionConverter::convert_function(&ident("LEN"), &[str_expr("abc")], false, &mut s);
        assert_eq!(result.unwrap(), "LENGTH('abc')");
    }

    #[test]
    fn convert_charindex_to_locate() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("CHARINDEX"),
            &[str_expr("a"), str_expr("abc")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "LOCATE('a', 'abc')");
    }

    #[test]
    fn convert_replicate_to_repeat() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("REPLICATE"),
            &[str_expr("x"), int_expr(3)],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "REPEAT('x', 3)");
    }

    #[test]
    fn convert_isnull_to_ifnull() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("ISNULL"),
            &[id_expr("name"), str_expr("N/A")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "IFNULL(`name`, 'N/A')");
    }

    #[test]
    fn convert_newid_to_uuid() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(&ident("NEWID"), &[], false, &mut s);
        assert_eq!(result.unwrap(), "UUID()");
    }

    #[test]
    fn convert_ceiling_to_ceil() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("CEILING"),
            &[id_expr("price")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "CEIL(`price`)");
    }

    #[test]
    fn convert_power_to_pow() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("POWER"),
            &[int_expr(2), int_expr(10)],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "POW(2, 10)");
    }

    // ===== convert_function: 名前不変パターン (reqs 8,9,10,11,12,15,19,22,24,25,27) =====

    #[test]
    fn convert_substring_passthrough() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("SUBSTRING"),
            &[str_expr("hello"), int_expr(1), int_expr(3)],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "SUBSTRING('hello', 1, 3)");
    }

    #[test]
    fn convert_left_passthrough() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("LEFT"),
            &[str_expr("hello"), int_expr(2)],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "LEFT('hello', 2)");
    }

    #[test]
    fn convert_right_passthrough() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("RIGHT"),
            &[str_expr("hello"), int_expr(2)],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "RIGHT('hello', 2)");
    }

    #[test]
    fn convert_ltrim_passthrough() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("LTRIM"),
            &[str_expr("  hi")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "LTRIM('  hi')");
    }

    #[test]
    fn convert_rtrim_passthrough() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("RTRIM"),
            &[str_expr("hi  ")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "RTRIM('hi  ')");
    }

    #[test]
    fn convert_replace_passthrough() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("REPLACE"),
            &[str_expr("a-b"), str_expr("-"), str_expr("_")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "REPLACE('a-b', '-', '_')");
    }

    #[test]
    fn convert_coalesce_passthrough() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("COALESCE"),
            &[id_expr("a"), id_expr("b"), str_expr("def")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "COALESCE(`a`, `b`, 'def')");
    }

    #[test]
    fn convert_abs_passthrough() {
        let mut s = make_stringifier();
        let result =
            FunctionConverter::convert_function(&ident("ABS"), &[int_expr(-5)], false, &mut s);
        assert_eq!(result.unwrap(), "ABS(-5)");
    }

    #[test]
    fn convert_floor_passthrough() {
        let mut s = make_stringifier();
        let result =
            FunctionConverter::convert_function(&ident("FLOOR"), &[id_expr("x")], false, &mut s);
        assert_eq!(result.unwrap(), "FLOOR(`x`)");
    }

    #[test]
    fn convert_round_passthrough() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("ROUND"),
            &[id_expr("v"), int_expr(2)],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "ROUND(`v`, 2)");
    }

    #[test]
    fn convert_sqrt_passthrough() {
        let mut s = make_stringifier();
        let result =
            FunctionConverter::convert_function(&ident("SQRT"), &[int_expr(16)], false, &mut s);
        assert_eq!(result.unwrap(), "SQRT(16)");
    }

    // ===== convert_function: 引数構造変化パターン =====

    #[test]
    fn convert_rand_drops_seed() {
        // req 21: RAND(seed) -> RAND() (MySQL はシード引数非サポート)
        let mut s = make_stringifier();
        let result =
            FunctionConverter::convert_function(&ident("RAND"), &[int_expr(42)], false, &mut s);
        assert_eq!(result.unwrap(), "RAND()");
    }

    #[test]
    fn convert_rand_no_args() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(&ident("RAND"), &[], false, &mut s);
        assert_eq!(result.unwrap(), "RAND()");
    }

    #[test]
    fn convert_patindex_to_locate() {
        // req 14: PATINDEX(pattern, s) -> LOCATE(pattern, s) (簡易変換)
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("PATINDEX"),
            &[str_expr("%foo%"), str_expr("xfooy")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "LOCATE('%foo%', 'xfooy')");
    }

    #[test]
    fn convert_stuff_to_insert() {
        // req 17: STUFF(s, start, len, insert) -> INSERT(s, start, len, insert)
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("STUFF"),
            &[
                str_expr("abcdef"),
                int_expr(2),
                int_expr(3),
                str_expr("XYZ"),
            ],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "INSERT('abcdef', 2, 3, 'XYZ')");
    }

    #[test]
    fn convert_stuff_wrong_arg_count_errors() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("STUFF"),
            &[str_expr("abc"), int_expr(1)],
            false,
            &mut s,
        );
        assert!(result.is_err());
    }

    // ===== DATEADD / DATEDIFF: 引数順逆転/再構成 (reqs 3,4,5,6) =====

    #[test]
    fn convert_dateadd_day() {
        // req 3: DATEADD(day, n, date) -> DATE_ADD(date, INTERVAL n DAY)
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("DATEADD"),
            &[str_expr("day"), int_expr(5), id_expr("created_at")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "DATE_ADD(`created_at`, INTERVAL 5 DAY)");
    }

    #[test]
    fn convert_dateadd_month() {
        // req 4: DATEADD(month, n, date) -> DATE_ADD(date, INTERVAL n MONTH)
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("DATEADD"),
            &[str_expr("month"), int_expr(1), id_expr("d")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "DATE_ADD(`d`, INTERVAL 1 MONTH)");
    }

    #[test]
    fn convert_dateadd_year() {
        // req 5: DATEADD(year, n, date) -> DATE_ADD(date, INTERVAL n YEAR)
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("DATEADD"),
            &[str_expr("year"), int_expr(2), id_expr("d")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "DATE_ADD(`d`, INTERVAL 2 YEAR)");
    }

    #[test]
    fn convert_dateadd_abbreviation_datepart() {
        // 略語 datepart (yy, mm, dd) も MySQL 単位へ正規化
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("DATEADD"),
            &[str_expr("yy"), int_expr(1), id_expr("d")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "DATE_ADD(`d`, INTERVAL 1 YEAR)");
    }

    #[test]
    fn convert_dateadd_case_insensitive_datepart() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("DATEADD"),
            &[str_expr("MONTH"), int_expr(1), id_expr("d")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "DATE_ADD(`d`, INTERVAL 1 MONTH)");
    }

    #[test]
    fn convert_dateadd_too_few_args_errors() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("DATEADD"),
            &[str_expr("day"), int_expr(5)],
            false,
            &mut s,
        );
        assert!(result.is_err());
        match result.unwrap_err() {
            EmitError::UnsupportedFunction { function_name } => {
                assert!(function_name.contains("DATEADD"));
            }
            other => panic!("expected UnsupportedFunction, got {other:?}"),
        }
    }

    #[test]
    fn convert_datediff_reverses_args() {
        // req 6: DATEDIFF(day, start, end) -> DATEDIFF(end, start) (引数順逆転)
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("DATEDIFF"),
            &[
                str_expr("day"),
                str_expr("2024-01-01"),
                str_expr("2024-01-31"),
            ],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "DATEDIFF('2024-01-31', '2024-01-01')");
    }

    #[test]
    fn convert_datediff_drops_datepart() {
        // datepart は結果に現れない
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("DATEDIFF"),
            &[str_expr("month"), id_expr("s"), id_expr("e")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "DATEDIFF(`e`, `s`)");
    }

    #[test]
    fn convert_datediff_too_few_args_errors() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("DATEDIFF"),
            &[str_expr("day"), id_expr("s")],
            false,
            &mut s,
        );
        assert!(result.is_err());
    }

    // ===== convert_datepart 単体テスト =====

    #[test]
    fn datepart_year_variants() {
        assert_eq!(FunctionConverter::convert_datepart("YEAR"), "YEAR");
        assert_eq!(FunctionConverter::convert_datepart("YYYY"), "YEAR");
        assert_eq!(FunctionConverter::convert_datepart("YY"), "YEAR");
    }

    #[test]
    fn datepart_month_variants() {
        assert_eq!(FunctionConverter::convert_datepart("MONTH"), "MONTH");
        assert_eq!(FunctionConverter::convert_datepart("MM"), "MONTH");
        assert_eq!(FunctionConverter::convert_datepart("M"), "MONTH");
    }

    #[test]
    fn datepart_day_variants() {
        assert_eq!(FunctionConverter::convert_datepart("DAY"), "DAY");
        assert_eq!(FunctionConverter::convert_datepart("DD"), "DAY");
        assert_eq!(FunctionConverter::convert_datepart("D"), "DAY");
    }

    #[test]
    fn datepart_hour_variants() {
        assert_eq!(FunctionConverter::convert_datepart("HOUR"), "HOUR");
        assert_eq!(FunctionConverter::convert_datepart("HH"), "HOUR");
    }

    #[test]
    fn datepart_minute_variants() {
        assert_eq!(FunctionConverter::convert_datepart("MINUTE"), "MINUTE");
        assert_eq!(FunctionConverter::convert_datepart("MI"), "MINUTE");
        assert_eq!(FunctionConverter::convert_datepart("N"), "MINUTE");
    }

    #[test]
    fn datepart_second_variants() {
        assert_eq!(FunctionConverter::convert_datepart("SECOND"), "SECOND");
        assert_eq!(FunctionConverter::convert_datepart("SS"), "SECOND");
        assert_eq!(FunctionConverter::convert_datepart("S"), "SECOND");
    }

    #[test]
    fn datepart_unknown_defaults_to_day() {
        assert_eq!(FunctionConverter::convert_datepart("UNKNOWN"), "DAY");
    }

    #[test]
    fn datepart_case_insensitive() {
        assert_eq!(FunctionConverter::convert_datepart("year"), "YEAR");
        assert_eq!(FunctionConverter::convert_datepart("Month"), "MONTH");
    }

    // ===== distinct / 未マッピング関数 / 入れ子 =====

    #[test]
    fn convert_distinct_inside_parens_for_unmapped_function() {
        // DISTINCT は括弧内の最初の引数前に展開: AGG(DISTINCT arg)
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("CUSTOM_AGG"),
            &[id_expr("c")],
            true,
            &mut s,
        );
        assert_eq!(result.unwrap(), "CUSTOM_AGG(DISTINCT `c`)");
    }

    #[test]
    fn convert_distinct_inside_parens_for_mapped_function() {
        // マップ済み関数でも DISTINCT は括弧内: SUM 相当の LEN(DISTINCT x)
        let mut s = make_stringifier();
        let result =
            FunctionConverter::convert_function(&ident("LEN"), &[id_expr("x")], true, &mut s);
        assert_eq!(result.unwrap(), "LENGTH(DISTINCT `x`)");
    }

    #[test]
    fn convert_unmapped_function_no_distinct_passthrough() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(
            &ident("MY_FUNC"),
            &[int_expr(1), int_expr(2)],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "MY_FUNC(1, 2)");
    }

    #[test]
    fn convert_nested_function_in_args() {
        // ISNULL(LEFT(name, 3), 'x') -> IFNULL(LEFT(`name`, 3), 'x')
        let mut s = make_stringifier();
        let inner = Expression::Function {
            name: ident("LEFT"),
            args: vec![id_expr("name"), int_expr(3)],
            distinct: false,
        };
        let result = FunctionConverter::convert_function(
            &ident("ISNULL"),
            &[inner, str_expr("x")],
            false,
            &mut s,
        );
        assert_eq!(result.unwrap(), "IFNULL(LEFT(`name`, 3), 'x')");
    }

    #[test]
    fn convert_no_args_unmapped_emits_empty_parens() {
        let mut s = make_stringifier();
        let result = FunctionConverter::convert_function(&ident("FOO"), &[], false, &mut s);
        assert_eq!(result.unwrap(), "FOO()");
    }
}
