//! AST（抽象構文木）ノードの定義
//!
//! T-SQLの抽象構文木を表現するノード型を定義する。

mod base;
mod batch;
mod control_flow;
mod data_modification;
mod ddl;
mod expression;
mod select;
mod to_common;

#[cfg(test)]
mod tests;

// Core exports
pub use base::AstNode;

// Statement exports
pub use batch::BatchSeparator;
pub use control_flow::{
    Assignment, Block, BreakStatement, ContinueStatement, DeclareStatement, IfStatement,
    ReturnStatement, SetStatement, VariableAssignment, VariableDeclaration, WhileStatement,
};
pub use data_modification::{
    Assignment as ColumnAssignment, DeleteStatement, InsertSource, InsertStatement, UpdateStatement,
};
pub use ddl::{
    ColumnConstraint, ColumnDefinition, CreateStatement, DataType, IndexDefinition,
    ParameterDefinition, ProcedureDefinition, TableConstraint, TableDefinition, ViewDefinition,
};
pub use select::{
    FromClause, Join, JoinType, LimitClause, OrderByItem, SelectItem, SelectStatement,
    TableReference,
};

// Expression exports
pub use expression::{
    BinaryOperator, CaseExpression, ColumnReference, FunctionArg, FunctionCall, Identifier, InList,
    IsValue, Literal, UnaryOperator,
};

// Expression must come after some statement types that reference it
pub use expression::Expression;

/// 文（Statement）
///
/// T-SQLの全ての文種別を表す列挙型。
#[derive(Debug, Clone)]
pub enum Statement {
    /// SELECT文
    Select(Box<SelectStatement>),
    /// INSERT文
    Insert(Box<InsertStatement>),
    /// UPDATE文
    Update(Box<UpdateStatement>),
    /// DELETE文
    Delete(Box<DeleteStatement>),
    /// CREATE文
    Create(Box<CreateStatement>),
    /// DECLARE文
    Declare(Box<DeclareStatement>),
    /// SET文
    Set(Box<SetStatement>),
    /// SELECT変数代入文（SELECT @var = expr）
    VariableAssignment(Box<VariableAssignment>),
    /// IF...ELSE文
    If(Box<IfStatement>),
    /// WHILE文
    While(Box<WhileStatement>),
    /// BEGIN...ENDブロック
    Block(Box<Block>),
    /// BREAK文
    Break(Box<BreakStatement>),
    /// CONTINUE文
    Continue(Box<ContinueStatement>),
    /// RETURN文
    Return(Box<ReturnStatement>),
    /// バッチ区切り（GO）
    BatchSeparator(BatchSeparator),
}

impl AstNode for Statement {
    fn span(&self) -> tsql_token::Span {
        match self {
            Statement::Select(s) => s.span,
            Statement::Insert(s) => s.span,
            Statement::Update(s) => s.span,
            Statement::Delete(s) => s.span,
            Statement::Create(s) => s.span(),
            Statement::Declare(s) => s.span,
            Statement::Set(s) => s.span,
            Statement::VariableAssignment(s) => s.span,
            Statement::If(s) => s.span,
            Statement::While(s) => s.span,
            Statement::Block(s) => s.span,
            Statement::Break(s) => s.span,
            Statement::Continue(s) => s.span,
            Statement::Return(s) => s.span,
            Statement::BatchSeparator(s) => s.span,
        }
    }
}
