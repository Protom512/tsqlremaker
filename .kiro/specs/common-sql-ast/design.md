# Common SQL AST - Design

## 概要

方言非依存の共通 SQL AST の設計について記述する。

## アーキテクチャ

### 全体構成

```
┌─────────────────────────────────────────────────────────────┐
│                    Common SQL AST                           │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Statement   │  │ Expression  │  │  DataType   │          │
│  │   Nodes     │  │   Nodes     │  │   Nodes     │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
│         │                │                 │                 │
│         └────────────────┴─────────────────┘                 │
│                           │                                  │
│                    ┌──────┴──────┐                           │
│                    │  Visitor    │                           │
│                    │  Trait      │                           │
│                    └─────────────┘                           │
└─────────────────────────────────────────────────────────────┘
```

### モジュール構成

```
common-sql/
├── Cargo.toml
├── src/
│   ├── lib.rs              # 公開APIの再エクスポート
│   ├── mod.rs              # モジュール宣言
│   ├── ast/
│   │   ├── mod.rs          # AST モジュール
│   │   ├── statement.rs    # Statement ノード
│   │   ├── expression.rs   # Expression ノード
│   │   ├── datatype.rs     # DataType ノード
│   │   ├── clause.rs       # クエリ句（FROM, WHERE 等）
│   │   └── span.rs         # 位置情報
│   └── visitor.rs          # Visitor trait
└── tests/
    ├── ast_tests.rs        # AST 単体テスト
    └── visitor_tests.rs    # Visitor テスト
```

## データ構造設計

### Span（位置情報）

```rust
/// ソースコード内の位置情報
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}
```

### Statement ノード

```rust
/// SQL 文の種類
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    CreateTable(CreateTableStatement),
    AlterTable(AlterTableStatement),
    DropTable(DropTableStatement),
    CreateIndex(CreateIndexStatement),
    DropIndex(DropIndexStatement),
}

/// SELECT 文
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub span: Span,
    pub with: Option<WithClause>,           // CTE (WITH 句)
    pub projection: Vec<SelectItem>,        // SELECT リスト
    pub from: Option<TableFactor>,          // FROM 句
    pub where_clause: Option<Expression>,   // WHERE 句
    pub group_by: Option<GroupByClause>,    // GROUP BY 句
    pub having: Option<Expression>,         // HAVING 句
    pub order_by: Option<OrderByClause>,    // ORDER BY 句
    pub limit: Option<LimitClause>,         // LIMIT 句
}

/// SELECT リストの項目
#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    Expression {
        expr: Expression,
        alias: Option<Identifier>,
    },
    QualifiedWildcard {
        table: Identifier,
    },
    Wildcard,  // *
}
```

### Expression ノード

```rust
/// 式の種類
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // リテラル
    Literal(Literal),

    // 識別子
    Identifier(Identifier),
    QualifiedIdentifier {
        table: Identifier,
        column: Identifier,
    },

    // 算術演算
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expression>,
    },

    // 論理演算
    LogicalOp {
        left: Box<Expression>,
        op: LogicalOperator,
        right: Box<Expression>,
    },

    // 比較演算
    Comparison {
        left: Box<Expression>,
        op: ComparisonOperator,
        right: Box<Expression>,
    },

    // 関数呼び出し
    Function {
        name: Identifier,
        args: Vec<Expression>,
        distinct: bool,
    },

    // CASE 式
    Case {
        operand: Option<Box<Expression>>,
        conditions: Vec<(Expression, Expression)>,
        else_result: Option<Box<Expression>>,
    },

    // サブクエリ
    Subquery(Box<SelectStatement>),

    // EXISTS / NOT EXISTS
    Exists {
        subquery: Box<SelectStatement>,
        negated: bool,
    },

    // IN
    In {
        expr: Box<Expression>,
        list: InList,
        negated: bool,
    },

    // BETWEEN
    Between {
        expr: Box<Expression>,
        low: Box<Expression>,
        high: Box<Expression>,
        negated: bool,
    },

    // CAST
    Cast {
        expr: Box<Expression>,
        data_type: DataType,
    },

    // NULL チェック
    IsNull {
        expr: Box<Expression>,
        negated: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Identifier {
    pub value: String,
    pub quoted: bool,  // クォートされた識別子かどうか
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add, Sub, Mul, Div, Mod,
    Concat,  // ||
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Plus, Minus, Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOperator {
    And, Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    Eq, Ne, Lt, Le, Gt, Ge,
    Like, NotLike,
    ILike, NotILike,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InList {
    Values(Vec<Expression>),
    Subquery(Box<SelectStatement>),
}
```

### DataType ノード

