//! AST基底トレイト

use tsql_token::Span;

/// 全てのASTノードの基底トレイト
pub trait AstNode {
    /// このノードのソースコード上の範囲を返す
    fn span(&self) -> Span;
}
