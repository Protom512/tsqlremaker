# PostgreSQL Emitter - Design

## 概要

PostgreSQL Emitter の設計について記述する。

## アーキテクチャ

### 全体構成

```
┌─────────────────────────────────────────────────────────────┐
│                   PostgreSQL Emitter                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │              PostgreSqlEmitter (Visitor)               │ │
│  │  ┌──────────────────────────────────────────────────┐ │ │
│  │  │           visit_* メソッド                        │ │ │
│  │  │  - visit_statement()   → SELECT, INSERT, etc.     │ │ │
│  │  │  - visit_expression()  → 式の生成                 │ │ │
│  │  │  - visit_data_type()   → 型のマッピング           │ │ │
│  │  └──────────────────────────────────────────────────┘ │ │
│  └────────────────────────────────────────────────────────┘ │
│                           │                                  │
│                           ▼                                  │
│  ┌────────────────────────────────────────────────────────┐ │
│  │                 変換ルールモジュール                     │ │
│  │  ┌──────────────────────────────────────────────────┐ │ │
│  │  │  DataTypeMapper      → データ型変換              │ │ │
│  │  │  FunctionMapper      → 関数変換                  │ │ │
│  │  │  SyntaxConverter     → 構文変換                  │ │ │
│  │  │  IdentifierQuoter    → 識別子のエスケープ        │ │ │
│  │  └──────────────────────────────────────────────────┘ │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### モジュール構成

```
postgresql-emitter/
├── Cargo.toml
├── src/
│   ├── lib.rs              # 公開API
│   ├── emitter.rs          # PostgreSqlEmitter 構造体
│   ├── context.rs          # EmissionContext (インデント等)
│   ├── mappers/
│   │   ├── mod.rs          # マッパーモジュール
│   │   ├── datatype.rs     # DataTypeMapper
│   │   ├── function.rs     # FunctionMapper
│   │   └── identifier.rs   # IdentifierQuoter
│   ├── visitors/
│   │   ├── mod.rs          # Visitor 実装
│   │   ├── statement.rs    # Statement 訪問者
│   │   ├── expression.rs   # Expression 訪問者
│   │   └── clause.rs       # Clause 訪問者
│   └── error.rs            # EmitError
└── tests/
    ├── emitter_tests.rs    # 単体テスト
    └── fixtures/
        └── samples.sql     # テスト用 SQL
```

## データ構造設計

### PostgreSqlEmitter

```rust
use common_sql::{
    Visitor, Visitable,
    ast::*,
};
use crate::context::EmissionContext;
use crate::error::{EmitError, Result};

/// PostgreSQL Emitter
pub struct PostgreSqlEmitter {
    context: EmissionContext,
    /// 変換オプション
    options: EmitterOptions,
}

#[derive(Debug, Clone)]
pub struct EmitterOptions {
    /// 予約語を大文字にする
    pub uppercase_keywords: bool,
    /// 識別子を常にクォートする
    pub quote_identifiers: bool,
    /// インデントサイズ
    pub indent_size: usize,
    /// サポートされない機能で警告を出す
    pub warn_unsupported: bool,
}

impl Default for EmitterOptions {
    fn default() -> Self {
        Self {
            uppercase_keywords: true,
            quote_identifiers: false,
            indent_size: 2,
            warn_unsupported: true,
        }
    }
}

impl PostgreSqlEmitter {
    pub fn new() -> Self {
        Self {
            context: EmissionContext::new(),
            options: EmitterOptions::default(),
        }
    }

    pub fn with_options(options: EmitterOptions) -> Self {
        Self {
            context: EmissionContext::new(),
            options,
        }
    }

    /// Statement を PostgreSQL SQL に変換
    pub fn emit(&mut self, stmt: &Statement) -> Result<String> {
        stmt.accept(self)
    }

    /// 複数の Statement を変換
    pub fn emit_batch(&mut self, stmts: &[Statement]) -> Result<String> {
        let mut results = Vec::new();
        for stmt in stmts {
            results.push(self.emit(stmt)?);
        }
        Ok(results.join(";\n\n") + ";")
    }
}

impl Default for PostgreSqlEmitter {
    fn default() -> Self {
        Self::new()
    }
}
```

### EmissionContext

```rust
/// SQL 生成時のコンテキスト
pub struct EmissionContext {
    /// 現在のインデントレベル
    indent_level: usize,
    /// 生成された SQL のバッファ
    buffer: String,
    /// 警告リスト
    warnings: Vec<EmitWarning>,
}

