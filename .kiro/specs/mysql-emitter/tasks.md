# Implementation Plan

## 概要

MySQL Emitter の実装タスク一覧。

## 依存関係

```
Group 0 (並列実行可):
├── Task 1.1: プロジェクトセットアップ
├── Task 1.2: EmitError の実装
└── Task 1.3: EmitterConfig の実装

Group 1 (Group 0 依存):
├── Task 2.1: DataTypeConverter の実装
├── Task 2.2: FunctionConverter の実装
└── Task 2.3: SyntaxConverter の実装

Group 2 (Group 1 依存):
├── Task 3.1: MySqlEmitter 構造体の実装
├── Task 3.2: Visitor trait - Statement の実装
└── Task 3.3: Visitor trait - Expression の実装

Group 3 (Group 2 依存):
├── Task 4.1: SELECT 文生成の実装
├── Task 4.2: INSERT 文生成の実装
├── Task 4.3: UPDATE/DELETE 文生成の実装
└── Task 4.4: DDL 文生成の実装

Group 4 (Group 3 依存):
├── Task 5.1: Formatter の実装
├── Task 5.2: 単体テストの実装
└── Task 5.3: 統合テストの実装
```

## Group 0: 基盤整備

### Task 1: プロジェクトセットアップ

**依存**: なし

- [ ] 1.1 (P) mysql-emitter クレートの作成
  - `crates/mysql-emitter/Cargo.toml` を作成
  - `common-sql-ast` への依存を追加
  - `thiserror` への依存を追加
  - `src/lib.rs` にモジュール宣言を追加
  - `_Requirements: 17_

- [ ] 1.2 (P) EmitError の実装
  - `EmitError` enum を実装
  - `UnsupportedStatement`, `UnsupportedExpression`, `UnsupportedDataType`, `UnsupportedFunction` variantを追加
  - `thiserror` を使用してエラー型を定義
  - `Debug` を derive
  - 単体テストを追加
  - `_Requirements: 13_

- [ ] 1.3 (P) EmitterConfig の実装
  - `EmitterConfig` 構造体を実装
  - `format: bool`, `indent_size: usize` フィールドを追加
  - `Debug`, `Clone` を derive
  - `Default` 実装（format: true, indent_size: 4）
  - 単体テストを追加
  - `_Requirements: 16_

## Group 1: Converter 実装

### Task 2: Converter コンポーネントの実装

**依存**: Group 0 完了

- [ ] 2.1 (P) DataTypeConverter の実装
  - `DataTypeConverter` 構造体を実装（ユニット構造体）
  - `convert()` メソッドを実装
    - `TinyInt` → `TINYINT`
    - `SmallInt` → `SMALLINT`
    - `Int` → `INT`
    - `BigInt` → `BIGINT`
    - `Decimal { p, s }` → `DECIMAL(p,s)`
    - `Numeric { p, s }` → `DECIMAL(p,s)`
    - `Real` → `DOUBLE`
    - `DoublePrecision` → `DOUBLE`
    - `Char { n }` → `CHAR(n)`
    - `VarChar { n }` → `VARCHAR(n)`
    - `Text` → `TEXT`
    - `NChar { n }` → `CHAR(n)`
    - `NVarChar { n }` → `VARCHAR(n)`
    - `Date` → `DATE`
    - `Time { p }` → `TIME(p)`
    - `DateTime { p }` → `DATETIME(p)`
    - `Timestamp { p }` → `TIMESTAMP(p)`
    - `Binary { n }` → `BINARY(n)`
    - `VarBinary { n }` → `VARBINARY(n)`
    - `Blob` → `BLOB`
    - `Boolean` → `TINYINT(1)`
    - `Uuid` → `CHAR(36)`
  - `format_params()` ヘルパーメソッドを実装
  - 単体テストを追加（全24パターン）
  - `_Requirements: 2_

