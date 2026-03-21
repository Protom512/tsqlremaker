# PostgreSQL Emitter - Requirements

## 概要

Common SQL AST を入力として、PostgreSQL 方言の SQL 文字列を出力する Emitter を実装する。

## 目的

この Emitter は以下の目的で使用される：

1. **Common SQL AST から PostgreSQL SQL を生成**
   - Parser が生成した共通 AST を受け取り、PostgreSQL 方言の SQL を出力する
   - AST の構造を変更せずに、他の Emitter と並存可能

2. **T-SQL から PostgreSQL への変換**
   - SAP ASE T-SQL の構文を PostgreSQL 互換の構文に変換する
   - 方言固有の機能を PostgreSQL の等価な機能にマッピングする

3. **他の方言 Emitter との整合性**
   - MySQL Emitter と同じ Visitor pattern を使用
   - 共通 AST への依存のみで、他方言に依存しない

## 機能要件

### FR-1: Visitor Pattern の実装

**WHEN** Common SQL AST のノードを受け取った場合、**PostgreSQL Emitter** shall **Visitor trait を実装して SQL を生成する**

- `Visitor<Output = String>` を実装する
- 全ての Statement ノード（SELECT, INSERT, UPDATE, DELETE, CREATE TABLE 等）に対応する
- 全ての Expression ノード（リテラル、識別子、演算子、関数等）に対応する
- 全ての DataType ノードに対応する

### FR-2: データ型変換

**WHEN** Common SQL AST の DataType ノードを受け取った場合、**PostgreSQL Emitter** shall **PostgreSQL 互換のデータ型文字列を生成する**

| Common SQL DataType | PostgreSQL 出力 |
|---------------------|-----------------|
| BigInt | BIGINT |
| VarChar { length: n } | VARCHAR(n) |
| DateTime { precision } | TIMESTAMP [(precision)] |
| Text | TEXT |
| NText | TEXT |
| Blob | BYTEA |
| Boolean | BOOLEAN |
| Uuid | UUID |
| Json | JSONB |
| Date | DATE |
| Time { precision } | TIME [(precision)] |

### FR-3: 関数変換

**WHEN** T-SQL 固有の関数を含む Expression を受け取った場合、**PostgreSQL Emitter** shall **PostgreSQL の等価な関数に変換する**

| T-SQL 関数 | PostgreSQL 関数 |
|-----------|----------------|
| GETDATE() | CURRENT_TIMESTAMP |
| DATEADD(day, n, date) | date + INTERVAL 'n days' |
| DATEADD(month, n, date) | date + INTERVAL 'n months' |
| DATEADD(year, n, date) | date + INTERVAL 'n years' |
| DATEDIFF(day, start, end) | DATE_PART('day', end - start) |
| LEN(s) | LENGTH(s) |
| SUBSTRING(s, start, len) | SUBSTRING(s FROM start FOR len) |
| ISNULL(expr, alt) | COALESCE(expr, alt) |
| GETUTCDATE() | (NOW() AT TIME ZONE 'UTC') |

### FR-4: 一時テーブル変換

**WHEN** T-SQL の一時テーブル定義を受け取った場合、**PostgreSQL Emitter** shall **PostgreSQL の一時テーブル構文に変換する**

| T-SQL 構文 | PostgreSQL 出力 |
|-----------|-----------------|
| `#temp_table` | `CREATE TEMP TABLE temp_table` |
| `##global_temp` | `CREATE TABLE global_temp`（警告付き） |

- ローカル一時テーブル（`#`）は PostgreSQL の TEMP TABLE に変換
- グローバル一時テーブル（`##`）は通常テーブルに変換し、コメントで警告を追加

### FR-5: 構文変換（TOP → LIMIT）

**WHEN** TOP 句を含む SELECT ステートメントを受け取った場合、**PostgreSQL Emitter** shall **LIMIT 句に変換する**

| T-SQL 構文 | PostgreSQL 出力 |
|-----------|-----------------|
| `SELECT TOP 10 * FROM t` | `SELECT * FROM t LIMIT 10` |
| `SELECT TOP 10 PERCENT * FROM t` | `SELECT * FROM t LIMIT (SELECT COUNT(*) * 0.1 FROM t)` |

- `TOP n` は `LIMIT n` に変換
- `TOP n PERCENT` はサブクエリを使用して変換

### FR-6: 変数構文の扱い

**WHEN** T-SQL の変数宣言・代入を受け取った場合、**PostgreSQL Emitter** shall **警告コメントを付与して可能な限り変換する**

