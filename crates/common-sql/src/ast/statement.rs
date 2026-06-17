//! SQL statement nodes.

use crate::ast::clause::{GroupByClause, LimitClause, OrderByClause, WithClause};
use crate::ast::identifier::Identifier;
use crate::ast::join::TableFactor;
use crate::ast::span::Span;
use crate::ast::Expression;

/// Top-level SQL statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// SELECT statement.
    Select(SelectStatement),
}

/// SELECT statement.
///
/// Full query representation covering projection, FROM (with JOIN support via
/// [`TableFactor`]), WHERE, GROUP BY, HAVING, ORDER BY, LIMIT, and an optional
/// WITH (CTE) clause.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    /// Source span of the entire statement.
    pub span: Span,
    /// `WITH` clause (one or more CTEs), if present.
    pub with: Option<WithClause>,
    /// Projection list (`SELECT` list).
    pub projection: Vec<SelectItem>,
    /// FROM clause (`None` for a `SELECT` with no FROM).
    pub from: Option<TableFactor>,
    /// WHERE clause.
    pub where_clause: Option<Expression>,
    /// GROUP BY clause.
    pub group_by: Option<GroupByClause>,
    /// HAVING clause.
    pub having: Option<Expression>,
    /// ORDER BY clause.
    pub order_by: Option<OrderByClause>,
    /// LIMIT / OFFSET clause.
    pub limit: Option<LimitClause>,
}

/// SELECT list item.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    /// An expression with optional alias.
    Expression {
        /// The projected expression.
        expr: Expression,
        /// Optional column alias (`AS name`).
        alias: Option<Identifier>,
    },
    /// `table.*` — all columns from a specific table.
    QualifiedWildcard {
        /// Table name.
        table: Identifier,
    },
    /// `*` — all columns.
    Wildcard,
}

// ---------------------------------------------------------------------------
// Minimal constructors for testing
// ---------------------------------------------------------------------------

impl SelectStatement {
    /// Create a minimal SELECT statement with just projection.
    ///
    /// All optional clauses (`with`, `from`, `where_clause`, `group_by`,
    /// `having`, `order_by`, `limit`) are initialized to `None`.
    #[must_use]
    pub fn simple(projection: Vec<SelectItem>) -> Self {
        Self {
            span: Span::new(0, 0),
            with: None,
            projection,
            from: None,
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
        }
    }
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
    use crate::ast::clause::{Cte, NullOrdering, SortDirection};
    use crate::ast::clause::{
        GroupByClause, GroupByItem, LimitClause, OrderByClause, OrderByItem, WithClause,
    };
    use crate::ast::identifier::{QualifiedName, TableAlias};
    use crate::ast::join::{Join, JoinCondition, JoinType, TableFactor};
    use crate::ast::literal::Literal;

    // -- helpers -------------------------------------------------------------

    fn ident_expr(name: &str) -> Expression {
        Expression::Identifier(Identifier::new(name.to_string()))
    }

    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn table_factor(name: &str) -> TableFactor {
        TableFactor::Table {
            name: QualifiedName::new(None, name.to_string()),
            alias: None,
        }
    }

    // -- simple() constructor: every optional clause defaults to None --------

    #[test]
    fn simple_initializes_all_new_fields_to_none() {
        let stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        assert!(stmt.with.is_none());
        assert!(stmt.from.is_none());
        assert!(stmt.where_clause.is_none());
        assert!(stmt.group_by.is_none());
        assert!(stmt.having.is_none());
        assert!(stmt.order_by.is_none());
        assert!(stmt.limit.is_none());
        assert_eq!(stmt.span, Span::new(0, 0));
    }

    #[test]
    fn simple_preserves_projection() {
        let proj = vec![
            SelectItem::Expression {
                expr: ident_expr("id"),
                alias: None,
            },
            SelectItem::Wildcard,
        ];
        let stmt = SelectStatement::simple(proj.clone());
        assert_eq!(stmt.projection.len(), 2);
        assert!(matches!(stmt.projection[1], SelectItem::Wildcard));
    }

    // -- struct-literal construction with every clause populated -------------

    #[test]
    fn full_select_statement_with_all_clauses() {
        let stmt = SelectStatement {
            span: Span::new(0, 100),
            with: Some(WithClause {
                recursive: false,
                ctes: vec![Cte {
                    name: "src".to_string(),
                    columns: vec![],
                    query: Box::new(SelectStatement::simple(vec![SelectItem::Wildcard])),
                    materialized: None,
                }],
            }),
            projection: vec![SelectItem::Wildcard],
            from: Some(table_factor("users")),
            where_clause: Some(ident_expr("active")),
            group_by: Some(GroupByClause {
                span: Span::new(20, 40),
                items: vec![GroupByItem::Expression(ident_expr("dept"))],
            }),
            having: Some(ident_expr("count")),
            order_by: Some(OrderByClause {
                span: Span::new(50, 70),
                items: vec![OrderByItem {
                    expr: ident_expr("name"),
                    direction: Some(SortDirection::Asc),
                    nulls: None,
                }],
            }),
            limit: Some(LimitClause {
                span: Span::new(80, 90),
                limit: int_expr(10),
                offset: None,
            }),
        };
        assert!(stmt.with.is_some());
        assert!(stmt.from.is_some());
        assert!(stmt.group_by.is_some());
        assert!(stmt.having.is_some());
        assert!(stmt.order_by.is_some());
        assert!(stmt.limit.is_some());
    }

