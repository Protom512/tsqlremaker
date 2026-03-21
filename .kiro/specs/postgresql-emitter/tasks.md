# PostgreSQL Emitter - Implementation Tasks

## 概要

PostgreSQL Emitter の実装タスク一覧。

## 依存関係

```
Group 0 (並列実行可):
├── Task 1.1: プロジェクトセットアップ
├── Task 1.2: エラー型の実装
└── Task 1.3: EmissionContext の実装

Group 1 (Group 0 依存):
├── Task 2.1: DataTypeMapper の実装
├── Task 2.2: FunctionMapper の実装
└── Task 2.3: IdentifierQuoter の実装

Group 2 (Group 1 依存):
├── Task 3.1: Expression Visitor の実装
├── Task 3.2: Clause Visitor の実装
└── Task 3.3: Statement Visitor 基本の実装

Group 3 (Group 2 依存):
├── Task 4.1: DML Statement Visitor の実装
└── Task 4.2: DDL Statement Visitor の実装

Group 4 (Group 3 依存):
├── Task 5.1: PostgreSqlEmitter 構造体の実装
└── Task 5.2: 単体テストの実装

Group 5 (Group 4 依存):
├── Task 6.1: 統合テストの実装
└── Task 6.2: フィクスチャテストの実装
```

## Group 0: 基盤整備

### Task 1.1: プロジェクトセットアップ

**依存**: なし
**ファイル**:
- `crates/postgresql-emitter/Cargo.toml`
- `crates/postgresql-emitter/src/lib.rs`

**内容**:
1. `postgresql-emitter` クレートを作成
2. `Cargo.toml` を設定
   - `common-sql` に依存
   - `thiserror` に依存
3. `src/lib.rs` にモジュール宣言を追加
4. 空のモジュールファイルを作成

**検証**:
- `cargo check --package postgresql-emitter` が成功すること

### Task 1.2: エラー型の実装

**依存**: Task 1.1
**ファイル**: `crates/postgresql-emitter/src/error.rs`

**内容**:
1. `EmitError` enum を実装
   - `Unsupported(String)`
   - `UnsupportedDataType(DataType)`
   - `UnsupportedFunction(String)`
   - `SyntaxError { message, span }`
   - `Io` (from std::io::Error)
   - `FormatError` (from std::fmt::Error)
2. `thiserror::Error` を derive
3. `Result<T>` 型エイリアスを定義
4. ユニットテストを追加

**検証**:
- エラー型が正しく構築できる
- エラーメッセージが正しく表示される

### Task 1.3: EmissionContext の実装

**依存**: Task 1.1, 1.2
**ファイル**: `crates/postgresql-emitter/src/context.rs`

**内容**:
1. `EmissionContext` 構造体を実装
   - `indent_level`: 現在のインデントレベル
   - `buffer`: 生成された SQL のバッファ
   - `warnings`: 警告リスト
2. `EmitWarning` 構造体を実装
3. メソッドを実装
   - `indent()`, `dedent()`
   - `indent_str()`
   - `push()`, `push_line()`
   - `add_warning()`
   - `into_string()`, `warnings()`
4. ユニットテストを追加

**検証**:
- インデントが正しく機能する
- 文字列の蓄積が正しく機能する

## Group 1: Mapper 実装

### Task 2.1: DataTypeMapper の実装

**依存**: Group 0 完了
**ファイル**: `crates/postgresql-emitter/src/mappers/datatype.rs`

**内容**:
1. `DataTypeMapper` 構造体を実装（static なメソッドのみ）
2. `map()` メソッドを実装
   - Common SQL `DataType` → PostgreSQL 型文字列
3. 全てのデータ型のマッピングを実装
   - 整数型、小数型、文字列型、日時型、バイナリ型等
4. ユニットテストを追加

**検証**:
- 全てのデータ型が正しくマッピングされる
- パラメータ付き型（VARCHAR(n)等）が正しく処理される

### Task 2.2: FunctionMapper の実装

**依存**: Group 0 完了
**ファイル**: `crates/postgresql-emitter/src/mappers/function.rs`

**内容**:
1. `FunctionMapper` 構造体を実装
2. `map_function_name()` メソッドを実装
   - T-SQL 関数名 → PostgreSQL 関数名
3. `map_function_call()` メソッドを実装
   - 関数呼び出し全体の変換
   - LEFT, RIGHT 等の引数順変更
   - DATEADD, DATEDIFF の特別処理
4. ユニットテストを追加

**検証**:
- 主要な T-SQL 関数が PostgreSQL 関数に変換される
- 引数の変換が正しく行われる

### Task 2.3: IdentifierQuoter の実装

**依存**: Group 0 完了
**ファイル**: `crates/postgresql-emitter/src/mappers/identifier.rs`

**内容**:
1. `IdentifierQuoter` 構造体を実装
2. `is_reserved_word()` メソッドを実装
   - PostgreSQL の予約語セットを定義
