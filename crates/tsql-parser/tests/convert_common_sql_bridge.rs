//! T1 RED PHASE (Task #35 / issue #148) — `Common* -> common_sql::ast` bridge.
//!
//! These integration tests exercise the **composite** half of the conversion
//! bridge that lives in `crates/tsql-parser/src/common/convert_common_sql.rs`:
//! the statement-level nodes and the `convert()` entry point. They are the
//! red-phase counterparts to the green-phase work tracked in tasks #36 / #40
//! / #44.
//!
//! The leaf conversions (Literal / Identifier / UnaryOperator / DataType) and
//! the T2 operator-dispatch + T3 expression bridge are covered by the inline
//! unit tests in `convert_common_sql.rs` and are NOT duplicated here. This
//! file targets the nodes the green phase has NOT yet wired:
//!
//!   * `convert(stmt: CommonStatement) -> Option<Statement>` entry point
//!     (design decision (a): `DialectSpecific -> None`, so a free fn, not a
//!     plain `From<CommonStatement>`)
//!   * `From<CommonStatement>`-equivalent dispatch for Select/Insert/Update/
//!     Delete/DialectSpecific
//!   * `From<CommonSelectStatement>` full mapping (projection, FROM-as-first-
//!     element, WHERE, GROUP BY, HAVING, ORDER BY, LIMIT)
//!   * `From<CommonTableReference>` (Table / Derived)
//!   * `From<CommonInsertStatement>` + `CommonInsertSource` (design decision
//!     (e): `DefaultValues -> InsertSource::Values(vec![])`)
//!   * `From<CommonUpdateStatement>`, `From<CommonDeleteStatement>`
//!   * `From<CommonAssignment>`, `From<CommonSelectItem>`,
//!     `From<CommonOrderByItem>`, `From<CommonLimitClause>`
//!
//! ## Pinned design decisions (from the estimate approval)
//!
//!   (a) `DialectSpecific` has no destination variant -> `convert()` returns
//!       `None`. The entry point is a free fn (or `TryFrom`), NOT a plain
//!       `From<CommonStatement>`.
//!   (b) `Vec<CommonTableReference>` (legacy FROM) -> `Option<TableFactor>`:
//!       take the first element; extras are dropped (documented lossy).
//!   (c) `LIKE` -> `ComparisonOperator::{Like, NotLike}`; the ESCAPE clause
//!       is silently dropped. `ILike` / `NotILike` are never produced.
//!   (d) CASE: `operand` is always `None` (legacy is searched-CASE only);
//!       the source `branches` field maps to the destination `conditions`.
//!   (e) `CommonInsertSource::DefaultValues` -> `InsertSource::Values(vec![])`
//!       (empty `Values`, documented as lossy).
//!
//! ## Span loss
//!
//! Span is silently dropped for every node crossing the bridge (the
//! destination `Expression` / `Statement` carry a `Span` field, but the
//! legacy source carries `tsql_token::Span`; the bridge fills the
//! destination span with a default). No test here asserts byte offsets.
//!
//! ## Exhaustiveness guards
//!
//! The legacy enums are NOT `#[non_exhaustive]`, so a `match` in the green
//! phase compiles to enforce lockstep: adding a new legacy variant breaks the
//! match. The exhaustiveness tests at the end of this file assert the known
//! variant counts (5 statement variants, 3 insert-source variants) so a
//! future contributor cannot silently drop one.

// Test code: unwrap/panic/expect are idiomatic here.
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use common_sql::ast::{
    InsertSource as SqlInsertSource, LimitClause as SqlLimitClause, OrderByItem as SqlOrderByItem,
    SelectItem as SqlSelectItem, SelectStatement as SqlSelectStatement,
    SortDirection as SqlSortDirection, Statement as SqlStatement, TableFactor as SqlTableFactor,
};
use tsql_parser::common::{
    CommonAssignment, CommonDeleteStatement, CommonExpression, CommonIdentifier,
    CommonInsertSource, CommonInsertStatement, CommonLimitClause, CommonLiteral, CommonOrderByItem,
    CommonSelectItem, CommonSelectStatement, CommonStatement, CommonTableReference,
    CommonUpdateStatement,
};
// The `convert` entry point will be re-exported from `tsql_parser::common`
// (approval condition: "Re-export the entry point from mod.rs"). Until the
// green phase lands it is expected to be missing -> this file does not
// compile, which is the intended red state.
use tsql_parser::common::convert;
// `From` impls live in `convert_common_sql.rs` and are in scope via the
// `common_sql::ast` types themselves (trait `From` is in the prelude).
use common_sql::ast::Expression as SqlExpression;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn legacy_span() -> tsql_token::Span {
    tsql_token::Span::new(1, 2)
}

