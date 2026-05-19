# API Migration Strategy

API移行（旧API → 新API）時の正しい手順を定義する。

## 原則

**deprecatedマークは最後につける — 先行してはならない**

---

## 正しい順序

```
1. 新APIを実装（旧APIと併存）
2. テストで新APIの動作を検証
3. 全呼び出し元を新APIに移行
4. 全テスト + clippy 通過を確認
5. 旧APIに #[deprecated] を付与
6. 一定期間後（次リリース等）に旧APIを削除
```

## 禁止パターン

### 新API作成直後のdeprecatedマーク

**❌ 禁止:**
```rust
// 新しい LineIndex を作った直後に
#[deprecated(note = "use LineIndex instead")]
pub(crate) fn offset_to_position(source: &str, offset: u32) -> (u32, u32) { ... }
```

**理由:** 呼び出し元がまだ旧APIを使っている場合、`-D warnings` でビルドが壊れる。
50件のdeprecated警告が発生し、全呼び出し元の移行が完了するまでビルド不可能になる。

### 移行途中でのコミット

**❌ 禁止:**
```bash
# 移行完了前にコミット → clippyが通らない → CI失敗
git commit -m "feat: add LineIndex and deprecate old functions"
```

**✅ 推奨:**
```bash
# 新APIの実装のみコミット
git commit -m "feat: add LineIndex struct with O(log n) position conversion"

# 呼び出し元の移行を別コミット
git commit -m "refactor: migrate all callers to LineIndex"

# deprecatedマークを最後のコミット
git commit -m "refactor: deprecate old offset_to_position functions"
```

---

## 移行の粒度

### 小規模移行（1-3箇所）

1つのPRで全て完了させる:
1. 新API追加 + テスト
2. 呼び出し元移行
3. deprecatedマーク

### 大規模移行（4箇所以上）

複数コミットに分割するが、**1つのPR内**で完結:
1. コミット1: 新API追加 + テスト
2. コミット2-N: 呼び出し元の移行（モジュールごと）
3. 最終コミット: deprecatedマーク

**重要**: PRの完了時点でclippyが通過すること。

---

## チェックリスト

API移行を開始する前に:

- [ ] 新APIの設計が完了している
- [ ] 新APIのテストが存在する
- [ ] 旧APIの全呼び出し元を特定した（`grep` または `LSP findReferences`）
- [ ] 移行計画が定まっている（1PR / 複数コミット）

API移行を完了する前に:

- [ ] 全呼び出し元が新APIに移行済み
- [ ] `cargo clippy --all-targets -- -D warnings` が通過
- [ ] `cargo nextest run --workspace` が通過
- [ ] 旧APIの `#[deprecated]` メッセージが新APIへの誘導を含む