```rust
/// データ型
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    // 整数型
    TinyInt,
    SmallInt,
    Int,
    BigInt,

    // 小数型
    Decimal { precision: Option<u8>, scale: Option<u8> },
    Numeric { precision: Option<u8>, scale: Option<u8> },
    Real,
    DoublePrecision,

    // 文字列型
    Char { length: Option<u64> },
    VarChar { length: Option<u64> },
    Text,
    NChar { length: Option<u64> },
    NVarChar { length: Option<u64> },
    NText,

    // 日時型
    Date,
    Time { precision: Option<u8> },
    DateTime { precision: Option<u8> },
    Timestamp { precision: Option<u8> },

    // バイナリ型
    Binary { length: Option<u64> },
    VarBinary { length: Option<u64> },
    Blob,

    // その他
    Boolean,
    Uuid,
    Json,
}
```

### JOIN 表現

```rust
/// JOIN 種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

/// JOIN
#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    pub span: Span,
    pub join_type: JoinType,
    pub table: TableFactor,
    pub condition: JoinCondition,
    pub lateral: bool,  // LATERAL キーワード
}

/// JOIN 条件
#[derive(Debug, Clone, PartialEq)]
pub enum JoinCondition {
    On(Expression),
    Using(Vec<Identifier>),
    Natural,  // NATURAL JOIN
}

/// テーブル参照
#[derive(Debug, Clone, PartialEq)]
pub enum TableFactor {
    Table {
        name: QualifiedName,
        alias: Option<TableAlias>,
    },
    Derived {
        subquery: Box<SelectStatement>,
        alias: Option<TableAlias>,
    },
    Join(Box<Join>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct QualifiedName {
    pub schema: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableAlias {
    pub name: String,
    pub columns: Vec<String>,  // 列別名
}
```

### クエリ句

```rust
/// GROUP BY 句
#[derive(Debug, Clone, PartialEq)]
pub struct GroupByClause {
    pub span: Span,
    pub items: Vec<GroupByItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GroupByItem {
    Expression(Expression),
    Rollup(Vec<Expression>),
    Cube(Vec<Expression>),
    GroupingSets(Vec<Vec<Expression>>),
}

/// ORDER BY 句
#[derive(Debug, Clone, PartialEq)]
pub struct OrderByClause {
    pub span: Span,
    pub items: Vec<OrderByItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderByItem {
    pub expr: Expression,
    pub direction: Option<SortDirection>,
    pub nulls: Option<NullOrdering>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullOrdering {
    NullsFirst,
    NullsLast,
}

/// LIMIT 句
#[derive(Debug, Clone, PartialEq)]
pub struct LimitClause {
    pub span: Span,
    pub limit: Expression,
    pub offset: Option<Expression>,
}

/// WITH 句（CTE）
#[derive(Debug, Clone, PartialEq)]
pub struct WithClause {
    pub recursive: bool,
    pub ctes: Vec<Cte>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    pub name: String,
    pub columns: Vec<String>,
    pub query: Box<SelectStatement>,
    pub materialized: Option<bool>,  // MATERIALIZED / NOT MATERIALIZED
}
```

### その他の Statement

```rust
/// INSERT 文
#[derive(Debug, Clone, PartialEq)]
pub struct InsertStatement {
    pub span: Span,
    pub table: QualifiedName,
    pub columns: Vec<Identifier>,
    pub source: InsertSource,
    pub on_conflict: Option<OnConflict>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertSource {
    Values(Vec<Vec<Expression>>),
    Select(Box<SelectStatement>),
}

/// UPDATE 文
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStatement {
    pub span: Span,
    pub table: TableFactor,
    pub assignments: Vec<Assignment>,
    pub from: Option<TableFactor>,
    pub where_clause: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub column: Identifier,
    pub value: Expression,
}

/// DELETE 文
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStatement {
    pub span: Span,
    pub table: TableFactor,
    pub using: Option<Vec<TableFactor>>,
    pub where_clause: Option<Expression>,
}

/// CREATE TABLE 文
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableStatement {
    pub span: Span,
    pub if_not_exists: bool,
    pub name: QualifiedName,
    pub columns: Vec<ColumnDef>,
    pub constraints: Vec<TableConstraint>,
    pub options: TableOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: Identifier,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<Expression>,
    pub constraints: Vec<ColumnConstraint>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConstraint {
    PrimaryKey,
    Unique,
    Check(Expression),
    References {
        table: QualifiedName,
        columns: Vec<String>,
    },
    AutoIncrement,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableConstraint {
    PrimaryKey { columns: Vec<Identifier> },
    Unique { name: Option<String>, columns: Vec<Identifier> },
    ForeignKey {
        name: Option<String>,
        columns: Vec<Identifier>,
        ref_table: QualifiedName,
        ref_columns: Vec<Identifier>,
    },
    Check { name: Option<String>, expr: Expression },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableOptions {
    pub engine: Option<String>,
    pub charset: Option<String>,
    pub collation: Option<String>,
    pub comment: Option<String>,
}
```

