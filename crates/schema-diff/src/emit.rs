//! `AlterOperation` and the `plan_operations` / `to_statements` transforms
//! (design §4 / §5).
//!
//! [`AlterOperation`] is the dialect-neutral IR between a [`crate::diff::SchemaDiff`]
//! and the `common_sql::ast::Statement`s that each emitter (T3/T4/T5) consumes.
//! [`plan_operations`] lifts a diff into this IR; [`to_statements`] projects
//! the IR 1:1 onto `common_sql::ast::Statement`.

use common_sql::ast::{
    self, AlterTableAction, AlterTableStatement, ColumnDef, CreateIndexStatement,
    CreateTableStatement, DropIndexStatement, DropTableStatement, Identifier, QualifiedName, Span,
    Statement,
};

use crate::diff::{ColumnChange, ColumnDiff, ConstraintDiff, IndexDiff, SchemaDiff, TableDiff};

// ===========================================================================
// §4.1  AlterOperation
// ===========================================================================

/// `SchemaDiff` を方言非依存の DDL 操作列に変換した中間表現 (design §4.1)。
///
/// 各 variant は [`to_statement`] で `common_sql::ast::Statement` に 1:1 変換される。
#[derive(Debug, Clone, PartialEq)]
pub enum AlterOperation {
    /// CREATE TABLE (`TableDiff::Added` に対応)。
    CreateTable(CreateTableStatement),
    /// DROP TABLE (`TableDiff::Removed` に対応)。
    DropTable {
        /// テーブル名。
        name: String,
    },
    /// ALTER TABLE (`ColumnDiff`/`ConstraintDiff` をアクション列に束ねたもの)。
    AlterTable {
        /// テーブル名。
        name: String,
        /// 適用するアクション列 (ソース順)。
        actions: Vec<AlterTableAction>,
    },
    /// CREATE INDEX (`IndexDiff::Added` に対応)。
    CreateIndex(CreateIndexStatement),
    /// DROP INDEX (`IndexDiff::Removed` に対応)。
    DropIndex {
        /// インデックス名。
        name: String,
        /// 対象テーブル名 (方言によっては省略可)。
        table: Option<String>,
    },
}

impl AlterOperation {
    /// Convert this `AlterOperation` into the equivalent
    /// `common_sql::ast::Statement` (design §4.1 1:1 mapping).
    ///
    /// `DropTable.name` and `AlterTable.name` become unqualified
    /// `QualifiedName`s (schema `None`); `DropIndex` carries the table
    /// qualifier through when present.
    #[must_use]
    pub fn to_statement(&self) -> Statement {
        match self {
            Self::CreateTable(create) => Statement::CreateTable(Box::new(create.clone())),
            Self::DropTable { name } => Statement::DropTable(Box::new(DropTableStatement {
                span: Span::default(),
                if_exists: false,
                names: vec![QualifiedName::new(None, name.clone())],
            })),
            Self::AlterTable { name, actions } => {
                Statement::AlterTable(Box::new(AlterTableStatement {
                    span: Span::default(),
                    name: QualifiedName::new(None, name.clone()),
                    actions: actions.clone(),
                }))
            }
            Self::CreateIndex(create) => Statement::CreateIndex(Box::new(create.clone())),
            Self::DropIndex { name, table } => Statement::DropIndex(Box::new(DropIndexStatement {
                span: Span::default(),
                if_exists: false,
                name: Identifier::new(name.clone()),
                table: table.as_ref().map(|t| QualifiedName::new(None, t.clone())),
            })),
        }
    }
}

// ===========================================================================
// §5  plan_operations
// ===========================================================================

