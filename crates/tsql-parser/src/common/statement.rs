//! Common SQL AST - 文ノード
//!
//! 方言非依存のSQL文（Statement）ノードを定義する。

use crate::common::expression::CommonExpression;
use tsql_token::Span;

/// Common SQL 文
///
/// 全てのSQL方言で共通する文種別を表す。
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum CommonStatement {
    /// SELECT文
    Select(CommonSelectStatement),
    /// INSERT文
    Insert(CommonInsertStatement),
    /// UPDATE文
    Update(CommonUpdateStatement),
    /// DELETE文
    Delete(CommonDeleteStatement),
    /// 方言固有の構文（変換不可）
    DialectSpecific {
        /// 元のT-SQL構文の説明
        description: String,
        /// 位置情報
        span: Span,
    },
}

/// Common SELECT 文
#[derive(Debug, Clone, PartialEq)]
pub struct CommonSelectStatement {
    /// 位置情報
    pub span: Span,
    /// DISTINCT指定
    pub distinct: bool,
    /// SELECTリスト
    pub columns: Vec<CommonSelectItem>,
    /// FROM句（テーブル参照）
    pub from: Vec<CommonTableReference>,
    /// WHERE句
    pub where_clause: Option<CommonExpression>,
    /// GROUP BY句
    pub group_by: Vec<CommonExpression>,
    /// HAVING句
    pub having: Option<CommonExpression>,
    /// ORDER BY句
    pub order_by: Vec<CommonOrderByItem>,
    /// LIMIT句
    pub limit: Option<CommonLimitClause>,
}

/// SELECT アイテム
#[derive(Debug, Clone, PartialEq)]
pub enum CommonSelectItem {
    /// 式（別名付き）
    Expression(CommonExpression, Option<String>),
    /// ワイルドカード（*）
    Wildcard,
    /// 修飾付きワイルドカード（table.*）
    QualifiedWildcard(String),
}

/// テーブル参照
#[derive(Debug, Clone, PartialEq)]
pub enum CommonTableReference {
    /// 通常のテーブル参照
    Table {
        /// テーブル名
        name: String,
        /// 別名
        alias: Option<String>,
        /// 位置情報
        span: Span,
    },
    /// 導出テーブル（サブクエリ）
    Derived {
        /// サブクエリ
        subquery: Box<CommonSelectStatement>,
        /// 別名
        alias: Option<String>,
        /// 位置情報
        span: Span,
    },
}

/// ORDER BY アイテム
#[derive(Debug, Clone, PartialEq)]
pub struct CommonOrderByItem {
    /// 並べ替え式
    pub expr: CommonExpression,
    /// 昇順
    pub asc: bool,
}

/// LIMIT 句
#[derive(Debug, Clone, PartialEq)]
pub struct CommonLimitClause {
    /// 制限数（式）
    pub limit: CommonExpression,
    /// オフセット
    pub offset: Option<CommonExpression>,
}

/// Common INSERT 文
#[derive(Debug, Clone, PartialEq)]
pub struct CommonInsertStatement {
    /// 位置情報
    pub span: Span,
    /// テーブル名
    pub table: String,
    /// カラムリスト
    pub columns: Vec<String>,
    /// 挿入データ
    pub source: CommonInsertSource,
}

/// INSERT データソース
#[derive(Debug, Clone, PartialEq)]
pub enum CommonInsertSource {
    /// 値リスト
    Values(Vec<Vec<CommonExpression>>),
    /// サブクエリ
    Select(Box<CommonSelectStatement>),
    /// デフォルト値
    DefaultValues,
}

/// Common UPDATE 文
#[derive(Debug, Clone, PartialEq)]
pub struct CommonUpdateStatement {
    /// 位置情報
    pub span: Span,
    /// テーブル名
    pub table: String,
    /// 代入リスト
    pub assignments: Vec<CommonAssignment>,
    /// WHERE句
    pub where_clause: Option<CommonExpression>,
}

/// 代入（カラム = 値）
#[derive(Debug, Clone, PartialEq)]
pub struct CommonAssignment {
    /// カラム名
    pub column: String,
    /// 値
    pub value: CommonExpression,
}

/// Common DELETE 文
#[derive(Debug, Clone, PartialEq)]
pub struct CommonDeleteStatement {
    /// 位置情報
    pub span: Span,
    /// テーブル名
    pub table: String,
    /// WHERE句
    pub where_clause: Option<CommonExpression>,
}
