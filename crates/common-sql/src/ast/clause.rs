//! SQL query clause nodes: GROUP BY, ORDER BY, LIMIT, WITH (CTE).

use crate::ast::span::Span;
use crate::ast::statement::SelectStatement;
use crate::ast::Expression;

// ---------------------------------------------------------------------------
// GROUP BY
// ---------------------------------------------------------------------------

/// An item in a `GROUP BY` clause.
#[derive(Debug, Clone, PartialEq)]
pub enum GroupByItem {
    /// A plain expression: `GROUP BY col`.
    Expression(Expression),
    /// `ROLLUP(expr, ...)`.
    Rollup(Vec<Expression>),
    /// `CUBE(expr, ...)`.
    Cube(Vec<Expression>),
    /// `GROUPING SETS((expr, ...), ...)`.
    GroupingSets(Vec<Vec<Expression>>),
}

/// `GROUP BY` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupByClause {
    /// Source span.
    pub span: Span,
    /// Group-by items.
    pub items: Vec<GroupByItem>,
}

// ---------------------------------------------------------------------------
// ORDER BY
// ---------------------------------------------------------------------------

/// Sort direction for `ORDER BY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortDirection {
    /// Ascending (`ASC`).
    Asc,
    /// Descending (`DESC`).
    Desc,
}

/// Null ordering within a sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NullOrdering {
    /// `NULLS FIRST`.
    NullsFirst,
    /// `NULLS LAST`.
    NullsLast,
}

/// An item in an `ORDER BY` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByItem {
    /// The expression to sort by.
    pub expr: Expression,
    /// Sort direction — `None` means implicit `ASC`.
    pub direction: Option<SortDirection>,
    /// Null ordering — `None` means database default.
    pub nulls: Option<NullOrdering>,
}

/// `ORDER BY` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByClause {
    /// Source span.
    pub span: Span,
    /// Sort items.
    pub items: Vec<OrderByItem>,
}

// ---------------------------------------------------------------------------
// LIMIT
// ---------------------------------------------------------------------------

/// `LIMIT` / `OFFSET` clause.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitClause {
    /// Source span.
    pub span: Span,
    /// Maximum number of rows to return.
    pub limit: Expression,
    /// Number of rows to skip.
    pub offset: Option<Expression>,
}

// ---------------------------------------------------------------------------
// WITH (CTE)
// ---------------------------------------------------------------------------

/// A single Common Table Expression (`name AS (query)`).
#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    /// CTE name.
    pub name: String,
    /// Optional column alias list: `name (col1, col2) AS (...)`.
    pub columns: Vec<String>,
    /// The subquery defining the CTE.
    pub query: Box<SelectStatement>,
    /// `MATERIALIZED` / `NOT MATERIALIZED` hint (`None` = unspecified).
    pub materialized: Option<bool>,
}

/// `WITH` clause (one or more CTEs).
#[derive(Debug, Clone, PartialEq)]
pub struct WithClause {
    /// `true` for `WITH RECURSIVE`.
    pub recursive: bool,
    /// CTE definitions.
    pub ctes: Vec<Cte>,
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
    use crate::ast::literal::Literal;
    use crate::ast::SelectItem;

    // Helper to build a trivial expression.
    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn ident_expr(name: &str) -> Expression {
        Expression::Identifier(crate::ast::Identifier::new(name.to_string()))
    }

    fn simple_select() -> SelectStatement {
        SelectStatement::simple(vec![SelectItem::Wildcard])
    }

    // ===== GroupByItem =====

    #[test]
    fn group_by_item_expression() {
        let item = GroupByItem::Expression(ident_expr("department"));
        assert!(matches!(item, GroupByItem::Expression(_)));
    }

    #[test]
    fn group_by_item_rollup() {
        let item = GroupByItem::Rollup(vec![ident_expr("year"), ident_expr("month")]);
        if let GroupByItem::Rollup(exprs) = &item {
            assert_eq!(exprs.len(), 2);
        } else {
            panic!("expected Rollup");
        }
    }

    #[test]
    fn group_by_item_cube() {
        let item = GroupByItem::Cube(vec![ident_expr("region"), ident_expr("product")]);
        if let GroupByItem::Cube(exprs) = &item {
            assert_eq!(exprs.len(), 2);
        } else {
            panic!("expected Cube");
        }
    }

