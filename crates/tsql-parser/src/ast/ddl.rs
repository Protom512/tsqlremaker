//! データ定義言語（CREATE, データ型）のASTノード

use tsql_token::Span;

use super::base::AstNode;
use super::expression::{Expression, Identifier};
use super::select::SelectStatement;

/// CREATE文
#[derive(Debug, Clone)]
pub enum CreateStatement {
    /// CREATE TABLE
    Table(TableDefinition),
    /// CREATE INDEX
    Index(IndexDefinition),
    /// CREATE VIEW
    View(ViewDefinition),
    /// CREATE PROCEDURE
    Procedure(ProcedureDefinition),
}

impl AstNode for CreateStatement {
    fn span(&self) -> Span {
        match self {
            CreateStatement::Table(d) => d.span,
            CreateStatement::Index(d) => d.span,
            CreateStatement::View(d) => d.span,
            CreateStatement::Procedure(d) => d.span,
        }
    }
}

/// テーブル定義
#[derive(Debug, Clone)]
pub struct TableDefinition {
    /// 位置情報
    pub span: Span,
    /// テーブル名
    pub name: Identifier,
    /// カラム定義リスト
    pub columns: Vec<ColumnDefinition>,
    /// テーブル制約リスト
    pub constraints: Vec<TableConstraint>,
    /// 一時テーブルフラグ
    pub temporary: bool,
}

impl AstNode for TableDefinition {
    fn span(&self) -> Span {
        self.span
    }
}

/// カラム定義
#[derive(Debug, Clone)]
pub struct ColumnDefinition {
    /// カラム名
    pub name: Identifier,
    /// データ型
    pub data_type: DataType,
    /// NULL制約（Some(true)=NULL可, Some(false)=NOT NULL, None=未指定）
    pub nullability: Option<bool>,
    /// デフォルト値
    pub default_value: Option<Expression>,
    /// IDENTITY指定
    pub identity: bool,
}

/// データ型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    /// INT
    Int,
    /// SMALLINT
    SmallInt,
    /// TINYINT
    TinyInt,
    /// BIGINT
    BigInt,
    /// VARCHAR(n)
    Varchar(Option<u32>),
    /// CHAR(n)
    Char(u32),
    /// DECIMAL(p,s)
    Decimal(Option<u8>, Option<u8>),
    /// NUMERIC(p,s)
    Numeric(Option<u8>, Option<u8>),
    /// FLOAT
    Float,
    /// REAL
    Real,
    /// DOUBLE
    Double,
    /// DATE
    Date,
    /// TIME
    Time,
    /// DATETIME
    Datetime,
    /// SMALLDATETIME
    SmallDateTime,
    /// TIMESTAMP
    Timestamp,
    /// BIT
    Bit,
    /// TEXT
    Text,
    /// BINARY(n)
    Binary(u32),
    /// VARBINARY(n)
    VarBinary(Option<u32>),
    /// UNIQUEIDENTIFIER
    UniqueIdentifier,
    /// MONEY
    Money,
    /// SMALLMONEY
    SmallMoney,
}

/// テーブル制約
#[derive(Debug, Clone)]
pub enum TableConstraint {
    /// PRIMARY KEY
    PrimaryKey {
        /// カラムリスト
        columns: Vec<Identifier>,
    },
    /// FOREIGN KEY
    Foreign {
        /// カラムリスト
        columns: Vec<Identifier>,
        /// 参照先テーブル
        ref_table: Identifier,
        /// 参照先カラムリスト
        ref_columns: Vec<Identifier>,
    },
    /// UNIQUE
    Unique {
        /// カラムリスト
        columns: Vec<Identifier>,
    },
    /// CHECK
    Check {
        /// チェック式
        expr: Expression,
    },
}

/// インデックス定義
#[derive(Debug, Clone)]
pub struct IndexDefinition {
    /// 位置情報
    pub span: Span,
    /// インデックス名
    pub name: Identifier,
    /// 対象テーブル
    pub table: Identifier,
    /// カラムリスト
    pub columns: Vec<Identifier>,
    /// UNIQUE指定
    pub unique: bool,
}

impl AstNode for IndexDefinition {
    fn span(&self) -> Span {
        self.span
    }
}

/// ビュー定義
#[derive(Debug, Clone)]
pub struct ViewDefinition {
    /// 位置情報
    pub span: Span,
    /// ビュー名
    pub name: Identifier,
    /// SELECTクエリ
    pub query: Box<SelectStatement>,
}

impl AstNode for ViewDefinition {
    fn span(&self) -> Span {
        self.span
    }
}

/// プロシージャ定義
#[derive(Debug, Clone)]
pub struct ProcedureDefinition {
    /// 位置情報
    pub span: Span,
    /// プロシージャ名
    pub name: Identifier,
    /// パラメータリスト
    pub parameters: Vec<ParameterDefinition>,
    /// プロシージャ本体
    pub body: Vec<crate::Statement>,
}

impl AstNode for ProcedureDefinition {
    fn span(&self) -> Span {
        self.span
    }
}

/// パラメータ定義
#[derive(Debug, Clone)]
pub struct ParameterDefinition {
    /// パラメータ名
    pub name: Identifier,
    /// データ型
    pub data_type: DataType,
    /// デフォルト値
    pub default_value: Option<Expression>,
    /// OUTPUT指定
    pub is_output: bool,
}
