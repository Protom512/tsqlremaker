//! バッチ区切り関連のASTノード

use tsql_token::Span;

use super::base::AstNode;

/// バッチ区切り（GO）
#[derive(Debug, Clone)]
pub struct BatchSeparator {
    /// 位置情報
    pub span: Span,
    /// 繰り返し回数（GO NのN）
    pub repeat_count: Option<u32>,
}

impl AstNode for BatchSeparator {
    fn span(&self) -> Span {
        self.span
    }
}