#[derive(Debug, Clone)]
pub struct EmitWarning {
    pub message: String,
    pub span: Option<Span>,
}

impl EmissionContext {
    pub fn new() -> Self {
        Self {
            indent_level: 0,
            buffer: String::new(),
            warnings: Vec::new(),
        }
    }

    /// インデントを追加
    pub fn indent(&mut self, size: usize) {
        self.indent_level += 1;
    }

    /// インデントを減少
    pub fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    /// インデント文字列を取得
    pub fn indent_str(&self, size: usize) -> String {
        " ".repeat(self.indent_level * size)
    }

    /// 文字列を追加
    pub fn push(&mut self, s: &str) {
        self.buffer.push_str(s);
    }

    /// 改行を追加
    pub fn push_line(&mut self, s: &str) {
        self.buffer.push_str(s);
        self.buffer.push('\n');
    }

    /// 警告を追加
    pub fn add_warning(&mut self, warning: EmitWarning) {
        self.warnings.push(warning);
    }

    /// 生成された SQL を取得
    pub fn into_string(self) -> String {
        self.buffer
    }

    /// 警告を取得
    pub fn warnings(&self) -> &[EmitWarning] {
        &self.warnings
    }
}
```

### EmitError

```rust
use thiserror::Error;

/// Emission エラー
#[derive(Debug, Error)]
pub enum EmitError {
    #[error("Unsupported feature: {0}")]
    Unsupported(String),

    #[error("Cannot map data type: {0:?}")]
    UnsupportedDataType(DataType),

    #[error("Cannot convert function: {0}")]
    UnsupportedFunction(String),

    #[error("Syntax error at {span:?}: {message}")]
    SyntaxError {
        message: String,
        span: Span,
    },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Format error: {0}")]
    FormatError(#[from] std::fmt::Error),
}

pub type Result<T> = std::result::Result<T, EmitError>;
```

## Mapper 設計

### DataTypeMapper

```rust
/// データ型マッパー
pub struct DataTypeMapper;

impl DataTypeMapper {
    /// Common SQL DataType を PostgreSQL 型にマッピング
    pub fn map(data_type: &DataType) -> Result<String> {
        match data_type {
            DataType::TinyInt => Ok("SMALLINT".to_string()),
            DataType::SmallInt => Ok("SMALLINT".to_string()),
            DataType::Int => Ok("INTEGER".to_string()),
            DataType::BigInt => Ok("BIGINT".to_string()),

            DataType::Decimal { precision, scale } => {
                match (precision, scale) {
                    (Some(p), Some(s)) => Ok(format!("NUMERIC({},{})", p, s)),
                    (Some(p), None) => Ok(format!("NUMERIC({})", p)),
                    (None, None) => Ok("NUMERIC".to_string()),
                }
            }
            DataType::Numeric { precision, scale } => {
                // Decimal と同じ
                match (precision, scale) {
                    (Some(p), Some(s)) => Ok(format!("NUMERIC({},{})", p, s)),
                    (Some(p), None) => Ok(format!("NUMERIC({})", p)),
                    (None, None) => Ok("NUMERIC".to_string()),
                }
            }
            DataType::Real => Ok("REAL".to_string()),
            DataType::DoublePrecision => Ok("DOUBLE PRECISION".to_string()),

            DataType::Char { length } => {
                match length {
                    Some(l) => Ok(format!("CHAR({})", l)),
                    None => Ok("CHAR".to_string()),
                }
            }
            DataType::VarChar { length } => {
                match length {
                    Some(l) => Ok(format!("VARCHAR({})", l)),
                    None => Ok("VARCHAR".to_string()),
                }
            }
            DataType::Text => Ok("TEXT".to_string()),
            DataType::NChar { length } => {
                // PostgreSQL は UTF-8
                match length {
                    Some(l) => Ok(format!("CHAR({})", l)),
                    None => Ok("CHAR".to_string()),
                }
            }
            DataType::NVarChar { length } => {
                // PostgreSQL は UTF-8
                match length {
                    Some(l) => Ok(format!("VARCHAR({})", l)),
                    None => Ok("VARCHAR".to_string()),
                }
            }
            DataType::NText => Ok("TEXT".to_string()),

            DataType::Date => Ok("DATE".to_string()),
            DataType::Time { precision } => {
                match precision {
                    Some(p) => Ok(format!("TIME({})", p)),
                    None => Ok("TIME".to_string()),
                }
            }
            DataType::DateTime { precision } => {
                // PostgreSQL は TIMESTAMP を使用
                match precision {
                    Some(p) => Ok(format!("TIMESTAMP({})", p)),
                    None => Ok("TIMESTAMP".to_string()),
                }
            }
            DataType::Timestamp { precision } => {
                match precision {
                    Some(p) => Ok(format!("TIMESTAMP({})", p)),
                    None => Ok("TIMESTAMP".to_string()),
                }
            }

            DataType::Binary { .. } | DataType::VarBinary { .. } | DataType::Blob => {
                Ok("BYTEA".to_string())
            }

            DataType::Boolean => Ok("BOOLEAN".to_string()),
            DataType::Uuid => Ok("UUID".to_string()),
            DataType::Json => Ok("JSONB".to_string()),
        }
    }
}
```

### FunctionMapper

```rust
/// 関数マッパー
pub struct FunctionMapper;

impl FunctionMapper {
    /// T-SQL 関数名を PostgreSQL 関数にマッピング
    pub fn map_function_name(name: &str) -> Option<String> {
        match name.to_uppercase().as_str() {
            "GETDATE" => Some("CURRENT_TIMESTAMP".to_string()),
            "LEN" => Some("LENGTH".to_string()),
            "ISNULL" => Some("COALESCE".to_string()),
            "CHARINDEX" => Some("STRPOS".to_string()),
            "DATEADD" => None, // 特別な処理が必要
            "DATEDIFF" => None, // 特別な処理が必要
            "DATEPART" => None, // 特別な処理が必要
            "LEFT" => Some("SUBSTRING".to_string()), // 引数順の変換が必要
            "RIGHT" => None, // 特別な処理が必要
            "GETUTCDATE" => Some("CURRENT_TIMESTAMP AT TIME ZONE 'UTC'".to_string()),
            "NEWID" => Some("gen_random_uuid()".to_string()),
            _ => None, // 変換不要（PostgreSQL にも同名関数がある場合）
        }
    }

