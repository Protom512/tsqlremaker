# 簡易実装箇所調査レポート

**作成日**: 2026-03-18
**調査担当**: TSQLRemaker Team

---

## 概要

TSQLRemaker プロジェクトにおける「簡易実装」「TODO」「placeholder」の箇所を調査し、完全実装に向けての課題を整理しました。

---

## 1. PostgreSQL Emitter: サブクエリ実装

### 現状

**ファイル**: `crates/postgresql-emitter/src/mappers/expression.rs`

```rust
// Line 205
CommonInList::Subquery(_) => "(subquery)".to_string(), // TODO: サブクエリの実装

// Line 211
fn emit_subquery(_query: &tsql_parser::common::CommonSelectStatement) -> String {
    // TODO: サブクエリの完全な実装
    "(subquery)".to_string()
}
```

### 不完全点

1. **IN句のサブクエリ**: `WHERE id IN (SELECT id FROM users)` のようなクエリで "(subquery)" が出力される
2. **EXISTS句のサブクエリ**: `WHERE EXISTS (SELECT 1 FROM users)` のようなクエリでもプレースホルダー使用

### 実装に必要な作業

1. `CommonSelectStatement` を再帰的に PostgreSQL SQL に変換する処理を実装
2. サブクエリ内の `SELECT`、`FROM`、`WHERE`、`GROUP BY`、`HAVING`、`ORDER BY`、`LIMIT` を処理
3. 既存の `PostgreSqlEmitter::visit_select_statement` を再利用

### 依存関係

- `PostgreSqlEmitter::visit_select_statement` は既に実装済み
- `ExpressionEmitter` から `PostgreSqlEmitter` の機能を呼び出す必要あり

### 見積もり

- **難易度**: 中（既存のロジックを再利用可能）
- **工数**: 2-4時間

---

## 2. Parser: プロシージャ本体

### 現状

**ファイル**: `crates/tsql-parser/src/parser.rs`

```rust
// Line 1348
// プロシージャ本体（簡易版：BEGIN...ENDまたは単一の文）
let body = if self.buffer.check(TokenKind::Begin) {
    let block = self.parse_block()?;
    vec![block]
} else {
    vec![self.parse_statement()?]
};
```

### 不完全点

1. **複文の制御フロー**: `BEGIN...END` ブロック内の複数の文のみ対応
2. **変数宣言スコープ**: `DECLARE` ブロックの処理が不完全
3. **例外処理**: `TRY...CATCH` ブロックに未対応

### 実装に必要な作業

1. `DECLARE` ブロックの適切なスコープ管理
2. `TRY...CATCH` 構文の解析
3. ネストされた `BEGIN...END` ブロックの処理
4. トランザクション制御（`BEGIN TRANSACTION`、`COMMIT`、`ROLLBACK`）

### 依存関係

- `parse_block()` の拡張
- 新しい AST ノードの追加可能性

### 見積もり

- **難易度**: 高
- **工数**: 1-2日

---

## 3. Parser: CREATE TABLE 制約

### 現状

**ファイル**: `crates/tsql-parser/src/parser.rs`

```rust
// Line 2669
// 制約付きCREATE TABLE（簡易実装：カラム制約は解析するがconstraintsリストには追加しない）
```

### 不完全点

1. **テーブルレベル制約**: `CONSTRAINT pk_name PRIMARY KEY (id)` のような制約が未処理
2. **外部キー制約**: `FOREIGN KEY (user_id) REFERENCES users(id)` に未対応
3. **CHECK 制約**: `CONSTRAINT chk_age CHECK (age >= 18)` に未対応
4. **UNIQUE 制約**: テーブルレベルの `UNIQUE` 制約が未処理

### 実装に必要な作業

1. テーブルレベル制約のパーサー実装
2. `ConstraintDefinition` 構造体の拡張
3. 制約名の保存
4. 複数カラムにまたがる制約の処理

### 依存関係

- `TableDefinition` 構造体の `constraints` フィールドの活用
- Common SQL AST へのマッピング

### 見積もり

- **難易度**: 中
- **工数**: 4-6時間

---

## 4. Parser: サブクエリ内のFROM句

### 現状

**ファイル**: `crates/tsql-parser/src/expression/mod.rs`

```rust
// Line 326
/// サブクエリ内のFROM句を解析（簡易版）
fn parse_subquery_from_clause(&mut self) -> ParseResult<crate::ast::FromClause> {
    // ...
    // 通常のテーブル参照のみ対応（サブクエリ内のサブクエリは複雑になるため）
}
```

### 不完全点

