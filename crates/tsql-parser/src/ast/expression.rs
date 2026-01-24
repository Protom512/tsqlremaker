//! 式（Expression）関連のASTノード

use tsql_token::Span;

use super::base::AstNode;
use super::select::SelectStatement;

/// 式（Expression）
///
/// T-SQLの全ての式種別を表す列挙型。
#[derive(Debug, Clone)]
pub enum Expression {
    /// リテラル値
    Literal(Literal),
    /// 識別子
    Identifier(Identifier),
    /// カラム参照
    ColumnReference(ColumnReference),
    /// 単項演算子
    UnaryOp {
        /// 演算子
        op: UnaryOperator,
        /// 被演算子
        expr: Box<Expression>,
        /// 位置情報
        span: Span,
    },
    /// 二項演算子
    BinaryOp {
        /// 左辺
        left: Box<Expression>,
        /// 演算子
        op: BinaryOperator,
        /// 右辺
        right: Box<Expression>,
        /// 位置情報
        span: Span,
    },
    /// 関数呼び出し
    FunctionCall(FunctionCall),
    /// CASE式
    Case(CaseExpression),
    /// サブクエリ
    Subquery(Box<SelectStatement>),
    /// EXISTS式
    Exists(Box<SelectStatement>),
    /// IN式
    In {
        /// 対象の式
        expr: Box<Expression>,
        /// INリスト
        list: InList,
        /// NOT INの場合はtrue
        negated: bool,
        /// 位置情報
        span: Span,
    },
    /// BETWEEN式
    Between {
        /// 対象の式
        expr: Box<Expression>,
        /// 下限値
        low: Box<Expression>,
        /// 上限値
        high: Box<Expression>,
        /// NOT BETWEENの場合はtrue
        negated: bool,
        /// 位置情報
        span: Span,
    },
    /// LIKE式
    Like {
        /// 対象の式
        expr: Box<Expression>,
        /// パターン
        pattern: Box<Expression>,
        /// エスケープ文字
        escape: Option<Box<Expression>>,
        /// NOT LIKEの場合はtrue
        negated: bool,
        /// 位置情報
        span: Span,
    },
    /// IS式
    Is {
        /// 対象の式
        expr: Box<Expression>,
        /// IS NOTの場合はtrue
        negated: bool,
        /// 比較対象
        value: IsValue,
        /// 位置情報
        span: Span,
    },
}

impl AstNode for Expression {
    fn span(&self) -> Span {
        match self {
            Expression::Literal(l) => l.span(),
            Expression::Identifier(i) => i.span,
            Expression::ColumnReference(c) => c.span,
            Expression::UnaryOp { span, .. } => *span,
            Expression::BinaryOp { span, .. } => *span,
            Expression::FunctionCall(f) => f.span,
            Expression::Case(c) => c.span,
            Expression::Subquery(s) => s.span,
            Expression::Exists(s) => s.span,
            Expression::In { span, .. } => *span,
            Expression::Between { span, .. } => *span,
            Expression::Like { span, .. } => *span,
            Expression::Is { span, .. } => *span,
        }
    }
}

/// リテラル値
#[derive(Debug, Clone)]
pub enum Literal {
    /// 文字列リテラル
    String(String, Span),
    /// 数値リテラル
    Number(String, Span),
    /// 浮動小数点数リテラル
    Float(String, Span),
    /// 16進数リテラル
    Hex(String, Span),
    /// NULLリテラル
    Null(Span),
    /// 真理値リテラル
    Boolean(bool, Span),
}

impl Literal {
    /// リテラルのspanを返す
    pub fn span(&self) -> Span {
        match self {
            Literal::String(_, s) => *s,
            Literal::Number(_, s) => *s,
            Literal::Float(_, s) => *s,
            Literal::Hex(_, s) => *s,
            Literal::Null(s) => *s,
            Literal::Boolean(_, s) => *s,
        }
    }
}

/// 識別子
#[derive(Debug, Clone)]
pub struct Identifier {
    /// 識別子名
    pub name: String,
    /// 位置情報
    pub span: Span,
}

impl AstNode for Identifier {
    fn span(&self) -> Span {
        self.span
    }
}

/// カラム参照
#[derive(Debug, Clone)]
pub struct ColumnReference {
    /// テーブル修飾子（省略可能）
    pub table: Option<Identifier>,
    /// カラム名
    pub column: Identifier,
    /// 位置情報
    pub span: Span,
}

impl AstNode for ColumnReference {
    fn span(&self) -> Span {
        self.span
    }
}

/// 単項演算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    /// +（正号）
    Plus,
    /// -（負号）
    Minus,
    /// ~（ビット否定）
    Tilde,
    /// NOT（論理否定）
    Not,
}

/// 二項演算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    /// +（加算）
    Plus,
    /// -（減算）
    Minus,
    /// *（乗算）
    Multiply,
    /// /（除算）
    Divide,
    /// %（剰余）
    Modulo,
    /// =（等価）
    Eq,
    /// !=（不等価）
    Ne,
    /// <>（不等価、別形式）
    NeAlt,
    /// <（より小さい）
    Lt,
    /// <=（以下）
    Le,
    /// >（より大きい）
    Gt,
    /// >=（以上）
    Ge,
    /// !<（以上）
    NotLt,
    /// !>（以下）
    NotGt,
    /// AND（論理積）
    And,
    /// OR（論理和）
    Or,
    /// IN（包含演算子）
    In,
    /// BETWEEN（範囲演算子）
    Between,
    /// ||（文字列連結）
    Concat,
}

/// 関数呼び出し
#[derive(Debug, Clone)]
pub struct FunctionCall {
    /// 関数名
    pub name: Identifier,
    /// 引数リスト
    pub args: Vec<FunctionArg>,
    /// DISTINCT指定があるか
    pub distinct: bool,
    /// 位置情報
    pub span: Span,
}

impl AstNode for FunctionCall {
    fn span(&self) -> Span {
        self.span
    }
}

/// 関数引数
#[derive(Debug, Clone)]
pub enum FunctionArg {
    /// 式
    Expression(Expression),
    /// 修飾付きワイルドカード（table.*）
    QualifiedWildcard(Identifier),
    /// ワイルドカード（*）
    Wildcard,
}

/// CASE式
#[derive(Debug, Clone)]
pub struct CaseExpression {
    /// WHEN...THENブランチのリスト
    pub branches: Vec<(Expression, Expression)>,
    /// ELSE節
    pub else_result: Option<Box<Expression>>,
    /// 位置情報
    pub span: Span,
}

impl AstNode for CaseExpression {
    fn span(&self) -> Span {
        self.span
    }
}

/// IN式のリスト
#[derive(Debug, Clone)]
pub enum InList {
    /// 値リスト
    Values(Vec<Expression>),
    /// サブクエリ
    Subquery(Box<SelectStatement>),
}

/// IS式の値
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsValue {
    /// NULL
    Null,
    /// TRUE
    True,
    /// FALSE
    False,
    /// UNKNOWN
    Unknown,
}
