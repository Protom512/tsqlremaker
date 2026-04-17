# SAP ASE Language Server - Feature Roadmap

DB系Language Serverのデファクトスタンダードに基づく機能ロードマップ。

## Tier 1: 実装済み (MVP)

| 機能 | LSP メソッド | 状態 | DDLでの価値 |
|------|-------------|------|------------|
| Diagnostics | `textDocument/publishDiagnostics` | ✅ 完了 | DDL構文エラーの検出。括弧不一致、無効な型、欠落キーワードの捕捉 |
| Semantic Tokens | `textDocument/semanticTokens/full` | ✅ 完了 | DDL認識のシンタックスハイライト。キーワード/データ型/識別子/制約キーワードの区別 |
| Document Symbols | `textDocument/documentSymbol` | ✅ 完了 | CREATE TABLE, CREATE PROCEDURE, CREATE INDEX, CREATE VIEW, BEGIN...END のアウトライン表示 |
| Folding Ranges | `textDocument/foldingRange` | ✅ 完了 | CREATE PROCEDURE本体、BEGIN...ENDブロック、長い列定義リストの折りたたみ |
| Completion | `textDocument/completion` | ✅ 完了 | キーワード、データ型、組み込み関数の自動補完（コンテキスト非依存） |

## Tier 2: 実装済み (Phase 2)

| 機能 | LSP メソッド | 状態 | DDLでの価値 |
|------|-------------|------|------------|
| **Hover** | `textDocument/hover` | ✅ 完了 | 識別子上の型情報、関数シグネチャ、ASE固有型のドキュメント表示。CREATE TABLE名ホバーでカラム一覧表示 |
| **Document Formatting** | `textDocument/formatting` | ✅ 完了 | DDLの自動フォーマット。キーワード大文字化、インデント、改行 |
| **Signature Help** | `textDocument/signatureHelp` | ✅ 完了 | 組み込み関数呼び出し時のパラメータ表示。ASEシステムプロシージャの多数パラメータ補完 |

## Tier 3: 実装済み (Phase 3 - スキーマ認識)

| 機能 | LSP メソッド | 状態 | DDLでの価値 |
|------|-------------|------|------------|
| Go to Definition | `textDocument/definition` | ✅ 完了 | テーブル参照→CREATE TABLE定義へジャンプ。プロシージャ呼び出し→定義へジャンプ。変数使用→DECLAREへジャンプ |
| Find References | `textDocument/references` | ✅ 完了 | テーブル名/列名/プロシージャ名の全出現箇所を検索。DDL変更前の影響分析に必須 |
| Symbol Table Builder | 内部基盤 | ✅ 完了 | ASTからテーブル/プロシージャ/ビュー/インデックス/変数のシンボル情報を抽出 |
| Schema-aware Hover | `textDocument/hover` | ✅ 完了 | テーブル名ホバーでカラム情報、変数ホバーで型情報を表示 |

## Tier 4: 実装済み (Phase 4 - コード操作)

| 機能 | LSP メソッド | 状態 | DDLでの価値 |
|------|-------------|------|------------|
| Workspace Symbols | `workspace/symbol` | ✅ 完了 | ワークスペース全体からテーブル/プロシージャ/ビュー/インデックス/変数を検索。大文字小文字区別なし |
| Code Actions | `textDocument/codeAction` | ✅ 完了 | SELECT *のカラム展開、INSERT骨組み生成、TRY...CATCHラッパー。シンボルテーブルを使用したカラム情報取得 |
| Rename | `textDocument/rename` | ✅ 完了 | テーブル/プロシージャ/変数の一括リネーム。大文字小文字区別なしで全参照箇所を更新 |

## Tier 5: 拡張機能 (将来実装)

| 機能 | LSP メソッド | 優先度 | DDLでの価値 |
|------|-------------|--------|------------|
| Document Links | `textDocument/documentLink` | 低 | DDLコメント内のURLをクリッカブルに |
| Inlay Hints | `textDocument/inlayHint` | 低 | 変数使用箇所に型注釈表示。長いストアドプロシージャで有用 |
| Code Lens | `textDocument/codeLens` | 低 | テーブル参照カウント等のインラインメタデータ |
| Schema Explorer | カスタム | 中 | データベーススキーマのツリー表示 |
| Query Execution | カスタム | 低 | エディタからクエリ実行と結果表示 |

## Sybase ASE固有のDDL考慮事項

### ロックスキーム（ASE固有）
```sql
CREATE TABLE foo (...) LOCK DATAROWS    -- 行レベルロック
CREATE TABLE foo (...) LOCK DATAPAGES   -- ページレベルロック
CREATE TABLE foo (...) LOCK ALLPAGES    -- テーブルレベルロック（デフォルト）
```

### ディスク管理（ASE固有）
```sql
DISK INIT name = 'dev1', physname = '/path/dev1.dat', size = '10M'
CREATE DATABASE mydb ON dev1 = '10M' LOG ON logdev = '5M'
```

### ASE固有データ型
- `UNICHAR(n)`, `UNIVARCHAR(n)`, `UNITEXT` — Unicode型
- `BIGDATETIME`, `BIGTIME` — 高精度時間型 (ASE 15.7+)
- `MONEY`, `SMALLMONEY` — 通貨型
- `VARBINARY` — ASE 15+での最大サイズがMSSQLと異なる

### ASEシステム変数
- `@@spid`, `@@servername`, `@@version`, `@@error`, `@@rowcount`
- `@@transtate` (MSSQLに同等なし)
- `@@isolation`, `@@textsize`, `@@nestlevel`

### 最も一般的なASE DDL（頻度順）
1. `CREATE TABLE` — ロックスキーム、IDENTITY列、制約含む
2. `CREATE PROCEDURE` / `ALTER PROCEDURE` — ASE開発の主軸。500-2000+行は日常的
3. `CREATE INDEX` — CLUSTERED/NONCLUSTERED + ASE固有WITH句
4. `CREATE TRIGGER` — 現在Parser未対応（今後の拡張点）
5. `CREATE VIEW` — WITH CHECK OPTION含む
6. `ALTER TABLE` — 現在Parser未対応
7. `CREATE DEFAULT` / `CREATE RULE` — ASE固有オブジェクト型

### ストアドプロシージャ開発に最も有用な機能
ASE開発の80%以上はストアドプロシージャ。長大なプロシージャのナビゲーションと理解が最重要。

| 機能 | プロシージャ開発での価値 |
|------|------------------------|
| Folding Ranges | 500-2000+行のBEGIN...END, IF, WHILE ブロックの折りたたみ |
| Document Symbols | DECLARE ブロック, IF/WHILE/BEGIN セクションのアウトライン |
| Diagnostics | 未終了文字列、不一致BEGIN/END、無効な変数参照の検出 |
| Hover | DECLARE @var VARCHAR(100) の型情報を全使用箇所で表示 |
| Completion | @@ システム変数, sp_ プレフィクスシステムプロシージャの補完 |
| Signature Help | sp_configure等の多数パラメータプロシージャの引数表示 |

## 参考
- [LSP 3.17 仕様](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
- [sqls-server/sqls](https://github.com/sqls-server/sqls) — Go製 SQL Language Server
- [joe-re/sql-language-server](https://github.com/joe-re/sql-language-server) — Node.js製 SQL Language Server
