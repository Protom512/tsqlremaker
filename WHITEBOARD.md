# 🖊️ WHITEBOARD

> **各エージェントへ**: 作業前に必ずこのファイルを読むこと。

**最終更新:** 2026-06-04 / Session 12 (Trigger recursion fix, integration tests, branch cleanup)

---

## 📊 現在の状態

| 項目 | 状態 |
|------|------|
| **テスト** | 1137 passed, 2 skipped |
| **Clippy** | clean (`-D warnings`) |
| **Fmt** | clean |
| **Open Issues** | 11 |
| **Open PRs** | 1 (#123) |
| **ブランチ** | master + feat/insert-column-list-v2 (#123) |

---

## 🔄 Session 12 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `583c2c7` | fix(lsp): add Trigger body recursion to folding, hover, diagnostics, code_actions |
| `b2694a7` | fix(references): detect CREATE UNIQUE INDEX and CREATE TRIGGER as definitions |
| `da35bbb` | test(lsp): add 12 integration tests for definition, references, rename, code actions, diagnostics |

### 変更内容
- **folding.rs**: `collect_ast_folds` に Trigger ボディ再帰追加。Procedure body も再帰するよう修正（従来はトップレベルフォールドのみ）
- **hover.rs**: `resolve_column_in_statement` に Trigger ボディ再帰追加。Trigger 内のカラムに hover 表示可能に
- **diagnostics.rs**: `collect_select_star_warnings` に Trigger ボディ再帰追加。Trigger 内の SELECT * が警告対象に
- **code_actions.rs**: `find_block_at_offset` に Trigger ボディ再帰追加。Trigger 内の TRY...CATCH code action が動作するように
- **references.rs**: (前セッション) CREATE UNIQUE INDEX と CREATE TRIGGER を定義として検出
- **テスト**: +5 テスト（folding 2, diagnostics 1, hover 1, code_actions 1）+ 12 統合テスト
- **ブランチ整理**: `feat/114-alter-table-parser`（古い）と `feat/code-action-insert-column-list`（旧PR）を削除。リモート旧ブランチも削除

### 調査結果
- TODO/FIXME/HACK マーカーはコード内に存在しない（テスト内アサーションのみ）
- 全 `#[allow(dead_code)]` は「将来のフォーマット機能用」として意図的に文書化済み
- 全 wildcard `_ => None` / `_ => {}` は意図的で正しいことを確認
- rename, semantic_tokens, completion, definition, signature_help, formatting はトークンレベル処理のため Trigger 再帰不要
- doc comments は全公開API関数に既に付与されていることを確認

---

## 🔄 Session 11 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `fa2bbce` | feat(parser): implement Display for DataType + fix hover output |

### コミット（PR #123 ブランチ）
| コミット | 内容 |
|---------|------|
| `5aab4d5` | test(lsp): add regression + direct unit tests for INSERT column list |

### 変更内容
- **tsql-parser/ast/ddl.rs**: `DataType` enum に `Display` trait を実装。SQL標準の形式（`INT`, `VARCHAR(100)`, `DECIMAL(10,2)`）を出力。従来の `Debug`（`Int`, `Varchar(Some(100))`）をユーザー向け出力から排除
- **hover.rs**: 全 `{:?}` を `{}` に置換し、`DataType` の Display を使用。TryCatch パターンを `chain().find_map()` に統一
- **code_actions.rs**（PR #123）: CodeRabbit レビュー指摘に対応。9テスト追加（複数INSERT回帰テスト、`resolve_insert_stmt_end`/`find_values_token_start`/`build_fallback_symbol_table` の直接テスト）

## 🔄 Session 10 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `5692413` | fix(analysis): avoid O(n) LineIndex recomputation in Clone impl |
| `9446695` | refactor(code_actions): extract make_quickfix/make_refactor helpers |
| `aa6c4ce` | refactor(workspace_symbols): deduplicate symbol iteration code |
| `680659e` | test(completion): add 3 edge case tests for completion module |
| `4e06ca7` | test(parser): add 12 to_common.rs conversion tests |
| `f71cea4` | docs(emitter): clarify dead_code indentation helpers as future use |

### 変更内容
- **analysis.rs**: `DocumentAnalysis::clone()` が `LineIndex::new()` を再計算していたバグを修正（`derive(Clone)` + 直接clone）
- **code_actions.rs**: 6箇所のCodeAction構築ボイラープレートを `make_quickfix()` / `make_refactor()` ヘルパーに抽出。`InsertSource` のワイルドカードマッチを明示的マッチに修正
- **workspace_symbols.rs**: `push_matching()` + `collect_symbols()` ヘルパーで~80行の重複コードを削除（171行→87行）
- **completion.rs**: 3テスト追加（システム変数、重複ラベル、構文エッジケース）
- **common_ast_conversion.rs**: 12テスト追加（INSERT SELECT, CASE, ORDER/GROUP/HAVING, EXISTS, unary minus, NOT BETWEEN/LIKE, hex literal, BatchSeparator, CREATE方言固有, UPDATE/DELETE FROM方言固有, サブクエリ, 制御フロー）
- **postgresql-emitter/sqlite-emitter**: `#[allow(dead_code)]` に「将来のフォーマット機能用」コメント追加

## 🔄 Session 8 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `18be817` | fix(parser): prevent infinite loop in parse_with_errors error recovery |

### 修正内容
- **根本原因**: `parse_with_errors` のエラー回復で `synchronize()` が同期ポイント（END等）で停止してもトークンを消費しないため、`END` が `parse_statement` の有効開始トークンでない場合に無限ループ発生
- **修正**: `synchronize()` 後に必ず1トークン消費して前進を保証
- **影響**: 複数行の `BEGIN TRY...END TRY BEGIN CATCH...END CATCH` 入力で `DocumentAnalysis::new` がスタックオーバーフローしていた問題が解消

---

## 🔄 Session 7 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `8d08592` | perf(analysis): remove unused tokens_with_comments computation |
| `6daa25d` | test(lsp): add 24 tests for folding, rename, definition, references coverage |
| `7eddb9b` | test(lsp): add 4 diagnostics tests for empty source and multi-statement coverage |
| `8705faa` | test(lsp): add 12 tests for diagnostics, formatting, and hover coverage |

### 変更内容
- **analysis.rs**: `tokens_with_comments` の不要なLexer二重実行を削除（1パス節約）
- **folding.rs**: +7テスト（TRY...CATCH, CREATE PROCEDURE, 単一行IF, 不一致BEGIN, 複数コメント, ネストWHILE, 空ソース）
- **rename.rs**: +7テスト（Analysis版 variable/table/empty/beyond-end/reject, placeholder）
- **definition.rs**: +5テスト（Analysis版 variable/table/empty/no-token/procedure）
- **references.rs**: +5テスト（Analysis版 variable/table/empty/no-token/exclude-declaration）
- **diagnostics.rs**: +4テスト（空ソース, 複数SELECT *, 位置インデックス）
- **formatting.rs**: +5テスト（空ソース, ドット表記, 括弧, HexString, 変更なし）
- **hover.rs**: +3テスト（空ソース, GETDATE関数, 範囲外位置）

### 発見した既存バグ
- ~~DocumentAnalysis スタックオーバーフロー~~ → **Session 8 で修正**（parser infinite loop）

---

## 🔀 申し送り（次セッションへ）

### 優先度高
1. **PR #123** (INSERT column list): レビュー指摘対応済み + カバレッジ改善プッシュ済み。マージ待ち。

### 優先度中
2. **DocumentAnalysis スタックオーバーフロー**: `BEGIN TRY\n    SELECT 1\n    SELECT 2\nEND TRY\nBEGIN CATCH\n    SELECT -1\nEND CATCH` で DocumentAnalysis::new がクラッシュ。根本原因調査が必要。
3. **#82 Parser error recovery**: 現在最初のエラーで停止。build_tolerant()で部分的に対応済み。
4. **#75 SQLite converter**: function_mapperは抽出済み。コンバータパターンの一般化。

### 残りのOpen Issues (11件)
| Issue | 分類 | 難易度 |
|-------|------|--------|
| #82 | Parser error recovery | Large |
| #81 | LSP configuration | Large |
| #75 | SQLite converter | Medium |
| #70 | Cross-file definition | Large |
| #65 | Multi-file workspace | Large |
| #61 | WASM AST conversion | Large |
| #60 | Range formatting | Medium |
| #54 | Context-aware completion | Large |
| #52 | Incremental sync | Large |
| #119 | Code Lens support | Medium |
| #118 | Inlay Hints support | Medium |

---

## 🏗️ アーキテクチャノート

### 依存関係 (更新なし)
```
ase-ls (tower-lsp 0.20, lsp-types 0.94.1)
  └── ase-ls-core (lsp-types 0.94.1)
        └── tsql-parser
              └── tsql-lexer (tsql-token)
```

### 結合度分析 (2026-05-30 cargo coupling)
- **Grade C** (Score 0.88): 4 High, 40 Medium issues
- 主な問題: tsql-token (68 dependents), tsql-parser (86 dependents), parser module (171 functions)
- 改善は長期的なリファクタリングとして計画が必要

---

## 📋 過去セッション成果（要約）

| Session | コミット数 | テスト数 | 主な成果 |
|---------|-----------|---------|---------|
| 3 | 1 | — | db_docs monolith split (#71) |
| 5 | 4 | 1018 | signature help fix (#77), dead code removal |
| 6 | 4 | 1049 | 4 issues closed, coverage +2.38% |
| 7 | 4 | 1085 | dead code removal, +36 tests across 8 modules |
| 8 | 1 | 1085 | fix parser infinite loop in error recovery |
| 9 | 2 | 1097 | +12 tests for lib.rs, symbol_table, line_index |
| 10 | 6 | 1115 | bug fix (analysis Clone), dedup 2 modules, +15 tests |
| 11 | 1 | 1115 | DataType Display impl + PR #123 review tests |
| 12 | 3 | 1137 | Trigger body recursion fix, +12 integration tests, branch cleanup |
