//! Diff data model and `diff_schema` pure function (design §2 / §5).
//!
//! `SchemaDiff` / `TableDiff` / `ColumnDiff` / `ColumnChange` / `IndexDiff` /
//! `ConstraintDiff` model the difference between a *current* catalog and a
//! *desired* schema. [`diff_schema`] is the pure function (design §5 AC-4)
//! that derives the diff from two [`crate::catalog::CatalogSchema`]s.
//!
//! ## Variant shape
//!
//! Per the T7 task contract, the diff enums carry catalog-level or
//! common-sql AST values directly (e.g. `TableDiff::Added(CatalogTable)`),
//! rather than wrapping them in intermediate structs. This keeps the public
//! surface small and matches the mapper round-trip (crate::mapper) already
//! established in T6. Design §2 fields are reflected faithfully — only the
//! enum ergonomics differ.

use common_sql::ast::{
    ColumnConstraint, ColumnDef, CreateIndexStatement, DataType, Expression, Identifier,
    TableConstraint,
};

use crate::catalog::{CatalogColumn, CatalogError, CatalogIndex, CatalogSchema, CatalogTable};
use crate::mapper::{catalog_to_create_index, create_table_to_catalog};
use crate::warning::MigrationWarning;

// ===========================================================================
// §2  Diff data model
// ===========================================================================

/// desired と current のスキーマ全体の差分 (design §2.1)。
///
/// 全ての `Vec` フィールドは決定的な順序 (名前順) でソート済み。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SchemaDiff {
    /// テーブル単位の差分 (テーブル名でソート済み、決定的順序)。
    pub table_diffs: Vec<TableDiff>,
    /// インデックス単位の差分 (テーブル名・インデックス名でソート済み)。
    pub index_diffs: Vec<IndexDiff>,
    /// 差分導出過程で発生した警告 (破壊的変更 / 非対応方言 等)。
    pub warnings: Vec<MigrationWarning>,
}

/// 1テーブル単位の差分 (design §2.2)。
///
/// Variant shape follows the T7 contract: `Added`/`Modified` carry catalog
/// values directly. `Unchanged` is emitted for parity (deterministic output)
/// but does not produce an `AlterOperation`.
#[derive(Debug, Clone, PartialEq)]
pub enum TableDiff {
    /// desired にのみ存在 (current 側に CREATE 必要)。
    Added(CatalogTable),
    /// current にのみ存在 (DROP 必要 → 破壊的警告対象)。
    Removed {
        /// テーブル名。
        name: String,
    },
    /// 両方に存在し、内容が異なる (ALTER 系操作のリスト)。
    Modified {
        /// テーブル名。
        name: String,
        /// カラム単位の差分 (カラム名でソート済み)。
        column_diffs: Vec<ColumnDiff>,
        /// 制約単位の差分 (制約正規化名でソート済み)。
        constraint_diffs: Vec<ConstraintDiff>,
    },
    /// 両方に存在し、内容が同一 (差分なし)。出力の完全性のために保持。
    Unchanged {
        /// テーブル名。
        name: String,
    },
}

/// 1カラム単位の差分 (design §2.3)。
///
/// `Modified` の `from` / `to` は `Box<ColumnDef>` で保持する
/// (`clippy::large_enum_variant` 対策 — `Modified` が2つの `ColumnDef` を
/// 持つため enum 全体のサイズが膨らむのを避ける)。
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnDiff {
    /// desired にのみ存在 (ADD COLUMN)。
    Added(ColumnDef),
    /// current にのみ存在 (DROP COLUMN → 破壊的警告対象)。
    Removed {
        /// カラム名。
        name: String,
    },
    /// 両方に存在し、型/制約が異なる (ALTER COLUMN)。
    Modified {
        /// カラム名。
        name: String,
        /// 変更前カラム定義。
        from: Box<ColumnDef>,
        /// 変更後カラム定義。
        to: Box<ColumnDef>,
        /// 検出された変更の内訳 (型変更 / nullability / default)。
        changes: Vec<ColumnChange>,
    },
}

/// ALTER COLUMN で変化した属性の内訳 (design §2.3.1)。
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnChange {
    /// データ型が変更された。
    TypeChanged {
        /// 変更前。
        from: DataType,
        /// 変更後。
        to: DataType,
        /// ナロー化 (安全でない変更) かどうか。
        is_narrowing: bool,
    },
    /// NULL許容性が変更された。
    NullabilityChanged {
        /// 変更前 (true = NULL可)。
        from: bool,
        /// 変更後。
        to: bool,
        /// NOT NULL 化 (安全でない) かどうか。
        tightens: bool,
    },
    /// DEFAULT 式が変更された。
    DefaultChanged {
        /// 変更前。
        from: Option<Expression>,
        /// 変更後。
        to: Option<Expression>,
    },
}

