# Common SQL AST - Implementation Tasks

## 概要

Common SQL AST の実装タスク一覧。

## 依存関係

```
Group 0 (並列実行可):
├── Task 1.1: プロジェクトセットアップ
├── Task 1.2: Span と Position の実装
└── Task 1.3: 基本識別子の実装

Group 1 (Group 0 依存):
├── Task 2.1: Literal の実装
├── Task 2.2: DataType の実装
└── Task 2.3: Expression 基本ノードの実装

Group 2 (Group 1 依存):
├── Task 3.1: Expression 演算子ノードの実装
├── Task 3.2: Expression 関数・CASE・サブクエリの実装
└── Task 3.3: JOIN 関連ノードの実装

Group 3 (Group 2 依存):
├── Task 4.1: クエリ句の実装
├── Task 4.2: SELECT Statement の実装
└── Task 4.3: DML Statement の実装

Group 4 (Group 3 依存):
├── Task 5.1: DDL Statement の実装
└── Task 5.2: Visitor Pattern の実装

Group 5 (Group 4 依存):
├── Task 6.1: 単体テストの実装
└── Task 6.2: 統合テストの実装
```

## Group 0: 基盤整備

### Task 1.1: プロジェクトセットアップ

**依存**: なし
**ファイル**:
- `crates/common-sql/Cargo.toml`
- `crates/common-sql/src/lib.rs`

**内容**:
1. `common-sql` クレートを作成
2. `Cargo.toml` を設定（依存なし）
3. `src/lib.rs` にモジュール宣言を追加
4. 必要な `#[allow]` 属性を設定

**検証**:
- `cargo check --package common-sql` が成功すること

### Task 1.2: Span と Position の実装

**依存**: なし
**ファイル**: `crates/common-sql/src/ast/span.rs`

**内容**:
1. `Position` 構造体を実装
2. `Span` 構造体を実装
3. `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash` を derive
4. ユニットテストを追加

**検証**:
- `Span::new()` で Span を作成可能
- 位置情報が正しく保持される

### Task 1.3: 基本識別子の実装

**依存**: なし
**ファイル**: `crates/common-sql/src/ast/identifier.rs`

**内容**:
1. `Identifier` 構造体を実装
2. `QualifiedName` 構造体を実装
3. `TableAlias` 構造体を実装
4. `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash` を derive
5. ユニットテストを追加

**検証**:
- 識別子がクォート有無を保持できる
- 修飾名が schema.name 形式を表現できる

## Group 1: 基本データ型

### Task 2.1: Literal の実装

**依存**: Group 0 完了
**ファイル**: `crates/common-sql/src/ast/literal.rs`

**内容**:
1. `Literal` enum を実装
   - `Integer(i64)`
   - `Float(f64)`
   - `Float(String)` （精度を保持するため文字列）
   - `String(String)`
   - `Boolean(bool)`
   - `Null`
2. `Debug`, `Clone`, `PartialEq`, `Eq` を derive
3. ユニットテストを追加

**検証**:
- 全てのリテラル型が作成可能
- Float の精度が保持される

### Task 2.2: DataType の実装

**依存**: Group 0 完了
**ファイル**: `crates/common-sql/src/ast/datatype.rs`

**内容**:
1. `DataType` enum を実装
   - 整数型: `TinyInt`, `SmallInt`, `Int`, `BigInt`
   - 小数型: `Decimal`, `Numeric`, `Real`, `DoublePrecision`
   - 文字列型: `Char`, `VarChar`, `Text`, `NChar`, `NVarChar`
   - 日時型: `Date`, `Time`, `DateTime`, `Timestamp`
   - バイナリ型: `Binary`, `VarBinary`, `Blob`
   - その他: `Boolean`, `Uuid`, `Json`
2. `Debug`, `Clone`, `PartialEq`, `Eq` を derive
3. ユニットテストを追加

**検証**:
- 全てのデータ型が作成可能
- 精度・スケール・長さのパラメータが保持できる

### Task 2.3: Expression 基本ノードの実装

**依存**: Task 1.3, 2.1
**ファイル**: `crates/common-sql/src/ast/expression.rs`

**内容**:
1. `Expression` enum の基本 variant を実装
   - `Literal(Literal)`
   - `Identifier(Identifier)`
   - `QualifiedIdentifier { table, column }`
2. `Debug`, `Clone`, `PartialEq` を derive
3. ユニットテストを追加

**検証**:
- 基本式が作成可能
- 等価性比較が正しく動作

