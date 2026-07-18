//! TDD tests (RED phase) for the direct T-SQL → common-sql converter.
//!
//! These tests assert the *composite* behavior of
//! [`to_common_sql`](crate::ast::to_common_sql), which merges the former
//! Stage-1 (`to_common.rs` → legacy `Common*`) and Stage-2
//! (`convert_common_sql.rs` `From` impls → `common_sql::ast`) into one pass.
//!
//! ## Pinned parity contract (must match the legacy two-stage pipeline)
//!
//! The direct converter must produce the **exact same** `common_sql::ast`
//! output that `convert(stmt.to_common_ast()?)` produced before. The behavior
//! is therefore the union of:
//!
//! * Stage-1 drops: `Hex` literals, `Tilde` unary op, `NotLt`/`NotGt`,
//!   `IS TRUE`/`IS FALSE`, `IN`/`BETWEEN` as bare binary operators, and any
//!   sub-expression that fails to convert → the whole `?` short-circuits to
//!   `None`.
//! * Stage-2 lossy mappings: `DialectSpecific → None`,
//!   `Vec<TableRef> → Option<TableFactor>` first-element, `DefaultValues →
//!   Values(vec![])`, `LIKE` → `Comparison::{Like,NotLike}` (ESCAPE dropped),
//!   CASE `operand = None`, spans filled with `Span::new(0,0)`.
//! * `BatchSeparator` → `None` (Stage-1 returned `None`).
//!
//! The integration parity is cross-checked against the legacy two-stage path
//! (`stmt.to_common_ast().and_then(|c| convert(c))`) in
//! `tests/convert_common_sql_bridge.rs`; this unit file focuses on direct
//! structural assertions against `common_sql::ast` shapes.

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use crate::ast::{
    self, BinaryOperator, ColumnReference, Expression, FromClause, Identifier, InsertSource,
    InsertStatement, LimitClause, Literal, OrderByItem, SelectItem, SelectStatement, Statement,
    TableReference, UpdateStatement,
};
use common_sql::ast::{
    Assignment as SqlAssignment, BinaryOperator as SqlBinaryOp,
    ComparisonOperator as SqlComparison, Expression as SqlExpr, GroupByClause, GroupByItem,
    InsertSource as SqlInsertSource, LimitClause as SqlLimit, Literal as SqlLit,
    LogicalOperator as SqlLogical, OrderByClause, QualifiedName, SelectItem as SqlSelectItem,
    SortDirection, Statement as SqlStmt, TableAlias, TableFactor as SqlTableFactor,
};
// UPDATE SET 用カラム代入は data_modification::Assignment (control_flow 版ではない)
use crate::ast::data_modification::Assignment as ColumnAssignment;
use crate::ast::to_common_sql;

// ---------------------------------------------------------------------------
// helpers — build minimal T-SQL AST nodes
// ---------------------------------------------------------------------------

fn span() -> tsql_token::Span {
    tsql_token::Span::new(10, 20)
}

fn ident_name(name: &str) -> Identifier {
    Identifier {
        name: name.to_string(),
        span: span(),
    }
}

fn expr_ident(name: &str) -> Expression {
    Expression::Identifier(ident_name(name))
}

fn expr_int(n: i64) -> Expression {
    Expression::Literal(Literal::Number(n.to_string(), span()))
}

fn expr_str(s: &str) -> Expression {
    Expression::Literal(Literal::String(s.to_string(), span()))
}

fn table_ref(name: &str) -> TableReference {
    TableReference::Table {
        name: ident_name(name),
        alias: None,
        span: span(),
    }
}

fn from_one(name: &str) -> FromClause {
    FromClause {
        tables: vec![table_ref(name)],
        joins: vec![],
    }
}

fn select_star(table: &str) -> SelectStatement {
    SelectStatement {
        span: span(),
        distinct: false,
        top: None,
        columns: vec![SelectItem::Wildcard],
        from: Some(from_one(table)),
        where_clause: None,
        group_by: vec![],
        having: None,
        order_by: vec![],
        limit: None,
    }
}

// ===================================================================
// 1. Entry-point dispatch: the four convertible variants + None cases
// ===================================================================

#[test]
fn select_statement_maps_to_statement_select() {
    let stmt = Statement::Select(Box::new(select_star("users")));
    let got = to_common_sql(&stmt).expect("SELECT must convert");
    assert!(matches!(got, SqlStmt::Select(_)));
}

#[test]
fn insert_statement_maps_to_statement_insert() {
    let ins = InsertStatement {
        span: span(),
        table: ident_name("users"),
        columns: vec![ident_name("id")],
        source: InsertSource::Values(vec![vec![expr_int(1)]]),
    };
    let stmt = Statement::Insert(Box::new(ins));
    let got = to_common_sql(&stmt).expect("INSERT must convert");
    assert!(matches!(got, SqlStmt::Insert(_)));
}

#[test]
fn update_statement_maps_to_statement_update() {
    let upd = UpdateStatement {
        span: span(),
        table: table_ref("users"),
        assignments: vec![],
        from_clause: None,
        where_clause: None,
    };
    let stmt = Statement::Update(Box::new(upd));
    let got = to_common_sql(&stmt).expect("UPDATE must convert");
    assert!(matches!(got, SqlStmt::Update(_)));
}

#[test]
fn delete_statement_maps_to_statement_delete() {
    let del = ast::DeleteStatement {
        span: span(),
        table: ident_name("users"),
        from_clause: None,
        where_clause: None,
    };
    let stmt = Statement::Delete(Box::new(del));
    let got = to_common_sql(&stmt).expect("DELETE must convert");
    assert!(matches!(got, SqlStmt::Delete(_)));
}

