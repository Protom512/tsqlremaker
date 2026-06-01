# 🖊️ WHITEBOARD

> **各エージェントへ**: 作業前に必ずこのファイルを読むこと。

**最終更新:** 2026-06-01 / Session 6 (test coverage improvements + bug fix)

---

## 📊 現在の状態

| 項目 | 状態 |
|------|------|
| **テスト** | 1049 passed, 2 skipped |
| **Clippy** | clean (`-D warnings`) |
| **Open Issues** | 11 |
| **Open PRs** | 1 (#123) |
| **ブランチ** | master + feat/insert-column-list-v2 (#123) |

---

## 🔄 Session 3 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `63bac60` | refactor(db_docs): split 1305-line monolith into focused modules (#71) |

### 追加実績
- 24 stale remote branches pruned
- dependabot alert resolved (rand 0.9.2 → 0.9.4)

---

## 🔄 Session 5 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `9ca9d8a` | refactor: extract helpers, remove dead code, improve emitter patterns |
| `6bf6e0d` | fix(lsp): stack-based signature help for nested function calls (#77) |

### 変更内容
- **hover.rs**: `format_column_hover()` + `in_span()` ヘルパー抽出（3箇所の重複コード解消）
- **sqlite-emitter**: `function_mapper` モジュール抽出（テーブル駆動の関数マッピング、datepart変換）
- **dead code削除**: postgresql/sqlite emitter の未使用 `Result<T>` 型エイリアス削除
- **unused imports解消**: mysql/sqlite emitter の不要な `#[allow(unused_imports)]` と未使用インポート削除
- テスト 1018 passed (+13 from previous session)
- **#77 Signature help nested**: CallFrame スタックベースの追跡に修正。ネストした関数呼び出しで内側の関数を正しく特定

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `87aa82e` | fix(parser): propagate parse errors after comma in EXEC arguments |
| `12ff01b` | refactor(postgresql-emitter): replace TODO comments with structured conversion hints |

### PR #123 更新（レビュー指摘対応）
| 内容 | 状態 |
|------|------|
| Token scanner境界制限（insert_end追加） | ✅ Pushed to feat/insert-column-list-v2 |
| 非破壊挿入（ゼロ幅insertion at VALUES） | ✅ Pushed |
| テスト3件追加（cross-statement, comments, zero-width） | ✅ |

---

## 🔄 Session 6 成果

### Issue クローズ（4件）
| Issue | 内容 |
|-------|------|
| #86 | tech-debt(parser): precedence cliff — 完了 (d65a149) |
| #79 | arch(LSP): error handling — 完了 (df81785) |
| #77 | tech-debt(LSP): signature help nested — 完了 (6bf6e0d) |
| #71 | arch(LSP): db_docs monolith — 完了 (63bac60) |

### PR #123 カバレッジ改善
| 内容 | コミット |
|------|---------|
| code_actions.rs 9テスト追加 (90.40%→91.03%) | `df876ff` on feat/insert-column-list-v2 |

### テストカバレッジ改善（master）
| コミット | モジュール | 変更 |
|---------|-----------|------|
| `950be14` | symbols.rs | 65.65% → **90.08%** (+10テスト) |
| `950be14` | workspace_symbols.rs | 64.35% → **100.00%** (+8テスト) |
| `17a0ff2` | signature_help.rs | 73.63% → **96.94%** (+6テスト) |
| `17a0ff2` | workspace_symbols.rs | index container_name バグ修正 |
| `dc33d8f` | semantic_tokens.rs | range API + トークンタイプ 8テスト追加 |

### 全体カバレッジ推移
- ase-ls-core: 87.44% → **89.82%** (+2.38%)
- テスト数: 1018 → **1049** (+31)

---

## 🔀 申し送り（次セッションへ）

### 優先度高
1. **PR #123** (INSERT column list): レビュー指摘対応済み + カバレッジ改善プッシュ済み。マージ待ち。

### 優先度中
2. **#82 Parser error recovery**: 現在最初のエラーで停止。build_tolerant()で部分的に対応済み。
3. **#75 SQLite converter**: function_mapperは抽出済み。コンバータパターンの一般化。

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
