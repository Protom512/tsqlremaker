//! Common SQL AST モジュール
//!
//! 方言非依存のSQL抽象構文木（AST）を定義する。
//! T-SQL 固有の構文から方言に依存しない表現への変換を目的とする。

mod expression;
mod statement;

pub use expression::{
    CommonBinaryOperator, CommonCaseExpression, CommonColumnReference, CommonExpression,
    CommonFunctionCall, CommonIdentifier, CommonLiteral, CommonUnaryOperator,
};
pub use statement::{
    CommonAssignment, CommonDeleteStatement, CommonInsertSource, CommonInsertStatement,
    CommonLimitClause, CommonOrderByItem, CommonSelectItem, CommonSelectStatement, CommonStatement,
    CommonTableReference, CommonUpdateStatement,
};

/// Common SQL AST 変換トレイト
///
/// T-SQL ASTノードを方言非依存の Common SQL AST に変換する。
pub trait ToCommonAst {
    /// Common SQL AST に変換
    ///
    /// # Returns
    ///
    /// 変換されたノード、または変換不可能な場合は None
    fn to_common_ast(&self) -> Option<CommonStatement>
    where
        Self: Sized,
    {
        None
    }

    /// 式を Common AST に変換
    ///
    /// # Returns
    ///
    /// 変換された式、または変換不可能な場合は None
    fn to_common_expression(&self) -> Option<CommonExpression>
    where
        Self: Sized,
    {
        None
    }
}
