# TDD Anti-Coupling Rules

TDD実践時にテストと実装が過剰に結合するのを防ぐルールを定義します。

## 原則

**テストは「振る舞い」を検証し、「実装の詳細」には依存してはならない**

---

## 禁止パターン

### 1. プライベートメソッド/フィールドのテスト禁止

**❌ 禁止:**
```rust
// テストコード
#[test]
fn test_private_method() {
    let lexer = Lexer::new(sql);
    // プライベートメソッドを直接テスト
    let result = lexer.read_char();  // read_char は private
    assert_eq!(result, 'S');
}
```

**理由:**
- リファクタリング時にテストが壊れる
- 実装詳細を変更できなくなる

**✅ 推奨:**
```rust
#[test]
fn test_tokenizes_keyword() {
    let mut lexer = Lexer::new("SELECT");
    let token = lexer.next_token().unwrap();

    // 公開APIを通じて振る舞いを検証
    assert_eq!(token.kind, TokenKind::SELECT);
    assert_eq!(token.literal, "SELECT");
}
```

---

### 2. 内部状態の直接検証禁止

**❌ 禁止:**
```rust
#[test]
fn test_internal_state() {
    let lexer = Lexer::new("SELECT");
    // 内部カーソル位置を検証
    assert_eq!(lexer.cursor_position, 6);  // cursor_position は private
    assert_eq!(lexer.tokens.len(), 1);      // tokens は private
}
```

**✅ 推奨:**
```rust
#[test]
fn test_consumes_all_input() {
    let lexer = Lexer::new("SELECT FROM");
    let tokens: Vec<_> = lexer.collect();

    // 結果として出力されるトークンで検証
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens.last().unwrap().kind, TokenKind::FROM);
}
```

---

### 3. 実装順序への依存禁止

**❌ 禁止:**
```rust
#[test]
fn test_tokenization_order() {
    let lexer = Lexer::new("SELECT * FROM users");
    let tokens: Vec<_> = lexer.collect();

    // トークンの順序に厳密に依存（実際には重要ではない場合も）
    assert_eq!(tokens[0].kind, TokenKind::SELECT);
    assert_eq!(tokens[1].literal, "*");  // 位置固定
    assert_eq!(tokens[2].kind, TokenKind::FROM);
    assert_eq!(tokens[3].literal, "users");
}
```

**問題:**
- 中間に空白トークンが増えただけでテストが壊れる

**✅ 推奨:**
```rust
#[test]
fn test_contains_expected_tokens() {
    let lexer = Lexer::new("SELECT * FROM users");
    let tokens: Vec<_> = lexer.collect();

    // 期待するトークンが含まれるか検証
    let kinds: Vec<_> = tokens.iter().map(|t| &t.kind).collect();

    assert!(kinds.contains(&&TokenKind::SELECT));
    assert!(kinds.contains(&&TokenKind::STAR));
    assert!(kinds.contains(&&TokenKind::FROM));
}
```

---

### 4. モックの過剰使用禁止

**❌ 禁止:**
```rust
#[test]
fn test_with_mock() {
    // 実装時にモック要件が決まっていないのに
    let mock_source = MockTokenSource::new();
    mock_source.expect_next()
        .return_once(|| Ok(Token::ident("foo")));

    let parser = Parser::new(mock_source);
    let result = parser.parse_ident().unwrap();

    assert_eq!(result.literal, "foo");
}
```

**問題:**
- インターフェースがテストのために作られている
- 実際の使用ではモックが不要

**✅ 推奨:**
```rust
#[test]
fn test_parse_identifier() {
    // 実際の Lexer を使用
    let input = "SELECT foo FROM bar";
    let mut parser = Parser::new(Lexer::new(input));

    let stmt = parser.parse_statement().unwrap();

    // パース結果を検証
    assert!(matches!(stmt, Statement::Select { .. }));
}
```

---

## 許容される結合

### 1. 公開APIへの結合

```rust
// ✅ 良い: 公開メソッドをテスト
#[test]
fn test_next_token_returns_eof() {
    let mut lexer = Lexer::new("");
    let token = lexer.next_token().unwrap();
    assert_eq!(token.kind, TokenKind::EOF);
}
```

### 2. 公開型への結合

```rust
// ✅ 良い: 公開された構造体をテスト
#[test]
fn test_token_span() {
    let token = Lexer::new("SELECT")
        .next_token()
        .unwrap();

    assert_eq!(token.span.start.line, 1);
    assert_eq!(token.span.start.column, 1);
}
```

### 3. 契約による結合

```rust
// ✅ 良い: trait で定義された契約をテスト
#[test]
fn test_iterator_contract() {
    let lexer = Lexer::new("SELECT FROM");

    // Iterator という契約を満たすか検証
    let count = lexer.count();
    assert_eq!(count, 2);
}
```

---

## テスト名のガイドライン

### 振る舞いを表す名前

| 禁止（実装詳細） | 推奨（振る舞い） |
|-----------------|----------------|
| `test_calls_read_char_3_times` | `test_tokenizes_keyword` |
| `test_sets_cursor_to_position_5` | `test_handles_offset_correctly` |
| `test_pushes_to_tokens_vector` | `test_emits_expected_tokens` |
| `test_returns_none_when_empty` | `test_handles_empty_input` |

