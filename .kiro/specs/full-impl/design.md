# Full Implementation Design Document

## 1. 概要

本ドキュメントは、簡易実装箇所の完全実装に向けた設計を定義する。

**作成日**: 2026-03-19
**ステータス**: 設計中
**言語**: 日本語

---

## 2. 対象機能

1. PostgreSQL Emitter: サブクエリ実装（簡易実装から完全実装へ）
2. Parser: CREATE TABLE テーブルレベル制約の完全実装
3. Parser: サブクエリ内FROM句の派生テーブル実装
4. Parser: プロシージャ本体の完全実装

---

## 3. アーキテクチャ原則

### 3.1 Balanced Coupling

**依存方向（単一方向）**:
```
┌─────────────────────────┐
│   PostgreSQL Emitter    │ (最上位: 高変動)
└──────────┬──────────────┘
           │ 依存
           ▼
┌─────────────────────────┐
│    Common SQL AST       │ (中間: 安定)
└──────────┬──────────────┘
           │ 依存
           ▼
┌─────────────────────────┐
│        Parser           │ (下位: 安定)
└──────────┬──────────────┘
           │ 依存
           ▼
┌─────────────────────────┐
│        Lexer            │ (最下位: 最も安定)
└─────────────────────────┘
```

### 3.2 Contract Coupling

- 各クレート間は `trait` で定義されたコントラクトのみ使用
- 内部実装への直接アクセス禁止
- 公開APIのみ使用

### 3.3 TDD

- 実装前にテストを記述
- テストは「振る舞い」を検証、「実装の詳細」には依存しない

### 3.4 エラーハンドリング

- ライブラリコードでは `panic` 禁止
- 全てのエラーは `Result` 型で返す

---

## 4. 既存コード構造の分析

### 4.1 Common SQL AST

**場所**: `crates/tsql-parser/src/common/`

#### 主な構造体

```rust
// SELECT文
pub struct CommonSelectStatement {
    pub span: Span,
    pub distinct: bool,
    pub columns: Vec<CommonSelectItem>,
    pub from: Vec<CommonTableReference>,  // 既に派生テーブルをサポート
    pub where_clause: Option<CommonExpression>,
    pub group_by: Vec<CommonExpression>,
    pub having: Option<CommonExpression>,
    pub order_by: Vec<CommonOrderByItem>,
    pub limit: Option<CommonLimitClause>,
}

// テーブル参照（既に実装済み）
pub enum CommonTableReference {
    Table { name: String, alias: Option<String>, span: Span },
    Derived { subquery: Box<CommonSelectStatement>, alias: Option<String>, span: Span },
}

// 式（既にサブクエリをサポート）
pub enum CommonExpression {
    // ... その他のバリアント
    Subquery { query: Box<CommonSelectStatement>, span: Span },
    Exists { query: Box<CommonSelectStatement>, negated: bool, span: Span },
}
```

**分析結果**:
- Common SQL AST は既にサブクエリ対応済み
- 派生テーブル（Derived）も既に定義されている
- **追加実装は不要**

### 4.2 T-SQL Parser AST

**場所**: `crates/tsql-parser/src/ast/`

#### 主な構造体

```rust
// SELECT文
pub struct SelectStatement {
    pub span: Span,
    pub distinct: bool,
    pub top: Option<Expression>,
    pub columns: Vec<SelectItem>,
    pub from: Option<FromClause>,
    pub where_clause: Option<Expression>,
    pub group_by: Vec<Expression>,
    pub having: Option<Expression>,
    pub order_by: Vec<OrderByItem>,
    pub limit: Option<LimitClause>,
}

// テーブル参照（既にサブクエリをサポート）
pub enum TableReference {
    Table { name: Identifier, alias: Option<Identifier>, span: Span },
    Subquery { query: Box<SelectStatement>, alias: Option<Identifier>, span: Span },
    Joined { joins: Vec<Join>, span: Span },
}

// テーブル制約
pub enum TableConstraint {
    PrimaryKey { columns: Vec<Identifier> },
    Foreign { columns: Vec<Identifier>, ref_table: Identifier, ref_columns: Vec<Identifier> },
    Unique { columns: Vec<Identifier> },
    Check { expr: Expression },
}
```