// --- DialectSpecific escape-hatch: T-SQL control-flow variants carry their
//     Debug-string classification + span into the common AST so a downstream
//     emitter can re-implement them natively (#158, T3). DDL (Create /
//     AlterTable) and BatchSeparator still map to None (dedicated variants /
//     out of scope). -----------------------------------------------------

/// Assert `to_common_sql` yields a `DialectSpecific` carrying a non-empty
/// `source` classification for the given SQL. The span is the AST node's own
/// span verbatim (the parser leaves `span.end = 0` for some multi-line
/// control-flow constructs — a known limitation — so only `source` is checked
/// here; the span-fidelity test below pins the round-trip separately).
fn assert_dialect_specific(sql: &str, label: &str) {
    let stmt = crate::parse_one(sql).unwrap_or_else(|e| panic!("parse {label}: {e}"));
    match to_common_sql(&stmt) {
        Some(SqlStmt::DialectSpecific { source, span: _ }) => {
            assert!(
                !source.is_empty(),
                "{label} source classification must be non-empty"
            );
        }
        other => panic!("{label} -> expected DialectSpecific, got {other:?}"),
    }
}

#[test]
fn declare_maps_to_dialect_specific() {
    assert_dialect_specific("DECLARE @x INT", "DECLARE");
}

#[test]
fn set_maps_to_dialect_specific() {
    assert_dialect_specific("SET @x = 1", "SET");
}

#[test]
fn if_maps_to_dialect_specific() {
    assert_dialect_specific("IF 1 = 1 SELECT 1", "IF");
}

#[test]
fn while_maps_to_dialect_specific() {
    assert_dialect_specific("WHILE 1 = 1 SELECT 1", "WHILE");
}

#[test]
fn begin_end_block_maps_to_dialect_specific() {
    assert_dialect_specific("BEGIN SELECT 1 END", "Block");
}

#[test]
fn exec_maps_to_dialect_specific() {
    assert_dialect_specific("EXEC p", "EXEC");
}

#[test]
fn variable_assignment_maps_to_dialect_specific() {
    // SELECT @v = 1 is a variable assignment, not a real SELECT.
    assert_dialect_specific("SELECT @v = 1", "VariableAssignment");
}

#[test]
fn dialect_specific_source_classifies_variant_kind() {
    // The `source` field carries the T-SQL variant Debug head so a downstream
    // emitter can dispatch on construct kind (the deleted postgresql-emitter
    // matched on `Declare(`/`If(` etc.). Pin the classification prefix.
    let stmt = crate::parse_one("DECLARE @x INT").expect("parse DECLARE");
    let SqlStmt::DialectSpecific { source, .. } = to_common_sql(&stmt).expect("DialectSpecific")
    else {
        panic!("expected DialectSpecific");
    };
    assert!(
        source.contains("Declare"),
        "DECLARE source must classify as Declare, got: {source}"
    );
}

#[test]
fn dialect_specific_span_matches_ast_node_span() {
    // The carried span must equal the T-SQL AST node's own span (not a
    // placeholder), so emitters can locate the original construct.
    use crate::ast::AstNode as _;
    let stmt = crate::parse_one("IF 1 = 1 SELECT 1").expect("parse IF");
    let expected_span = stmt.span();
    let SqlStmt::DialectSpecific { span, .. } = to_common_sql(&stmt).expect("DialectSpecific")
    else {
        panic!("expected DialectSpecific");
    };
    assert_eq!(span.start, expected_span.start);
    assert_eq!(span.end, expected_span.end);
}

// --- DDL / BatchSeparator: T2 wires CREATE TABLE / CREATE INDEX / ALTER TABLE
//     into dedicated common-sql variants (Some). CREATE PROCEDURE / VIEW /
//     TRIGGER stay None (no destination). BatchSeparator stays None.

#[test]
fn create_table_parses_and_converts_to_create_table() {
    let stmt = crate::parse_one("CREATE TABLE t (id INT)").expect("parse CREATE TABLE");
    assert!(
        matches!(to_common_sql(&stmt), Some(SqlStmt::CreateTable(_))),
        "CREATE TABLE -> Some(CreateTable)"
    );
}

#[test]
fn create_procedure_returns_none() {
    let sql = "CREATE PROCEDURE p AS SELECT 1";
    let stmt = crate::parse_one(sql).expect("parse CREATE PROC");
    assert!(to_common_sql(&stmt).is_none(), "CREATE PROC -> None");
}

#[test]
fn alter_table_parses_and_converts_to_alter_table() {
    let stmt = crate::parse_one("ALTER TABLE t ADD c INT").expect("parse ALTER TABLE");
    assert!(
        matches!(to_common_sql(&stmt), Some(SqlStmt::AlterTable(_))),
        "ALTER TABLE ADD COLUMN -> Some(AlterTable)"
    );
}

// ===================================================================
// 2. SELECT structural mapping
// ===================================================================

#[test]
fn select_wildcard_projection_and_from_table() {
    let stmt = Statement::Select(Box::new(select_star("users")));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert_eq!(sel.projection.len(), 1);
    assert!(matches!(sel.projection[0], SqlSelectItem::Wildcard));
    let Some(SqlTableFactor::Table { name, alias }) = &sel.from else {
        panic!("expected Table factor");
    };
    assert_eq!(name.name(), "users");
    assert!(name.schema().is_none());
    assert!(alias.is_none());
}

#[test]
fn select_qualified_wildcard_maps_to_qualified_wildcard_identifier() {
    let mut s = select_star("users");
    s.columns = vec![SelectItem::QualifiedWildcard(ident_name("u"))];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match &sel.projection[0] {
        SqlSelectItem::QualifiedWildcard { table } => {
            assert_eq!(table.value(), "u");
        }
        other => panic!("expected QualifiedWildcard, got {other:?}"),
    }
}

