//! DDL (Data Definition Language) statement nodes.
//!
//! Covers `CREATE TABLE`, `ALTER TABLE`, `DROP TABLE`, `CREATE INDEX`, and
//! `DROP INDEX`. These nodes form the dialect-independent representation that
//! every SQL emitter consumes. Field shapes mirror `tsql-parser`'s
//! `TableDefinition` / `IndexDefinition` so the future conversion layer is a
//! near 1:1 mapping, and they carry the extra metadata the MySQL / PostgreSQL
//! emitters need (engine, charset, `IF NOT EXISTS`, etc.).

use crate::ast::clause::SortDirection;
use crate::ast::datatype::DataType;
use crate::ast::expression::Expression;
use crate::ast::identifier::{Identifier, QualifiedName};
use crate::ast::span::Span;

// ---------------------------------------------------------------------------
// Column / constraint primitives (shared by CREATE TABLE and ALTER TABLE)
// ---------------------------------------------------------------------------

/// A column definition inside a `CREATE TABLE` or `ALTER TABLE ... ADD`.
///
/// Represents `name data_type [NOT NULL] [DEFAULT expr] [constraints...]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    /// Source span of the column definition.
    pub span: Span,
    /// Column name.
    pub name: Identifier,
    /// Column data type.
    pub data_type: DataType,
    /// Whether the column allows NULL (`false` = `NOT NULL`).
    ///
    /// Defaults to `true` to match SQL's nullability default (columns are
    /// nullable unless `NOT NULL` is specified).
    pub nullable: bool,
    /// Optional `DEFAULT` expression.
    pub default: Option<Expression>,
    /// Column-level constraints.
    pub constraints: Vec<ColumnConstraint>,
}

/// A constraint attached to a single column.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConstraint {
    /// `PRIMARY KEY` (column-level).
    PrimaryKey,
    /// `UNIQUE` (column-level).
    Unique,
    /// `CHECK (expr)` (column-level).
    Check(Expression),
    /// `REFERENCES table (columns...)` — a foreign key reference.
    References {
        /// Referenced table name.
        table: QualifiedName,
        /// Referenced column names.
        columns: Vec<String>,
    },
    /// `AUTO_INCREMENT` / `IDENTITY`.
    AutoIncrement,
}

/// A table-level constraint (named, multi-column).
#[derive(Debug, Clone, PartialEq)]
pub enum TableConstraint {
    /// `PRIMARY KEY (cols...)`.
    PrimaryKey {
        /// Optional constraint name.
        name: Option<String>,
        /// Column list.
        columns: Vec<Identifier>,
    },
    /// `UNIQUE (cols...)`.
    Unique {
        /// Optional constraint name.
        name: Option<String>,
        /// Column list.
        columns: Vec<Identifier>,
    },
    /// `FOREIGN KEY (cols...) REFERENCES ref_table (ref_cols...)`.
    ForeignKey {
        /// Optional constraint name.
        name: Option<String>,
        /// Local column list.
        columns: Vec<Identifier>,
        /// Referenced table name.
        ref_table: QualifiedName,
        /// Referenced column list.
        ref_columns: Vec<Identifier>,
    },
    /// `CHECK (expr)`.
    Check {
        /// Optional constraint name.
        name: Option<String>,
        /// Check predicate.
        expr: Expression,
    },
}

/// Storage / table-level options emitted after the column list.
///
/// These are mostly MySQL-specific (`ENGINE`, `CHARSET`, `COLLATE`,
/// `COMMENT`) but harmless to other dialects, which ignore them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableOptions {
    /// Storage engine (e.g. `InnoDB`).
    pub engine: Option<String>,
    /// Character set (e.g. `utf8mb4`).
    pub charset: Option<String>,
    /// Collation (e.g. `utf8mb4_unicode_ci`).
    pub collation: Option<String>,
    /// Table comment.
    pub comment: Option<String>,
}

