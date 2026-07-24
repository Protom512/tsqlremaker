# Tasks: schema-diff (sqldef 相当スキーマ差分マイグレーション生成)

> Feature: Issue #162 / 優先度 P2 / サイズ XL
> 言語: 日本語 (`spec.json.language = "ja"`)
> **並列実行可能タスクは parallel-impl-workflow の Group 化で表現** (estimate 承認条件 #5 / feedback #1 準拠)。
> 各タスクは TDD (red-green-refactor) で実装する。

## 依存関係グラフ (Group 化の根拠)

```
T1 (本 spec, design gate) ─────────────────────────────────────────┐
                                                                    │
                     ┌──────────────────────────────────────────────┘
                     ▼
        ┌────────────────────────────┐
        │ Group A (直列クリティカル)   │
        │  T2: parser→common-sql 橋渡し │ ← hard prereq (to_common_sql.rs:144 => None)
        └─────────────┬──────────────┘
                      │ T2 完了で T3/T4/T5 と T6/T7 が並列解放
                      ▼
   ┌──────────────────┴────────────────────────┐
   │                                            │
   ▼                                            ▼
┌─────────────────────┐               ┌────────────────────┐
│ Group B (3 emitter) │               │ Group C (diff core) │
│  T3: mysql DDL emit │ (3者独立並列)  │  T6: crate skel    │→ T7: diff_schema
│  T4: pg DDL emit    │               │  T8: CatalogProvider│
│  T5: sqlite DDL emit│               │  (mapper.rs)        │
└──────────┬──────────┘               └─────────┬──────────┘
           │                                     │
           └──────────────────┬──────────────────┘
                              ▼
                ┌─────────────────────────┐
                │ Group D (統合)           │
                │  T7: diff_schema 本体    │
                │  T10: SQLite ALTER 制限  │
                └────────────┬────────────┘
                             ▼
                ┌─────────────────────────┐
                │ Group E ( adapter + CLI) │
                │  T9:  ase-rs adapter ▼   │ (feature `ase` gate 内)
                │  T11: CLI / JSON dump    │ (feature gate 外、publishable)
                └────────────┬────────────┘
                             ▼
                ┌─────────────────────────┐
                │ Group F (品質 gate)       │
                │  T12: fmt/clippy/nextest │
                └─────────────────────────┘
```

## クリティカルパス

`T1 → T2 → (T3‖T4‖T5‖T6) → T7 → (T9‖T11) → T12`

T1 (本 spec) は全12タスクのクリティカルパス上にあるため、PM phase の時間制約は厳格に管理する (design gate 即通過が前提)。

---

## Task 1.1: (T1, 本タスク — 完了) PM phase: requirements / design / tasks 作成

- **状態**: 完了 (本 files)
- **依存**: なし
- **検証**: design gate (CTO / `kiro:validate-design`) が P5 ルール (全フィールド列挙 / dangling 参照解決 / 破壊的変更方針 / common-sql stability contract) **加えて §0.5 wasm parity 決定 と §0.6 DataType 短絡方針** を満たすことを承認するまで UNAPPROVED。

---

## Group A (直列クリティカルパス)

### Task 2.1: (T2) parser→common-sql DDL 橋渡し

- **依存**: T1 gate 通過 (design §0.5 wasm parity 決定 + §0.6 DataType 短絡方針 の凍結を含む)
- **files**: `crates/tsql-parser/src/ast/to_common_sql.rs`
- **内容**:
  - `to_common_sql` (`to_common_sql.rs:144`) の `Statement::Create(_) | AlterTable(_) | BatchSeparator(_) => None` を拡張:
    - `Statement::Create(CreateStatement::Table(td))` → `Some(SqlStmt::CreateTable(convert_create_table(td)?))`
    - `Statement::Create(CreateStatement::Index(id))` → `Some(SqlStmt::CreateIndex(...))`
    - `Statement::AlterTable(at)` → `Some(SqlStmt::AlterTable(...))`
  - T-SQL 側の `TableDefinition` (`tsql-parser/src/ast/ddl.rs:38`) / `IndexDefinition` (`:218`) / `AlterTableStatement` (`:288`) から common-sql 側 (`ddl.rs` 検証済み) への変換関数を新設。
  - `BatchSeparator` (GO) は引き続き `None` (バッチ境界は文ではない)。
  - **DataType 変換**: `convert_data_type(dt: &tsql::DataType) -> Option<csql::DataType>` を新設し、design §0.6 の対応表・短絡方針 (非対応型 `SmallDateTime`/`Bit`/`Money`/`SmallMoney` は `None`、列 None → 文全体 None) に厳密に従うこと。`UniqueIdentifier` は `Uuid` にマップ (非対応型ではない)。
- **テスト要件** (TDD red 先行):
  - 正常系: CREATE TABLE 1カラム / CREATE INDEX / ALTER TABLE ADD COLUMN の3パターンで `Some(...)` が返ること
  - エッジケース: CREATE VIEW / PROCEDURE / TRIGGER は `None` (common-sql 未対応、非スコープ)
  - **DataType 非対応型短絡 (design §0.6)**: `Bit` / `Money` / `SmallMoney` / `SmallDateTime` を含む CREATE TABLE は `None` を返すこと (各型 1テスト = 計4)。`UniqueIdentifier` を含む CREATE TABLE は `Uuid` にマップされ `Some` を返すこと (非対応型ではないことの検証 = 計1)。
  - 既存の SELECT/INSERT 系テストが回帰しないこと
  - **downstream parity 回帰テスト (design §0.5)** — 以下を `crates/tsql-parser` の単体テストまたは `crates/wasm` の統合テストのいずれかで検証 (wasm feature mask 教訓: `cargo check -p tsql-remaker-wasm --features wasm` を忘れないこと):
    - **R1 (DDL+DML 混在バッチ)**: `CREATE TABLE users (id INT); SELECT * FROM users;` を `to_common_sql` で変換したとき、T2 完了直後の中間状態では DDL は `Some(CreateTable)` となり emitter 到達 → emitter 未対応により hard-fail となることを、wasm `convert_to` の戻り値 (`JsConversionResult::Error`、ただしメッセージは emitter 起因) で検証。T3/T4/T5 完了後に DDL も正常 emit されることに更新する。
    - **R2 (純 DML バッチ無回帰)**: `SELECT 1;` のみ等の DDL を含まないバッチは、T2 前後で `to_common_sql` / `convert_to` の出力が不変であることを検証 (DML パスの無回帰)。
- **品質ゲート** (retrospective P-003 準拠、**単 crate 実行は不可**):
  - `cargo fmt --all --check`
  - `cargo clippy --all-targets -- -D warnings`
  - **`cargo nextest run --workspace`** (単 `-p tsql-parser` では wasm/mysql/postgresql/sqlite emitter の downstream 回帰が見えない。T2.4 parity テストが emitter 回帰を担うため workspace 実行が必須)
  - 完了宣言には nextest の `N tests run: N passed` Summary 行を貼付 (Definition of Ready)

---

## Group B (3 emitter DDL — T2 完了後に3者完全独立並列実行可能)

> 並列化の前提: T3/T4/T5 はそれぞれ別ファイル (`crates/<dialect>-emitter/src/ddl.rs` 等) を編集するため、コンフリクトしない。並列実装時は git worktree 分離推奨 (`parallel-impl-workflow.md` 準拠)。

### Task 3.1: (T3) MySQL emitter — DDL 5文型 emittance

- **依存**: T2 完了
- **files**: `crates/mysql-emitter/src/lib.rs`, `crates/mysql-emitter/src/ddl.rs` (新設)
- **内容**:
  - 現状の `EmitError::UnsupportedStatement` 返却 (mysql `lib.rs` DDL dispatch) を実装:
    - `Statement::CreateTable` → MySQL `CREATE TABLE` 文字列生成 (`Identifier` をバッククォート、`TableOptions` を末尾付与)
    - `Statement::AlterTable` → 6 `AlterTableAction` variants の MySQL 変換
    - `Statement::DropTable` / `CreateIndex` / `DropIndex` 同様
- **テスト要件**: 5文型 × 正常系各1 + 制約あり CREATE TABLE 1 = 計6テスト以上
- **品質ゲート**: 個別 `cargo nextest run -p mysql-emitter`

### Task 4.1: (T4) PostgreSQL emitter — DDL 5文型 emittance

- **依存**: T2 完了 (T3 と並列可能)
- **files**: `crates/postgresql-emitter/src/lib.rs`, `crates/postgresql-emitter/src/ddl.rs` (新設)
- **内容**: T3 と同構造だが、識別子をダブルクォート、`SERIAL`/`BIGSERIAL` 検討 (AutoIncrement)、方言固有の型マッピング。
- **テスト要件**: 5文型 × 正常系各1 + 計6テスト以上
- **品質ゲート**: `cargo nextest run -p postgresql-emitter`

### Task 5.1: (T5) SQLite emitter — DDL 5文型 emittance (ALTER 制限付き)

- **依存**: T2 完了 (T3/T4 と並列可能)
- **files**: `crates/sqlite-emitter/src/lib.rs`, `crates/sqlite-emitter/src/ddl.rs` (新設)
- **内容**: T3/T4 と同構造だが、`ALTER COLUMN` (型変更) / `DROP CONSTRAINT` は `EmitError::Unsupported` 維持 (design §0.4 SQLite 制限)。
- **テスト要件**: CREATE/DROP TABLE + CREATE/DROP INDEX + ADD COLUMN + (ALTER COLUMN 型変更で Unsupported) = 計6テスト以上
- **品質ゲート**: `cargo nextest run -p sqlite-emitter`

---

## Group C (schema-diff crate core)

### Task 6.1: (T6) schema-diff crate スケルトン + CatalogProvider trait + mapper.rs

- **依存**: T1 gate 通過 (T2 とは独立、T3/T4/T5 とも独立)
- **files**: `crates/schema-diff/Cargo.toml`, `crates/schema-diff/src/{lib,catalog,mapper,warning}.rs` (design §6 レイアウト)
- **内容**:
  - `Cargo.toml` (design §6.1) を作成し workspace `Cargo.toml` の members に追加
  - §3 の `CatalogSchema`/`CatalogTable`/`CatalogColumn`/`CatalogIndex`/`CatalogProvider` trait/`CatalogError` を実装
  - §7 mapper.rs の3関数 (`create_table_to_catalog`/`catalog_to_create_table`/`catalog_to_create_index`) を実装
  - §2.6 `MigrationWarning` を実装
- **テスト要件** (TDD red 先行):
  - mapper 相互変換のラウンドトリップテスト (CatalogTable → CreateTableStatement → CatalogTable で等価) 正常系3
  - CatalogError 各 variant の Display 実装テスト
- **品質ゲート**: `cargo nextest run -p schema-diff`

### Task 8.1: (T8) CatalogProvider の JSON adapter (feature gate 外)

- **依存**: T6 完了 (T6 と直列、Group B とは並列可能)
- **files**: `crates/schema-diff/src/adapters/{mod,json}.rs`
- **内容**:
  - `JsonCatalogProvider` (catalog JSON dump 文字列 → `CatalogSchema`) を実装
  - `serde`/`serde_json` を dev ではなく通常依存に追加 (T11 CLI も使うため)
  - design §0.1 の feature gate 設計 (`ase` feature は default off) を遵守
- **テスト要件**: UC-1/UC-2 のカタログ JSON を入力としたパーステスト 正常系3 + 不正 JSON で `CatalogError::ParseFailed` エッジケース1
- **品質ゲート**: `cargo nextest run -p schema-diff`

---

## Group D (diff engine 統合 — Group B/C 完了後)

### Task 7.1: (T7) diff_schema 純粋関数 + plan_operations + to_statements

- **依存**: T6 完了 (T2 完了も必要、desired 構築のため)。T3/T4/T5 完了で end-to-end 検証可能。
- **files**: `crates/schema-diff/src/{diff,emit}.rs`
- **内容**:
  - §2 `SchemaDiff`/`TableDiff`/`ColumnDiff`/`ColumnChange`/`IndexDiff`/`ConstraintDiff` を実装
  - §5 `diff_schema(current, desired) -> SchemaDiff` 純粋関数 (AC-4)
  - §4 `AlterOperation` + `plan_operations(SchemaDiff) -> Vec<AlterOperation>` + `to_statements(ops) -> Vec<Statement>`
  - 破壊的変更検出 (ナロー化判定: VARCHAR 長縮小 / DECIMAL 精度低下 / NOT NULL 追加 / DROP) → `MigrationWarning::Destructive` (方針 A)
- **テスト要件** (TDD red 先行):
  - UC-1 (空 → 1テーブル) で SchemaDiff.table_diffs に `TableDiff::Added` が1つ
  - UC-2 (カラム追加) で `ColumnDiff::Added`
  - UC-3 破壊的3種で `MigrationWarning::Destructive` が3つ生成されること
  - 同一スキーマで `SchemaDiff` が空 (差分なし)
  - エッジケース: desired/current とも空で warnings 空
  - 計8テスト以上
- **品質ゲート**: `cargo nextest run -p schema-diff`

### Task 10.1: (T10) SQLite ALTER 制限ハンドリング

- **依存**: T7 完了
- **files**: `crates/schema-diff/src/emit.rs`
- **内容**:
  - design §0.4 準拠: SQLite 向け `to_statements` で `AlterColumn` (型変更) / `DropConstraint` を検知した場合 `MigrationWarning::UnsupportedDialect` を付与しつつ、サポート範囲の SQL のみ生成
- **テスト要件**: SQLite 向け ALTER COLUMN 型変更で警告生成テスト1 + ADD COLUMN は警告なしテスト1
- **品質ゲート**: `cargo nextest run -p schema-diff`

---

## Group E (adapter + CLI — feature gate 設計の要)

### Task 9.1: (T9) ase-rs adapter (feature `ase` gate 内)

- **依存**: T8 完了 (CatalogProvider trait 実装対象のため)。T11 とは独立。
- **files**: `crates/schema-diff/src/adapters/ase.rs`, `crates/schema-diff/Cargo.toml` (`ase` feature 追加)
- **内容**:
  - `#[cfg(feature = "ase")]` 配下で `AseCatalogProvider` を実装 (ase-rs でASE接続 → カタログクエリ → `CatalogSchema` 構築)
  - design §0.1 の feature gate を厳守: `ase` 無しビルドではコンパイル対象外
  - ASE 固有データ型 → common-sql `DataType` (24 variants) へのマッピング表を実装
- **テスト要件** (feature gate 内):
  - ASE 型マッピング単体テスト (ase-rs 接続不要、型変換関数のみ) 正常系3 + 未対応型で `CatalogError::UnsupportedCatalogShape` エッジケース1
  - ※ 実ASE接続テストは CI ではスキップ (`#[ignore]` + ASE エンドポイント必須)
- **品質ゲート**: `cargo nextest run -p schema-diff --features ase` (ローカル) / `cargo nextest run -p schema-diff` (default、ase.rs はコンパイル外)

### Task 11.1: (T11) CLI + catalog JSON dump 入力 (feature gate 外、publishable)

- **依存**: T8 完了。T9 とは独立 (T9 が無くても CLI は JSON 入力で完全動作 — estimate 条件 #6)。
- **files**: `crates/schema-diff/src/bin/schema-diff.rs` (binary target) または `examples/` 配下
- **内容**:
  - CLI 引数: `--current <catalog.json>` / `--desired <ddl.sql>` / `--dialect mysql|postgresql|sqlite`
  - `JsonCatalogProvider` で current を、`build_desired_schema` (T2/T6 拡張) で desired を構築
  - `diff_schema` → `plan_operations` → `to_statements` → 該当方言 emitter で SQL 文字列化し STDOUT へ
  - `MigrationWarning` を STDERR へ人間可読出力
  - **design §0.1 準拠**: `ase` feature 無しでコンパイル・実行可能 (AC-5/AC-6)
- **テスト要件**: UC-1/UC-2 を catalog JSON 入力で再現する統合テスト2 + 不正入力で graceful 終了1
- **品質ゲート**: `cargo nextest run -p schema-diff` (default features)

---

## Group F (最終品質 gate)

### Task 12.1: (T12) workspace 全体品質 gate

- **依存**: 全タスク完了
- **内容** (`.claude/rules/pre-commit-rust.md` 準拠):
  ```bash
  cargo fmt --all --check
  cargo clippy --all-targets -- -D warnings
  cargo nextest run --workspace
  ```
- **検証**: 上記3コマンドの出力 (nextest は `N tests run: N passed` の Summary 行) を完了宣言に貼付 (pre-implementation-checklist.md / Definition of Ready 準拠)。

---

## ファイル所有権マップ (並列コンフリクト回避)

| Group | Task | files (新規/編集) |
|-------|------|-------------------|
| A | T2 | `crates/tsql-parser/src/ast/to_common_sql.rs` (+関連変換関数ファイル) |
| B | T3 | `crates/mysql-emitter/src/{lib,ddl}.rs` |
| B | T4 | `crates/postgresql-emitter/src/{lib,ddl}.rs` |
| B | T5 | `crates/sqlite-emitter/src/{lib,ddl}.rs` |
| C | T6 | `crates/schema-diff/Cargo.toml`, `src/{lib,catalog,mapper,warning}.rs` |
| C | T8 | `crates/schema-diff/src/adapters/{mod,json}.rs` |
| D | T7 | `crates/schema-diff/src/{diff,emit}.rs` |
| D | T10 | `crates/schema-diff/src/emit.rs` (T7 続き) |
| E | T9 | `crates/schema-diff/src/adapters/ase.rs`, `Cargo.toml` (feature 行のみ) |
| E | T11 | `crates/schema-diff/src/bin/schema-diff.rs` |
| F | T12 | (編集なし、検証のみ) |

**競合リスト** (要調整):
- `crates/schema-diff/Cargo.toml`: T6 (作成) → T9 (`ase` feature 行追加)。T9 は T6 完了後に直列実行。
- `crates/schema-diff/src/emit.rs`: T7 (作成) → T10 (SQLite 分岐追加)。T10 は T7 完了後に直列実行。

## チェックリスト (実行前)

> **状態 (2026-07-24 update)**: T1 meta gate は CTO 承認済み。T2-T11 全タスク master 着地済み (PR #181/#183/#185/#187/#189/#191/#193/#201)。Issue #162 CLOSED (2026-07-20)。以下は実行前 gate として満たされた項目の実績記録。

- [x] design gate (CTO) が T1 を承認 (全フィールド列挙 / dangling 参照解決 / 破壊的変更方針 / common-sql stability contract / ASE 依存決定 / **wasm silent-drop→hard-fail parity (§0.5)** / **DataType 非対応型短絡方針 (§0.6)** の7点)
- [x] **`.kiro/specs/schema-diff/` が git tracked であること** (spec 未コミット状態での ESTIMATE 承認・実装着手は不可 — parallel-impl-workflow 前提)
- [x] T2 が Group B/C 解放の hard prereq であることを全員が認識
- [x] T11 CLI が `ase` feature 無しで完全テスト可能であることを T1 時点で検証済み (design §0.1 + §5)
- [x] 並列実行時は `parallel-impl-workflow.md` の git worktree 分離を適用 (同じファイルへの並列編集回避)
- [x] T2 の品質 gate が `cargo nextest run --workspace` であることを Engineer が認識 (単 `-p tsql-parser` は P-003 違反)
