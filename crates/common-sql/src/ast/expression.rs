//! SQL expression nodes.
//!
//! Covers literals, identifiers, operators, function calls, CASE expressions,
//! subqueries, EXISTS, IN, BETWEEN, CAST, and IS NULL.

use crate::ast::datatype::DataType;
use crate::ast::identifier::Identifier;
use crate::ast::literal::Literal;
use crate::ast::statement::SelectStatement;

// ---------------------------------------------------------------------------
// Operator enums (Task 3.1)
// ---------------------------------------------------------------------------

/// Binary arithmetic / string operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    /// Addition (`+`).
    Add,
    /// Subtraction (`-`).
    Sub,
    /// Multiplication (`*`).
    Mul,
    /// Division (`/`).
    Div,
    /// Modulo (`%`).
    Mod,
    /// String concatenation (`||`).
    Concat,
}

/// Unary prefix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOperator {
    /// Positive sign (`+expr`).
    Plus,
    /// Negative sign (`-expr`).
    Minus,
    /// Logical negation (`NOT expr`).
    Not,
}

/// Logical connective operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalOperator {
    /// Logical AND.
    And,
    /// Logical OR.
    Or,
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComparisonOperator {
    /// Equal (`=`).
    Eq,
    /// Not equal (`!=` / `<>`).
    Ne,
    /// Less than (`<`).
    Lt,
    /// Less than or equal (`<=`).
    Le,
    /// Greater than (`>`).
    Gt,
    /// Greater than or equal (`>=`).
    Ge,
    /// Pattern match (`LIKE`).
    Like,
    /// Negated pattern match (`NOT LIKE`).
    NotLike,
    /// Case-insensitive pattern match (`ILIKE` — PostgreSQL).
    ILike,
    /// Negated case-insensitive pattern match (`NOT ILIKE`).
    NotILike,
}

// ---------------------------------------------------------------------------
// InList (Task 3.2)
// ---------------------------------------------------------------------------

/// The right-hand side of an `IN` expression.
#[derive(Debug, Clone, PartialEq)]
pub enum InList {
    /// Explicit value list: `expr IN (1, 2, 3)`.
    Values(Vec<Expression>),
    /// Subquery: `expr IN (SELECT ...)`.
    Subquery(Box<SelectStatement>),
}

// ---------------------------------------------------------------------------
// Expression (Tasks 2.3 + 3.1 + 3.2)
// ---------------------------------------------------------------------------