3. `needs_quoting()` メソッドを実装
4. `quote()` メソッドを実装
   - 識別子をダブルクォートで囲む
   - エスケープ処理（`"` → `""`）
5. ユニットテストを追加

**検証**:
- 予約語が正しくクォートされる
- 特殊文字を含む識別子が正しくエスケープされる

## Group 2: Visitor 基本実装

### Task 3.1: Expression Visitor の実装

**依存**: Group 1 完了
**ファイル**: `crates/postgresql-emitter/src/visitors/expression.rs`

**内容**:
1. `PostgreSqlEmitter` に Expression 用 visit メソッドを実装
2. 各 Expression variant の処理を実装
   - `visit_literal()`: リテラルの出力
   - `visit_identifier()`: 識別子の出力（クォート処理）
   - `visit_qualified_identifier()`: 修飾識別子の出力
   - `visit_binary_op()`: 二項演算子の出力（`||` 対応）
   - `visit_unary_op()`: 単項演算子の出力
   - `visit_logical_op()`: 論理演算子の出力
   - `visit_comparison()`: 比較演算子の出力
   - `visit_function()`: 関数呼び出しの出力（FunctionMapper 使用）
   - `visit_case()`: CASE 式の出力
   - `visit_subquery()`: サブクエリの出力
   - `visit_exists()`: EXISTS の出力
   - `visit_in()`: IN の出力
   - `visit_between()`: BETWEEN の出力
   - `visit_cast()`: CAST の出力
   - `visit_is_null()`: IS NULL の出力
3. ユニットテストを追加

**検証**:
- 全ての式が PostgreSQL SQL に変換される
- 複雑な入れ子の式が正しく処理される

### Task 3.2: Clause Visitor の実装

**依存**: Group 1 完了
**ファイル**: `crates/postgresql-emitter/src/visitors/clause.rs`

**内容**:
1. 各句用の visit メソッドを実装
2. `visit_with_clause()`: WITH 句の出力
3. `visit_select_items()`: SELECT リストの出力
4. `visit_table_factor()`: テーブル参照の出力
5. `visit_join()`: JOIN の出力
6. `visit_group_by_clause()`: GROUP BY 句の出力
7. `visit_order_by_clause()`: ORDER BY 句の出力
8. `visit_limit_clause()`: LIMIT 句の出力
9. `visit_insert_source()`: INSERT ソースの出力
10. ユニットテストを追加

**検証**:
- 全ての句が正しく出力される
- 複雑な JOIN が正しく処理される

### Task 3.3: Statement Visitor 基本の実装

**依存**: Task 3.1, 3.2
**ファイル**: `crates/postgresql-emitter/src/visitors/statement.rs`

**内容**:
1. `Visitor` trait を `PostgreSqlEmitter` に実装
2. `type Output = Result<String>` を設定
3. `visit_statement()` メソッドを実装（dispatch）
4. `visit_select_statement()` を実装
5. ユニットテストを追加

**検証**:
- 基本的な SELECT 文が変換できる

## Group 3: DML/DDL Visitor

### Task 4.1: DML Statement Visitor の実装

**依存**: Task 3.3
**ファイル**: `crates/postgresql-emitter/src/visitors/statement.rs` (追記)

**内容**:
1. `visit_insert_statement()` を実装
2. `visit_update_statement()` を実装
3. `visit_delete_statement()` を実装
4. `visit_on_conflict()` を実装（PostgreSQL 固有）
5. ユニットテストを追加

**検証**:
- INSERT/UPDATE/DELETE 文が正しく変換される
- ON CONFLICT 句が正しく出力される

### Task 4.2: DDL Statement Visitor の実装

**依存**: Task 3.3
**ファイル**: `crates/postgresql-emitter/src/visitors/statement.rs` (追記)

**内容**:
1. `visit_create_table_statement()` を実装
   - カラム定義の出力
   - 制約の出力（PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK）
   - SERIAL/BIGSERIAL への変換
2. `visit_alter_table_statement()` を実装
3. `visit_drop_table_statement()` を実装
4. `visit_create_index_statement()` を実装
5. `visit_drop_index_statement()` を実装
6. ユニットテストを追加

**検証**:
- CREATE TABLE が正しく変換される
- IDENTITY が SERIAL に変換される

## Group 4: Emitter 構造体

### Task 5.1: PostgreSqlEmitter 構造体の実装

**依存**: Group 3 完了
**ファイル**: `crates/postgresql-emitter/src/emitter.rs`

**内容**:
1. `PostgreSqlEmitter` 構造体を実装
   - `context: EmissionContext`
   - `options: EmitterOptions`
2. `EmitterOptions` 構造体を実装
   - `uppercase_keywords: bool`
   - `quote_identifiers: bool`
   - `indent_size: usize`
   - `warn_unsupported: bool`
