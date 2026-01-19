//! # T-SQL Lexer
//!
//! このクレートは、SAP ASE T-SQL の字句解析器（Lexer）を提供する。
//!
//! ## 機能
//!
//! - SAP ASE T-SQL の完全なトークン化
//! - ネストされたブロックコメントのサポート
//! - 変数プレフィックス（`@`, `@@`, `#`, `##`）の認識
//! - Unicode 文字列リテラルのサポート
//! - エラー位置の正確な追跡

#![warn(missing_docs)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

pub mod cursor;
pub mod error;
pub mod lexer;

pub use error::{BracketType, LexError};
pub use lexer::{Lexer, Token};
pub use tsql_token::{Position, Span, TokenKind};
