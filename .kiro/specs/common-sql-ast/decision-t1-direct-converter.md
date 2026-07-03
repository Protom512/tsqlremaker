# T1 決定ゲート: 直接変換器アーキテクチャ (Issue #163)

> **ステータス**: APPROVED (T2 コード実装のブロッカー条件を満たす)
> **決定日**: 2026-07-03
> **決定者**: Implementation Engineer (T1)
> **承認根拠**: CTO Estimate Approval (条件: T1 ゲート解決 + 損失マッピング表の文書化)

## 1. 決定事項

**採用案: (A) 単一直接変換器 `tsql_parser::ast::Statement -> common_sql::ast::Statement`**

- 変換器の**配置場所**: `tsql-parser` クレート内 (`crates/tsql-parser/src/ast/to_common_sql.rs`)。`tsql-parser` は既に `common-sql` に依存している (`Cargo.toml` 検証済み) ため、クリーン DAG (architecture-coupling-balance.md §1) を**反転させない**。
- **再エクスポート**: 変換トレイト・関数は `tsql_parser` の public API として再エクスポートする (wasm および emitter が `tsql_parser` 経由で利用できるようにするため)。
- レガシー `CommonStatement` 形状 (5バリアント + `DialectSpecific` エスケープハッチ) は**維持しない**。これはチケット #163 の削除目標を満たすため、かつ (B) 案が「チケットの削除目標を満たさない」と CTO gate が指摘したため。

**却下案: (B)** — レガシー `CommonStatement` を内部に保持しつつ `tsql_parser::common` 名前空間から移動するだけでは、公開ブリッジの削除というチケットの核心目標を達成できない。また 2 段階パイプライン (wasm `to_common_ast` → `convert`) の技術的負債を温存することになるため却下。

## 2. 構造的非等価性 (Risk #1 — 変換器書き直しの根拠)

レガシー `CommonStatement` と宛先 `common_sql::ast::Statement` は**構造的に非等価**である。これは「再エクスポート置換」ではなく「変換器の書き直し」である理由の全容。

| 側 | バリアント数 | `DialectSpecific` | 場所 |
|----|------------|-------------------|------|
| レガシー `CommonStatement` | 5 (Select/Insert/Update/Delete/DialectSpecific) | **あり** `{description, span}` | `common-sql/src/statement.rs:13` |
| 宛先 `common_sql::ast::Statement` | 9 (Select/Insert/Update/Delete/CreateTable/AlterTable/DropTable/CreateIndex/DropIndex) | **なし** | `common-sql/src/ast/statement.rs:18` |

注記: `common-sql` クレート内に**2つの** `Statement` 系 enum が存在する (設計上の過渡期の残存)。本チケットは宛先側 `ast::Statement` (9バリアント・DDL対応) を正とし、`statement.rs` のレガシー `CommonStatement` は最終的に削除対象である。ただし #163 のスコープは `tsql-parser` 側ブリッジの削除に限定し、`common-sql` 内のレガシー enum の削除は後続チケットに委ねる (CTO gate の「単一フォーカス PR」条件に従う)。

## 3. 損失マッピング表 (Lossy Mapping Table)