```rust
// ❌ 禁止: 実装詳細を表す
#[test]
fn test_uses_hashmap_for_lookup() {

// ✅ 推奨: 振る舞いを表す
#[test]
fn test_resolves_keyword_correctly() {
```

---

## テストの脆弱性チェックリスト

テストが以下の質問に「はい」と答える場合、過剰結合しています：

- [ ] 実装をリファクタリング（メソッド名変更、分割等）しただけでテストが壊れるか？
- [ ] プライベートメソッド/フィールドを直接参照しているか？
- [ ] 特定のデータ構造（Vec、HashMap等）に依存しているか？
- [ ] 内部状態の順序に依存しているか？
- [ ] テストのために公開されていないインターフェースを使用しているか？

---

## TDDサイクルでの適用

### Red: テストを書くとき

```rust
// 1. まず「やりたいこと」をテストで表現
#[test]
fn test_tokenizes_select_keyword() {
    let input = "SELECT";
    let mut lexer = Lexer::new(input);

    let token = lexer.next_token().unwrap();

    // 期待する結果（振る舞い）を記述
    assert_eq!(token.kind, TokenKind::SELECT);
}

// 2. 実装の詳細を書かない
// ❌ だめ: assert_eq!(lexer.internal_index, 6);
```

### Green: 実装を書くとき

```rust
// テストを通すための最小実装
impl Lexer {
    pub fn next_token(&mut self) -> ParseResult<Token> {
        // 内部実装は自由に変更可能
        // テストは振る舞いのみを見ている
    }
}
```

### Refactor: リファクタリングするとき

```rust
// 内部メソッドを分割・再構成しても
// テストは壊れないはず
fn read_char(&mut self) -> char { /* ... */ }  // private メソッド
fn skip_whitespace(&mut self) { /* ... */ }    // 追加してもOK
```

---

## Black Box テストの原則

テストは実装を「ブラックボックス」として扱う：

```
┌─────────────────────────────────────┐
│           実装（ブラックボックス）      │
│  ┌─────────┐    ┌─────────────────┐  │
│  │ private │    │  private state  │  │
│  │ methods │    │                 │  │
│  └─────────┘    └─────────────────┘  │
└─────────────────────────────────────┘
           ▲                │
           │                ▼
      ┌────┴────┐      ┌───┴───┐
      │  Input  │      │ Output │
      └─────────┘      └────────┘
       (テスト可能)      (テスト可能)
```

### テスト可能なもの

- ✅ 公開メソッドへの入力
- ✅ 公開メソッドからの出力
- ✅ 公開されたエラー型

### テスト不可なもの

- ❌ プライベートメソッド
- ❌ 内部状態
- ❌ 実行順序（結果に影響しない場合）

---

## 具体例: Lexer のテスト

### 悪い例

```rust
#[test]
fn test_lexer() {
    let mut lexer = Lexer::new("SELECT");

    // 内部実装をテスト
    assert_eq!(lexer.cursor, 0);
    lexer.read_char();
    assert_eq!(lexer.cursor, 1);
    assert_eq!(lexer.current_char, 'S');

    lexer.read_identifier();
    assert_eq!(lexer.tokens.len(), 1);
}
```

### 良い例

```rust
#[test]
fn test_tokenizes_select_keyword() {
    let tokens: Vec<_> = Lexer::new("SELECT").collect();

    assert_eq!(tokens.len(), 2);  // SELECT + EOF
    assert_eq!(tokens[0].kind, TokenKind::SELECT);
    assert_eq!(tokens[0].literal, "SELECT");
    assert_eq!(tokens[1].kind, TokenKind::EOF);
}
```

---

## 例外: テストダブルを使用するケース

以下の場合はモック/スタブの使用が許容されます：

### 1. 外部依存の分離

```rust
// ✅ 許容: 外部サービスをモック
trait Filesystem {
    fn read(&self, path: &str) -> Result<String, Error>;
}

struct TestFs {
    contents: HashMap<String, String>,
}

impl Filesystem for TestFs {
    fn read(&self, path: &str) -> Result<String, Error> {
        self.contents.get(path)
            .cloned()
            .ok_or_else(|| Error::NotFound(path.to_string()))
    }
}
```

### 2. 非決定論的動作の固定

```rust
// ✅ 許容: 日時・乱数を固定
struct DeterministicClock {
    fixed_time: SystemTime,
}

impl Clock for DeterministicClock {
    fn now(&self) -> SystemTime {
        self.fixed_time
    }
}
```

---

## チェックリスト: コミット前

- [ ] 全てのテストが公開APIのみを使用している
- [ ] プライベートメソッドを直接テストしていない
- [ ] 内部状態を直接検証していない
- [ ] テスト名が振る舞いを表している
- [ ] モックが必要以上に使われていない

---

## Clippy Lint による自動検出

```toml
# Cargo.toml 或は .clippy.toml
[lints.clippy]
# プライベート型をテストから見えるようにしていないか
# （必要な場合のみ pub visible_for_testing を使用）
```

### `#[cfg(test)]` 内でのアクセス

必要な場合のみ `pub(crate)` または `pub` を使用：

```rust
// 通常は private
struct Lexer {
    tokens: Vec<Token>,  // private
}

// テストのために公開するのは最後の手段
#[cfg(test)]
impl Lexer {
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}
```

---

## 参考

- Growing Object-Oriented Software, Guided by Tests (GOOS)
- Working Effectively with Legacy Code
- Test-Driven Development by Example