#[test]
fn select_expression_item_with_alias_maps() {
    let mut s = select_star("users");
    s.columns = vec![SelectItem::Expression(
        expr_ident("id"),
        Some(ident_name("uid")),
    )];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match &sel.projection[0] {
        SqlSelectItem::Expression { expr, alias } => {
            assert!(matches!(expr, SqlExpr::Identifier(_)));
            assert_eq!(alias.as_ref().expect("alias").value(), "uid");
        }
        other => panic!("expected Expression item, got {other:?}"),
    }
}

#[test]
fn select_distinct_flag_passes_through() {
    let mut s = select_star("users");
    s.distinct = true;
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    // distinct is NOT carried on the destination SelectStatement (it has no
    // distinct field) — but conversion must still succeed. Sanity: shape ok.
    assert_eq!(sel.projection.len(), 1);
}

#[test]
fn select_from_with_alias_maps_alias_to_table_alias() {
    let mut s = select_star("users");
    s.from = Some(FromClause {
        tables: vec![TableReference::Table {
            name: ident_name("users"),
            alias: Some(ident_name("u")),
            span: span(),
        }],
        joins: vec![],
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    let Some(SqlTableFactor::Table { alias, .. }) = &sel.from else {
        panic!("expected Table factor");
    };
    assert_eq!(alias.as_ref().expect("alias").name(), "u");
}

#[test]
fn select_where_clause_maps() {
    let mut s = select_star("users");
    // WHERE id = 5
    s.where_clause = Some(Expression::BinaryOp {
        left: Box::new(expr_ident("id")),
        op: BinaryOperator::Eq,
        right: Box::new(expr_int(5)),
        span: span(),
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert!(matches!(sel.where_clause, Some(SqlExpr::Comparison { .. })));
}

#[test]
fn select_group_by_maps_into_groupbyclause_expression_items() {
    let mut s = select_star("users");
    s.group_by = vec![expr_ident("dept"), expr_ident("year")];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    let Some(GroupByClause { items, .. }) = sel.group_by else {
        panic!("expected GROUP BY clause");
    };
    assert_eq!(items.len(), 2);
    assert!(matches!(items[0], GroupByItem::Expression(_)));
}

#[test]
fn select_group_by_empty_yields_none() {
    let stmt = Statement::Select(Box::new(select_star("users")));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert!(sel.group_by.is_none());
}

#[test]
fn select_order_by_maps_with_explicit_direction() {
    let mut s = select_star("users");
    s.order_by = vec![OrderByItem {
        expr: expr_ident("name"),
        asc: false, // DESC
    }];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    let Some(OrderByClause { items, .. }) = sel.order_by else {
        panic!("expected ORDER BY clause");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].direction, Some(SortDirection::Desc));
    assert!(items[0].nulls.is_none());
}

#[test]
fn select_order_by_asc_direction() {
    let mut s = select_star("users");
    s.order_by = vec![OrderByItem {
        expr: expr_ident("name"),
        asc: true,
    }];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    let Some(OrderByClause { items, .. }) = sel.order_by else {
        panic!("expected ORDER BY clause");
    };
    assert_eq!(items[0].direction, Some(SortDirection::Asc));
}

#[test]
fn select_limit_offset_maps() {
    let mut s = select_star("users");
    s.limit = Some(LimitClause {
        limit: expr_int(10),
        offset: Some(expr_int(2)),
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    let Some(SqlLimit { limit, offset, .. }) = sel.limit else {
        panic!("expected LIMIT clause");
    };
    assert!(matches!(limit, SqlExpr::Literal(SqlLit::Integer(10))));
    assert!(offset.is_some());
}

#[test]
fn select_with_cte_clause_is_always_none() {
    // destination `with` has no legacy source — always None.
    let stmt = Statement::Select(Box::new(select_star("users")));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert!(sel.with.is_none());
}

#[test]
fn select_from_multi_table_takes_first_drops_rest() {
    // Lossy: Vec<TableRef> -> first element only.
    let mut s = select_star("users");
    s.from = Some(FromClause {
        tables: vec![table_ref("a"), table_ref("b")],
        joins: vec![],
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    let Some(SqlTableFactor::Table { name, .. }) = &sel.from else {
        panic!("expected Table factor");
    };
    assert_eq!(name.name(), "a"); // first only; b silently dropped
}

#[test]
fn select_from_subquery_maps_to_derived_factor() {
    let mut s = select_star("users");
    s.from = Some(FromClause {
        tables: vec![TableReference::Subquery {
            query: Box::new(select_star("inner")),
            alias: Some(ident_name("sub")),
            span: span(),
        }],
        joins: vec![],
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    let Some(SqlTableFactor::Derived { alias, .. }) = &sel.from else {
        panic!("expected Derived factor");
    };
    assert_eq!(alias.as_ref().expect("alias").name(), "sub");
}

// ===================================================================
// 3. Expression mapping (operator-split + advanced nodes)
// ===================================================================

#[test]
fn arithmetic_binary_op_dispatches_to_binaryop() {
    for (op, sql_op) in [
        (BinaryOperator::Plus, SqlBinaryOp::Add),
        (BinaryOperator::Minus, SqlBinaryOp::Sub),
        (BinaryOperator::Multiply, SqlBinaryOp::Mul),
        (BinaryOperator::Divide, SqlBinaryOp::Div),
        (BinaryOperator::Modulo, SqlBinaryOp::Mod),
        (BinaryOperator::Concat, SqlBinaryOp::Concat),
    ] {
        let mut s = select_star("t");
        s.where_clause = Some(Expression::BinaryOp {
            left: Box::new(expr_ident("a")),
            op,
            right: Box::new(expr_ident("b")),
            span: span(),
        });
        let stmt = Statement::Select(Box::new(s));
        let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
            panic!("expected Select for {op:?}");
        };
        match &sel.where_clause {
            Some(SqlExpr::BinaryOp { op: got, .. }) => assert_eq!(*got, sql_op),
            other => panic!("expected BinaryOp for {op:?}, got {other:?}"),
        }
    }
}

#[test]
fn comparison_binary_op_dispatches_to_comparison() {
    for (op, sql_op) in [
        (BinaryOperator::Eq, SqlComparison::Eq),
        (BinaryOperator::Ne, SqlComparison::Ne),
        (BinaryOperator::NeAlt, SqlComparison::Eq), // <> -> Eq (legacy parity!)
        (BinaryOperator::Lt, SqlComparison::Lt),
        (BinaryOperator::Le, SqlComparison::Le),
        (BinaryOperator::Gt, SqlComparison::Gt),
        (BinaryOperator::Ge, SqlComparison::Ge),
    ] {
        let mut s = select_star("t");
        s.where_clause = Some(Expression::BinaryOp {
            left: Box::new(expr_ident("a")),
            op,
            right: Box::new(expr_ident("b")),
            span: span(),
        });
        let stmt = Statement::Select(Box::new(s));
        let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
            panic!("expected Select for {op:?}");
        };
        match &sel.where_clause {
            Some(SqlExpr::Comparison { op: got, .. }) => assert_eq!(*got, sql_op),
            other => panic!("expected Comparison for {op:?}, got {other:?}"),
        }
    }
}

#[test]
fn logical_binary_op_dispatches_to_logicalop() {
    for (op, sql_op) in [
        (BinaryOperator::And, SqlLogical::And),
        (BinaryOperator::Or, SqlLogical::Or),
    ] {
        let mut s = select_star("t");
        s.where_clause = Some(Expression::BinaryOp {
            left: Box::new(expr_ident("a")),
            op,
            right: Box::new(expr_ident("b")),
            span: span(),
        });
        let stmt = Statement::Select(Box::new(s));
        let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
            panic!("expected Select for {op:?}");
        };
        match &sel.where_clause {
            Some(SqlExpr::LogicalOp { op: got, .. }) => assert_eq!(*got, sql_op),
            other => panic!("expected LogicalOp for {op:?}, got {other:?}"),
        }
    }
}

#[test]
fn ase_specific_operators_notlt_notgt_short_circuit_to_none() {
    // Stage-1 returns None for these -> whole statement becomes None.
    for op in [BinaryOperator::NotLt, BinaryOperator::NotGt] {
        let mut s = select_star("t");
        s.where_clause = Some(Expression::BinaryOp {
            left: Box::new(expr_ident("a")),
            op,
            right: Box::new(expr_ident("b")),
            span: span(),
        });
        let stmt = Statement::Select(Box::new(s));
        assert!(
            to_common_sql(&stmt).is_none(),
            "ASE-specific op {op:?} must yield None"
        );
    }
}

#[test]
fn in_between_as_binary_operator_short_circuits_to_none() {
    // Stage-1 maps BinaryOperator::In / Between to None.
    for op in [BinaryOperator::In, BinaryOperator::Between] {
        let mut s = select_star("t");
        s.where_clause = Some(Expression::BinaryOp {
            left: Box::new(expr_ident("a")),
            op,
            right: Box::new(expr_ident("b")),
            span: span(),
        });
        let stmt = Statement::Select(Box::new(s));
        assert!(to_common_sql(&stmt).is_none(), "op {op:?} must yield None");
    }
}

#[test]
fn column_reference_with_table_maps_to_qualified_identifier() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::ColumnReference(ColumnReference {
        table: Some(ident_name("u")),
        column: ident_name("id"),
        span: span(),
    }));
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert!(matches!(
        sel.where_clause,
        Some(SqlExpr::QualifiedIdentifier { .. })
    ));
}

#[test]
fn column_reference_without_table_maps_to_identifier() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::ColumnReference(ColumnReference {
        table: None,
        column: ident_name("id"),
        span: span(),
    }));
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert!(matches!(sel.where_clause, Some(SqlExpr::Identifier(_))));
}

#[test]
fn unary_plus_minus_not_map() {
    for op in [
        crate::ast::UnaryOperator::Plus,
        crate::ast::UnaryOperator::Minus,
        crate::ast::UnaryOperator::Not,
    ] {
        let mut s = select_star("t");
        s.where_clause = Some(Expression::UnaryOp {
            op,
            expr: Box::new(expr_ident("a")),
            span: span(),
        });
        let stmt = Statement::Select(Box::new(s));
        let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
            panic!("expected Select for {op:?}");
        };
        assert!(sel.where_clause.is_some(), "{op:?} must map");
    }
}

#[test]
fn unary_tilde_short_circuits_to_none() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::UnaryOp {
        op: crate::ast::UnaryOperator::Tilde,
        expr: Box::new(expr_ident("a")),
        span: span(),
    });
    let stmt = Statement::Select(Box::new(s));
    assert!(to_common_sql(&stmt).is_none(), "Tilde -> None");
}