/// 1インデックス単位の差分 (design §2.4)。
#[derive(Debug, Clone, PartialEq)]
pub enum IndexDiff {
    /// desired にのみ存在 (CREATE INDEX)。
    Added(CreateIndexStatement),
    /// current にのみ存在 (DROP INDEX → 破壊的警告対象)。
    Removed {
        /// インデックス名。
        name: String,
        /// 対象テーブル名。
        table: String,
    },
    /// 両方に存在し定義が異なる (DROP + CREATE のペアで表現、RENAME は非スコープ)。
    Modified(CreateIndexStatement),
}

/// 1制約 (テーブルレベル) 単位の差分 (design §2.5)。
#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintDiff {
    /// desired にのみ存在 (ADD CONSTRAINT)。
    Added {
        /// 制約の正規化名 (未指定時は `<type>_<cols>` を生成)。
        name: String,
        /// 追加される制約。
        constraint: TableConstraint,
    },
    /// current にのみ存在 (DROP CONSTRAINT → 破壊的警告対象)。
    Removed {
        /// 制約名。
        name: String,
    },
    /// 両方に存在し定義が異なる (DROP + ADD のペア)。
    Modified {
        /// 制約名。
        name: String,
        /// 変更後制約。
        new_constraint: TableConstraint,
    },
}

// ===========================================================================
// §5  diff_schema  pure function
// ===========================================================================

/// desired (DDL から構築) と current (catalog) の差分を計算する純粋関数
/// (design §5 AC-4)。
///
/// # 引数順序 (CTO 条件 #1 不変量)
///
/// `diff_schema(current, desired)` — **第1引数が current (既存)、第2引数が
/// desired (目標)**。引数を入れ替えると `Added`/`Removed` が対称に入れ替わる
/// (`diff_schema(a, b)` の Added は `diff_schema(b, a)` の Removed に対応)。
///
/// # ナローイング判定 (T7.2 最小実装)
///
/// design §2.3.1 の `is_narrowing` / `tightens` は以下の3ケースのみ `true`:
/// - `VarChar { length }` の `length` 縮小 (Some → より小さい Some、None → Some も縮小)
/// - `Decimal` / `Numeric` の `precision` 低下
/// - `nullable: true → false` (NOT NULL 追加 = `tightens: true`)
///
/// 他の型変更 (拡大・整数間等) は `is_narrowing: false`。完全マトリクスは
/// design §10 の通り将来課題 (非スコープ)。
#[must_use]
pub fn diff_schema(current: &CatalogSchema, desired: &CatalogSchema) -> SchemaDiff {
    let mut warnings = Vec::new();

    let table_diffs = diff_tables(current, desired, &mut warnings);
    let index_diffs = diff_indices(current, desired, &mut warnings);

    SchemaDiff {
        table_diffs,
        index_diffs,
        warnings,
    }
}

// ===========================================================================
// Table-level diff
// ===========================================================================

fn diff_tables(
    current: &CatalogSchema,
    desired: &CatalogSchema,
    warnings: &mut Vec<MigrationWarning>,
) -> Vec<TableDiff> {
    let current_by_name: BTreeMap<&str, &CatalogTable> = current
        .tables
        .iter()
        .map(|t| (t.name.as_str(), t))
        .collect();
    let desired_by_name: BTreeMap<&str, &CatalogTable> = desired
        .tables
        .iter()
        .map(|t| (t.name.as_str(), t))
        .collect();

    let mut names: Vec<&str> = current_by_name
        .keys()
        .chain(desired_by_name.keys())
        .copied()
        .collect();
    names.sort_unstable();
    names.dedup();

    let mut diffs: Vec<TableDiff> = names
        .into_iter()
        .filter_map(|name| {
            let cur = current_by_name.get(name);
            let des = desired_by_name.get(name);
            match (cur, des) {
                (Some(c), Some(d)) => Some(diff_single_table(c, d, warnings)),
                (Some(c), None) => {
                    warnings.push(MigrationWarning::destructive(
                        c.name.clone(),
                        "DROP TABLE (destructive: data loss)".to_string(),
                    ));
                    Some(TableDiff::Removed {
                        name: c.name.clone(),
                    })
                }
                (None, Some(d)) => Some(TableDiff::Added((*d).clone())),
                (None, None) => None,
            }
        })
        .collect();

    diffs.sort_by(|a, b| table_diff_name(a).cmp(table_diff_name(b)));
    diffs
}

fn diff_single_table(
    current: &CatalogTable,
    desired: &CatalogTable,
    warnings: &mut Vec<MigrationWarning>,
) -> TableDiff {
    let column_diffs = diff_columns(current, desired, warnings);
    let constraint_diffs = diff_constraints(current, desired, warnings);

    if column_diffs.is_empty() && constraint_diffs.is_empty() {
        TableDiff::Unchanged {
            name: current.name.clone(),
        }
    } else {
        TableDiff::Modified {
            name: current.name.clone(),
            column_diffs,
            constraint_diffs,
        }
    }
}

