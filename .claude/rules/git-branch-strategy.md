# Git Branch Strategy

ブランチ管理戦略とコミット粒度のルール。

## ブランチ命名規則

| パターン | 用途 | 例 |
|---------|------|-----|
| `feature/<scope>-<description>` | 新機能実装 | `feature/ase-ls-phase4-code-ops` |
| `fix/<scope>-<description>` | バグ修正 | `fix/parser-create-unique-index` |
| `refactor/<scope>-<description>` | リファクタリング | `refactor/symbol-table-extraction` |
| `impl/<spec-name>` | スペック駆動の並列実装 | `impl/sap-ase-lexer` |

### LSP機能ブランチ

```
master
 └── feature/ase-ls-phase<N>-<description>
      ├── 1コミット = 1機能（原則）
      ├── 全テスト通過必須
      └── PR作成後にmasterへマージ
```

## コミット粒度

### 原則: 1コミット = 1論理的変更

```bash
# ✅ 良い: 機能ごとにコミット
git commit -m "feat(lsp): add workspace symbols handler"
git commit -m "feat(lsp): add code actions for SELECT * expansion"
git commit -m "test(lsp): add workspace symbols tests"

# ❌ 悪い: 全機能を1コミットに詰め込む
git commit -m "feat(lsp): add Phase 1-3 features"  # 8000行
```

### コミットメッセージ規約

```
<type>(<scope>): <description>

[optional body]

Co-Authored-By: glm 4.7 <noreply@zhipuai.cn>
```

| type | 用途 |
|------|------|
| `feat` | 新機能 |
| `fix` | バグ修正 |
| `refactor` | リファクタリング |
| `test` | テスト追加・修正 |
| `docs` | ドキュメント |
| `chore` | ビルド・設定変更 |

## ブランチライフサイクル

### 作成

```bash
# masterから最新を取得
git checkout master && git pull

# フィーチャーブランチを作成
git checkout -b feature/ase-ls-phase5-inlay-hints
```

### マージ

```bash
# PR経由でmasterへマージ（squash merge推奨）
gh pr create --title "feat(lsp): Phase 5 inlay hints" --base master
```

### クリーンアップ

マージ済みブランチは速やかに削除:

```bash
# マージ済みブランチの一覧
git branch --merged master

# 削除
git branch -d feature/ase-ls-phase5-inlay-hints
git push origin --delete feature/ase-ls-phase5-inlay-hints
```

## stash 管理

### ルール

- stash は一時的な作業保存のみに使用
- 永続的な変更はブランチにコミットすること
- セッション開始時に `git stash list` を確認し、不要なstashを削除

```bash
# stashの確認
git stash list

# 不要なstashの削除
git stash drop stash@{N}
```

## チェックリスト

### ブランチ作成時

- [ ] masterから分岐している
- [ ] ブランチ名が命名規則に従っている
- [ ] 作業開始前に `git pull` を実行している

### コミット時

- [ ] 1コミット = 1論理的変更
- [ ] `cargo test --all` が通る
- [ ] `cargo clippy -- -D warnings` が通る
- [ ] `cargo fmt --check` が通る

### マージ時

- [ ] 全テストが通る
- [ ] コミット履歴が整理されている
- [ ] PR説明に変更内容が記載されている
