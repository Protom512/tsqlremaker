# Common SQL AST - Requirements

## 概要

SAP ASE T-SQL、MySQL、PostgreSQL などの SQL 方言間の変換を可能にする、方言非依存の共通抽象構文木（AST）を定義する。

## 目的

この AST は以下の目的で使用される：

1. **T-SQL Parser から出力される統一された中間表現**
   - Parser は方言固有の構文を共通 AST に変換する
   - 方言依存の要素は可能な限り共通形式で表現する

2. **MySQL Emitter、PostgreSQL Emitter が入力として受け取るデータ構造**
   - Emitter は共通 AST から方言固有の SQL を生成する
   - AST の構造を変更せずに、新しい Emitter を追加できる

3. **将来の他の方言（Oracle、SQL Server 等）にも拡張可能**
   - 新しい方言の追加が既存 AST に影響しない
   - 必要に応じて AST を拡張可能

## 機能要件

### FR-1: Statement ノード

以下の SQL 文を表現できるノードを提供する：

- `SELECT` - クエリ（問い合わせ）
- `INSERT` - データ挿入
- `UPDATE` - データ更新
- `DELETE` - データ削除
- `CREATE TABLE` - テーブル作成
- `ALTER TABLE` - テーブル変更
- `DROP TABLE` - テーブル削除
- `CREATE INDEX` - インデックス作成
- `DROP INDEX` - インデックス削除

### FR-2: Expression ノード

以下の式を表現できるノードを提供する：

- **リテラル**: 数値、文字列、真偽値、NULL
- **識別子**: 列名、テーブル名、エイリアス
- **算術演算**: `+`, `-`, `*`, `/`, `%`
- **論理演算**: `AND`, `OR`, `NOT`
- **比較演算**: `=`, `!=`, `<`, `>`, `<=`, `>=`, `LIKE`, `IN`, `BETWEEN`
- **関数呼び出し**: 組み込み関数、ユーザー定義関数
- **CASE 式**: 単純 CASE、検索 CASE
- **サブクエリ**: スカラサブクエリ、存在チェックサブクエリ
- **集計関数**: `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`

### FR-3: DataType ノード

以下のデータ型を表現できるノードを提供する：

- **整数型**: `TINYINT`, `SMALLINT`, `INT`, `BIGINT`
- **小数型**: `DECIMAL`, `NUMERIC`, `FLOAT`, `REAL`
- **文字列型**: `CHAR`, `VARCHAR`, `TEXT`, `NCHAR`, `NVARCHAR`
- **日時型**: `DATE`, `TIME`, `DATETIME`, `TIMESTAMP`
- **バイナリ型**: `BINARY`, `VARBINARY`, `BLOB`
- **ブール型**: `BOOLEAN`
- **その他**: `UUID`, `JSON`

### FR-4: JOIN 表現

以下の JOIN を表現できる：

- `INNER JOIN`
- `LEFT OUTER JOIN`
- `RIGHT OUTER JOIN`
- `FULL OUTER JOIN`
- `CROSS JOIN`
- 結合条件（ON 句、USING 句）

### FR-5: クエリ句

以下の句を表現できる：

- `FROM` - データソース
- `WHERE` - フィルタ条件
- `GROUP BY` - グルーピング
- `HAVING` - グループ後のフィルタ
- `ORDER BY` - ソート
- `LIMIT` / `OFFSET` - 結果の制限

### FR-6: Visitor パターン

Emitter が SQL を生成するための Visitor パターンを提供する：

- `Visitor` trait を定義
- 各 AST ノードが `accept` メソッドを実装
- Emitter は `Visitor` trait を実装して SQL を生成

## 非機能要件

### NFR-1: 方言非依存性

- 特定の方言に依存しない構造を持つ
- 方言固有の機能は拡張ポイントで対応可能

### NFR-2: 拡張性

- 新しい Statement/Expression ノードを追加可能
- 新しい DataType を追加可能
- 既存コードへの影響を最小限に抑える

### NFR-3: 不変性

- AST ノードは不変（immutable）であることが望ましい
- 一度構築された AST は変更されない

### NFR-4: エラー報告

- 各ノードは位置情報（`Span`）を持つ
- エラー発生時に適切な位置情報を提供可能

### NFR-5: シリアライズ可能性

- AST はデバッグのためにシリアライズ可能であることが望ましい
- `Debug`, `Clone`, `PartialEq` トレイトを実装

## 制約事項

### C-1: 依存関係

- Common SQL AST は他のクレートに依存しない
- 他の全てのクレートが Common SQL AST に依存する可能性がある

### C-2: 互換性

- 破壊的変更はバージョン更新で明示する
- 可能な限り後方互換性を維持する

### C-3: パニック禁止

- AST 操作でパニックを起こさない
- エラーは `Result` 型で返す

## 優先順位

1. **高**: FR-1 (Statement), FR-2 (Expression), FR-6 (Visitor)
2. **中**: FR-3 (DataType), FR-4 (JOIN), FR-5 (クエリ句)
3. **低**: NFR-5 (シリアライズ), 高度な式（ウィンドウ関数等）

## 成功基準

1. 全ての主要な SQL 文が AST で表現できる
2. T-SQL Parser が AST を生成できる
3. MySQL Emitter が AST から MySQL SQL を生成できる
4. Visitor パターンを使用して新しい Emitter を追加できる
5. 単体テストカバレッジが 80% 以上

## 除外事項

- ストアドプロシージャの制御フロー（IF、WHILE、TRY-CATCH）は将来の対応
- トリガー、ビュー、ユーザー定義型は将来の対応
- DCL（GRANT、REVOKE）は将来の対応
- トランザクション制御（BEGIN、COMMIT、ROLLBACK）は将来の対応
