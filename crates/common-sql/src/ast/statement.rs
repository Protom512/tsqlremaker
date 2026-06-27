//! SQL statement nodes.

use crate::ast::clause::{GroupByClause, LimitClause, OrderByClause, WithClause};
use crate::ast::ddl::{
    AlterTableStatement, CreateIndexStatement, CreateTableStatement, DropIndexStatement,
    DropTableStatement,
};
use crate::ast::identifier::{Identifier, QualifiedName};
use crate::ast::join::TableFactor;
use crate::ast::span::Span;
use crate::ast::Expression;

/// Top-level SQL statement.
///
/// Variants are boxed to keep the enum small regardless of which statement
/// type dominates (avoids `clippy::large_enum_variant`).
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// SELECT statement.
    Select(Box<SelectStatement>),
    /// INSERT statement.
    Insert(Box<InsertStatement>),
    /// UPDATE statement.
    Update(Box<UpdateStatement>),
    /// DELETE statement.
    Delete(Box<DeleteStatement>),
    /// `CREATE TABLE` statement.
    CreateTable(Box<CreateTableStatement>),
    /// `ALTER TABLE` statement.
    AlterTable(Box<AlterTableStatement>),
    /// `DROP TABLE` statement.
    DropTable(Box<DropTableStatement>),
    /// `CREATE INDEX` statement.
    CreateIndex(Box<CreateIndexStatement>),
    /// `DROP INDEX` statement.
    DropIndex(Box<DropIndexStatement>),
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
// ON CONFLICT support (Task 4.3 / Task 2)
// ---------------------------------------------------------------------------

/// A column assignment used in `UPDATE SET` and `ON CONFLICT DO UPDATE`.
///
/// Represents `column = value`.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    /// The target column.
    pub column: Identifier,
    /// The value to assign.
    pub value: Expression,
}

/// The action to take when an `ON CONFLICT` clause fires.
///
/// Mirrors PostgreSQL's `ON CONFLICT [DO NOTHING | DO UPDATE SET ...]`.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictAction {
    /// `ON CONFLICT DO NOTHING` — discard the conflicting row.
    DoNothing,
    /// `ON CONFLICT DO UPDATE SET ...` — update the existing row.
    ///
    /// Carries the `SET` assignments (e.g. `name = EXCLUDED.name`).
    DoUpdate(Vec<Assignment>),
}

/// An `ON CONFLICT` clause on an [`InsertStatement`].
///
/// PostgreSQL-specific; carries an optional conflict target (the columns or
/// constraint inference target) and the [`ConflictAction`] to perform.
/// This type unblocks `postgresql-emitter`'s `ON CONFLICT` emission.
#[derive(Debug, Clone, PartialEq)]
pub struct OnConflict {
    /// Source span of the entire `ON CONFLICT ...` clause.
    pub span: Span,
    /// What to do when a conflict is detected.
    pub action: ConflictAction,
    /// Columns (or constraint inference target) the clause applies to.
    ///
    /// `None` means no explicit target (catch-all conflict handler).
    pub conflict_target: Option<Vec<Identifier>>,
}

// ---------------------------------------------------------------------------
// DML statements: INSERT / UPDATE / DELETE (Task 4.3)
// ---------------------------------------------------------------------------

/// INSERT statement.
///
/// Represents `INSERT INTO table [(columns...)] { VALUES (...) | SELECT ... }`
/// with an optional `ON CONFLICT` clause (PostgreSQL).
#[derive(Debug, Clone, PartialEq)]
pub struct InsertStatement {
    /// Source span of the entire statement.
    pub span: Span,
    /// Target table name (`schema.table` or just `table`).
    pub table: QualifiedName,
    /// Explicit column list, if present.
    pub columns: Vec<Identifier>,
    /// The source of rows to insert (`VALUES` or `SELECT`).
    pub source: InsertSource,
    /// Optional `ON CONFLICT` clause (PostgreSQL).
    pub on_conflict: Option<OnConflict>,
}

/// The row source of an [`InsertStatement`].
#[derive(Debug, Clone, PartialEq)]
pub enum InsertSource {
    /// `VALUES (row0), (row1), ...` — each inner `Vec` is one row of values.
    Values(Vec<Vec<Expression>>),
    /// `INSERT INTO ... SELECT ...` — the rows come from a subquery.
    Select(Box<SelectStatement>),
}