#[test]
fn like_maps_to_comparison_like_and_drops_escape() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::Like {
        expr: Box::new(expr_ident("name")),
        pattern: Box::new(expr_str("J%")),
        escape: Some(Box::new(expr_str("\\"))),
        negated: false,
        span: span(),
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match sel.where_clause {
        Some(SqlExpr::Comparison { op, .. }) => assert_eq!(op, SqlComparison::Like),
        other => panic!("expected Comparison Like, got {other:?}"),
    }
}

#[test]
fn not_like_maps_to_comparison_notlike() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::Like {
        expr: Box::new(expr_ident("name")),
        pattern: Box::new(expr_str("J%")),
        escape: None,
        negated: true,
        span: span(),
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match sel.where_clause {
        Some(SqlExpr::Comparison { op, .. }) => assert_eq!(op, SqlComparison::NotLike),
        other => panic!("expected Comparison NotLike, got {other:?}"),
    }
}

#[test]
fn between_maps_with_negated_flag() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::Between {
        expr: Box::new(expr_ident("age")),
        low: Box::new(expr_int(18)),
        high: Box::new(expr_int(65)),
        negated: true,
        span: span(),
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match sel.where_clause {
        Some(SqlExpr::Between { negated, .. }) => assert!(negated),
        other => panic!("expected Between, got {other:?}"),
    }
}