fn table_diff_name(d: &TableDiff) -> &str {
    match d {
        TableDiff::Added(t) => &t.name,
        TableDiff::Removed { name } => name,
        TableDiff::Modified { name, .. } => name,
        TableDiff::Unchanged { name } => name,
    }
}

// ===========================================================================
// Column-level diff
// ===========================================================================

fn diff_columns(
    current: &CatalogTable,
    desired: &CatalogTable,
    warnings: &mut Vec<MigrationWarning>,
) -> Vec<ColumnDiff> {
    let cur_cols: BTreeMap<&str, &CatalogColumn> = current
        .columns
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();
    let des_cols: BTreeMap<&str, &CatalogColumn> = desired
        .columns
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    let mut names: Vec<&str> = cur_cols.keys().chain(des_cols.keys()).copied().collect();
    names.sort_unstable();
    names.dedup();

    let mut diffs: Vec<ColumnDiff> = names
        .into_iter()
        .filter_map(|name| {
            let cur = cur_cols.get(name);
            let des = des_cols.get(name);
            match (cur, des) {
                (Some(c), Some(d)) => diff_single_column(current, c, d, warnings),
                (Some(c), None) => {
                    warnings.push(MigrationWarning::destructive(
                        format!("{}.{}", current.name, c.name),
                        "DROP COLUMN (destructive: data loss)".to_string(),
                    ));
                    Some(ColumnDiff::Removed {
                        name: c.name.clone(),
                    })
                }
                (None, Some(d)) => Some(ColumnDiff::Added(catalog_column_to_column_def(d))),
                (None, None) => None,
            }
        })
        .collect();

    diffs.sort_by(|a, b| column_diff_name(a).cmp(column_diff_name(b)));
    diffs
}

fn diff_single_column(
    table: &CatalogTable,
    current: &CatalogColumn,
    desired: &CatalogColumn,
    warnings: &mut Vec<MigrationWarning>,
) -> Option<ColumnDiff> {
    let from = catalog_column_to_column_def(current);
    let to = catalog_column_to_column_def(desired);

    if from == to {
        return None;
    }

    let mut changes = Vec::new();

    if current.data_type != desired.data_type {
        let is_narrowing = is_narrowing_type_change(&current.data_type, &desired.data_type);
        if is_narrowing {
            warnings.push(MigrationWarning::destructive(
                format!("{}.{}", table.name, current.name),
                format!(
                    "type narrowing {:?} -> {:?} (destructive: may truncate data)",
                    current.data_type, desired.data_type
                ),
            ));
        }
        changes.push(ColumnChange::TypeChanged {
            from: current.data_type.clone(),
            to: desired.data_type.clone(),
            is_narrowing,
        });
    }

    if current.nullable != desired.nullable {
        let tightens = current.nullable && !desired.nullable;
        if tightens {
            warnings.push(MigrationWarning::destructive(
                format!("{}.{}", table.name, current.name),
                "NOT NULL added (destructive: existing NULLs violate constraint)".to_string(),
            ));
        }
        changes.push(ColumnChange::NullabilityChanged {
            from: current.nullable,
            to: desired.nullable,
            tightens,
        });
    }

    if current.default != desired.default {
        changes.push(ColumnChange::DefaultChanged {
            from: current.default.clone(),
            to: desired.default.clone(),
        });
    }

    if changes.is_empty() {
        None
    } else {
        Some(ColumnDiff::Modified {
            name: current.name.clone(),
            from: Box::new(from),
            to: Box::new(to),
            changes,
        })
    }
}

fn column_diff_name(d: &ColumnDiff) -> &str {
    match d {
        ColumnDiff::Added(c) => c.name.value(),
        ColumnDiff::Removed { name } => name,
        ColumnDiff::Modified { name, .. } => name,
    }
}

/// Convert a `CatalogColumn` into a common-sql `ColumnDef` (mirror of
/// [`crate::mapper::catalog_to_create_table`]'s per-column logic, kept local
/// so the diff layer does not depend on a future table-level helper).
fn catalog_column_to_column_def(col: &CatalogColumn) -> ColumnDef {
    let mut constraints = col.constraints.clone();
    if col.identity
        && !constraints
            .iter()
            .any(|c| matches!(c, ColumnConstraint::AutoIncrement))
    {
        constraints.push(ColumnConstraint::AutoIncrement);
    }
    ColumnDef {
        span: common_sql::ast::Span::default(),
        name: Identifier::new(col.name.clone()),
        data_type: col.data_type.clone(),
        nullable: col.nullable,
        default: col.default.clone(),
        constraints,
    }
}

