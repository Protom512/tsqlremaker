//! PostgreSQL 式エミッター
//!
//! Common SQL AST の式を PostgreSQL SQL に変換します。

use tsql_parser::common::{
    CommonBinaryOperator, CommonCaseExpression, CommonColumnReference, CommonExpression,
    CommonFunctionCall, CommonIdentifier, CommonInList, CommonLiteral, CommonUnaryOperator,
};

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
    pub fn emit(expr: &CommonExpression) -> String {
        match expr {
            // リテラル
            CommonExpression::Literal(lit) => Self::emit_literal(lit),

            // 識別子
            CommonExpression::Identifier(ident) => Self::emit_identifier(ident),

            // カラム参照
            CommonExpression::ColumnReference(col) => Self::emit_column_reference(col),

            // 単項演算子
            CommonExpression::UnaryOp { op, expr, .. } => {
                format!("{} {}", Self::emit_unary_op(op), Self::emit(expr))
            }

            // 二項演算子
            CommonExpression::BinaryOp {
                left, op, right, ..
            } => {
                format!(
                    "({} {} {})",
                    Self::emit(left),
                    Self::emit_binary_op(op),
                    Self::emit(right)
                )
            }

            // 関数呼び出し
            CommonExpression::FunctionCall(func) => Self::emit_function_call(func),

            // CASE式
            CommonExpression::Case(case) => Self::emit_case(case),

            // IN式
            CommonExpression::In {
                expr,
                list,
                negated,
                ..
            } => {
                let neg = if *negated { " NOT" } else { "" };
                let list_str = Self::emit_in_list(list);
                format!("{}{} IN {}", Self::emit(expr), neg, list_str)
            }

            // BETWEEN式
            CommonExpression::Between {
                expr,
                low,
                high,
                negated,
                ..
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

            // LIKE式
            CommonExpression::Like {
                expr,
                pattern,
                escape,
                negated,
                ..
            } => {
                let not_str = if *negated { "NOT " } else { "" };
                let escape_str = match escape {
                    Some(esc) => format!(" ESCAPE {}", Self::emit(esc)),
                    None => String::new(),
                };
                format!(
                    "{} {}LIKE {}{}",
                    Self::emit(expr),
                    not_str,
                    Self::emit(pattern),
                    escape_str
                )
            }

            // IS NULL
            CommonExpression::IsNull { expr, negated, .. } => {
                let not_str = if *negated { "NOT " } else { "" };
                format!("{} IS {}NULL", Self::emit(expr), not_str)
            }

            // サブクエリ
            CommonExpression::Subquery { query, .. } => {
                format!("({})", Self::emit_subquery(query))
            }

            // EXISTS
            CommonExpression::Exists { query, negated, .. } => {
                let not_str = if *negated { "NOT " } else { "" };
                format!("{}EXISTS ({})", not_str, Self::emit_subquery(query))
            }
        }
    }

    /// リテラル値を発行
    fn emit_literal(lit: &CommonLiteral) -> String {
        match lit {
            CommonLiteral::String(s) => format!("'{}'", s.replace('\'', "''")),
            CommonLiteral::Integer(n) => n.to_string(),
            CommonLiteral::Float(f) => f.to_string(),
            CommonLiteral::Null => "NULL".to_string(),
            CommonLiteral::Boolean(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        }
    }

    /// 識別子を発行
    fn emit_identifier(ident: &CommonIdentifier) -> String {
        // * はワイルドカードとして特別扱い（クォート不要）
        if ident.name == "*" {
            return "*".to_string();
        }

        // PostgreSQLの識別子は必要に応じて二重引用符で囲む
        let name = &ident.name;
        if needs_quoting(name) {
            format!("\"{}\"", name.replace('"', "\"\""))
        } else {
            name.clone()
        }
    }

    /// カラム参照を発行
    fn emit_column_reference(col: &CommonColumnReference) -> String {
        // * はワイルドカードとして特別扱い
        if col.column == "*" {
            return match &col.table {
                Some(table) => format!("{}.*", table),
                None => "*".to_string(),
            };
        }

        match &col.table {
            Some(table) => {
                format!(
                    "{}.{}",
                    Self::emit_identifier(&CommonIdentifier {
                        name: table.clone()
                    }),
                    Self::emit_identifier(&CommonIdentifier {
                        name: col.column.clone()
                    })
                )
            }
            None => Self::emit_identifier(&CommonIdentifier {
                name: col.column.clone(),
            }),
        }
    }

    /// 単項演算子を発行
    fn emit_unary_op(op: &CommonUnaryOperator) -> String {
        match op {
            CommonUnaryOperator::Plus => "+".to_string(),
            CommonUnaryOperator::Minus => "-".to_string(),
            CommonUnaryOperator::Not => "NOT".to_string(),
        }
    }

    /// 二項演算子を発行
    fn emit_binary_op(op: &CommonBinaryOperator) -> String {
        match op {
            CommonBinaryOperator::Plus => "+".to_string(),
            CommonBinaryOperator::Minus => "-".to_string(),
            CommonBinaryOperator::Multiply => "*".to_string(),
            CommonBinaryOperator::Divide => "/".to_string(),
            CommonBinaryOperator::Modulo => "%".to_string(),
            CommonBinaryOperator::Eq => "=".to_string(),
            CommonBinaryOperator::Ne => "<>".to_string(),
            CommonBinaryOperator::Lt => "<".to_string(),
            CommonBinaryOperator::Le => "<=".to_string(),
            CommonBinaryOperator::Gt => ">".to_string(),
            CommonBinaryOperator::Ge => ">=".to_string(),
            CommonBinaryOperator::And => "AND".to_string(),
            CommonBinaryOperator::Or => "OR".to_string(),
            CommonBinaryOperator::Concat => "||".to_string(),
        }
    }

    /// 関数呼び出しを発行
    fn emit_function_call(func: &CommonFunctionCall) -> String {
        let args: Vec<String> = func.args.iter().map(Self::emit).collect();
        let distinct = if func.distinct { "DISTINCT " } else { "" };
        format!("{}{}({})", func.name, distinct, args.join(", "))
    }

    /// CASE式を発行
    fn emit_case(case: &CommonCaseExpression) -> String {
        let mut parts = vec!["CASE".to_string()];

        for (cond, result) in &case.branches {
            parts.push(format!(
                "    WHEN {} THEN {}",
                Self::emit(cond),
                Self::emit(result)
            ));
        }

        if let Some(else_result) = &case.else_result {
            parts.push(format!("    ELSE {}", Self::emit(else_result)));
        }

        parts.push("END".to_string());
        parts.join("\n")
    }

    /// INリストを発行
    fn emit_in_list(list: &CommonInList) -> String {
        match list {
            CommonInList::Values(values) => {
                let items: Vec<String> = values.iter().map(Self::emit).collect();
                format!("({})", items.join(", "))
            }
            CommonInList::Subquery(query) => {
                // サブクエリをレンダリング
                format!("({})", super::SelectStatementRenderer::emit(query))
            }
        }
    }

    /// サブクエリを発行
    fn emit_subquery(query: &tsql_parser::common::CommonSelectStatement) -> String {
        // SelectStatementRenderer を使用してサブクエリをレンダリング
        super::SelectStatementRenderer::emit(query)
    }

    /// 識別子がクォートを必要とするか判定
    ///
    /// PostgreSQLでは以下の場合に識別子を二重引用符で囲む必要がある:
    /// - 大文字を含む（ケースを保存するため）
    /// - 数字で始まる
    /// - 予約語である
    /// - 特殊文字を含む
    /// - 空文字列
    ///
    /// ※純粋な小文字識別子はクォート不要（PostgreSQLが自動的に小文字に変換するため）
    #[allow(dead_code)]
    fn needs_quoting(name: &str) -> bool {
        if name.is_empty() {
            return true;
        }

        // 最初の文字を取得（空文字列はチェック済みなので Some が保証される）
        let first_char = match name.chars().next() {
            Some(c) => c,
            None => return true,
        };

        // 数字で始まる場合はクォートが必要
        if first_char.is_ascii_digit() {
            return true;
        }

        // 大文字を含む場合はクォートが必要（ケース保存のため）
        if name.chars().any(|c| c.is_ascii_uppercase()) {
            return true;
        }

        // 特殊文字を含む場合はクォートが必要
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return true;
        }

        false
    }
}

