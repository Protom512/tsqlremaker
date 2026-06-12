//! JOIN-related AST nodes.
//!
//! Covers join types, join conditions, table factors (table references that
//! appear in FROM / JOIN clauses), and the `DialectHint` extension point.

use crate::ast::expression::Expression;
use crate::ast::identifier::{Identifier, QualifiedName, TableAlias};
use crate::ast::span::Span;
use crate::ast::statement::SelectStatement;

// ---------------------------------------------------------------------------
// JoinType
// ---------------------------------------------------------------------------

/// The kind of JOIN operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoinType {
    /// `INNER JOIN` (or plain `JOIN`).
    Inner,
    /// `LEFT [OUTER] JOIN`.
    Left,
    /// `RIGHT [OUTER] JOIN`.
    Right,
    /// `FULL [OUTER] JOIN`.
    Full,
    /// `CROSS JOIN`.
    Cross,
}

// ---------------------------------------------------------------------------
// JoinCondition
// ---------------------------------------------------------------------------

/// The condition that follows a JOIN.
#[derive(Debug, Clone, PartialEq)]
pub enum JoinCondition {
    /// `ON expression` — explicit join condition.
    On(Expression),
    /// `USING (col1, col2, …)` — join on shared column names.
    Using(Vec<Identifier>),
    /// `NATURAL JOIN` — implicit join on matching column names.
    Natural,
}

// ---------------------------------------------------------------------------
// TableFactor
// ---------------------------------------------------------------------------

/// A table reference that can appear in a FROM or JOIN clause.
#[derive(Debug, Clone, PartialEq)]
pub enum TableFactor {
    /// A direct table reference, optionally qualified and aliased.
    Table {
        /// Qualified table name (`schema.table` or just `table`).
        name: QualifiedName,
        /// Optional alias (`AS alias`).
        alias: Option<TableAlias>,
    },
    /// A derived table (subquery in FROM clause).
    Derived {
        /// The subquery.
        subquery: Box<SelectStatement>,
        /// Optional alias — required by most dialects but modeled as optional.
        alias: Option<TableAlias>,
    },
    /// A nested JOIN expression (for chained joins).
    Join(Box<Join>),
}

// ---------------------------------------------------------------------------
// Join
// ---------------------------------------------------------------------------

/// A single JOIN operation.
///
/// Represents `left [join_type] JOIN right [condition]` where the left side
/// is implied by the position in the FROM clause or a surrounding `TableFactor::Join`.
#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    /// Source span of the entire JOIN clause.
    pub span: Span,
    /// The type of join (INNER, LEFT, RIGHT, FULL, CROSS).
    pub join_type: JoinType,
    /// The right-side table reference.
    pub table: TableFactor,
    /// The join condition (ON, USING, or NATURAL).
    pub condition: JoinCondition,
    /// Whether the LATERAL keyword was specified.
    pub lateral: bool,
}

// ---------------------------------------------------------------------------
// DialectHint
// ---------------------------------------------------------------------------

/// An opaque key-value pair for dialect-specific metadata.
///
/// Emitters can use this to carry extra information that does not map to any
/// standard AST node (e.g., vendor-specific hints like `/*+ INDEX(t idx) */`).
#[derive(Debug, Clone, PartialEq)]
pub struct DialectHint {
    /// Hint key.
    pub key: String,
    /// Hint value.
    pub value: String,
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
    use crate::ast::{SelectItem, Span};

    // ===== JoinType tests =====

    #[test]
    fn join_type_inner() {
        let jt = JoinType::Inner;
        assert_eq!(jt, JoinType::Inner);
    }

    #[test]
    fn join_type_all_variants_copy() {
        let variants = [
            JoinType::Inner,
            JoinType::Left,
            JoinType::Right,
            JoinType::Full,
            JoinType::Cross,
        ];
        for v in &variants {
            let copied = *v;
            assert_eq!(*v, copied);
        }
    }

