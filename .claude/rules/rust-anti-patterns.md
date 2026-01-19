# Rust Anti-Patterns Rules

Rust コードで避けるべきアンチパターンと、推奨される代替手法を定義します。

## 原則

**ライブラリコードでは panic を起こさない**

panic はプログラムを強制終了させるため、ライブラリコード（パーサー、Lexer等）では使用してはいけません。代わりに `Result` 型でエラーを返してください。

---

## 禁止パターン

### 1. `.unwrap()` の禁止

**❌ 禁止:**
```rust
let token = self.tokens.next().unwrap();
let value = map.get("key").unwrap();
```

**✅ 推奨:**
```rust
// エラーを伝播
let token = self.tokens.next().ok_or(ParseError::UnexpectedEOF)?;

// カスタムエラー
let value = map.get("key")
    .ok_or_else(|| ParseError::MissingKey("key".to_string()))?;

// デフォルト値
let value = map.get("key").unwrap_or(&default);
```

**例外（許容されるケース）:**
- テストコード（`#[cfg(test)]` 内）
- アサーション（`assert!()` はテスト用途のみ）

---

### 2. `.expect()` の禁止

**❌ 禁止:**
```rust
let token = self.tokens.next().expect("token should exist");
let value = map.get("key").expect("key should exist");
```

**✅ 推奨:**
```rust
let token = self.tokens.next()
    .ok_or(ParseError::UnexpectedEOF {
        expected: "token".to_string(),
        span: self.current_span(),
    })?;
```

**理由:** `expect()` は panic するため、エラーメッセージをカスタマイズできない。`Result` ならエラー情報を構造化できる。

---

### 3. `.panic!()` / `.unreachable!()` の禁止

**❌ 禁止:**
```rust
if self.ch == '*' {
    self.read_char();
} else {
    panic!("Expected * character");
}

match token_type {
    TokenType::Number => { /* ... */ }
    _ => panic!("Unexpected token type"),
}
```

**✅ 推奨:**
```rust
if self.ch == '*' {
    self.read_char();
} else {
    return Err(LexError::UnexpectedCharacter {
        expected: '*',
        found: self.ch,
        position: self.position,
    });
}

match token_type {
    TokenType::Number => { /* ... */ }
    _ => Err(ParseError::UnexpectedTokenType {
        expected: vec![TokenType::Number],
        found: token_type,
    })?,
}
```

---

### 4. `.unwrap_*()` 系メソッドの禁止

**❌ 禁止:**
```rust
let s = String::from_utf8(bytes).unwrap();
let num = s.parse::<i64>().unwrap();
let ptr = Box::leak(boxed_value); // メモリリークの恐れ
```

**✅ 推奨:**
```rust
let s = String::from_utf8(bytes)
    .map_err(|e| ParseError::InvalidUtf8 { bytes: e.into_bytes() })?;

let num = s.parse::<i64>()
    .map_err(|_| ParseError::InvalidNumber { input: s })?;
```

---

### 5. インデックスアクセスの禁止（パニックの可能性）

**❌ 禁止:**
```rust
let first = tokens[0];  // パニックの可能性
let last = tokens[tokens.len() - 1];  // len() == 0 でパニック
```

**✅ 推奨:**
```rust
let first = tokens.first()
    .ok_or(ParseError::EmptyTokenList)?;

let last = tokens.last()
    .ok_or(ParseError::EmptyTokenList)?;

// .get() は Option を返す
let item = tokens.get(index)
    .ok_or(ParseError::IndexOutOfBounds { index, max: tokens.len() })?;
```

---

## エラーハンドリングパターン

### ? 演算子の使用

```rust
// 良い: エラーを伝播
fn parse_statement(&mut self) -> ParseResult<Statement> {
    self.expect_keyword(Keyword::SELECT)?;  // エラーなら即座にreturn
    let columns = self.parse_columns()?;
    // ...
}

// 良い: エラーハンドリングとロジックを分離
fn parse_columns(&mut self) -> ParseResult<Vec<Expression>> {
    let mut columns = Vec::new();

    loop {
        columns.push(self.parse_expression()?);

        if !self.match_token(TokenKind::Comma) {
            break;
        }
    }

    Ok(columns)
}
```

