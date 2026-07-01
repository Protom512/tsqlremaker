//! PostgreSQL 式エミッター
//!
//! Common SQL AST の式を PostgreSQL SQL に変換します。
//!
//! P1 結合負債是正 (Issue #157): `tsql_parser::common::*` に依存せず、
//! リーンな `common_sql::ast` のみを消費する。

use common_sql::ast::{
    BinaryOperator, ComparisonOperator, Expression, Identifier, InList, Literal, LogicalOperator,
    SelectStatement, UnaryOperator,
};

use super::identifier::IdentifierQuoter;

/// 式エミッター
pub struct ExpressionEmitter;

impl ExpressionEmitter {
    /// 式をPostgreSQL SQL文字列に変換
    ///
    /// # Arguments
    ///
    /// * `expr` - 変換する式
    ///
    /// # Returns
    ///
    /// PostgreSQL SQLの式文字列
    #[must_use]
    pub fn emit(expr: &Expression) -> String {
        match expr {
            // リテラル
            Expression::Literal(lit) => Self::emit_literal(lit),

            // 識別子（単純名）
            Expression::Identifier(ident) => Self::emit_identifier(ident),

            // schema.table 形式の修飾識別子。
            // 旧 CommonColumnReference に相当: common-sql には ColumnReference バリアントが
            // 存在しないため、QualifiedIdentifier または Identifier へ畳み込む (注意点 b)。
            Expression::QualifiedIdentifier { table, column } => {
                Self::emit_qualified_identifier(table, column)
            }

            // 単項演算子
            Expression::UnaryOp { op, expr } => {
                format!("{} {}", Self::emit_unary_op(op), Self::emit(expr))
            }

            // 二項算術/文字列演算子 (注意点 a: 3分岐のうち BinaryOp)
            Expression::BinaryOp { left, op, right } => {
                format!(
                    "({} {} {})",
                    Self::emit(left),
                    Self::emit_binary_op(op),
                    Self::emit(right)
                )
            }

            // 比較演算子 (注意点 a: 3分岐のうち Comparison)
            Expression::Comparison { left, op, right } => {
                format!(
                    "({} {} {})",
                    Self::emit(left),
                    Self::emit_comparison_op(op),
                    Self::emit(right)
                )
            }

            // 論理演算子 (注意点 a: 3分岐のうち LogicalOp)
            Expression::LogicalOp { left, op, right } => {
                format!(
                    "({} {} {})",
                    Self::emit(left),
                    Self::emit_logical_op(op),
                    Self::emit(right)
                )
            }

            // 関数呼び出し (注意点 e: Function{name, args, distinct})
            Expression::Function {
                name,
                args,
                distinct,
            } => Self::emit_function(name, args, *distinct),

            // CASE式 (注意点 c: Case{operand, conditions, else_result})
            Expression::Case {
                operand,
                conditions,
                else_result,
            } => Self::emit_case(operand, conditions, else_result.as_deref()),

            // IN式 (注意点 d: In{expr, list:InList, negated})
            Expression::In {
                expr,
                list,
                negated,
            } => {
                let neg = if *negated { " NOT" } else { "" };
                let list_str = Self::emit_in_list(list);
                format!("{}{} IN {}", Self::emit(expr), neg, list_str)
            }

            // BETWEEN式
            Expression::Between {
                expr,
                low,
                high,
                negated,
            } => {
                let not_str = if *negated { "NOT " } else { "" };
                format!(
                    "{} {}BETWEEN {} AND {}",
                    Self::emit(expr),
                    not_str,
                    Self::emit(low),
                    Self::emit(high)
                )
            }

            // CAST式 (common-sql に存在するが旧 CommonExpression にはなかったバリアント。
            // common-sql への移行に伴い自然にサポートされるようになった)
            Expression::Cast { expr, data_type } => {
                format!(
                    "CAST({} AS {})",
                    Self::emit(expr),
                    Self::emit_data_type(data_type)
                )
            }

            // IS NULL
            Expression::IsNull { expr, negated } => {
                let not_str = if *negated { "NOT " } else { "" };
                format!("{} IS {}NULL", Self::emit(expr), not_str)
            }

            // サブクエリ (注意点: common-sql は Box<SelectStatement> を直接保持)
            Expression::Subquery(query) => {
                format!("({})", Self::emit_subquery(query))
            }

            // EXISTS
            Expression::Exists { subquery, negated } => {
                let not_str = if *negated { "NOT " } else { "" };
                format!("{}EXISTS ({})", not_str, Self::emit_subquery(subquery))
            }
        }
    }

