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
cargo test --all
```

## 各チェックの意味

| コマンド | 目的 | 失敗時の対処 |
|---------|------|-------------|
| `cargo fmt --all --check` | コードフォーマットの検証 | `cargo fmt --all` を実行して修正 |
| `cargo check --all` | コンパイルエラーの検出 | コンパイルエラーを修正 |
| `cargo clippy -- -D warnings` | Lint警告の検出 | Clippy警告を修正 |
| `cargo test --all` | テストの実行 | テスト失敗を修正 |

## Claude Code Hook

このプロジェクトでは `command-start-hook` が設定されており、`git commit` 実行時に自動的に上記チェックが実行されます。

## チェックが失敗した場合

1. **フォーマットエラー**: `cargo fmt --all` を実行
2. **Checkエラー**: エラーメッセージを確認して修正
3. **Clippy警告**: 警告を修正（`-D warnings` は警告をエラーとして扱う）
4. **テスト失敗**: テストエラーを修正

## スキップする場合

緊急時やdraft commitの場合は、手動で `git commit --no-verify` を使用することでチェックをスキップできます。

```bash
git commit --no-verify -m "WIP: draft commit"
```
