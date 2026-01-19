//! # T-SQL Token Definitions
//!
//! このクレートは、SAP ASE T-SQL のトークン型を定義する。
//!
//! ## 構成要素
//!
//! - [`TokenKind`]: トークン種別の列挙型
//! - [`Position`]: ソースコード上の人間可読な位置（行、列、オフセット）
//! - [`Span`]: ソースコード上のバイト単位の範囲

#![warn(missing_docs)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

pub mod kind;
pub mod position;

pub use kind::TokenKind;
pub use position::{Position, Span};
