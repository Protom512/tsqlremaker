//! # MySQL Emitter — Expression Visitor (Task 3.3)
//!
//! `MySqlExpressionEmitter` の式 visitor 実装。[`common_sql::ast::Expression`] の全 15
//! バリアントを MySQL SQL 文字列へ変換する。
//!
//! ## 設計（ハイブリッド）
//!
//! 要求 1.5 のコントラクト適合のため [`common_sql::Visitor`] trait を実装する
//! （`type Output = String`）。ただし `Visitor::Output` はエラー型を内包できないた
//! め、実際のエラー伝播は private な `Result` 返却型の再帰メソッド群
//! （[`emit_expression`](MySqlExpressionEmitter::emit_expression) および各 `visit_*`）で行う。
//! `Visitor` 実装はこれら private メソッドへ委譲し、エラーは [`Self::last_error`]
//! に退避される。これにより:
//!
//! - 公開コントラクト（`Visitor`）: 任意の AST ノードを String へ変換可能
//! - ライブラリ内エラー伝播: `Result<String, EmitError>` で確実に伝播
//!
//! リテラル・識別子は `common_sql` の `value()` アクセサ経由で読み出す。

use common_sql::ast::{
    BinaryOperator, ComparisonOperator, DataType, Expression, Identifier, InList, Literal,
    LogicalOperator, SelectStatement, UnaryOperator,
};
use common_sql::Visitor;

use crate::converters::FunctionConverter;
use crate::{EmitError, EmitterConfig};

/// MySQL Emitter
///
/// [`common_sql::ast::Expression`] を MySQL 方言の SQL 文字列へ変換する。
#[derive(Debug)]
pub struct MySqlExpressionEmitter {
    /// 出力バッファ
    buffer: String,
    /// コンフィグ
    config: EmitterConfig,
    /// `Visitor` 実装経由で発生した直近のエラー。
    ///
    /// `Visitor::Output = String` はエラーを返せないため、エラーはここに退避され、
    /// 次回の呼び出し開始時にクリアされる。
    last_error: Option<EmitError>,
}

impl MySqlExpressionEmitter {
    /// 新しい Emitter を作成する。
    #[must_use]
    pub fn new(config: EmitterConfig) -> Self {
        Self {
            buffer: String::new(),
            config,
            last_error: None,
        }
    }

    /// コンフィグへの参照を返す。
    #[must_use]
    pub const fn config(&self) -> &EmitterConfig {
        &self.config
    }

    /// デフォルト設定で Emitter を構築する。
    #[must_use]
    pub fn default_config() -> Self {
        Self::new(EmitterConfig::default())
    }

    /// 式を MySQL SQL 文字列へ変換する（private Result 返却のエントリポイント）。
    ///
    /// これはエラー伝播の主経路である。[`Visitor`] trait の公開メソッドは内部で
    /// これへ委譲し、エラーを [`Self::last_error`] へ退避する。
    ///
    /// # Errors
    ///
    /// サポート対象外の式（例: `ILIKE`）に遭遇した場合は [`EmitError`] を返す。
    pub fn emit_expression(&mut self, expr: &Expression) -> Result<String, EmitError> {
        let old_buffer = std::mem::take(&mut self.buffer);

        self.visit_expression_inner(expr)?;
        let result = std::mem::take(&mut self.buffer);
        self.buffer = old_buffer;
        Ok(result)
    }

    /// 式をディスパッチしバッファへ出力する（バッファ退避なしの内部実装）。
    fn visit_expression_inner(&mut self, expr: &Expression) -> Result<(), EmitError> {
        match expr {
            Expression::Literal(lit) => self.visit_literal(lit),
            Expression::Identifier(ident) => self.visit_identifier(ident),
            Expression::QualifiedIdentifier { table, column } => {
                self.visit_qualified_identifier(table, column)
            }
            Expression::BinaryOp { left, op, right } => self.visit_binary_op(left, *op, right),
            Expression::UnaryOp { op, expr } => self.visit_unary_op(*op, expr),
            Expression::LogicalOp { left, op, right } => self.visit_logical_op(left, *op, right),
            Expression::Comparison { left, op, right } => self.visit_comparison(left, *op, right),
            Expression::Function {
                name,
                args,
                distinct,
            } => self.visit_function(name, args, *distinct),
            Expression::Case {
                operand,
                conditions,
                else_result,
            } => self.visit_case(operand, conditions, else_result),
            Expression::Subquery(query) => self.visit_subquery(query),
            Expression::Exists { subquery, negated } => self.visit_exists(subquery, *negated),
            Expression::In {
                expr,
                list,
                negated,
            } => self.visit_in(expr, list, *negated),
            Expression::Between {
                expr,
                low,
                high,
                negated,
            } => self.visit_between(expr, low, high, *negated),
            Expression::Cast { expr, data_type } => self.visit_cast(expr, data_type),
            Expression::IsNull { expr, negated } => self.visit_is_null(expr, *negated),
        }
    }

