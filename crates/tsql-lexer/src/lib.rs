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

pub use tsql_token::{Position, Span, TokenKind};

/// 字句解析器
///
/// ソースコードをトークンストリームに変換する。
pub struct Lexer<'src> {
    _input: &'src str,
}

impl<'src> Lexer<'src> {
    /// 新しい Lexer を作成する
    ///
    /// # Arguments
    ///
    /// * `input` - 字句解析するソースコード
    #[must_use]
    pub const fn new(input: &'src str) -> Self {
        Self { _input: input }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_creation() {
        let lexer = Lexer::new("SELECT * FROM users");
        assert_eq!(lexer._input, "SELECT * FROM users");
    }
}