**分析結果**:
- AST定義は既にサブクエリ対応済み
- テーブル制約も定義済み

### 4.3 Parser実装

**場所**: `crates/tsql-parser/src/expression/mod.rs`

#### 既存実装

```rust
fn parse_subquery_from_clause(&mut self) -> ParseResult<FromClause> {
    // ... 既に実装されている
    // 派生テーブルの解析ロジックが存在
}
```

**分析結果**:
- `parse_subquery_from_clause` は既に実装されている
- 派生テーブルの解析ロジックが存在する

### 4.4 PostgreSQL Emitter

**場所**: `crates/postgresql-emitter/src/`

#### 既存実装

```rust
// PostgreSqlEmitter::visit_table_reference
fn visit_table_reference(&mut self, table: &CommonTableReference) -> Result<(), EmitError> {
    match table {
        CommonTableReference::Derived { subquery, alias, .. } => {
            self.write("(");
            self.visit_select_statement(subquery)?;  // 再帰的にサブクエリを処理
            self.write(")");
            // ... エイリアス処理
        }
        // ...
    }
}
```

**分析結果**:
- PostgreSQL Emitter は既に派生テーブルを処理できる
- `visit_select_statement` を再帰呼び出ししている

---

## 5. 設計方針

### 5.1 PostgreSQL Emitter: サブクエリ実装

**現状分析**:
- `PostgreSqlEmitter::visit_table_reference` は既に派生テーブルに対応している
- `ExpressionEmitter::emit` もサブクエリ式に対応済み
- `SelectStatementRenderer::emit_table_reference` も実装済み

**設計方針**:
1. **追加実装は不要**: 既存実装で完全に機能している
2. テストカバレッジを向上させる（エッジケースの確認）
3. ネストされたサブクエリの動作確認

**追加テスト項目**:
```rust
#[test]
fn test_nested_subquery() {
    // SELECT * FROM (SELECT * FROM (SELECT * FROM users) AS u1) AS u2
}

#[test]
fn test_subquery_in_where_clause() {
    // SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)
}

#[test]
fn test_subquery_in_select_list() {
    // SELECT (SELECT COUNT(*) FROM orders WHERE user_id = u.id) FROM users u
}
```

### 5.2 Parser: CREATE TABLE テーブルレベル制約

**現状分析**:
- AST定義は `TableConstraint` として存在
- Parserの `parse_table_constraints` メソッド確認が必要

**設計方針**:
1. `TableConstraint` の解析ロジックを確認
2. 以下の制約タイプを完全実装:
   - PRIMARY KEY (複数カラム対応)
   - FOREIGN KEY
   - UNIQUE (複数カラム対応)
   - CHECK

**Parser実装**:
```rust
fn parse_table_constraint(&mut self) -> ParseResult<TableConstraint> {
    match self.buffer.current()?.kind {
        TokenKind::Primary => self.parse_primary_key_constraint(),
        TokenKind::Foreign => self.parse_foreign_key_constraint(),
        TokenKind::Unique => self.parse_unique_constraint(),
        TokenKind::Check => self.parse_check_constraint(),
        _ => Err(ParseError::unexpected_token(...)),
    }
}

fn parse_primary_key_constraint(&mut self) -> ParseResult<TableConstraint> {
    self.consume_keyword(TokenKind::Primary)?;
    self.consume_keyword(TokenKind::Key)?;

    let columns = self.parse_identifier_list()?;
    let constraint_name = self.parse_constraint_name()?;  // CONSTRAINT name オプション

    Ok(TableConstraint::PrimaryKey { columns, name: constraint_name })
}

fn parse_foreign_key_constraint(&mut self) -> ParseResult<TableConstraint> {
    self.consume_keyword(TokenKind::Foreign)?;
    self.consume_keyword(TokenKind::Key)?;

    let columns = self.parse_identifier_list()?;
    self.consume_keyword(TokenKind::References)?;

    let ref_table = self.parse_identifier()?;
    let ref_columns = self.parse_identifier_list()?;

    let on_delete = self.parse_referentialAction()?;  // ON DELETE オプション
    let on_update = self.parse_referentialAction()?;  // ON UPDATE オプション

    Ok(TableConstraint::Foreign {
        columns,
        ref_table,
        ref_columns,
        on_delete,
        on_update,
        name: None,  // TODO: CONSTRAINT name対応
    })
}
```

