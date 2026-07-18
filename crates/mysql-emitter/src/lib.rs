//! # MySQL Emitter
//!
//! Common SQL AST ([`common_sql::ast::Statement`]) を MySQL 方言の SQL 文字列へ
//! トランスパイルするライブラリです。
//!
//! ## 概要
//!
//! このライブラリは、[`common_sql`] の AST を入力として受け取り、MySQL 方言の
//! SQL 文字列を出力します。設計上の依存先は [`common_sql`] のみであり、
//! `tsql-parser` / `tsql-token` への直接依存を持ちません
//! (結合負債の是正 — architecture §1.2)。
//!
//! ## 設計 (Task 3.1 / design Req 1.5)
//!
//! [`MySqlEmitter`] は公開の [`common_sql::Visitor`] トレイトを実装して
//! コントラクトに適合します (`type Output = String`)。ただし `Visitor::Output`
//! はエラー型を内包できないため、実際のエラー伝播は private な `Result` 返却型の
//! 再帰メソッド群で行います。`Visitor` 実装はこれら private メソッドへ委譲し、
//! エラーは [`MySqlEmitter`] の `last_error` へ退避されます (ハイブリッド設計)。
//!
//! ## 使用例
//!
//! ```rust,ignore
//! use mysql_emitter::{MySqlEmitter, EmitterConfig};
//! use common_sql::ast::{Statement, SelectStatement, SelectItem};
//!
//! let mut emitter = MySqlEmitter::new(EmitterConfig::default());
//! let stmt = Statement::Select(Box::new(SelectStatement::simple(vec![SelectItem::Wildcard])));
//! let sql = emitter.emit(&stmt).unwrap();
//! assert_eq!(sql, "SELECT *");
//! ```
//!
//! ## 機能
//!
//! - SELECT / INSERT / UPDATE / DELETE / CREATE TABLE / DROP TABLE /
//!   CREATE INDEX / DROP INDEX / ALTER TABLE の生成
//! - データ型・関数の MySQL 方言への変換
//! - 式 visitor (全 15 バリアント)

#![warn(missing_docs)]
// workspace.lints から clippy 設定を継承
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

mod config;
mod converters;
mod ddl;
mod emitter;
mod error;
mod statement;

pub use config::EmitterConfig;
pub use converters::{DataTypeConverter, FunctionConverter, SyntaxConverter};
pub use error::EmitError;
pub use statement::MySqlEmitter;