#[test]
fn in_values_maps_with_negated_flag() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::In {
        expr: Box::new(expr_ident("id")),
        list: crate::ast::InList::Values(vec![expr_int(1), expr_int(2)]),
        negated: true,
        span: span(),
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match sel.where_clause {
        Some(SqlExpr::In { negated, .. }) => assert!(negated),
        other => panic!("expected In, got {other:?}"),
    }
}

#[test]
fn case_always_has_none_operand() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(
        Expression::Case(crate::ast::CaseExpression {
            branches: vec![(expr_int(1), expr_str("one"))],
            else_result: None,
            span: span(),
        }),
        None,
    )];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match &sel.projection[0] {
        SqlSelectItem::Expression { expr, .. } => match expr {
            SqlExpr::Case {
                operand,
                conditions,
                ..
            } => {
                assert!(operand.is_none(), "operand must be None (lossy)");
                assert_eq!(conditions.len(), 1);
            }
            other => panic!("expected Case, got {other:?}"),
        },
        other => panic!("expected Expression item, got {other:?}"),
    }
}

#[test]
fn is_null_maps_to_isnull_node() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::Is {
        expr: Box::new(expr_ident("name")),
        negated: true,
        value: crate::ast::IsValue::Null,
        span: span(),
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match sel.where_clause {
        Some(SqlExpr::IsNull { negated, .. }) => assert!(negated),
        other => panic!("expected IsNull, got {other:?}"),
    }
}

#[test]
fn is_unknown_maps_to_isnull_node() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::Is {
        expr: Box::new(expr_ident("name")),
        negated: false,
        value: crate::ast::IsValue::Unknown,
        span: span(),
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert!(matches!(sel.where_clause, Some(SqlExpr::IsNull { .. })));
}

#[test]
fn is_true_is_false_short_circuit_to_none() {
    for v in [crate::ast::IsValue::True, crate::ast::IsValue::False] {
        let mut s = select_star("t");
        s.where_clause = Some(Expression::Is {
            expr: Box::new(expr_ident("name")),
            negated: false,
            value: v,
            span: span(),
        });
        let stmt = Statement::Select(Box::new(s));
        assert!(to_common_sql(&stmt).is_none(), "IS {v:?} -> None");
    }
}

#[test]
fn function_call_maps_with_distinct_flag() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(
        Expression::FunctionCall(crate::ast::FunctionCall {
            name: ident_name("COUNT"),
            args: vec![crate::ast::FunctionArg::Expression(expr_ident("id"))],
            distinct: true,
            span: span(),
        }),
        None,
    )];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match &sel.projection[0] {
        SqlSelectItem::Expression { expr, .. } => match expr {
            SqlExpr::Function {
                name,
                args,
                distinct,
            } => {
                assert_eq!(name.value(), "COUNT");
                assert_eq!(args.len(), 1);
                assert!(*distinct);
            }
            other => panic!("expected Function, got {other:?}"),
        },
        other => panic!("expected Expression item, got {other:?}"),
    }
}

#[test]
fn function_arg_wildcard_maps_to_star_identifier() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(
        Expression::FunctionCall(crate::ast::FunctionCall {
            name: ident_name("COUNT"),
            args: vec![crate::ast::FunctionArg::Wildcard],
            distinct: false,
            span: span(),
        }),
        None,
    )];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match &sel.projection[0] {
        SqlSelectItem::Expression { expr, .. } => match expr {
            SqlExpr::Function { args, .. } => {
                assert!(matches!(args[0], SqlExpr::Identifier(_)));
            }
            other => panic!("expected Function, got {other:?}"),
        },
        other => panic!("expected Expression item, got {other:?}"),
    }
}

#[test]
fn exists_subquery_maps_with_negated_false() {
    let mut s = select_star("t");
    s.where_clause = Some(Expression::Exists(Box::new(select_star("inner"))));
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match sel.where_clause {
        Some(SqlExpr::Exists { negated, .. }) => assert!(!negated),
        other => panic!("expected Exists, got {other:?}"),
    }
}

#[test]
fn scalar_subquery_expression_maps() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(
        Expression::Subquery(Box::new(select_star("inner"))),
        None,
    )];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert!(matches!(
        &sel.projection[0],
        SqlSelectItem::Expression {
            expr: SqlExpr::Subquery(_),
            ..
        }
    ));
}

// ===================================================================
// 4. Literal mapping
// ===================================================================

#[test]
fn number_literal_parses_to_integer() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(expr_int(42), None)];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match &sel.projection[0] {
        SqlSelectItem::Expression { expr, .. } => {
            assert!(matches!(expr, SqlExpr::Literal(SqlLit::Integer(42))));
        }
        other => panic!("expected Expression item, got {other:?}"),
    }
}

