## Summary
<!-- PRの内容を1〜2行で要約してください -->

<!-- Example: SAP ASE T-SQLのTOP句をMySQLのLIMIT句に変換する機能を追加 -->

## Related Issue
<!-- 関連するIssueを記載してください -->
<!-- Fixes #123 -->

## Changes
<!-- 変更内容を記載してください -->
<!--
- 変更点1
- 変更点2
- 変更点3
-->

## Type of Change
<!-- 変更の種類にチェックを入れてください -->
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update
- [ ] Refactoring
- [ ] Test improvement
- [ ] Other

## Testing
<!-- テスト方法を記載してください -->
<!--
### テスト手順
1. `cargo test --workspace` を実行
2. 以下のSQLを入力: `...`
3. 期待する結果: `...`
-->

### Test Results
<!-- テスト結果を貼り付けてください -->
<!--
```
running 21 tests
test test1 ... ok
test test2 ... ok
...
```
-->

## Checklist
<!-- 以下のチェックリストを確認してください -->
- [ ] コードがプロジェクトのコーディング規約に従っている
- [ ] `cargo fmt -- --check` にパスしている
- [ ] `cargo clippy --workspace -- -D warnings` にパスしている
- [ ] `cargo test --workspace` にパスしている
- [ ] テストが追加/更新されている
- [ ] ドキュメントが更新されている（必要な場合）
- [ ] 変更によって破壊的変更が生じる場合、`CHANGELOG.md` が更新されている

## Architecture Considerations
<!-- アーキテクチャへの影響がある場合は記載してください -->
<!-- .claude/rules/architecture-coupling-balance.md を参照 -->
<!--
- 新しい依存関係: なし
- 公開APIの変更: なし
- 他クレートへの影響: なし
-->

## Additional Notes
<!-- その他、レビュアーへの情報があれば記載してください -->