    #[test]
    fn join_type_equality() {
        assert_eq!(JoinType::Left, JoinType::Left);
        assert_ne!(JoinType::Left, JoinType::Right);
    }

    // ===== JoinCondition tests =====

    #[test]
    fn join_condition_on() {
        let cond = JoinCondition::On(Expression::Comparison {
            left: Box::new(Expression::Identifier(Identifier::new("t1.id".to_string()))),
            op: crate::ast::ComparisonOperator::Eq,
            right: Box::new(Expression::Identifier(Identifier::new("t2.id".to_string()))),
        });
        assert!(matches!(cond, JoinCondition::On(_)));
    }

    #[test]
    fn join_condition_using() {
        let cond = JoinCondition::Using(vec![
            Identifier::new("id".to_string()),
            Identifier::new("dept_id".to_string()),
        ]);
        if let JoinCondition::Using(cols) = &cond {
            assert_eq!(cols.len(), 2);
            assert_eq!(cols[0].value(), "id");
            assert_eq!(cols[1].value(), "dept_id");
        } else {
            panic!("expected Using");
        }
    }

    #[test]
    fn join_condition_using_empty() {
        // Edge case: empty USING list
        let cond = JoinCondition::Using(vec![]);
        if let JoinCondition::Using(cols) = &cond {
            assert!(cols.is_empty());
        } else {
            panic!("expected Using");
        }
    }

    #[test]
    fn join_condition_natural() {
        let cond = JoinCondition::Natural;
        assert!(matches!(cond, JoinCondition::Natural));
    }

    // ===== TableFactor tests =====