## Visitor パターン設計

### Visitor Trait

```rust
/// Visitor trait - SQL 生成用
pub trait Visitor: Sized {
    /// 訪問の結果型
    type Output;

    /// Statement を訪問
    fn visit_statement(&mut self, stmt: &Statement) -> Self::Output {
        match stmt {
            Statement::Select(s) => self.visit_select_statement(s),
            Statement::Insert(s) => self.visit_insert_statement(s),
            Statement::Update(s) => self.visit_update_statement(s),
            Statement::Delete(s) => self.visit_delete_statement(s),
            Statement::CreateTable(s) => self.visit_create_table_statement(s),
            Statement::AlterTable(s) => self.visit_alter_table_statement(s),
            Statement::DropTable(s) => self.visit_drop_table_statement(s),
            Statement::CreateIndex(s) => self.visit_create_index_statement(s),
            Statement::DropIndex(s) => self.visit_drop_index_statement(s),
        }
    }

    /// 各 Statement タイプの訪問メソッド
    fn visit_select_statement(&mut self, stmt: &SelectStatement) -> Self::Output;
    fn visit_insert_statement(&mut self, stmt: &InsertStatement) -> Self::Output;
    fn visit_update_statement(&mut self, stmt: &UpdateStatement) -> Self::Output;
    fn visit_delete_statement(&mut self, stmt: &DeleteStatement) -> Self::Output;
    fn visit_create_table_statement(&mut self, stmt: &CreateTableStatement) -> Self::Output;
    fn visit_alter_table_statement(&mut self, stmt: &AlterTableStatement) -> Self::Output;
    fn visit_drop_table_statement(&mut self, stmt: &DropTableStatement) -> Self::Output;
    fn visit_create_index_statement(&mut self, stmt: &CreateIndexStatement) -> Self::Output;
    fn visit_drop_index_statement(&mut self, stmt: &DropIndexStatement) -> Self::Output;

    /// Expression を訪問
    fn visit_expression(&mut self, expr: &Expression) -> Self::Output {
        match expr {
            Expression::Literal(l) => self.visit_literal(l),
            Expression::Identifier(i) => self.visit_identifier(i),
            Expression::QualifiedIdentifier { table, column } => {
                self.visit_qualified_identifier(table, column)
            }
            Expression::BinaryOp { left, op, right } => {
                self.visit_binary_op(left, *op, right)
            }
            Expression::UnaryOp { op, expr } => self.visit_unary_op(*op, expr),
            Expression::LogicalOp { left, op, right } => {
                self.visit_logical_op(left, *op, right)
            }
            Expression::Comparison { left, op, right } => {
                self.visit_comparison(left, *op, right)
            }
            Expression::Function { name, args, distinct } => {
                self.visit_function(name, args, *distinct)
            }
            Expression::Case { operand, conditions, else_result } => {
                self.visit_case(operand, conditions, else_result)
            }
            Expression::Subquery(sq) => self.visit_subquery(sq),
            Expression::Exists { subquery, negated } => {
                self.visit_exists(subquery, *negated)
            }
            Expression::In { expr, list, negated } => {
                self.visit_in(expr, list, *negated)
            }
            Expression::Between { expr, low, high, negated } => {
                self.visit_between(expr, low, high, *negated)
            }
            Expression::Cast { expr, data_type } => self.visit_cast(expr, data_type),
            Expression::IsNull { expr, negated } => self.visit_is_null(expr, *negated),
        }
    }

    /// 各 Expression タイプの訪問メソッド
    fn visit_literal(&mut self, literal: &Literal) -> Self::Output;
    fn visit_identifier(&mut self, ident: &Identifier) -> Self::Output;
    fn visit_qualified_identifier(&mut self, table: &Identifier, column: &Identifier) -> Self::Output;
    fn visit_binary_op(&mut self, left: &Expression, op: BinaryOperator, right: &Expression) -> Self::Output;
    fn visit_unary_op(&mut self, op: UnaryOperator, expr: &Expression) -> Self::Output;
    fn visit_logical_op(&mut self, left: &Expression, op: LogicalOperator, right: &Expression) -> Self::Output;
    fn visit_comparison(&mut self, left: &Expression, op: ComparisonOperator, right: &Expression) -> Self::Output;
    fn visit_function(&mut self, name: &Identifier, args: &[Expression], distinct: bool) -> Self::Output;
    fn visit_case(&mut self, operand: &Option<Box<Expression>>, conditions: &[(Expression, Expression)], else_result: &Option<Box<Expression>>) -> Self::Output;
    fn visit_subquery(&mut self, subquery: &SelectStatement) -> Self::Output;
    fn visit_exists(&mut self, subquery: &SelectStatement, negated: bool) -> Self::Output;
    fn visit_in(&mut self, expr: &Expression, list: &InList, negated: bool) -> Self::Output;
    fn visit_between(&mut self, expr: &Expression, low: &Expression, high: &Expression, negated: bool) -> Self::Output;
    fn visit_cast(&mut self, expr: &Expression, data_type: &DataType) -> Self::Output;
    fn visit_is_null(&mut self, expr: &Expression, negated: bool) -> Self::Output;

    /// DataType を訪問
    fn visit_data_type(&mut self, data_type: &DataType) -> Self::Output;
}
```