    /// 関数呼び出しを変換
    pub fn map_function_call(
        name: &Identifier,
        args: &[Expression],
    ) -> Result<(String, Vec<Expression>)> {
        let func_name = name.value.to_uppercase();

        match func_name.as_str() {
            "LEFT" => {
                // LEFT(s, n) → SUBSTRING(s, 1, n)
                if args.len() == 2 {
                    Ok(("SUBSTRING".to_string(), vec![
                        args[0].clone(),
                        Expression::Literal(Literal::Integer(1)),
                        args[1].clone(),
                    ]))
                } else {
                    Err(EmitError::UnsupportedFunction("LEFT with wrong arity".to_string()))
                }
            }
            "RIGHT" => {
                // RIGHT(s, n) → SUBSTRING(s, LENGTH(s) - n + 1, n)
                if args.len() == 2 {
                    Ok(("SUBSTRING".to_string(), vec![
                        args[0].clone(),
                        Expression::BinaryOp {
                            left: Box::new(Expression::Function {
                                name: Identifier { value: "LENGTH".to_string(), quoted: false },
                                args: vec![args[0].clone()],
                                distinct: false,
                            }),
                            op: BinaryOperator::Sub,
                            right: Box::new(Expression::BinaryOp {
                                left: Box::new(args[1].clone()),
                                op: BinaryOperator::Add,
                                right: Box::new(Expression::Literal(Literal::Integer(1))),
                            }),
                        },
                        args[1].clone(),
                    ]))
                } else {
                    Err(EmitError::UnsupportedFunction("RIGHT with wrong arity".to_string()))
                }
            }
            "DATEADD" => {
                // DATEADD(day, n, date) → date + interval 'n days'
                // これは呼び出し元で特別に処理する必要がある
                Err(EmitError::Unsupported(
                    "DATEADD requires special handling in expression visitor".to_string()
                ))
            }
            _ => {
                // 名前のマッピングを試みる
                let mapped_name = Self::map_function_name(&name.value)
                    .unwrap_or_else(|| name.value.clone());
                Ok((mapped_name, args.to_vec()))
            }
        }
    }
}
```

### IdentifierQuoter

```rust
/// PostgreSQL 識別子クォーター
pub struct IdentifierQuoter;

impl IdentifierQuoter {
    /// PostgreSQL の予約語セット
    fn is_reserved_word(word: &str) -> bool {
        matches!(word.to_uppercase().as_str(),
            "SELECT" | "FROM" | "WHERE" | "INSERT" | "UPDATE" | "DELETE" |
            "CREATE" | "DROP" | "ALTER" | "TABLE" | "INDEX" | "VIEW" |
            "JOIN" | "INNER" | "OUTER" | "LEFT" | "RIGHT" | "FULL" | "CROSS" |
            "ON" | "USING" | "GROUP" | "BY" | "ORDER" | "HAVING" | "LIMIT" |
            "OFFSET" | "AND" | "OR" | "NOT" | "NULL" | "TRUE" | "FALSE" |
            "AS" | "DISTINCT" | "CASE" | "WHEN" | "THEN" | "ELSE" | "END" |
            "UNION" | "INTERSECT" | "EXCEPT" | "INTO" | "VALUES" | "SET" |
            "PRIMARY" | "FOREIGN" | "KEY" | "REFERENCES" | "CHECK" | "UNIQUE" |
            "DEFAULT" | "CONSTRAINT" | "CASCADE" | "RESTRICT" | "NO" | "ACTION" |
            "TIMESTAMP" | "DATE" | "TIME" | "INTERVAL" | "YEAR" | "MONTH" | "DAY"
        )
    }