// ===========================================================================
// Narrowing detection (T7.2 minimal — design §10: full matrix is future work)
// ===========================================================================

/// Detect whether `from -> to` is a *narrowing* (unsafe) type change.
///
/// Minimal implementation (UC-3 covers these three only):
/// - `VarChar { length }`: shrink if both `Some` and `to < from`, or
///   `from == None && to == Some(_)` (unbounded → bounded).
/// - `Decimal` / `Numeric`: shrink if both `precision` are `Some` and
///   `to < from`.
///
/// All other changes (widening, integer-to-integer, cross-category) return
/// `false`. The full matrix is design §10 non-scope.
fn is_narrowing_type_change(from: &DataType, to: &DataType) -> bool {
    match (from, to) {
        (
            DataType::VarChar {
                length: Some(from_len),
            },
            DataType::VarChar {
                length: Some(to_len),
            },
        ) => to_len < from_len,
        (DataType::VarChar { length: None }, DataType::VarChar { length: Some(_) }) => true,
        (
            DataType::Decimal {
                precision: Some(from_p),
                ..
            },
            DataType::Decimal {
                precision: Some(to_p),
                ..
            },
        ) => to_p < from_p,
        (
            DataType::Numeric {
                precision: Some(from_p),
                ..
            },
            DataType::Numeric {
                precision: Some(to_p),
                ..
            },
        ) => to_p < from_p,
        // NVarChar / Char / NChar / Binary / VarBinary narrowing and the full
        // cross-type matrix are intentionally out of scope (design §10).
        _ => false,
    }
}

// ===========================================================================
// Constraint-level diff
// ===========================================================================

fn diff_constraints(
    current: &CatalogTable,
    desired: &CatalogTable,
    warnings: &mut Vec<MigrationWarning>,
) -> Vec<ConstraintDiff> {
    let cur = index_constraints_by_name(&current.constraints);
    let des = index_constraints_by_name(&desired.constraints);

    let mut names: Vec<String> = cur.keys().chain(des.keys()).cloned().collect();
    names.sort_unstable();
    names.dedup();

    let mut diffs: Vec<ConstraintDiff> = names
        .into_iter()
        .filter_map(|name| {
            let c = cur.get(&name);
            let d = des.get(&name);
            match (c, d) {
                (Some(c), Some(d)) if c == d => None,
                (Some(_), Some(d)) => Some(ConstraintDiff::Modified {
                    name: name.clone(),
                    new_constraint: d.clone(),
                }),
                (Some(_), None) => {
                    warnings.push(MigrationWarning::destructive(
                        format!("{}.{}", current.name, name),
                        "DROP CONSTRAINT (destructive: may allow invalid data)".to_string(),
                    ));
                    Some(ConstraintDiff::Removed { name })
                }
                (None, Some(d)) => Some(ConstraintDiff::Added {
                    name,
                    constraint: d.clone(),
                }),
                (None, None) => None,
            }
        })
        .collect();

    diffs.sort_by(|a, b| constraint_diff_name(a).cmp(constraint_diff_name(b)));
    diffs
}

fn constraint_diff_name(d: &ConstraintDiff) -> &str {
    match d {
        ConstraintDiff::Added { name, .. } => name,
        ConstraintDiff::Removed { name } => name,
        ConstraintDiff::Modified { name, .. } => name,
    }
}

/// Stable key for a `TableConstraint` (design §2.5: `name` if specified,
/// otherwise a normalized `<type>_<cols>` synthetic name).
fn constraint_name(tc: &TableConstraint) -> String {
    if let Some(n) = constraint_declared_name(tc) {
        return n;
    }
    let kind = match tc {
        TableConstraint::PrimaryKey { .. } => "pk",
        TableConstraint::Unique { .. } => "uq",
        TableConstraint::ForeignKey { .. } => "fk",
        TableConstraint::Check { .. } => "ck",
    };
    let cols = constraint_columns(tc);
    if cols.is_empty() {
        kind.to_string()
    } else {
        format!("{kind}_{}", cols.join("_"))
    }
}

fn constraint_declared_name(tc: &TableConstraint) -> Option<String> {
    match tc {
        TableConstraint::PrimaryKey { name, .. }
        | TableConstraint::Unique { name, .. }
        | TableConstraint::ForeignKey { name, .. }
        | TableConstraint::Check { name, .. } => name.clone(),
    }
}

fn constraint_columns(tc: &TableConstraint) -> Vec<String> {
    match tc {
        TableConstraint::PrimaryKey { columns, .. }
        | TableConstraint::Unique { columns, .. }
        | TableConstraint::ForeignKey { columns, .. } => {
            columns.iter().map(|c| c.value().to_string()).collect()
        }
        TableConstraint::Check { .. } => Vec::new(),
    }
}