**Common SQL ASTへのマッピング**:
```rust
impl ToCommonAst for TableConstraint {
    fn to_common(&self) -> Option<CommonTableConstraint> {
        match self {
            TableConstraint::PrimaryKey { columns, .. } => {
                Some(CommonTableConstraint::PrimaryKey {
                    columns: columns.iter().map(|id| id.name.clone()).collect(),
                })
            }
            TableConstraint::Foreign { columns, ref_table, ref_columns, .. } => {
                Some(CommonTableConstraint::Foreign {
                    columns: columns.iter().map(|id| id.name.clone()).collect(),
                    ref_table: ref_table.name.clone(),
                    ref_columns: ref_columns.iter().map(|id| id.name.clone()).collect(),
                })
            }
            TableConstraint::Unique { columns, .. } => {
                Some(CommonTableConstraint::Unique {
                    columns: columns.iter().map(|id| id.name.clone()).collect(),
                })
            }
            TableConstraint::Check { expr, .. } => {
                Some(CommonTableConstraint::Check {
                    expr: expr.to_common_expression()?,
                })
            }
        }
    }
}
```

### 5.3 Parser: サブクエリ内FROM句の派生テーブル実装

**現状分析**:
- `parse_subquery_from_clause` は既に実装されている
- `TableReference::Subquery` も使用可能

**設計方針**:
1. 既存実装を確認し、必要に応じてリファクタリング
2. エッジケースのテストを追加
3. エラーメッセージの改善

**確認事項**:
```rust
// 既存実装の確認
fn parse_subquery_from_clause(&mut self) -> ParseResult<FromClause> {
    // 1. FROMキーワードの消費
    // 2. 派生テーブルの検出 (LParen)
    // 3. サブクエリの解析
    // 4. RParenの確認
    // 5. エイリアスの解析
    // 6. 複数テーブルのカンマ区切り対応
}
```

### 5.4 Parser: プロシージャ本体の完全実装

**現状分析**:
- `ProcedureDefinition` はAST定義済み
- `body: Vec<Statement>` で本体を表現

**設計方針**:
1. プロシージャ内で使用可能な全ての文種別をサポート
2. パラメータの `OUTPUT` 指定に対応
3. `RETURN` 文の戻り値に対応

