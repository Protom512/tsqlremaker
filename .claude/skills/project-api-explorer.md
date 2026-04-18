# Project API Explorer Skill

プロジェクト内の crate API を素早く確認するための探索スキル。

## トリガー

- 新しいクレートの実装を始める時
- 外部 crate の型を使用する時
- "api explorer", "型確認", "API 確認"
- コンパイルエラーの原因が型不一致の場合

---

## 目的

**推測でコードを書かない**。実際のソースコードを確認してから実装する。

autopilotセッションで最も多かったペインポイントは「型を推測して書いた結果のコンパイルエラー」だった。このスキルは実装前の型確認を強制する。

---

## 実行手順

### Step 1: 使用する型のソースファイルを特定

```bash
# Glob でファイルを見つける
Glob: crates/tsql-parser/src/ast/*.rs
Glob: crates/tsql-lexer/src/*.rs
Glob: crates/tsql-token/src/*.rs
```

### Step 2: 型定義を実際に読む

```
Read: crates/tsql-parser/src/ast/expression.rs
Read: crates/tsql-parser/src/ast/select.rs
Read: crates/tsql-parser/src/ast/ddl.rs
Read: crates/tsql-parser/src/error.rs
```

### Step 3: 確認すべきポイント

確認項目:
1. **フィールドの型** - `String` か `&str` か、`T` か `Box<T>` か
2. **enum の全バリアント** - 漏れがないか
3. **Option の有無** - `Option<Identifier>` か `Identifier` か
4. **トレイト実装** - `Display`, `From`, `Iterator` 等
5. **可視性** - `pub` か private か
6. **デフォルト値** - `preserve_comments: false` 等

### Step 4: 確認結果をメモしてから実装

---

## クイックリファレンス

### Parser API の場所

| 型 | ファイル |
|----|---------|
| `Parser` | `crates/tsql-parser/src/lib.rs` |
| `Statement` | `crates/tsql-parser/src/ast/mod.rs` |
| `CreateStatement` | `crates/tsql-parser/src/ast/ddl.rs` |
| `SelectStatement` | `crates/tsql-parser/src/ast/select.rs` |
| `TableReference` | `crates/tsql-parser/src/ast/select.rs` |
| `Expression` | `crates/tsql-parser/src/ast/expression.rs` |
| `Identifier` | `crates/tsql-parser/src/ast/expression.rs` |
| `ParseError` | `crates/tsql-parser/src/error.rs` |

### Lexer API の場所

| 型 | ファイル |
|----|---------|
| `Lexer` | `crates/tsql-lexer/src/lexer.rs` |
| `Token<'src>` | `crates/tsql-lexer/src/lexer.rs` |
| `LexError` | `crates/tsql-lexer/src/error.rs` |

### Token API の場所

| 型 | ファイル |
|----|---------|
| `TokenKind` | `crates/tsql-token/src/kind.rs` |
| `Span` | `crates/tsql-token/src/position.rs` |
| `Position` | `crates/tsql-token/src/position.rs` |

---

## リファレンスファイル

プロジェクトのAST型リファレンスは `.claude/rules/project-ast-types.md` に常時更新される。
新しい型を発見したら、そちらも更新すること。