fn lit_int(n: i64) -> CommonExpression {
    CommonExpression::Literal(CommonLiteral::Integer(n))
}

fn ident(name: &str) -> CommonExpression {
    CommonExpression::Identifier(CommonIdentifier {
        name: name.to_string(),
    })
}

fn trivial_select() -> CommonSelectStatement {
    CommonSelectStatement {
        span: legacy_span(),
        distinct: false,
        columns: vec![CommonSelectItem::Wildcard],
        from: vec![],
        where_clause: None,
        group_by: vec![],
        having: None,
        order_by: vec![],
        limit: None,
    }
}

// =====================================================================
// CommonSelectItem / CommonOrderByItem / CommonLimitClause
// =====================================================================

#[test]
fn select_item_expression_with_alias_maps() {
    let src = CommonSelectItem::Expression(ident("c"), Some("alias".to_string()));
    let got = SqlSelectItem::from(src);
    match got {
        SqlSelectItem::Expression { alias, .. } => {
            assert_eq!(alias.unwrap().value(), "alias");
        }
        other => panic!("expected Expression, got {other:?}"),
    }
}

#[test]
fn select_item_expression_without_alias_maps() {
    let src = CommonSelectItem::Expression(ident("c"), None);
    let got = SqlSelectItem::from(src);
    match got {
        SqlSelectItem::Expression { alias, .. } => assert!(alias.is_none()),
        other => panic!("expected Expression, got {other:?}"),
    }
}

#[test]
fn select_item_wildcard_maps() {
    let got = SqlSelectItem::from(CommonSelectItem::Wildcard);
    assert!(matches!(got, SqlSelectItem::Wildcard));
}

#[test]
fn select_item_qualified_wildcard_maps() {
    let got = SqlSelectItem::from(CommonSelectItem::QualifiedWildcard("t".to_string()));
    match got {
        SqlSelectItem::QualifiedWildcard { table } => assert_eq!(table.value(), "t"),
        other => panic!("expected QualifiedWildcard, got {other:?}"),
    }
}

#[test]
fn order_by_item_asc_maps_to_direction_asc() {
    let src = CommonOrderByItem {
        expr: ident("name"),
        asc: true,
    };
    let got = SqlOrderByItem::from(src);
    assert_eq!(got.direction, Some(SqlSortDirection::Asc));
}

#[test]
fn order_by_item_desc_maps_to_direction_desc() {
    let src = CommonOrderByItem {
        expr: ident("name"),
        asc: false,
    };
    let got = SqlOrderByItem::from(src);
    assert_eq!(got.direction, Some(SqlSortDirection::Desc));
}

#[test]
fn limit_clause_maps_limit_and_offset() {
    let src = CommonLimitClause {
        limit: lit_int(10),
        offset: Some(lit_int(2)),
    };
    let got = SqlLimitClause::from(src);
    assert!(matches!(got.limit, SqlExpression::Literal(_)));
    assert!(got.offset.is_some());
}

#[test]
fn limit_clause_without_offset_maps() {
    let src = CommonLimitClause {
        limit: lit_int(10),
        offset: None,
    };
    let got = SqlLimitClause::from(src);
    assert!(got.offset.is_none());
}

// =====================================================================
// CommonTableReference / CommonAssignment
// =====================================================================

