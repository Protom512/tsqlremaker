# Design: schema-diff (sqldef 相当スキーマ差分マイグレーション生成)

> Feature: Issue #162 / 優先度 P2 / サイズ XL
> 言語: 日本語 (`spec.json.language = "ja"`)
> **2026-07-09 レトロスペクティブ P5 ルール準拠**: 本設計は全構造体フィールド (名前・型・pub/non-pub) を列挙し、全参照型を同一設計書内または検証済み既存コード内で解決する。dangling 参照・「後で決める」フィールド形状は存在しない。

## 0. 設計 gate 前に凍結した決定事項

> 本節の 0.1〜0.6 は design gate (CTO / `kiro:validate-design`) 承認の前提条件である。特に **0.5 (wasm silent-drop→hard-fail) と 0.6 (DataType 非対応型短絡)** は T2 (parser→common-sql DDL 橫渡し) に起因する破壊的契約変更であり、T2 実装に入る前に凍結必須 (estimate DEFER 条件)。

### 0.1 ASE 依存決定 (estimate 承認条件 #1) — feature-flag trait 抽象を採用

**決定**: `ase-rs` (非 crates.io 公開 git upstream) への **直接 git 依存は却下**。
`CatalogProvider` trait (§3) を介した **feature-flag (`ase`) 隔離** のみを採用する。

