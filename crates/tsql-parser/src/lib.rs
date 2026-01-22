//! # T-SQL Parser
//!
//! このクレートは、SAP ASE (Sybase Adaptive Server Enterprise) の T-SQL 方言で記述された
//! SQL コードを構文解析する Parser を提供する。
//!
//! ## 機能
//!
//! - DML: SELECT, INSERT, UPDATE, DELETE
//! - DDL: CREATE (TABLE, INDEX, VIEW, PROCEDURE)
//! - 制御フロー: IF...ELSE, WHILE, BEGIN...END, BREAK, CONTINUE, RETURN
//! - 変数: DECLARE, SET
//! - 式: 算術、比較、論理演算子、関数、CASE 式
//! - バッチ処理: GO キーワードによるバッチ区切り
//!
//! ## 使用例
//!
//! ```
//! use tsql_parser::{Parser, ParserMode};
//!
//! let sql = "SELECT * FROM users WHERE id = 1";
//! let mut parser = Parser::new(sql);
//!
//! // 文を解析
//! let statements = parser.parse().unwrap();
//!
//! // 単一文モードで解析
//! let mut parser = Parser::new(sql).with_mode(ParserMode::SingleStatement);
//! let stmt = parser.parse_statement().unwrap();
//! ```

#![warn(missing_docs)]
// workspace.lints から clippy 設定を継承
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

pub mod ast;
pub mod buffer;
pub mod error;
pub mod expression;
pub mod parser;

// 公開APIの再エクスポート
pub use ast::{
    Assignment, AstNode, BinaryOperator, Block, BreakStatement, CaseExpression, ColumnDefinition,
    ColumnReference, CreateStatement, DataType, DeclareStatement, DeleteStatement, Expression,
    FunctionArg, FunctionCall, Identifier, InList, InsertSource, InsertStatement, IsValue, Join,
    JoinType, Literal, OrderByItem, ParameterDefinition, ProcedureDefinition, ReturnStatement,
    SelectItem, SelectStatement, SetStatement, Statement, TableConstraint, TableDefinition,
    UnaryOperator, UpdateStatement, VariableDeclaration, WhileStatement,
};
pub use error::{ParseError, ParseResult};
pub use expression::ExpressionParser;
pub use parser::{Parser, ParserMode};
pub use tsql_token::{Position, Span, TokenKind};

// トークン構造体も再エクスポート
pub use tsql_lexer::Token;

/// SQL文を解析するヘルパー関数
///
/// # Arguments
///
/// * `input` - 解析するSQLソースコード
///
/// # Returns
///
/// 文のリスト、またはエラー
///
/// # Examples
///
/// ```
/// use tsql_parser::parse;
///
/// let sql = "SELECT * FROM users";
/// let statements = parse(sql).unwrap();
/// assert_eq!(statements.len(), 1);
/// ```
pub fn parse(input: &str) -> ParseResult<Vec<Statement>> {
    let mut parser = Parser::new(input);
    parser.parse()
}

/// 単一のSQL文を解析するヘルパー関数
///
/// # Arguments
///
/// * `input` - 解析するSQLソースコード
///
/// # Returns
///
/// 文、またはエラー
///
/// # Examples
///
/// ```
/// use tsql_parser::parse_one;
///
/// let sql = "SELECT * FROM users";
/// let stmt = parse_one(sql).unwrap();
/// ```
pub fn parse_one(input: &str) -> ParseResult<Statement> {
    let mut parser = Parser::new(input).with_mode(ParserMode::SingleStatement);
    parser.parse_statement()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_function() {
        let sql = "SELECT * FROM users";
        let result = parse(sql);
        assert!(result.is_ok());
        let statements = result.unwrap();
        assert_eq!(statements.len(), 1);
    }

    #[test]
    fn test_parse_one_function() {
        let sql = "SELECT 1";
        let result = parse_one(sql);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_with_error() {
        let sql = "SELCT * FROM users"; // typo
        let result = parse(sql);
        assert!(result.is_err());
    }
}
