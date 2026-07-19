//! # schema-diff
//!
//! Dialect-neutral schema diff engine for the SAP ASE T-SQL transpiler.
//!
//! Compares a *current* catalog (ASE introspection or JSON dump) against a
//! *desired* schema (derived from DDL) and produces dialect-independent
//! `AlterOperation`s that the mysql/postgresql/sqlite emitters consume.
//!
//! ## Public API (design §5)
//!
//! - [`diff_schema`]: pure diff of two `CatalogSchema`s into a [`diff::SchemaDiff`].
//! - [`plan_operations`]: lift a `SchemaDiff` into dialect-neutral `AlterOperation`s.
//! - [`to_statements`]: map `AlterOperation`s into `common_sql::ast::Statement`s.
//! - [`build_desired_schema`]: parse DDL source into a `CatalogSchema` (desired side).
//!
//! ## Module layout
//!
//! - [`catalog`]: catalog data model and the `CatalogProvider` trait (design §3).
//! - [`diff`]: diff data model + `diff_schema` (design §2 / §5).
//! - [`emit`]: `AlterOperation` + `plan_operations` + `to_statements` (design §4 / §5).
//! - [`warning`]: `MigrationWarning` (design §2.6).
//! - [`mapper`]: common-sql AST ↔ `CatalogSchema` conversions (design §7).

pub mod catalog;
pub mod diff;
pub mod emit;
pub mod mapper;
pub mod warning;

use common_sql::ast;

use crate::catalog::{CatalogError, CatalogSchema};

/// desired (DDL から構築) と current (catalog) の差分を計算する純粋関数。
///
/// IO を含まない (design §5 AC-4)。両入力は呼び出し側が構築済みの
/// `CatalogSchema` 表現。引数順序は `diff_schema(current, desired)` で
/// 固定 (CTO 条件 #1 不変量・[`diff::diff_schema`] 参照)。
#[must_use]
pub fn diff_schema(current: &CatalogSchema, desired: &CatalogSchema) -> diff::SchemaDiff {
    diff::diff_schema(current, desired)
}

/// `SchemaDiff` を方言非依存の `AlterOperation` 列に変換する。
///
/// 警告 (`MigrationWarning::Destructive` 等) は `SchemaDiff.warnings` から
/// 引き継がれる (`AlterOperation` には載せない — 呼び出し側が `SchemaDiff`
/// を保持して警告を参照する設計)。
#[must_use]
pub fn plan_operations(diff: &diff::SchemaDiff) -> Vec<emit::AlterOperation> {
    emit::plan_operations(diff)
}

/// `AlterOperation` 列を `common_sql::ast::Statement` 列に変換する。
///
/// 各 `AlterOperation` は 1:1 で `Statement` に変換される (design §4.1)。
/// これを各 emitter (T3/T4/T5 拡張後) に渡して方言別 SQL 文字列を得る。
#[must_use]
pub fn to_statements(ops: &[emit::AlterOperation]) -> Vec<ast::Statement> {
    emit::to_statements(ops)
}

/// CREATE TABLE 系 DDL 文列をパースして desired 側 `CatalogSchema` を構築する。
///
/// 内部で tsql-parser の `parse_with_errors` + `to_common_sql` を呼び、
/// `CreateTable` / `CreateIndex` を mapper 逆変換で `CatalogSchema` に組立てる。
///
/// # Errors
///
/// DDL にパースエラーが含まれる場合 `CatalogError::ParseFailed` を返す。
pub fn build_desired_schema(ddl_source: &str) -> Result<CatalogSchema, CatalogError> {
    diff::build_desired_schema(ddl_source)
}
