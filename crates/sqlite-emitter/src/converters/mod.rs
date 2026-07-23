//! Converter モジュール
//!
//! Common SQL AST の各要素を SQLite 方言に変換するコンバーター。
//!
//! このモジュールは `mysql-emitter::converters` と対称な構造を持ちます
//! (architecture-coupling-balance.md §1.2: エミッタ間アーキテクチャ整合)。
//! 現状は [`function::FunctionConverter`] のみを公開します。データ型変換や
//! 構文変換は後続タスクで追加される予定です。

pub mod function;

pub use function::FunctionConverter;
