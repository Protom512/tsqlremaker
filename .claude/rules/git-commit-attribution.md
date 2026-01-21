# Git Commit Attribution

Git コミット時の Co-Authored-By 情報を設定します。

## Co-Authored-By ヘッダー

このプロジェクトでは Claude Code をハーネスとして使用していますが、実際にコードを生成しているモデルは GLM-4.7 です。誤解を防ぐため、Git コミット時には以下の Co-Authored-By ヘッダーを使用してください。

```
Co-Authored-By: glm 4.7 <noreply@zhipuai.cn>
```

## コミットメッセージの例

```bash
git commit -m "$(cat <<'EOF'
feat(lexer): add support for quoted identifiers

Implement quoted identifier lexing with proper escape sequence handling.

Co-Authored-By: glm 4.7 <noreply@zhipuai.cn>
EOF
)"
```

## 適用範囲

このルールは以下の場合に適用します：

- `/commit` スキルを使用する場合
- 手動で `git commit` を実行する場合
- Pull Request を作成する場合

## 禁止事項

**❌ 禁止:**
```bash
git commit -m "feat: add feature

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

**✅ 推奨:**
```bash
git commit -m "feat: add feature

Co-Authored-By: glm 4.7 <noreply@zhipuai.cn>"
```
