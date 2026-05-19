# Pre-Commit Rust Checks

## ルール

Git コミットを作成する前に、以下のコマンドを必ず実行し、すべて成功することを確認してください。

## 必須チェック

```bash
# 1. フォーマットチェック
cargo fmt --all --check

# 2. 型チェック
cargo check --all

# 3. Lint
cargo clippy --all-targets -- -D warnings

# 4. テスト
cargo nextest run --workspace
```

## 各チェックの意味

| コマンド | 目的 | 失敗時の対処 |
|---------|------|-------------|
| `cargo fmt --all --check` | コードフォーマットの検証 | `cargo fmt --all` を実行して修正 |
| `cargo check --all` | コンパイルエラーの検出 | コンパイルエラーを修正 |
| `cargo clippy -- -D warnings` | Lint警告の検出 | Clippy警告を修正 |
| `cargo nextest run --workspace` | テストの実行 | テスト失敗を修正 |

## Claude Code Hook

このプロジェクトでは `command-start-hook` が設定されており、`git commit` 実行時に自動的に上記チェックが実行されます。

## チェックが失敗した場合

1. **フォーマットエラー**: `cargo fmt --all` を実行
2. **Checkエラー**: エラーメッセージを確認して修正
3. **Clippy警告**: 警告を修正（`-D warnings` は警告をエラーとして扱う）
4. **テスト失敗**: テストエラーを修正
5. **nextest未インストール**: `cargo install cargo-nextest --locked` を実行

## WIPコミット（Work In Progress）

実装が途中でセッションを継続している場合、変更消失を防ぐためにWIPコミットを許可する。

### WIPコミットの条件

- 実装が途中でコンテキスト制限に近い場合
- セッション断絶のリスクがある場合
- 大規模な移行作業の途中経過を保存したい場合

### WIPコミットの手順

```bash
# WIPコミット（pre-commit hookをスキップ）
git add -A
git commit --no-verify -m "WIP: <description of current progress>"
```

### WIPコミットの解除

次セッションで作業を再開する際:

```bash
# WIPコミットを解除してステージングに戻す
git reset HEAD~1

# 変更を確認して続きを実装
git status
```

### 注意事項

- WIPコミットは **pushしない**（ローカルのみ保持）
- WIPコミットのメッセージには進捗状況を記述する
- 作業完了時にWIPコミットは適切なコミットに分割する