#[test]
fn table_reference_table_maps_to_table_factor() {
    let src = CommonTableReference::Table {
        name: "users".to_string(),
        alias: Some("u".to_string()),
        span: legacy_span(),
    };
    let got = SqlTableFactor::from(src);
    match got {
        SqlTableFactor::Table { name, alias } => {
            assert_eq!(name.name(), "users");
            assert_eq!(alias.unwrap().name(), "u");
        }
        other => panic!("expected Table, got {other:?}"),
    }
}

#[test]
fn table_reference_table_without_alias_maps() {
    let src = CommonTableReference::Table {
        name: "users".to_string(),
        alias: None,
        span: legacy_span(),
    };
    let got = SqlTableFactor::from(src);
    match got {
        SqlTableFactor::Table { alias, .. } => assert!(alias.is_none()),
        other => panic!("expected Table, got {other:?}"),
    }
}

#[test]
fn table_reference_derived_maps_to_derived_factor() {
    let src = CommonTableReference::Derived {
        subquery: Box::new(trivial_select()),
        alias: Some("sub".to_string()),
        span: legacy_span(),
    };
    let got = SqlTableFactor::from(src);
    match got {
        SqlTableFactor::Derived { alias, .. } => {
            assert_eq!(alias.unwrap().name(), "sub");
        }
        other => panic!("expected Derived, got {other:?}"),
    }
}

#[test]
fn assignment_maps_column_and_value() {
    let src = CommonAssignment {
        column: "status".to_string(),
        value: lit_int(1),
    };
    let got = common_sql::ast::Assignment::from(src);
    assert_eq!(got.column.value(), "status");
    assert!(matches!(got.value, SqlExpression::Literal(_)));
}

// =====================================================================
// CommonSelectStatement (full mapping, not just the projection-only stub)
// =====================================================================

#[test]
fn select_statement_maps_projection_and_first_from() {
    // Design decision (b): Vec<CommonTableReference> -> Option<TableFactor>
    // takes the first element; extras are dropped (documented lossy).
    let src = CommonSelectStatement {
        span: legacy_span(),
        distinct: true,
        columns: vec![CommonSelectItem::Wildcard],
        from: vec![CommonTableReference::Table {
            name: "users".to_string(),
            alias: None,
            span: legacy_span(),
        }],
        where_clause: None,
        group_by: vec![],
        having: None,
        order_by: vec![],
        limit: None,
    };
    let got = SqlSelectStatement::from(src);
    assert_eq!(got.projection.len(), 1);
    assert!(got.from.is_some());
}

#[test]
fn select_statement_empty_from_yields_none() {
    let got = SqlSelectStatement::from(trivial_select());
    assert!(got.from.is_none());
}

#[test]
fn select_statement_carries_where_group_having_order_limit() {
    let src = CommonSelectStatement {
        span: legacy_span(),
        distinct: false,
        columns: vec![CommonSelectItem::Wildcard],
        from: vec![],
        where_clause: Some(ident("active")),
        group_by: vec![ident("dept")],
        having: Some(ident("total")),
        order_by: vec![CommonOrderByItem {
            expr: ident("name"),
            asc: true,
        }],
        limit: Some(CommonLimitClause {
            limit: lit_int(10),
            offset: None,
        }),
    };
    let got = SqlSelectStatement::from(src);
    assert!(got.where_clause.is_some());
    assert!(got.group_by.is_some());
    assert!(got.having.is_some());
    assert!(got.order_by.is_some());
    assert!(got.limit.is_some());
}

// =====================================================================
// convert() entry point + CommonStatement dispatch
// =====================================================================

#[test]
fn convert_select_statement_returns_some_select() {
    let got = convert(CommonStatement::Select(trivial_select()));
    assert!(got.is_some());
    assert!(matches!(got.unwrap(), SqlStatement::Select(_)));
}

#[test]
fn convert_insert_values_returns_some_insert() {
    let inner = CommonInsertStatement {
        span: legacy_span(),
        table: "users".to_string(),
        columns: vec!["id".to_string()],
        source: CommonInsertSource::Values(vec![vec![lit_int(1)]]),
    };
    let got = convert(CommonStatement::Insert(inner));
    assert!(got.is_some());
    assert!(matches!(got.unwrap(), SqlStatement::Insert(_)));
}

