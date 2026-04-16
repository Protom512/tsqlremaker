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

## Tier 2: 次期実装 (Phase 2)

| 機能 | LSP メソッド | 優先度 | DDLでの価値 |
|------|-------------|--------|------------|
| **Hover** | `textDocument/hover` | 最高 | 識別子上の型情報、関数シグネチャ、ASE固有型（UNICHAR, BIGDATETIME, MONEY）のドキュメント表示 |
| **Document Formatting** | `textDocument/formatting` | 高 | DDLの自動フォーマット。キーワード大文字化、インデント、改行。 ASE開発者はフォーマット不良のレガシーDDLスクリプトを扱うことが多い |
| **Signature Help** | `textDocument/signatureHelp` | 高 | 組み込み関数呼び出し時のパラメータ表示。ASEシステムプロシージャ（sp_configure等）の多数の位置パラメータ補完に有用 |

## Tier 3: スキーマ認識機能 (Phase 3)

| 機能 | LSP メソッド | 優先度 | DDLでの価値 |
|------|-------------|--------|------------|
| Go to Definition | `textDocument/definition` | 高 | テーブル参照→CREATE TABLE定義へジャンプ。プロシージャ呼び出し→定義へジャンプ。変数使用→DECLAREへジャンプ |
| Find References | `textDocument/references` | 高 | テーブル名/列名/プロシージャ名の全出現箇所を検索。DDL変更前の影響分析に必須 |
| Code Actions | `textDocument/codeAction` | 中 | クイックフィックス: "SELECT * FROM table生成", "INSERT骨組み生成", "TRY...CATCHラッパー", "sp_helptext→CREATE PROCEDURE変換" |
| Rename | `textDocument/rename` | 中 | テーブル/列/プロシージャ/変数の一括リネーム。DDLリファクタリングに有用 |

## Tier 4: 拡張機能 (Phase 4)

| 機能 | LSP メソッド | 優先度 | DDLでの価値 |
|------|-------------|--------|------------|
| Document Links | `textDocument/documentLink` | 低 | DDLコメント内のURLをクリッカブルに |
| Inlay Hints | `textDocument/inlayHint` | 低 | 変数使用箇所に型注釈表示。長いストアドプロシージャで有用 |
| Workspace Symbols | `workspace/symbol` | 低 | ワークスペース全体からテーブル/プロシージャ/ビュー/トリガー定義を検索 |
| Code Lens | `textDocument/codeLens` | 低 | テーブル参照カウント等のインラインメタデータ |

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