### Visitable Trait

```rust
/// Visitable trait - AST ノードが実装
pub trait Visitable {
    /// Visitor を受け入れる
    fn accept<V: Visitor>(&self, visitor: &mut V) -> V::Output;
}

// Statement に Visitable を実装
impl Visitable for Statement {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> V::Output {
        visitor.visit_statement(self)
    }
}

// Expression に Visitable を実装
impl Visitable for Expression {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> V::Output {
        visitor.visit_expression(self)
    }
}

// DataType に Visitable を実装
impl Visitable for DataType {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> V::Output {
        visitor.visit_data_type(self)
    }
}
```

## 依存関係図

```
┌─────────────────────────────────────────────────────────────────┐
│                         common-sql                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │                     ast モジュール                      │    │
│  │  ┌──────────┐  ┌────────────┐  ┌──────────────┐        │    │
│  │  │ statement│  │ expression │  │  datatype    │        │    │
│  │  └────┬─────┘  └─────┬──────┘  └──────┬───────┘        │    │
│  │       │              │                 │                │    │
│  │       └──────────────┴─────────────────┘                │    │
│  │                      │                                  │    │
│  │              ┌───────┴────────┐                         │    │
│  │              │   span.rs      │                         │    │
│  │              └────────────────┘                         │    │
│  └────────────────────────────────────────────────────────┐    │
│                                                                  │
│  ┌────────────────────────────────────────────────────────┐    │
│  │                   visitor.rs                            │    │
│  │  ┌──────────┐         ┌──────────────────┐             │    │
│  │  │ Visitor  │◄────────│   Visitable      │             │    │
│  │  │  Trait   │         │    Trait         │             │    │
│  │  └──────────┘         └──────────────────┘             │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## 拡張性考慮

### 新しい Statement の追加

1. `Statement` enum に新しい variant を追加
2. `Visitor::visit_statement` に match arm を追加
3. 新しい訪問メソッドを `Visitor` trait に追加

### 新しい Expression の追加

1. `Expression` enum に新しい variant を追加
2. `Visitor::visit_expression` に match arm を追加
3. 新しい訪問メソッドを `Visitor` trait に追加

### 方言固有機能の扱い

```rust
// 方言固有のオプションを表現するパターン
#[derive(Debug, Clone, PartialEq)]
pub struct DialectOptions {
    pub mysql: Option<MySqlOptions>,
    pub postgres: Option<PostgresOptions>,
    pub tsql: Option<TSqlOptions>,
}

// 各方言が特別な機能を持つ場合
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableStatement {
    pub span: Span,
    pub if_not_exists: bool,
    pub name: QualifiedName,
    pub columns: Vec<ColumnDef>,
    pub constraints: Vec<TableConstraint>,
    pub options: TableOptions,
    pub dialect_options: DialectOptions,  // 拡張ポイント
}
```

## 実装時の注意事項

1. **derive マクロ**: `Debug`, `Clone`, `PartialEq` を全ての構造体に適用
2. **不変性**: 構造体のフィールドは pub にせず、必要に応じて getter メソッドを提供
3. **エラー処理**: Visitor の `Output` 型は `Result<String, E>` にする
4. **ボックス化**: 再帰的なデータ構造では `Box` を使用

## テスト戦略

1. **構造テスト**: 各ノードが正しく構築できるか
2. **Visitor テスト**: ダミー Visitor で全ノードを訪問可能か
3. **等価性テスト**: 同じ内容のノードが等価と判定されるか
4. **クローンテスト**: 全ノードがクローン可能か
