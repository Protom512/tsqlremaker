# Workspace Lint Allowances

このプロジェクトの workspace lint 設定と、それに伴う `#[allow]` の使用パターンを定義する。

## Workspace Lint 設定

```toml
# Cargo.toml (workspace root)
[workspace.lints.clippy]
panic = "deny"
expect_used = "deny"
unwrap_used = "deny"
```

これらは**ライブラリコードもテストコードも両方に適用される**。

---

## テストモジュールの標準ヘッダ

テストコードでは panic/unwrap/expect が妥当なため、以下の `#[allow]` を必ず追加する。

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        let result = parse("SELECT * FROM t").unwrap();
        assert_eq!(result.len(), 1);
    }
}
```

### 各 `#[allow]` の理由

| `#[allow]` | 理由 |
|------------|------|
| `clippy::unwrap_used` | テストでは `.unwrap()` がイディオマティック |
| `clippy::panic` | `panic!("Expected X")` はテスト失敗メッセージとして適切 |
| `clippy::expect_used` | テストでは `.expect("msg")` が可読性が高い |

---

## ライブラリコードでの `#[allow]`

### deprecated フィールドへのアクセス

```rust
// lsp-types 0.94 の DocumentSymbol.deprecated は #[deprecated]
#[allow(deprecated)]
fn make_symbol(name: String, kind: SymbolKind, range: Range) -> DocumentSymbol {
    DocumentSymbol {
        name,
        kind,
        range,
        selection_range: range,
        children: None,
        detail: None,
        tags: None,
        deprecated: None,   // ← deprecated フィールド
    }
}
```

### コンパイラ警告の抑制（正当な理由がある場合のみ）

```rust
// 意図的に未使用（将来の拡張用、または trait 実装の要求）
#[allow(dead_code)]
fn placeholder() {}

// インターフェース互換性のため
#[allow(unused_variables)]
fn process(data: &str, _config: &Config) {}
```

---

## ❌ 禁止パターン

```rust
// ❌ ライブラリコードで unwrap を使用（#[allow] なし）
fn parse(input: &str) -> Result<AST> {
    let first = input.chars().next().unwrap();  // deny!
}

// ❌ テスト外で panic を使用
fn process(tokens: Vec<Token>) -> AST {
    if tokens.is_empty() {
        panic!("empty input");  // deny!
    }
}
```

---

## チェックリスト

- [ ] テストモジュールに3つの `#[allow]` を追加した
- [ ] ライブラリコードで `unwrap`/`expect`/`panic` を使用していない
- [ ] `deprecated` フィールドへのアクセスに `#[allow(deprecated)]` を追加した
- [ ] `cargo clippy --all-targets -- -D warnings` がパスする
