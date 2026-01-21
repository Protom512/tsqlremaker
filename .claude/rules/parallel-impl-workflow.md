# Parallel Implementation Workflow

並列実行対応の実装ワークフローを定義します。

## ワークフロー概要

```
┌─────────────────────────────────────────────────────────────────┐
│                     並列実行ワークフロー                          │
└─────────────────────────────────────────────────────────────────┘

1. 依存関係解析
    ↓
2. タスクグループ分け (Group 0, 1, 2, ...)
    ↓
3. Git Worktree 作成
    ↓
4. グループ順次実行
    │
    ├─ Group 0 → 並列実行 (複数エージェント) → 全完了 → マージ
    │
    ├─ Group 1 → 並列実行 (複数エージェント) → 全完了 → マージ
    │
    └─ Group N → 並列実行 (複数エージェント) → 全完了 → マージ
    ↓
5. 最終マージとテスト
    ↓
6. クリーンアップ
```

## 使用方法

### コマンド

```bash
# 全タスクを並列実行
/kiro:spec-impl-parallel sap-ase-lexer

# 特定タスクのみ実行
/kiro:spec-impl-parallel sap-ase-lexer 1.1,1.2,1.3

# ステータス確認
/kiro:spec-impl-parallel sap-ase-lexer --status
```

### ステータス確認コマンド

```bash
# 実行中のタスク一覧
/kiro:spec-impl-status sap-ase-lexer

# worktree 一覧
git worktree list

# ブランチ一覧
git branch | grep impl/
```

## タスクグループ分けのルール

### 依存関係の定義 (tasks.md)

```markdown
## Task 1.1: Token構造体の定義
depends: []
files: [crates/tsql-token/src/token.rs]

## Task 1.2: TokenKind列挙型の定義
depends: []
files: [crates/tsql-token/src/kind.rs]

## Task 2.1: Lexerメイン構造体
depends: [1.1, 1.2, 1.3]  # Token 関連が完了している必要
files: [crates/tsql-lexer/src/lexer.rs]
```

### 自動グループ分け

```
依存関係なし同士 → 同一グループ (並列実行可)
依存関係あり → 別グループ (順次実行)

例:
Task 1.1 (deps:[]) ─┐
Task 1.2 (deps:[])  ├→ Group 0 (並列実行)
Task 1.3 (deps:[]) ─┘
        ↓
Task 2.1 (deps:[1.1,1.2,1.3]) → Group 1 (Group 0 完了後に実行)
```

## Git Worktree 構成

### ディレクトリ構成

```
tsqlremaker/
├── .git/
├── .worktrees/
│   └── sap-ase-lexer/
│       ├── group-0/          # Worktree for Group 0
│       │   ├── .git/
│       │   ├── crates/
│       │   └── ...
│       ├── group-1/          # Worktree for Group 1
│       │   ├── .git/
│       │   ├── crates/
│       │   └── ...
│       └── group-2/          # Worktree for Group 2
│           ├── .git/
│           ├── crates/
│           └── ...
├── crates/
└── ...
```

### ブランチ構成

```
main (ベース)
 └── impl/sap-ase-lexer (実装ブランチ)
     ├── impl/sap-ase-lexer-group-0 (Group 0 用)
     ├── impl/sap-ase-lexer-group-1 (Group 1 用)
     └── impl/sap-ase-lexer-group-2 (Group 2 用)
```

## ファイル所有権の管理

### 所有権割り当てルール

| タスク種類 | 所有権 | 例 |
|-----------|--------|-----|
| 専用モジュール実装 | 単一タスク | `token.rs` → Task 1.1 のみ |
| 共有インターフェース | 調整タスク | `lib.rs` → 先行タスクで追加 |
| テストファイル | 単一タスク | `token_tests.rs` → Task 1.1 のみ |
| Cargo.toml | 調整タスク | 依存関係の追加のみ |

### 所有権表の例

```markdown
## File Ownership Map

### Group 0
| Task | Files |
|------|-------|
| 1.1 | crates/tsql-token/src/token.rs |
| 1.2 | crates/tsql-token/src/kind.rs |
| 1.3 | crates/tsql-token/src/position.rs |

### Group 1
| Task | Files |
|------|-------|
| 2.1 | crates/tsql-lexer/src/lexer.rs |
| 2.2 | crates/tsql-lexer/src/cursor.rs |

### Shared (Read-only during impl)
| File | Access |
|------|--------|
| crates/tsql-token/src/lib.rs | Read-only |
| Cargo.toml | Read-only |
```

