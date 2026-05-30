//! Converter モジュール
//!
//! Common SQL AST の各要素を MySQL 方言に変換するコンバーター。

mod function;

pub use function::FunctionConverter;