fn index_constraints_by_name(constraints: &[TableConstraint]) -> BTreeMap<String, TableConstraint> {
    let mut map = BTreeMap::new();
    for c in constraints {
        map.insert(constraint_name(c), c.clone());
    }
    map
}

// ===========================================================================
// Index-level diff
// ===========================================================================

fn diff_indices(
    current: &CatalogSchema,
    desired: &CatalogSchema,
    warnings: &mut Vec<MigrationWarning>,
) -> Vec<IndexDiff> {
    let cur = index_indices_by_key(&current.indices);
    let des = index_indices_by_key(&desired.indices);

    let mut keys: Vec<String> = cur.keys().chain(des.keys()).cloned().collect();
    keys.sort_unstable();
    keys.dedup();

    let mut diffs: Vec<IndexDiff> = keys
        .into_iter()
        .filter_map(|key| {
            let c = cur.get(&key);
            let d = des.get(&key);
            match (c, d) {
                (Some(c), Some(d)) if indices_equal(c, d) => None,
                (Some(_), Some(d)) => Some(IndexDiff::Modified(catalog_to_create_index(d))),
                (Some(c), None) => {
                    warnings.push(MigrationWarning::destructive(
                        format!("{}.{}", c.table, c.name),
                        "DROP INDEX (destructive: query performance regression)".to_string(),
                    ));
                    Some(IndexDiff::Removed {
                        name: c.name.clone(),
                        table: c.table.clone(),
                    })
                }
                (None, Some(d)) => Some(IndexDiff::Added(catalog_to_create_index(d))),
                (None, None) => None,
            }
        })
        .collect();

    diffs.sort_by_key(index_diff_key);
    diffs
}

/// Composite key (`table\0name`) so two indices with the same name on
/// different tables do not collide.
fn index_key(idx: &CatalogIndex) -> String {
    format!("{}\u{0}{}", idx.table, idx.name)
}

fn indices_equal(a: &CatalogIndex, b: &CatalogIndex) -> bool {
    a.columns == b.columns && a.unique == b.unique
}

fn index_diff_key(d: &IndexDiff) -> String {
    match d {
        IndexDiff::Added(s) | IndexDiff::Modified(s) => index_stmt_key(s),
        IndexDiff::Removed { name, table } => format!("{table}\u{0}{name}"),
    }
}

fn index_stmt_key(s: &CreateIndexStatement) -> String {
    format!("{}\u{0}{}", s.table.name(), s.name.value())
}

fn index_indices_by_key(indices: &[CatalogIndex]) -> BTreeMap<String, CatalogIndex> {
    let mut map = BTreeMap::new();
    for i in indices {
        map.insert(index_key(i), i.clone());
    }
    map
}

// ===========================================================================
// §5  build_desired_schema
// ===========================================================================

/// CREATE TABLE 系 DDL 文列をパースして desired 側 `CatalogSchema` を構築する
/// (design §5)。
///
/// 内部で tsql-parser の `parse_with_errors` + `to_common_sql` を呼び、
/// `CreateTable` / `CreateIndex` を mapper 逆変換で `CatalogSchema` に組立てる。
///
/// # Errors
///
/// DDL にパースエラーが含まれる場合 `CatalogError::ParseFailed` を返す。
pub fn build_desired_schema(ddl_source: &str) -> Result<CatalogSchema, CatalogError> {
    let (stmts, parse_errors) = tsql_parser::parse_with_errors(ddl_source);
    if !parse_errors.is_empty() {
        let messages: Vec<String> = parse_errors.iter().map(|e| e.to_string()).collect();
        return Err(CatalogError::ParseFailed {
            message: messages.join("; "),
        });
    }

    let mut tables = Vec::new();
    let mut indices = Vec::new();

    for stmt in &stmts {
        let common = tsql_parser::ast::to_common_sql::to_common_sql(stmt);
        let Some(common) = common else {
            // non-DDL / unsupported (VIEW/PROC/TRIGGER/BatchSeparator/control-flow):
            // skip silently. Desired schema only tracks TABLE + INDEX.
            continue;
        };
        match common {
            common_sql::ast::Statement::CreateTable(create) => {
                tables.push(create_table_to_catalog(&create));
            }
            common_sql::ast::Statement::CreateIndex(create_idx) => {
                indices.push(create_index_to_catalog(&create_idx));
            }
            // Other variants (DML / AlterTable / Drop* / DialectSpecific) are
            // out of scope for *desired schema construction* (design §5:
            // build_desired_schema consumes CREATE TABLE / CREATE INDEX DDL).
            _ => {}
        }
    }

    // Deterministic ordering: tables by name, indices by (table, name).
    tables.sort_by(|a, b| a.name.cmp(&b.name));
    indices.sort_by(|a, b| a.table.cmp(&b.table).then_with(|| a.name.cmp(&b.name)));

    Ok(CatalogSchema {
        schema_name: String::new(),
        tables,
        indices,
    })
}

