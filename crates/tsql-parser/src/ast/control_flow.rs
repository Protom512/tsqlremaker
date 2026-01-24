//! 制御フロー関連のASTノード

use tsql_token::Span;

use super::base::AstNode;
use super::ddl::DataType;
use super::expression::{Expression, Identifier};
use super::Statement;

/// DECLARE文
#[derive(Debug, Clone)]
pub struct DeclareStatement {
    /// 位置情報
    pub span: Span,
    /// 変数宣言リスト
    pub variables: Vec<VariableDeclaration>,
}

impl AstNode for DeclareStatement {
    fn span(&self) -> Span {
        self.span
    }
}

/// 変数宣言
#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    /// 変数名（@variable）
    pub name: Identifier,
    /// データ型
    pub data_type: DataType,
    /// デフォルト値
    pub default_value: Option<Expression>,
}

/// SET文
#[derive(Debug, Clone)]
pub struct SetStatement {
    /// 位置情報
    pub span: Span,
    /// 変数名
    pub variable: Identifier,
    /// 代入値
    pub value: Expression,
}

impl AstNode for SetStatement {
    fn span(&self) -> Span {
        self.span
    }
}

/// SELECT変数代入文
///
/// T-SQLの `SELECT @variable = expression` 構文を表す。
/// 複数の変数をカンマ区切りで代入できる。
#[derive(Debug, Clone)]
pub struct VariableAssignment {
    /// 位置情報
    pub span: Span,
    /// 代入リスト
    pub assignments: Vec<Assignment>,
}

impl AstNode for VariableAssignment {
    fn span(&self) -> Span {
        self.span
    }
}

/// 変数への代入
#[derive(Debug, Clone)]
pub struct Assignment {
    /// 変数名（@variable）
    pub variable: Identifier,
    /// 代入値
    pub value: Expression,
}

/// IF文
#[derive(Debug, Clone)]
pub struct IfStatement {
    /// 位置情報
    pub span: Span,
    /// 条件式
    pub condition: Expression,
    /// THENブロック
    pub then_branch: Statement,
    /// ELSEブロック
    pub else_branch: Option<Statement>,
}

impl AstNode for IfStatement {
    fn span(&self) -> Span {
        self.span
    }
}

/// WHILE文
#[derive(Debug, Clone)]
pub struct WhileStatement {
    /// 位置情報
    pub span: Span,
    /// 条件式
    pub condition: Expression,
    /// ループ本体
    pub body: Statement,
}

impl AstNode for WhileStatement {
    fn span(&self) -> Span {
        self.span
    }
}

/// ブロック（BEGIN...END）
#[derive(Debug, Clone)]
pub struct Block {
    /// 位置情報
    pub span: Span,
    /// 文リスト
    pub statements: Vec<Statement>,
}

impl AstNode for Block {
    fn span(&self) -> Span {
        self.span
    }
}

/// BREAK文
#[derive(Debug, Clone)]
pub struct BreakStatement {
    /// 位置情報
    pub span: Span,
}

impl AstNode for BreakStatement {
    fn span(&self) -> Span {
        self.span
    }
}

/// CONTINUE文
#[derive(Debug, Clone)]
pub struct ContinueStatement {
    /// 位置情報
    pub span: Span,
}

impl AstNode for ContinueStatement {
    fn span(&self) -> Span {
        self.span
    }
}

/// RETURN文
#[derive(Debug, Clone)]
pub struct ReturnStatement {
    /// 位置情報
    pub span: Span,
    /// 戻り値式
    pub expression: Option<Expression>,
}

impl AstNode for ReturnStatement {
    fn span(&self) -> Span {
        self.span
    }
}