    /// 識別子をクォートする必要があるか判定
    pub fn needs_quoting(ident: &str, quote_all: bool) -> bool {
        if quote_all {
            return true;
        }

        // 小文字で始まり、英数字とアンダースコアのみならクォート不要
        if ident.chars().next().map_or(false, |c| c.is_ascii_lowercase()) {
            ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        } else {
            true
        }
    }

    /// 識別子をクォート
    pub fn quote(ident: &str, quote_all: bool) -> String {
        if Self::needs_quoting(ident, quote_all) {
            format!("\"{}\"", ident.replace('"', "\"\""))
        } else {
            ident.to_string()
        }
    }
}
```

## Visitor 実装

### Statement Visitor

```rust
impl Visitor for PostgreSqlEmitter {
    type Output = Result<String>;

    fn visit_select_statement(&mut self, stmt: &SelectStatement) -> Self::Output {
        let mut sql = String::new();

        // WITH 句
        if let Some(with_clause) = &stmt.with {
            sql.push_str(&self.visit_with_clause(with_clause)?);
            sql.push(' ');
        }

        // SELECT
        sql.push_str("SELECT ");

        // DISTINCT
        // TODO: SelectItem に distinct フラグが必要か確認

        // プロジェクション
        sql.push_str(&self.visit_select_items(&stmt.projection)?);

        // FROM
        if let Some(from) = &stmt.from {
            sql.push_str("\nFROM ");
            sql.push_str(&self.visit_table_factor(from)?);
        }

        // WHERE
        if let Some(where_clause) = &stmt.where_clause {
            sql.push_str("\nWHERE ");
            sql.push_str(&where_clause.accept(self)?);
        }

        // GROUP BY
        if let Some(group_by) = &stmt.group_by {
            sql.push_str(&self.visit_group_by_clause(group_by)?);
        }

        // HAVING
        if let Some(having) = &stmt.having {
            sql.push_str("\nHAVING ");
            sql.push_str(&having.accept(self)?);
        }

        // ORDER BY
        if let Some(order_by) = &stmt.order_by {
            sql.push_str(&self.visit_order_by_clause(order_by)?);
        }

        // LIMIT
        if let Some(limit) = &stmt.limit {
            sql.push_str(&self.visit_limit_clause(limit)?);
        }

        Ok(sql)
    }

    fn visit_insert_statement(&mut self, stmt: &InsertStatement) -> Self::Output {
        let mut sql = String::new();

        sql.push_str("INSERT INTO ");
        sql.push_str(&self.visit_qualified_name(&stmt.table)?);

        if !stmt.columns.is_empty() {
            sql.push_str(" (");
            sql.push_str(&stmt.columns.iter()
                .map(|c| self.visit_identifier(c))
                .collect::<Result<Vec<_>>>()?
                .join(", "));
            sql.push_str(")");
        }

        sql.push_str("\n");
        sql.push_str(&self.visit_insert_source(&stmt.source)?);

        // ON CONFLICT は PostgreSQL 固有の構文
        if let Some(on_conflict) = &stmt.on_conflict {
            sql.push_str(&self.visit_on_conflict(on_conflict)?);
        }

        Ok(sql)
    }

