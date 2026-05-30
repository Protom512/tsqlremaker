# 🖊️ WHITEBOARD

> **各エージェントへ**: 作業前に必ずこのファイルを読むこと。

**最終更新:** 2026-05-31 / Session (EXEC parser #115)

---

## 📊 現在の状態

| 項目 | 状態 |
|------|------|
| **テスト** | 988 passed, 2 skipped |
| **Clippy** | clean (`-D warnings`) |
| **Open Issues** | 19 |
| **Open PRs** | 2 (#106, #121) |
| **ブランチ** | master + feat/code-action-insert-column-list + feat/115-exec-execute-parser |

---

## 🔄 今セッションの成果

### マージ済み PR
| PR | 内容 | Issue |
|----|------|-------|
| #109 | semantic tokens delta削除 | #59 Closed |
| #112 | mysql-emitter デッドコード削除 | #74 Closed |
| #113 | AST-aware TRY...CATCH (PR#102の再作成) | #68 Closed |
| #107 | CaseInsensitiveKey for symbol table | — |
| #108 | hover列名解決 | #78 Closed |
| #103 | SELECT * セマンティック警告 | — |

### Issue分解
| 親Issue | サブIssue |
|---------|-----------|
| #58 (Parser未対応) | #114 ALTER TABLE, #115 EXEC/EXECUTE, #116 CREATE TRIGGER |
| #84 (Phase 5) | #117 Code Lens, #118 Inlay Hints, #119 Document Links |
| #87 (lsp-types version) | Closed — documented accepted limitation |

### ブランチ整理
- 20本以上のstaleブランチを削除（local + remote）
- アクティブブランチは #106 のみ残存

---

## 🔀 申し送り（次セッションへ）

### 優先度高
1. **PR #121** (EXEC parser): レビュー待ち。マージ後 #115 Close。
2. **PR #106** (INSERT column list): masterにリベースが必要。CI未完了。マージ後にconflict解消。
3. **#116 CREATE TRIGGER parser**: #58サブIssueの残り。

### 優先度中
3. **#71 db_docs.rs**: 1305行モノリス。データとロジックの分離。
4. **#82 Parser error recovery**: 現在最初のエラーで停止。build_tolerant()で部分的に対応済み。

### 残りのOpen Issues (14件、新規サブIssue除く)
| Issue | 分類 | 難易度 |
|-------|------|--------|
| #86 | Parser precedence | Medium |
| #82 | Parser error recovery | Large |
| #81 | LSP configuration | Large |
| #79 | LSP error handling | Medium |
| #77 | Signature help nested | Medium |
| #75 | SQLite converter | Medium |
| #71 | db_docs.rs monolith | Medium |
| #70 | Cross-file definition | Large |
| #65 | Multi-file workspace | Large |
| #61 | WASM AST conversion | Large |
| #60 | Range formatting | Medium |
| #58 | Parser statements (remaining) | Medium (sub-issues: #114-116) |
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