    #[test]
    fn group_by_item_grouping_sets() {
        let item = GroupByItem::GroupingSets(vec![
            vec![ident_expr("a")],
            vec![ident_expr("b"), ident_expr("c")],
        ]);
        if let GroupByItem::GroupingSets(sets) = &item {
            assert_eq!(sets.len(), 2);
            assert_eq!(sets[1].len(), 2);
        } else {
            panic!("expected GroupingSets");
        }
    }

    // ===== GroupByClause =====

    #[test]
    fn group_by_clause_with_items() {
        let clause = GroupByClause {
            span: Span::new(10, 30),
            items: vec![
                GroupByItem::Expression(ident_expr("dept")),
                GroupByItem::Expression(ident_expr("year")),
            ],
        };
        assert_eq!(clause.items.len(), 2);
        assert_eq!(clause.span.start, 10);
    }

    #[test]
    fn group_by_clause_empty_items() {
        let clause = GroupByClause {
            span: Span::default(),
            items: vec![],
        };
        assert!(clause.items.is_empty());
    }

    #[test]
    fn group_by_clause_mixed_items() {
        let clause = GroupByClause {
            span: Span::new(0, 50),
            items: vec![
                GroupByItem::Expression(ident_expr("a")),
                GroupByItem::Rollup(vec![ident_expr("b")]),
                GroupByItem::Cube(vec![ident_expr("c")]),
            ],
        };
        assert_eq!(clause.items.len(), 3);
    }

    // ===== SortDirection =====

    #[test]
    fn sort_direction_copy_and_equality() {
        let asc = SortDirection::Asc;
        let desc = SortDirection::Desc;
        let asc_copy = asc; // Copy
        assert_eq!(asc, asc_copy);
        assert_ne!(asc, desc);
    }

    // ===== NullOrdering =====

    #[test]
    fn null_ordering_copy_and_equality() {
        let first = NullOrdering::NullsFirst;
        let last = NullOrdering::NullsLast;
        let first_copy = first; // Copy
        assert_eq!(first, first_copy);
        assert_ne!(first, last);
    }

    // ===== OrderByItem =====

    #[test]
    fn order_by_item_with_direction() {
        let item = OrderByItem {
            expr: ident_expr("name"),
            direction: Some(SortDirection::Asc),
            nulls: None,
        };
        assert_eq!(item.direction, Some(SortDirection::Asc));
        assert!(item.nulls.is_none());
    }

    #[test]
    fn order_by_item_with_nulls_first() {
        let item = OrderByItem {
            expr: ident_expr("salary"),
            direction: Some(SortDirection::Desc),
            nulls: Some(NullOrdering::NullsFirst),
        };
        assert_eq!(item.direction, Some(SortDirection::Desc));
        assert_eq!(item.nulls, Some(NullOrdering::NullsFirst));
    }

    #[test]
    fn order_by_item_without_direction() {
        // Implicit ASC — direction is None
        let item = OrderByItem {
            expr: int_expr(1),
            direction: None,
            nulls: None,
        };
        assert!(item.direction.is_none());
    }

    // ===== OrderByClause =====

    #[test]
    fn order_by_clause_multiple_items() {
        let clause = OrderByClause {
            span: Span::new(20, 60),
            items: vec![
                OrderByItem {
                    expr: ident_expr("last_name"),
                    direction: Some(SortDirection::Asc),
                    nulls: None,
                },
                OrderByItem {
                    expr: ident_expr("first_name"),
                    direction: Some(SortDirection::Desc),
                    nulls: Some(NullOrdering::NullsLast),
                },
            ],
        };
        assert_eq!(clause.items.len(), 2);
    }

    #[test]
    fn order_by_clause_empty_items() {
        let clause = OrderByClause {
            span: Span::default(),
            items: vec![],
        };
        assert!(clause.items.is_empty());
    }

    // ===== LimitClause =====

    #[test]
    fn limit_clause_without_offset() {
        let clause = LimitClause {
            span: Span::new(40, 50),
            limit: int_expr(10),
            offset: None,
        };
        assert_eq!(clause.span.start, 40);
        assert!(clause.offset.is_none());
    }

    #[test]
    fn limit_clause_with_offset() {
        let clause = LimitClause {
            span: Span::new(40, 60),
            limit: int_expr(20),
            offset: Some(int_expr(5)),
        };
        assert_eq!(clause.offset, Some(int_expr(5)));
    }