#[test]
fn string_literal_maps_to_string() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(expr_str("hi"), None)];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match &sel.projection[0] {
        SqlSelectItem::Expression {
            expr: SqlExpr::Literal(SqlLit::String(x)),
            ..
        } => assert_eq!(x, "hi"),
        other => panic!("expected String literal, got {other:?}"),
    }
}

#[test]
fn null_literal_maps() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(
        Expression::Literal(Literal::Null(span())),
        None,
    )];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert!(matches!(
        &sel.projection[0],
        SqlSelectItem::Expression {
            expr: SqlExpr::Literal(SqlLit::Null),
            ..
        }
    ));
}

#[test]
fn boolean_literal_maps() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(
        Expression::Literal(Literal::Boolean(true, span())),
        None,
    )];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    assert!(matches!(
        &sel.projection[0],
        SqlSelectItem::Expression {
            expr: SqlExpr::Literal(SqlLit::Boolean(true)),
            ..
        }
    ));
}

#[test]
fn hex_literal_short_circuits_to_none() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(
        Expression::Literal(Literal::Hex("0xFF".to_string(), span())),
        None,
    )];
    let stmt = Statement::Select(Box::new(s));
    assert!(to_common_sql(&stmt).is_none(), "Hex -> None");
}

#[test]
fn float_literal_renders_to_string() {
    let mut s = select_star("t");
    s.columns = vec![SelectItem::Expression(
        Expression::Literal(Literal::Float("1.5".to_string(), span())),
        None,
    )];
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match &sel.projection[0] {
        SqlSelectItem::Expression {
            expr: SqlExpr::Literal(SqlLit::Float(f)),
            ..
        } => assert_eq!(f, "1.5"),
        other => panic!("expected Float literal, got {other:?}"),
    }
}

// ===================================================================
// 5. INSERT mapping (incl. DefaultValues lossy mapping)
// ===================================================================

#[test]
fn insert_values_maps_table_columns_and_rows() {
    let ins = InsertStatement {
        span: span(),
        table: ident_name("users"),
        columns: vec![ident_name("id"), ident_name("name")],
        source: InsertSource::Values(vec![vec![expr_int(1), expr_str("a")]]),
    };
    let stmt = Statement::Insert(Box::new(ins));
    let SqlStmt::Insert(ins) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Insert");
    };
    assert_eq!(ins.columns.len(), 2);
    assert_eq!(ins.columns[0].value(), "id");
    assert_eq!(ins.table.name(), "users");
    assert!(ins.table.schema().is_none());
    assert!(ins.on_conflict.is_none());
    match &ins.source {
        SqlInsertSource::Values(rows) => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].len(), 2);
        }
        other => panic!("expected Values, got {other:?}"),
    }
}

#[test]
fn insert_default_values_maps_lossily_to_empty_values() {
    let ins = InsertStatement {
        span: span(),
        table: ident_name("users"),
        columns: vec![],
        source: InsertSource::DefaultValues,
    };
    let stmt = Statement::Insert(Box::new(ins));
    let SqlStmt::Insert(ins) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Insert");
    };
    match &ins.source {
        SqlInsertSource::Values(rows) => assert!(rows.is_empty()),
        other => panic!("expected empty Values for DefaultValues, got {other:?}"),
    }
}

#[test]
fn insert_select_maps_to_insert_source_select() {
    let ins = InsertStatement {
        span: span(),
        table: ident_name("archive"),
        columns: vec![ident_name("id")],
        source: InsertSource::Select(Box::new(select_star("source"))),
    };
    let stmt = Statement::Insert(Box::new(ins));
    let SqlStmt::Insert(ins) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Insert");
    };
    assert!(matches!(ins.source, SqlInsertSource::Select(_)));
}

// ===================================================================
// 6. UPDATE / DELETE mapping
// ===================================================================

#[test]
fn update_basic_maps_assignments_and_where() {
    let upd = UpdateStatement {
        span: span(),
        table: table_ref("users"),
        assignments: vec![ColumnAssignment {
            column: ident_name("name"),
            value: expr_str("x"),
        }],
        from_clause: None,
        where_clause: Some(Expression::BinaryOp {
            left: Box::new(expr_ident("id")),
            op: BinaryOperator::Eq,
            right: Box::new(expr_int(5)),
            span: span(),
        }),
    };
    let stmt = Statement::Update(Box::new(upd));
    let SqlStmt::Update(upd) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Update");
    };
    assert_eq!(upd.assignments.len(), 1);
    assert!(upd.from.is_none()); // legacy has no FROM here
    let SqlTableFactor::Table { name, alias } = &upd.table else {
        panic!("expected Table factor");
    };
    assert_eq!(name.name(), "users");
    assert!(alias.is_none());
    let SqlAssignment { column, .. } = &upd.assignments[0];
    assert_eq!(column.value(), "name");
}

#[test]
fn update_with_from_clause_returns_none() {
    // Stage-1: UPDATE with FROM -> DialectSpecific -> Stage-2 -> None.
    let upd = UpdateStatement {
        span: span(),
        table: table_ref("t"),
        assignments: vec![],
        from_clause: Some(from_one("other")),
        where_clause: None,
    };
    let stmt = Statement::Update(Box::new(upd));
    assert!(to_common_sql(&stmt).is_none(), "UPDATE..FROM -> None");
}

#[test]
fn delete_basic_maps() {
    let del = ast::DeleteStatement {
        span: span(),
        table: ident_name("users"),
        from_clause: None,
        where_clause: Some(Expression::BinaryOp {
            left: Box::new(expr_ident("id")),
            op: BinaryOperator::Eq,
            right: Box::new(expr_int(5)),
            span: span(),
        }),
    };
    let stmt = Statement::Delete(Box::new(del));
    let SqlStmt::Delete(del) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Delete");
    };
    let SqlTableFactor::Table { name, .. } = &del.table else {
        panic!("expected Table factor");
    };
    assert_eq!(name.name(), "users");
    assert!(del.using.is_none());
    assert!(del.where_clause.is_some());
}