    /// リテラル値を発行
    fn emit_literal(lit: &Literal) -> String {
        match lit {
            Literal::String(s) => format!("'{}'", s.replace('\'', "''")),
            Literal::Integer(n) => n.to_string(),
            // common-sql の Float は精度保持のため String (旧 CommonLiteral::Float(f64) とは異なる)
            Literal::Float(s) => s.clone(),
            Literal::Null => "NULL".to_string(),
            Literal::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        }
    }

    /// 識別子を発行
    fn emit_identifier(ident: &Identifier) -> String {
        let name = ident.value();
        // * はワイルドカードとして特別扱い（クォート不要）
        if name == "*" {
            return "*".to_string();
        }

        // IdentifierQuoterを使用して識別子をクォート
        IdentifierQuoter::quote(name)
    }

    /// 修飾識別子 (table.column) を発行。
    /// 旧 CommonColumnReference に相当する処理 (注意点 b)。
    fn emit_qualified_identifier(table: &Identifier, column: &Identifier) -> String {
        // 列側が "*" (テーブル修飾ワイルドカード) の特別扱い
        if column.value() == "*" {
            return format!("{}.*", Self::emit_identifier(table));
        }

        format!(
            "{}.{}",
            Self::emit_identifier(table),
            Self::emit_identifier(column)
        )
    }

    /// 単項演算子を発行
    fn emit_unary_op(op: &UnaryOperator) -> String {
        match op {
            UnaryOperator::Plus => "+".to_string(),
            UnaryOperator::Minus => "-".to_string(),
            UnaryOperator::Not => "NOT".to_string(),
        }
    }

    /// 二項算術/文字列演算子を発行 (注意点 a: BinaryOperator 分岐)
    fn emit_binary_op(op: &BinaryOperator) -> String {
        match op {
            BinaryOperator::Add => "+".to_string(),
            BinaryOperator::Sub => "-".to_string(),
            BinaryOperator::Mul => "*".to_string(),
            BinaryOperator::Div => "/".to_string(),
            BinaryOperator::Mod => "%".to_string(),
            BinaryOperator::Concat => "||".to_string(),
        }
    }

    /// 比較演算子を発行 (注意点 a: ComparisonOperator 分岐)
    /// common-sql では LIKE/ILIKE も ComparisonOperator に統合されている。
    fn emit_comparison_op(op: &ComparisonOperator) -> String {
        match op {
            ComparisonOperator::Eq => "=".to_string(),
            ComparisonOperator::Ne => "<>".to_string(),
            ComparisonOperator::Lt => "<".to_string(),
            ComparisonOperator::Le => "<=".to_string(),
            ComparisonOperator::Gt => ">".to_string(),
            ComparisonOperator::Ge => ">=".to_string(),
            ComparisonOperator::Like => "LIKE".to_string(),
            ComparisonOperator::NotLike => "NOT LIKE".to_string(),
            // PostgreSQL 拡張: ILIKE は common-sql で定義されるが PostgreSQL ネイティブで支持される
            ComparisonOperator::ILike => "ILIKE".to_string(),
            ComparisonOperator::NotILike => "NOT ILIKE".to_string(),
        }
    }

    /// 論理演算子を発行 (注意点 a: LogicalOperator 分岐)
    fn emit_logical_op(op: &LogicalOperator) -> String {
        match op {
            LogicalOperator::And => "AND".to_string(),
            LogicalOperator::Or => "OR".to_string(),
        }
    }

    /// 関数呼び出しを発行 (注意点 e)
    fn emit_function(name: &Identifier, args: &[Expression], distinct: bool) -> String {
        let args_str: Vec<String> = args.iter().map(Self::emit).collect();
        let distinct_str = if distinct { "DISTINCT " } else { "" };
        format!("{}({}{})", name.value(), distinct_str, args_str.join(", "))
    }