#[test]
fn convert_insert_default_values_maps_to_empty_values_rows() {
    // Design decision (e): DefaultValues -> InsertSource::Values(vec![]).
    let inner = CommonInsertStatement {
        span: legacy_span(),
        table: "t".to_string(),
        columns: vec![],
        source: CommonInsertSource::DefaultValues,
    };
    let got = convert(CommonStatement::Insert(inner)).expect("Some");
    match got {
        SqlStatement::Insert(ins) => match ins.source {
            SqlInsertSource::Values(rows) => assert!(rows.is_empty()),
            other => panic!("expected Values, got {other:?}"),
        },
        other => panic!("expected Insert, got {other:?}"),
    }
}

#[test]
fn convert_insert_select_returns_some_insert() {
    let inner = CommonInsertStatement {
        span: legacy_span(),
        table: "archive".to_string(),
        columns: vec!["id".to_string()],
        source: CommonInsertSource::Select(Box::new(trivial_select())),
    };
    let got = convert(CommonStatement::Insert(inner));
    assert!(got.is_some());
}

#[test]
fn convert_update_returns_some_update() {
    let inner = CommonUpdateStatement {
        span: legacy_span(),
        table: "users".to_string(),
        assignments: vec![CommonAssignment {
            column: "name".to_string(),
            value: lit_int(1),
        }],
        where_clause: None,
    };
    let got = convert(CommonStatement::Update(inner));
    assert!(got.is_some());
    assert!(matches!(got.unwrap(), SqlStatement::Update(_)));
}

#[test]
fn convert_delete_returns_some_delete() {
    let inner = CommonDeleteStatement {
        span: legacy_span(),
        table: "users".to_string(),
        where_clause: None,
    };
    let got = convert(CommonStatement::Delete(inner));
    assert!(got.is_some());
    assert!(matches!(got.unwrap(), SqlStatement::Delete(_)));
}

#[test]
fn convert_dialect_specific_returns_none() {
    // Design decision (a): DialectSpecific has no destination variant, so the
    // free fn `convert` returns None (NOT a plain From<CommonStatement>).
    let src = CommonStatement::DialectSpecific {
        description: "GRANT".to_string(),
        span: legacy_span(),
    };
    let got = convert(src);
    assert!(got.is_none());
}

// =====================================================================
// Exhaustiveness guards
// =====================================================================

#[test]
fn insert_source_exhaustiveness_all_three_variants_map() {
    let cases = [
        CommonInsertSource::Values(vec![]),
        CommonInsertSource::Select(Box::new(trivial_select())),
        CommonInsertSource::DefaultValues,
    ];
    for c in cases {
        let stmt = CommonInsertStatement {
            span: legacy_span(),
            table: "t".to_string(),
            columns: vec![],
            source: c,
        };
        assert!(convert(CommonStatement::Insert(stmt)).is_some());
    }
}

#[test]
fn statement_exhaustiveness_all_five_variants_handled() {
    // DialectSpecific -> None; the other four -> Some(...).
    let cases: Vec<CommonStatement> = vec![
        CommonStatement::Select(trivial_select()),
        CommonStatement::Insert(CommonInsertStatement {
            span: legacy_span(),
            table: "t".to_string(),
            columns: vec![],
            source: CommonInsertSource::DefaultValues,
        }),
        CommonStatement::Update(CommonUpdateStatement {
            span: legacy_span(),
            table: "t".to_string(),
            assignments: vec![],
            where_clause: None,
        }),
        CommonStatement::Delete(CommonDeleteStatement {
            span: legacy_span(),
            table: "t".to_string(),
            where_clause: None,
        }),
        CommonStatement::DialectSpecific {
            description: "x".to_string(),
            span: legacy_span(),
        },
    ];
    let mut some = 0;
    let mut none = 0;
    for c in cases {
        if convert(c).is_some() {
            some += 1;
        } else {
            none += 1;
        }
    }
    assert_eq!(some, 4);
    assert_eq!(none, 1);
}
