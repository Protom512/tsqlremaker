//! # schema-diff
//!
//! Dialect-neutral schema diff engine for the SAP ASE T-SQL transpiler.
//!
//! Compares a *current* catalog (ASE introspection or JSON dump) against a
//! *desired* schema (derived from DDL) and produces dialect-independent
//! `AlterOperation`s that the mysql/postgresql/sqlite emitters consume.
//!
//! ## Module layout (T6 scope)
//!
//! This crate skeleton (T6) exposes three modules:
//!
//! - [`catalog`]: catalog data model (`CatalogSchema`/`CatalogTable`/...) and
//!   the `CatalogProvider` trait (design §3).
//! - [`warning`]: `MigrationWarning` (design §2.6).
//! - [`mapper`]: common-sql AST ↔ `CatalogSchema` conversions (design §7).
//!
//! The public diff API (`diff_schema`/`plan_operations`/`to_statements`/
//! `build_desired_schema`) and the `adapters` module are deferred to T7/T8/T11
//! and are intentionally NOT declared here (see spec tasks.md Group C/D/E).

pub mod catalog;
pub mod mapper;
pub mod warning;
