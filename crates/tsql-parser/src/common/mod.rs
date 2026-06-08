//! Common SQL AST モジュール
//!
//! 方言非依存のSQL抽象構文木（AST）を定義する。
//! T-SQL 固有の構文から方言に依存しない表現への変換を目的とする。
//!
//! 実体は `common-sql` クレートに定義されており、このモジュールは
//! 後方互換性のために再エクスポートを提供する。

pub use common_sql::{
    CommonAssignment, CommonBinaryOperator, CommonCaseExpression, CommonColumnReference,
    CommonDataType, CommonDeleteStatement, CommonExpression, CommonFunctionCall, CommonIdentifier,
    CommonInList, CommonInsertSource, CommonInsertStatement, CommonLimitClause, CommonLiteral,
    CommonOrderByItem, CommonSelectItem, CommonSelectStatement, CommonStatement,
    CommonTableReference, CommonUnaryOperator, CommonUpdateStatement, ToCommonAst,
};