    // -----------------------------------------------------------------------
    // Private visit_* : Result 返却の再帰（エラー伝播の主経路）
    // -----------------------------------------------------------------------

    /// リテラルを訪問する。
    fn visit_literal(&mut self, lit: &Literal) -> Result<(), EmitError> {
        match lit {
            Literal::String(s) => {
                // シングルクォートをエスケープしてクォートで囲む
                self.write(&format!("'{}'", s.replace('\'', "''")));
            }
            Literal::Integer(n) => self.write(&n.to_string()),
            Literal::Float(s) => self.write(s),
            Literal::Null => self.write("NULL"),
            Literal::Boolean(b) => self.write(if *b { "TRUE" } else { "FALSE" }),
        }
        Ok(())
    }

    /// 識別子を訪問する（`value()` アクセサ経由）。
    fn visit_identifier(&mut self, ident: &Identifier) -> Result<(), EmitError> {
        // MySQL の識別子エスケープ: バッククォートで囲む
        self.write(&format!("`{}`", ident.value()));
        Ok(())
    }

    /// 修飾識別子 (`table.column`) を訪問する。
    fn visit_qualified_identifier(
        &mut self,
        table: &Identifier,
        column: &Identifier,
    ) -> Result<(), EmitError> {
        self.write(&format!("`{}`.`{}`", table.value(), column.value()));
        Ok(())
    }

    /// 二項演算子（算術・文字列連結）を訪問する。
    fn visit_binary_op(
        &mut self,
        left: &Expression,
        op: BinaryOperator,
        right: &Expression,
    ) -> Result<(), EmitError> {
        let left_str = self.emit_expression(left)?;
        self.write(&left_str);
        self.write(" ");
        let op_str = match op {
            BinaryOperator::Add => "+",
            BinaryOperator::Sub => "-",
            BinaryOperator::Mul => "*",
            BinaryOperator::Div => "/",
            BinaryOperator::Mod => "%",
            BinaryOperator::Concat => "||",
        };
        self.write(op_str);
        self.write(" ");
        let right_str = self.emit_expression(right)?;
        self.write(&right_str);
        Ok(())
    }

    /// 単項演算子を訪問する。
    fn visit_unary_op(&mut self, op: UnaryOperator, expr: &Expression) -> Result<(), EmitError> {
        let op_str = match op {
            UnaryOperator::Plus => "+",
            UnaryOperator::Minus => "-",
            UnaryOperator::Not => "NOT ",
        };
        self.write(op_str);
        let expr_str = self.emit_expression(expr)?;
        self.write(&expr_str);
        Ok(())
    }

    /// 論理演算子 (AND / OR) を訪問する。
    fn visit_logical_op(
        &mut self,
        left: &Expression,
        op: LogicalOperator,
        right: &Expression,
    ) -> Result<(), EmitError> {
        let left_str = self.emit_expression(left)?;
        self.write(&left_str);
        self.write(" ");
        self.write(match op {
            LogicalOperator::And => "AND",
            LogicalOperator::Or => "OR",
        });
        self.write(" ");
        let right_str = self.emit_expression(right)?;
        self.write(&right_str);
        Ok(())
    }

