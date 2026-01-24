//! SELECT文関連のASTノード

use tsql_token::Span;

use super::base::AstNode;
use super::expression::{Expression, Identifier};

/// SELECT文
#[derive(Debug, Clone)]
pub struct SelectStatement {
    /// 位置情報
    pub span: Span,
    /// DISTINCT指定
    pub distinct: bool,
    /// TOP句
    pub top: Option<Expression>,
    /// SELECTリスト
    pub columns: Vec<SelectItem>,
    /// FROM句
    pub from: Option<FromClause>,
    /// WHERE句
    pub where_clause: Option<Expression>,
    /// GROUP BY句
    pub group_by: Vec<Expression>,
    /// HAVING句
    pub having: Option<Expression>,
    /// ORDER BY句
    pub order_by: Vec<OrderByItem>,
    /// LIMIT句（非標準）
    pub limit: Option<LimitClause>,
}

impl AstNode for SelectStatement {
    fn span(&self) -> Span {
        self.span
    }
}

/// SELECTアイテム
#[derive(Debug, Clone)]
pub enum SelectItem {
    /// 式（別名付き）
    Expression(Expression, Option<Identifier>),
    /// ワイルドカード（*）
    Wildcard,
    /// 修飾付きワイルドカード（table.*）
    QualifiedWildcard(Identifier),
}

/// FROM句
#[derive(Debug, Clone)]
pub struct FromClause {
    /// テーブル参照リスト
    pub tables: Vec<TableReference>,
    /// JOINリスト
    pub joins: Vec<Join>,
}

/// テーブル参照
#[derive(Debug, Clone)]
pub enum TableReference {
    /// 通常のテーブル
    Table {
        /// テーブル名
        name: Identifier,
        /// 別名
        alias: Option<Identifier>,
        /// 位置情報
        span: Span,
    },
    /// サブクエリ（導出テーブル）
    Subquery {
        /// サブクエリ
        query: Box<SelectStatement>,
        /// 別名
        alias: Option<Identifier>,
        /// 位置情報
        span: Span,
    },
    /// 結合済みテーブル
    Joined {
        /// JOINリスト
        joins: Vec<Join>,
        /// 位置情報
        span: Span,
    },
}

/// JOIN
#[derive(Debug, Clone)]
pub struct Join {
    /// JOIN種別
    pub join_type: JoinType,
    /// 結合するテーブル
    pub table: TableReference,
    /// ON条件
    pub on_condition: Option<Expression>,
    /// USINGカラムリスト
    pub using_columns: Vec<Identifier>,
    /// 位置情報
    pub span: Span,
}

/// JOIN種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    /// INNER JOIN
    Inner,
    /// LEFT JOIN
    Left,
    /// LEFT OUTER JOIN
    LeftOuter,
    /// RIGHT JOIN
    Right,
    /// RIGHT OUTER JOIN
    RightOuter,
    /// FULL JOIN
    Full,
    /// FULL OUTER JOIN
    FullOuter,
    /// CROSS JOIN
    Cross,
}

/// ORDER BYアイテム
#[derive(Debug, Clone)]
pub struct OrderByItem {
    /// 並べ替え式
    pub expr: Expression,
    /// 昇順の場合はtrue、降順の場合はfalse
    pub asc: bool,
}

/// LIMIT句
#[derive(Debug, Clone)]
pub struct LimitClause {
    /// 制限数
    pub limit: Expression,
    /// オフセット
    pub offset: Option<Expression>,
}