| T-SQL 構文 | PostgreSQL 出力 |
|-----------|-----------------|
| `DECLARE @var INT` | `-- TODO: T-SQL 変数宣言 (DECLARE @var)`<br>`-- PostgreSQL は DO ブロック内で変数宣言が必要です` |
| `SET @var = value` | `-- TODO: T-SQL 変数代入 (SET @var = value または SELECT @var = expr)`<br>`-- PostgreSQL では変数代入は SELECT INTO または := で代替してください` |
| `IF @var > 0 ...` | `-- TODO: T-SQL IF...ELSE 文`<br>`-- PostgreSQL では IF...THEN...ELSE...END IF を使用してください` |
| `WHILE @var > 0 ...` | `-- TODO: T-SQL WHILE ループ`<br>`-- PostgreSQL では WHILE...LOOP...END LOOP を使用してください` |

- T-SQL の変数構文は直接変換できない場合、コメントで警告
- 可能な場合は PostgreSQL の等価な構造（WITH 句、SELECT INTO 等）を提案
- `warn_unsupported` オプションで警告を無効化可能（デフォルト: true）

### FR-7: JOIN 構文

**WHEN** JOIN を含むクエリを受け取った場合、**PostgreSQL Emitter** shall **標準 SQL の JOIN 構文を生成する**

- INNER JOIN, LEFT OUTER JOIN, RIGHT OUTER JOIN, FULL OUTER JOIN, CROSS JOIN に対応
- ON 句、USING 句、NATURAL JOIN に対応
- 複数の JOIN を正しく連結する

### FR-8: サブクエリと CTE

**WHEN** サブクエリまたは CTE（WITH 句）を含むステートメントを受け取った場合、**PostgreSQL Emitter** shall **PostgreSQL のサブクエリ構文を生成する**

- スカラサブクエリ、EXISTS、IN に対応
- WITH 句（CTE）を正しく生成する
- 再帰 CTE に対応する

## 非機能要件

### NFR-1: 単方向依存

**The** PostgreSQL Emitter **shall** **Common SQL AST にのみ依存し、他方言 Emitter に依存しない**

- `common-sql` クレートにのみ依存する
- `mysql-emitter` 等の他方言コードを参照しない

### NFR-2: エラーハンドリング

**IF** AST ノードの生成中にエラーが発生した場合、**PostgreSQL Emitter** shall **Result 型でエラーを返す**

- パニックを起こさない
- エラー内容と位置情報を含むエラー型を返す

### NFR-3: 出力の可読性

**The** PostgreSQL Emitter **shall** **人間が読みやすく整形された SQL を出力する**

- 適切なインデントを行う
- 不要な括弧を省略する
- 予約語は大文字で出力する

### NFR-4: 拡張性

**WHEN** 新しい AST ノードが Common SQL AST に追加された場合、**PostgreSQL Emitter** shall **コンパイルエラーで検知可能である**

- Visitor trait の実装漏れがコンパイルエラーで分かるようにする
- デフォルト実装ではなく明示的な実装を強制する

## 制約事項

### C-1: 依存関係

- PostgreSQL Emitter は Common SQL AST のみに依存する
- 他方言 Emitter と並存可能

### C-2: 変換不可能な構文

以下の T-SQL 構文は PostgreSQL に直接変換できない：

- ストアドプロシージャの制御フロー（IF、WHILE、TRY-CATCH）
- カーソル操作
- 一時テーブルのスコープ規則
- トランザクション分離レベルの一部

これらは警告コメントを出力する。

### C-3: PostgreSQL バージョン

- PostgreSQL 12+ をターゲットとする
- 最新バージョンの機能を積極的に使用する

## 優先順位

1. **高**: FR-1 (Visitor), FR-2 (データ型), FR-3 (関数), FR-5 (TOP→LIMIT)
2. **中**: FR-4 (一時テーブル), FR-7 (JOIN), FR-8 (サブクエリ)
3. **低**: FR-6 (変数構文), 高度な T-SQL 固有機能

## 成功基準

1. Common SQL AST の全ての主要ノードが PostgreSQL SQL に変換できる
2. 主要な T-SQL 関数が PostgreSQL 関数に正しく変換される
3. 出力される SQL が PostgreSQL で実行可能である
4. 単体テストカバレッジが 80% 以上
5. 変換不可能な構文に対して適切な警告が出力される

## 除外事項

- ストアドプロシージャの完全な変換（PL/pgSQL への変換は将来対応）
- T-SQL のシステムストアドプロシージャ呼び出し
- データベース設定オプションの変換（SET コマンド等）
- バッチ内の複数ステートメントの最適化
