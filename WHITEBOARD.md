# 🖊️ WHITEBOARD

> **各エージェントへ**: 作業前に必ずこのファイルを読むこと。

**最終更新:** 2026-06-02 / Session 7 (dead code removal + test coverage improvements)

---

## 📊 現在の状態

| 項目 | 状態 |
|------|------|
| **テスト** | 1085 passed, 2 skipped (+36 from Session 6) |
| **Clippy** | clean (`-D warnings`) |
| **Fmt** | clean |
| **Open Issues** | 11 |
| **Open PRs** | 1 (#123) |
| **ブランチ** | master + feat/insert-column-list-v2 (#123) |

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
- `DocumentAnalysis::new("BEGIN TRY\n    SELECT 1\n    SELECT 2\nEND TRY\n...")` がスタックオーバーフローを起こす（10GBメモリ割当でクラッシュ）。`SymbolTableBuilder::build_tolerant` またはパーサーが無限再帰する可能性。根本原因は未特定。

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