## Group 2: Expression 拡張

### Task 3.1: Expression 演算子ノードの実装

**依存**: Task 2.3
**ファイル**: `crates/common-sql/src/ast/expression.rs` (追記)

**内容**:
1. 演算子 enum を実装
   - `BinaryOperator`: Add, Sub, Mul, Div, Mod, Concat
   - `UnaryOperator`: Plus, Minus, Not
   - `LogicalOperator`: And, Or
   - `ComparisonOperator`: Eq, Ne, Lt, Le, Gt, Ge, Like, NotLike
2. 演算子式 variant を `Expression` に追加
   - `BinaryOp { left, op, right }`
   - `UnaryOp { op, expr }`
   - `LogicalOp { left, op, right }`
   - `Comparison { left, op, right }`
3. ユニットテストを追加

**検証**:
- 全ての演算子が使用可能
- 入れ子の式が表現できる

### Task 3.2: Expression 関数・CASE・サブクエリの実装

**依存**: Task 2.3, 3.1
**ファイル**: `crates/common-sql/src/ast/expression.rs` (追記)

**内容**:
1. 高度な式 variant を `Expression` に追加
   - `Function { name, args, distinct }`
   - `Case { operand, conditions, else_result }`
   - `Subquery(Box<SelectStatement>)`
   - `Exists { subquery, negated }`
   - `In { expr, list, negated }`
   - `Between { expr, low, high, negated }`
   - `Cast { expr, data_type }`
   - `IsNull { expr, negated }`
2. `InList` enum を実装
   - `Values(Vec<Expression>)`
   - `Subquery(Box<SelectStatement>)`
3. ユニットテストを追加

**検証**:
- 関数呼び出しが表現できる
- CASE 式が表現できる
- サブクエリが式の中で使用できる

### Task 3.3: JOIN 関連ノードの実装

**依存**: Task 1.3
**ファイル**: `crates/common-sql/src/ast/join.rs`

**内容**:
1. `JoinType` enum を実装
   - Inner, Left, Right, Full, Cross
2. `JoinCondition` enum を実装
   - `On(Expression)`
   - `Using(Vec<Identifier>)`
   - `Natural`
3. `Join` 構造体を実装
4. `TableFactor` enum を実装
   - `Table { name, alias }`
   - `Derived { subquery, alias }`
   - `Join(Box<Join>)`
5. `Debug`, `Clone`, `PartialEq` を derive
6. ユニットテストを追加

**検証**:
- 全ての JOIN タイプが表現できる
- 複雑な JOIN チェーンが表現できる

## Group 3: Statement 実装

### Task 4.1: クエリ句の実装

**依存**: Task 2.3, 3.3
**ファイル**: `crates/common-sql/src/ast/clause.rs`

**内容**:
1. `SelectItem` enum を実装
   - `Expression { expr, alias }`
   - `QualifiedWildcard { table }`
   - `Wildcard`
2. `GroupByClause` と `GroupByItem` を実装
3. `OrderByClause` と `OrderByItem` を実装
4. `LimitClause` を実装
5. `WithClause` と `Cte` を実装
6. `Debug`, `Clone`, `PartialEq` を derive
7. ユニットテストを追加

**検証**:
- 全ての句が正しく表現できる
- CTE (WITH 句) が表現できる

### Task 4.2: SELECT Statement の実装

**依存**: Task 4.1
**ファイル**: `crates/common-sql/src/ast/statement.rs`

**内容**:
1. `SelectStatement` 構造体を実装
   - span, with, projection, from, where, group_by, having, order_by, limit
2. `Statement` enum を実装（まず SELECT のみ）
3. `Debug`, `Clone`, `PartialEq` を derive
4. ユニットテストを追加

**検証**:
- 基本的な SELECT 文が作成できる
- 複雑な SELECT 文（JOIN、サブクエリ等）が作成できる

### Task 4.3: DML Statement の実装

**依存**: Task 4.2
**ファイル**: `crates/common-sql/src/ast/statement.rs` (追記)

**内容**:
1. `InsertStatement` 構造体を実装
   - span, table, columns, source, on_conflict
2. `InsertSource` enum を実装
3. `UpdateStatement` 構造体を実装
   - span, table, assignments, from, where
4. `Assignment` 構造体を実装
5. `DeleteStatement` 構造体を実装
   - span, table, using, where
6. DML variant を `Statement` に追加
7. ユニットテストを追加

