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
//!
//! ## パフォーマンス特性
//!
//! ### ゼロコピー設計
//!
//! この Lexer はゼロコピー設計を採用しており、トークンはソースコードへの参照（`&str`）を保持します。
//! これにより、不要な文字列割り当てを回避し、メモリ消費を最小限に抑えています。
//!
//! ### 静的キーワードマップ
//!
//! キーワード解決には `once_cell::sync::Lazy` による静的 HashMap を使用しており、
//! プログラム起動時に1回のみ初期化されます。これにより、キーワード解決のオーバーヘッドが
//! O(1) で、再作成コストが発生しません。
//!
//! ### 効率的な文字列トラバーサル
//!
//! `CharIndices` イテレータを使用した文字列トラバーサルにより、
//! Unicode 文字を含む入力を正しく処理しつつ、効率的な走査を実現しています。
//!
//! ## 使用例
//!
//! ```
//! use tsql_lexer::Lexer;
//!
//! let sql = "SELECT * FROM users WHERE id = 1";
//! let mut lexer = Lexer::new(sql);
//!
//! // イテレータとしてトークンを収集
//! let tokens: Vec<_> = lexer.by_ref().collect();
//!
//! // または個別に取得
//! let mut lexer = Lexer::new(sql);
//! while let Ok(token) = lexer.next_token() {
//!     if token.kind == tsql_token::TokenKind::Eof {
//!         break;
//!     }
//!     println!("{:?}", token.kind);
//! }
//! ```

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
