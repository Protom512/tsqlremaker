//! Converter モジュール
//!
//! Common SQL AST の各要素を MySQL 方言に変換するコンバーター。

mod datatype;
mod function;
mod syntax;

// Data type and syntax converters are not yet fully integrated
// They are defined but not exported to avoid unused warnings
pub use function::FunctionConverter;
