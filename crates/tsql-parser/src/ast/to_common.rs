//! T-SQL AST から Common SQL AST への変換実装
//!
//! このモジュールでは、T-SQL 固有のASTノードを方言非依存の
//! Common SQL AST に変換するトレイト実装を提供する。

use crate::ast::data_modification::Assignment as ColumnAssignment;
use crate::ast::*;
use crate::common::*;

impl ToCommonAst for Statement {
    fn to_common_ast(&self) -> Option<CommonStatement> {
        match self {
            Statement::Select(stmt) => stmt.to_common_ast(),
            Statement::Insert(stmt) => stmt.to_common_ast(),
            Statement::Update(stmt) => stmt.to_common_ast(),
            Statement::Delete(stmt) => stmt.to_common_ast(),
            // 制御フロー文は方言固有として扱う
            Statement::Declare(_)
            | Statement::Set(_)
            | Statement::If(_)
            | Statement::While(_)
            | Statement::Block(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Return(_) => Some(CommonStatement::DialectSpecific {
                description: format!("{:?}", self),
                span: self.span(),
            }),
            // CREATE文も方言固有として扱う（DDLは実装で差が大きいため）
            Statement::Create(_) => Some(CommonStatement::DialectSpecific {
                description: format!("CREATE statement: {:?}", self),
                span: self.span(),
            }),
            // 変数代入文も方言固有
            Statement::VariableAssignment(_) => Some(CommonStatement::DialectSpecific {
                description: format!("Variable assignment: {:?}", self),
                span: self.span(),
            }),
            // バッチ区切りはCommon ASTには含めない
            Statement::BatchSeparator(_) => None,
        }
    }
}

impl ToCommonAst for SelectStatement {
    fn to_common_ast(&self) -> Option<CommonStatement> {
        let columns = self
            .columns
            .iter()
            .filter_map(|item| item.to_common())
            .collect();
        let from = self
            .from
            .as_ref()
            .and_then(|f| f.to_common())
            .unwrap_or_default();
        let where_clause = self
            .where_clause
            .as_ref()
            .and_then(|e| e.to_common_expression());
        let group_by = self
            .group_by
            .iter()
            .filter_map(|e| e.to_common_expression())
            .collect();
        let having = self.having.as_ref().and_then(|e| e.to_common_expression());

        let mut order_by = Vec::new();
        for item in &self.order_by {
            if let Some(expr) = item.expr.to_common_expression() {
                order_by.push(CommonOrderByItem {
                    expr,
                    asc: item.asc,
                });
            }
        }

        let limit = self.limit.as_ref().and_then(|l| l.to_common());

        Some(CommonStatement::Select(CommonSelectStatement {
            span: self.span,
            distinct: self.distinct,
            columns,
            from,
            where_clause,
            group_by,
            having,
            order_by,
            limit,
        }))
    }
}

impl ToCommonAst for InsertStatement {
    fn to_common_ast(&self) -> Option<CommonStatement> {
        let columns = self.columns.iter().map(|id| id.name.clone()).collect();
        let source = self.source.to_common()?;

        Some(CommonStatement::Insert(CommonInsertStatement {
            span: self.span,
            table: self.table.name.clone(),
            columns,
            source,
        }))
    }
}

impl ToCommonAst for UpdateStatement {
    fn to_common_ast(&self) -> Option<CommonStatement> {
        // FROM句がある場合は方言固有として扱う（ASE固有機能）
        if self.from_clause.is_some() {
            return Some(CommonStatement::DialectSpecific {
                description: "UPDATE with FROM clause (ASE-specific)".to_string(),
                span: self.span,
            });
        }

        let table = match &self.table {
            TableReference::Table { name, .. } => name.name.clone(),
            _ => {
                return Some(CommonStatement::DialectSpecific {
                    description: "UPDATE with complex table reference".to_string(),
                    span: self.span,
                });
            }
        };

        let mut assignments = Vec::new();
        for a in &self.assignments {
            if let Some(common) = a.to_common() {
                assignments.push(common);
            }
        }

        let where_clause = self
            .where_clause
            .as_ref()
            .and_then(|e| e.to_common_expression());

        Some(CommonStatement::Update(CommonUpdateStatement {
            span: self.span,
            table,
            assignments,
            where_clause,
        }))
    }
}

impl ToCommonAst for DeleteStatement {
    fn to_common_ast(&self) -> Option<CommonStatement> {
        // FROM句がある場合は方言固有として扱う
        if self.from_clause.is_some() {
            return Some(CommonStatement::DialectSpecific {
                description: "DELETE with FROM clause (ASE-specific)".to_string(),
                span: self.span,
            });
        }

        let where_clause = self
            .where_clause
            .as_ref()
            .and_then(|e| e.to_common_expression());

        Some(CommonStatement::Delete(CommonDeleteStatement {
            span: self.span,
            table: self.table.name.clone(),
            where_clause,
        }))
    }
}

// SelectItem の変換ヘルパー
trait SelectItemExt {
    fn to_common(&self) -> Option<CommonSelectItem>;
}

impl SelectItemExt for SelectItem {
    fn to_common(&self) -> Option<CommonSelectItem> {
        match self {
            SelectItem::Expression(expr, alias) => {
                let common_expr = expr.to_common_expression()?;
                let alias_str = alias.as_ref().map(|a| a.name.clone());
                Some(CommonSelectItem::Expression(common_expr, alias_str))
            }
            SelectItem::Wildcard => Some(CommonSelectItem::Wildcard),
            SelectItem::QualifiedWildcard(id) => {
                Some(CommonSelectItem::QualifiedWildcard(id.name.clone()))
            }
        }
    }
}

// FromClause の変換ヘルパー
trait FromClauseExt {
    fn to_common(&self) -> Option<Vec<CommonTableReference>>;
}

impl FromClauseExt for FromClause {
    fn to_common(&self) -> Option<Vec<CommonTableReference>> {
        let mut tables = Vec::new();
        for table_ref in &self.tables {
            match table_ref {
                TableReference::Table { name, alias, span } => {
                    tables.push(CommonTableReference::Table {
                        name: name.name.clone(),
                        alias: alias.as_ref().map(|a| a.name.clone()),
                        span: *span,
                    });
                }
                // サブクエリの変換
                TableReference::Subquery { query, alias, span } => {
                    let common_select = match query.to_common_ast()? {
                        CommonStatement::Select(s) => s,
                        _ => return None,
                    };
                    tables.push(CommonTableReference::Derived {
                        subquery: Box::new(common_select),
                        alias: alias.as_ref().map(|a| a.name.clone()),
                        span: *span,
                    });
                }
                TableReference::Joined { .. } => {
                    // JOIN は別途処理
                }
            }
        }
        Some(tables)
    }
}

// LimitClause の変換ヘルパー
trait LimitClauseExt {
    fn to_common(&self) -> Option<CommonLimitClause>;
}

impl LimitClauseExt for LimitClause {
    fn to_common(&self) -> Option<CommonLimitClause> {
        Some(CommonLimitClause {
            limit: self.limit.to_common_expression()?,
            offset: self.offset.as_ref().and_then(|e| e.to_common_expression()),
        })
    }
}

// InsertSource の変換ヘルパー
trait InsertSourceExt {
    fn to_common(&self) -> Option<CommonInsertSource>;
}

impl InsertSourceExt for InsertSource {
    fn to_common(&self) -> Option<CommonInsertSource> {
        match self {
            InsertSource::Values(rows) => {
                let mut common_rows = Vec::new();
                for row in rows {
                    let common_row: Vec<_> = row
                        .iter()
                        .filter_map(|e| e.to_common_expression())
                        .collect();
                    if common_row.len() == row.len() {
                        common_rows.push(common_row);
                    }
                }
                Some(CommonInsertSource::Values(common_rows))
            }
            InsertSource::Select(select) => {
                let common_select = match select.to_common_ast()? {
                    CommonStatement::Select(s) => s,
                    _ => return None,
                };
                Some(CommonInsertSource::Select(Box::new(common_select)))
            }
            InsertSource::DefaultValues => Some(CommonInsertSource::DefaultValues),
        }
    }
}

// Assignment の変換ヘルパー
trait AssignmentExt {
    fn to_common(&self) -> Option<CommonAssignment>;
}

impl AssignmentExt for ColumnAssignment {
    fn to_common(&self) -> Option<CommonAssignment> {
        Some(CommonAssignment {
            column: self.column.name.clone(),
            value: self.value.to_common_expression()?,
        })
    }
}

// 式の変換実装
impl ToCommonAst for Expression {
    fn to_common_ast(&self) -> Option<CommonStatement> {
        // 式から文への変換はサポートしない
        None
    }