### カスタムエラー型の定義

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    UnexpectedCharacter {
        expected: char,
        found: char,
        position: Position,
    },
    UnterminatedString {
        start: Position,
    },
    UnterminatedComment {
        start: Position,
    },
    InvalidUtf8 {
        bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedToken {
        expected: Vec<TokenKind>,
        found: TokenKind,
        span: Span,
    },
    UnexpectedEOF {
        expected: String,
    },
    InvalidSyntax {
        message: String,
        span: Span,
    },
}

// std::error::Error を実装
impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedCharacter { expected, found, position } => {
                write!(f, "{}:{}: expected '{}', found '{}'",
                    position.line, position.column, expected, found)
            }
            // ...
        }
    }
}

impl std::error::Error for LexError {}
```

---

## 組み込みメソッドの安全な代替

| 禁止（パニックの可能性） | 推奨（安全） |
|------------------------|-------------|
| `vec[i]` | `vec.get(i)` |
| `vec.first()` (Optionを返すのでOK) | - |
| `Option::unwrap()` | `Option::ok_or_else(\|\| Error)?` |
| `Result::unwrap()` | `Result?` |
| `String::from_utf8(vec).unwrap()` | `String::from_utf8(vec)?` |
| `slice[1..]` (空でパニック) | `slice.get(1..)` |
| `str.parse::<T>().unwrap()` | `str.parse::<T>()?` |

---

## バリデーション

### 事前条件のチェック

```rust
// ❌ 禁止
fn divide(a: i64, b: i64) -> i64 {
    if b == 0 {
        panic!("Division by zero");
    }
    a / b
}

// ✅ 推奨
fn divide(a: i64, b: i64) -> Result<i64, MathError> {
    if b == 0 {
        return Err(MathError::DivisionByZero);
    }
    Ok(a / b)
}
```

---

## テストコードでの例外

テストコード（`#[cfg(test)]` 内）では、以下は許可されます：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unwrap_in_tests() {
        // テスト内では unwrap は許可
        let token = tokenize("SELECT").unwrap();
        assert_eq!(token.kind, TokenKind::SELECT);
    }

    #[test]
    #[should_panic(expected = "Division by zero")]
    fn test_panic() {
        // panic テスト
        let _ = divide(1, 0).unwrap();
    }
}
```

---

## チェックリスト

コードを書く際、以下を確認してください：

- [ ] `.unwrap()` を使用していない
- [ ] `.expect()` を使用していない
- [ ] `panic!()` を使用していない
- [ ] `unreachable!()` を使用していない
- [ ] インデックスアクセス `vec[i]` ではなく `vec.get(i)` を使用している
- [ ] 全てのエラーが `Result` 型で返されている
- [ ] エラー型に `std::error::Error` が実装されている
- [ ] エラーメッセージに位置情報（行、列）が含まれている

---

## 静的解析ツール

以下のツールを導入して、自動検出することを推奨します：

### Clippy Lints

```toml
# Cargo.toml
[lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
unimplemented = "deny"
todo = "warn"
indexing_slicing = "deny"
```

またはコマンドライン：

```bash
cargo clippy -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic
```

### pre-commit フック

```bash
#!/bin/sh
# .git/hooks/pre-commit

# unwrap/expect/panic のチェック
if cargo clippy -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic 2>&1 | grep -q "unwrap_used\|expect_used\|panic"; then
    echo "Error: unwrap, expect, or panic detected. Use Result instead."
    exit 1
fi
```

---

## 違反時の対応

ルール違反が見つかった場合：

1. **即座に修正**: unwrap/expect/panic を Result に置き換え
2. **エラー型の追加**: 必要に応じて新しいエラー variant を追加
3. **テストの更新**: テストもエラーケースを検証するように変更

---

## 例: 変換前後

### Before (禁止パターン)

```rust
fn parse_number(&mut self) -> Token {
    let start = self.position;

    while self.ch.map_or(false, |c| c.is_numeric()) {
        self.read_char();
    }

    let literal = &self.input[start..self.position];  // パニック可能性
    Token {
        kind: TokenKind::NUMBER,
        literal: literal.to_string(),
    }
}
```

### After (推奨パターン)

```rust
fn parse_number(&mut self) -> ParseResult<Token> {
    let start = self.position;

    while self.ch.map_or(false, |c| c.is_numeric()) {
        self.read_char();
    }

    let literal = self.input.get(start..self.position)
        .ok_or_else(|| ParseError::InvalidSlice {
            start,
            end: self.position,
        })?;

    Ok(Token {
        kind: TokenKind::NUMBER,
        literal: literal.to_string(),
    })
}
```
