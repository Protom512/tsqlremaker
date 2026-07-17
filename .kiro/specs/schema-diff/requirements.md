# Requirements: schema-diff (sqldef 相当スキーマ差分マイグレーション生成)

> Feature: `feat: sqldef-like schema-diff migration generation (ase-rs + tsqlremaker)` — Issue #162
> 優先度: P2 / サイズ: XL
> 言語: 日本語 (`spec.json.language = "ja"`)

## 1. ペルソナ (Persona)

**SAP ASE 移行エンジニア / DBA**。数百テーブルの ASE スキーマを MySQL / PostgreSQL / SQLite に
継続マイグレーションする作業を担う。既存の手作業 DDL 書き換えは頻度が高く、転記漏れ・型誤変換・
制約忘れが繰り返し発生している。sqldef の DX（＝ desired schema を与えると差分 ALTER/CREATE/DROP を
生成してくれる）を ASE ソースに対して得たいが、既存 sqldef 実装は ASE カタログをイントロスペクトできない。

## 2. 動機 (Motivation)

現状の tsqlremaker は「T-SQL → 3方言エミッション」の部品 (Lexer/Parser/common-sql AST/3 emitter) は揃ったが、
sqldef の核心である **差分エンジン** (catalog→AST mapper / diff algorithm / ALTER-generation emitter) が未実装である。
また 3 emitter とも DDL 系 (`CreateTable`/`AlterTable`/`DropTable`/`CreateIndex`/`DropIndex`) は
`EmitError::Unsupported` を返す実装途上状態にあり (mysql: `UnsupportedStatement` / pg・sqlite: `Unsupported`)、
`to_common_sql` も `Statement::Create(_) | AlterTable(_) | BatchSeparator(_) => None` で DDL を未処理 (`crates/tsql-parser/src/ast/to_common_sql.rs:144` 検証済み)。

本機能は「競合なしの差別化能力」になる。ASE 実カタログ (ase-rs) と tsqlremaker の common-sql AST hub +
3 emitter を compose し、ASE スキーマから各方言への **冪等な差分マイグレーション SQL** を自動生成する。

## 3. ユースケース (Use Cases)

### UC-1: 新規テーブルのマイグレーション (CREATE 生成)

**入力 (desired schema, DDL ファイル):**
```sql
CREATE TABLE users (
    id   BIGINT NOT NULL,
    name VARCHAR(255) NOT NULL,
    CONSTRAINT pk_users PRIMARY KEY (id)
);
```
**入力 (current catalog, JSON ダンプ — `ase` feature 無しでも与えられる):**
```json
{ "tables": [] }
```
**期待出力 (MySQL):**
```sql
CREATE TABLE `users` (
    `id` BIGINT NOT NULL,
    `name` VARCHAR(255) NOT NULL,
    CONSTRAINT `pk_users` PRIMARY KEY (`id`)
);
```

### UC-2: カラム追加の差分 (ALTER 生成)

**入力 (desired):** UC-1 に `email VARCHAR(255) NULL` を追加
**入力 (current catalog):** UC-1 適用後 (id, name のみ)
**期待出力 (PostgreSQL):**
```sql
ALTER TABLE "users" ADD COLUMN "email" VARCHAR(255);
```

### UC-3: 破壊的変更の検出と警告 (DROP / ナロー化 / NOT NULL 追加)

**入力 (desired):** `name VARCHAR(50) NOT NULL` へナロー化 + `legacy_col` を DROP
**入力 (current catalog):** `name VARCHAR(255)`, `legacy_col INT`
**期待出力:** マイグレーション SQL に加え、破壊的変更としての **警告リスト** (設計決定 A: 実行は継続、停止はしない — 詳細は `design.md` §破壊的変更方針)。

```text
WARN destructive: column "users"."legacy_col" will be DROPPED
WARN destructive: column "users"."name" type narrows VARCHAR(255) -> VARCHAR(50)
WARN destructive: column "users"."name" nullability tightens NULL -> NOT NULL
```

## 4. 非スコープ (Non-Goals)

- **データマイグレーション**: 行データの変換・コピーは対象外 (スキーマのみ)。
- **マイグレーション実行**: 生成された SQL の DB 適用実行は対象外 (ファイル/STDOUT 出力のみ)。
- **down マイグレーション (ロールバック)**: 現状 `current → desired` の up 方向のみ。down は将来課題。
- **ASE 固有オブジェクト** (trigger / procedure / rule / default / view): カタログ情報は表示するが差分 SQL は生成しない (parser/`common-sql` が未対応のため)。
- **スキーマ名 (`dbo.users`) を跨ぐ差分**: 単一スキーマ (通常 `dbo`) に限定。
- **ストアドの内容差分** (CREATE PROCEDURE body diff): 対象外。