**Parser実装**:
```rust
fn parse_procedure_body(&mut self) -> ParseResult<Vec<Statement>> {
    self.consume_keyword(TokenKind::As)?;
    self.consume_keyword(TokenKind::Begin)?;

    let mut statements = Vec::new();

    while !self.buffer.check(TokenKind::End) && !self.buffer.check(TokenKind::EOF) {
        let stmt = self.parse_procedure_statement()?;
        statements.push(stmt);
    }

    self.consume_keyword(TokenKind::End)?;
    Ok(statements)
}

fn parse_procedure_statement(&mut self) -> ParseResult<Statement> {
    match self.buffer.current()?.kind {
        TokenKind::Declare => self.parse_declare_statement(),
        TokenKind::Set => self.parse_set_statement(),
        TokenKind::Select => self.parse_select_statement(),
        TokenKind::Insert => self.parse_insert_statement(),
        TokenKind::Update => self.parse_update_statement(),
        TokenKind::Delete => self.parse_delete_statement(),
        TokenKind::If => self.parse_if_statement(),
        TokenKind::While => self.parse_while_statement(),
        TokenKind::Return => self.parse_return_statement(),
        TokenKind::Break => self.parse_break_statement(),
        TokenKind::Continue => self.parse_continue_statement(),
        _ => Err(ParseError::unexpected_token(...)),
    }
}

fn parse_procedure_parameter(&mut self) -> ParseResult<ParameterDefinition> {
    let name = self.parse_identifier()?;
    let data_type = self.parse_data_type()?;

    let default_value = if self.buffer.check(TokenKind::Eq) {
        self.buffer.consume()?;
        Some(self.parse_expression()?)
    } else {
        None
    };

    let is_output = if self.buffer.check(TokenKind::Output) {
        self.buffer.consume()?;
        true
    } else {
        false
    };

    Ok(ParameterDefinition {
        name,
        data_type,
        default_value,
        is_output,
    })
}
```

---

## 6. 依存関係

### 6.1 モジュール間依存

```
┌─────────────────────────────────────────────────────┐
│                    PostgreSQL Emitter                │
│  (postgres-emitter/src/lib.rs, mappers/)            │
│         依存: common-sql (tsql-parser::common)       │
└─────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│                     Common SQL AST                   │
│              (tsql-parser/src/common/)               │
│         依存: なし（他から依存される）                 │
└─────────────────────────────────────────────────────┘
                         △
                         │
┌─────────────────────────────────────────────────────┐
│                        Parser                        │
│      (tsql-parser/src/parser.rs, expression/)        │
│         依存: tsql-lexer                             │
└─────────────────────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────┐
│                        Lexer                         │
│              (tsql-lexer/src/lib.rs)                 │
│         依存: tsql-token                             │
└─────────────────────────────────────────────────────┘
```

### 6.2 実装順序

推奨される実装順序:

1. **Parser: CREATE TABLE テーブルレベル制約**
   - 依存: Lexer のみ
   - 出力: Parser AST

2. **Parser: サブクエリ内FROM句の派生テーブル**
   - 依存: 既存のサブクエリ実装
   - 出力: Parser AST

3. **Parser: プロシージャ本体**
   - 依存: 全ての文種別のParser
   - 出力: Parser AST

4. **PostgreSQL Emitter: サブクエリ**
   - 依存: Common SQL AST
   - 出力: PostgreSQL SQL

---

## 7. テスト戦略

### 7.1 TDDサイクル

1. **Red**: 期待する振る舞いをテストで記述
2. **Green**: テストを通す最小実装を行う
3. **Refactor**: 実装をリファクタリング（テストは壊れないはず）

### 7.2 テストカバレッジ目標

| モジュール | カバレッジ目標 | 重要度 |
|-----------|---------------|--------|
| Parser (CREATE TABLE) | 90%+ | 高 |
| Parser (サブクエリ) | 90%+ | 高 |
| Parser (プロシージャ) | 85%+ | 中 |
| PostgreSQL Emitter | 80%+ | 中 |

### 7.3 結合の回避

**禁止**:
```rust
// ❌ プライベートメソッドをテスト
#[test]
fn test_parse_primary_key() {
    let parser = Parser::new(sql);
    let result = parser.parse_primary_key_constraint();  // private
}
```

**推奨**:
```rust
// ✅ 公開APIを通じてテスト
#[test]
fn test_parse_create_table_with_primary_key() {
    let sql = "CREATE TABLE users (id INT, PRIMARY KEY (id))";
    let result = parse_one(sql);
    assert!(matches!(result, Ok(Statement::Create(_))));
}
```

---

## 8. 追加・変更が必要な構造体・関数

### 8.1 Parser