    /// CASE式を発行 (注意点 c)
    /// common-sql の Case は `operand` (simple CASE の被験式), `conditions` (WHEN/THEN 対),
    /// `else_result` を持つ。旧 CommonCaseExpression.branches/else_result から形状変更。
    fn emit_case(
        operand: &Option<Box<Expression>>,
        conditions: &[(Expression, Expression)],
        else_result: Option<&Expression>,
    ) -> String {
        let mut parts = vec!["CASE".to_string()];

        // simple CASE (operand あり) の場合は "CASE <operand>"
        if let Some(operand_expr) = operand {
            parts[0] = format!("CASE {}", Self::emit(operand_expr));
        }

        for (when_expr, then_expr) in conditions {
            parts.push(format!(
                "    WHEN {} THEN {}",
                Self::emit(when_expr),
                Self::emit(then_expr)
            ));
        }

        if let Some(else_expr) = else_result {
            parts.push(format!("    ELSE {}", Self::emit(else_expr)));
        }

        parts.push("END".to_string());
        parts.join("\n")
    }

    /// INリストを発行 (注意点 d)
    fn emit_in_list(list: &InList) -> String {
        match list {
            InList::Values(values) => {
                let items: Vec<String> = values.iter().map(Self::emit).collect();
                format!("({})", items.join(", "))
            }
            InList::Subquery(query) => {
                // サブクエリをレンダリング (注意点 d: InList::Subquery(Box<SelectStatement>))
                format!("({})", super::SelectStatementRenderer::emit(query))
            }
        }
    }

    /// サブクエリを発行
    fn emit_subquery(query: &SelectStatement) -> String {
        // SelectStatementRenderer を使用してサブクエリをレンダリング
        super::SelectStatementRenderer::emit(query)
    }

    /// データ型を発行 (CAST 式で使用)。
    /// 詳細なデータ型マッピングは datatype.rs (Task のスコープ外) に委譲し、
    /// ここでは PostgreSQL の基本的なデータ型名を生成する。
    fn emit_data_type(data_type: &common_sql::ast::DataType) -> String {
        use common_sql::ast::DataType;
        match data_type {
            DataType::TinyInt => "SMALLINT".to_string(),
            DataType::SmallInt => "SMALLINT".to_string(),
            DataType::Int => "INTEGER".to_string(),
            DataType::BigInt => "BIGINT".to_string(),
            DataType::Boolean => "BOOLEAN".to_string(),
            DataType::VarChar { length } => match length {
                Some(len) => format!("VARCHAR({len})"),
                None => "VARCHAR".to_string(),
            },
            DataType::Char { length } => match length {
                Some(len) => format!("CHAR({len})"),
                None => "CHAR".to_string(),
            },
            DataType::Text => "TEXT".to_string(),
            DataType::Real => "REAL".to_string(),
            DataType::DoublePrecision => "DOUBLE PRECISION".to_string(),
            DataType::Decimal { precision, scale } => match (precision, scale) {
                (Some(p), Some(s)) => format!("DECIMAL({p},{s})"),
                (Some(p), None) => format!("DECIMAL({p})"),
                (None, _) => "DECIMAL".to_string(),
            },
            DataType::Numeric { precision, scale } => match (precision, scale) {
                (Some(p), Some(s)) => format!("NUMERIC({p},{s})"),
                (Some(p), None) => format!("NUMERIC({p})"),
                (None, _) => "NUMERIC".to_string(),
            },
            DataType::Date => "DATE".to_string(),
            DataType::Timestamp { .. } => "TIMESTAMP".to_string(),
            _ => "TEXT".to_string(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::{Identifier, Literal, SelectItem, SelectStatement};

    // ---- ヘルパー ----

    fn id(name: &str) -> Identifier {
        Identifier::new(name.to_string())
    }

    fn id_expr(name: &str) -> Expression {
        Expression::Identifier(id(name))
    }

    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn simple_select_all_from(table: &str) -> SelectStatement {
        use common_sql::ast::{QualifiedName, TableFactor};
        SelectStatement {
            span: common_sql::ast::Span::new(0, 0),
            with: None,
            projection: vec![SelectItem::Wildcard],
            from: Some(TableFactor::Table {
                name: QualifiedName::new(None, table.to_string()),
                alias: None,
            }),
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
        }
    }

    // ============================================================
    // リテラル
    // ============================================================

    #[test]
    fn emit_literal_string() {
        let lit = Literal::String("hello".to_string());
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "'hello'");
    }

    #[test]
    fn emit_literal_string_with_quote() {
        let lit = Literal::String("it's".to_string());
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "'it''s'");
    }

    #[test]
    fn emit_literal_integer() {
        let lit = Literal::Integer(42);
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "42");
    }