    // ... 他の visit メソッドも実装
}
```

### Expression Visitor

```rust
impl PostgreSqlEmitter {
    fn visit_expression_inner(&mut self, expr: &Expression) -> Result<String> {
        match expr {
            Expression::Literal(lit) => self.visit_literal(lit),
            Expression::Identifier(ident) => self.visit_identifier(ident),
            Expression::QualifiedIdentifier { table, column } => {
                self.visit_qualified_identifier(table, column)
            }
            Expression::BinaryOp { left, op, right } => {
                self.visit_binary_op(left, *op, right)
            }
            Expression::UnaryOp { op, expr } => {
                self.visit_unary_op(*op, expr)
            }
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
            Expression::Subquery(sq) => {
                self.visit_subquery(sq)
            }
            Expression::Exists { subquery, negated } => {
                self.visit_exists(subquery, *negated)
            }
            Expression::In { expr, list, negated } => {
                self.visit_in(expr, list, *negated)
            }
            Expression::Between { expr, low, high, negated } => {
                self.visit_between(expr, low, high, *negated)
            }
            Expression::Cast { expr, data_type } => {
                self.visit_cast(expr, data_type)
            }
            Expression::IsNull { expr, negated } => {
                self.visit_is_null(expr, *negated)
            }
        }
    }

    fn visit_binary_op(
        &mut self,
        left: &Expression,
        op: BinaryOperator,
        right: &Expression,
    ) -> Result<String> {
        let left_sql = self.visit_expression_inner(left)?;
        let right_sql = self.visit_expression_inner(right)?;

        let op_str = match op {
            BinaryOperator::Add => "+",
            BinaryOperator::Sub => "-",
            BinaryOperator::Mul => "*",
            BinaryOperator::Div => "/",
            BinaryOperator::Mod => "%",
            BinaryOperator::Concat => "||", // PostgreSQL は || で文字列連結
        };

        Ok(format!("{} {} {}", left_sql, op_str, right_sql))
    }

    fn visit_function(
        &mut self,
        name: &Identifier,
        args: &[Expression],
        distinct: bool,
    ) -> Result<String> {
        // 関数マッパーで変換を試みる
        let (func_name, mapped_args) = match FunctionMapper::map_function_call(name, args) {
            Ok(result) => result,
            Err(EmitError::UnsupportedFunction(msg)) => {
                // 警告を出力して元の関数名を使用
                self.context.add_warning(EmitWarning {
                    message: format!("Unsupported function: {}", msg),
                    span: None,
                });
                (name.value.clone(), args.to_vec())
            }
            Err(e) => return Err(e),
        };

        let args_sql: Result<Vec<String>> = mapped_args
            .iter()
            .map(|arg| self.visit_expression_inner(arg))
            .collect();
        let args_sql = args_sql?;

        let distinct_str = if *distinct { "DISTINCT " } else { "" };

        Ok(format!("{}({}{})", func_name, distinct_str, args_sql.join(", ")))
    }

    // ... 他のメソッドも実装
}
```

## 依存関係

```
┌─────────────────────────────────────────────────────────────┐
│                  postgresql-emitter                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│   依存先                                                      │
│   ┌─────────────────────────────────────────────────────┐   │
│   │              common-sql                              │   │
│   │  - Visitor trait                                    │   │
│   │  - AST ノード型                                       │   │
│   │  - DataType enum                                     │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │              thiserror                               │   │
│   │  - Error マクロ                                       │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## 実装時の注意事項

1. **所有権と借用**: Visitor の `&mut self` パターンに従う
2. **エラー伝播**: `?` 演算子でエラーを適切に伝播
3. **文字列フォーマット**: `format!` より `String::push_str` を優先して使用
4. **パニック禁止**: `unwrap()`, `expect()` を使用しない
5. **クロージャの使用**: 複雑なマッピングでクロージャを活用

## テスト戦略

1. **単体テスト**: 各 Mapper のテスト
2. **結合テスト**: 完全な SQL 文の変換テスト
3. **エッジケース**: NULL、空文字、特殊文字等のテスト
4. **フィクスチャ**: T-SQL → 期待される PostgreSQL のペアを用意