    #[test]
    fn table_factor_table_simple() {
        let tf = TableFactor::Table {
            name: QualifiedName::new(None, "users".to_string()),
            alias: None,
        };
        if let TableFactor::Table { name, alias } = &tf {
            assert_eq!(name.name(), "users");
            assert!(alias.is_none());
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn table_factor_table_with_alias() {
        let tf = TableFactor::Table {
            name: QualifiedName::new(None, "users".to_string()),
            alias: Some(TableAlias::new("u".to_string(), vec![])),
        };
        if let TableFactor::Table { alias, .. } = &tf {
            let a = alias.as_ref().expect("alias should exist");
            assert_eq!(a.name(), "u");
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn table_factor_table_qualified() {
        let tf = TableFactor::Table {
            name: QualifiedName::new(Some("dbo".to_string()), "users".to_string()),
            alias: Some(TableAlias::new("u".to_string(), vec![])),
        };
        if let TableFactor::Table { name, .. } = &tf {
            assert_eq!(name.schema(), Some("dbo"));
            assert_eq!(name.name(), "users");
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn table_factor_derived() {
        let sub = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let tf = TableFactor::Derived {
            subquery: Box::new(sub),
            alias: Some(TableAlias::new("sub".to_string(), vec![])),
        };
        if let TableFactor::Derived { alias, .. } = &tf {
            let a = alias.as_ref().expect("alias should exist");
            assert_eq!(a.name(), "sub");
        } else {
            panic!("expected Derived");
        }
    }

    #[test]
    fn table_factor_derived_without_alias() {
        // Edge case: derived table without alias (some dialects allow this)
        let sub = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let tf = TableFactor::Derived {
            subquery: Box::new(sub),
            alias: None,
        };
        if let TableFactor::Derived { alias, .. } = &tf {
            assert!(alias.is_none());
        } else {
            panic!("expected Derived");
        }
    }

    #[test]
    fn table_factor_join() {
        let inner_join = Join {
            span: Span::new(0, 30),
            join_type: JoinType::Inner,
            table: TableFactor::Table {
                name: QualifiedName::new(None, "t2".to_string()),
                alias: None,
            },
            condition: JoinCondition::On(Expression::Comparison {
                left: Box::new(Expression::Identifier(Identifier::new("t1.id".to_string()))),
                op: crate::ast::ComparisonOperator::Eq,
                right: Box::new(Expression::Identifier(Identifier::new("t2.id".to_string()))),
            }),
            lateral: false,
        };
        let tf = TableFactor::Join(Box::new(inner_join));
        assert!(matches!(tf, TableFactor::Join(_)));
    }

    // ===== Join struct tests =====

    #[test]
    fn join_inner_basic() {
        let join = Join {
            span: Span::new(10, 50),
            join_type: JoinType::Inner,
            table: TableFactor::Table {
                name: QualifiedName::new(None, "orders".to_string()),
                alias: Some(TableAlias::new("o".to_string(), vec![])),
            },
            condition: JoinCondition::On(Expression::Comparison {
                left: Box::new(Expression::Identifier(Identifier::new("u.id".to_string()))),
                op: crate::ast::ComparisonOperator::Eq,
                right: Box::new(Expression::Identifier(Identifier::new(
                    "o.user_id".to_string(),
                ))),
            }),
            lateral: false,
        };
        assert_eq!(join.join_type, JoinType::Inner);
        assert!(!join.lateral);
        assert_eq!(join.span.start, 10);
        assert_eq!(join.span.end, 50);
    }

    #[test]
    fn join_left() {
        let join = Join {
            span: Span::default(),
            join_type: JoinType::Left,
            table: TableFactor::Table {
                name: QualifiedName::new(None, "profiles".to_string()),
                alias: None,
            },
            condition: JoinCondition::Natural,
            lateral: false,
        };
        assert_eq!(join.join_type, JoinType::Left);
        assert!(matches!(join.condition, JoinCondition::Natural));
    }

    #[test]
    fn join_cross() {
        let join = Join {
            span: Span::default(),
            join_type: JoinType::Cross,
            table: TableFactor::Table {
                name: QualifiedName::new(None, "categories".to_string()),
                alias: None,
            },
            condition: JoinCondition::Natural,
            lateral: false,
        };
        assert_eq!(join.join_type, JoinType::Cross);
    }

    #[test]
    fn join_right_with_using() {
        let join = Join {
            span: Span::default(),
            join_type: JoinType::Right,
            table: TableFactor::Table {
                name: QualifiedName::new(None, "departments".to_string()),
                alias: Some(TableAlias::new("d".to_string(), vec![])),
            },
            condition: JoinCondition::Using(vec![Identifier::new("dept_id".to_string())]),
            lateral: false,
        };
        assert_eq!(join.join_type, JoinType::Right);
        if let JoinCondition::Using(cols) = &join.condition {
            assert_eq!(cols.len(), 1);
        } else {
            panic!("expected Using");
        }
    }

    #[test]
    fn join_full_outer() {
        let join = Join {
            span: Span::new(0, 0),
            join_type: JoinType::Full,
            table: TableFactor::Table {
                name: QualifiedName::new(None, "countries".to_string()),
                alias: None,
            },
            condition: JoinCondition::On(Expression::Comparison {
                left: Box::new(Expression::Identifier(Identifier::new("c.id".to_string()))),
                op: crate::ast::ComparisonOperator::Eq,
                right: Box::new(Expression::Identifier(Identifier::new(
                    "r.country_id".to_string(),
                ))),
            }),
            lateral: false,
        };
        assert_eq!(join.join_type, JoinType::Full);
    }

    #[test]
    fn join_lateral() {
        // LATERAL JOIN: SELECT * FROM t1 JOIN LATERAL (SELECT ...) AS sub ON ...
        let sub = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let join = Join {
            span: Span::new(20, 80),
            join_type: JoinType::Inner,
            table: TableFactor::Derived {
                subquery: Box::new(sub),
                alias: Some(TableAlias::new("sub".to_string(), vec![])),
            },
            condition: JoinCondition::On(Expression::Literal(crate::ast::Literal::Boolean(true))),
            lateral: true,
        };
        assert!(join.lateral);
        assert!(matches!(join.table, TableFactor::Derived { .. }));
    }

    // ===== Chained JOIN (t1 JOIN t2 ON ... JOIN t3 ON ...) =====

    #[test]
    fn chained_join() {
        // t1 JOIN t2 ON t1.id = t2.id  ->  then JOIN t3 ON t2.id = t3.id
        let join_t2 = Join {
            span: Span::new(10, 40),
            join_type: JoinType::Inner,
            table: TableFactor::Table {
                name: QualifiedName::new(None, "t2".to_string()),
                alias: None,
            },
            condition: JoinCondition::On(Expression::Comparison {
                left: Box::new(Expression::Identifier(Identifier::new("t1.id".to_string()))),
                op: crate::ast::ComparisonOperator::Eq,
                right: Box::new(Expression::Identifier(Identifier::new("t2.id".to_string()))),
            }),
            lateral: false,
        };
        let join_t3 = Join {
            span: Span::new(41, 80),
            join_type: JoinType::Left,
            table: TableFactor::Join(Box::new(join_t2)),
            condition: JoinCondition::On(Expression::Comparison {
                left: Box::new(Expression::Identifier(Identifier::new("t2.id".to_string()))),
                op: crate::ast::ComparisonOperator::Eq,
                right: Box::new(Expression::Identifier(Identifier::new("t3.id".to_string()))),
            }),
            lateral: false,
        };
        // Verify the outer join is LEFT and contains an inner JOIN
        assert_eq!(join_t3.join_type, JoinType::Left);
        if let TableFactor::Join(inner) = &join_t3.table {
            assert_eq!(inner.join_type, JoinType::Inner);
        } else {
            panic!("expected nested Join");
        }
    }

    // ===== DialectHint tests =====

    #[test]
    fn dialect_hint_basic() {
        let hint = DialectHint {
            key: "index".to_string(),
            value: "idx_users_name".to_string(),
        };
        assert_eq!(hint.key, "index");
        assert_eq!(hint.value, "idx_users_name");
    }

    #[test]
    fn dialect_hint_equality() {
        let a = DialectHint {
            key: "hint".to_string(),
            value: "val".to_string(),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn dialect_hint_inequality() {
        let a = DialectHint {
            key: "hint".to_string(),
            value: "val1".to_string(),
        };
        let b = DialectHint {
            key: "hint".to_string(),
            value: "val2".to_string(),
        };
        assert_ne!(a, b);
    }

    // ===== Clone / Debug round-trip =====

    #[test]
    fn join_clone_equality() {
        let join = Join {
            span: Span::new(0, 10),
            join_type: JoinType::Inner,
            table: TableFactor::Table {
                name: QualifiedName::new(None, "t".to_string()),
                alias: None,
            },
            condition: JoinCondition::Natural,
            lateral: false,
        };
        let cloned = join.clone();
        assert_eq!(join, cloned);
    }

    #[test]
    fn table_factor_clone_equality() {
        let tf = TableFactor::Table {
            name: QualifiedName::new(Some("schema".to_string()), "tbl".to_string()),
            alias: Some(TableAlias::new("a".to_string(), vec!["c1".to_string()])),
        };
        let cloned = tf.clone();
        assert_eq!(tf, cloned);
    }

    #[test]
    fn join_condition_clone_equality() {
        let cond = JoinCondition::Using(vec![Identifier::new("id".to_string())]);
        let cloned = cond.clone();
        assert_eq!(cond, cloned);
    }

    // ===== Edge case: join without alias =====

    #[test]
    fn join_without_alias() {
        let join = Join {
            span: Span::default(),
            join_type: JoinType::Inner,
            table: TableFactor::Table {
                name: QualifiedName::new(None, "raw_table".to_string()),
                alias: None,
            },
            condition: JoinCondition::On(Expression::Literal(crate::ast::Literal::Boolean(true))),
            lateral: false,
        };
        if let TableFactor::Table { alias, .. } = &join.table {
            assert!(alias.is_none());
        } else {
            panic!("expected Table");
        }
    }
}