**新規追加**:
- `parse_table_constraint(&mut self) -> ParseResult<TableConstraint>`
- `parse_primary_key_constraint(&mut self) -> ParseResult<TableConstraint>`
- `parse_foreign_key_constraint(&mut self) -> ParseResult<TableConstraint>`
- `parse_unique_constraint(&mut self) -> ParseResult<TableConstraint>`
- `parse_check_constraint(&mut self) -> ParseResult<TableConstraint>`
- `parse_referentialAction(&mut self) -> ParseResult<ReferentialAction>`
- `parse_procedure_body(&mut self) -> ParseResult<Vec<Statement>>`
- `parse_procedure_parameter(&mut self) -> ParseResult<ParameterDefinition>`

**変更可能性あり**:
- `parse_table_definition`: テーブル制約の解析を追加

### 8.2 Common SQL AST

**追加は不要**: 既存の定義で十分

### 8.3 PostgreSQL Emitter

**追加は不要**: 既存の実装で十分

### 8.4 ToCommon 変換

**新規追加**:
- `impl ToCommonAst for TableConstraint`
- `impl ToCommonAst for CreateStatement` (CREATE TABLE対応)

---

## 9. エラーハンドリング

### 9.1 ParseError

既存の `ParseError` を使用:

```rust
pub enum ParseError {
    UnexpectedToken {
        expected: Vec<TokenKind>,
        found: TokenKind,
        span: Span,
    },
    InvalidSyntax {
        message: String,
        span: Span,
    },
    // ...
}
```

### 9.2 EmitError

既存の `EmitError` を使用:

```rust
pub enum EmitError {
    Unsupported(String),
    // ...
}
```

---

## 10. 検証計画

### 10.1 単体テスト

各モジュールの単体テストを実施:

```bash
# Parser テスト
cargo test --package tsql-parser

# PostgreSQL Emitter テスト
cargo test --package postgresql-emitter
```

### 10.2 統合テスト

Parser → Common AST → Emitter の流れをテスト:

```rust
#[test]
fn test_full_pipeline_subquery() {
    let sql = "SELECT * FROM (SELECT id FROM users) AS u";
    let stmt = parse_one(sql).unwrap();
    let common = stmt.to_common_ast().unwrap();
    let pg_sql = emit(&common).unwrap();
    assert!(pg_sql.contains("SELECT * FROM (SELECT id FROM users) AS u"));
}
```

### 10.3 エッジケース

- ネストされたサブクエリ
- 複数のテーブル制約
- 複雑なプロシージャ（IF/WHILEのネスト）

---

## 11. 実装チェックリスト

### 11.1 Parser: CREATE TABLE テーブルレベル制約

- [ ] PRIMARY KEY 制約の解析
- [ ] FOREIGN KEY 制約の解析
- [ ] UNIQUE 制約の解析
- [ ] CHECK 制約の解析
- [ ] 複数カラムの制約対応
- [ ] 制約名のオプション対応
- [ ] Common SQL AST への変換
- [ ] テストカバレッジ90%以上

### 11.2 Parser: サブクエリ内FROM句の派生テーブル

- [ ] 既存実装の確認
- [ ] エッジケースのテスト追加
- [ ] エラーメッセージの改善
- [ ] テストカバレッジ90%以上

### 11.3 Parser: プロシージャ本体

- [ ] DECLARE 文の解析
- [ ] SET 文の解析
- [ ] IF...ELSE 文の解析
- [ ] WHILE 文の解析
- [ ] RETURN 文の解析
- [ ] BREAK/CONTINUE 文の解析
- [ ] パラメータの OUTPUT 指定対応
- [ ] テストカバレッジ85%以上

### 11.4 PostgreSQL Emitter: サブクエリ

- [ ] 既存実装の確認
- [ ] エッジケースのテスト追加
- [ ] テストカバレッジ80%以上

---

## 12. 参考資料

- `.claude/rules/architecture-coupling-balance.md`
- `.claude/rules/tdd-coupling.md`
- `.claude/rules/rust-anti-patterns.md`
- SAP ASE T-SQL リファレンス
- PostgreSQL SQL リファレンス