    #[test]
    fn emit_literal_integer_negative() {
        let lit = Literal::Integer(-7);
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "-7");
    }

    #[test]
    fn emit_literal_float_preserves_string() {
        // common-sql の Float は精度保持のために String を保持する。
        // f64 への変換で精度が失われないことを検証。
        let lit = Literal::Float("123.456".to_string());
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "123.456");
    }

    #[test]
    fn emit_literal_float_decimal_precision() {
        let lit = Literal::Float("123456789012.3456".to_string());
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "123456789012.3456");
    }

    #[test]
    fn emit_literal_null() {
        let lit = Literal::Null;
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "NULL");
    }

    #[test]
    fn emit_literal_boolean_true_false() {
        assert_eq!(
            ExpressionEmitter::emit_literal(&Literal::Boolean(true)),
            "TRUE"
        );
        assert_eq!(
            ExpressionEmitter::emit_literal(&Literal::Boolean(false)),
            "FALSE"
        );
    }

    // ============================================================
    // 識別子 (注意点 b: Identifier への畳み込み)
    // ============================================================

    #[test]
    fn emit_identifier_lowercase_no_quote() {
        assert_eq!(ExpressionEmitter::emit_identifier(&id("users")), "users");
    }

    #[test]
    fn emit_identifier_underscore_no_quote() {
        assert_eq!(
            ExpressionEmitter::emit_identifier(&id("user_name")),
            "user_name"
        );
    }

    #[test]
    fn emit_identifier_uppercase_quoted() {
        assert_eq!(
            ExpressionEmitter::emit_identifier(&id("Users")),
            "\"Users\""
        );
    }

    #[test]
    fn emit_identifier_mixed_case_quoted() {
        assert_eq!(
            ExpressionEmitter::emit_identifier(&id("UserId")),
            "\"UserId\""
        );
    }

    #[test]
    fn emit_identifier_wildcard_star() {
        // * はワイルドカードとして特別扱い (クォート不要)
        assert_eq!(ExpressionEmitter::emit_identifier(&id("*")), "*");
    }

    // ============================================================
    // QualifiedIdentifier (注意点 b: ColumnReference → QualifiedIdentifier)
    // ============================================================

    #[test]
    fn emit_qualified_identifier_simple() {
        let expr = Expression::QualifiedIdentifier {
            table: id("users"),
            column: id("id"),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "users.id");
    }

    #[test]
    fn emit_qualified_identifier_uppercase_table_quoted() {
        let expr = Expression::QualifiedIdentifier {
            table: id("Users"),
            column: id("id"),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "\"Users\".id");
    }

    #[test]
    fn emit_qualified_identifier_uppercase_column_quoted() {
        let expr = Expression::QualifiedIdentifier {
            table: id("users"),
            column: id("ID"),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "users.\"ID\"");
    }

    #[test]
    fn emit_qualified_identifier_table_qualified_wildcard() {
        // table.* 形式 (テーブル修飾ワイルドカード)
        let expr = Expression::QualifiedIdentifier {
            table: id("users"),
            column: id("*"),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "users.*");
    }

    // ============================================================
    // 単項演算子 (exhaustive: Plus / Minus / Not)
    // ============================================================

    #[test]
    fn emit_unary_op_plus() {
        assert_eq!(ExpressionEmitter::emit_unary_op(&UnaryOperator::Plus), "+");
    }

    #[test]
    fn emit_unary_op_minus() {
        assert_eq!(ExpressionEmitter::emit_unary_op(&UnaryOperator::Minus), "-");
    }

    #[test]
    fn emit_unary_op_not() {
        assert_eq!(ExpressionEmitter::emit_unary_op(&UnaryOperator::Not), "NOT");
    }

    #[test]
    fn emit_unary_expression_minus() {
        let expr = Expression::UnaryOp {
            op: UnaryOperator::Minus,
            expr: Box::new(int_expr(5)),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "- 5");
    }

    #[test]
    fn emit_unary_expression_not() {
        let expr = Expression::UnaryOp {
            op: UnaryOperator::Not,
            expr: Box::new(id_expr("flag")),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "NOT flag");
    }

    // ============================================================
    // 二項算術/文字列演算子 (注意点 a: BinaryOperator, exhaustive)
    // ============================================================

    #[test]
    fn emit_binary_op_add() {
        assert_eq!(ExpressionEmitter::emit_binary_op(&BinaryOperator::Add), "+");
    }

    #[test]
    fn emit_binary_op_sub() {
        assert_eq!(ExpressionEmitter::emit_binary_op(&BinaryOperator::Sub), "-");
    }

    #[test]
    fn emit_binary_op_mul() {
        assert_eq!(ExpressionEmitter::emit_binary_op(&BinaryOperator::Mul), "*");
    }

    #[test]
    fn emit_binary_op_div() {
        assert_eq!(ExpressionEmitter::emit_binary_op(&BinaryOperator::Div), "/");
    }

    #[test]
    fn emit_binary_op_mod() {
        assert_eq!(ExpressionEmitter::emit_binary_op(&BinaryOperator::Mod), "%");
    }

    #[test]
    fn emit_binary_op_concat() {
        assert_eq!(
            ExpressionEmitter::emit_binary_op(&BinaryOperator::Concat),
            "||"
        );
    }

    #[test]
    fn emit_binary_expression_arithmetic() {
        let expr = Expression::BinaryOp {
            left: Box::new(int_expr(1)),
            op: BinaryOperator::Add,
            right: Box::new(int_expr(2)),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "(1 + 2)");
    }

    #[test]
    fn emit_binary_expression_concat() {
        let expr = Expression::BinaryOp {
            left: Box::new(id_expr("a")),
            op: BinaryOperator::Concat,
            right: Box::new(id_expr("b")),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "(a || b)");
    }

    // ============================================================
    // 比較演算子 (注意点 a: ComparisonOperator, exhaustive)
    // ============================================================

    #[test]
    fn emit_comparison_op_eq() {
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::Eq),
            "="
        );
    }

    #[test]
    fn emit_comparison_op_ne() {
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::Ne),
            "<>"
        );
    }

    #[test]
    fn emit_comparison_op_lt() {
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::Lt),
            "<"
        );
    }

    #[test]
    fn emit_comparison_op_le() {
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::Le),
            "<="
        );
    }

    #[test]
    fn emit_comparison_op_gt() {
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::Gt),
            ">"
        );
    }

    #[test]
    fn emit_comparison_op_ge() {
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::Ge),
            ">="
        );
    }

    #[test]
    fn emit_comparison_op_like() {
        // common-sql では LIKE は ComparisonOperator に統合 (旧 CommonExpression::Like とは異なる)
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::Like),
            "LIKE"
        );
    }

    #[test]
    fn emit_comparison_op_not_like() {
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::NotLike),
            "NOT LIKE"
        );
    }

    #[test]
    fn emit_comparison_op_ilike() {
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::ILike),
            "ILIKE"
        );
    }

    #[test]
    fn emit_comparison_op_not_ilike() {
        assert_eq!(
            ExpressionEmitter::emit_comparison_op(&ComparisonOperator::NotILike),
            "NOT ILIKE"
        );
    }

    #[test]
    fn emit_comparison_expression_eq() {
        let expr = Expression::Comparison {
            left: Box::new(id_expr("id")),
            op: ComparisonOperator::Eq,
            right: Box::new(int_expr(1)),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "(id = 1)");
    }

    #[test]
    fn emit_comparison_expression_ne() {
        let expr = Expression::Comparison {
            left: Box::new(id_expr("status")),
            op: ComparisonOperator::Ne,
            right: Box::new(Expression::Literal(Literal::String("x".to_string()))),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "(status <> 'x')");
    }

    // ============================================================
    // 論理演算子 (注意点 a: LogicalOperator, exhaustive)
    // ============================================================

    #[test]
    fn emit_logical_op_and() {
        assert_eq!(
            ExpressionEmitter::emit_logical_op(&LogicalOperator::And),
            "AND"
        );
    }

    #[test]
    fn emit_logical_op_or() {
        assert_eq!(
            ExpressionEmitter::emit_logical_op(&LogicalOperator::Or),
            "OR"
        );
    }

    #[test]
    fn emit_logical_expression_and() {
        let expr = Expression::LogicalOp {
            left: Box::new(id_expr("a")),
            op: LogicalOperator::And,
            right: Box::new(id_expr("b")),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "(a AND b)");
    }

    #[test]
    fn emit_logical_expression_or() {
        let expr = Expression::LogicalOp {
            left: Box::new(id_expr("a")),
            op: LogicalOperator::Or,
            right: Box::new(id_expr("b")),
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "(a OR b)");
    }

    // ============================================================
    // 関数呼び出し (注意点 e: Function{name, args, distinct})
    // ============================================================

    #[test]
    fn emit_function_basic() {
        let expr = Expression::Function {
            name: id("COUNT"),
            args: vec![id_expr("id")],
            distinct: false,
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "COUNT(id)");
    }

    #[test]
    fn emit_function_distinct() {
        let expr = Expression::Function {
            name: id("SUM"),
            args: vec![id_expr("salary")],
            distinct: true,
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "SUM(DISTINCT salary)");
    }

    #[test]
    fn emit_function_no_args() {
        let expr = Expression::Function {
            name: id("NOW"),
            args: vec![],
            distinct: false,
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "NOW()");
    }

    #[test]
    fn emit_function_multiple_args() {
        let expr = Expression::Function {
            name: id("COALESCE"),
            args: vec![
                id_expr("a"),
                id_expr("b"),
                Expression::Literal(Literal::Null),
            ],
            distinct: false,
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "COALESCE(a, b, NULL)");
    }

    // ============================================================
    // CASE式 (注意点 c: Case{operand, conditions, else_result})
    // ============================================================

    #[test]
    fn emit_case_searched_no_else() {
        let expr = Expression::Case {
            operand: None,
            conditions: vec![(
                Expression::Comparison {
                    left: Box::new(id_expr("x")),
                    op: ComparisonOperator::Gt,
                    right: Box::new(int_expr(0)),
                },
                Expression::Literal(Literal::String("pos".to_string())),
            )],
            else_result: None,
        };
        let result = ExpressionEmitter::emit(&expr);
        assert!(result.starts_with("CASE\n"));
        assert!(result.contains("    WHEN (x > 0) THEN 'pos'"));
        assert!(result.ends_with("\nEND"));
        assert!(!result.contains("ELSE"));
    }

    #[test]
    fn emit_case_searched_with_else() {
        let expr = Expression::Case {
            operand: None,
            conditions: vec![(
                Expression::Comparison {
                    left: Box::new(id_expr("x")),
                    op: ComparisonOperator::Lt,
                    right: Box::new(int_expr(0)),
                },
                Expression::Literal(Literal::String("neg".to_string())),
            )],
            else_result: Some(Box::new(Expression::Literal(Literal::String(
                "zero".to_string(),
            )))),
        };
        let result = ExpressionEmitter::emit(&expr);
        assert!(result.contains("    WHEN (x < 0) THEN 'neg'"));
        assert!(result.contains("    ELSE 'zero'"));
    }

    #[test]
    fn emit_case_simple_with_operand() {
        // simple CASE: CASE x WHEN 1 THEN 'one' ELSE 'other' END
        let expr = Expression::Case {
            operand: Some(Box::new(id_expr("x"))),
            conditions: vec![(
                int_expr(1),
                Expression::Literal(Literal::String("one".to_string())),
            )],
            else_result: Some(Box::new(Expression::Literal(Literal::String(
                "other".to_string(),
            )))),
        };
        let result = ExpressionEmitter::emit(&expr);
        // simple CASE は "CASE x" で始まる
        assert!(result.starts_with("CASE x\n"));
        assert!(result.contains("    WHEN 1 THEN 'one'"));
        assert!(result.contains("    ELSE 'other'"));
    }

    #[test]
    fn emit_case_multiple_branches() {
        let expr = Expression::Case {
            operand: None,
            conditions: vec![
                (
                    Expression::Comparison {
                        left: Box::new(id_expr("x")),
                        op: ComparisonOperator::Eq,
                        right: Box::new(int_expr(1)),
                    },
                    Expression::Literal(Literal::String("one".to_string())),
                ),
                (
                    Expression::Comparison {
                        left: Box::new(id_expr("x")),
                        op: ComparisonOperator::Eq,
                        right: Box::new(int_expr(2)),
                    },
                    Expression::Literal(Literal::String("two".to_string())),
                ),
            ],
            else_result: None,
        };
        let result = ExpressionEmitter::emit(&expr);
        assert!(result.contains("    WHEN (x = 1) THEN 'one'"));
        assert!(result.contains("    WHEN (x = 2) THEN 'two'"));
    }

    // ============================================================
    // IN式 (注意点 d: In{expr, list:InList, negated})
    // ============================================================

    #[test]
    fn emit_in_values() {
        let expr = Expression::In {
            expr: Box::new(id_expr("status")),
            list: InList::Values(vec![
                Expression::Literal(Literal::String("active".to_string())),
                Expression::Literal(Literal::String("pending".to_string())),
            ]),
            negated: false,
        };
        assert_eq!(
            ExpressionEmitter::emit(&expr),
            "status IN ('active', 'pending')"
        );
    }

    #[test]
    fn emit_in_values_negated() {
        let expr = Expression::In {
            expr: Box::new(id_expr("id")),
            list: InList::Values(vec![int_expr(1), int_expr(2)]),
            negated: true,
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "id NOT IN (1, 2)");
    }

    #[test]
    fn emit_in_subquery() {
        let sub = SelectStatement::simple(vec![SelectItem::Expression {
            expr: id_expr("id"),
            alias: None,
        }]);
        let expr = Expression::In {
            expr: Box::new(id_expr("user_id")),
            list: InList::Subquery(Box::new(sub)),
            negated: false,
        };
        let result = ExpressionEmitter::emit(&expr);
        assert!(result.starts_with("user_id IN ("));
    }

    // ============================================================
    // BETWEEN
    // ============================================================

    #[test]
    fn emit_between() {
        let expr = Expression::Between {
            expr: Box::new(id_expr("age")),
            low: Box::new(int_expr(18)),
            high: Box::new(int_expr(65)),
            negated: false,
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "age BETWEEN 18 AND 65");
    }

    #[test]
    fn emit_not_between() {
        let expr = Expression::Between {
            expr: Box::new(id_expr("age")),
            low: Box::new(int_expr(18)),
            high: Box::new(int_expr(65)),
            negated: true,
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "age NOT BETWEEN 18 AND 65");
    }

    // ============================================================
    // IS NULL
    // ============================================================

    #[test]
    fn emit_is_null() {
        let expr = Expression::IsNull {
            expr: Box::new(id_expr("email")),
            negated: false,
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "email IS NULL");
    }

    #[test]
    fn emit_is_not_null() {
        let expr = Expression::IsNull {
            expr: Box::new(id_expr("email")),
            negated: true,
        };
        assert_eq!(ExpressionEmitter::emit(&expr), "email IS NOT NULL");
    }

    // ============================================================
    // サブクエリ / EXISTS (SelectStatementRenderer シグネチャ更新の確認)
    // ============================================================

    #[test]
    fn emit_subquery_expression() {
        let sub = simple_select_all_from("users");
        let expr = Expression::Subquery(Box::new(sub));
        let result = ExpressionEmitter::emit(&expr);
        assert!(result.contains("(SELECT * FROM users)"));
    }

    #[test]
    fn emit_exists_subquery() {
        let sub = simple_select_all_from("users");
        let expr = Expression::Exists {
            subquery: Box::new(sub),
            negated: false,
        };
        let result = ExpressionEmitter::emit(&expr);
        assert!(result.contains("EXISTS (SELECT * FROM users)"));
    }

    #[test]
    fn emit_not_exists_subquery() {
        let sub = simple_select_all_from("users");
        let expr = Expression::Exists {
            subquery: Box::new(sub),
            negated: true,
        };
        let result = ExpressionEmitter::emit(&expr);
        assert!(result.contains("NOT EXISTS (SELECT * FROM users)"));
    }

    // ============================================================
    // 複合式 (入れ子構造の回帰テスト)
    // ============================================================

    #[test]
    fn emit_nested_binary_in_comparison() {
        // (a + b) > 0
        let add = Expression::BinaryOp {
            left: Box::new(id_expr("a")),
            op: BinaryOperator::Add,
            right: Box::new(id_expr("b")),
        };
        let cmp = Expression::Comparison {
            left: Box::new(add),
            op: ComparisonOperator::Gt,
            right: Box::new(int_expr(0)),
        };
        assert_eq!(ExpressionEmitter::emit(&cmp), "((a + b) > 0)");
    }

    #[test]
    fn emit_logical_chain() {
        // (x = 1) AND (y = 2)
        let left = Expression::Comparison {
            left: Box::new(id_expr("x")),
            op: ComparisonOperator::Eq,
            right: Box::new(int_expr(1)),
        };
        let right = Expression::Comparison {
            left: Box::new(id_expr("y")),
            op: ComparisonOperator::Eq,
            right: Box::new(int_expr(2)),
        };
        let and = Expression::LogicalOp {
            left: Box::new(left),
            op: LogicalOperator::And,
            right: Box::new(right),
        };
        assert_eq!(ExpressionEmitter::emit(&and), "((x = 1) AND (y = 2))");
    }
}