// ---------------------------------------------------------------------------
// CREATE TABLE
// ---------------------------------------------------------------------------

/// `CREATE TABLE` statement.
///
/// Represents `CREATE [TEMPORARY] TABLE [IF NOT EXISTS] name (columns,
/// constraints) options`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableStatement {
    /// Source span of the entire statement.
    pub span: Span,
    /// `IF NOT EXISTS` guard.
    pub if_not_exists: bool,
    /// `TEMPORARY` flag.
    pub temporary: bool,
    /// Target table name (`schema.table` or just `table`).
    pub name: QualifiedName,
    /// Column definitions.
    pub columns: Vec<ColumnDef>,
    /// Table-level constraints.
    pub constraints: Vec<TableConstraint>,
    /// Table-level options (`ENGINE`, `CHARSET`, ...).
    pub options: TableOptions,
}

// ---------------------------------------------------------------------------
// ALTER TABLE
// ---------------------------------------------------------------------------

/// A single `ALTER TABLE` action.
#[derive(Debug, Clone, PartialEq)]
pub enum AlterTableAction {
    /// `ADD COLUMN col def` (the `COLUMN` keyword is optional in most dialects).
    AddColumn(ColumnDef),
    /// `DROP COLUMN col`.
    DropColumn(Identifier),
    /// `ALTER COLUMN col ...` (type / default changes).
    AlterColumn {
        /// Target column name.
        column: Identifier,
        /// New data type, if changed.
        data_type: Option<DataType>,
        /// New default expression, if changed (`Some(None)` drops the default).
        default: Option<Option<Expression>>,
        /// New nullability, if changed.
        nullable: Option<bool>,
    },
    /// `ADD table-constraint`.
    AddConstraint(TableConstraint),
    /// `DROP CONSTRAINT name`.
    DropConstraint(String),
    /// `RENAME TO new_name`.
    RenameTo(QualifiedName),
}

/// `ALTER TABLE` statement.
///
/// Carries the target table and an ordered list of [`AlterTableAction`]s.
#[derive(Debug, Clone, PartialEq)]
pub struct AlterTableStatement {
    /// Source span of the entire statement.
    pub span: Span,
    /// Target table name.
    pub name: QualifiedName,
    /// Actions to apply, in source order.
    pub actions: Vec<AlterTableAction>,
}

// ---------------------------------------------------------------------------
// DROP TABLE
// ---------------------------------------------------------------------------

/// `DROP TABLE` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct DropTableStatement {
    /// Source span of the entire statement.
    pub span: Span,
    /// `IF EXISTS` guard.
    pub if_exists: bool,
    /// Tables to drop.
    pub names: Vec<QualifiedName>,
}

// ---------------------------------------------------------------------------
// CREATE INDEX
// ---------------------------------------------------------------------------

/// An ordered column inside a `CREATE INDEX`.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexColumn {
    /// The indexed column (or expression identifier).
    pub name: Identifier,
    /// Sort direction (`ASC` / `DESC`); `None` means default.
    ///
    /// Reuses [`SortDirection`](crate::ast::clause::SortDirection) so that
    /// index columns and `ORDER BY` items share one canonical type.
    pub direction: Option<SortDirection>,
}

/// `CREATE [UNIQUE] INDEX name ON table (cols...)`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateIndexStatement {
    /// Source span of the entire statement.
    pub span: Span,
    /// `UNIQUE` flag.
    pub unique: bool,
    /// `IF NOT EXISTS` guard.
    pub if_not_exists: bool,
    /// Index name.
    pub name: Identifier,
    /// Target table name.
    pub table: QualifiedName,
    /// Indexed columns (ordered).
    pub columns: Vec<IndexColumn>,
}

// ---------------------------------------------------------------------------
// DROP INDEX
// ---------------------------------------------------------------------------