/// UPDATE statement.
///
/// Represents `UPDATE table SET assignments [FROM from] [WHERE where_clause]`.
/// The `FROM` clause is supported by T-SQL and PostgreSQL.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStatement {
    /// Source span of the entire statement.
    pub span: Span,
    /// Target table reference.
    pub table: TableFactor,
    /// `SET` assignments (`column = value`).
    pub assignments: Vec<Assignment>,
    /// Optional `FROM` clause (T-SQL / PostgreSQL).
    pub from: Option<TableFactor>,
    /// WHERE clause.
    pub where_clause: Option<Expression>,
}

/// DELETE statement.
///
/// Represents `DELETE FROM table [USING ...] [WHERE where_clause]`.
/// The `USING` clause is a PostgreSQL extension for multi-table deletes.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStatement {
    /// Source span of the entire statement.
    pub span: Span,
    /// Target table reference.
    pub table: TableFactor,
    /// Optional `USING` clause (PostgreSQL multi-table delete).
    pub using: Option<Vec<TableFactor>>,
    /// WHERE clause.
    pub where_clause: Option<Expression>,
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
    use crate::ast::expression::{BinaryOperator, ComparisonOperator, InList};
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
        assert!(inner.from.is_none());
        assert!(inner.with.is_none());
        let stmt = Statement::Select(Box::new(inner));
        // Real discriminant checks against all four Statement variants.
        assert!(matches!(stmt, Statement::Select(_)));
        assert!(!matches!(stmt, Statement::Insert(_)));
        assert!(!matches!(stmt, Statement::Update(_)));
        assert!(!matches!(stmt, Statement::Delete(_)));
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

    // ===== Task 4.3 / OnConflict (Task 2): minimal ON CONFLICT type =====

    #[test]
    fn on_conflict_do_nothing_without_target() {
        // INSERT INTO t (id) VALUES (1) ON CONFLICT DO NOTHING
        let oc = OnConflict {
            span: Span::new(0, 10),
            action: ConflictAction::DoNothing,
            conflict_target: None,
        };
        assert!(matches!(oc.action, ConflictAction::DoNothing));
        assert!(oc.conflict_target.is_none());
    }

    #[test]
    fn on_conflict_do_nothing_with_target() {
        // ON CONFLICT (id) DO NOTHING
        let oc = OnConflict {
            span: Span::new(0, 10),
            action: ConflictAction::DoNothing,
            conflict_target: Some(vec![Identifier::new("id".to_string())]),
        };
        let target = oc.conflict_target.as_ref().expect("target");
        assert_eq!(target.len(), 1);
        assert_eq!(target[0].value(), "id");
    }

    #[test]
    fn on_conflict_do_update_with_assignments() {
        // ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name
        let oc = OnConflict {
            span: Span::new(0, 40),
            action: ConflictAction::DoUpdate(vec![Assignment {
                column: Identifier::new("name".to_string()),
                value: ident_expr("excluded_name"),
            }]),
            conflict_target: Some(vec![Identifier::new("id".to_string())]),
        };
        if let ConflictAction::DoUpdate(assigns) = &oc.action {
            assert_eq!(assigns.len(), 1);
            assert_eq!(assigns[0].column.value(), "name");
        } else {
            panic!("expected DoUpdate");
        }
    }

    #[test]
    fn on_conflict_do_update_multiple_assignments() {
        let oc = OnConflict {
            span: Span::new(0, 60),
            action: ConflictAction::DoUpdate(vec![
                Assignment {
                    column: Identifier::new("name".to_string()),
                    value: ident_expr("x"),
                },
                Assignment {
                    column: Identifier::new("count".to_string()),
                    value: int_expr(1),
                },
            ]),
            conflict_target: None,
        };
        if let ConflictAction::DoUpdate(assigns) = &oc.action {
            assert_eq!(assigns.len(), 2);
        } else {
            panic!("expected DoUpdate");
        }
    }

    #[test]
    fn on_conflict_clone_and_equality() {
        let oc = OnConflict {
            span: Span::new(0, 10),
            action: ConflictAction::DoNothing,
            conflict_target: Some(vec![Identifier::new("id".to_string())]),
        };
        let cloned = oc.clone();
        assert_eq!(oc, cloned);
    }

    #[test]
    fn on_conflict_inequality_on_action() {
        let a = OnConflict {
            span: Span::new(0, 10),
            action: ConflictAction::DoNothing,
            conflict_target: None,
        };
        let b = OnConflict {
            span: Span::new(0, 10),
            action: ConflictAction::DoUpdate(vec![]),
            conflict_target: None,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn assignment_basic() {
        let a = Assignment {
            column: Identifier::new("status".to_string()),
            value: int_expr(1),
        };
        assert_eq!(a.column.value(), "status");
        assert!(matches!(a.value, Expression::Literal(Literal::Integer(1))));
    }

    #[test]
    fn assignment_clone_equality() {
        let a = Assignment {
            column: Identifier::new("name".to_string()),
            value: ident_expr("other"),
        };
        let cloned = a.clone();
        assert_eq!(a, cloned);
    }

    // ===== Task 4.3: INSERT statement =====

    fn qualified(name: &str) -> QualifiedName {
        QualifiedName::new(None, name.to_string())
    }

    #[test]
    fn insert_values_basic() {
        // INSERT INTO users (id, name) VALUES (1, 'a'), (2, 'b')
        let stmt = InsertStatement {
            span: Span::new(0, 50),
            table: qualified("users"),
            columns: vec![
                Identifier::new("id".to_string()),
                Identifier::new("name".to_string()),
            ],
            source: InsertSource::Values(vec![
                vec![int_expr(1), ident_expr("a")],
                vec![int_expr(2), ident_expr("b")],
            ]),
            on_conflict: None,
        };
        assert_eq!(stmt.columns.len(), 2);
        if let InsertSource::Values(rows) = &stmt.source {
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].len(), 2);
        } else {
            panic!("expected Values source");
        }
        assert!(stmt.on_conflict.is_none());
    }

    #[test]
    fn insert_with_on_conflict_do_nothing() {
        let stmt = InsertStatement {
            span: Span::new(0, 40),
            table: qualified("users"),
            columns: vec![Identifier::new("id".to_string())],
            source: InsertSource::Values(vec![vec![int_expr(1)]]),
            on_conflict: Some(OnConflict {
                span: Span::new(20, 40),
                action: ConflictAction::DoNothing,
                conflict_target: None,
            }),
        };
        let oc = stmt.on_conflict.as_ref().expect("on_conflict");
        assert!(matches!(oc.action, ConflictAction::DoNothing));
    }

    #[test]
    fn insert_values_empty_rows_edge_case() {
        // Edge case: VALUES with no rows (degenerate but representable).
        let stmt = InsertStatement {
            span: Span::new(0, 10),
            table: qualified("t"),
            columns: vec![],
            source: InsertSource::Values(vec![]),
            on_conflict: None,
        };
        if let InsertSource::Values(rows) = &stmt.source {
            assert!(rows.is_empty());
        } else {
            panic!("expected Values source");
        }
    }

    #[test]
    fn insert_clone_equality() {
        let stmt = InsertStatement {
            span: Span::new(0, 10),
            table: qualified("t"),
            columns: vec![Identifier::new("id".to_string())],
            source: InsertSource::Values(vec![vec![int_expr(1)]]),
            on_conflict: None,
        };
        let cloned = stmt.clone();
        assert_eq!(stmt, cloned);
    }

    // ===== Task 4.3: INSERT ... SELECT (nested subquery) =====

    #[test]
    fn insert_select_from_plain_table() {
        // INSERT INTO archive (id) SELECT id FROM source
        let sel = SelectStatement {
            span: Span::new(0, 30),
            with: None,
            projection: vec![SelectItem::Expression {
                expr: ident_expr("id"),
                alias: None,
            }],
            from: Some(table_factor("source")),
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
        };
        let stmt = InsertStatement {
            span: Span::new(0, 50),
            table: qualified("archive"),
            columns: vec![Identifier::new("id".to_string())],
            source: InsertSource::Select(Box::new(sel)),
            on_conflict: None,
        };
        // Clone + PartialEq survive the Select(SelectStatement) recursion.
        let cloned = stmt.clone();
        assert_eq!(stmt, cloned);
        assert!(matches!(stmt.source, InsertSource::Select(_)));
    }

    #[test]
    fn insert_select_from_derived_subquery_recursion() {
        // INSERT INTO t (x) SELECT x FROM (SELECT * FROM base JOIN other ON ...) AS sub
        // Proves Clone/PartialEq survive the TableFactor::Derived -> Join recursion
        // inside the InsertSource::Select branch of an INSERT node.
        let inner_join = Join {
            span: Span::new(0, 30),
            join_type: JoinType::Inner,
            table: table_factor("other"),
            condition: JoinCondition::On(ident_expr("base.id")),
            lateral: false,
        };
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
        let middle_select = SelectStatement {
            span: Span::new(0, 80),
            with: None,
            projection: vec![SelectItem::Expression {
                expr: ident_expr("x"),
                alias: None,
            }],
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
        let insert = InsertStatement {
            span: Span::new(0, 100),
            table: qualified("t"),
            columns: vec![Identifier::new("x".to_string())],
            source: InsertSource::Select(Box::new(middle_select)),
            on_conflict: None,
        };
        let cloned = insert.clone();
        assert_eq!(insert, cloned);
        if let InsertSource::Select(sel) = &insert.source {
            if let Some(TableFactor::Derived { alias, .. }) = &sel.from {
                assert_eq!(alias.as_ref().expect("alias").name(), "sub");
            } else {
                panic!("expected Derived factor in INSERT...SELECT source");
            }
        } else {
            panic!("expected Select source");
        }
    }

    // ===== Task 4.3: UPDATE statement =====

    #[test]
    fn update_basic_assignments() {
        // UPDATE users SET name = 'x', count = count + 1 WHERE id = 5
        let stmt = UpdateStatement {
            span: Span::new(0, 60),
            table: table_factor("users"),
            assignments: vec![
                Assignment {
                    column: Identifier::new("name".to_string()),
                    value: ident_expr("x"),
                },
                Assignment {
                    column: Identifier::new("count".to_string()),
                    value: Expression::BinaryOp {
                        left: Box::new(ident_expr("count")),
                        op: BinaryOperator::Add,
                        right: Box::new(int_expr(1)),
                    },
                },
            ],
            from: None,
            where_clause: Some(Expression::Comparison {
                left: Box::new(ident_expr("id")),
                op: ComparisonOperator::Eq,
                right: Box::new(int_expr(5)),
            }),
        };
        assert_eq!(stmt.assignments.len(), 2);
        assert!(stmt.from.is_none());
        assert!(stmt.where_clause.is_some());
    }

    #[test]
    fn update_with_from_clause() {
        // UPDATE t SET ... FROM other WHERE t.id = other.id  (T-SQL / PostgreSQL)
        let stmt = UpdateStatement {
            span: Span::new(0, 40),
            table: table_factor("t"),
            assignments: vec![Assignment {
                column: Identifier::new("val".to_string()),
                value: ident_expr("other.val"),
            }],
            from: Some(table_factor("other")),
            where_clause: None,
        };
        assert!(stmt.from.is_some());
        assert!(stmt.where_clause.is_none());
    }

    #[test]
    fn update_with_subquery_in_where() {
        // UPDATE t SET val = 1 WHERE id IN (SELECT id FROM src)
        // Proves Clone/PartialEq survive an Expression subquery inside the
        // new UpdateStatement node.
        let sub = SelectStatement {
            span: Span::new(0, 30),
            with: None,
            projection: vec![SelectItem::Expression {
                expr: ident_expr("id"),
                alias: None,
            }],
            from: Some(table_factor("src")),
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
        };
        let where_clause = Expression::In {
            expr: Box::new(ident_expr("id")),
            list: InList::Subquery(Box::new(sub)),
            negated: false,
        };
        let stmt = UpdateStatement {
            span: Span::new(0, 60),
            table: table_factor("t"),
            assignments: vec![Assignment {
                column: Identifier::new("val".to_string()),
                value: int_expr(1),
            }],
            from: None,
            where_clause: Some(where_clause),
        };
        let cloned = stmt.clone();
        assert_eq!(stmt, cloned);
        assert!(stmt.where_clause.is_some());
    }

    #[test]
    fn update_no_assignments_edge_case() {
        // Edge case: UPDATE with no assignments (degenerate but representable).
        let stmt = UpdateStatement {
            span: Span::new(0, 10),
            table: table_factor("t"),
            assignments: vec![],
            from: None,
            where_clause: None,
        };
        assert!(stmt.assignments.is_empty());
    }

    // ===== Task 4.3: DELETE statement =====

    #[test]
    fn delete_basic_with_where() {
        // DELETE FROM users WHERE id = 5
        let stmt = DeleteStatement {
            span: Span::new(0, 30),
            table: table_factor("users"),
            using: None,
            where_clause: Some(Expression::Comparison {
                left: Box::new(ident_expr("id")),
                op: ComparisonOperator::Eq,
                right: Box::new(int_expr(5)),
            }),
        };
        assert!(stmt.using.is_none());
        assert!(stmt.where_clause.is_some());
    }

    #[test]
    fn delete_with_using_clause() {
        // DELETE FROM t USING other WHERE t.id = other.id  (PostgreSQL USING)
        let stmt = DeleteStatement {
            span: Span::new(0, 40),
            table: table_factor("t"),
            using: Some(vec![table_factor("other")]),
            where_clause: None,
        };
        let using = stmt.using.as_ref().expect("using");
        assert_eq!(using.len(), 1);
    }

    #[test]
    fn delete_multiple_using_tables() {
        // DELETE FROM t USING a, b WHERE ...
        let stmt = DeleteStatement {
            span: Span::new(0, 50),
            table: table_factor("t"),
            using: Some(vec![table_factor("a"), table_factor("b")]),
            where_clause: None,
        };
        let using = stmt.using.as_ref().expect("using");
        assert_eq!(using.len(), 2);
    }

    #[test]
    fn delete_without_where_deletes_all() {
        // Edge case: DELETE FROM t  (no WHERE, deletes all rows)
        let stmt = DeleteStatement {
            span: Span::new(0, 10),
            table: table_factor("t"),
            using: None,
            where_clause: None,
        };
        assert!(stmt.where_clause.is_none());
    }

    #[test]
    fn delete_clone_equality() {
        let stmt = DeleteStatement {
            span: Span::new(0, 20),
            table: table_factor("t"),
            using: Some(vec![table_factor("a")]),
            where_clause: Some(ident_expr("flag")),
        };
        let cloned = stmt.clone();
        assert_eq!(stmt, cloned);
    }

    // ===== Task 4.3: Statement enum discriminants =====

    #[test]
    fn statement_insert_wraps_insert_statement() {
        let inner = InsertStatement {
            span: Span::new(0, 10),
            table: qualified("t"),
            columns: vec![],
            source: InsertSource::Values(vec![]),
            on_conflict: None,
        };
        let stmt = Statement::Insert(Box::new(inner));
        assert!(matches!(stmt, Statement::Insert(_)));
        assert!(!matches!(stmt, Statement::Select(_)));
        assert!(!matches!(stmt, Statement::Update(_)));
        assert!(!matches!(stmt, Statement::Delete(_)));
        assert_eq!(stmt.clone(), stmt);
    }

    #[test]
    fn statement_update_wraps_update_statement() {
        let inner = UpdateStatement {
            span: Span::new(0, 10),
            table: table_factor("t"),
            assignments: vec![],
            from: None,
            where_clause: None,
        };
        let stmt = Statement::Update(Box::new(inner));
        assert!(matches!(stmt, Statement::Update(_)));
        assert!(!matches!(stmt, Statement::Select(_)));
        assert!(!matches!(stmt, Statement::Insert(_)));
        assert!(!matches!(stmt, Statement::Delete(_)));
        assert_eq!(stmt.clone(), stmt);
    }

    #[test]
    fn statement_delete_wraps_delete_statement() {
        let inner = DeleteStatement {
            span: Span::new(0, 10),
            table: table_factor("t"),
            using: None,
            where_clause: None,
        };
        let stmt = Statement::Delete(Box::new(inner));
        assert!(matches!(stmt, Statement::Delete(_)));
        assert!(!matches!(stmt, Statement::Select(_)));
        assert!(!matches!(stmt, Statement::Insert(_)));
        assert!(!matches!(stmt, Statement::Update(_)));
        assert_eq!(stmt.clone(), stmt);
    }
}