    /// 比較演算子を訪問する（`visit_binary_op` から分離）。
    ///
    /// `LIKE` / `NOT LIKE` は MySQL では直接演算子として出力可能。`ILIKE` /
    /// `NOT ILIKE` は PostgreSQL 固有のため MySQL ではサポート対象外。
    fn visit_comparison(
        &mut self,
        left: &Expression,
        op: ComparisonOperator,
        right: &Expression,
    ) -> Result<(), EmitError> {
        let left_str = self.emit_expression(left)?;
        self.write(&left_str);
        self.write(" ");
        let op_str = match op {
            ComparisonOperator::Eq => "=",
            ComparisonOperator::Ne => "!=",
            ComparisonOperator::Lt => "<",
            ComparisonOperator::Le => "<=",
            ComparisonOperator::Gt => ">",
            ComparisonOperator::Ge => ">=",
            ComparisonOperator::Like => "LIKE",
            ComparisonOperator::NotLike => "NOT LIKE",
            ComparisonOperator::ILike | ComparisonOperator::NotILike => {
                return Err(EmitError::UnsupportedExpression {
                    expression_type: format!(
                        "{op:?} (case-insensitive LIKE not supported in MySQL)"
                    ),
                });
            }
        };
        self.write(op_str);
        self.write(" ");
        let right_str = self.emit_expression(right)?;
        self.write(&right_str);
        Ok(())
    }

    /// 関数呼び出しを訪問する（[`FunctionConverter`] へ委譲）。
    fn visit_function(
        &mut self,
        name: &Identifier,
        args: &[Expression],
        distinct: bool,
    ) -> Result<(), EmitError> {
        // FunctionConverter は引数の文字列化をクロージャ (ArgStringifier) 経由で受け取る。
        // ここでは self.emit_expression を渡し、変換結果の文字列をバッファへ書き出す。
        // クロージャの借用を convert_function 呼び出しに限定するため、結果の書き出しは
        //独立の文で行う（NLL により convert_function のリターン後に借用は解放される）。
        let result = {
            let mut stringify = |arg: &Expression| self.emit_expression(arg);
            FunctionConverter::convert_function(name, args, distinct, &mut stringify)?
        };
        self.write(&result);
        Ok(())
    }

    /// CASE 式を訪問する。
    fn visit_case(
        &mut self,
        operand: &Option<Box<Expression>>,
        conditions: &[(Expression, Expression)],
        else_result: &Option<Box<Expression>>,
    ) -> Result<(), EmitError> {
        // CASE 式は他の式ノードと同様に単行で描画する。
        // 複数行の整形は Formatter コンポーネント (design Task 5.1) の責務。
        self.write("CASE");

        // simple CASE: CASE operand WHEN ... THEN ...
        if let Some(operand_expr) = operand {
            self.write(" ");
            let operand_str = self.emit_expression(operand_expr)?;
            self.write(&operand_str);
        }

        for (when_expr, then_expr) in conditions {
            self.write(" WHEN ");
            let when_str = self.emit_expression(when_expr)?;
            self.write(&when_str);
            self.write(" THEN ");
            let then_str = self.emit_expression(then_expr)?;
            self.write(&then_str);
        }

        if let Some(else_expr) = else_result {
            self.write(" ELSE ");
            let else_str = self.emit_expression(else_expr)?;
            self.write(&else_str);
        }

        self.write(" END");
        Ok(())
    }

    /// スカラーサブクエリを訪問する: `(SELECT ...)`。
    fn visit_subquery(&mut self, query: &SelectStatement) -> Result<(), EmitError> {
        self.write("(");
        self.visit_select_statement(query)?;
        self.write(")");
        Ok(())
    }

    /// EXISTS / NOT EXISTS を訪問する。
    fn visit_exists(&mut self, query: &SelectStatement, negated: bool) -> Result<(), EmitError> {
        self.write(if negated { "NOT EXISTS (" } else { "EXISTS (" });
        self.visit_select_statement(query)?;
        self.write(")");
        Ok(())
    }

