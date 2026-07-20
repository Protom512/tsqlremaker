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

use crate::dialect::Dialect;
use crate::diff::{ColumnChange, ColumnDiff, ConstraintDiff, IndexDiff, SchemaDiff, TableDiff};
use crate::warning::MigrationWarning;

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
// §0.4  to_statements_for_dialect (T10)
// ===========================================================================

/// SQLite でネイティブ非サポートの `AlterTableAction` を判定する
/// (design §0.4)。`AlterColumn` (型変更) と `DropConstraint` は SQLite が
/// サポートしないため、呼び出し側で警告化して SQL から除外する。
///
/// `AddColumn` / `DropColumn` / `AddConstraint` / `RenameTo` はサポート外ではない。
fn is_unsupported_in_sqlite(action: &AlterTableAction) -> bool {
    matches!(
        action,
        AlterTableAction::AlterColumn { .. } | AlterTableAction::DropConstraint(_)
    )
}

/// 非サポートアクションの `MigrationWarning::operation` 文字列を構築する。
///
/// design §0.4 "possible-range SQL only" — 警告は非サポート操作の種別を
/// 人間可読に伝える。`AlterColumn` は "ALTER COLUMN" を含む必要がある
/// (UC-1 が `"ALTER COLUMN"` 部分文字列を検証するため)。
fn unsupported_operation_description(action: &AlterTableAction) -> String {
    match action {
        AlterTableAction::AlterColumn { column, .. } => {
            format!("ALTER COLUMN type change on {}", column.value())
        }
        AlterTableAction::DropConstraint(name) => {
            format!("DROP CONSTRAINT {name}")
        }
        // 呼び出し側 (`to_statements_for_dialect`) は SQLite で非サポートの
        // アクションのみこの関数に渡すため、ここには到達しない。到達した場合は
        // 呼び出し側のバグであり、空文字列ではなく具体的なアクション名を返して
        // 診断可能にする (panic は workspace lint で禁止)。
        other => format!("unsupported action: {other:?}"),
    }
}