以下のマッピングは T2 実装において**厳密に**適用すること。CTO gate 条件により、mysql-emitter (PR #152) / postgresql-emitter (PR #159) で実証済みの mappers パターンと**同一規約**を使用し、第3のマッピング規約を発明しないこと。

### 3.1 ステートメントレベル

| レガシー (tsql_parser) | 宛先 (common_sql::ast) | 損失有無 | 根拠 |
|------------------------|------------------------|----------|------|
| `Statement::Select` | `Statement::Select` | なし | 1:1 |
| `Statement::Insert` | `Statement::Insert` | なし | 1:1 |
| `Statement::Update` | `Statement::Update` | なし | 1:1 |
| `Statement::Delete` | `Statement::Delete` | なし | 1:1 |
| `Statement::Create(Table)` | `Statement::CreateTable` | なし | DDL対応 (新規) |
| `Statement::AlterTable` | `Statement::AlterTable` | なし | DDL対応 (新規) |
| `Statement::Create(Index)` | `Statement::CreateIndex` | なし | DDL対応 (新規) |
| `Statement::Declare` `Set` `If` `While` `Block` `Break` `Continue` `Return` `TryCatch` `Transaction` `Throw` `Raiserror` | **`None`** (旧 `DialectSpecific`) | **損失あり** | 制御フローは方言固有。宛先 AST に `DialectSpecific` が存在しないため、`Option<Statement>` の `None` へ折叠。 |
| `Statement::Create(View/Procedure)` | **`None`** (旧 `DialectSpecific`) | **損失あり** | 宛先 AST に View/Procedure バリアントが存在しないため。 |
| `Statement::Exec` | **`None`** (旧 `DialectSpecific`) | **損失あり** | EXEC/EXECUTE は方言固有。 |
| `Statement::VariableAssignment` | **`None`** (旧 `DialectSpecific`) | **損失あり** | 変数代入は方言固有。 |
| `Statement::BatchSeparator` | **`None`** | なし | 旧来通り (バッチ区切りは Common AST に含めない)。 |

> **API 署名の決定**: 旧 `to_common_ast(&self) -> Option<CommonStatement>` は **`to_common_sql(&self) -> Option<common_sql::ast::Statement>`** へ変更する。`None` は上記「方言固有 → ドロップ」を一意に表現する。旧 `DialectSpecific{description,span}` のメタ情報は**破棄**される (sqlite-emitter は元来これを `EmitError::Unsupported` に変換していたが、ドロップ後は emitter 側で到達しなくなる)。

### 3.2 SELECT 文レベルの損失マッピング

| レガシーフィールド | 宛先フィールド | 損失内容 |
|--------------------|----------------|----------|
| `from: Vec<CommonTableReference>` | `from: Option<TableFactor>` | **`Vec` の先頭要素のみ採用**。2番目以降のテーブルは破棄。JOIN は別途 `TableFactor::Join` で表現されるべきだが、レガシー bridge は JOIN を `to_common()` で無視していた (`to_common.rs:236-238`) ので、本移行でも JOIN 情報は旧来通り欠落する (改善は後続チケット)。 |
| `columns: Vec<CommonSelectItem>` | `projection: Vec<SelectItem>` | なし (要素毎に下記 3.3 適用) |
| `distinct: bool` | (削除) | **損失あり**: 宛先 `SelectStatement` に `distinct` フィールドがない。postgresql-emitter は comment (`select_statement.rs:529`) で「旧 distinct は削除された」と明記済み。`DISTINCT` は投影要素として `SelectItem` 側で表現されるべきだが、現状はドロップされる。 |
| `group_by: Vec<CommonExpression>` | `group_by: Option<GroupByClause>` | 空 `Vec` → `None`。 |
| `order_by: Vec<CommonOrderByItem>` | `order_by: Option<OrderByClause>` | 空 `Vec` → `None`。 |
| `limit: Option<CommonLimitClause>` | `limit: Option<LimitClause>` | なし。 |
| `span: Span` | `span: Span` | なし (型は異なるが意味は等価)。 |

### 3.3 SELECT リストアイテム

| レガシー | 宛先 | 損失内容 |
|----------|------|----------|
| `CommonSelectItem::Expression(expr, Option<String>)` | `SelectItem::Expression { expr, alias: Option<Identifier> }` | `String` → `Identifier`。**有損失なし** (文字列内容は保持)。 |
| `CommonSelectItem::Wildcard` | `SelectItem::Wildcard` | なし。 |
| `CommonSelectItem::QualifiedWildcard(String)` | `SelectItem::QualifiedWildcard { table: Identifier }` | **`String` → `Identifier`** (unquoted)。CTO gate 条件の「`QualifiedWildcard String -> Identifier`」。 |

### 3.4 式レベルの損失マッピング

| レガシー `CommonExpression` | 宛先 `common_sql::ast::Expression` | 損失内容 |
|------------------------------|--------------------------------------|----------|
| `Like { expr, pattern, escape: Option<Box<...>>, negated, span }` | `Comparison { left, op: ComparisonOperator::Like/NotLike, right }` | **LIKE ESCAPE 句ドロップ** (CTO gate 条件)。`negated` → `NotLike`。`escape` は破棄。 |
| `Case(CommonCaseExpression { branches, else_result })` | `Case { operand, conditions, else_result }` | **CASE operand → `None`** (CTO gate 条件)。レガシーは simple CASE の operand を保持しない (常に searched CASE として扱う) ので `operand: None`。 |
| `ColumnReference(CommonColumnReference { table, column })` | `QualifiedIdentifier { table, column }` | なし (両 `Identifier` 化)。 |
| `FunctionCall(CommonFunctionCall { name, args, distinct })` | `Function { name, args, distinct }` | なし。 |
| `IsNull { expr, negated }` | `IsNull { expr, negated }` | なし。 |
| `Between { expr, low, high, negated }` | `Between { expr, low, high, negated }` | なし。 |
| `In { expr, list, negated }` | `In { expr, list, negated }` | なし。`InList::Values`/`Subquery` は構造等価。 |
| `Subquery { query, span }` | `Subquery(Box<SelectStatement>)` | `span` 破棄 (宛先が span を持たない)。 |
| `Exists { query, negated, span }` | `Exists { subquery, negated }` | `span` 破棄。 |
| `BinaryOp { left, op, right, span }` | `BinaryOp`/`Comparison`/`LogicalOp` (演算子種別で振り分け) | `span` 破棄。レガシー単一 14バリアント enum → 3 enum 分割は既存 bridge と同一ロジック。 |
| `UnaryOp { op, expr, span }` | `UnaryOp { op, expr }` | `span` 破棄。`UnaryOperator::Tilde` (ビット否定) は `None` (レガシー bridge 準拠)。 |
| `Literal(CommonLiteral)` | `Literal(Literal)` | `CommonLiteral::Float(f64)` → `Literal::Float(String)` (Display 経由)。 |
| `Identifier(CommonIdentifier { name })` | `Identifier(Identifier::new(name))` | なし。 |

### 3.5 INSERT ソース

| レガシー `CommonInsertSource` | 宛先 `common_sql::ast::InsertSource` | 損失内容 |
|--------------------------------|---------------------------------------|----------|
| `Values(Vec<Vec<CommonExpression>>)` | `Values(Vec<Vec<Expression>>)` | なし。 |
| `Select(Box<CommonSelectStatement>)` | `Select(Box<SelectStatement>)` | なし。 |
| `DefaultValues` | **`Values(vec![])`** | **損失あり** (CTO gate 条件): 宛先 `InsertSource` に `DefaultValues` バリアントがないため、空 VALUES へ折叠。postgresql-emitter は comment (`lib.rs:230`, `lib.rs:570`) でこの折叠を明記済み。 |

### 3.6 データ型

| レガシー `CommonDataType::Float` | 宛先 `common_sql::ast::DataType::DoublePrecision` | **損失あり**: 精度破棄 (既存 bridge `convert_common_sql.rs:52` 準拠。範囲安全)。 |
|-----------------------------------|---------------------------------------------------|---|
| その他のデータ型 | 1:1 | なし。 |

### 3.7 UPDATE / DELETE

| レガシー | 宛先 | 損失内容 |
|----------|------|----------|
| `UPDATE ... FROM clause` | (旧 `DialectSpecific` → ドロップ) | **損失あり**: ASE 固有の `UPDATE ... FROM` は `None` へ。 |
| `DELETE ... FROM clause` | (旧 `DialectSpecific` → ドロップ) | **損失あり**: 同上。 |
| `UPDATE table` (plain) | `UpdateStatement { table: TableFactor::Table, ... }` | なし。 |
| `DELETE table` (plain) | `DeleteStatement { table: TableFactor::Table, ... }` | なし。 |

## 4. sqlite-emitter (T3) 書き直し要件

CTO gate 条件により、sqlite-emitter は mysql-emitter (PR #152) / postgresql-emitter (PR #159) の**実証済みパターンを厳密に踏襲**すること。第3のマッピング規約を発明しないこと。

- 入力シグネチャ: `emit(&common_sql::ast::Statement)` (旧 `emit(&CommonStatement)`)
- `SelectItem::QualifiedWildcard { table: Identifier }` 形式を使用 (postgresql-emitter `select_statement.rs:106` 準拠)。
- `TableFactor` の 3バリアント (Table/Derived/Join) を全て処理 (postgresql-emitter `select_statement.rs:125` 準拠)。
- `ComparisonOperator::{Like, NotLike}` を使用 (旧 `CommonExpression::Like` ではない)。
- `DialectSpecific` バリアントは宛先 AST に存在しないため、sqlite-emitter の `EmitError::Unsupported(description)` パスは到達不能となり削除される。

## 5. wasm パイプライン (T4)

wasm は現在 2段階パイプライン (`to_common_ast` → `convert`) を使用 (`wasm/src/lib.rs:183`, `:202`, `:244`)。本移行により単一呼び出し `to_common_sql()` へ統合される。CTO gate が指摘した通り、これは Stage-1 (`to_common.rs` 600行) の吸収を意味し、スコープクリープだが #163 完結に必須。

## 6. テストカバレッジ保全 (T6)

CTO gate 条件: `common_ast_conversion.rs` (940行) と `convert_common_sql_bridge.rs` (474行) のテストカバレッジを保全すること。新直接変換器の `common_sql::ast::Statement` 出力に対して再アサートするか、削除する場合は文書化されたカバレッジギャップを明示すること。

## 7. 検証ゲート (T7)

1. `cargo fmt --all --check`
2. `cargo check --all-targets` (**`--all` ではなく**、テストコンパイルエラーを捕捉)
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo nextest run --workspace`
5. **残留ゼロチェック**: `rg 'tsql_parser::common|crate::common|ToCommonAst|to_common_ast'` が空であること
6. **wasm-pack ビルド確認** (wasm ターゲット)
7. **単一フォーカス PR**: #134/#162 とはバンドルしない (これらは本チケットが生成するクリーンな単一ソース AST ハブに依存する)

## 8. 参照

- CTO Estimate Approval (条件 1-6)
- architecture-coupling-balance.md §1 (DAG 依存方向)
- 既存 Stage-2 bridge: `crates/tsql-parser/src/common/convert_common_sql.rs` (損失マッピングの先例)
- 実証済み emitter パターン: `crates/postgresql-emitter/src/mappers/select_statement.rs`, `crates/mysql-emitter/src/converters/function.rs`