/// 識別子がクォートを必要とするか判定（ヘルパー関数）
#[allow(dead_code)]
fn needs_quoting(name: &str) -> bool {
    if name.is_empty() {
        return true;
    }

    // 最初の文字を取得（空文字列はチェック済みなので Some が保証される）
    let first_char = match name.chars().next() {
        Some(c) => c,
        None => return true,
    };

    // 数字で始まる場合はクォートが必要
    if first_char.is_ascii_digit() {
        return true;
    }

    // 大文字を含む場合はクォートが必要（ケース保存のため）
    if name.chars().any(|c| c.is_ascii_uppercase()) {
        return true;
    }

    // 特殊文字を含む場合はクォートが必要
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsql_parser::common::{CommonIdentifier, CommonLiteral};

    #[test]
    fn test_emit_literal_string() {
        let lit = CommonLiteral::String("hello".to_string());
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "'hello'");
    }

    #[test]
    fn test_emit_literal_string_with_quote() {
        let lit = CommonLiteral::String("it's".to_string());
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "'it''s'");
    }

    #[test]
    fn test_emit_literal_integer() {
        let lit = CommonLiteral::Integer(42);
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "42");
    }

    #[test]
    fn test_emit_literal_float() {
        let lit = CommonLiteral::Float(123.456);
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "123.456");
    }

    #[test]
    fn test_emit_literal_null() {
        let lit = CommonLiteral::Null;
        assert_eq!(ExpressionEmitter::emit_literal(&lit), "NULL");
    }

    #[test]
    fn test_emit_literal_boolean() {
        assert_eq!(
            ExpressionEmitter::emit_literal(&CommonLiteral::Boolean(true)),
            "TRUE"
        );
        assert_eq!(
            ExpressionEmitter::emit_literal(&CommonLiteral::Boolean(false)),
            "FALSE"
        );
    }

    #[test]
    fn test_emit_identifier() {
        let ident = CommonIdentifier {
            name: "Users".to_string(),
        };
        assert_eq!(ExpressionEmitter::emit_identifier(&ident), "\"Users\"");

        let lower_ident = CommonIdentifier {
            name: "users".to_string(),
        };
        assert_eq!(ExpressionEmitter::emit_identifier(&lower_ident), "users");

        let ident_with_underscore = CommonIdentifier {
            name: "user_name".to_string(),
        };
        assert_eq!(
            ExpressionEmitter::emit_identifier(&ident_with_underscore),
            "user_name"
        );

        let mixed_case = CommonIdentifier {
            name: "UserId".to_string(),
        };
        assert_eq!(
            ExpressionEmitter::emit_identifier(&mixed_case),
            "\"UserId\""
        );
    }

    #[test]
    fn test_emit_column_reference() {
        let col = CommonColumnReference {
            table: None,
            column: "id".to_string(),
        };
        assert_eq!(ExpressionEmitter::emit_column_reference(&col), "id");

        let qualified_col = CommonColumnReference {
            table: Some("Users".to_string()),
            column: "id".to_string(),
        };
        assert_eq!(
            ExpressionEmitter::emit_column_reference(&qualified_col),
            "\"Users\".id"
        );

        let uppercase_col = CommonColumnReference {
            table: None,
            column: "ID".to_string(),
        };
        assert_eq!(
            ExpressionEmitter::emit_column_reference(&uppercase_col),
            "\"ID\""
        );
    }

    #[test]
    fn test_emit_binary_op() {
        assert_eq!(
            ExpressionEmitter::emit_binary_op(&CommonBinaryOperator::Plus),
            "+"
        );
        assert_eq!(
            ExpressionEmitter::emit_binary_op(&CommonBinaryOperator::And),
            "AND"
        );
        assert_eq!(
            ExpressionEmitter::emit_binary_op(&CommonBinaryOperator::Concat),
            "||"
        );
    }

    #[test]
    fn test_emit_expression() {
        // リテラル式
        let expr = CommonExpression::Literal(CommonLiteral::Integer(42));
        assert_eq!(ExpressionEmitter::emit(&expr), "42");

        // 識別子（小文字 - クォート不要）
        let ident_expr = CommonExpression::Identifier(CommonIdentifier {
            name: "users".to_string(),
        });
        assert_eq!(ExpressionEmitter::emit(&ident_expr), "users");

        // 識別子（大文字 - クォート必要）
        let upper_ident_expr = CommonExpression::Identifier(CommonIdentifier {
            name: "Users".to_string(),
        });
        assert_eq!(ExpressionEmitter::emit(&upper_ident_expr), "\"Users\"");
    }

    #[test]
    fn test_emit_subquery() {
        use tsql_parser::common::{CommonSelectItem, CommonSelectStatement, CommonTableReference};
        use tsql_parser::Span;

        // サブクエリ式
        let subquery = CommonSelectStatement {
            span: Span { start: 0, end: 20 },
            distinct: false,
            columns: vec![CommonSelectItem::Wildcard],
            from: vec![CommonTableReference::Table {
                name: "users".to_string(),
                alias: None,
                span: Span { start: 7, end: 11 },
            }],
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let expr = CommonExpression::Subquery {
            query: Box::new(subquery),
            span: Span { start: 0, end: 22 },
        };

        let result = ExpressionEmitter::emit(&expr);
        assert!(result.contains("(SELECT * FROM users)"));
    }

    #[test]
    fn test_emit_exists_subquery() {
        use tsql_parser::common::{CommonSelectItem, CommonSelectStatement, CommonTableReference};
        use tsql_parser::Span;

        // EXISTS サブクエリ
        let subquery = CommonSelectStatement {
            span: Span { start: 8, end: 30 },
            distinct: false,
            columns: vec![CommonSelectItem::Wildcard],
            from: vec![CommonTableReference::Table {
                name: "users".to_string(),
                alias: None,
                span: Span { start: 15, end: 19 },
            }],
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let expr = CommonExpression::Exists {
            query: Box::new(subquery),
            negated: false,
            span: Span { start: 0, end: 32 },
        };

        let result = ExpressionEmitter::emit(&expr);
        assert!(result.contains("EXISTS (SELECT * FROM users)"));
    }
}