/// `SchemaDiff` を方言非依外の `AlterOperation` 列に変換する (design §5)。
///
/// 警告 (`MigrationWarning::Destructive` 等) は `SchemaDiff.warnings` が
/// 保持するため `AlterOperation` には載せない。呼び出し側は `SchemaDiff`
/// を手元に置いたまま警告を参照できる。並びは `SchemaDiff` 内の順序
/// (テーブル名順 → カラム名順) を維持する。`TableDiff::Unchanged` は
/// 操作を生成しない。
#[must_use]
pub fn plan_operations(diff: &SchemaDiff) -> Vec<AlterOperation> {
    let mut ops = Vec::new();

    for table in &diff.table_diffs {
        match table {
            TableDiff::Added(t) => {
                ops.push(AlterOperation::CreateTable(
                    crate::mapper::catalog_to_create_table(t),
                ));
            }
            TableDiff::Removed { name } => {
                ops.push(AlterOperation::DropTable { name: name.clone() });
            }
            TableDiff::Modified {
                name,
                column_diffs,
                constraint_diffs,
            } => {
                let actions = build_alter_actions(column_diffs, constraint_diffs);
                if !actions.is_empty() {
                    ops.push(AlterOperation::AlterTable {
                        name: name.clone(),
                        actions,
                    });
                }
            }
            TableDiff::Unchanged { .. } => {}
        }
    }

    for index in &diff.index_diffs {
        match index {
            IndexDiff::Added(stmt) | IndexDiff::Modified(stmt) => {
                ops.push(AlterOperation::CreateIndex(stmt.clone()));
            }
            IndexDiff::Removed { name, table } => {
                ops.push(AlterOperation::DropIndex {
                    name: name.clone(),
                    table: Some(table.clone()),
                });
            }
        }
    }

    ops
}

/// Flatten a table's `ColumnDiff` + `ConstraintDiff` lists into a single
/// ordered `AlterTableAction` list.
///
/// Order: columns first (sorted by ColumnDiff order — already name-sorted in
/// `diff_schema`), then constraints. This deterministic order keeps generated
/// SQL stable across runs.
fn build_alter_actions(
    column_diffs: &[ColumnDiff],
    constraint_diffs: &[ConstraintDiff],
) -> Vec<AlterTableAction> {
    let mut actions = Vec::new();

    for col in column_diffs {
        match col {
            ColumnDiff::Added(def) => {
                actions.push(AlterTableAction::AddColumn(def.clone()));
            }
            ColumnDiff::Removed { name } => {
                actions.push(AlterTableAction::DropColumn(Identifier::new(name.clone())));
            }
            ColumnDiff::Modified {
                name, to, changes, ..
            } => {
                actions.extend(modified_column_actions(name, to, changes));
            }
        }
    }

    for c in constraint_diffs {
        match c {
            ConstraintDiff::Added { constraint, .. } => {
                actions.push(AlterTableAction::AddConstraint(constraint.clone()));
            }
            ConstraintDiff::Removed { name } => {
                actions.push(AlterTableAction::DropConstraint(name.clone()));
            }
            ConstraintDiff::Modified {
                name,
                new_constraint,
            } => {
                // DROP + ADD pair (design §2.5: "DROP + ADD のペア").
                actions.push(AlterTableAction::DropConstraint(name.clone()));
                actions.push(AlterTableAction::AddConstraint(new_constraint.clone()));
            }
        }
    }

    actions
}

/// Project a `ColumnDiff::Modified` into one or more `AlterTableAction`s.
///
/// The single-AST `AlterColumn` action carries all three deltas (type,
/// default, nullability) at once, so we emit exactly one `AlterColumn`
/// when any change exists. (`common-sql`'s `AlterColumn` models this
/// compound shape — design §8 verified.)
fn modified_column_actions(
    name: &str,
    to: &ColumnDef,
    changes: &[ColumnChange],
) -> Vec<AlterTableAction> {
    if changes.is_empty() {
        return Vec::new();
    }
    let mut data_type = None;
    let mut nullable: Option<bool> = None;
    let mut default: Option<Option<ast::Expression>> = None;
    for change in changes {
        match change {
            ColumnChange::TypeChanged { to: dt, .. } => data_type = Some(dt.clone()),
            ColumnChange::NullabilityChanged { to: n, .. } => nullable = Some(*n),
            ColumnChange::DefaultChanged { to: d, .. } => {
                default = Some(d.clone());
            }
        }
    }
    // If nullability was unchanged, fall back to the `to` column's nullability
    // so the emitted ALTER COLUMN carries a coherent target shape.
    let nullable = nullable.unwrap_or(to.nullable);
    vec![AlterTableAction::AlterColumn {
        column: Identifier::new(name.to_string()),
        data_type,
        default,
        nullable: Some(nullable),
    }]
}