/// A SQL expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // ----- Task 2.3: basic nodes -----
    /// A literal value.
    Literal(Literal),
    /// A simple identifier.
    Identifier(Identifier),
    /// A schema-qualified identifier (`table.column`).
    QualifiedIdentifier {
        /// Table or schema qualifier.
        table: Identifier,
        /// Column name.
        column: Identifier,
    },

    // ----- Task 3.1: operator nodes -----
    /// Binary arithmetic / string operation.
    BinaryOp {
        /// Left-hand side.
        left: Box<Expression>,
        /// The operator.
        op: BinaryOperator,
        /// Right-hand side.
        right: Box<Expression>,
    },
    /// Unary prefix operation.
    UnaryOp {
        /// The operator.
        op: UnaryOperator,
        /// The operand.
        expr: Box<Expression>,
    },
    /// Logical connective.
    LogicalOp {
        /// Left-hand side.
        left: Box<Expression>,
        /// The operator.
        op: LogicalOperator,
        /// Right-hand side.
        right: Box<Expression>,
    },
    /// Comparison expression.
    Comparison {
        /// Left-hand side.
        left: Box<Expression>,
        /// The operator.
        op: ComparisonOperator,
        /// Right-hand side.
        right: Box<Expression>,
    },

    // ----- Task 3.2: advanced nodes -----
    /// Function call: `name([DISTINCT] args...)`.
    Function {
        /// Function name.
        name: Identifier,
        /// Argument list.
        args: Vec<Expression>,
        /// Whether `DISTINCT` was specified.
        distinct: bool,
    },
    /// CASE expression.
    Case {
        /// Optional CASE operand (simple CASE).
        operand: Option<Box<Expression>>,
        /// `(when, then)` pairs.
        conditions: Vec<(Expression, Expression)>,
        /// ELSE result.
        else_result: Option<Box<Expression>>,
    },
    /// Scalar subquery: `(SELECT ...)`.
    Subquery(Box<SelectStatement>),
    /// `EXISTS (SELECT ...)` / `NOT EXISTS (SELECT ...)`.
    Exists {
        /// The subquery.
        subquery: Box<SelectStatement>,
        /// `true` for `NOT EXISTS`.
        negated: bool,
    },
    /// `expr IN (list)` / `expr NOT IN (list)`.
    In {
        /// The expression before `IN`.
        expr: Box<Expression>,
        /// Value list or subquery.
        list: InList,
        /// `true` for `NOT IN`.
        negated: bool,
    },
    /// `expr BETWEEN low AND high` / `expr NOT BETWEEN low AND high`.
    Between {
        /// The expression before `BETWEEN`.
        expr: Box<Expression>,
        /// Lower bound.
        low: Box<Expression>,
        /// Upper bound.
        high: Box<Expression>,
        /// `true` for `NOT BETWEEN`.
        negated: bool,
    },
    /// `CAST(expr AS data_type)`.
    Cast {
        /// The expression to cast.
        expr: Box<Expression>,
        /// Target data type.
        data_type: DataType,
    },
    /// `expr IS NULL` / `expr IS NOT NULL`.
    IsNull {
        /// The expression to test.
        expr: Box<Expression>,
        /// `true` for `IS NOT NULL`.
        negated: bool,
    },
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::ast::SelectItem;

    // ===== Task 2.3: basic expression tests =====

    #[test]
    fn literal_expression() {
        let expr = Expression::Literal(Literal::Integer(42));
        assert!(matches!(expr, Expression::Literal(Literal::Integer(42))));
    }

    #[test]
    fn identifier_expression() {
        let expr = Expression::Identifier(Identifier::new("col".to_string()));
        assert!(matches!(expr, Expression::Identifier(_)));
    }

    #[test]
    fn qualified_identifier_expression() {
        let expr = Expression::QualifiedIdentifier {
            table: Identifier::new("t".to_string()),
            column: Identifier::new("c".to_string()),
        };
        assert!(matches!(expr, Expression::QualifiedIdentifier { .. }));
    }

    // ===== Task 3.1: operator expression tests =====

    #[test]
    fn binary_op_expression() {
        let expr = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(1))),
            op: BinaryOperator::Add,
            right: Box::new(Expression::Literal(Literal::Integer(2))),
        };
        if let Expression::BinaryOp { op, .. } = &expr {
            assert_eq!(*op, BinaryOperator::Add);
        } else {
            panic!("expected BinaryOp");
        }
    }

    #[test]
    fn unary_op_expression() {
        let expr = Expression::UnaryOp {
            op: UnaryOperator::Minus,
            expr: Box::new(Expression::Literal(Literal::Integer(5))),
        };
        if let Expression::UnaryOp { op, .. } = &expr {
            assert_eq!(*op, UnaryOperator::Minus);
        } else {
            panic!("expected UnaryOp");
        }
    }

    #[test]
    fn logical_op_expression() {
        let expr = Expression::LogicalOp {
            left: Box::new(Expression::Literal(Literal::Boolean(true))),
            op: LogicalOperator::And,
            right: Box::new(Expression::Literal(Literal::Boolean(false))),
        };
        if let Expression::LogicalOp { op, .. } = &expr {
            assert_eq!(*op, LogicalOperator::And);
        } else {
            panic!("expected LogicalOp");
        }
    }

    #[test]
    fn comparison_expression() {
        let expr = Expression::Comparison {
            left: Box::new(Expression::Identifier(Identifier::new("x".to_string()))),
            op: ComparisonOperator::Eq,
            right: Box::new(Expression::Literal(Literal::Integer(1))),
        };
        if let Expression::Comparison { op, .. } = &expr {
            assert_eq!(*op, ComparisonOperator::Eq);
        } else {
            panic!("expected Comparison");
        }
    }

    #[test]
    fn nested_binary_op() {
        // (1 + 2) * 3
        let inner = Expression::BinaryOp {
            left: Box::new(Expression::Literal(Literal::Integer(1))),
            op: BinaryOperator::Add,
            right: Box::new(Expression::Literal(Literal::Integer(2))),
        };
        let outer = Expression::BinaryOp {
            left: Box::new(inner),
            op: BinaryOperator::Mul,
            right: Box::new(Expression::Literal(Literal::Integer(3))),
        };
        if let Expression::BinaryOp { op, .. } = &outer {
            assert_eq!(*op, BinaryOperator::Mul);
        } else {
            panic!("expected BinaryOp");
        }
    }

    // ===== Task 3.2: advanced expression tests =====

    // --- Function ---

    #[test]
    fn function_call_basic() {
        let expr = Expression::Function {
            name: Identifier::new("COUNT".to_string()),
            args: vec![Expression::Identifier(Identifier::new("id".to_string()))],
            distinct: false,
        };
        if let Expression::Function {
            name,
            args,
            distinct,
        } = &expr
        {
            assert_eq!(name.value(), "COUNT");
            assert_eq!(args.len(), 1);
            assert!(!distinct);
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn function_call_distinct() {
        let expr = Expression::Function {
            name: Identifier::new("SUM".to_string()),
            args: vec![Expression::Identifier(Identifier::new(
                "salary".to_string(),
            ))],
            distinct: true,
        };
        if let Expression::Function { distinct, .. } = &expr {
            assert!(distinct);
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn function_call_no_args() {
        let expr = Expression::Function {
            name: Identifier::new("NOW".to_string()),
            args: vec![],
            distinct: false,
        };
        if let Expression::Function { args, .. } = &expr {
            assert!(args.is_empty());
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn function_call_multiple_args() {
        let expr = Expression::Function {
            name: Identifier::new("COALESCE".to_string()),
            args: vec![
                Expression::Identifier(Identifier::new("a".to_string())),
                Expression::Identifier(Identifier::new("b".to_string())),
                Expression::Literal(Literal::Null),
            ],
            distinct: false,
        };
        if let Expression::Function { args, .. } = &expr {
            assert_eq!(args.len(), 3);
        } else {
            panic!("expected Function");
        }
    }

    // --- CASE ---

    #[test]
    fn case_simple() {
        // CASE x WHEN 1 THEN 'one' WHEN 2 THEN 'two' ELSE 'other' END
        let expr = Expression::Case {
            operand: Some(Box::new(Expression::Identifier(Identifier::new(
                "x".to_string(),
            )))),
            conditions: vec![
                (
                    Expression::Literal(Literal::Integer(1)),
                    Expression::Literal(Literal::String("one".to_string())),
                ),
                (
                    Expression::Literal(Literal::Integer(2)),
                    Expression::Literal(Literal::String("two".to_string())),
                ),
            ],
            else_result: Some(Box::new(Expression::Literal(Literal::String(
                "other".to_string(),
            )))),
        };
        if let Expression::Case {
            operand,
            conditions,
            else_result,
        } = &expr
        {
            assert!(operand.is_some());
            assert_eq!(conditions.len(), 2);
            assert!(else_result.is_some());
        } else {
            panic!("expected Case");
        }
    }

    #[test]
    fn case_searched() {
        // CASE WHEN x > 0 THEN 'pos' WHEN x < 0 THEN 'neg' END
        let expr = Expression::Case {
            operand: None,
            conditions: vec![(
                Expression::Comparison {
                    left: Box::new(Expression::Identifier(Identifier::new("x".to_string()))),
                    op: ComparisonOperator::Gt,
                    right: Box::new(Expression::Literal(Literal::Integer(0))),
                },
                Expression::Literal(Literal::String("pos".to_string())),
            )],
            else_result: None,
        };
        if let Expression::Case {
            operand,
            conditions,
            else_result,
        } = &expr
        {
            assert!(operand.is_none());
            assert_eq!(conditions.len(), 1);
            assert!(else_result.is_none());
        } else {
            panic!("expected Case");
        }
    }

    // --- Subquery ---

    #[test]
    fn subquery_expression() {
        let sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let expr = Expression::Subquery(Box::new(sel));
        assert!(matches!(expr, Expression::Subquery(_)));
    }

    #[test]
    fn subquery_in_select_list() {
        // SELECT (SELECT MAX(id) FROM t) AS max_id
        let sub = Expression::Subquery(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: Expression::Function {
                    name: Identifier::new("MAX".to_string()),
                    args: vec![Expression::Identifier(Identifier::new("id".to_string()))],
                    distinct: false,
                },
                alias: None,
            },
        ])));
        assert!(matches!(sub, Expression::Subquery(_)));
    }

    // --- EXISTS ---

    #[test]
    fn exists_expression() {
        let sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let expr = Expression::Exists {
            subquery: Box::new(sel),
            negated: false,
        };
        if let Expression::Exists { negated, .. } = &expr {
            assert!(!negated);
        } else {
            panic!("expected Exists");
        }
    }

    #[test]
    fn not_exists_expression() {
        let sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let expr = Expression::Exists {
            subquery: Box::new(sel),
            negated: true,
        };
        if let Expression::Exists { negated, .. } = &expr {
            assert!(negated);
        } else {
            panic!("expected Exists");
        }
    }

    // --- IN ---

    #[test]
    fn in_values_expression() {
        let expr = Expression::In {
            expr: Box::new(Expression::Identifier(Identifier::new(
                "status".to_string(),
            ))),
            list: InList::Values(vec![
                Expression::Literal(Literal::String("active".to_string())),
                Expression::Literal(Literal::String("pending".to_string())),
            ]),
            negated: false,
        };
        if let Expression::In { list, negated, .. } = &expr {
            assert!(!negated);
            if let InList::Values(vals) = list {
                assert_eq!(vals.len(), 2);
            } else {
                panic!("expected InList::Values");
            }
        } else {
            panic!("expected In");
        }
    }

    #[test]
    fn not_in_values_expression() {
        let expr = Expression::In {
            expr: Box::new(Expression::Identifier(Identifier::new("id".to_string()))),
            list: InList::Values(vec![Expression::Literal(Literal::Integer(1))]),
            negated: true,
        };
        if let Expression::In { negated, .. } = &expr {
            assert!(negated);
        } else {
            panic!("expected In");
        }
    }

    #[test]
    fn in_subquery_expression() {
        let sel = SelectStatement::simple(vec![SelectItem::Expression {
            expr: Expression::Identifier(Identifier::new("id".to_string())),
            alias: None,
        }]);
        let expr = Expression::In {
            expr: Box::new(Expression::Identifier(Identifier::new(
                "user_id".to_string(),
            ))),
            list: InList::Subquery(Box::new(sel)),
            negated: false,
        };
        if let Expression::In { list, .. } = &expr {
            assert!(matches!(list, InList::Subquery(_)));
        } else {
            panic!("expected In");
        }
    }

    // --- BETWEEN ---

    #[test]
    fn between_expression() {
        let expr = Expression::Between {
            expr: Box::new(Expression::Identifier(Identifier::new("age".to_string()))),
            low: Box::new(Expression::Literal(Literal::Integer(18))),
            high: Box::new(Expression::Literal(Literal::Integer(65))),
            negated: false,
        };
        if let Expression::Between { negated, .. } = &expr {
            assert!(!negated);
        } else {
            panic!("expected Between");
        }
    }

    #[test]
    fn not_between_expression() {
        let expr = Expression::Between {
            expr: Box::new(Expression::Identifier(Identifier::new("price".to_string()))),
            low: Box::new(Expression::Literal(Literal::Float("0.0".to_string()))),
            high: Box::new(Expression::Literal(Literal::Float("100.0".to_string()))),
            negated: true,
        };
        if let Expression::Between { negated, .. } = &expr {
            assert!(negated);
        } else {
            panic!("expected Between");
        }
    }

    // --- CAST ---

    #[test]
    fn cast_expression() {
        let expr = Expression::Cast {
            expr: Box::new(Expression::Identifier(Identifier::new("price".to_string()))),
            data_type: DataType::Decimal {
                precision: Some(18),
                scale: Some(4),
            },
        };
        if let Expression::Cast { data_type, .. } = &expr {
            assert_eq!(
                *data_type,
                DataType::Decimal {
                    precision: Some(18),
                    scale: Some(4),
                }
            );
        } else {
            panic!("expected Cast");
        }
    }

    #[test]
    fn cast_to_varchar() {
        let expr = Expression::Cast {
            expr: Box::new(Expression::Literal(Literal::Integer(123))),
            data_type: DataType::VarChar { length: Some(50) },
        };
        if let Expression::Cast { data_type, .. } = &expr {
            assert_eq!(*data_type, DataType::VarChar { length: Some(50) });
        } else {
            panic!("expected Cast");
        }
    }

    // --- IS NULL ---

    #[test]
    fn is_null_expression() {
        let expr = Expression::IsNull {
            expr: Box::new(Expression::Identifier(Identifier::new("email".to_string()))),
            negated: false,
        };
        if let Expression::IsNull { negated, .. } = &expr {
            assert!(!negated);
        } else {
            panic!("expected IsNull");
        }
    }

    #[test]
    fn is_not_null_expression() {
        let expr = Expression::IsNull {
            expr: Box::new(Expression::Identifier(Identifier::new("name".to_string()))),
            negated: true,
        };
        if let Expression::IsNull { negated, .. } = &expr {
            assert!(negated);
        } else {
            panic!("expected IsNull");
        }
    }

    // ===== Cross-cutting: equality and clone =====

    #[test]
    fn expression_equality() {
        let a = Expression::Literal(Literal::Integer(1));
        let b = Expression::Literal(Literal::Integer(1));
        assert_eq!(a, b);
    }

    #[test]
    fn expression_inequality() {
        let a = Expression::Literal(Literal::Integer(1));
        let b = Expression::Literal(Literal::Integer(2));
        assert_ne!(a, b);
    }

    #[test]
    fn expression_clone() {
        let expr = Expression::Function {
            name: Identifier::new("COUNT".to_string()),
            args: vec![Expression::Literal(Literal::Integer(1))],
            distinct: true,
        };
        let cloned = expr.clone();
        assert_eq!(expr, cloned);
    }

    #[test]
    fn complex_nested_expression() {
        // (a + b) > 0 AND EXISTS (SELECT 1) AND x IS NOT NULL
        let inner = Expression::BinaryOp {
            left: Box::new(Expression::Identifier(Identifier::new("a".to_string()))),
            op: BinaryOperator::Add,
            right: Box::new(Expression::Identifier(Identifier::new("b".to_string()))),
        };
        let cmp = Expression::Comparison {
            left: Box::new(inner),
            op: ComparisonOperator::Gt,
            right: Box::new(Expression::Literal(Literal::Integer(0))),
        };
        let exists = Expression::Exists {
            subquery: Box::new(SelectStatement::simple(vec![SelectItem::Expression {
                expr: Expression::Literal(Literal::Integer(1)),
                alias: None,
            }])),
            negated: false,
        };
        let is_not_null = Expression::IsNull {
            expr: Box::new(Expression::Identifier(Identifier::new("x".to_string()))),
            negated: true,
        };
        let and1 = Expression::LogicalOp {
            left: Box::new(cmp),
            op: LogicalOperator::And,
            right: Box::new(exists),
        };
        let full = Expression::LogicalOp {
            left: Box::new(and1),
            op: LogicalOperator::And,
            right: Box::new(is_not_null),
        };
        // Verify the full tree is a LogicalOp::And
        if let Expression::LogicalOp { op, .. } = &full {
            assert_eq!(*op, LogicalOperator::And);
        } else {
            panic!("expected LogicalOp");
        }
    }

    // ===== Operator enum tests =====

    #[test]
    fn all_binary_operators_are_copy() {
        let ops = [
            BinaryOperator::Add,
            BinaryOperator::Sub,
            BinaryOperator::Mul,
            BinaryOperator::Div,
            BinaryOperator::Mod,
            BinaryOperator::Concat,
        ];
        for op in ops {
            let copied = op;
            assert_eq!(op, copied);
        }
    }

    #[test]
    fn all_comparison_operators_are_copy() {
        let ops = [
            ComparisonOperator::Eq,
            ComparisonOperator::Ne,
            ComparisonOperator::Lt,
            ComparisonOperator::Le,
            ComparisonOperator::Gt,
            ComparisonOperator::Ge,
            ComparisonOperator::Like,
            ComparisonOperator::NotLike,
            ComparisonOperator::ILike,
            ComparisonOperator::NotILike,
        ];
        for op in ops {
            let copied = op;
            assert_eq!(op, copied);
        }
    }

    #[test]
    fn in_list_values_equality() {
        let a = InList::Values(vec![
            Expression::Literal(Literal::Integer(1)),
            Expression::Literal(Literal::Integer(2)),
        ]);
        let b = InList::Values(vec![
            Expression::Literal(Literal::Integer(1)),
            Expression::Literal(Literal::Integer(2)),
        ]);
        assert_eq!(a, b);
    }

    #[test]
    fn in_list_subquery_clone() {
        let sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let list = InList::Subquery(Box::new(sel));
        let cloned = list.clone();
        assert_eq!(list, cloned);
    }
}