- [ ] 2.2 (P) FunctionConverter の実装
  - `FunctionConverter` 構造体を実装（ユニット構造体）
  - `convert_function()` メソッドを実装
    - 引数: `name: &Identifier`, `args: &[Expression]`, `distinct: bool`, `emitter: &mut MySqlEmitter`
    - 戻り値: `Result<String, EmitError>`
  - `map_function_name()` ヘルパーメソッドを実装
    - `GETDATE` → `NOW`
    - `GETUTCDATE` → `UTC_TIMESTAMP`
    - `LEN` → `LENGTH`
    - `CHARINDEX` → `LOCATE`
    - `REPLICATE` → `REPEAT`
    - `ISNULL` → `IFNULL`
    - `NEWID` → `UUID`
    - `CEILING` → `CEIL`
    - `POWER` → `POW`
  - DATEADD/DATEDIFF の特殊変換ロジックを実装
    - `DATEADD(part, n, date)` → `DATE_ADD(date, INTERVAL n part)`
    - `DATEDIFF(part, start, end)` → `DATEDIFF(end, start)`（引数順逆転）
  - 単体テストを追加（全27パターン）
  - `_Requirements: 3_

- [ ] 2.3 (P) SyntaxConverter の実装
  - `SyntaxConverter` 構造体を実装（ユニット構造体）
  - `convert_top_to_limit()` メソッドを実装
    - `TOP n` を `LIMIT n` に変換
  - `convert_variable_assignment()` メソッドを実装
    - `SELECT @var = expr` を `SET @var = (SELECT expr)` に変換
  - `convert_temp_table()` メソッドを実装
    - `#temp_table` → `temp_table`
    - `##global_temp` → `global_temp`（フラグ付き）
  - 単体テストを追加
  - `_Requirements: 4, 10, 11_

## Group 2: Emitter コア実装

### Task 3: MySqlEmitter コアの実装

**依存**: Group 0, 1 完了

- [ ] 3.1 MySqlEmitter 構造体の実装
  - `MySqlEmitter` 構造体を実装
    - `buffer: String` フィールド
    - `indent_level: usize` フィールド
    - `config: EmitterConfig` フィールド
  - `new()` コンストラクタを実装
  - `emit()` メソッドを実装（単一ステートメント）
  - `emit_batch()` メソッドを実装（複数ステートメント）
  - `reset()` メソッドを実装（バッファクリア）
  - 単体テストを追加
  - `_Requirements: 1, 14, 16_

- [ ] 3.2 Visitor trait - Statement の実装
  - `Visitor` trait を `MySqlEmitter` に実装
  - `type Output = String` を定義
  - `visit_statement()` メソッドを実装
    - 各 Statement タイプに応じたディスパッチ
  - 以下の visit メソッドを実装
    - `visit_select_statement()`
    - `visit_insert_statement()`
    - `visit_update_statement()`
    - `visit_delete_statement()`
    - `visit_create_table_statement()`
    - `visit_drop_table_statement()`
  - 単体テストを追加
  - `_Requirements: 1, 5, 6, 7, 8, 9_

- [ ] 3.3 Visitor trait - Expression の実装
  - 以下の visit メソッドを実装
    - `visit_expression()` - ディスパッチ
    - `visit_literal()`
    - `visit_identifier()`
    - `visit_qualified_identifier()`
    - `visit_binary_op()`
    - `visit_unary_op()`
    - `visit_logical_op()`
    - `visit_comparison()`
    - `visit_function()` - FunctionConverter を使用
    - `visit_case()`
    - `visit_subquery()`
    - `visit_exists()`
    - `visit_in()`
    - `visit_between()`
    - `visit_cast()`
    - `visit_is_null()`
  - `visit_data_type()` - DataTypeConverter を使用
  - 単体テストを追加
  - `_Requirements: 1_

## Group 3: 文生成実装

### Task 4: 各文生成の実装

**依存**: Group 2 完了

- [ ] 4.1 SELECT 文生成の実装
  - `visit_select_statement()` の詳細実装
    - WITH 句（CTE）の出力
    - SELECT リストの出力
    - FROM 句の出力
    - WHERE 句の出力
    - JOIN の出力
    - GROUP BY/HAVING の出力
    - ORDER BY の出力
    - LIMIT の出力（TOP からの変換）
    - DISTINCT の出力
    - UNION の出力
  - サブクエリを括弧で囲んで出力
  - 単体テストを追加
  - `_Requirements: 5, 1_

- [ ] 4.2 INSERT 文生成の実装
  - `visit_insert_statement()` の詳細実装
    - `INSERT INTO table VALUES (...)`
    - `INSERT INTO table (cols) VALUES (...)`
    - `INSERT INTO table SELECT ...`
  - `INSERT ... EXEC` は警告コメントを出力
  - 単体テストを追加
  - `_Requirements: 6, 1_

- [ ] 4.3 UPDATE/DELETE 文生成の実装
  - `visit_update_statement()` の詳細実装
    - FROM 句がある場合、JOIN に変換
    - TOP を LIMIT に変換
  - `visit_delete_statement()` の詳細実装
    - FROM 句がある場合、JOIN に変換
    - TOP を LIMIT に変換
  - 単体テストを追加
  - `_Requirements: 7, 8, 4, 1_

- [ ] 4.4 DDL 文生成の実装
  - `visit_create_table_statement()` の詳細実装
    - `CREATE TABLE table (...)`
    - カラム定義の出力（名前、型、制約）
    - NOT NULL / DEFAULT / PRIMARY KEY 制約の出力
    - UNIQUE 制約の出力
    - FOREIGN KEY 制約の出力
    - CHECK 制約の出力
    - テーブルレベル制約の出力
    - IDENTITY を AUTO_INCREMENT に変換
  - `visit_drop_table_statement()` の実装
  - 単体テストを追加
  - `_Requirements: 9, 2, 1_

## Group 4: 仕上げとテスト

### Task 5: 仕上げとテスト

**依存**: Group 3 完了