**検証**:
- INSERT/UPDATE/DELETE 文が作成できる
- VALUES と SELECT の両方の INSERT が表現できる

## Group 4: DDL と Visitor

### Task 5.1: DDL Statement の実装

**依存**: Task 2.2
**ファイル**: `crates/common-sql/src/ast/statement.rs` (追記)

**内容**:
1. `ColumnDef` 構造体を実装
2. `ColumnConstraint` enum を実装
3. `TableConstraint` enum を実装
4. `TableOptions` 構造体を実装
5. `CreateTableStatement` 構造体を実装
6. `AlterTableStatement` 構造体を実装
7. `DropTableStatement` 構造体を実装
8. `CreateIndexStatement` 構造体を実装
9. `DropIndexStatement` 構造体を実装
10. DDL variant を `Statement` に追加
11. ユニットテストを追加

**検証**:
- CREATE TABLE が完全に表現できる
- 制約（主キー、外部キー等）が表現できる

### Task 5.2: Visitor Pattern の実装

**依存**: Task 4.3, 5.1
**ファイル**:
- `crates/common-sql/src/visitor.rs`
- `crates/common-sql/src/ast/mod.rs` (更新)

**内容**:
1. `Visitor` trait を実装
   - `type Output`
   - Statement 用訪問メソッド
   - Expression 用訪問メソッド
   - DataType 用訪問メソッド
2. `Visitable` trait を実装
   - `accept<V: Visitor>(&self, visitor: &mut V) -> V::Output`
3. `Statement`, `Expression`, `DataType` に `Visitable` を実装
4. ダミー Visitor を作成してテスト
5. ユニットテストを追加

**検証**:
- 全てのノードが Visitor で訪問可能
- カスタム Visitor を実装できる

## Group 5: テスト

### Task 6.1: 単体テストの実装

**依存**: Task 5.2
**ファイル**: `crates/common-sql/src/ast/*_tests.rs` または `#[cfg(test)]` モジュール

**内容**:
1. 各ノードの構造テスト
2. 等価性テスト
3. クローンテスト
4. デバッグ出力テスト
5. エッジケーステスト

**検証**:
- カバレッジ 80% 以上
- 全てのテストがパス

### Task 6.2: 統合テストの実装

**依存**: Task 6.1
**ファイル**: `crates/common-sql/tests/`

**内容**:
1. 複雑な AST 構築の統合テスト
2. Visitor パターンの統合テスト
3. 実際の SQL に相当する AST 構築テスト

**検証**:
- 統合テストがパス
- 実用的な AST が構築可能

## ファイル所有権表

| Task | 所有ファイル |
|------|-------------|
| 1.1 | `Cargo.toml`, `src/lib.rs` |
| 1.2 | `src/ast/span.rs` |
| 1.3 | `src/ast/identifier.rs` |
| 2.1 | `src/ast/literal.rs` |
| 2.2 | `src/ast/datatype.rs` |
| 2.3 | `src/ast/expression.rs` (初期) |
| 3.1 | `src/ast/expression.rs` (演算子) |
| 3.2 | `src/ast/expression.rs` (高度な式) |
| 3.3 | `src/ast/join.rs` |
| 4.1 | `src/ast/clause.rs` |
| 4.2 | `src/ast/statement.rs` (SELECT) |
| 4.3 | `src/ast/statement.rs` (DML) |
| 5.1 | `src/ast/statement.rs` (DDL) |
| 5.2 | `src/visitor.rs`, `src/ast/mod.rs` |
| 6.1 | 各ファイルの `#[cfg(test)]` |
| 6.2 | `tests/*` |

## 共有ファイル（読み取り専用）

| ファイル | アクセス |
|---------|--------|
| `src/lib.rs` | 全タスクで pub use 追加可能 |
| `src/ast/mod.rs` | 全タスクで mod 宣言追加可能 |
| `Cargo.toml` | 必要に応て依存追加 |

## チェックリスト

### 各タスク完了時

- [ ] コードが `cargo fmt` でフォーマットされている
- [ ] `cargo clippy` で警告がない
- [ ] `cargo check` でコンパイルエラーがない
- [ ] ユニットテストが追加されている
- [ ] テストがパスしている

### 実装完了時（全タスク）

- [ ] 全てのタスクが完了
- [ ] `cargo test --package common-sql` がパス
- [ ] カバレッジ 80% 以上
- [ ] ドキュメントコメントが追加されている
- [ ] `cargo doc --no-deps` で警告がない
