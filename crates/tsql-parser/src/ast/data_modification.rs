//! データ操作言語（INSERT, UPDATE, DELETE）のASTノード

use tsql_token::Span;

use super::base::AstNode;
use super::expression::{Expression, Identifier};
use super::select::{FromClause, SelectStatement, TableReference};

/// INSERT文
#[derive(Debug, Clone)]
pub struct InsertStatement {
    /// 位置情報
    pub span: Span,
    /// 対象テーブル
    pub table: Identifier,
    /// カラムリスト
    pub columns: Vec<Identifier>,
    /// データソース
    pub source: InsertSource,
}

impl AstNode for InsertStatement {
    fn span(&self) -> Span {
        self.span
    }
}

/// INSERTデータソース
#[derive(Debug, Clone)]
pub enum InsertSource {
    /// VALUES句
    Values(Vec<Vec<Expression>>),
    /// INSERT-SELECT
    Select(Box<SelectStatement>),
    /// DEFAULT VALUES
    DefaultValues,
}

/// UPDATE文
#[derive(Debug, Clone)]
pub struct UpdateStatement {
    /// 位置情報
    pub span: Span,
    /// 対象テーブル
    pub table: TableReference,
    /// 代入リスト
    pub assignments: Vec<Assignment>,
    /// FROM句（ASE固有）
    pub from_clause: Option<FromClause>,
    /// WHERE句
    pub where_clause: Option<Expression>,
}

impl AstNode for UpdateStatement {
    fn span(&self) -> Span {
        self.span
    }
}

/// 代入
#[derive(Debug, Clone)]
pub struct Assignment {
    /// 代入先カラム
    pub column: Identifier,
    /// 代入値
    pub value: Expression,
}

/// DELETE文
#[derive(Debug, Clone)]
pub struct DeleteStatement {
    /// 位置情報
    pub span: Span,
    /// 対象テーブル
    pub table: Identifier,
    /// FROM句（結合用）
    pub from_clause: Option<FromClause>,
    /// WHERE句
    pub where_clause: Option<Expression>,
}

impl AstNode for DeleteStatement {
    fn span(&self) -> Span {
        self.span
    }
}