/// `AlterOperation` 列を指定方言向けに `common_sql::ast::Statement` 列に変換する
/// (design §0.4 / tasks.md Task 10.1)。
///
/// SQLite 以外の方言では [`to_statements`] と同一の 1:1 変換を行い、
/// 警告は発生しない (parity)。SQLite の場合は design §0.4 に基づき
/// per-action partition を行う:
///
/// - `AlterTable` アクション列のうち `AlterColumn` / `DropConstraint` は
///   `MigrationWarning::UnsupportedDialect` を生成して SQL から除外する。
/// - 同一 `AlterTable` 内の `AddColumn` 等のサポートされるアクションは
///   残し、`AlterTable` 全体を破棄しない ("possible-range SQL only")。
/// - アクション列が partition 後に空になった場合のみ、その `AlterTable`
///   statement 自体を生成しない。
///
/// 戻り値は `(statements, warnings)`。`warnings` は `SchemaDiff.warnings`
/// (破壊的変更等) とは独立して、方言起因の非サポート警告のみを含む。
/// 呼び出し側は両方を STDERR に出力すること (design §5 / T11)。
#[must_use]
pub fn to_statements_for_dialect(
    ops: &[AlterOperation],
    dialect: Dialect,
) -> (Vec<Statement>, Vec<MigrationWarning>) {
    if dialect != Dialect::Sqlite {
        // mysql / postgresql: 全アクションネイティブサポート → parity。
        return (to_statements(ops), Vec::new());
    }

    let mut statements = Vec::with_capacity(ops.len());
    let mut warnings = Vec::new();
    let dialect_str = dialect.as_kebab();

    for op in ops {
        match op {
            AlterOperation::AlterTable { name, actions } => {
                let mut kept = Vec::with_capacity(actions.len());
                for action in actions {
                    if is_unsupported_in_sqlite(action) {
                        warnings.push(MigrationWarning::unsupported_dialect(
                            dialect_str,
                            unsupported_operation_description(action),
                        ));
                    } else {
                        kept.push(action.clone());
                    }
                }
                // per-action partition: サポートされるアクションが1つでも残れば
                // ALTER TABLE 文を生成 (design §0.4)。全アクションが非サポートの
                // 場合のみ statement をスキップする (空 actions の ALTER TABLE は
                // 無意味であり生成しない)。
                if !kept.is_empty() {
                    statements.push(Statement::AlterTable(Box::new(AlterTableStatement {
                        span: Span::default(),
                        name: QualifiedName::new(None, name.clone()),
                        actions: kept,
                    })));
                }
            }
            // CREATE/DROP TABLE, CREATE/DROP INDEX は SQLite でもネイティブ
            // サポートのため 1:1 変換 (警告なし)。
            other => statements.push(other.to_statement()),
        }
    }

    (statements, warnings)
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

    // ========================================================================
    // T10: to_statements_for_dialect — SQLite ALTER handling (design §0.4)
    // ========================================================================
    //
    // tasks.md Task 10.1 UC-1/UC-2/UC-3 + edge + parity. These tests exercise
    // the per-action partition: SQLite strips `AlterColumn` / `DropConstraint`
    // and warns, while keeping `AddColumn` (and the parent ALTER TABLE when at
    // least one action survives). mysql/postgresql emit everything unchanged.

    use crate::dialect::Dialect as LibDialect;
    use common_sql::ast::{Expression, Literal, TableConstraint};

    /// Builds an `AlterOperation::AlterTable` with a single `AlterColumn`
    /// (type-change) action — the SQLite-unsupported case (UC-1).
    fn alter_column_type_op(table: &str, col: &str) -> AlterOperation {
        AlterOperation::AlterTable {
            name: table.to_string(),
            actions: vec![AlterTableAction::AlterColumn {
                column: Identifier::new(col.to_string()),
                data_type: Some(DataType::VarChar { length: Some(100) }),
                default: None,
                nullable: None,
            }],
        }
    }

    /// Builds an `AlterOperation::AlterTable` with a single `AddColumn`
    /// action — supported by SQLite (UC-3).
    fn add_column_op(table: &str, col: &str) -> AlterOperation {
        AlterOperation::AlterTable {
            name: table.to_string(),
            actions: vec![AlterTableAction::AddColumn(ColumnDef {
                span: Span::default(),
                name: Identifier::new(col.to_string()),
                data_type: DataType::VarChar { length: Some(255) },
                nullable: true,
                default: None,
                constraints: vec![],
            })],
        }
    }

    /// Builds an `AlterOperation::AlterTable` with a single `DropConstraint`
    /// action — SQLite-unsupported (UC-2). `DropConstraint` is produced by
    /// `plan_operations` when `ConstraintDiff::Removed` is present.
    fn drop_constraint_op(table: &str, constraint: &str) -> AlterOperation {
        AlterOperation::AlterTable {
            name: table.to_string(),
            actions: vec![AlterTableAction::DropConstraint(constraint.to_string())],
        }
    }

    /// Extracts a single `AlterTable` statement from `stmts`, asserting it is
    /// the only one present (used in the per-action partition tests).
    fn sole_alter_table(stmts: &[Statement]) -> &common_sql::ast::AlterTableStatement {
        let alters: Vec<_> = stmts
            .iter()
            .filter_map(|s| match s {
                Statement::AlterTable(inner) => Some(inner.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(
            alters.len(),
            1,
            "expected exactly one ALTER TABLE, got {alters:?}"
        );
        alters[0]
    }

    // ---- UC-1: SQLite ALTER COLUMN type change → 1 warning + stripped ----

    #[test]
    fn sqlite_alter_column_type_change_is_warned_and_stripped() {
        let ops = [alter_column_type_op("users", "email")];
        let (stmts, warnings) = to_statements_for_dialect(&ops, LibDialect::Sqlite);

        // Unsupported action is stripped — no ALTER TABLE statement survives
        // (it was the only action; partition leaves an empty action list and
        // design §0.4 says we do not emit an empty ALTER TABLE).
        assert!(
            stmts.is_empty(),
            "SQLite must strip AlterColumn; got statements: {stmts:?}"
        );

        // Exactly one UnsupportedDialect warning naming sqlite + ALTER COLUMN.
        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            MigrationWarning::UnsupportedDialect { dialect, operation } => {
                assert_eq!(dialect, "sqlite");
                assert!(
                    operation.contains("ALTER COLUMN"),
                    "operation must mention ALTER COLUMN, got: {operation}"
                );
            }
            other => panic!("expected UnsupportedDialect, got {other:?}"),
        }
    }

    // ---- UC-2: SQLite DROP CONSTRAINT → warning + stripped ----

    #[test]
    fn sqlite_drop_constraint_is_warned_and_stripped() {
        let ops = [drop_constraint_op("users", "pk_users")];
        let (stmts, warnings) = to_statements_for_dialect(&ops, LibDialect::Sqlite);

        assert!(
            stmts.is_empty(),
            "SQLite must strip DropConstraint; got statements: {stmts:?}"
        );
        assert_eq!(warnings.len(), 1);
        match &warnings[0] {
            MigrationWarning::UnsupportedDialect { dialect, operation } => {
                assert_eq!(dialect, "sqlite");
                assert!(
                    operation.contains("DROP CONSTRAINT"),
                    "operation must mention DROP CONSTRAINT, got: {operation}"
                );
                assert!(
                    operation.contains("pk_users"),
                    "operation must name the constraint, got: {operation}"
                );
            }
            other => panic!("expected UnsupportedDialect, got {other:?}"),
        }
    }

    // ---- UC-3: SQLite ADD COLUMN → no warning + AddColumn survives ----

    #[test]
    fn sqlite_add_column_passes_through_unchanged() {
        let ops = [add_column_op("users", "email")];
        let (stmts, warnings) = to_statements_for_dialect(&ops, LibDialect::Sqlite);

        assert!(
            warnings.is_empty(),
            "AddColumn is supported; got warnings: {warnings:?}"
        );
        assert_eq!(stmts.len(), 1);
        let alter = sole_alter_table(&stmts);
        assert_eq!(alter.name.name(), "users");
        assert_eq!(alter.actions.len(), 1);
        assert!(
            matches!(&alter.actions[0], AlterTableAction::AddColumn(_)),
            "AddColumn must survive partition; got {:?}",
            alter.actions[0]
        );
    }

    // ---- parity: mysql/postgresql emit everything unchanged, no warnings ----

    #[test]
    fn mysql_and_postgresql_emit_unsupported_actions_without_warnings() {
        let ops = [
            alter_column_type_op("users", "email"),
            drop_constraint_op("users", "pk_users"),
            add_column_op("users", "name"),
        ];
        for dialect in LibDialect::all() {
            if dialect == LibDialect::Sqlite {
                continue;
            }
            let (stmts, warnings) = to_statements_for_dialect(&ops, dialect);
            assert!(
                warnings.is_empty(),
                "{dialect:?} must not produce warnings; got {warnings:?}"
            );
            // 3 ops → 3 statements (1:1, no partition).
            assert_eq!(stmts.len(), 3, "{dialect:?} parity: 3 ops → 3 stmts");
        }
    }

    // ---- edge: SQLite [AddColumn, AlterColumn] mix → AddColumn survives, ----
    // ---- AlterColumn stripped+warned, single ALTER TABLE remains      ----

    #[test]
    fn sqlite_mixed_add_and_alter_partitions_per_action() {
        let ops = [AlterOperation::AlterTable {
            name: "users".to_string(),
            actions: vec![
                AlterTableAction::AddColumn(ColumnDef {
                    span: Span::default(),
                    name: Identifier::new("email".to_string()),
                    data_type: DataType::VarChar { length: Some(255) },
                    nullable: true,
                    default: None,
                    constraints: vec![],
                }),
                AlterTableAction::AlterColumn {
                    column: Identifier::new("name".to_string()),
                    data_type: Some(DataType::VarChar { length: Some(50) }),
                    default: None,
                    nullable: None,
                },
            ],
        }];
        let (stmts, warnings) = to_statements_for_dialect(&ops, LibDialect::Sqlite);

        // Exactly one warning (AlterColumn), AddColumn is NOT warned.
        assert_eq!(
            warnings.len(),
            1,
            "only AlterColumn warns; got {warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .all(|w| matches!(w, MigrationWarning::UnsupportedDialect { .. })),
            "warnings must all be UnsupportedDialect"
        );

        // A single ALTER TABLE survives with only AddColumn inside.
        assert_eq!(stmts.len(), 1, "one ALTER TABLE must remain; got {stmts:?}");
        let alter = sole_alter_table(&stmts);
        assert_eq!(alter.name.name(), "users");
        assert_eq!(
            alter.actions.len(),
            1,
            "only AddColumn must survive partition"
        );
        assert!(
            matches!(&alter.actions[0], AlterTableAction::AddColumn(_)),
            "surviving action must be AddColumn; got {:?}",
            alter.actions[0]
        );
    }

    // ---- parity helper: diff-schema-driven end-to-end for mysql ----
    //
    // Guards that a real `ColumnDiff::Modified` flow (not just a hand-built
    // AlterColumn op) still produces zero warnings under mysql. This is the
    // regression net for the "mysql should not be affected by T10" contract.

    #[test]
    fn mysql_real_type_change_diff_produces_no_dialect_warnings() {
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
        let (stmts, warnings) = to_statements_for_dialect(&ops, LibDialect::Mysql);
        assert!(
            warnings.is_empty(),
            "mysql parity: no warnings; got {warnings:?}"
        );
        assert_eq!(stmts.len(), 1, "mysql parity: one ALTER TABLE");
    }

    // ---- regression: a default-change-only AlterColumn is still stripped ----
    //
    // design §0.4 strips *any* `AlterColumn` variant — not just type changes.
    // A default-only or nullability-only AlterColumn is equally unsupported by
    // SQLite's native ALTER TABLE, so the partition must treat it identically.

    #[test]
    fn sqlite_alter_column_default_change_is_also_stripped() {
        let ops = [AlterOperation::AlterTable {
            name: "t".to_string(),
            actions: vec![AlterTableAction::AlterColumn {
                column: Identifier::new("status".to_string()),
                data_type: None,
                default: Some(Some(Expression::Literal(Literal::Integer(0)))),
                nullable: None,
            }],
        }];
        let (stmts, warnings) = to_statements_for_dialect(&ops, LibDialect::Sqlite);
        assert!(
            stmts.is_empty(),
            "SQLite strips default-only AlterColumn too"
        );
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, MigrationWarning::UnsupportedDialect { dialect, .. } if dialect == "sqlite")),
            "must warn for default-only AlterColumn under sqlite"
        );
    }

    // ---- regression: SQLite-irrelevant ops (CREATE/DROP TABLE/INDEX) pass ----

    #[test]
    fn sqlite_create_table_and_drop_index_pass_through() {
        let ops = vec![
            AlterOperation::CreateTable(crate::mapper::catalog_to_create_table(&table(
                "t",
                vec![col("id", DataType::Int)],
            ))),
            AlterOperation::DropIndex {
                name: "idx_x".to_string(),
                table: Some("t".to_string()),
            },
        ];
        let (stmts, warnings) = to_statements_for_dialect(&ops, LibDialect::Sqlite);
        assert!(
            warnings.is_empty(),
            "CREATE/DROP are SQLite-native; got {warnings:?}"
        );
        assert_eq!(stmts.len(), 2);
        assert!(matches!(stmts[0], Statement::CreateTable(_)));
        assert!(matches!(stmts[1], Statement::DropIndex(_)));
    }

    // ---- T10 internal helpers: is_unsupported_in_sqlite truth table ----

    #[test]
    fn is_unsupported_in_sqlite_classifies_each_action_correctly() {
        // Unsupported.
        assert!(is_unsupported_in_sqlite(&AlterTableAction::AlterColumn {
            column: Identifier::new("c".to_string()),
            data_type: None,
            default: None,
            nullable: None,
        }));
        assert!(is_unsupported_in_sqlite(&AlterTableAction::DropConstraint(
            "pk".to_string()
        )));

        // Supported.
        assert!(!is_unsupported_in_sqlite(&AlterTableAction::AddColumn(
            ColumnDef {
                span: Span::default(),
                name: Identifier::new("c".to_string()),
                data_type: DataType::Int,
                nullable: true,
                default: None,
                constraints: vec![],
            }
        )));
        assert!(!is_unsupported_in_sqlite(&AlterTableAction::DropColumn(
            Identifier::new("c".to_string())
        )));
        assert!(!is_unsupported_in_sqlite(&AlterTableAction::AddConstraint(
            TableConstraint::Unique {
                name: None,
                columns: vec![Identifier::new("c".to_string())],
            }
        )));
        assert!(!is_unsupported_in_sqlite(&AlterTableAction::RenameTo(
            QualifiedName::new(None, "t2".to_string())
        )));
    }

    // ---- TableConstraint import anchor (UC-2 helper uses named constraint) ----

    #[test]
    fn _table_constraint_variant_accessible() {
        let _c = TableConstraint::PrimaryKey {
            name: Some("pk_t".to_string()),
            columns: vec![Identifier::new("id".to_string())],
        };
    }
}
