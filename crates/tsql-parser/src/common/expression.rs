//! Common SQL AST - 式ノード
//!
//! 方言非依存のSQL式（Expression）ノードを定義する。

use super::statement::CommonSelectStatement;
use tsql_token::Span;

/// Common SQL 式
///
/// 全てのSQL方言で共通する式種別を表す。
#[derive(Debug, Clone, PartialEq)]
pub enum CommonExpression {
    /// リテラル値
    Literal(CommonLiteral),
    /// 識別子
    Identifier(CommonIdentifier),
    /// カラム参照
    ColumnReference(CommonColumnReference),
    /// 単項演算子
    UnaryOp {
        /// 演算子
        op: CommonUnaryOperator,
        /// 被演算子
        expr: Box<CommonExpression>,
        /// 位置情報
        span: Span,
    },
    /// 二項演算子
    BinaryOp {
        /// 左辺
        left: Box<CommonExpression>,
        /// 演算子
        op: CommonBinaryOperator,
        /// 右辺
        right: Box<CommonExpression>,
        /// 位置情報
        span: Span,
    },
    /// 関数呼び出し
    FunctionCall(CommonFunctionCall),
    /// CASE式
    Case(CommonCaseExpression),
    /// IN式
    In {
        /// 対象の式
        expr: Box<CommonExpression>,
        /// 値リストまたはサブクエリ
        list: CommonInList,
        /// NOT INの場合はtrue
        negated: bool,
        /// 位置情報
        span: Span,
    },
    /// BETWEEN式
    Between {
        /// 対象の式
        expr: Box<CommonExpression>,
        /// 下限値
        low: Box<CommonExpression>,
        /// 上限値
        high: Box<CommonExpression>,
        /// NOT BETWEENの場合はtrue
        negated: bool,
        /// 位置情報
        span: Span,
    },
    /// LIKE式
    Like {
        /// 対象の式
        expr: Box<CommonExpression>,
        /// パターン
        pattern: Box<CommonExpression>,
        /// ESCAPE句（省略可能）
        escape: Option<Box<CommonExpression>>,
        /// NOT LIKEの場合はtrue
        negated: bool,
        /// 位置情報
        span: Span,
    },
    /// IS NULL / IS NOT NULL 式
    IsNull {
        /// 対象の式
        expr: Box<CommonExpression>,
        /// IS NOT NULLの場合はtrue
        negated: bool,
        /// 位置情報
        span: Span,
    },
    /// スカラサブクエリ
    Subquery {
        /// サブクエリ
        query: Box<CommonSelectStatement>,
        /// 位置情報
        span: Span,
    },
    /// EXISTS式
    Exists {
        /// サブクエリ
        query: Box<CommonSelectStatement>,
        /// NOT EXISTSの場合はtrue
        negated: bool,
        /// 位置情報
        span: Span,
    },
}

/// Common リテラル値
#[derive(Debug, Clone, PartialEq)]
pub enum CommonLiteral {
    /// 文字列リテラル
    String(String),
    /// 数値リテラル（整数）
    Integer(i64),
    /// 数値リテラル（浮動小数点）
    Float(f64),
    /// NULLリテラル
    Null,
    /// 真理値リテラル
    Boolean(bool),
}

/// Common 識別子
#[derive(Debug, Clone, PartialEq)]
pub struct CommonIdentifier {
    /// 識別子名
    pub name: String,
}

/// Common カラム参照
#[derive(Debug, Clone, PartialEq)]
pub struct CommonColumnReference {
    /// テーブル修飾子（省略可能）
    pub table: Option<String>,
    /// カラム名
    pub column: String,
}

/// Common 単項演算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonUnaryOperator {
    /// +（正号）
    Plus,
    /// -（負号）
    Minus,
    /// NOT（論理否定）
    Not,
}

/// Common 二項演算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonBinaryOperator {
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
    /// != または <>（不等価）
    Ne,
    /// <（より小さい）
    Lt,
    /// <=（以下）
    Le,
    /// >（より大きい）
    Gt,
    /// >=（以上）
    Ge,
    /// AND（論理積）
    And,
    /// OR（論理和）
    Or,
    /// ||（文字列連結）
    Concat,
}

/// Common 関数呼び出し
#[derive(Debug, Clone, PartialEq)]
pub struct CommonFunctionCall {
    /// 関数名
    pub name: String,
    /// 引数リスト
    pub args: Vec<CommonExpression>,
    /// DISTINCT指定があるか
    pub distinct: bool,
}

/// Common CASE 式
#[derive(Debug, Clone, PartialEq)]
pub struct CommonCaseExpression {
    /// WHEN...THENブランチのリスト
    pub branches: Vec<(CommonExpression, CommonExpression)>,
    /// ELSE節
    pub else_result: Option<Box<CommonExpression>>,
}

/// Common IN式の値リストまたはサブクエリ
#[derive(Debug, Clone, PartialEq)]
pub enum CommonInList {
    /// 値リスト
    Values(Vec<CommonExpression>),
    /// サブクエリ
    Subquery(Box<CommonSelectStatement>),
}