/// `DROP INDEX name [ON table]`.
#[derive(Debug, Clone, PartialEq)]
pub struct DropIndexStatement {
    /// Source span of the entire statement.
    pub span: Span,
    /// `IF EXISTS` guard.
    pub if_exists: bool,
    /// Index name.
    pub name: Identifier,
    /// Optional table qualifier (some dialects require `ON table`).
    pub table: Option<QualifiedName>,
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
    use crate::ast::{ComparisonOperator, Literal};

    fn ident(s: &str) -> Identifier {
        Identifier::new(s.to_string())
    }

    fn qualified(s: &str) -> QualifiedName {
        QualifiedName::new(None, s.to_string())
    }

    // ===== ColumnDef / ColumnConstraint =====

    #[test]
    fn column_def_basic() {
        let col = ColumnDef {
            span: Span::new(0, 10),
            name: ident("id"),
            data_type: DataType::BigInt,
            nullable: false,
            default: None,
            constraints: vec![ColumnConstraint::PrimaryKey],
        };
        assert_eq!(col.name.value(), "id");
        assert!(!col.nullable);
        assert_eq!(col.constraints.len(), 1);
    }

    #[test]
    fn column_def_nullable_default_is_true() {
        let col = ColumnDef {
            span: Span::new(0, 5),
            name: ident("email"),
            data_type: DataType::VarChar { length: Some(255) },
            nullable: true,
            default: None,
            constraints: vec![],
        };
        assert!(col.nullable);
    }

    #[test]
    fn column_def_with_default_and_constraints() {
        let col = ColumnDef {
            span: Span::new(0, 20),
            name: ident("status"),
            data_type: DataType::Int,
            nullable: false,
            default: Some(Expression::Literal(Literal::Integer(0))),
            constraints: vec![ColumnConstraint::Unique, ColumnConstraint::AutoIncrement],
        };
        assert!(col.default.is_some());
        assert_eq!(col.constraints.len(), 2);
    }

    #[test]
    fn column_constraint_references() {
        let c = ColumnConstraint::References {
            table: qualified("users"),
            columns: vec!["id".to_string()],
        };
        if let ColumnConstraint::References { table, columns } = &c {
            assert_eq!(table.name(), "users");
            assert_eq!(columns.len(), 1);
        } else {
            panic!("expected References");
        }
    }

    #[test]
    fn column_constraint_check_carries_expression() {
        let c = ColumnConstraint::Check(Expression::Comparison {
            left: Box::new(Expression::Identifier(ident("age"))),
            op: ComparisonOperator::Ge,
            right: Box::new(Expression::Literal(Literal::Integer(18))),
        });
        assert!(matches!(c, ColumnConstraint::Check(_)));
    }

    #[test]
    fn column_constraint_clone_equality() {
        let c = ColumnConstraint::PrimaryKey;
        assert_eq!(c, c.clone());
    }

    #[test]
    fn column_def_clone_equality() {
        let col = ColumnDef {
            span: Span::new(0, 10),
            name: ident("id"),
            data_type: DataType::Int,
            nullable: false,
            default: None,
            constraints: vec![ColumnConstraint::PrimaryKey],
        };
        assert_eq!(col, col.clone());
    }

    // ===== TableConstraint =====

    #[test]
    fn table_constraint_primary_key_with_columns() {
        let tc = TableConstraint::PrimaryKey {
            name: Some("pk_users".to_string()),
            columns: vec![ident("id")],
        };
        if let TableConstraint::PrimaryKey { name, columns } = &tc {
            assert_eq!(name.as_ref().unwrap(), "pk_users");
            assert_eq!(columns.len(), 1);
        } else {
            panic!("expected PrimaryKey");
        }
    }

    #[test]
    fn table_constraint_unique_unnamed() {
        let tc = TableConstraint::Unique {
            name: None,
            columns: vec![ident("email")],
        };
        if let TableConstraint::Unique { name, columns } = &tc {
            assert!(name.is_none());
            assert_eq!(columns.len(), 1);
        } else {
            panic!("expected Unique");
        }
    }