**理由**:
- `architecture-coupling-balance.md` §4.1 (腐敗防止層 / ACL) の模範適用。高変動性 (非公開 upstream) を隔離する唯一の手段。
- `ase` feature 無しでも crate が publishable であり、catalog JSON dump 入力 (T11) により diff/migration 層が **ase-rs に依存せず完全テスト可能** (AC-5/AC-6/estimate 条件 #6)。
- 直接 git 依存は、upstream の breaking change が本 crate のデフォルトビルドを壊すリスクを抱えるため却下。

**影響**: T9 (ase-rs adapter) のみが `ase` feature の背後にあり、`ase-rs = { git = "..." }` 依存はオプショナル。T11 CLI の catalog JSON 入力経路は feature gate 外で常時コンパイル可能。

### 0.2 破壊的変更方針 (estimate 承認条件 #3) — 設計決定 A: 警告・継続

**決定**: 破壊的変更 (型ナロー化 `VARCHAR(255)->VARCHAR(50)` / NOT NULL 追加 / カラム DROP) は
**警告 (`MigrationWarning::Destructive`) として報告し、マイグレーション SQL 生成は継続** する。
エラーで停止はしない。

**理由**:
- DBA は警告を見て判断する運用フローを前提。停止 (error) は大量の既存スキーマ適用を阻害する。
- 警告は構造化 (`MigrationWarning` enum) に保持し、CLI (T11) が STDERR に人間可読形式で出力。テストは警告の *存在と種別* を検証可能。
- 将来オプション: `--strict-destructive` フラグで警告を error に格上げ (初期版の非スコープだが enum 設計で拡張点を残す)。

### 0.3 common-sql AST 前方互換性保証 (stability contract, estimate 承認条件 #4)

**決定**: `common_sql::ast::*` は **public stability contract** を宣言する。
schema-diff および将来分離レポ (`ase-schema-def`) が public contract として common-sql AST に依存するため、
以下を design 時に文書化する。

- **破壊的変更の波及範囲**: common-sql AST のフィールド削除・型変更・variant 削除は、3 emitter・parser converter・schema-diff の全コンパイルを破壊する (`#[derive(PartialEq)]` と完全パターンマッチにより即座に検出)。
- **許容される進化**: (a) enum への variant 追加 (non-exhaustive 扱い、`_` arm 必須), (b) struct フィールド追加 (デフォルト値付き `Default` 実装維持), (c) アクセサメソッド追加。
- **必須フック**: variant 追加時は `visitor.rs` の `Visitor` trait メソッド追加を併せて行う。
- **本設計が依存する不変前提**: `Identifier`/`QualifiedName` は private フィールド + accessor (`value()`/`schema()`/`name()`) であり、schema-diff は accessor 経由のみで利用する (検証: `crates/common-sql/src/ast/identifier.rs`)。`DataType` は `Eq + Hash` を維持し、差分キーとして利用可能。

### 0.4 SQLite ALTER 制限の取扱 (risk #2)

**決定**: 初期版は SQLite に対し、**ネイティブ非サポートの操作 (`ALTER COLUMN` 型変更 / `DROP CONSTRAINT`) は `MigrationWarning::UnsupportedDialect` 警告を出しつつ、可能な範囲の SQL のみ生成** する。table-rebuild (一時テーブル作成→コピー→入れ替え) は将来 L-XL 課題として非スコープ。`ADD COLUMN` / `DROP COLUMN` (SQLite 3.35+) はサポートする。

### 0.5 wasm `convert_to` の DDL 混在バッチ挙動 — silent-drop → hard-fail 反転 (T2 由来の破壊的契約変更)

> **本決定は T2 (parser→common-sql DDL 橫渡し) が引き起こす wasm 公開契約の破壊的変更を凍結するものである。**
> T2 完了前に本節を `kiro:validate-design` が承認しなければ、T2 実装に入れない (estimate DEFER 条件 #1/#2)。

#### 現状 (T2 前) の契約

`crates/wasm/src/lib.rs:185` の公開関数 `convert_to` は:

```rust
let common_stmts: Vec<_> = stmts
    .iter()
    .filter_map(|stmt| to_common_sql(stmt))  // DDL → None で静かに除外
    .collect();
```

`to_common_sql` が `Statement::Create(_) | AlterTable(_) | BatchSeparator(_) => None` を返す (`to_common_sql.rs:144`) ため、**DDL 文は `filter_map` で静かにドロップされる (silent-drop)**。結果:

- **純 DDL バッチ** (例: `CREATE TABLE users (id INT)` のみ): `common_stmts.is_empty()` (`lib.rs:188`) に到達し、`JsConversionResult::Error { message: "Statement contains unsupported features for conversion" }` を返す。
- **DDL + DML 混在バッチ** (例: `CREATE TABLE users (id INT); SELECT * FROM users;`): DDL は silent-drop され、**DML のみが emit される** (部分出力、エラー無し)。これは現状の「利用可能な文だけ出力」という暗黙契約である。

#### T2 後の挙動反転 (影響分析)

T2 が `to_common_sql` の `Statement::Create(_) => None` を `Some(SqlStmt::CreateTable(...))` 等に変更すると、DDL 文は `filter_map` を通過して emitter に到達する。しかし 3 emitter はまだ DDL emittance を実装しておらず (T3/T4/T5 未完了):

- mysql: `Statement::CreateTable(_) | AlterTable(_) | DropTable(_) | CreateIndex(_) | DropIndex(_) => Err(EmitError::UnsupportedStatement)` (`crates/mysql-emitter/src/statement.rs:74-81`)
- postgresql: 同様 `Err(EmitError::Unsupported)` (`crates/postgresql-emitter/src/lib.rs:194-197`)
- sqlite: 同様 `Err(EmitError::Unsupported)` (`crates/sqlite-emitter/src/lib.rs:157-160`)

**帰結**: T2 直後 (T3/T4/T5 完了前) の中間状態では、DDL+DML 混在バッチが現状の **silent-drop (DML のみ emit)** から **hard-fail (emit 結果 `EmitError` で全件出力無し)** に反転する。これは wasm の公開契約の破壊的変更であり、`convert_to` の利用者が「DDL が含まれていても DML は出力される」と前提している場合、出力が空になる挙動変化となる。

#### 設計決定: hard-fail を意図的に受け入れる (silent-drop は維持しない)

**採用決定**: **silent-drop は廃止し、T2 完了後は DDL 到達 = hard-fail とする。** ただし T2 単体では emitter 未対応により wasm `convert_to` の DDL 系バッチ出力が空になる中間状態を許容し、**T3/T4/T5 完了によって DDL も正常 emit されるよう完成后に契約が安定化する**ことを文書化する。

**理由**:

1. **silent-drop の再現コストが高く、かつ誤魔化しである**: T2 完了後にあえて wasm 側で「DDL variant を事前スキップして filter_map に掛けない」ラッパーを挟むことは、DDL が無視されていることを呼び出し側から隠す。schema-diff の全体ビジョン (#162) が「DDL を正しく扱う」ことにあるため、DDL 静的除外はアーキテクチャと矛盾する。
2. **T2〜T5 の中間状態は短命**: 並列ワークフロー (tasks.md Group B) により T3/T4/T5 は T2 完了直後に3者並列で着工される。中間状態 (DDL が hard-fail) の期間は Group B 完了までであり、恒久仕様ではない。`spec.json.phase = "tasks"` 通過後の単一リリース単位で T2〜T5 を束ねる運用とする。
3. **wasm `convert_to` の呼び出し側は限定的**: 本 wasm は tsqlremaker の playgound/Demo 用であり、外部 API 消費者は存在しない。契約変更の波及はプロジェクト内テスト (`crates/wasm/src/lib.rs` のテスト、及び `wasm-feature-masks-emitter-rewiring` memory 教訓) に閉じる。

**波及する required action (tasks.md Task 2.1 受入基準に明記済み)**:

- T2 のテスト要件に、**wasm `convert_to` の DDL 混在バッチ回帰テスト** を含めること。具体的には:
  - **回帰テスト R1 (混在バッチ)**: `CREATE TABLE users (id INT); SELECT * FROM users;` を入力としたとき、T2 完了直後の中間状態では `convert_to` がエラー結果 (現状の `JsConversionResult::Error { "unsupported features" }` とは異なり、emitter 起因の変換失敗) を返すことを検証する。T3/T4/T5 完了後は DDL も正常 emit され、両文の出力が得られることに同じテストを更新する。
  - **回帰テスト R2 (純 DML バッチ)**: `SELECT 1;` のみ等の DDL を含まないバッチは、T2 前後で出力が不変であることを検証する (DML パスの無回帰)。
- **品質 gate は `cargo nextest run --workspace` 必須** (単 `-p tsql-parser` では wasm/emitter 回帰が見えない、tasks.md 修正済み)。

**非スコープ (本決定では扱わない)**:

- wasm `convert_to` の API 形状変更 (引数/戻り値の型変更) は行わない。`JsConversionResult` enum は維持し、エラーメッセージの内容のみ emitter 起因に変わる。
- wasm 側での DDL 事前スキップラッパー (silent-drop 再現) は実装しない (上記理由 1)。
- T2 の完了定義を「T3/T4/T5 の完了まで含める」ことではない。T2 単体は to_common_sql の拡張と単体テストで完了とし、wasm 回帰テストは T2 が追加する (downstream parity 検証 = T2.4)。中間状態の hard-fail は T2 単体テストで「期待挙動」として固定する。

### 0.6 T-SQL DataType 非対応型の短絡方針 (T2 実装者が推測しない)

T2 が `convert_create_table` / `convert_column_def` を実装する際、`tsql_parser::ast::DataType` (27 variants, `crates/tsql-parser/src/ast/ddl.rs:94-141`) から `common_sql::ast::DataType` (24 variants, `crates/common-sql/src/ast/datatype.rs:5-102`) への変換が必要である。両 enum は完全には一致せず、非対応型の取扱を実装者が推測しないよう本節で凍結する。

#### 対応表 (検証済み)

| T-SQL `DataType` | common-sql `DataType` | 備考 |
|------------------|----------------------|------|
| `Int` | `Int` | 直接マップ |
| `SmallInt` | `SmallInt` | 直接マップ |
| `TinyInt` | `TinyInt` | 直接マップ |
| `BigInt` | `BigInt` | 直接マップ |
| `Varchar(Option<u32>)` | `VarChar { length: Option<u64> }` | `u32 → u64` 拡幅 |
| `Char(u32)` | `Char { length: Option<u64> }` | `u32 → u64` 拡幅 (Char は常length付きだが dest は Option、`Some(n as u64)`) |
| `Decimal(Option<u8>, Option<u8>)` | `Decimal { precision, scale }` | 直接マップ |
| `Numeric(Option<u8>, Option<u8>)` | `Numeric { precision, scale }` | 直接マップ |
| `Float` | `DoublePrecision` | T-SQL `FLOAT` はデフォルト double precision |
| `Real` | `Real` | 直接マップ |
| `Double` | `DoublePrecision` | 直接マップ |
| `Date` | `Date` | 直接マップ |
| `Time` | `Time { precision: None }` | T-SQL `TIME` は精度を持たない → `None` |
| `Datetime` | `DateTime { precision: None }` | 直接 (精度 `None`) |
| `Timestamp` | `Timestamp { precision: None }` | 直接 (精度 `None`) |
| `Text` | `Text` | 直接マップ |
| `Binary(u32)` | `Binary { length: Some(n as u64) }` | 拡幅 |
| `VarBinary(Option<u32>)` | `VarBinary { length: Option<u64> }` | 拡幅 |
| `UniqueIdentifier` | `Uuid` | 直接マップ (ASE `UNIQUEIDENTIFIER` = `UUID`) |
| **`SmallDateTime`** | *(非対応)* | common-sql に相当 variant 無し |
| **`Bit`** | *(非対応)* | common-sql に相当 variant 無し (Boolean は意味が異なる) |
| **`Money`** | *(非対応)* | common-sql に金額型無し (Decimal への正規化は精度情報欠落) |
| **`SmallMoney`** | *(非対応)* | 同上 |

#### 短絡方針 (決定): 文全体 `None` 短絡 (列単位では無い)

**決定**: 非対応型 (`SmallDateTime` / `Bit` / `Money` / `SmallMoney`) を含む `CREATE TABLE` / `ALTER TABLE ... ADD COLUMN` 変換は、**その列のみならず文全体を `None` に短絡する** (文単位 None)。

**理由**:

1. **現行 `convert_select` のパリティ契約と一致**: `to_common_sql.rs:172-249` の `convert_select` は、部分式が `None` になった場合 `.and_then` で飲み込まず `?` で文全体を `None` にする (明示的なパリティ契約コメント: 「変換不能な項目があれば文全体を None にする。filter_map で飲み込むと暗黙に消失する」)。CREATE TABLE もこれに合わせることで、converter 全体の短絡粒度が一貫する。
2. **列単位 None は corrupt な CREATE TABLE を生成する**: ある列だけ `None` で落とすと、`CREATE TABLE users (id INT, name VARCHAR(255))` が `CREATE TABLE users (id INT)` に化ける。これは schema-diff の desired schema 構築で致命的 (存在する列が欠落) であり、警告ではなく明示的な失敗が適切。
3. **wasm parity (§0.5) と整合**: 文全体 `None` の場合、wasm `convert_to` は DDL を silent-drop せず `filter_map` で除外 → 混在バッチは DML のみ残存するが、これは §0.5 の「T2 直後の中間状態」ではなく「非対応型を含む DDL」のケース。§0.5 は「DDL variant 自体が emitter 未対応」による hard-fail を論じており、本節は「DDL variant 内の非対応 DataType による converter `None`」を論じる。両者は直交し、後者は §0.5 の silent-drop 議論の対象外 (現行通り `filter_map` で落ちる) である。

**実装上の契約**:

- `convert_data_type(dt: &tsql::DataType) -> Option<csql::DataType>` を新設する。
- 非対応型 (`SmallDateTime` / `Bit` / `Money` / `SmallMoney`) は `return None`。
- `convert_column_def` は `convert_data_type(col.data_type)?` と `?` で伝播 (列 None → 文 None)。
- `convert_create_table` は全列を走査し、いずれかの列が `None` なら全体 `None` (`?` 伝播で自然に実現)。
- `convert_alter_table` (`AddColumn` action の場合) も同様。

**T9 ASE カタログ側の短絡との対応**: 本節の `convert_data_type → None` 短絡は
**desired 側 (DDL ソース → CatalogSchema 構築, T2/T7)** の契約である。
**current 側 (live ASE カタログ → CatalogSchema, T9)** の対応する short-circuit は
`map_ase_type` が非対応 ASE 型 (`AseDataType`) を検出した際に
`CatalogError::UnsupportedCatalogShape` を返す経路 (§3.5.1)。これにより desired/current
両経路で非対応型が一貫して表面化し、暗黙ドロップは発生しない。
さらに T9 では CTO 2026-07-14 条件 #3 により、design.md が具体的カタログ問い合わせ
(sysobjects/syscolumns/sysindexes) を規定しない表面 (= T9b イントロスペクション未実装) は
暗黙の空スキーマではなく `CatalogError::NotImplemented` (§3.5.1) として明示的に表面化する
(実装: `crates/schema-diff/src/catalog.rs:96-106`)。

**テスト要件** (tasks.md Task 2.1 に明記):

- 非対応型 (`Bit` 等を含む) CREATE TABLE は `to_common_sql` が `None` を返すこと。
- `Money` / `SmallMoney` / `SmallDateTime` 各々で `None` を返すこと。
- 対応型 `UniqueIdentifier` は `Uuid` にマップされ `Some` を返すこと (非対応型ではないことの検証)。

## 1. アーキテクチャ概要

```
┌─────────────────────────────────────────────────────────────┐
│                    schema-diff crate                        │
│  (workspace member, depends only on common-sql)            │
│                                                             │
│  ┌───────────────┐    ┌──────────────┐    ┌──────────────┐ │
│  │ CatalogProvider│   │  diff_schema │    │ emit_migration│ │
│  │    trait       │──>│  (純粋関数)  │──> │  (各 dialect) │ │
│  └───────┬───────┘    └──────────────┘    └──────┬───────┘ │
│          │                                      │         │
│   ┌──────┴──────┐                          3 emitter       │
│   │ adapters/   │                          (既存, 拡張)    │
│   │ json.rs     │                                         │
│   │ ase.rs ▼    │  feature `ase` gate                    │
│   └─────────────┘                                         │
└─────────────────────────────────────────────────────────────┘
              │ common_sql::ast::*  (安定ハブ, 唯一)
              ▼
         (3 emitter / parser converter も同一 hub に依存)
```

依存の方向 (単方向, `architecture-coupling-balance.md` 準拠):
```
schema-diff ──> common-sql ──> (tsql-token Span のみ, common-sql 内部で再定義済み)
```
schema-diff は tsql-parser / ase-rs / 各 emitter に **直接依存しない**。emitter の出力方言差は schema-diff 側で生成した `common_sql::ast::Statement` (DDL) を emitter に渡す形 (T3/T4/T5 で DDL emittance を追加した後)。

## 2. データモデル — Diff 系 (全フィールド列挙)

> 全て `pub` で宣言 (schema-diff crate の public API)。`#[derive(Debug, Clone, PartialEq)]` を標準とする。

### 2.1 `SchemaDiff` (トップレベル差分結果)

```rust
/// desired と current のスキーマ全体の差分。
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaDiff {
    /// テーブル単位の差分 (テーブル名でソート済み、決定的順序)。
    pub table_diffs: Vec<TableDiff>,
    /// インデックス単位の差分 (テーブル名・インデックス名でソート済み)。
    pub index_diffs: Vec<IndexDiff>,
    /// 差分導出過程で発生した警告 (破壊的変更 / 非対応方言 等)。
    pub warnings: Vec<MigrationWarning>,
}
```
参照型: `TableDiff` (§2.2), `IndexDiff` (§2.4), `MigrationWarning` (§2.6) — 全て本設計書内で定義済み。

### 2.2 `TableDiff`

```rust
/// 1テーブル単位の差分。
#[derive(Debug, Clone, PartialEq)]
pub enum TableDiff {
    /// desired にのみ存在 (current 側に CREATE 必要)。
    Added {
        /// テーブル名 (スキーマ修飾なし、単一スキーマ前提)。
        name: String,
        /// CREATE されるテーブルの common-sql 定義。
        definition: common_sql::ast::CreateTableStatement,
    },
    /// current にのみ存在 (DROP 必要 → 破壊的警告対象)。
    Removed {
        /// テーブル名。
        name: String,
    },
    /// 両方に存在し、内容が異なる (ALTER 系操作のリスト)。
    Modified {
        /// テーブル名。
        name: String,
        /// カラム単位の差分 (カラム名でソート済み)。
        column_diffs: Vec<ColumnDiff>,
        /// 制約単位の差分 (制約名でソート済み、名称未指定は正規化名)。
        constraint_diffs: Vec<ConstraintDiff>,
    },
    /// 両方に存在し、内容が同一 (差分なし)。
    Unchanged {
        /// テーブル名。
        name: String,
    },
}
```
参照型: `common_sql::ast::CreateTableStatement` (検証済み: `crates/common-sql/src/ast/ddl.rs:124`, フィールド `span: Span`, `if_not_exists: bool`, `temporary: bool`, `name: QualifiedName`, `columns: Vec<ColumnDef>`, `constraints: Vec<TableConstraint>`, `options: TableOptions`)。`ColumnDiff` (§2.3), `ConstraintDiff` (§2.5)。

### 2.3 `ColumnDiff`

```rust
/// 1カラム単位の差分。
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnDiff {
    /// desired にのみ存在 (ADD COLUMN)。
    Added {
        /// カラム名。
        name: String,
        /// 追加されるカラム定義。
        column: common_sql::ast::ColumnDef,
    },
    /// current にのみ存在 (DROP COLUMN → 破壊的警告対象)。
    Removed {
        /// カラム名。
        name: String,
    },
    /// 両方に存在し、型/制約が異なる (ALTER COLUMN)。
    Modified {
        /// カラム名。
        name: String,
        /// 変更前カラム定義。
        from: common_sql::ast::ColumnDef,
        /// 変更後カラム定義。
        to: common_sql::ast::ColumnDef,
        /// 検出された変更の内訳 (型変更 / nullability / default)。
        changes: Vec<ColumnChange>,
    },
}
```
参照型: `common_sql::ast::ColumnDef` (検証済み: `ddl.rs:24`, フィールド `span: Span`, `name: Identifier`, `data_type: DataType`, `nullable: bool`, `default: Option<Expression>`, `constraints: Vec<ColumnConstraint>`)。`ColumnChange` (§2.3.1)。

#### 2.3.1 `ColumnChange`

```rust
/// ALTER COLUMN で変化した属性の内訳。
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnChange {
    /// データ型が変更された。
    TypeChanged {
        /// 変更前。
        from: common_sql::ast::DataType,
        /// 変更後。
        to: common_sql::ast::DataType,
        /// ナロー化 (安全でない変更) かどうか。
        is_narrowing: bool,
    },
    /// NULL許容性が変更された。
    NullabilityChanged {
        /// 変更前 (true = NULL可)。
        from: bool,
        /// 変更後。
        to: bool,
        /// NOT NULL 化 (安全でない) かどうか。
        tightens: bool,
    },
    /// DEFAULT 式が変更された。
    DefaultChanged {
        /// 変更前。
        from: Option<common_sql::ast::Expression>,
        /// 変更後。
        to: Option<common_sql::ast::Expression>,
    },
}
```
参照型: `common_sql::ast::DataType` (検証済み: `ast/datatype.rs`, 24 variants, `Eq + Hash`), `common_sql::ast::Expression` (検証済み: `ast/expression.rs`, 公開 enum)。

### 2.4 `IndexDiff`

```rust
/// 1インデックス単位の差分。
#[derive(Debug, Clone, PartialEq)]
pub enum IndexDiff {
    /// desired にのみ存在 (CREATE INDEX)。
    Added {
        /// インデックス名。
        name: String,
        /// 対象テーブル名。
        table: String,
        /// 作成されるインデックス定義。
        definition: common_sql::ast::CreateIndexStatement,
    },
    /// current にのみ存在 (DROP INDEX → 破壊的警告対象)。
    Removed {
        /// インデックス名。
        name: String,
        /// 対象テーブル名。
        table: String,
    },
    /// 両方に存在し定義が異なる (DROP + CREATE のペアで表現、RENAME は非スコープ)。
    Modified {
        /// インデックス名。
        name: String,
        /// 対象テーブル名。
        table: String,
        /// 変更後定義 (DROP+CREATE を適用した結果)。
        new_definition: common_sql::ast::CreateIndexStatement,
    },
}
```
参照型: `common_sql::ast::CreateIndexStatement` (検証済み: `ddl.rs:217`, フィールド `span: Span`, `unique: bool`, `if_not_exists: bool`, `name: Identifier`, `table: QualifiedName`, `columns: Vec<IndexColumn>`)。

### 2.5 `ConstraintDiff`

```rust
/// 1制約 (テーブルレベル) 単位の差分。
#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintDiff {
    /// desired にのみ存在 (ADD CONSTRAINT)。
    Added {
        /// 制約の正規化名 (未指定時は `<type>_<table>_<cols>` を生成)。
        name: String,
        /// 追加される制約。
        constraint: common_sql::ast::TableConstraint,
    },
    /// current にのみ存在 (DROP CONSTRAINT → 破壊的警告対象)。
    Removed {
        /// 制約名。
        name: String,
    },
    /// 両方に存在し定義が異なる (DROP + ADD のペア)。
    Modified {
        /// 制約名。
        name: String,
        /// 変更後制約。
        new_constraint: common_sql::ast::TableConstraint,
    },
}
```
参照型: `common_sql::ast::TableConstraint` (検証済み: `ddl.rs:64`, enum 4 variants: `PrimaryKey{name, columns}`, `Unique{name, columns}`, `ForeignKey{name, columns, ref_table, ref_columns}`, `Check{name, expr}`)。

### 2.6 `MigrationWarning`

```rust
/// 差分導出・マイグレーション生成過程の警告。
#[derive(Debug, Clone, PartialEq)]
pub enum MigrationWarning {
    /// 破壊的変更 (方針 A: 継続、停止しない)。
    Destructive {
        /// 人間可読な位置情報 ("table.column" 形式)。
        target: String,
        /// 変更内容の説明。
        detail: String,
    },
    /// 対象方言がネイティブ非サポート (SQLite の ALTER COLUMN 等)。
    UnsupportedDialect {
        /// 方言名 ("sqlite" / "postgresql" / "mysql")。
        dialect: String,
        /// 非対応操作の説明。
        operation: String,
    },
}
```
参照型: なし (全フィールド `String`)。

## 3. データモデル — Catalog 系 (全フィールド列挙)

> `CatalogProvider` trait が返す、ASE カタログイントロスペクション結果の dialect-neutral 表現。
> common-sql AST とは別物 (カタログ実態を表す。変換は §4 の mapper が行う)。

### 3.1 `CatalogSchema`

```rust
/// カタログ全体 (1スキーマ分)。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatalogSchema {
    /// スキーマ名 (通常 "dbo")。
    pub schema_name: String,
    /// テーブル一覧 (テーブル名でソート済み)。
    pub tables: Vec<CatalogTable>,
    /// インデックス一覧 (テーブル名・インデックス名でソート済み)。
    pub indices: Vec<CatalogIndex>,
}
```

### 3.2 `CatalogTable`

```rust
/// カタログから取得した1テーブル情報。
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogTable {
    /// テーブル名 (スキーマ修飾なし)。
    pub name: String,
    /// カラム一覧 (序数順)。
    pub columns: Vec<CatalogColumn>,
    /// テーブルレベル制約一覧。
    pub constraints: Vec<common_sql::ast::TableConstraint>,
}
```
参照型: `CatalogColumn` (§3.3), `common_sql::ast::TableConstraint` (検証済み、§2.5 参照)。catalog は common-sql の制約表現を直接再利用 (catalog 実態が既に dialect-neutral な構造を持つため)。

### 3.3 `CatalogColumn`

```rust
/// カタログから取得した1カラム情報。
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogColumn {
    /// カラム名。
    pub name: String,
    /// データ型 (common-sql 表現)。
    pub data_type: common_sql::ast::DataType,
    /// NULL 許容 (true = NULL可)。
    pub nullable: bool,
    /// DEFAULT 式 (パース済みの場合。未パースの文字列の場合は None、別途 raw_default に保持)。
    pub default: Option<common_sql::ast::Expression>,
    /// DEFAULT 式の生文字列 (式パース失敗時のフォールバック)。
    pub raw_default: Option<String>,
    /// IDENTITY / AUTO_INCREMENT 指定。
    pub identity: bool,
    /// カラムレベル制約一覧。
    pub constraints: Vec<common_sql::ast::ColumnConstraint>,
}
```
参照型: `common_sql::ast::DataType` (検証済み §2.3.1), `common_sql::ast::Expression` (検証済み), `common_sql::ast::ColumnConstraint` (検証済み: `ddl.rs:44`, enum 5 variants: `PrimaryKey`, `Unique`, `Check(Expression)`, `References{table, columns}`, `AutoIncrement`)。

### 3.4 `CatalogIndex`

```rust
/// カタログから取得した1インデックス情報。
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogIndex {
    /// インデックス名。
    pub name: String,
    /// 対象テーブル名。
    pub table: String,
    /// インデックス対象カラム (序数順、ソート方向付き)。
    pub columns: Vec<common_sql::ast::IndexColumn>,
    /// UNIQUE 指定。
    pub unique: bool,
}
```
参照型: `common_sql::ast::IndexColumn` (検証済み: `ddl.rs:205`, フィールド `name: Identifier`, `direction: Option<SortDirection>`)。

### 3.5 `CatalogProvider` trait (ASE 統合契約)

```rust
/// ASE カタログ (または同等の JSON dump) からスキーマ情報を取得する契約。
///
/// `ase` feature が有効な場合のみ ase-rs を叩く実装 (T9) が提供される。
/// feature 無しビルドでは `JsonCatalogProvider` (T11 用) のみが利用可能。
pub trait CatalogProvider {
    /// スキーマ全体を取得する。
    ///
    /// # Errors
    /// カタログアクセス失敗 (接続エラー・権限不足等) の場合 `CatalogError` を返す。
    fn load_schema(&self) -> Result<CatalogSchema, CatalogError>;
}
```

#### 3.5.1 `CatalogError`

```rust
/// カタログ取得エラー。
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CatalogError {
    /// カタログアクセス失敗 (接続・クエリエラー)。
    #[error("catalog access failed: {message}")]
    AccessFailed {
        /// エラーメッセージ。
        message: String,
    },
    /// カタログ情報のパース失敗 (JSON 不正・型不整合)。
    #[error("catalog parse failed: {message}")]
    ParseFailed {
        /// エラーメッセージ。
        message: String,
    },
    /// 未対応の ASE 固有データ型・構造 (design §0.6 短絡方針)。
    #[error("unsupported catalog shape: {detail}")]
    UnsupportedCatalogShape {
        /// 詳細。
        detail: String,
    },
    /// 機能が未実装 (CTO 2026-07-14 条件 #3: design.md が具体的カタログ問い合わせを
    /// 規定しない表面は T9 範囲外とし、暗黙ドロップではなく明示的に表面化する)。
    ///
    /// T9 では `AseCatalogProvider::load_schema` のカタログイントロスペクション
    /// (sysobjects/syscolumns/sysindexes 読み出し) がこの状態になる (T9b follow-up で実装)。
    #[error("not implemented: {what}")]
    NotImplemented {
        /// 未実装の対象。
        what: String,
    },
}
```

参照型: `thiserror::Error` (workspace 依存、`#[error("...")]` 属性で `Display` を導出)。実装は `crates/schema-diff/src/catalog.rs:76-107` と完全一致 (retrospective-P5: design.md は実装の全 variant を列挙)。

## 4. AlterOperation (diff → ALTER SQL 変換の中間表現)

> `diff_schema` の出力 (`SchemaDiff`) から、各方言の emitter が消費する
> `common_sql::ast::Statement` を生成するための中間 IR。

### 4.1 `AlterOperation`

```rust
/// `SchemaDiff` を方言非依存の DDL 操作列に変換した中間表現。
#[derive(Debug, Clone, PartialEq)]
pub enum AlterOperation {
    /// CREATE TABLE (TableDiff::Added に対応)。
    CreateTable(common_sql::ast::CreateTableStatement),
    /// DROP TABLE (TableDiff::Removed に対応)。
    DropTable {
        /// テーブル名。
        name: String,
    },
    /// ALTER TABLE (ColumnDiff/ConstraintDiff をアクション列に束ねたもの)。
    AlterTable {
        /// テーブル名。
        name: String,
        /// 適用するアクション列 (ソース順)。
        actions: Vec<common_sql::ast::AlterTableAction>,
    },
    /// CREATE INDEX (IndexDiff::Added に対応)。
    CreateIndex(common_sql::ast::CreateIndexStatement),
    /// DROP INDEX (IndexDiff::Removed に対応)。
    DropIndex {
        /// インデックス名。
        name: String,
        /// 対象テーブル名 (方言によっては省略可)。
        table: Option<String>,
    },
}
```
参照型: `common_sql::ast::CreateTableStatement` (検証済み), `common_sql::ast::AlterTableAction` (検証済み: `ddl.rs:147`, enum 6 variants: `AddColumn(ColumnDef)`, `DropColumn(Identifier)`, `AlterColumn{column, data_type, default, nullable}`, `AddConstraint(TableConstraint)`, `DropConstraint(String)`, `RenameTo(QualifiedName)`), `common_sql::ast::CreateIndexStatement` (検証済み)。

**注記**: `AlterOperation` は `common_sql::ast::Statement` と **1:1 に変換可能** であり (design で `to_statement(self) -> common_sql::ast::Statement` を規定)、最終的に各 emitter が消費する。これにより emitter 拡張 (T3/T4/T5) と diff ロジックの結合を切る。

## 5. 公開 API 署名 (crate の public surface)

> crate 名: `schema-diff` (`crates/schema-diff/`)
> 全関数・型は `pub`。`unwrap`/`expect`/`panic` はライブラリコードで禁止 (workspace lint 準拠)。

```rust
// crates/schema-diff/src/lib.rs

pub mod catalog;   // §3 の CatalogSchema/CatalogTable/CatalogColumn/CatalogIndex/CatalogProvider/CatalogError
pub mod diff;      // §2 の Diff 系 + diff_schema
pub mod emit;      // §4 の AlterOperation + to_common_sql::Statement 変換
pub mod warning;   // §2.6 MigrationWarning

#[cfg(feature = "ase")]
pub mod adapters;  // T9 ase-rs adapter (feature gate 内)

use common_sql::ast;

/// desired (DDL から構築) と current (catalog) の差分を計算する純粋関数。
///
/// IO を含まない (AC-4)。両入力は呼び出し側が構築済みの AST/catalog 表現。
#[must_use]
pub fn diff_schema(
    current: &catalog::CatalogSchema,
    desired: &catalog::CatalogSchema,
) -> diff::SchemaDiff;

/// `SchemaDiff` を方言非依存の `AlterOperation` 列に変換する。
/// 警告 (`MigrationWarning::Destructive` 等) は `SchemaDiff.warnings` から引き継がれる。
#[must_use]
pub fn plan_operations(diff: &diff::SchemaDiff) -> Vec<emit::AlterOperation>;

/// `AlterOperation` 列を `common_sql::ast::Statement` 列に変換する。
/// これを各 emitter (T3/T4/T5 拡張後) に渡して方言別 SQL 文字列を得る。
#[must_use]
pub fn to_statements(ops: &[emit::AlterOperation]) -> Vec<ast::Statement>;
```

`CatalogSchema` 構築ヘルパー (T2/T6 関連、`desired` 側):
```rust
/// CREATE TABLE 系 DDL 文列をパースして desired 側 `CatalogSchema` を構築する。
/// 内部で tsql-parser の `parse_with_errors` + T2 拡張後の `to_common_sql` を呼ぶ。
///
/// # Errors
/// DDL にパースエラーが含まれる場合 `CatalogError::ParseFailed` を返す。
pub fn build_desired_schema(ddl_source: &str) -> Result<catalog::CatalogSchema, CatalogError>;
```
(本関数は T2 完了後に実装可能。T1 時点では署名のみ規定し、実装は T6 が担う。)

## 6. Crate 構成 (ファイルレイアウト)

```
crates/schema-diff/
├── Cargo.toml
└── src/
    ├── lib.rs          // re-exports + diff_schema/plan_operations/to_statements
    ├── catalog.rs      // CatalogSchema/CatalogTable/CatalogColumn/CatalogIndex/CatalogProvider/CatalogError
    ├── diff.rs         // SchemaDiff/TableDiff/ColumnDiff/ColumnChange/IndexDiff/ConstraintDiff + diff_schema
    ├── emit.rs         // AlterOperation + plan_operations + to_statements
    ├── warning.rs      // MigrationWarning
    ├── mapper.rs       // common-sql AST <-> CatalogSchema 相互変換 (§7)
    └── adapters/
        ├── mod.rs      // #[cfg(feature="ase")] pub mod ase; (常に pub mod json;)
        ├── json.rs     // JsonCatalogProvider (feature gate 外、T11 用)
        └── ase.rs      // #[cfg(feature="ase")] AseCatalogProvider (T9, feature gate 内)
```

### 6.1 `Cargo.toml` (スケルトン、T6 で作成)

```toml
[package]
name = "schema-diff"
version = "0.1.0"
edition = "2021"

# CI note: ase-rs is public-read, but actions/checkout@v7 (default
# persist-credentials: true) injects the workspace GITHUB_TOKEN that GitHub
# rejects (401/403, no anonymous fallback) for the foreign ase-rs URL when
# cargo fetches it. The first-attempt fix — clearing extraHeader in
# .cargo/config.toml — did NOT work (cargo emitted `unused config key` and
# the fetch still failed). The effective fix is
# .github/workflows/ci.yml setting `persist-credentials: false` on every
# actions/checkout@v7, so the token is never written to the git extraheader
# in the first place (see PR #201 / T9.6). Do NOT re-enable
# persist-credentials on those steps without a verified replacement.
[features]
default = []
# ASE ライブカタログ取得 (非公開 git upstream `Sou-Tokuda/ase-rs`)。default off。
#
# CTO 2026-07-14 条件 #1 (design gate 修正): 上流ルート Cargo.toml は
# `[workspace]`-only で `[lib]` を持たず、`dep:ase-rs` は解決不能
# (実証済み: root は members=[ase-driver, ase-tds, ase-types, ase-dsn] のみ)。
# design §6.1 旧版の `ase = ["dep:ase-rs"]` / `[dependencies.ase-rs]` は
# dangling 参照 (retrospective-P5 違反) のため、実ワークスペースメンバ
# (`ase-driver` / `ase-tds` / `ase-types` / `ase-dsn`) に読み替える。
# `ase-driver` は AseDataType を re-export しないため、型名には `ase-types` を直接参照する。
#
# T9.1 (型マッピング + provider 構造体) は `ase-types` + `ase-driver` のみ必要。
# T9.3 (live 接続パス) は加えて `ase-dsn` (DSN 文字列 → ConnectionConfig)、
# `ase-tds` (TdsConnection::connect(config))、`tokio` (current_thread runtime で
# async `ase_driver::Connection::query` を sync `load_schema` から駆動) を必要とする。
# 全 5 optional dep が同一 git upstream から導入される (実装と一致:
# `crates/schema-diff/Cargo.toml` 行 50)。
ase = ["dep:ase-types", "dep:ase-driver", "dep:ase-dsn", "dep:ase-tds", "dep:tokio"]

[dependencies]
common-sql = { path = "../common-sql" }
thiserror = { workspace = true }

# T9: 以下 5 dep は全て同一 git upstream (`Sou-Tokuda/ase-rs`) のオプショナル依存。
# default = [] の背後にあるため、デフォルト publishable ビルド (T11 CLI) は
# 上流を解決しない (design §0.1 / AC-5/AC-6)。各 dep の役割は実装
# (`crates/schema-diff/Cargo.toml` 行 50-73) と完全一致させること。
#
# T9.1: `ase-types` は `AseDataType` enum を所有 (map_ase_type のシグネチャで命名)。
#        `ase-driver` は high-level live `Connection` を所有
#        (AseCatalogProvider が保持するハンドル)。
# T9.3: `ase-dsn` は DSN 文字列を `ConnectionConfig` にパースする。
#        `ase-tds` は `TdsConnection::connect(config)` を所有し、DSN → Connection 橋を完成させる。
#        `tokio` は `current_thread` + `enable_all` runtime で async query を
#        sync `CatalogProvider::load_schema` (design §3.5) から `block_on` 駆動する
#        (スキーマイントロスペクションは CPU 並列不要なため current_thread で十分)。
#        `default-features = false` + `features = ["rt"]` で dep を最小化。
[dependencies.ase-types]
git = "https://github.com/Sou-Tokuda/ase-rs"
optional = true

[dependencies.ase-driver]
git = "https://github.com/Sou-Tokuda/ase-rs"
optional = true

[dependencies.ase-dsn]
git = "https://github.com/Sou-Tokuda/ase-rs"
optional = true

[dependencies.ase-tds]
git = "https://github.com/Sou-Tokuda/ase-rs"
optional = true

[dependencies.tokio]
version = "1"
optional = true
default-features = false
features = ["rt"]

[dev-dependencies]
rstest = { workspace = true }

[lints]
workspace = true
```

## 7. common-sql AST と CatalogSchema のマッピング (mapper.rs)

`CatalogSchema` (§3) ↔ `common_sql::ast::CreateTableStatement`/`CreateIndexStatement` (検証済み既存型) の
相互変換関数 (T6 実装、T1 時点では契約のみ規定):

```rust
/// `CreateTableStatement` (common-sql) を `CatalogTable` に変換。
#[must_use]
pub fn create_table_to_catalog(stmt: &ast::CreateTableStatement) -> CatalogTable;

/// `CatalogTable` を `CreateTableStatement` に変換 (ALTER 操作生成時に使用)。
#[must_use]
pub fn catalog_to_create_table(t: &CatalogTable) -> ast::CreateTableStatement;

/// `CatalogIndex` を `CreateIndexStatement` に変換。
#[must_use]
pub fn catalog_to_create_index(idx: &CatalogIndex) -> ast::CreateIndexStatement;
```

**変換契約 (フィールド対応、全て検証済み AST を根拠)**:
- `CatalogColumn.data_type: DataType` ↔ `ColumnDef.data_type: DataType` (同一型、直接コピー)
- `CatalogColumn.nullable: bool` ↔ `ColumnDef.nullable: bool` (同一)
- `CatalogColumn.constraints: Vec<ColumnConstraint>` ↔ `ColumnDef.constraints: Vec<ColumnConstraint>` (同一型)
- `CatalogColumn.identity: bool` → `ColumnConstraint::AutoIncrement` の有無に変換 (カラム制約として表現)
- `Identifier` の private フィールド (`value`/`quoted`) は accessor (`Identifier::new(value)` / `value()`) 経由のみで扱う (§0.3 stability contract)

## 8. 検証済み参照型の出典 (dangling 参照チェックリスト)

本設計が参照する **全ての外部型** と、その出典 (検証済みソース):

| 参照型 | 出典 (検証済み) | フィールド/variants |
|--------|----------------|---------------------|
| `common_sql::ast::Statement` | `crates/common-sql/src/ast/statement.rs:18` | 10 variants (Select/Insert/Update/Delete/**CreateTable**/**AlterTable**/**DropTable**/**CreateIndex**/**DropIndex**/DialectSpecific) |
| `common_sql::ast::CreateTableStatement` | `crates/common-sql/src/ast/ddl.rs:124` | span, if_not_exists, temporary, name:QualifiedName, columns, constraints, options |
| `common_sql::ast::AlterTableStatement` | `crates/common-sql/src/ast/ddl.rs:175` | span, name:QualifiedName, actions:Vec\<AlterTableAction\> |
| `common_sql::ast::AlterTableAction` | `crates/common-sql/src/ast/ddl.rs:147` | 6 variants (AddColumn/DropColumn/AlterColumn/AddConstraint/DropConstraint/RenameTo) |
| `common_sql::ast::DropTableStatement` | `crates/common-sql/src/ast/ddl.rs:190` | span, if_exists, names:Vec\<QualifiedName\> |
| `common_sql::ast::CreateIndexStatement` | `crates/common-sql/src/ast/ddl.rs:217` | span, unique, if_not_exists, name:Identifier, table:QualifiedName, columns:Vec\<IndexColumn\> |
| `common_sql::ast::DropIndexStatement` | `crates/common-sql/src/ast/ddl.rs:238` | span, if_exists, name:Identifier, table:Option\<QualifiedName\> |
| `common_sql::ast::ColumnDef` | `crates/common-sql/src/ast/ddl.rs:24` | span, name:Identifier, data_type:DataType, nullable:bool, default, constraints |
| `common_sql::ast::ColumnConstraint` | `crates/common-sql/src/ast/ddl.rs:44` | 5 variants (PrimaryKey/Unique/Check/References/AutoIncrement) |
| `common_sql::ast::TableConstraint` | `crates/common-sql/src/ast/ddl.rs:64` | 4 variants (PrimaryKey/Unique/ForeignKey/Check) |
| `common_sql::ast::IndexColumn` | `crates/common-sql/src/ast/ddl.rs:205` | name:Identifier, direction:Option\<SortDirection\> |
| `common_sql::ast::DataType` | `crates/common-sql/src/ast/datatype.rs:5` | 24 variants, derives Eq+Hash |
| `common_sql::ast::Expression` | `crates/common-sql/src/ast/expression.rs` | 公開 enum |
| `common_sql::ast::Identifier` | `crates/common-sql/src/ast/identifier.rs:5` | private {value:String, quoted:bool} + accessors |
| `common_sql::ast::QualifiedName` | `crates/common-sql/src/ast/identifier.rs:43` | private {schema:Option\<String\>, name:String} + accessors |
| `common_sql::ast::Span` | `crates/common-sql/src/ast/span.rs:11` | pub {start:u32, end:u32} |
| `common_sql::ast::SortDirection` | `crates/common-sql/src/ast/clause.rs` (re-export at mod.rs:14) | Asc/Desc |

**dangling 参照なし**: 上記以外の外部型は本設計書から参照しない。本設計書内で新規定義する型 (§2, §3, §4) は全て自己完結している。

## 9. テスト容易性設計 (TDD 前提)

- `diff_schema` は純粋関数 (AC-4) → ユニットテストは入出力の構造体等価比較のみ。
- `ase` feature 無しで `diff_schema`/`plan_operations`/`to_statements` の全テストが実行可能 (T11 catalog JSON 入力で current 側を構築)。
- テストモジュールには `#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]` を付与 (workspace lint 準拠)。

## 10. 非スコープ (設計レベル)

- ALTER OPERATION の依存順序ソート (FK 制約のためにテーブル CREATE 順序を制御等) は初期版では未実装 (入力順序を維持)。将来課題。
- カラム RENAME 検出 (ADD+DROP ペアからの推論) は非スコープ (§requirements.md 非スコープ準拠)。
- マイグレーション SQL の DB 適用実行機能は非スコープ。