1. **派生テーブル**: `FROM (SELECT * FROM users) AS t` に未対応
2. **JOIN**: サブクエリ内の `JOIN` が不完全
3. **CTE（共通テーブル式）**: `WITH cte AS (...)` に未対応

### 実装に必要な作業

1. 派生テーブルの解析実装
2. 既存の `parse_from_clause` ロジックの再利用
3. ネストしたサブクエリの処理

### 依存関係

- 通常の `parse_from_clause` とのコード共有

### 見積もり

- **難易度**: 中
- **工数**: 2-3時間

---

## 5. WASM: Placeholder Function

### 現状

**ファイル**: `crates/wasm/src/lib.rs`

```rust
// Line 154
/// This is a placeholder function. The actual conversion will be implemented
/// once the MySQL/PostgreSQL emitters are completed.
#[wasm_bindgen(js_name = convertTo)]
pub fn convert_to(_input: &str, dialect: TargetDialect) -> JsValue {
    // Emitter未実装のためエラーを返す
}
```

### 不完全点

1. **未実装**: Emitter が完成していないため、JavaScript からの変換機能が動作しない

### 実装に必要な作業

1. PostgreSQL/MySQL Emitter の完了（前提条件）
2. WASM からの Emitter 呼び出し
3. エラーハンドリングの実装
4. JS への結果返却

### 依存関係

- **PostgreSQL Emitter の完全実装（前提）**
- **MySQL Emitter の実装（前提）**

### 見積もり

- **難易度**: 低（Emitter 実装済みなら）
- **工数**: 1-2時間

---

## 6. MySQL Emitter: 未実装

### 現状

**Spec**: `.kiro/specs/mysql-emitter/`
- `spec.json`: `ready_for_implementation: false`

### 不完全点

1. **未実装**: Emitter 自体が存在しない

### 実装に必要な作業

1. `crates/mysql-emitter/` の作成
2. PostgreSQL Emitter を参考にした実装
3. MySQL 固有の構文・関数への対応

### 依存関係

- Common SQL AST（完成済み）
- PostgreSQL Emitter の実装（参考）

### 見積もり

- **難易度**: 中（PostgreSQL Emitter があるため）
- **工数**: 1-2週間

---

## 優先順位評価

| 箇所 | 影響範囲 | 複雑さ | 重要度 | 優先度 | 見積もり |
|------|----------|--------|--------|--------|----------|
| PostgreSQL Emitter: サブクエリ | 式エミッター全体 | 中 | 高 | **1** | 2-4時間 |
| Parser: CREATE TABLE制約 | DDL | 中 | 中 | **2** | 4-6時間 |
| Parser: サブクエリ内FROM | サブクエリ | 中 | 中 | **3** | 2-3時間 |
| Parser: プロシージャ本体 | ストアドプロシージャ | 高 | 中 | **4** | 1-2日 |
| WASM: Placeholder | Web UI | 低 | 中 | **5** | 1-2時間 |
| MySQL Emitter | 新規Emitter | 中 | 高 | **6** | 1-2週間 |

---

## 推奨実装順序

### Phase 1: PostgreSQL Emitter 完全実装（優先）

1. **PostgreSQL Emitter: サブクエリ**（2-4時間）
   - `emit_subquery` の実装
   - `emit_in_list` の `Subquery` case 実装
   - 既存の `PostgreSqlEmitter::visit_select_statement` の再利用

### Phase 2: Parser 機能強化

2. **Parser: CREATE TABLE 制約**（4-6時間）
   - テーブルレベル制約の実装
   - `constraints` フィールドの活用

3. **Parser: サブクエリ内 FROM**（2-3時間）
   - 派生テーブルの対応
   - 既存ロジックの再利用

4. **Parser: プロシージャ本体**（1-2日）
   - `DECLARE` ブロック
   - `TRY...CATCH`
   - トランザクション制御

### Phase 3: WASM 実装

5. **WASM: Placeholder 解消**（1-2時間）
   - Emitter 実装後の統合

### Phase 4: MySQL Emitter 実装

6. **MySQL Emitter 新規実装**（1-2週間）
   - PostgreSQL Emitter を参考に実装

---

## 技術的留意点

### アーキテクチャ

- **Balanced Coupling 原則**: 単一方向依存を維持
- **コントラクト結合**: 公開APIのみを使用

### TDD

- **テストファースト**: 各実装前にテストを記述
- **表駆動テスト**: 複数ケースを網羅

### エラーハンドリング

- **Result 型**: panic を起こさない
- **エラー回復**: Parser のエラー回復機能

---

## 次のステップ

1. **PostgreSQL Emitter: サブクエリ実装**を開始
2. 各実装後にレビューを実施
3. テストカバレッジを維持