    #[test]
    fn table_constraint_foreign_key_full() {
        let tc = TableConstraint::ForeignKey {
            name: Some("fk_order_user".to_string()),
            columns: vec![ident("user_id")],
            ref_table: qualified("users"),
            ref_columns: vec![ident("id")],
        };
        if let TableConstraint::ForeignKey {
            ref_table,
            ref_columns,
            ..
        } = &tc
        {
            assert_eq!(ref_table.name(), "users");
            assert_eq!(ref_columns.len(), 1);
        } else {
            panic!("expected ForeignKey");
        }
    }

    #[test]
    fn table_constraint_check() {
        let tc = TableConstraint::Check {
            name: None,
            expr: Expression::Comparison {
                left: Box::new(Expression::Identifier(ident("total"))),
                op: ComparisonOperator::Ge,
                right: Box::new(Expression::Literal(Literal::Integer(0))),
            },
        };
        assert!(matches!(tc, TableConstraint::Check { .. }));
    }

    #[test]
    fn table_constraint_clone_equality() {
        let tc = TableConstraint::PrimaryKey {
            name: Some("pk".to_string()),
            columns: vec![ident("id")],
        };
        assert_eq!(tc, tc.clone());
    }

    // ===== TableOptions =====

    #[test]
    fn table_options_default_all_none() {
        let opts = TableOptions::default();
        assert!(opts.engine.is_none());
        assert!(opts.charset.is_none());
        assert!(opts.collation.is_none());
        assert!(opts.comment.is_none());
    }

    #[test]
    fn table_options_mysql_engine_and_charset() {
        let opts = TableOptions {
            engine: Some("InnoDB".to_string()),
            charset: Some("utf8mb4".to_string()),
            collation: Some("utf8mb4_unicode_ci".to_string()),
            comment: Some("user table".to_string()),
        };
        assert_eq!(opts.engine.as_ref().unwrap(), "InnoDB");
        assert_eq!(opts.charset.as_ref().unwrap(), "utf8mb4");
    }

    // ===== CreateTableStatement =====

    #[test]
    fn create_table_basic() {
        let stmt = CreateTableStatement {
            span: Span::new(0, 100),
            if_not_exists: false,
            temporary: false,
            name: qualified("users"),
            columns: vec![ColumnDef {
                span: Span::new(0, 10),
                name: ident("id"),
                data_type: DataType::BigInt,
                nullable: false,
                default: None,
                constraints: vec![ColumnConstraint::PrimaryKey],
            }],
            constraints: vec![],
            options: TableOptions::default(),
        };
        assert_eq!(stmt.columns.len(), 1);
        assert!(!stmt.if_not_exists);
        assert!(!stmt.temporary);
    }

    #[test]
    fn create_table_if_not_exists_temporary_with_constraint() {
        let stmt = CreateTableStatement {
            span: Span::new(0, 50),
            if_not_exists: true,
            temporary: true,
            name: QualifiedName::new(Some("tempdb".to_string()), "t".to_string()),
            columns: vec![],
            constraints: vec![TableConstraint::PrimaryKey {
                name: None,
                columns: vec![ident("id")],
            }],
            options: TableOptions {
                engine: Some("InnoDB".to_string()),
                charset: None,
                collation: None,
                comment: None,
            },
        };
        assert!(stmt.if_not_exists);
        assert!(stmt.temporary);
        assert_eq!(stmt.name.schema(), Some("tempdb"));
        assert_eq!(stmt.constraints.len(), 1);
    }

    #[test]
    fn create_table_clone_equality() {
        let stmt = CreateTableStatement {
            span: Span::new(0, 10),
            if_not_exists: false,
            temporary: false,
            name: qualified("t"),
            columns: vec![ColumnDef {
                span: Span::new(0, 5),
                name: ident("c"),
                data_type: DataType::Int,
                nullable: true,
                default: None,
                constraints: vec![],
            }],
            constraints: vec![],
            options: TableOptions::default(),
        };
        assert_eq!(stmt, stmt.clone());
    }