3. `new()`, `with_options()` コンストラクタを実装
4. `emit()` メソッドを実装（公開API）
5. `emit_batch()` メソッドを実装
6. `Default` trait を実装
7. `visitors/mod.rs` で Visitor 実装を再エクスポート
8. ユニットテストを追加

**検証**:
- Emitter がインスタンス化できる
- `emit()` で SQL が生成できる

### Task 5.2: 単体テストの実装

**依存**: Task 5.1
**ファイル**: `crates/postgresql-emitter/src/` 各ファイルの `#[cfg(test)]`

**内容**:
1. 各モジュールの単体テストを実装
2. エッジケースのテスト
3. エラーケースのテスト
4. カバレッジを確認

**検証**:
- `cargo test --package postgresql-emitter` がパス
- カバレッジ 80% 以上

## Group 5: 統合テスト

### Task 6.1: 統合テストの実装

**依存**: Task 5.2
**ファイル**: `crates/postgresql-emitter/tests/integration_tests.rs`

**内容**:
1. 完全な SQL 文の変換テストを実装
2. 複雑なクエリの変換テスト
3. 複数の Statement を含むバッチのテスト

**検証**:
- 統合テストがパス
- 生成された SQL が有効であること

### Task 6.2: フィクスチャテストの実装

**依存**: Task 6.1
**ファイル**:
- `crates/postgresql-emitter/tests/fixtures/tsql_samples.sql`
- `crates/postgresql-emitter/tests/fixtures/postgres_expected.sql`
- `crates/postgresql-emitter/tests/fixture_tests.rs`

**内容**:
1. T-SQL サンプルを用意
2. 期待される PostgreSQL 出力を用意
3. フィクスチャベースのテストを実装
4. 実際の T-SQL → PostgreSQL 変換のテスト

**検証**:
- フィクスチャテストがパス
- 実用的な変換が可能であること

## ファイル所有権表

| Task | 所有ファイル |
|------|-------------|
| 1.1 | `Cargo.toml`, `src/lib.rs`, `src/mod.rs` |
| 1.2 | `src/error.rs` |
| 1.3 | `src/context.rs` |
| 2.1 | `src/mappers/mod.rs`, `src/mappers/datatype.rs` |
| 2.2 | `src/mappers/function.rs` |
| 2.3 | `src/mappers/identifier.rs` |
| 3.1 | `src/visitors/mod.rs`, `src/visitors/expression.rs` |
| 3.2 | `src/visitors/clause.rs` |
| 3.3 | `src/visitors/statement.rs` (SELECT) |
| 4.1 | `src/visitors/statement.rs` (DML) |
| 4.2 | `src/visitors/statement.rs` (DDL) |
| 5.1 | `src/emitter.rs` |
| 5.2 | 各ファイルの `#[cfg(test)]` |
| 6.1 | `tests/integration_tests.rs` |
| 6.2 | `tests/fixtures/*`, `tests/fixture_tests.rs` |

## 共有ファイル

| ファイル | アクセス |
|---------|--------|
| `src/lib.rs` | 全タスクで pub use 追加可能 |
| `src/mod.rs` | 全タスクで mod 宣言追加可能 |
| `Cargo.toml` | 必要に応じて依存追加 |

## チェックリスト

### 各タスク完了時

- [ ] コードが `cargo fmt` でフォーマットされている
- [ ] `cargo clippy` で警告がない
- [ ] `cargo check` でコンパイルエラーがない
- [ ] ユニットテストが追加されている
- [ ] テストがパスしている

### 実装完了時（全タスク）

- [ ] 全てのタスクが完了
- [ ] `cargo test --package postgresql-emitter` がパス
- [ ] カバレッジ 80% 以上
- [ ] ドキュメントコメントが追加されている
- [ ] `cargo doc --no-deps` で警告がない
- [ ] 実際の T-SQL が PostgreSQL に変換できる

## T-SQL → PostgreSQL 変換フィクスチャ例

### 入力（T-SQL）

```sql
SELECT TOP 10
    u.User_ID,
    u.UserName,
    COUNT(o.Order_ID) AS OrderCount
FROM Users u
INNER JOIN Orders o ON u.User_ID = o.User_ID
WHERE u.CreateDate >= GETDATE() - 7
GROUP BY u.User_ID, u.UserName
ORDER BY OrderCount DESC
```

### 期待出力（PostgreSQL）

```sql
SELECT
    u."User_ID",
    u."UserName",
    COUNT(o."Order_ID") AS "OrderCount"
FROM "Users" u
INNER JOIN "Orders" o ON u."User_ID" = o."User_ID"
WHERE u."CreateDate" >= CURRENT_TIMESTAMP - INTERVAL '7 days'
GROUP BY u."User_ID", u."UserName"
ORDER BY "OrderCount" DESC
LIMIT 10;
```
