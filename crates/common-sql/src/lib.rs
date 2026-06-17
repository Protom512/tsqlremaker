//! # Common SQL AST
//!
//! Dialect-independent SQL AST for transpilation.
//! This crate defines the shared intermediate representation used by
//! all SQL emitters (MySQL, PostgreSQL, SQLite).

#![warn(missing_docs)]
// workspace.lints から clippy 設定を継承
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

pub mod ast;