    // ===== AlterTableStatement / AlterTableAction =====

    #[test]
    fn alter_table_add_column() {
        let stmt = AlterTableStatement {
            span: Span::new(0, 30),
            name: qualified("users"),
            actions: vec![AlterTableAction::AddColumn(ColumnDef {
                span: Span::new(10, 30),
                name: ident("email"),
                data_type: DataType::VarChar { length: Some(255) },
                nullable: true,
                default: None,
                constraints: vec![],
            })],
        };
        assert_eq!(stmt.actions.len(), 1);
        assert!(matches!(stmt.actions[0], AlterTableAction::AddColumn(_)));
    }

    #[test]
    fn alter_table_drop_column() {
        let stmt = AlterTableStatement {
            span: Span::new(0, 20),
            name: qualified("users"),
            actions: vec![AlterTableAction::DropColumn(ident("email"))],
        };
        assert!(matches!(stmt.actions[0], AlterTableAction::DropColumn(_)));
    }

    #[test]
    fn alter_table_alter_column_type() {
        let stmt = AlterTableStatement {
            span: Span::new(0, 20),
            name: qualified("users"),
            actions: vec![AlterTableAction::AlterColumn {
                column: ident("name"),
                data_type: Some(DataType::VarChar { length: Some(200) }),
                default: None,
                nullable: None,
            }],
        };
        if let AlterTableAction::AlterColumn {
            column, data_type, ..
        } = &stmt.actions[0]
        {
            assert_eq!(column.value(), "name");
            assert!(data_type.is_some());
        } else {
            panic!("expected AlterColumn");
        }
    }

    #[test]
    fn alter_table_add_constraint_and_drop_constraint() {
        let stmt = AlterTableStatement {
            span: Span::new(0, 60),
            name: qualified("users"),
            actions: vec![
                AlterTableAction::AddConstraint(TableConstraint::Unique {
                    name: Some("uk_email".to_string()),
                    columns: vec![ident("email")],
                }),
                AlterTableAction::DropConstraint("uk_email".to_string()),
            ],
        };
        assert_eq!(stmt.actions.len(), 2);
        assert!(matches!(
            stmt.actions[0],
            AlterTableAction::AddConstraint(_)
        ));
        assert!(matches!(
            stmt.actions[1],
            AlterTableAction::DropConstraint(_)
        ));
    }

    #[test]
    fn alter_table_rename_to() {
        let stmt = AlterTableStatement {
            span: Span::new(0, 20),
            name: qualified("old"),
            actions: vec![AlterTableAction::RenameTo(qualified("new"))],
        };
        if let AlterTableAction::RenameTo(new) = &stmt.actions[0] {
            assert_eq!(new.name(), "new");
        } else {
            panic!("expected RenameTo");
        }
    }

    #[test]
    fn alter_table_clone_equality() {
        let stmt = AlterTableStatement {
            span: Span::new(0, 10),
            name: qualified("t"),
            actions: vec![AlterTableAction::DropColumn(ident("c"))],
        };
        assert_eq!(stmt, stmt.clone());
    }

    // ===== DropTableStatement =====

    #[test]
    fn drop_table_single() {
        let stmt = DropTableStatement {
            span: Span::new(0, 20),
            if_exists: false,
            names: vec![qualified("users")],
        };
        assert_eq!(stmt.names.len(), 1);
        assert!(!stmt.if_exists);
    }

    #[test]
    fn drop_table_if_exists_multiple() {
        let stmt = DropTableStatement {
            span: Span::new(0, 40),
            if_exists: true,
            names: vec![qualified("a"), qualified("b")],
        };
        assert!(stmt.if_exists);
        assert_eq!(stmt.names.len(), 2);
    }

    #[test]
    fn drop_table_clone_equality() {
        let stmt = DropTableStatement {
            span: Span::new(0, 10),
            if_exists: false,
            names: vec![qualified("t")],
        };
        assert_eq!(stmt, stmt.clone());
    }