    /// IN / NOT IN を訪問する。
    fn visit_in(
        &mut self,
        expr: &Expression,
        list: &InList,
        negated: bool,
    ) -> Result<(), EmitError> {
        let expr_str = self.emit_expression(expr)?;
        self.write(&expr_str);
        self.write(if negated { " NOT IN (" } else { " IN (" });
        match list {
            InList::Values(values) => {
                for (i, value) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    let value_str = self.emit_expression(value)?;
                    self.write(&value_str);
                }
            }
            InList::Subquery(query) => {
                self.visit_select_statement(query)?;
            }
        }
        self.write(")");
        Ok(())
    }

    /// BETWEEN / NOT BETWEEN を訪問する。
    fn visit_between(
        &mut self,
        expr: &Expression,
        low: &Expression,
        high: &Expression,
        negated: bool,
    ) -> Result<(), EmitError> {
        let expr_str = self.emit_expression(expr)?;
        self.write(&expr_str);
        self.write(if negated {
            " NOT BETWEEN "
        } else {
            " BETWEEN "
        });
        let low_str = self.emit_expression(low)?;
        self.write(&low_str);
        self.write(" AND ");
        let high_str = self.emit_expression(high)?;
        self.write(&high_str);
        Ok(())
    }

    /// CAST 式を訪問する: `CAST(expr AS type)`。
    fn visit_cast(&mut self, expr: &Expression, data_type: &DataType) -> Result<(), EmitError> {
        self.write("CAST(");
        let expr_str = self.emit_expression(expr)?;
        self.write(&expr_str);
        self.write(" AS ");
        let type_str = Self::format_data_type(data_type)?;
        self.write(&type_str);
        self.write(")");
        Ok(())
    }

    /// IS NULL / IS NOT NULL を訪問する。
    fn visit_is_null(&mut self, expr: &Expression, negated: bool) -> Result<(), EmitError> {
        let expr_str = self.emit_expression(expr)?;
        self.write(&expr_str);
        self.write(if negated { " IS NOT NULL" } else { " IS NULL" });
        Ok(())
    }

    /// SELECT 文を訪問する（サブクエリ内で使用される最小実装）。
    ///
    /// 本タスク (3.3) のスコープは式 visitor であるため、SELECT 全体の生成は
    /// Task 4.1 で詳細化される。ここでは投影リストのみを出力し、サブクエリが
    /// 式文脈で評価可能な最小限の出力を提供する。
    fn visit_select_statement(&mut self, stmt: &SelectStatement) -> Result<(), EmitError> {
        self.write("SELECT ");
        for (i, item) in stmt.projection.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            match item {
                common_sql::ast::SelectItem::Wildcard => self.write("*"),
                common_sql::ast::SelectItem::QualifiedWildcard { table } => {
                    self.write(&format!("`{}`.*", table.value()));
                }
                common_sql::ast::SelectItem::Expression { expr, alias } => {
                    let expr_str = self.emit_expression(expr)?;
                    self.write(&expr_str);
                    if let Some(a) = alias {
                        self.write(&format!(" AS `{}`", a.value()));
                    }
                }
            }
        }
        Ok(())
    }

    /// データ型を MySQL 型名へフォーマットする。
    fn format_data_type(dt: &DataType) -> Result<String, EmitError> {
        Ok(match dt {
            DataType::TinyInt => "TINYINT".to_string(),
            DataType::SmallInt => "SMALLINT".to_string(),
            DataType::Int => "INT".to_string(),
            DataType::BigInt => "BIGINT".to_string(),
            DataType::Decimal { precision, scale } => {
                Self::format_params(*precision, *scale, "DECIMAL")
            }
            DataType::Numeric { precision, scale } => {
                Self::format_params(*precision, *scale, "DECIMAL")
            }
            DataType::Real | DataType::DoublePrecision => "DOUBLE".to_string(),
            DataType::Char { length } => Self::format_single_param(*length, "CHAR"),
            DataType::VarChar { length } => Self::format_single_param(*length, "VARCHAR"),
            DataType::Text => "TEXT".to_string(),
            DataType::NChar { length } => Self::format_single_param(*length, "CHAR"),
            DataType::NVarChar { length } => Self::format_single_param(*length, "VARCHAR"),
            DataType::NText => "TEXT".to_string(),
            DataType::Date => "DATE".to_string(),
            DataType::Time { precision } => Self::format_single_param(*precision, "TIME"),
            DataType::DateTime { precision } => Self::format_single_param(*precision, "DATETIME"),
            DataType::Timestamp { precision } => Self::format_single_param(*precision, "TIMESTAMP"),
            DataType::Binary { length } => Self::format_single_param(*length, "BINARY"),
            DataType::VarBinary { length } => Self::format_single_param(*length, "VARBINARY"),
            DataType::Blob => "BLOB".to_string(),
            DataType::Boolean => "TINYINT(1)".to_string(),
            DataType::Uuid => "CHAR(36)".to_string(),
            DataType::Json => "JSON".to_string(),
        })
    }

    /// `(p,s)` 形式のパラメータをフォーマットする。
    fn format_params(precision: Option<u8>, scale: Option<u8>, name: &str) -> String {
        match (precision, scale) {
            (Some(p), Some(s)) => format!("{name}({p}, {s})"),
            (Some(p), None) => format!("{name}({p})"),
            (None, _) => name.to_string(),
        }
    }

    /// `(n)` 形式の単一パラメータをフォーマットする。
    fn format_single_param<T: std::fmt::Display>(value: Option<T>, name: &str) -> String {
        match value {
            Some(v) => format!("{name}({v})"),
            None => name.to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // バッファ操作ヘルパ
    // -----------------------------------------------------------------------

    /// バッファへ文字列を追加する。
    fn write(&mut self, s: &str) {
        self.buffer.push_str(s);
    }
}

impl Default for MySqlExpressionEmitter {
    fn default() -> Self {
        Self::default_config()
    }
}

// ---------------------------------------------------------------------------
// 公開コントラクト: common_sql::Visitor 実装（Req 1.5）
// ---------------------------------------------------------------------------

impl Visitor for MySqlExpressionEmitter {
    /// エラーを内包できないため `String`。エラーは [`MySqlExpressionEmitter::last_error`] へ退避。
    type Output = String;

    fn default_output(&self) -> Self::Output {
        String::new()
    }

    fn visit_expression(&mut self, expr: &Expression) -> Self::Output {
        // 呼び出し毎にエラーをクリア
        self.last_error = None;
        match self.emit_expression(expr) {
            Ok(s) => s,
            Err(e) => {
                self.last_error = Some(e);
                String::new()
            }
        }
    }
}

// ===========================================================================
// Tests (TDD: written FIRST against common_sql::Expression, 15 variants)
// ===========================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::{
        BinaryOperator as BinOp, ComparisonOperator as CmpOp, Expression, Identifier, InList,
        Literal, LogicalOperator as LogOp, SelectItem, SelectStatement, UnaryOperator as UnOp,
    };

    // -- helpers -----------------------------------------------------------

    fn ident(name: &str) -> Identifier {
        Identifier::new(name.to_string())
    }

    fn ident_expr(name: &str) -> Expression {
        Expression::Identifier(ident(name))
    }

    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn emit(expr: &Expression) -> String {
        let mut e = MySqlExpressionEmitter::default_config();
        e.emit_expression(expr).unwrap()
    }

    // -- Task 3.3: basic nodes (Literal / Identifier / QualifiedIdentifier) -

    #[test]
    fn visit_literal_string() {
        assert_eq!(
            emit(&Expression::Literal(Literal::String("hello".to_string()))),
            "'hello'"
        );
    }

    #[test]
    fn visit_literal_string_escapes_single_quote() {
        assert_eq!(
            emit(&Expression::Literal(Literal::String("it's".to_string()))),
            "'it''s'"
        );
    }

    #[test]
    fn visit_literal_integer() {
        assert_eq!(emit(&int_expr(42)), "42");
    }

    #[test]
    fn visit_literal_integer_negative() {
        assert_eq!(emit(&int_expr(-7)), "-7");
    }

    #[test]
    fn visit_literal_float_preserves_precision_string() {
        assert_eq!(
            emit(&Expression::Literal(Literal::Float("3.14".to_string()))),
            "3.14"
        );
    }

    #[test]
    fn visit_literal_null() {
        assert_eq!(emit(&Expression::Literal(Literal::Null)), "NULL");
    }

    #[test]
    fn visit_literal_boolean_true() {
        assert_eq!(emit(&Expression::Literal(Literal::Boolean(true))), "TRUE");
    }

    #[test]
    fn visit_literal_boolean_false() {
        assert_eq!(emit(&Expression::Literal(Literal::Boolean(false))), "FALSE");
    }

    #[test]
    fn visit_identifier_uses_value_accessor() {
        assert_eq!(emit(&ident_expr("users")), "`users`");
    }

    #[test]
    fn visit_qualified_identifier() {
        let expr = Expression::QualifiedIdentifier {
            table: ident("t"),
            column: ident("c"),
        };
        assert_eq!(emit(&expr), "`t`.`c`");
    }

    // -- Task 3.3: operator nodes (BinaryOp / UnaryOp / LogicalOp) ---------

    #[test]
    fn visit_binary_op_add() {
        let expr = Expression::BinaryOp {
            left: Box::new(int_expr(10)),
            op: BinOp::Add,
            right: Box::new(int_expr(5)),
        };
        assert_eq!(emit(&expr), "10 + 5");
    }

    #[test]
    fn visit_binary_op_all_arithmetic() {
        let ops = [
            (BinOp::Add, "+"),
            (BinOp::Sub, "-"),
            (BinOp::Mul, "*"),
            (BinOp::Div, "/"),
            (BinOp::Mod, "%"),
            (BinOp::Concat, "||"),
        ];
        for (op, sym) in ops {
            let expr = Expression::BinaryOp {
                left: Box::new(int_expr(1)),
                op,
                right: Box::new(int_expr(2)),
            };
            assert_eq!(emit(&expr), format!("1 {sym} 2"));
        }
    }

    #[test]
    fn visit_unary_op_minus() {
        let expr = Expression::UnaryOp {
            op: UnOp::Minus,
            expr: Box::new(int_expr(5)),
        };
        assert_eq!(emit(&expr), "-5");
    }

    #[test]
    fn visit_unary_op_plus() {
        let expr = Expression::UnaryOp {
            op: UnOp::Plus,
            expr: Box::new(int_expr(5)),
        };
        assert_eq!(emit(&expr), "+5");
    }

    #[test]
    fn visit_unary_op_not() {
        let expr = Expression::UnaryOp {
            op: UnOp::Not,
            expr: Box::new(Expression::Literal(Literal::Boolean(true))),
        };
        assert_eq!(emit(&expr), "NOT TRUE");
    }

    #[test]
    fn visit_logical_op_and() {
        let expr = Expression::LogicalOp {
            left: Box::new(Expression::Literal(Literal::Boolean(true))),
            op: LogOp::And,
            right: Box::new(Expression::Literal(Literal::Boolean(false))),
        };
        assert_eq!(emit(&expr), "TRUE AND FALSE");
    }

    #[test]
    fn visit_logical_op_or() {
        let expr = Expression::LogicalOp {
            left: Box::new(int_expr(1)),
            op: LogOp::Or,
            right: Box::new(int_expr(2)),
        };
        assert_eq!(emit(&expr), "1 OR 2");
    }

    // -- Task 3.3: comparison (split from binary) --------------------------

    #[test]
    fn visit_comparison_eq() {
        let expr = Expression::Comparison {
            left: Box::new(ident_expr("id")),
            op: CmpOp::Eq,
            right: Box::new(int_expr(1)),
        };
        assert_eq!(emit(&expr), "`id` = 1");
    }

    #[test]
    fn visit_comparison_all_operators() {
        let ops = [
            (CmpOp::Eq, "="),
            (CmpOp::Ne, "!="),
            (CmpOp::Lt, "<"),
            (CmpOp::Le, "<="),
            (CmpOp::Gt, ">"),
            (CmpOp::Ge, ">="),
            (CmpOp::Like, "LIKE"),
            (CmpOp::NotLike, "NOT LIKE"),
        ];
        for (op, sym) in ops {
            let expr = Expression::Comparison {
                left: Box::new(ident_expr("a")),
                op,
                right: Box::new(ident_expr("b")),
            };
            assert_eq!(emit(&expr), format!("`a` {sym} `b`"));
        }
    }

    #[test]
    fn visit_comparison_ilike_is_unsupported() {
        let expr = Expression::Comparison {
            left: Box::new(ident_expr("a")),
            op: CmpOp::ILike,
            right: Box::new(ident_expr("b")),
        };
        let mut e = MySqlExpressionEmitter::default_config();
        let result = e.emit_expression(&expr);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            EmitError::UnsupportedExpression { .. }
        ));
    }

    // -- Task 3.3: function (delegates to FunctionConverter) ---------------

    #[test]
    fn visit_function_getdate_maps_to_now() {
        let expr = Expression::Function {
            name: ident("GETDATE"),
            args: vec![],
            distinct: false,
        };
        assert_eq!(emit(&expr), "NOW()");
    }

    #[test]
    fn visit_function_isnull_maps_to_ifnull() {
        let expr = Expression::Function {
            name: ident("ISNULL"),
            args: vec![
                ident_expr("name"),
                Expression::Literal(Literal::String("N/A".to_string())),
            ],
            distinct: false,
        };
        assert_eq!(emit(&expr), "IFNULL(`name`, 'N/A')");
    }

    #[test]
    fn visit_function_with_distinct() {
        let expr = Expression::Function {
            name: ident("SUM"),
            args: vec![ident_expr("salary")],
            distinct: true,
        };
        assert_eq!(emit(&expr), "SUM(DISTINCT `salary`)");
    }

    #[test]
    fn visit_function_unknown_passes_through() {
        let expr = Expression::Function {
            name: ident("CUSTOM_FN"),
            args: vec![int_expr(1), int_expr(2)],
            distinct: false,
        };
        assert_eq!(emit(&expr), "CUSTOM_FN(1, 2)");
    }

    // -- Task 3.3: CASE ----------------------------------------------------

    #[test]
    fn visit_case_searched_compact() {
        let expr = Expression::Case {
            operand: None,
            conditions: vec![(
                Expression::Comparison {
                    left: Box::new(ident_expr("x")),
                    op: CmpOp::Gt,
                    right: Box::new(int_expr(0)),
                },
                Expression::Literal(Literal::String("pos".to_string())),
            )],
            else_result: Some(Box::new(Expression::Literal(Literal::String(
                "neg".to_string(),
            )))),
        };
        assert_eq!(emit(&expr), "CASE WHEN `x` > 0 THEN 'pos' ELSE 'neg' END");
    }

    #[test]
    fn visit_case_simple_with_operand() {
        let expr = Expression::Case {
            operand: Some(Box::new(ident_expr("x"))),
            conditions: vec![(
                int_expr(1),
                Expression::Literal(Literal::String("one".to_string())),
            )],
            else_result: None,
        };
        assert_eq!(emit(&expr), "CASE `x` WHEN 1 THEN 'one' END");
    }

    // -- Task 3.3: Subquery / Exists ---------------------------------------

    #[test]
    fn visit_subquery_wraps_in_parens() {
        let sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let expr = Expression::Subquery(Box::new(sel));
        assert_eq!(emit(&expr), "(SELECT *)");
    }

    #[test]
    fn visit_exists() {
        let sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let expr = Expression::Exists {
            subquery: Box::new(sel),
            negated: false,
        };
        assert_eq!(emit(&expr), "EXISTS (SELECT *)");
    }

    #[test]
    fn visit_not_exists() {
        let sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let expr = Expression::Exists {
            subquery: Box::new(sel),
            negated: true,
        };
        assert_eq!(emit(&expr), "NOT EXISTS (SELECT *)");
    }

    // -- Task 3.3: IN / BETWEEN --------------------------------------------

    #[test]
    fn visit_in_values() {
        let expr = Expression::In {
            expr: Box::new(ident_expr("id")),
            list: InList::Values(vec![int_expr(1), int_expr(2), int_expr(3)]),
            negated: false,
        };
        assert_eq!(emit(&expr), "`id` IN (1, 2, 3)");
    }

    #[test]
    fn visit_not_in_values() {
        let expr = Expression::In {
            expr: Box::new(ident_expr("id")),
            list: InList::Values(vec![int_expr(1), int_expr(2)]),
            negated: true,
        };
        assert_eq!(emit(&expr), "`id` NOT IN (1, 2)");
    }

    #[test]
    fn visit_in_subquery() {
        let sel = SelectStatement::simple(vec![SelectItem::Expression {
            expr: ident_expr("id"),
            alias: None,
        }]);
        let expr = Expression::In {
            expr: Box::new(ident_expr("user_id")),
            list: InList::Subquery(Box::new(sel)),
            negated: false,
        };
        assert_eq!(emit(&expr), "`user_id` IN (SELECT `id`)");
    }

    #[test]
    fn visit_between() {
        let expr = Expression::Between {
            expr: Box::new(ident_expr("age")),
            low: Box::new(int_expr(18)),
            high: Box::new(int_expr(65)),
            negated: false,
        };
        assert_eq!(emit(&expr), "`age` BETWEEN 18 AND 65");
    }

    #[test]
    fn visit_not_between() {
        let expr = Expression::Between {
            expr: Box::new(ident_expr("age")),
            low: Box::new(int_expr(18)),
            high: Box::new(int_expr(65)),
            negated: true,
        };
        assert_eq!(emit(&expr), "`age` NOT BETWEEN 18 AND 65");
    }

    // -- Task 3.3: CAST (new) ----------------------------------------------

    #[test]
    fn visit_cast_to_decimal() {
        let expr = Expression::Cast {
            expr: Box::new(ident_expr("price")),
            data_type: DataType::Decimal {
                precision: Some(18),
                scale: Some(4),
            },
        };
        assert_eq!(emit(&expr), "CAST(`price` AS DECIMAL(18, 4))");
    }

    #[test]
    fn visit_cast_to_varchar() {
        let expr = Expression::Cast {
            expr: Box::new(int_expr(123)),
            data_type: DataType::VarChar { length: Some(50) },
        };
        assert_eq!(emit(&expr), "CAST(123 AS VARCHAR(50))");
    }

    #[test]
    fn visit_cast_to_int() {
        let expr = Expression::Cast {
            expr: Box::new(ident_expr("v")),
            data_type: DataType::Int,
        };
        assert_eq!(emit(&expr), "CAST(`v` AS INT)");
    }

    // -- Task 3.3: IS NULL / IS NOT NULL ----------------------------------

    #[test]
    fn visit_is_null() {
        let expr = Expression::IsNull {
            expr: Box::new(ident_expr("email")),
            negated: false,
        };
        assert_eq!(emit(&expr), "`email` IS NULL");
    }

    #[test]
    fn visit_is_not_null() {
        let expr = Expression::IsNull {
            expr: Box::new(ident_expr("email")),
            negated: true,
        };
        assert_eq!(emit(&expr), "`email` IS NOT NULL");
    }

    // -- Nesting / complex expressions -------------------------------------

    #[test]
    fn nested_binary_in_comparison() {
        // (a + b) > 0
        let inner = Expression::BinaryOp {
            left: Box::new(ident_expr("a")),
            op: BinOp::Add,
            right: Box::new(ident_expr("b")),
        };
        let cmp = Expression::Comparison {
            left: Box::new(inner),
            op: CmpOp::Gt,
            right: Box::new(int_expr(0)),
        };
        assert_eq!(emit(&cmp), "`a` + `b` > 0");
    }

    #[test]
    fn logical_chain_with_is_null() {
        // x > 0 AND y IS NOT NULL
        let cmp = Expression::Comparison {
            left: Box::new(ident_expr("x")),
            op: CmpOp::Gt,
            right: Box::new(int_expr(0)),
        };
        let is_not_null = Expression::IsNull {
            expr: Box::new(ident_expr("y")),
            negated: true,
        };
        let and_expr = Expression::LogicalOp {
            left: Box::new(cmp),
            op: LogOp::And,
            right: Box::new(is_not_null),
        };
        assert_eq!(emit(&and_expr), "`x` > 0 AND `y` IS NOT NULL");
    }

    // -- Visitor trait contract (Req 1.5) ----------------------------------

    #[test]
    fn visitor_trait_dispatches_expression() {
        let mut e = MySqlExpressionEmitter::default_config();
        // Visitor::visit_expression は String を返す（エラーは last_error へ退避）
        let out: String = common_sql::Visitor::visit_expression(&mut e, &int_expr(42));
        assert_eq!(out, "42");
        assert!(e.last_error.is_none());
    }

    #[test]
    fn visitor_trait_stashes_error_on_ilike() {
        let mut e = MySqlExpressionEmitter::default_config();
        let bad = Expression::Comparison {
            left: Box::new(ident_expr("a")),
            op: CmpOp::ILike,
            right: Box::new(ident_expr("b")),
        };
        let out: String = common_sql::Visitor::visit_expression(&mut e, &bad);
        assert!(out.is_empty());
        assert!(e.last_error.is_some());
    }

    #[test]
    fn default_output_is_empty_string() {
        let e = MySqlExpressionEmitter::default_config();
        assert_eq!(Visitor::default_output(&e), String::new());
    }

    // -- Buffer isolation: nested emit does not corrupt parent buffer ------

    #[test]
    fn nested_emit_isolates_buffer() {
        // a = (SELECT 1)  — subquery 内の emit_expression が親バッファを汚さないこと
        let sel = SelectStatement::simple(vec![SelectItem::Expression {
            expr: int_expr(1),
            alias: None,
        }]);
        let expr = Expression::Comparison {
            left: Box::new(ident_expr("a")),
            op: CmpOp::Eq,
            right: Box::new(Expression::Subquery(Box::new(sel))),
        };
        assert_eq!(emit(&expr), "`a` = (SELECT 1)");
    }
}