/// Reverse of [`crate::mapper::catalog_to_create_index`]: build a
/// `CatalogIndex` from a common-sql `CreateIndexStatement`.
fn create_index_to_catalog(stmt: &CreateIndexStatement) -> CatalogIndex {
    CatalogIndex {
        name: stmt.name.value().to_string(),
        table: stmt.table.name().to_string(),
        columns: stmt.columns.clone(),
        unique: stmt.unique,
    }
}

// ===========================================================================
// Internal helpers
// ===========================================================================

/// `BTreeMap` alias local to this module (avoids repeating the full path).
type BTreeMap<K, V> = std::collections::BTreeMap<K, V>;

// ===========================================================================
// Tests (T7.6 — UC-1..UC-5 + edge cases + argument-order invariant)
// ===========================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::catalog::{CatalogColumn, CatalogIndex, CatalogSchema, CatalogTable};
    use common_sql::ast::{ColumnConstraint, DataType, Identifier, IndexColumn};

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

    fn col_nn(name: &str, dt: DataType) -> CatalogColumn {
        let mut c = col(name, dt);
        c.nullable = false;
        c
    }

    fn table(name: &str, columns: Vec<CatalogColumn>) -> CatalogTable {
        CatalogTable {
            name: name.to_string(),
            columns,
            constraints: vec![],
        }
    }

    fn idx(name: &str, table: &str, col: &str, unique: bool) -> CatalogIndex {
        CatalogIndex {
            name: name.to_string(),
            table: table.to_string(),
            columns: vec![IndexColumn {
                name: Identifier::new(col.to_string()),
                direction: None,
            }],
            unique,
        }
    }

    fn empty_schema() -> CatalogSchema {
        CatalogSchema::default()
    }

    // ===== UC-1: empty -> 1 table produces one TableDiff::Added =====

    #[test]
    fn uc1_empty_to_one_table_yields_added() {
        let current = empty_schema();
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::BigInt)])],
            indices: vec![],
        };
        let diff = diff_schema(&current, &desired);
        assert_eq!(diff.table_diffs.len(), 1);
        match &diff.table_diffs[0] {
            TableDiff::Added(t) => {
                assert_eq!(t.name, "users");
                assert_eq!(t.columns.len(), 1);
            }
            other => panic!("expected Added, got {other:?}"),
        }
        assert!(diff.warnings.is_empty(), "UC-1 must not warn");
    }

    // ===== UC-2: column added =====

    #[test]
    fn uc2_column_added() {
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
        assert_eq!(diff.table_diffs.len(), 1);
        let TableDiff::Modified { column_diffs, .. } = &diff.table_diffs[0] else {
            panic!("expected Modified");
        };
        assert_eq!(column_diffs.len(), 1);
        match &column_diffs[0] {
            ColumnDiff::Added(c) => assert_eq!(c.name.value(), "email"),
            other => panic!("expected ColumnDiff::Added, got {other:?}"),
        }
    }

    // ===== UC-3: three destructive changes each emit a warning =====

    #[test]
    fn uc3_varchar_length_shrink_is_narrowing() {
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
        let destructive_count = diff
            .warnings
            .iter()
            .filter(|w| matches!(w, MigrationWarning::Destructive { .. }))
            .count();
        assert_eq!(destructive_count, 1, "VARCHAR shrink must warn once");
        let TableDiff::Modified { column_diffs, .. } = &diff.table_diffs[0] else {
            panic!("expected Modified");
        };
        let ColumnDiff::Modified { changes, .. } = &column_diffs[0] else {
            panic!("expected ColumnDiff::Modified");
        };
        assert!(matches!(
            changes[0],
            ColumnChange::TypeChanged {
                is_narrowing: true,
                ..
            }
        ));
    }

    #[test]
    fn uc3_decimal_precision_drop_is_narrowing() {
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table(
                "accounts",
                vec![col(
                    "balance",
                    DataType::Decimal {
                        precision: Some(18),
                        scale: Some(4),
                    },
                )],
            )],
            indices: vec![],
        };
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table(
                "accounts",
                vec![col(
                    "balance",
                    DataType::Decimal {
                        precision: Some(10),
                        scale: Some(4),
                    },
                )],
            )],
            indices: vec![],
        };
        let diff = diff_schema(&current, &desired);
        assert!(diff
            .warnings
            .iter()
            .any(|w| matches!(w, MigrationWarning::Destructive { .. })));
        let TableDiff::Modified { column_diffs, .. } = &diff.table_diffs[0] else {
            panic!("expected Modified");
        };
        let ColumnDiff::Modified { changes, .. } = &column_diffs[0] else {
            panic!("expected ColumnDiff::Modified");
        };
        assert!(matches!(
            changes[0],
            ColumnChange::TypeChanged {
                is_narrowing: true,
                ..
            }
        ));
    }

    #[test]
    fn uc3_not_null_added_tightens() {
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table(
                "users",
                vec![col("email", DataType::VarChar { length: Some(50) })],
            )],
            indices: vec![],
        };
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table(
                "users",
                vec![col_nn("email", DataType::VarChar { length: Some(50) })],
            )],
            indices: vec![],
        };
        let diff = diff_schema(&current, &desired);
        assert!(diff
            .warnings
            .iter()
            .any(|w| matches!(w, MigrationWarning::Destructive { .. })));
        let TableDiff::Modified { column_diffs, .. } = &diff.table_diffs[0] else {
            panic!("expected Modified");
        };
        let ColumnDiff::Modified { changes, .. } = &column_diffs[0] else {
            panic!("expected ColumnDiff::Modified");
        };
        assert!(matches!(
            changes[0],
            ColumnChange::NullabilityChanged { tightens: true, .. }
        ));
    }

    // ===== UC-4: identical schema yields empty SchemaDiff =====

    #[test]
    fn uc4_identical_schema_is_empty_diff() {
        let schema = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::BigInt)])],
            indices: vec![idx("idx_users_id", "users", "id", true)],
        };
        let diff = diff_schema(&schema, &schema);
        assert!(diff
            .table_diffs
            .iter()
            .all(|d| matches!(d, TableDiff::Unchanged { .. })));
        assert!(diff.index_diffs.is_empty());
        assert!(diff.warnings.is_empty());
    }

    // ===== UC-5: both empty -> empty diff, no warnings =====

    #[test]
    fn uc5_both_empty_no_warnings() {
        let diff = diff_schema(&empty_schema(), &empty_schema());
        assert!(diff.table_diffs.is_empty());
        assert!(diff.index_diffs.is_empty());
        assert!(diff.warnings.is_empty());
    }

    // ===== Argument-order invariant (CTO condition #1) =====

    #[test]
    fn argument_order_added_removed_are_symmetric() {
        // diff_schema(a, b): a has table t1, b has table t2.
        // Added(b-only) in (a,b) == Removed in (b,a).
        let a = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("t1", vec![col("id", DataType::Int)])],
            indices: vec![],
        };
        let b = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("t2", vec![col("id", DataType::Int)])],
            indices: vec![],
        };
        let forward = diff_schema(&a, &b);
        let backward = diff_schema(&b, &a);

        let forward_added: Vec<&str> = forward
            .table_diffs
            .iter()
            .filter_map(|d| match d {
                TableDiff::Added(t) => Some(t.name.as_str()),
                _ => None,
            })
            .collect();
        let forward_removed: Vec<&str> = forward
            .table_diffs
            .iter()
            .filter_map(|d| match d {
                TableDiff::Removed { name } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(forward_added, vec!["t2"]);
        assert_eq!(forward_removed, vec!["t1"]);

        let backward_added: Vec<&str> = backward
            .table_diffs
            .iter()
            .filter_map(|d| match d {
                TableDiff::Added(t) => Some(t.name.as_str()),
                _ => None,
            })
            .collect();
        let backward_removed: Vec<&str> = backward
            .table_diffs
            .iter()
            .filter_map(|d| match d {
                TableDiff::Removed { name } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(backward_added, vec!["t1"]);
        assert_eq!(backward_removed, vec!["t2"]);
    }

    // ===== Index diff: Added / Removed / Modified =====

    #[test]
    fn index_added_removed_modified() {
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::Int)])],
            indices: vec![
                idx("idx_a", "users", "a", false), // removed
                idx("idx_b", "users", "b", false), // modified (column change)
            ],
        };
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::Int)])],
            indices: vec![
                idx("idx_b", "users", "c", false), // modified
                idx("idx_c", "users", "c", true),  // added
            ],
        };
        let diff = diff_schema(&current, &desired);

        let has_added = diff
            .index_diffs
            .iter()
            .any(|d| matches!(d, IndexDiff::Added(s) if s.name.value() == "idx_c"));
        let has_removed = diff
            .index_diffs
            .iter()
            .any(|d| matches!(d, IndexDiff::Removed { name, .. } if name == "idx_a"));
        let has_modified = diff
            .index_diffs
            .iter()
            .any(|d| matches!(d, IndexDiff::Modified(s) if s.name.value() == "idx_b"));
        assert!(
            has_added,
            "expected idx_c Added, got {:?}",
            diff.index_diffs
        );
        assert!(has_removed, "expected idx_a Removed");
        assert!(has_modified, "expected idx_b Modified");
    }

    // ===== Constraint diff: Added =====

    #[test]
    fn constraint_added_to_table() {
        let pk = common_sql::ast::TableConstraint::PrimaryKey {
            name: Some("pk_users".to_string()),
            columns: vec![Identifier::new("id".to_string())],
        };
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![CatalogTable {
                name: "users".to_string(),
                columns: vec![col("id", DataType::BigInt)],
                constraints: vec![],
            }],
            indices: vec![],
        };
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![CatalogTable {
                name: "users".to_string(),
                columns: vec![col("id", DataType::BigInt)],
                constraints: vec![pk],
            }],
            indices: vec![],
        };
        let diff = diff_schema(&current, &desired);
        let TableDiff::Modified {
            constraint_diffs, ..
        } = &diff.table_diffs[0]
        else {
            panic!("expected Modified");
        };
        assert_eq!(constraint_diffs.len(), 1);
        assert!(matches!(
            &constraint_diffs[0],
            ConstraintDiff::Added { name, .. } if name == "pk_users"
        ));
    }

    // ===== build_desired_schema: parse DDL =====

    #[test]
    fn build_desired_schema_parses_create_table_and_index() {
        let ddl =
            "CREATE TABLE users (id BIGINT NOT NULL);\nCREATE INDEX idx_users_id ON users (id);";
        let schema = build_desired_schema(ddl).expect("valid DDL must parse");
        assert_eq!(schema.tables.len(), 1);
        assert_eq!(schema.tables[0].name, "users");
        assert_eq!(schema.tables[0].columns.len(), 1);
        assert_eq!(schema.tables[0].columns[0].data_type, DataType::BigInt);
        assert!(!schema.tables[0].columns[0].nullable);
        assert_eq!(schema.indices.len(), 1);
        assert_eq!(schema.indices[0].name, "idx_users_id");
        assert_eq!(schema.indices[0].table, "users");
    }

    #[test]
    fn build_desired_schema_returns_parse_failed_on_garbage() {
        let result = build_desired_schema("THIS IS NOT SQL");
        assert!(matches!(result, Err(CatalogError::ParseFailed { .. })));
    }

    // ===== narrowing edge cases =====

    #[test]
    fn varchar_none_to_some_is_narrowing() {
        assert!(is_narrowing_type_change(
            &DataType::VarChar { length: None },
            &DataType::VarChar { length: Some(50) },
        ));
    }

    #[test]
    fn varchar_widening_is_not_narrowing() {
        assert!(!is_narrowing_type_change(
            &DataType::VarChar { length: Some(50) },
            &DataType::VarChar { length: Some(255) },
        ));
    }

    #[test]
    fn int_to_bigint_is_not_narrowing() {
        assert!(!is_narrowing_type_change(&DataType::Int, &DataType::BigInt,));
    }

    // ===== drop column warns (destructive) =====

    #[test]
    fn drop_column_emits_destructive_warning() {
        let current = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table(
                "users",
                vec![col("id", DataType::BigInt), col("legacy", DataType::Int)],
            )],
            indices: vec![],
        };
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![col("id", DataType::BigInt)])],
            indices: vec![],
        };
        let diff = diff_schema(&current, &desired);
        assert!(diff.warnings.iter().any(|w| matches!(
            w,
            MigrationWarning::Destructive { detail, .. } if detail.contains("DROP COLUMN")
        )));
    }

    // ===== identity flag preserved in ColumnDiff::Added =====

    #[test]
    fn added_column_preserves_identity() {
        let mut identity_col = col("id", DataType::BigInt);
        identity_col.identity = true;
        let desired = CatalogSchema {
            schema_name: String::new(),
            tables: vec![table("users", vec![identity_col])],
            indices: vec![],
        };
        let diff = diff_schema(&empty_schema(), &desired);
        let TableDiff::Added(t) = &diff.table_diffs[0] else {
            panic!("expected Added");
        };
        assert!(t.columns[0].identity);
        // And the column-def carry-through includes AutoIncrement.
        let cd = catalog_column_to_column_def(&t.columns[0]);
        assert!(cd
            .constraints
            .iter()
            .any(|c| matches!(c, ColumnConstraint::AutoIncrement)));
    }

    // ===== default change detected =====

    #[test]
    fn default_change_detected() {
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
        let TableDiff::Modified { column_diffs, .. } = &diff.table_diffs[0] else {
            panic!("expected Modified");
        };
        let ColumnDiff::Modified { changes, .. } = &column_diffs[0] else {
            panic!("expected ColumnDiff::Modified");
        };
        assert!(changes
            .iter()
            .any(|c| matches!(c, ColumnChange::DefaultChanged { .. })));
    }
}