    // ===== CreateIndexStatement / IndexColumn / SortDirection =====

    #[test]
    fn create_index_basic() {
        let stmt = CreateIndexStatement {
            span: Span::new(0, 40),
            unique: false,
            if_not_exists: false,
            name: ident("idx_name"),
            table: qualified("users"),
            columns: vec![IndexColumn {
                name: ident("name"),
                direction: None,
            }],
        };
        assert_eq!(stmt.name.value(), "idx_name");
        assert!(!stmt.unique);
        assert_eq!(stmt.columns.len(), 1);
    }

    #[test]
    fn create_index_unique_with_direction() {
        let stmt = CreateIndexStatement {
            span: Span::new(0, 50),
            unique: true,
            if_not_exists: false,
            name: ident("uk_email"),
            table: qualified("users"),
            columns: vec![IndexColumn {
                name: ident("email"),
                direction: Some(SortDirection::Desc),
            }],
        };
        assert!(stmt.unique);
        assert_eq!(stmt.columns[0].direction, Some(SortDirection::Desc));
    }

    #[test]
    fn create_index_multi_column() {
        let stmt = CreateIndexStatement {
            span: Span::new(0, 60),
            unique: false,
            if_not_exists: true,
            name: ident("idx_last_first"),
            table: qualified("users"),
            columns: vec![
                IndexColumn {
                    name: ident("last"),
                    direction: Some(SortDirection::Asc),
                },
                IndexColumn {
                    name: ident("first"),
                    direction: None,
                },
            ],
        };
        assert!(stmt.if_not_exists);
        assert_eq!(stmt.columns.len(), 2);
    }

    #[test]
    fn create_index_clone_equality() {
        let stmt = CreateIndexStatement {
            span: Span::new(0, 10),
            unique: false,
            if_not_exists: false,
            name: ident("i"),
            table: qualified("t"),
            columns: vec![IndexColumn {
                name: ident("c"),
                direction: None,
            }],
        };
        assert_eq!(stmt, stmt.clone());
    }

    #[test]
    fn sort_direction_copy_equality() {
        let asc = SortDirection::Asc;
        let copied = asc;
        assert_eq!(asc, copied);
        assert_ne!(SortDirection::Asc, SortDirection::Desc);
    }

    // ===== DropIndexStatement =====

    #[test]
    fn drop_index_basic() {
        let stmt = DropIndexStatement {
            span: Span::new(0, 20),
            if_exists: false,
            name: ident("idx_name"),
            table: Some(qualified("users")),
        };
        assert_eq!(stmt.name.value(), "idx_name");
        assert!(stmt.table.is_some());
    }

    #[test]
    fn drop_index_if_exists_no_table() {
        let stmt = DropIndexStatement {
            span: Span::new(0, 15),
            if_exists: true,
            name: ident("idx"),
            table: None,
        };
        assert!(stmt.if_exists);
        assert!(stmt.table.is_none());
    }

    #[test]
    fn drop_index_clone_equality() {
        let stmt = DropIndexStatement {
            span: Span::new(0, 10),
            if_exists: false,
            name: ident("i"),
            table: Some(qualified("t")),
        };
        assert_eq!(stmt, stmt.clone());
    }

    // ===== Debug output spot checks =====

    #[test]
    fn debug_output_create_table() {
        let stmt = CreateTableStatement {
            span: Span::new(0, 1),
            if_not_exists: false,
            temporary: false,
            name: qualified("t"),
            columns: vec![],
            constraints: vec![],
            options: TableOptions::default(),
        };
        let s = format!("{stmt:?}");
        assert!(s.contains("CreateTableStatement"));
    }

    #[test]
    fn debug_output_alter_table_action_variants() {
        let a = AlterTableAction::DropColumn(ident("c"));
        let s = format!("{a:?}");
        assert!(s.contains("DropColumn"));
    }
}