## 5. 受入基準 (Acceptance Criteria)

- [ ] **AC-1**: UC-1 (空カタログ vs 1テーブル desired) で、3方言 (MySQL/PostgreSQL/SQLite) すべての CREATE TABLE SQL が生成される。
- [ ] **AC-2**: UC-2 (カラム追加) で、3方言の ALTER TABLE ... ADD COLUMN SQL が生成される。
- [ ] **AC-3**: UC-3 の破壊的変更3種 (DROP COLUMN / 型ナロー化 / NOT NULL 追加) がすべて **警告** として報告され、かつ SQL 自体は生成される (設計決定 A 準拠)。
- [ ] **AC-4**: `diff_schema(current, desired)` は純粋関数であり、IO を行わない (catalog fetch を含まない)。これはテスト容易性と `ase` feature gate 外での完全テストを保証する。
- [ ] **AC-5**: `CatalogProvider` trait を介した catalog 取得は feature flag `ase` の背後に隠蔽され、`ase` 無しビルドでも `cargo check`/`nextest` が通る (publishable 維持)。
- [ ] **AC-6**: `catalog JSON dump` を入力とした CLI が、`ase` feature 無しでコンパイル・実行でき、UC-1/UC-2 と同一結果を再現する (ase-rs git 依存無しでの完全テスト経路)。
- [ ] **AC-7**: `cargo fmt --all --check` / `cargo clippy --all-targets -- -D warnings` / `cargo nextest run --workspace` がすべて成功する。

## 6. 依存 (Dependencies)

### 前提 (prerequisite, 別タスク T2)
- **T2 (parser→common-sql DDL 橋渡し)**: `to_common_sql` が `Statement::Create(_)` を `CreateTable`/`CreateIndex` に、`Statement::AlterTable(_)` を `AlterTable` にマップするよう拡張されること。現状 (`to_common_sql.rs:144`) は `=> None` で未対応。これが無いと desired 側 DDL をパースして共通 AST に乗せられない。

### 利用する既存部品 (検証済み)
- **common-sql AST** (`crates/common-sql/src/ast/`): `Statement` (10 variants, DDL 5種含む), `CreateTableStatement`, `AlterTableStatement` + `AlterTableAction` (6 variants), `ColumnDef`, `TableConstraint` (4), `ColumnConstraint` (5), `DataType` (24, Eq+Hash), `Identifier`, `QualifiedName`, `Span`。これらは既存完全実装。
- **3 emitter**: mysql/pg/sqlite — 現状 DDL は `Unsupported` 返却 (T3/T4/T5 で DDL emittance を追加実装する前提)。

### 外部依存 (feature gate で隔離)
- **ase-rs** (非 crates.io 公开 git upstream): ASE カタログイントロスペクション。feature `ase` の背後の adapter (T9) 経由のみで利用。デフォルトビルドからは隔離。

## 7. リスク (Risks, estimate 引継ぎ + 検証結果)

1. **DDL emittance 未実装** (検証済み): 3 emitter とも DDL 系は `Unsupported`。T3/T4/T5 が必要。
2. **SQLite ALTER 制限**: SQLite は `ALTER COLUMN` (型変更) / `DROP CONSTRAINT` をネイティブ非サポート。設計決定事項として design.md で明示 (table-rebuild 方針 or unsupported 通知)。
3. **破壊的変更の誤適用**: 設計決定 A (警告・継続) により軽減。
4. **common-sql AST の前方互換性**: schema-diff と将来分離レポ `ase-schema-def` が public contract として common-sql に依存 → design.md で stability contract 宣言。
5. **ase-rs 非公開 upstream の不安定性**: feature-flag trait 抽象 (CatalogProvider) で隔離。直接 git 依存は却下 (estimate 承認条件 #1)。
6. **型ナロー化の判定**: VARCHAR 長縮小等の「安全でない変更」検出ロジックの抜け漏れ。
7. **カラム名リネーム検出**: ADD+DROP のペアを RENAME と誤判定しないよう、初期版は RENAME 検出なし (非スコープ化)。
8. **インデックス順序依存**: PK/UNIQUE 制約と独立インデックスの重複報告を避ける正規化。
