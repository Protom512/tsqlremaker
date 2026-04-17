# Pre-Implementation Checklist

実装開始前に必ず実施するチェックリスト。
「コードを書く前に5分確認すれば、何時間もの修正サイクルを避けられる」

---

## 1. 依存関係の確認

### 新規依存の追加時

```bash
# 推移的依存バージョンの確認
cargo tree -p <crate-name> --depth 1

# 既存依存とのバージョン整合性確認
cargo tree -p <crate-name> -i <existing-dep>
```

- [ ] 追加する依存が既存の推移的依存とバージョン競合しない
- [ ] `cargo check -p <target-crate>` が通ることを確認

### 既知のバージョン制約

| 依存ペア | 互換バージョン |
|---------|-------------|
| tower-lsp + lsp-types | 0.20 + 0.94 (NOT 0.97) |

---

## 2. API署名の確認

実装で使用する型・メソッドの**実際の定義**をソースコードで確認する。推測で書かない。

### 確認すべき項目

- [ ] 構造体のフィールド名と型（`pub` フィールドのみアクセス可能）
- [ ] メソッドの戻り値の型（`Option<T>` vs `T`, `Result<T, E>` vs `T`）
- [ ] enumの全バリアント（wildcard `_` で逃げない）
- [ ] トレイトの実装有無（`Display`, `From`, `Into` 等）
- [ ] モジュールの可視性（`pub` vs private）

### 確認方法

```bash
# ソースコードを直接確認
cat crates/tsql-parser/src/ast/mod.rs

# grep で型定義を検索
rg "pub struct Identifier" crates/
rg "pub enum Statement" crates/
```

### よくある罠

| 罠 | 実際 | よくある誤解 |
|----|------|------------|
| `ParseError::span()` | `Option<Span>` | `Span` |
| `Identifier` の表示 | `.name.clone()` | `format!("{}", id)` |
| `parse_with_errors()` | `Err` を返す | 部分結果を返す（誤） |
| `Identifier.name` | `String` | `&str` |

---

## 3. Parser能力の確認

実装する機能が対象とするSQL構文が、Parserで**実際にパース可能か**を確認する。

### 現在のParser未対応構文

| 構文 | 状態 | 代替アプローチ |
|------|------|-------------|
| `CREATE UNIQUE INDEX` | 未対応 | `CREATE INDEX` のみ対応 |
| `ALTER TABLE` | 未対応 | 対象外として処理 |
| `CREATE TRIGGER` | 未対応 | 対象外として処理 |
| `GRANT` / `REVOKE` | 未対応 | 対象外として処理 |
| `EXEC` / `EXECUTE` | 未対応 | トークンレベルで処理 |

### 確認方法

```bash
# テストで実際にパースを試行
cargo test -p tsql-parser -- <pattern>

# 対話的確認
cargo run --example parse_check "CREATE UNIQUE INDEX idx ON t (c)"
```

---

## 4. LSP型のバージョン固有API確認

lsp-types 0.94 は他バージョンとAPIが異なる。実装前に確認すること。

### 確認すべきLSP型

```bash
# 使用するLSP型の定義を確認
rg "pub struct RenameParams" -A 5 ~/.cargo/registry/src/*/lsp-types-0.94.*/
rg "pub enum SemanticTokensResult" -A 5 ~/.cargo/registry/src/*/lsp-types-0.94.*/
```

### 0.94 固有の差異（抜粋）

| 型 | 0.94 | 0.97+ |
|----|------|-------|
| `RenameParams` | `.text_document_position` | `.text_document_position_params` |
| `SemanticTokensResult` | `::Tokens(...)` | `::Ok(Some(...))` |
| `symbol()` 戻り値 | `Vec<SymbolInformation>` | `WorkspaceSymbolResponse` |
| `DocumentSymbol.deprecated` | `#[deprecated]` フィールド | なし |

---

## 5. テスト計画

実装前にテストシナリオを定義する。

### テストシナリオテンプレート

```
機能: [機能名]

正常系:
1. [入力] → [期待出力]
2. [入力] → [期待出力]

エッジケース:
3. [空入力] → [None/空]
4. [不正入力] → [None/空]

エラー系:
5. [パース失敗するSQL] → [gracefulな処理]
```

### テスト数の目安

| モジュール規模 | テスト数目安 |
|-------------|-----------|
| 小（<100行） | 5件以上 |
| 中（100-300行） | 8件以上 |
| 大（300行+） | 10件以上 |

---

## チェックリスト（サマリー）

実装開始前に全てにチェックを入れること：

- [ ] 依存関係のバージョン互換性を確認した
- [ ] 使用する型/メソッドの実際の署名をソースで確認した
- [ ] Parserが対象SQL構文に対応していることを確認した
- [ ] lsp-types 0.94 のバージョン固有APIを確認した
- [ ] テストシナリオ（正常系＋エッジケース）を定義した
- [ ] 影響を受ける既存モジュールを特定した