## 実行ステータスの追跡

### ステータスファイル

`.kiro/specs/{feature}/.execution-status.json`

```json
{
  "feature": "sap-ase-lexer",
  "started_at": "2026-01-20T10:00:00Z",
  "status": "running",
  "groups": [
    {
      "id": 0,
      "status": "completed",
      "tasks": [
        {"id": "1.1", "status": "completed", "commit": "a1b2c3d"},
        {"id": "1.2", "status": "completed", "commit": "d4e5f6g"},
        {"id": "1.3", "status": "completed", "commit": "h7i8j9k"}
      ],
      "merged_at": "2026-01-20T10:05:00Z"
    },
    {
      "id": 1,
      "status": "running",
      "tasks": [
        {"id": "2.1", "status": "running", "agent_id": "xxx"},
        {"id": "2.2", "status": "pending"}
      ]
    }
  ]
}
```

### ステータス確認

```bash
# JSON でステータス表示
cat .kiro/specs/sap-ase-lexer/.execution-status.json | jq

# 人間 readable なサマリー
/kiro:spec-impl-status sap-ase-lexer
```

## コンフリクト解消戦略

### 予防的措置

1. **ファイル所有権の分離**: 各タスクが異なるファイルを編集
2. **pub use の先行追加**: lib.rs への export は事前に追加
3. **型定義の先行実装**: 共通で使う型は先に定義

### コンフリクト解消パターン

| パターン | 解消方法 |
|---------|----------|
| `pub use` の重複 | 統合（重複削除） |
| テストの重複 | マージ |
| 型定義の重複 | 最初の定義を採用 |
| 実装ロジックの競合 | 手動解消 |

### 解消コマンド

```bash
# コンフリクトファイルを表示
git diff --name-only --diff-filter=U

# コンフリクトをマージツールで開く
git mergetool <file>

# 解消後マージ完了
git add <file>
git commit
```

## クリーンアップ

### 実行完了後

```bash
# worktree の削除
git worktree remove .worktrees/sap-ase-lexer/group-*

# 一時ブランチの削除
git branch -D impl/sap-ase-lexer-group-*

# ステータスファイルの削除
rm .kiro/specs/sap-ase-lexer/.execution-status.json
```

### 中断時のクリーンアップ

```bash
# 全worktreeの削除
git worktree remove .worktrees/sap-ase-lexer/

# 実装ブランチの削除
git branch -D impl/sap-ase-lexer impl/sap-ase-lexer-group-*
```

## エラーハンドリング

### サブエージェント失敗時

```
1. 同一グループの他タスクをキャンセル
2. エラー内容を報告
3. 失敗したタスクのみ再実行
4. 3回失敗したら手動介入を依頼
```

### Git 操作失敗時

```
1. Git エラー内容を報告
2. クリーンアップ手順を提示
3. 状態復旧コマンドを提案
```

### ワークツリー破損時

```
1. 破損した worktree を特定
2. worktree を prune して削除
3. 再作成を試行
```

## チェックリスト

### 実行前

- [ ] ワーキングディレクトリが clean
- [ ] tasks.md が存在
- [ ] 依存関係に循環がない
- [ ] git worktree が使用可能

### 実行中

- [ ] 各グループのタスクが並列実行されている
- [ ] 所有ファイル外の変更が発生していない
- [ ] コミットが適切に作成されている

### 実行後

- [ ] 全タスクが完了
- [ ] マージが成功
- [ ] cargo test がパス
- [ ] cargo clippy がパス
- [ ] worktree が削除されている

## トラブルシューティング

### 問題: 並列実行されない

**原因**: 依存関係が正しく設定されていない

**解決**:
```bash
# tasks.md の depends フィールドを確認
grep -A 2 "depends:" .kiro/specs/*/tasks.md
```

### 問題: ファイル衝突が発生

**原因**: 複数タスクが同一ファイルを編集

**解決**:
1. tasks.md の files フィールドを確認
2. 所有権を調整
3. 必要に応じて調整タスクを追加

### 問題: worktree が作成できない

**原因**: 既存の worktree が残っている

**解決**:
```bash
# 既存 worktree を確認
git worktree list

# 不要な worktree を削除
git worktree remove <path>

# prune で破損した worktree を削除
git worktree prune
```

## 参考情報

- Git Worktree Documentation: https://git-scm.com/docs/git-worktree
- TDD Best Practices: `.claude/skills/rust-tdd.md`
- Architecture Rules: `.claude/rules/architecture-coupling-balance.md`
