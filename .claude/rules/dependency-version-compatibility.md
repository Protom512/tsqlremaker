# Dependency Version Compatibility

クレート間のバージョン互換性を実装前に確認するルール。

## 原則

**実装開始前に依存クレートのバージョン互換性を必ず確認する**

バージョン不一致は型エラーやリンクエラーの主原因。コードを書く前に5分確認すれば、何時間ものビルド修正サイクルを避けられる。

---

## 必須チェック: 新規クレート追加時

### 1. 依存先クレートの推移的依存を確認

```bash
# Cargo.toml に追加する前に、推移的依存バージョンを確認
cargo tree -p <crate-name> --depth 1

# 例: tower-lsp が lsp-types のどのバージョンを使うか確認
cargo tree -p tower-lsp -i lsp-types
```

### 2. バージョンを揃える

**❌ 禁止: 推移的依存と異なるバージョンを指定**
```toml
# tower-lsp 0.20 は lsp-types 0.94 に依存しているのに
[dependencies]
tower-lsp = "0.20"
lsp-types = "0.97"   # ← バージョン不一致！型が非互換
```

**✅ 推奨: 推移的依存と同じバージョンを使用**
```toml
[dependencies]
tower-lsp = "0.20"
lsp-types = "0.94"   # ← tower-lsp 0.20 と互換
```

### 3. `cargo check` で即座に検証

```bash
# Cargo.toml 編集後、すぐに型チェック
cargo check -p <new-crate>
```

---

## 既知のバージョン互換性情報

### tower-lsp と lsp-types

| tower-lsp | lsp-types | 備考 |
|-----------|-----------|------|
| 0.20 | 0.94 | 安定版。`SemanticTokensResult::Tokens` バリアント |
| 0.20.x | 0.94.x | マイナーアップデートは互換 |

### このプロジェクトのクレート依存関係

```
ase-ls (tower-lsp 0.20, lsp-types 0.94)
  └── ase-ls-core (lsp-types 0.94)
        └── tsql-parser
              └── tsql-lexer (tsql-token)
```

**ルール**: `ase-ls` と `ase-ls-core` は**同じバージョンの lsp-types** を使用すること。

---

## lsp-types 0.94 固有のAPI（他バージョンとの差異）

### SemanticTokensResult

```rust
// lsp-types 0.94
pub enum SemanticTokensResult {
    Tokens(SemanticTokens),      // ← 0.94 では Tokens
    // 0.97 では Ok(Some(SemanticTokens)) になる
}

// ✅ 0.94 でのパターンマッチ
match result {
    SemanticTokensResult::Tokens(tokens) => { /* ... */ }
}

// ❌ 0.97 のパターン（0.94 ではコンパイルエラー）
match result {
    SemanticTokensResult::Ok(Some(tokens)) => { /* ... */ }
}
```

### From 実装の差異

```rust
// 0.94: From<SemanticTokens> のみ実装
let result: SemanticTokensResult = tokens.into();  // OK
let result: SemanticTokensResult = Some(tokens).into();  // ❌ コンパイルエラー

// .into() は SemanticTokens に対して直接呼ぶ
SemanticTokens { result_id: None, data: tokens }.into()
```

### DocumentSymbol.deprecated フィールド

```rust
// 0.94: deprecated フィールドが非推奨（#[deprecated]）
DocumentSymbol {
    // ...
    deprecated: None,   // ← #[allow(deprecated)] が必要
};
```

---

## チェックリスト

新規クレートを `Cargo.toml` に追加する前に:

- [ ] `cargo tree -p <new-crate>` で推移的依存を確認した
- [ ] 既存クレートと共有する依存のバージョンが一致している
- [ ] `cargo check -p <new-crate>` が成功することを確認した
- [ ] バージョン固有のAPI差異を理解している

---

## 関連ルール

- `.claude/rules/architecture-coupling-balance.md` - クレート間の依存方向
- `.claude/rules/pre-commit-rust.md` - コミット前チェック