#[test]
fn delete_with_from_clause_returns_none() {
    let del = ast::DeleteStatement {
        span: span(),
        table: ident_name("t"),
        from_clause: Some(from_one("other")),
        where_clause: None,
    };
    let stmt = Statement::Delete(Box::new(del));
    assert!(to_common_sql(&stmt).is_none(), "DELETE..FROM -> None");
}

// ===================================================================
// 7. Nested recursion survives
// ===================================================================

#[test]
fn nested_binary_op_recurses() {
    // (a + b) > 0  ->  Comparison { left: BinaryOp{Add}, op: Gt, right: 0 }
    let inner = Expression::BinaryOp {
        left: Box::new(expr_ident("a")),
        op: BinaryOperator::Plus,
        right: Box::new(expr_ident("b")),
        span: span(),
    };
    let mut s = select_star("t");
    s.where_clause = Some(Expression::BinaryOp {
        left: Box::new(inner),
        op: BinaryOperator::Gt,
        right: Box::new(expr_int(0)),
        span: span(),
    });
    let stmt = Statement::Select(Box::new(s));
    let SqlStmt::Select(sel) = to_common_sql(&stmt).expect("converts") else {
        panic!("expected Select");
    };
    match sel.where_clause {
        Some(SqlExpr::Comparison { op, left, .. }) => {
            assert_eq!(op, SqlComparison::Gt);
            assert!(matches!(*left, SqlExpr::BinaryOp { .. }));
        }
        other => panic!("expected Comparison, got {other:?}"),
    }
}

// ===================================================================
// 8. QualifiedName used correctly (sanity for shape)
// ===================================================================

#[test]
fn qualified_name_constructs_without_schema() {
    // Confirm the destination QualifiedName shape we rely on.
    let qn = QualifiedName::new(None, "users".to_string());
    assert_eq!(qn.name(), "users");
    assert!(qn.schema().is_none());
}

#[test]
fn table_alias_constructs() {
    let a = TableAlias::new("u".to_string(), vec![]);
    assert_eq!(a.name(), "u");
}

// ===================================================================
// 9. DDL bridge: CREATE TABLE / CREATE INDEX / ALTER TABLE  (Task T2.1a RED)
//
// `to_common_sql` currently maps `Statement::Create(_) | AlterTable(_) |
// BatchSeparator(_)` to `None` (to_common_sql.rs:144). Task T2 wires DDL into
// dedicated `common_sql` destination variants:
//   * `Create(Table)`            -> `Some(CreateTable(CreateTableStatement))`
//   * `Create(Index)`            -> `Some(CreateIndex(CreateIndexStatement))`
//   * `AlterTable(AddColumn)`    -> `Some(AlterTable(AlterTableStatement))`
//   * `Create(View|Procedure|Trigger)` -> `None` (no destination)
//   * `BatchSeparator`           -> `None` (unchanged)
//
// Per design.md §0.6, a CREATE TABLE whose column list contains a DataType
// with no common-sql destination (Bit/Money/SmallMoney/SmallDateTime) short-
// circuits the whole statement to None (parity with convert_select). Only
// UniqueIdentifier maps (to Uuid) and returns Some.
// ===================================================================

use crate::ast::{
    AddColumnDefinition, AlterTableOperation, AlterTableStatement,
    ColumnDefinition as TsqlColumnDefinition, CreateStatement, DataType as TsqlDataType,
    IndexDefinition, TableDefinition, TriggerDefinition, TriggerEvent, ViewDefinition,
};
use common_sql::ast::{
    AlterTableAction as CsqlAlterAction, AlterTableStatement as CsqlAlterTable,
    ColumnDef as CsqlColumnDef, CreateIndexStatement as CsqlCreateIndex,
    CreateTableStatement as CsqlCreateTable, IndexColumn as CsqlIndexColumn, Statement as CsqlStmt,
};
use tsql_token::Span as TsqlSpan;

fn cspan() -> TsqlSpan {
    TsqlSpan::new(0, 100)
}

fn col_def(name: &str, dt: TsqlDataType) -> TsqlColumnDefinition {
    TsqlColumnDefinition {
        name: ident_name(name),
        data_type: dt,
        nullability: None,
        default_value: None,
        identity: false,
        constraints: vec![],
    }
}

fn table_def(name: &str, columns: Vec<TsqlColumnDefinition>) -> TableDefinition {
    TableDefinition {
        span: cspan(),
        name: ident_name(name),
        columns,
        constraints: vec![],
        temporary: false,
    }
}

// --- Happy path ---

#[test]
fn create_table_single_column_returns_create_table() {
    let td = table_def("users", vec![col_def("id", TsqlDataType::Int)]);
    let stmt = Statement::Create(Box::new(CreateStatement::Table(td)));
    let converted = to_common_sql(&stmt);
    let Some(CsqlStmt::CreateTable(boxed)) = converted else {
        panic!("expected Some(CreateTable), got {converted:?}");
    };
    let CsqlCreateTable {
        name,
        columns,
        temporary,
        if_not_exists,
        ..
    } = boxed.as_ref();
    assert_eq!(name.name(), "users");
    assert!(!temporary);
    assert!(!if_not_exists);
    assert_eq!(columns.len(), 1);
    let CsqlColumnDef {
        name, data_type, ..
    } = &columns[0];
    assert_eq!(name.value(), "id");
    assert_eq!(*data_type, common_sql::ast::DataType::Int);
}