    // -- individual clause assignment ---------------------------------------

    #[test]
    fn select_with_with_clause_recursive() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.with = Some(WithClause {
            recursive: true,
            ctes: vec![],
        });
        let wc = stmt.with.as_ref().expect("with should be set");
        assert!(wc.recursive);
        assert!(wc.ctes.is_empty());
    }

    #[test]
    fn select_from_table_factor() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(TableFactor::Table {
            name: QualifiedName::new(Some("dbo".to_string()), "orders".to_string()),
            alias: Some(TableAlias::new("o".to_string(), vec![])),
        });
        if let Some(TableFactor::Table { name, alias }) = &stmt.from {
            assert_eq!(name.schema(), Some("dbo"));
            assert_eq!(name.name(), "orders");
            assert_eq!(alias.as_ref().expect("alias").name(), "o");
        } else {
            panic!("expected Table factor");
        }
    }

    #[test]
    fn select_group_by_and_having() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.group_by = Some(GroupByClause {
            span: Span::new(0, 10),
            items: vec![
                GroupByItem::Expression(ident_expr("a")),
                GroupByItem::Rollup(vec![ident_expr("b")]),
            ],
        });
        stmt.having = Some(ident_expr("total"));
        assert_eq!(stmt.group_by.as_ref().expect("group_by").items.len(), 2);
        assert!(stmt.having.is_some());
    }

    #[test]
    fn select_order_by_with_nulls_ordering() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.order_by = Some(OrderByClause {
            span: Span::new(0, 20),
            items: vec![OrderByItem {
                expr: ident_expr("salary"),
                direction: Some(SortDirection::Desc),
                nulls: Some(NullOrdering::NullsFirst),
            }],
        });
        let ob = stmt.order_by.as_ref().expect("order_by");
        assert_eq!(ob.items[0].nulls, Some(NullOrdering::NullsFirst));
    }

    #[test]
    fn select_limit_with_offset() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.limit = Some(LimitClause {
            span: Span::new(0, 5),
            limit: int_expr(25),
            offset: Some(int_expr(5)),
        });
        let lim = stmt.limit.as_ref().expect("limit");
        assert_eq!(lim.offset, Some(int_expr(5)));
    }

    // -- Statement enum still wraps SelectStatement --------------------------

    #[test]
    fn statement_select_wraps_select_statement() {
        let inner = SelectStatement::simple(vec![SelectItem::Wildcard]);
        // Verify the wrapped statement carries the default empty shape before
        // wrapping (Statement currently has a single variant, so we inspect
        // the inner value directly rather than pattern-matching the enum).
        assert!(inner.from.is_none());
        assert!(inner.with.is_none());
        let stmt = Statement::Select(inner);
        // Document the wrapping relationship; this becomes a real discriminant
        // check once Task 4.3 adds Insert/Update/Delete variants.
        assert!(matches!(stmt, Statement::Select(_)));
        // Clone + PartialEq preserved through the new fields.
        assert_eq!(stmt.clone(), stmt);
    }

    // -- Recursive nesting (SelectStatement <-> TableFactor) -----------------
    // SELECT * FROM (SELECT * FROM t1 JOIN t2 ON ...) AS sub
    #[test]
    fn recursive_nesting_derived_subquery_containing_join() {
        let inner_join = Join {
            span: Span::new(0, 30),
            join_type: JoinType::Inner,
            table: table_factor("t2"),
            condition: JoinCondition::On(ident_expr("t1.id")),
            lateral: false,
        };
        // inner SELECT: projection references the joined table factor
        let inner_select = SelectStatement {
            span: Span::new(0, 50),
            with: None,
            projection: vec![SelectItem::Wildcard],
            from: Some(TableFactor::Join(Box::new(inner_join))),
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
        };
        // outer SELECT: FROM is a Derived subquery wrapping the inner select
        let outer = SelectStatement {
            span: Span::new(0, 100),
            with: None,
            projection: vec![SelectItem::Wildcard],
            from: Some(TableFactor::Derived {
                subquery: Box::new(inner_select),
                alias: Some(TableAlias::new("sub".to_string(), vec![])),
            }),
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
        };
        // Verify the full recursion depth round-trips through Clone + PartialEq.
        let cloned = outer.clone();
        assert_eq!(outer, cloned);
        if let Some(TableFactor::Derived { subquery, alias }) = &outer.from {
            assert_eq!(alias.as_ref().expect("alias").name(), "sub");
            assert!(matches!(
                subquery.from.as_ref().expect("inner from"),
                TableFactor::Join(_)
            ));
        } else {
            panic!("expected Derived factor");
        }
    }

    // -- Edge case: bare select with no projection ---------------------------

    #[test]
    fn empty_projection_select() {
        let stmt = SelectStatement::simple(vec![]);
        assert!(stmt.projection.is_empty());
        assert!(stmt.from.is_none());
    }
}