// ===========================================================================
// §5  to_statements
// ===========================================================================

/// `AlterOperation` 列を `common_sql::ast::Statement` 列に変換する (design §5)。
///
/// 各 `AlterOperation` は [`AlterOperation::to_statement`] で 1:1 変換される。
#[must_use]
pub fn to_statements(ops: &[AlterOperation]) -> Vec<Statement> {
    ops.iter().map(AlterOperation::to_statement).collect()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::catalog::{CatalogColumn, CatalogIndex, CatalogSchema, CatalogTable};
    use crate::diff::{diff_schema, ColumnChange};
    use common_sql::ast::{ColumnConstraint, DataType, Identifier, IndexColumn, Statement};

    // ---- helpers ----

    fn col(name: &str, dt: DataType) -> CatalogColumn {
        CatalogColumn {
            name: name.to_string(),
            data_type: dt,
            nullable: true,
            default: None,
            raw_default: None,
            identity: false,
            constraints: vec![],
        }
    }

    fn table(name: &str, columns: Vec<CatalogColumn>) -> CatalogTable {
        CatalogTable {
            name: name.to_string(),
            columns,
            constraints: vec![],
        }
    }

    // ===== plan_operations: TableDiff::Added -> CreateTable =====

    #[test]
    fn plan_added_table_yields_create_table() {
        let current = CatalogSchema::default();
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::BigInt)])],
            indices: vec![],
        };
        let diff = diff_schema(&current, &desired);
        let ops = plan_operations(&diff);
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            AlterOperation::CreateTable(c) => assert_eq!(c.name.name(), "users"),
            other => panic!("expected CreateTable, got {other:?}"),
        }
    }

    // ===== plan_operations: TableDiff::Removed -> DropTable =====

    #[test]
    fn plan_removed_table_yields_drop_table() {
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("legacy", vec![col("id", DataType::Int)])],
            indices: vec![],
        };
        let desired = CatalogSchema::default();
        let diff = diff_schema(&current, &desired);
        let ops = plan_operations(&diff);
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            AlterOperation::DropTable { name } => assert_eq!(name, "legacy"),
            other => panic!("expected DropTable, got {other:?}"),
        }
    }

    // ===== plan_operations: ColumnDiff::Added -> AlterTable AddColumn =====

    #[test]
    fn plan_added_column_yields_alter_add_column() {
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::BigInt)])],
            indices: vec![],
        };
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table(
                "users",
                vec![
                    col("id", DataType::BigInt),
                    col("email", DataType::VarChar { length: Some(255) }),
                ],
            )],
            indices: vec![],
        };
        let diff = diff_schema(&current, &desired);
        let ops = plan_operations(&diff);
        assert_eq!(ops.len(), 1);
        let AlterOperation::AlterTable { name, actions } = &ops[0] else {
            panic!("expected AlterTable");
        };
        assert_eq!(name, "users");
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AlterTableAction::AddColumn(_)));
    }

    // ===== plan_operations: ColumnDiff::Modified -> AlterColumn =====

    #[test]
    fn plan_modified_column_yields_alter_column() {
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table(
                "users",
                vec![col("email", DataType::VarChar { length: Some(255) })],
            )],
            indices: vec![],
        };
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table(
                "users",
                vec![col("email", DataType::VarChar { length: Some(50) })],
            )],
            indices: vec![],
        };
        let diff = diff_schema(&current, &desired);
        let ops = plan_operations(&diff);
        let AlterOperation::AlterTable { actions, .. } = &ops[0] else {
            panic!("expected AlterTable");
        };
        assert!(matches!(actions[0], AlterTableAction::AlterColumn { .. }));
    }

    // ===== plan_operations: IndexDiff =====

    #[test]
    fn plan_index_added_and_removed() {
        let idx_added = CatalogIndex {
            name: "idx_new".to_string(),
            table: "users".to_string(),
            columns: vec![IndexColumn {
                name: Identifier::new("id".to_string()),
                direction: None,
            }],
            unique: false,
        };
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::Int)])],
            indices: vec![],
        };
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::Int)])],
            indices: vec![idx_added],
        };
        let diff = diff_schema(&current, &desired);
        let ops = plan_operations(&diff);
        assert!(ops
            .iter()
            .any(|o| matches!(o, AlterOperation::CreateIndex(s) if s.name.value() == "idx_new")));

        // Reverse: DROP INDEX.
        let diff_back = diff_schema(&desired, &current);
        let ops_back = plan_operations(&diff_back);
        assert!(ops_back.iter().any(|o| matches!(
            o,
            AlterOperation::DropIndex { name, .. } if name == "idx_new"
        )));
    }

    // ===== to_statements: 1:1 projection =====

    #[test]
    fn to_statements_projects_each_op() {
        let ops = vec![
            AlterOperation::CreateTable(crate::mapper::catalog_to_create_table(&table(
                "t",
                vec![col("id", DataType::Int)],
            ))),
            AlterOperation::DropTable {
                name: "old".to_string(),
            },
        ];
        let stmts = to_statements(&ops);
        assert_eq!(stmts.len(), 2);
        assert!(matches!(stmts[0], Statement::CreateTable(_)));
        assert!(matches!(stmts[1], Statement::DropTable(_)));
    }

    // ===== to_statement: AlterTable and CreateIndex shapes =====

    #[test]
    fn to_statement_alter_table_shape() {
        let op = AlterOperation::AlterTable {
            name: "users".to_string(),
            actions: vec![AlterTableAction::AddColumn(ColumnDef {
                span: Span::default(),
                name: Identifier::new("email".to_string()),
                data_type: DataType::VarChar { length: Some(255) },
                nullable: true,
                default: None,
                constraints: vec![],
            })],
        };
        let stmt = op.to_statement();
        if let Statement::AlterTable(inner) = stmt {
            assert_eq!(inner.name.name(), "users");
            assert_eq!(inner.actions.len(), 1);
        } else {
            panic!("expected AlterTable");
        }
    }

    #[test]
    fn to_statement_drop_index_with_table() {
        let op = AlterOperation::DropIndex {
            name: "idx_x".to_string(),
            table: Some("users".to_string()),
        };
        let stmt = op.to_statement();
        if let Statement::DropIndex(inner) = stmt {
            assert_eq!(inner.name.value(), "idx_x");
            assert_eq!(inner.table.as_ref().map(|t| t.name()), Some("users"));
        } else {
            panic!("expected DropIndex");
        }
    }

    // ===== Unchanged does not produce ops =====

    #[test]
    fn unchanged_table_produces_no_op() {
        let schema = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::Int)])],
            indices: vec![],
        };
        let diff = diff_schema(&schema, &schema);
        let ops = plan_operations(&diff);
        assert!(ops.is_empty(), "Unchanged must not emit ops, got {ops:?}");
    }

    // ===== Default change survives into AlterColumn.default =====

    #[test]
    fn default_change_projects_into_alter_column() {
        use common_sql::ast::{Expression, Literal};
        let cur = col("status", DataType::Int);
        let mut des = col("status", DataType::Int);
        des.default = Some(Expression::Literal(Literal::Integer(0)));
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("t", vec![cur])],
            indices: vec![],
        };
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("t", vec![des])],
            indices: vec![],
        };
        let diff = diff_schema(&current, &desired);
        let ops = plan_operations(&diff);
        let AlterOperation::AlterTable { actions, .. } = &ops[0] else {
            panic!("expected AlterTable");
        };
        let AlterTableAction::AlterColumn { default, .. } = &actions[0] else {
            panic!("expected AlterColumn");
        };
        assert!(default.is_some(), "default delta must be carried");
    }

    // ===== _column_change_anchor: silence unused import if no test hits it =====

    #[test]
    fn _column_change_variant_is_accessible() {
        let _c = ColumnChange::DefaultChanged {
            from: None,
            to: None,
        };
    }

    // ===== ColumnConstraint sanity (guards against accidental shadowing) =====

    #[test]
    fn _column_constraint_variant_accessible() {
        let _c = ColumnConstraint::PrimaryKey;
    }
}
