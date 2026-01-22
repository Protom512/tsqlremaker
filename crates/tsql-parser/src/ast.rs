//! AST（抽象構文木）ノードの定義
//!
//! T-SQLの抽象構文木を表現するノード型を定義する。

use tsql_token::Span;

/// 全てのASTノードの基底トレイト
pub trait AstNode {
    /// このノードのソースコード上の範囲を返す
    fn span(&self) -> Span;
}

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
    fn span(&self) -> Span {
        match self {
            Statement::Select(s) => s.span,
            Statement::Insert(s) => s.span,
            Statement::Update(s) => s.span,
            Statement::Delete(s) => s.span,
            Statement::Create(s) => s.span(),
            Statement::Declare(s) => s.span,
            Statement::Set(s) => s.span,
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
    pub body: Vec<Statement>,
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

/// バッチ区切り（GO）
#[derive(Debug, Clone)]
pub struct BatchSeparator {
    /// 位置情報
    pub span: Span,
    /// 繰り返し回数（GO NのN）
    pub repeat_count: Option<u32>,
}

impl AstNode for BatchSeparator {
    fn span(&self) -> Span {
        self.span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifier_ast_node() {
        let ident = Identifier {
            name: "test".to_string(),
            span: Span { start: 0, end: 4 },
        };
        assert_eq!(ident.span(), Span { start: 0, end: 4 });
    }

    #[test]
    fn test_select_statement_span() {
        let select = SelectStatement {
            span: Span { start: 0, end: 100 },
            distinct: false,
            top: None,
            columns: vec![],
            from: None,
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };
        assert_eq!(select.span(), Span { start: 0, end: 100 });
    }

    #[test]
    fn test_literal_span() {
        let string_lit = Literal::String("test".to_string(), Span { start: 0, end: 6 });
        assert_eq!(string_lit.span().start, 0);
        assert_eq!(string_lit.span().end, 6); // including quotes

        let number_lit = Literal::Number("123".to_string(), Span { start: 0, end: 3 });
        assert_eq!(number_lit.span().start, 0);
        assert_eq!(number_lit.span().end, 3);

        let null_lit = Literal::Null(Span { start: 0, end: 4 });
        assert_eq!(null_lit.span().start, 0);
        assert_eq!(null_lit.span().end, 4);

        let bool_lit = Literal::Boolean(true, Span { start: 0, end: 4 });
        assert_eq!(bool_lit.span().start, 0);
        assert_eq!(bool_lit.span().end, 4); // TRUE
    }
}