    // ===== Cte =====

    #[test]
    fn cte_basic() {
        let cte = Cte {
            name: "sq".to_string(),
            columns: vec![],
            query: Box::new(simple_select()),
            materialized: None,
        };
        assert_eq!(cte.name, "sq");
        assert!(cte.columns.is_empty());
        assert!(cte.materialized.is_none());
    }

    #[test]
    fn cte_with_column_list() {
        let cte = Cte {
            name: "src".to_string(),
            columns: vec!["id".to_string(), "name".to_string()],
            query: Box::new(simple_select()),
            materialized: None,
        };
        assert_eq!(cte.columns.len(), 2);
        assert_eq!(cte.columns[0], "id");
    }

    #[test]
    fn cte_with_materialized() {
        let cte = Cte {
            name: "mat".to_string(),
            columns: vec![],
            query: Box::new(simple_select()),
            materialized: Some(true),
        };
        assert_eq!(cte.materialized, Some(true));
    }

    #[test]
    fn cte_not_materialized() {
        let cte = Cte {
            name: "nomat".to_string(),
            columns: vec![],
            query: Box::new(simple_select()),
            materialized: Some(false),
        };
        assert_eq!(cte.materialized, Some(false));
    }

    // ===== WithClause =====

    #[test]
    fn with_clause_non_recursive() {
        let wc = WithClause {
            recursive: false,
            ctes: vec![Cte {
                name: "t".to_string(),
                columns: vec![],
                query: Box::new(simple_select()),
                materialized: None,
            }],
        };
        assert!(!wc.recursive);
        assert_eq!(wc.ctes.len(), 1);
    }

    #[test]
    fn with_clause_recursive() {
        let wc = WithClause {
            recursive: true,
            ctes: vec![Cte {
                name: "r".to_string(),
                columns: vec!["n".to_string()],
                query: Box::new(simple_select()),
                materialized: None,
            }],
        };
        assert!(wc.recursive);
        assert_eq!(wc.ctes[0].columns.len(), 1);
    }

    #[test]
    fn with_clause_multiple_ctes() {
        let wc = WithClause {
            recursive: false,
            ctes: vec![
                Cte {
                    name: "a".to_string(),
                    columns: vec![],
                    query: Box::new(simple_select()),
                    materialized: None,
                },
                Cte {
                    name: "b".to_string(),
                    columns: vec!["x".to_string()],
                    query: Box::new(simple_select()),
                    materialized: Some(true),
                },
            ],
        };
        assert_eq!(wc.ctes.len(), 2);
    }

    #[test]
    fn with_clause_empty_ctes() {
        let wc = WithClause {
            recursive: false,
            ctes: vec![],
        };
        assert!(wc.ctes.is_empty());
    }

    // ===== Clone / PartialEq =====

    #[test]
    fn group_by_clause_clone_equality() {
        let clause = GroupByClause {
            span: Span::new(0, 10),
            items: vec![GroupByItem::Expression(ident_expr("x"))],
        };
        let cloned = clause.clone();
        assert_eq!(clause, cloned);
    }

    #[test]
    fn order_by_clause_clone_equality() {
        let clause = OrderByClause {
            span: Span::new(5, 15),
            items: vec![OrderByItem {
                expr: ident_expr("y"),
                direction: Some(SortDirection::Desc),
                nulls: Some(NullOrdering::NullsFirst),
            }],
        };
        let cloned = clause.clone();
        assert_eq!(clause, cloned);
    }

    #[test]
    fn limit_clause_clone_equality() {
        let clause = LimitClause {
            span: Span::new(0, 5),
            limit: int_expr(100),
            offset: Some(int_expr(10)),
        };
        let cloned = clause.clone();
        assert_eq!(clause, cloned);
    }

    #[test]
    fn cte_clone_equality() {
        let cte = Cte {
            name: "cte1".to_string(),
            columns: vec!["a".to_string()],
            query: Box::new(simple_select()),
            materialized: Some(false),
        };
        let cloned = cte.clone();
        assert_eq!(cte, cloned);
    }

    #[test]
    fn with_clause_clone_equality() {
        let wc = WithClause {
            recursive: true,
            ctes: vec![Cte {
                name: "src".to_string(),
                columns: vec![],
                query: Box::new(simple_select()),
                materialized: None,
            }],
        };
        let cloned = wc.clone();
        assert_eq!(wc, cloned);
    }
}