- [ ] 5.1 Formatter の実装
  - `Formatter` 構造体を実装
  - `format()` メソッドを実装
    - 適切な位置で改行を挿入
    - キーワードの大文字化（オプション）
  - `indent()` ヘルパーメソッドを実装
  - 単体テストを追加
  - `_Requirements: 16_

- [ ] 5.2 単体テストの実装
  - 各コンポーネントの単体テストを実装
    - `DataTypeConverter` テスト（24パターン）
    - `FunctionConverter` テスト（27パターン）
    - `SyntaxConverter` テスト
    - `MySqlEmitter` テスト（各 visit メソッド）
  - エラーケースのテストを実装
  - カバレッジ 80% 以上を達成
  - クリティカルパス（データ型、関数、構文変換）は 90% 以上
  - `_Requirements: 13, 15_

- [ ] 5.3 統合テストの実装
  - `tests/fixtures/` に SQL フィクスチャを作成
    - `select.sql` - 各種 SELECT クエリ
    - `insert.sql` - INSERT 文
    - `update.sql` - UPDATE 文
    - `delete.sql` - DELETE 文
    - `create_table.sql` - CREATE TABLE 文
    - `stored_procedures.sql` - 複雑なストアドプロシージャ
  - 統合テストを実装
    - Common SQL AST を経由して MySQL SQL を生成
    - 生成された SQL が MySQL で実行可能であることを確認
  - カバレッジを確認
  - `_Requirements: 1, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15_

## ファイル所有権表

| Task | 所有ファイル |
|------|-------------|
| 1.1 | `Cargo.toml`, `src/lib.rs` |
| 1.2 | `src/error.rs` |
| 1.3 | `src/config.rs` |
| 2.1 | `src/converters/datatype.rs` |
| 2.2 | `src/converters/function.rs` |
| 2.3 | `src/converters/syntax.rs` |
| 3.1 | `src/emitter.rs` (構造体) |
| 3.2 | `src/emitter.rs` (Statement visitor) |
| 3.3 | `src/emitter.rs` (Expression visitor) |
| 4.1 | `src/emitter.rs` (SELECT 実装) |
| 4.2 | `src/emitter.rs` (INSERT 実装) |
| 4.3 | `src/emitter.rs` (UPDATE/DELETE 実装) |
| 4.4 | `src/emitter.rs` (DDL 実装) |
| 5.1 | `src/formatter.rs` |
| 5.2 | 各ファイルの `#[cfg(test)]` モジュール |
| 5.3 | `tests/*` |

## 共有ファイル（読み取り専用）

| ファイル | アクセス |
|---------|--------|
| `src/lib.rs` | 全タスクで pub use 追加可能 |
| `src/emitter.rs` | 全タスクで同一ファイルに追加実装 |
| `Cargo.toml` | 必要に応じて依存追加 |

## チェックリスト

### 各タスク完了時

- [ ] コードが `cargo fmt` でフォーマットされている
- [ ] `cargo clippy` で警告がない
- [ ] `cargo check` でコンパイルエラーがない
- [ ] `cargo test` でテストがパスしている
- [ ] `panic!` / `unwrap()` / `expect()` が使用されていない
- [ ] エラー処理が `Result` 型で実装されている

### 実装完了時（全タスク）

- [ ] 全てのタスクが完了
- [ ] `cargo test --package mysql-emitter` がパス
- [ ] カバレッジ 80% 以上（クリティカルパス 90% 以上）
- [ ] ドキュメントコメントが追加されている
- [ ] `cargo doc --no-deps` で警告がない
- [ ] `common-sql-ast` のみに依存している
- [ ] `tsql-parser` / `tsql-lexer` に直接依存していない

## 要件カバレッジ

| Requirement | カバーするタスク |
|-------------|------------------|
| 1: AST トラバーサル | 3.1, 3.2, 3.3 |
| 2: データ型変換 | 2.1, 3.3, 5.2 |
| 3: 関数変換 | 2.2, 3.3, 5.2 |
| 4: 構文変換 | 2.3, 3.2, 3.3, 5.2 |
| 5: SELECT 文生成 | 4.1, 5.2, 5.3 |
| 6: INSERT 文生成 | 4.2, 5.2, 5.3 |
| 7: UPDATE 文生成 | 4.3, 5.2, 5.3 |
| 8: DELETE 文生成 | 4.3, 5.2, 5.3 |
| 9: CREATE TABLE 文生成 | 4.4, 5.2, 5.3 |
| 10: 一時テーブル変換 | 2.3, 5.2, 5.3 |
| 11: 変数代入構文変換 | 2.3, 3.2, 5.2 |
| 12: 制御フロー構文変換 | 3.2, 5.2, 5.3 |
| 13: エラーハンドリング | 1.2, 5.2 |
| 14: パフォーマンス | 3.1, 5.3 |
| 15: テストカバレッジ | 5.2, 5.3 |
| 16: 出力フォーマット | 1.3, 5.1, 5.3 |
| 17: 依存関係ルール | 1.1, 全タスク |
