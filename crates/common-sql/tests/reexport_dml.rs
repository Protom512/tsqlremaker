//! Task 4.3 / Task 3: verify DML nodes are re-exported from the crate root.
//!
//! These imports go through `common_sql::ast::*` (the public surface). If the
//! `mod.rs` re-exports are missing, this file fails to compile.

#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use common_sql::ast::{
    Assignment, DeleteStatement, InsertSource, InsertStatement, OnConflict, Statement,
    UpdateStatement,
};
use common_sql::ast::{
    Expression, Identifier, Literal, QualifiedName, SelectItem, SelectStatement, Span, TableAlias,
    TableFactor,
};

fn ident_expr(name: &str) -> Expression {
    Expression::Identifier(Identifier::new(name.to_string()))
}

fn int_expr(n: i64) -> Expression {
    Expression::Literal(Literal::Integer(n))
}

fn qualified(name: &str) -> QualifiedName {
    QualifiedName::new(None, name.to_string())
}

fn table_factor(name: &str) -> TableFactor {
    TableFactor::Table {
        name: qualified(name),
        alias: None,
    }
}

#[test]
fn insert_statement_is_reachable_and_buildable() {
    let stmt = InsertStatement {
        span: Span::new(0, 10),
        table: qualified("users"),
        columns: vec![Identifier::new("id".to_string())],
        source: InsertSource::Values(vec![vec![int_expr(1)]]),
        on_conflict: None,
    };
    let wrapped = Statement::Insert(Box::new(stmt.clone()));
    assert!(matches!(wrapped, Statement::Insert(_)));
    assert_eq!(wrapped.clone(), wrapped);
}

#[test]
fn insert_source_select_variant_is_reachable() {
    let inner = SelectStatement::simple(vec![SelectItem::Wildcard]);
    let src = InsertSource::Select(Box::new(inner));
    assert!(matches!(src, InsertSource::Select(_)));
}

#[test]
fn update_statement_is_reachable_and_buildable() {
    let stmt = UpdateStatement {
        span: Span::new(0, 10),
        table: table_factor("users"),
        assignments: vec![Assignment {
            column: Identifier::new("name".to_string()),
            value: ident_expr("bob"),
        }],
        from: None,
        where_clause: Some(ident_expr("cond")),
    };
    let wrapped = Statement::Update(Box::new(stmt));
    assert!(matches!(wrapped, Statement::Update(_)));
}

#[test]
fn delete_statement_is_reachable_and_buildable() {
    let stmt = DeleteStatement {
        span: Span::new(0, 10),
        table: table_factor("users"),
        using: None,
        where_clause: Some(ident_expr("cond")),
    };
    let wrapped = Statement::Delete(Box::new(stmt));
    assert!(matches!(wrapped, Statement::Delete(_)));
}

#[test]
fn on_conflict_remains_reachable() {
    let oc = OnConflict {
        span: Span::new(0, 1),
        action: common_sql::ast::ConflictAction::DoNothing,
        conflict_target: None,
    };
    let ins = InsertStatement {
        span: Span::new(0, 1),
        table: qualified("t"),
        columns: vec![],
        source: InsertSource::Values(vec![]),
        on_conflict: Some(oc),
    };
    assert!(ins.on_conflict.is_some());
}

// Touch TableAlias so the import is used regardless of fn bodies above.
#[test]
fn _table_alias_import_used() {
    let _ = TableAlias::new("x".to_string(), vec![]);
}