#[test]
fn create_index_returns_create_index() {
    let idx = IndexDefinition {
        span: cspan(),
        name: ident_name("idx_users_id"),
        table: ident_name("users"),
        columns: vec![ident_name("id")],
        unique: true,
    };
    let stmt = Statement::Create(Box::new(CreateStatement::Index(idx)));
    let converted = to_common_sql(&stmt);
    let Some(CsqlStmt::CreateIndex(boxed)) = converted else {
        panic!("expected Some(CreateIndex), got {converted:?}");
    };
    let CsqlCreateIndex {
        name,
        table,
        columns,
        unique,
        if_not_exists,
        ..
    } = boxed.as_ref();
    assert_eq!(name.value(), "idx_users_id");
    assert_eq!(table.name(), "users");
    assert!(unique);
    assert!(!if_not_exists);
    assert_eq!(columns.len(), 1);
    let CsqlIndexColumn { name: cn, .. } = &columns[0];
    assert_eq!(cn.value(), "id");
}

#[test]
fn alter_table_add_column_returns_alter_table() {
    let alter = AlterTableStatement {
        span: cspan(),
        table: ident_name("users"),
        operation: AlterTableOperation::AddColumn(AddColumnDefinition {
            name: ident_name("email"),
            data_type: TsqlDataType::Varchar(Some(255)),
            nullability: Some(true),
            identity: false,
        }),
    };
    let stmt = Statement::AlterTable(Box::new(alter));
    let converted = to_common_sql(&stmt);
    let Some(CsqlStmt::AlterTable(boxed)) = converted else {
        panic!("expected Some(AlterTable), got {converted:?}");
    };
    // T-SQL operation (singular) must be wrapped into actions: Vec (plural).
    let CsqlAlterTable { name, actions, .. } = boxed.as_ref();
    assert_eq!(name.name(), "users");
    assert_eq!(actions.len(), 1);
    let CsqlAlterAction::AddColumn(CsqlColumnDef {
        name: cn,
        data_type,
        ..
    }) = &actions[0]
    else {
        panic!("expected AddColumn action, got {:?}", actions[0]);
    };
    assert_eq!(cn.value(), "email");
    assert_eq!(
        *data_type,
        common_sql::ast::DataType::VarChar { length: Some(255) }
    );
}

// --- Edge: Create variants with no destination -> None ---

#[test]
fn create_view_returns_none_t2() {
    let view = ViewDefinition {
        span: cspan(),
        name: ident_name("v_users"),
        query: Box::new(select_star("users")),
    };
    let stmt = Statement::Create(Box::new(CreateStatement::View(view)));
    assert!(to_common_sql(&stmt).is_none(), "CREATE VIEW -> None");
}

#[test]
fn create_trigger_returns_none_t2() {
    let trig = TriggerDefinition {
        span: cspan(),
        name: ident_name("tr_users_ins"),
        table: ident_name("users"),
        events: vec![TriggerEvent::Insert],
        body: vec![],
    };
    let stmt = Statement::Create(Box::new(CreateStatement::Trigger(trig)));
    assert!(to_common_sql(&stmt).is_none(), "CREATE TRIGGER -> None");
}

#[test]
fn batch_separator_returns_none_unchanged() {
    let stmt = Statement::BatchSeparator(ast::BatchSeparator {
        span: cspan(),
        repeat_count: None,
    });
    assert!(to_common_sql(&stmt).is_none(), "BatchSeparator -> None");
}

// --- DataType short-circuit (design.md §0.6) ---
//
// Bit / Money / SmallMoney / SmallDateTime have no common-sql destination:
// the whole CREATE TABLE returns None (parity with convert_select's whole-
// statement None contract). UniqueIdentifier maps to Uuid and returns Some.

fn table_with_col(dt: TsqlDataType) -> Statement {
    let td = table_def("t", vec![col_def("c", dt)]);
    Statement::Create(Box::new(CreateStatement::Table(td)))
}

#[test]
fn create_table_bit_column_short_circuits_to_none() {
    assert!(to_common_sql(&table_with_col(TsqlDataType::Bit)).is_none());
}

#[test]
fn create_table_money_column_short_circuits_to_none() {
    assert!(to_common_sql(&table_with_col(TsqlDataType::Money)).is_none());
}

#[test]
fn create_table_smallmoney_column_short_circuits_to_none() {
    assert!(
        to_common_sql(&table_with_col(TsqlDataType::SmallMoney)).is_none(),
        "SmallMoney has no common-sql dest -> whole stmt None"
    );
}

#[test]
fn create_table_smalldatetime_column_short_circuits_to_none() {
    assert!(
        to_common_sql(&table_with_col(TsqlDataType::SmallDateTime)).is_none(),
        "SmallDateTime has no common-sql dest -> whole stmt None"
    );
}

#[test]
fn create_table_uniqueidentifier_maps_to_uuid() {
    let stmt = table_with_col(TsqlDataType::UniqueIdentifier);
    let Some(CsqlStmt::CreateTable(boxed)) = to_common_sql(&stmt) else {
        panic!("expected Some(CreateTable) for UniqueIdentifier");
    };
    let col = &boxed.columns[0];
    assert_eq!(col.data_type, common_sql::ast::DataType::Uuid);
}

// --- Non-regression: existing None-propagation for DML unchanged ---

#[test]
fn existing_create_none_dispatch_does_not_touch_select() {
    // SELECT still converts even though Create/AlterTable/BatchSeparator share
    // the dispatch table — the DDL wiring must not alter SELECT/INSERT paths.
    let sel = select_star("users");
    let stmt = Statement::Select(Box::new(sel));
    assert!(to_common_sql(&stmt).is_some(), "SELECT must still convert");
}