    fn to_common_expression(&self) -> Option<CommonExpression> {
        match self {
            Expression::Literal(lit) => lit.to_common_expression(),
            Expression::Identifier(id) => id.to_common_expression(),
            Expression::ColumnReference(col) => col.to_common_expression(),
            Expression::UnaryOp { op, expr, span } => Some(CommonExpression::UnaryOp {
                op: op.to_common()?,
                expr: Box::new(expr.to_common_expression()?),
                span: *span,
            }),
            Expression::BinaryOp {
                left,
                op,
                right,
                span,
            } => Some(CommonExpression::BinaryOp {
                left: Box::new(left.to_common_expression()?),
                op: op.to_common()?,
                right: Box::new(right.to_common_expression()?),
                span: *span,
            }),
            Expression::FunctionCall(func) => func.to_common_expression(),
            Expression::Case(case) => case.to_common_expression(),
            Expression::In {
                expr,
                list,
                negated,
                span,
            } => {
                let common_list = match list {
                    InList::Values(vals) => {
                        let values: Vec<_> = vals
                            .iter()
                            .filter_map(|e| e.to_common_expression())
                            .collect();
                        CommonInList::Values(values)
                    }
                    InList::Subquery(select) => {
                        let common_select = match select.to_common_ast()? {
                            CommonStatement::Select(s) => s,
                            _ => return None,
                        };
                        CommonInList::Subquery(Box::new(common_select))
                    }
                };
                Some(CommonExpression::In {
                    expr: Box::new(expr.to_common_expression()?),
                    list: common_list,
                    negated: *negated,
                    span: *span,
                })
            }
            Expression::Between {
                expr,
                low,
                high,
                negated,
                span,
            } => Some(CommonExpression::Between {
                expr: Box::new(expr.to_common_expression()?),
                low: Box::new(low.to_common_expression()?),
                high: Box::new(high.to_common_expression()?),
                negated: *negated,
                span: *span,
            }),
            Expression::Like {
                expr,
                pattern,
                escape,
                negated,
                span,
            } => {
                let common_escape = match escape {
                    Some(e) => Some(Box::new(e.to_common_expression()?)),
                    None => None,
                };
                Some(CommonExpression::Like {
                    expr: Box::new(expr.to_common_expression()?),
                    pattern: Box::new(pattern.to_common_expression()?),
                    escape: common_escape,
                    negated: *negated,
                    span: *span,
                })
            }
            Expression::Is {
                expr,
                negated,
                value,
                span,
            } => {
                match value {
                    IsValue::Null | IsValue::Unknown => Some(CommonExpression::IsNull {
                        expr: Box::new(expr.to_common_expression()?),
                        negated: *negated,
                        span: *span,
                    }),
                    // IS TRUE/FALSE は通常の比較として扱う
                    _ => None,
                }
            }
            // サブクエリ式の変換
            Expression::Subquery(select) => {
                let span = select.span();
                let common_select = match select.to_common_ast()? {
                    CommonStatement::Select(s) => s,
                    _ => return None,
                };
                Some(CommonExpression::Subquery {
                    query: Box::new(common_select),
                    span,
                })
            }
            Expression::Exists(select) => {
                let span = select.span();
                let common_select = match select.to_common_ast()? {
                    CommonStatement::Select(s) => s,
                    _ => return None,
                };
                Some(CommonExpression::Exists {
                    query: Box::new(common_select),
                    negated: false,
                    span,
                })
            }
        }
    }
}

// リテラルの変換
trait LiteralExt {
    fn to_common_expression(&self) -> Option<CommonExpression>;
}

impl LiteralExt for Literal {
    fn to_common_expression(&self) -> Option<CommonExpression> {
        let common_lit = match self {
            Literal::String(s, _) => CommonLiteral::String(s.clone()),
            Literal::Number(n, _) => {
                // 整数パース
                n.parse::<i64>().ok().map(CommonLiteral::Integer)?
            }
            Literal::Float(f, _) => f.parse::<f64>().ok().map(CommonLiteral::Float)?,
            Literal::Hex(_, _) => {
                // 16進数は整数として扱う
                return None;
            }
            Literal::Null(_) => CommonLiteral::Null,
            Literal::Boolean(b, _) => CommonLiteral::Boolean(*b),
        };
        Some(CommonExpression::Literal(common_lit))
    }
}

// 識別子の変換
trait IdentifierExt {
    fn to_common_expression(&self) -> Option<CommonExpression>;
}

impl IdentifierExt for Identifier {
    fn to_common_expression(&self) -> Option<CommonExpression> {
        Some(CommonExpression::Identifier(CommonIdentifier {
            name: self.name.clone(),
        }))
    }
}

// カラム参照の変換
trait ColumnReferenceExt {
    fn to_common_expression(&self) -> Option<CommonExpression>;
}

impl ColumnReferenceExt for ColumnReference {
    fn to_common_expression(&self) -> Option<CommonExpression> {
        Some(CommonExpression::ColumnReference(CommonColumnReference {
            table: self.table.as_ref().map(|t| t.name.clone()),
            column: self.column.name.clone(),
        }))
    }
}

// 演算子の変換ヘルパー
trait UnaryOperatorExt {
    fn to_common(&self) -> Option<CommonUnaryOperator>;
}

impl UnaryOperatorExt for UnaryOperator {
    fn to_common(&self) -> Option<CommonUnaryOperator> {
        match self {
            UnaryOperator::Plus => Some(CommonUnaryOperator::Plus),
            UnaryOperator::Minus => Some(CommonUnaryOperator::Minus),
            UnaryOperator::Not => Some(CommonUnaryOperator::Not),
            UnaryOperator::Tilde => None, // ビット否定はCommon ASTに含めない
        }
    }
}

trait BinaryOperatorExt {
    fn to_common(&self) -> Option<CommonBinaryOperator>;
}

impl BinaryOperatorExt for BinaryOperator {
    fn to_common(&self) -> Option<CommonBinaryOperator> {
        match self {
            BinaryOperator::Plus => Some(CommonBinaryOperator::Plus),
            BinaryOperator::Minus => Some(CommonBinaryOperator::Minus),
            BinaryOperator::Multiply => Some(CommonBinaryOperator::Multiply),
            BinaryOperator::Divide => Some(CommonBinaryOperator::Divide),
            BinaryOperator::Modulo => Some(CommonBinaryOperator::Modulo),
            BinaryOperator::Eq | BinaryOperator::NeAlt => Some(CommonBinaryOperator::Eq),
            BinaryOperator::Ne => Some(CommonBinaryOperator::Ne),
            BinaryOperator::Lt => Some(CommonBinaryOperator::Lt),
            BinaryOperator::Le => Some(CommonBinaryOperator::Le),
            BinaryOperator::Gt => Some(CommonBinaryOperator::Gt),
            BinaryOperator::Ge => Some(CommonBinaryOperator::Ge),
            BinaryOperator::NotLt | BinaryOperator::NotGt => {
                // ASE固有演算子は変換しない
                None
            }
            BinaryOperator::And => Some(CommonBinaryOperator::And),
            BinaryOperator::Or => Some(CommonBinaryOperator::Or),
            BinaryOperator::In => None,      // INは別途式として扱う
            BinaryOperator::Between => None, // BETWEENは別途式として扱う
            BinaryOperator::Concat => Some(CommonBinaryOperator::Concat),
        }
    }
}

// 関数呼び出しの変換
trait FunctionCallExt {
    fn to_common_expression(&self) -> Option<CommonExpression>;
}

impl FunctionCallExt for FunctionCall {
    fn to_common_expression(&self) -> Option<CommonExpression> {
        let mut args = Vec::new();
        for arg in &self.args {
            match arg {
                FunctionArg::Expression(expr) => {
                    if let Some(common) = expr.to_common_expression() {
                        args.push(common);
                    }
                }
                FunctionArg::Wildcard | FunctionArg::QualifiedWildcard(_) => {
                    args.push(CommonExpression::Identifier(CommonIdentifier {
                        name: "*".to_string(),
                    }));
                }
            }
        }

        Some(CommonExpression::FunctionCall(CommonFunctionCall {
            name: self.name.name.clone(),
            args,
            distinct: self.distinct,
        }))
    }
}

// CASE式の変換
trait CaseExpressionExt {
    fn to_common_expression(&self) -> Option<CommonExpression>;
}

impl CaseExpressionExt for CaseExpression {
    fn to_common_expression(&self) -> Option<CommonExpression> {
        let mut branches = Vec::new();
        for (cond, result) in &self.branches {
            if let (Some(common_cond), Some(common_result)) =
                (cond.to_common_expression(), result.to_common_expression())
            {
                branches.push((common_cond, common_result));
            } else {
                return None;
            }
        }

        let else_result = self
            .else_result
            .as_ref()
            .and_then(|e| e.to_common_expression())
            .map(Box::new);

        Some(CommonExpression::Case(CommonCaseExpression {
            branches,
            else_result,
        }))
    }
}
