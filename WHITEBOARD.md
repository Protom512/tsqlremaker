# 🖊️ WHITEBOARD

> **各エージェントへ**: 作業前に必ずこのファイルを読むこと。

**最終更新:** 2026-05-31 / Session 4 (parser rename + tracing)

---

## 📊 現在の状態

| 項目 | 状態 |
|------|------|
| **テスト** | 1005 passed, 2 skipped |
| **Clippy** | clean (`-D warnings`) |
| **Open Issues** | 19 |
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

## 🔄 今セッションの成果

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

## 🔀 申し送り（次セッションへ）

### 優先度高
1. **PR #123** (INSERT column list): レビュー指摘3件対応済み。マージ待ち。
2. **#116 CREATE TRIGGER parser**: #58サブIssueの残り（PR #121でEXEC対応済み、#114 ALTER TABLEも対応済み）。

### 優先度中
3. **#71 db_docs.rs**: 1305行モノリス。データとロジックの分離。
4. **#82 Parser error recovery**: 現在最初のエラーで停止。build_tolerant()で部分的に対応済み。

### 残りのOpen Issues (14件)
| Issue | 分類 | 難易度 |
|-------|------|--------|
| #86 | ~~Parser precedence~~ → **完了** (d65a149) | ~~Medium~~ |
| #82 | Parser error recovery | Large |
| #81 | LSP configuration | Large |
| #79 | ~~LSP error handling~~ → **完了** (df81785) | ~~Medium~~ |
| #77 | Signature help nested | Medium |
| #75 | SQLite converter | Medium |
| #71 | ~~db_docs.rs monolith~~ → **完了** (63bac60) | ~~Medium~~ |
| #70 | Cross-file definition | Large |
| #65 | Multi-file workspace | Large |
| #61 | WASM AST conversion | Large |
| #60 | Range formatting | Medium |
| #58 | Parser statements (remaining: #116 only) | Medium |
| #54 | Context-aware completion | Large |
| #52 | Incremental sync | Large |

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
