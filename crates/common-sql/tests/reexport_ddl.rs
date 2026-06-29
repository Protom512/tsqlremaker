//! Task 5.1 / Task 4: verify DDL nodes are re-exported and reachable from the
//! public surface, and that the `Statement` enum wires all five DDL variants.
//!
//! These imports go through `common_sql::ast::*` (the public surface). If the
//! `mod.rs` `pub use ddl::*;` re-export is missing, or any DDL `Statement`
//! variant is absent, this file fails to compile.

#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use common_sql::ast::{
    AlterTableAction, AlterTableStatement, ColumnConstraint, ColumnDef, CreateIndexStatement,
    CreateTableStatement, DataType, DropIndexStatement, DropTableStatement, Expression, Identifier,
    IndexColumn, QualifiedName, SelectItem, SortDirection, Statement, TableConstraint,
    TableOptions,
};
// Literal is needed to build a DEFAULT expression inside ColumnDef.
use common_sql::ast::Literal;
use common_sql::ast::Span;

fn ident(s: &str) -> Identifier {
    Identifier::new(s.to_string())
}

fn qualified(s: &str) -> QualifiedName {
    QualifiedName::new(None, s.to_string())
}

fn pk_column() -> ColumnDef {
    ColumnDef {
        span: Span::new(0, 10),
        name: ident("id"),
        data_type: DataType::BigInt,
        nullable: false,
        default: None,
        constraints: vec![ColumnConstraint::PrimaryKey],
    }
}

// ===== Reachability: every DDL type imports through the crate surface =====

#[test]
fn all_ddl_types_are_reachable_from_crate_root() {
    // Touch every re-exported DDL type so a missing re-export fails the build.
    let _col: ColumnDef = pk_column();
    let _cc: ColumnConstraint = ColumnConstraint::Unique;
    let _tc: TableConstraint = TableConstraint::PrimaryKey {
        name: None,
        columns: vec![ident("id")],
    };
    let _opts: TableOptions = TableOptions::default();
    let _action: AlterTableAction = AlterTableAction::DropColumn(ident("c"));
    let _idx_col: IndexColumn = IndexColumn {
        name: ident("c"),
        direction: None,
    };
    let _dir: SortDirection = SortDirection::Asc;
}

// ===== Statement enum wiring: all five DDL variants =====

#[test]
fn statement_create_table_wraps_create_table_statement() {
    let inner = CreateTableStatement {
        span: Span::new(0, 20),
        if_not_exists: false,
        temporary: false,
        name: qualified("users"),
        columns: vec![pk_column()],
        constraints: vec![],
        options: TableOptions::default(),
    };
    let stmt = Statement::CreateTable(Box::new(inner));
    assert!(matches!(stmt, Statement::CreateTable(_)));
    // DDL discriminants must be distinguishable from DML ones.
    assert!(!matches!(stmt, Statement::Select(_)));
    assert!(!matches!(stmt, Statement::Insert(_)));
    // Clone + PartialEq survive the boxed DDL variant.
    assert_eq!(stmt.clone(), stmt);
}

#[test]
fn statement_alter_table_wraps_alter_table_statement() {
    let inner = AlterTableStatement {
        span: Span::new(0, 20),
        name: qualified("users"),
        actions: vec![AlterTableAction::AddColumn(ColumnDef {
            span: Span::new(5, 20),
            name: ident("email"),
            data_type: DataType::VarChar { length: Some(255) },
            nullable: true,
            default: None,
            constraints: vec![],
        })],
    };
    let stmt = Statement::AlterTable(Box::new(inner));
    assert!(matches!(stmt, Statement::AlterTable(_)));
    assert_eq!(stmt.clone(), stmt);
}

#[test]
fn statement_drop_table_wraps_drop_table_statement() {
    let inner = DropTableStatement {
        span: Span::new(0, 20),
        if_exists: false,
        names: vec![qualified("users")],
    };
    let stmt = Statement::DropTable(Box::new(inner));
    assert!(matches!(stmt, Statement::DropTable(_)));
    assert_eq!(stmt.clone(), stmt);
}

#[test]
fn statement_create_index_wraps_create_index_statement() {
    let inner = CreateIndexStatement {
        span: Span::new(0, 40),
        unique: true,
        if_not_exists: false,
        name: ident("uk_email"),
        table: qualified("users"),
        columns: vec![IndexColumn {
            name: ident("email"),
            direction: Some(SortDirection::Desc),
        }],
    };
    let stmt = Statement::CreateIndex(Box::new(inner));
    assert!(matches!(stmt, Statement::CreateIndex(_)));
    assert_eq!(stmt.clone(), stmt);
}

#[test]
fn statement_drop_index_wraps_drop_index_statement() {
    let inner = DropIndexStatement {
        span: Span::new(0, 20),
        if_exists: false,
        name: ident("uk_email"),
        table: Some(qualified("users")),
    };
    let stmt = Statement::DropIndex(Box::new(inner));
    assert!(matches!(stmt, Statement::DropIndex(_)));
    assert_eq!(stmt.clone(), stmt);
}

// ===== End-to-end: a CREATE TABLE carrying a CHECK constraint + DEFAULT =====

#[test]
fn create_table_round_trips_with_constraint_and_default() {
    let inner = CreateTableStatement {
        span: Span::new(0, 80),
        if_not_exists: true,
        temporary: false,
        name: qualified("accounts"),
        columns: vec![
            pk_column(),
            ColumnDef {
                span: Span::new(20, 40),
                name: ident("balance"),
                data_type: DataType::Int,
                nullable: false,
                default: Some(Expression::Literal(Literal::Integer(0))),
                constraints: vec![ColumnConstraint::Check(Expression::Comparison {
                    left: Box::new(Expression::Identifier(ident("balance"))),
                    op: common_sql::ast::ComparisonOperator::Ge,
                    right: Box::new(Expression::Literal(Literal::Integer(0))),
                })],
            },
        ],
        constraints: vec![],
        options: TableOptions::default(),
    };
    let stmt = Statement::CreateTable(Box::new(inner));
    // Force the wildcard re-export path to resolve every DDL symbol.
    let cloned = stmt.clone();
    assert_eq!(stmt, cloned);
}

// Keep SelectItem import used so the test file does not warn on it.
#[test]
fn _select_item_import_used() {
    let _ = SelectItem::Wildcard;
}
