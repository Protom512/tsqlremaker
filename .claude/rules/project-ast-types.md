# Project AST Types Reference

このプロジェクト固有のAST・Parser・Lexer型の実態をドキュメント化する。
外部 crate の API を使う際、**必ず実際のソースコードを確認**し、このリファレンスと照合すること。

---

## Parser (`tsql-parser`)

### 公開API

```rust
// crates/tsql-parser/src/lib.rs
pub struct Parser<'a> { /* ... */ }

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self;
    pub fn parse(&mut self) -> Result<Vec<Statement>, ParseError>;
    pub fn parse_one(&mut self) -> Result<Statement, ParseError>;
    pub fn parse_with_errors(&mut self) -> Result<(Vec<Statement>, Vec<ParseError>), ParseErrors>;
}
```

### Statement 列挙型（19 variants）

```rust
// crates/tsql-parser/src/ast/mod.rs
pub enum Statement {
    Select(Box<SelectStatement>),
    Insert(Box<InsertStatement>),
    Update(Box<UpdateStatement>),
    Delete(Box<DeleteStatement>),
    Create(Box<CreateStatement>),
    Declare(Box<DeclareStatement>),
    Set(Box<SetStatement>),
    VariableAssignment(Box<VariableAssignment>),
    If(Box<IfStatement>),
    While(Box<WhileStatement>),
    Block(Box<Block>),           // BEGIN...END
    Break(Box<BreakStatement>),
    Continue(Box<ContinueStatement>),
    Return(Box<ReturnStatement>),
    TryCatch(Box<TryCatchStatement>),
    Transaction(TransactionStatement),
    Throw(Box<ThrowStatement>),
    Raiserror(Box<RaiserrorStatement>),
    BatchSeparator(BatchSeparator),  // GO
}
```

**注意**: `Create`, `Delete`, `Block` 等は `Box<T>` で包まれている。

### CreateStatement（4 variants, Trigger なし）

```rust
// crates/tsql-parser/src/ast/ddl.rs
pub enum CreateStatement {
    Table(TableDefinition),
    Index(IndexDefinition),
    View(ViewDefinition),
    Procedure(ProcedureDefinition),
    // ⚠️ Trigger は存在しない
}
```

### Identifier（name は String, Display は未実装）

```rust
// crates/tsql-parser/src/ast/expression.rs
pub struct Identifier {
    pub name: String,    // ← String, &str ではない
    pub span: Span,
}
```

**使用パターン**:
```rust
// ✅ name フィールドに直接アクセス
let name: String = identifier.name.clone();
let name_ref: &str = &identifier.name;

// ❌ Display は実装されていない可能性がある
let name = format!("{}", identifier);  // ← 確認が必要
```

### TableReference（3 variants）

```rust
// crates/tsql-parser/src/ast/select.rs
pub enum TableReference {
    Table {
        name: Identifier,
        alias: Option<Identifier>,
        span: Span,
    },
    Subquery {
        query: Box<SelectStatement>,
        alias: Option<Identifier>,
        span: Span,
    },
    Joined {
        joins: Vec<Join>,
        span: Span,
    },
}
```

**注意**: `Joined` バリアントを忘れないこと。wildcard match `_ => {}` で処理するか、3つとも明示的に処理すること。

### InsertStatement

```rust
pub struct InsertStatement {
    pub span: Span,
    pub table: Identifier,         // ← Identifier, TableReference ではない
    pub columns: Vec<Identifier>,
    pub source: InsertSource,
}
```

### UpdateStatement

```rust
pub struct UpdateStatement {
    pub span: Span,
    pub table: TableReference,     // ← TableReference, Identifier ではない
    // ...
}
```

### DeleteStatement

```rust
pub struct DeleteStatement {
    pub span: Span,
    pub table: Identifier,         // ← Identifier, TableReference ではない
    // ...
}
```

---

## Parser Error

### ParseError

```rust
// crates/tsql-parser/src/error.rs
pub enum ParseError {
    UnexpectedToken { expected: Vec<TokenKind>, found: TokenKind, position: Position },
    UnexpectedEof { expected: String, position: Position },
    InvalidSyntax { message: String, position: Position },
    RecursionLimitExceeded { limit: usize, position: Position },
    BatchError { batch_number: usize, error: Box<ParseError> },
}
```

**重要メソッド**:
```rust
impl ParseError {
    pub fn span(&self) -> Option<Span>;     // ← Option<Span>, Span ではない
    pub fn position(&self) -> Position;      // ← 常に有効
}

impl fmt::Display for ParseError;            // format!("{error}") で使用可能
```

### ParseErrors（複数エラー）

```rust
pub struct ParseErrors {
    pub errors: Vec<ParseError>,
}
```

---

## Lexer (`tsql-lexer`)

### 公開API

```rust
// crates/tsql-lexer/src/lib.rs
pub use lexer::{Lexer, Token};
pub use tsql_token::{Position, Span, TokenKind};
```

### Token（ゼロコピー）

```rust
// crates/tsql-lexer/src/lexer.rs
pub struct Token<'src> {
    pub kind: TokenKind,
    pub text: &'src str,    // ← &str (借用), String ではない
    pub span: Span,
}
```

### Lexer の重要メソッド

```rust
impl<'a> Lexer<'a> {
    pub fn new(source: &str) -> Self;
    pub fn with_comments(self, preserve: bool) -> Self;  // ← 重要！
}
```

**⚠️ `with_comments()` のデフォルトは `false`**:
```rust
// ❌ ブロックコメントがスキップされる
let lexer = Lexer::new(source);

// ✅ ブロックコメントを保持する場合
let lexer = Lexer::new(source).with_comments(true);
```

ブロックコメント（`/* ... */`）を処理する必要がある機能（フォールディング等）では、**必ず `with_comments(true)` を使用**すること。

---

## Token Types (`tsql-token`)

### Span と Position

```rust
// crates/tsql-token/src/position.rs
pub struct Span {
    pub start: u32,    // ← バイトオフセット, 行/列ではない
    pub end: u32,
}

pub struct Position {
    pub line: u32,     // ← 1-indexed
    pub column: u32,   // ← 1-indexed
    pub offset: u32,   // ← バイトオフセット
}
```

### TokenKind（主要なもの）

100以上のバリアントが存在。主なカテゴリ:
- キーワード: `Select`, `From`, `Where`, `Insert`, `Update`, `Delete`, `Create`, ...
- データ型: `Int`, `Varchar`, `Datetime`, `Unichar`, `Bigdatetime`, ...
- リテラル: `Number`, `FloatLiteral`, `String`, `NString`, `HexString`
- コメント: `LineComment`, `BlockComment`
- 変数: `LocalVar`, `GlobalVar`
- 一時テーブル: `TempTable`, `GlobalTempTable`
- 演算子: `Eq`, `Ne`, `Lt`, `Gt`, `Plus`, `Minus`, ...

**判定メソッド**:
```rust
impl TokenKind {
    pub fn is_keyword(&self) -> bool;
}
```

---

## モジュール可視性の注意点

```rust
// ✅ 正しい: ast モジュールから re-export されている
use tsql_parser::ast::TableReference;
use tsql_parser::ast::Identifier;
use tsql_parser::ast::CreateStatement;

// ❌ エラー: サブモジュールは private
use tsql_parser::ast::select::TableReference;   // private!
use tsql_parser::ast::expression::Identifier;    // private!
```

**ルール**: `tsql_parser::ast::*` 経由でインポートすること。サブモジュール（`ast::select`, `ast::expression`等）はprivate。

---

## Parser 未対応構文（2026-04-17 現在）

以下のSQL構文はParserが対応しておらず、パースエラーになる。LSP機能の設計時に考慮すること。

### DDL 未対応

| 構文 | 影響 | ワークアラウンド |
|------|------|-----------------|
| `CREATE UNIQUE INDEX` | インデックス定義がパースエラー | `CREATE INDEX` として処理（UNIQUE は無視） |
| `ALTER TABLE` | テーブル変更がパースエラー | 対象外として graceful に処理 |
| `CREATE TRIGGER` | トリガー定義がパースエラー | CreateStatement に Trigger variant なし |
| `CREATE DEFAULT` | ASE固有オブジェクトがパースエラー | 対象外 |
| `CREATE RULE` | ASE固有オブジェクトがパースエラー | 対象外 |
| `GRANT` / `REVOKE` | 権限制御がパースエラー | 対象外 |

### DML/DCL 未対応

| 構文 | 影響 | ワークアラウンド |
|------|------|-----------------|
| `EXEC` / `EXECUTE` | プロシージャ呼び出しがパースエラー | トークンレベルで処理（シンボルテーブル不使用） |
| `DISK INIT` | ASEディスク管理がパースエラー | 対象外 |
| `LOAD DATABASE` / `DUMP DATABASE` | バックアップ系がパースエラー | 対象外 |

### LSP機能への影響

- **Diagnostics**: 未対応構文はパースエラーとして報告される（現状許容）
- **Symbol Table**: `build_tolerant()` で未対応構文を含むバッチはスキップ
- **Definition/References**: 未対応構文内の識別子は追跡不可
- **Code Actions**: 未対応構文に対するQuick Fixは生成不可
- **Hover**: トークンレベルのHoverは機能する（パース不要のため）

### 今後の拡張優先度

1. `CREATE UNIQUE INDEX` — 使用頻度高、対応コスト低
2. `ALTER TABLE` — DDL開発で頻出
3. `EXEC` / `EXECUTE` — プロシージャ呼び出しの追跡に必須
4. `CREATE TRIGGER` — ASE開発で一般的

---

## チェックリスト

外部 crate の型を使用する際:

- [ ] `pub` フィールドの実際の型をソースコードで確認した
- [ ] `enum` の全バリアントを把握した（推測で書かない）
- [ ] `Option<T>` と `T` の違いを確認した
- [ ] `Box<T>` と `T` の違いを確認した
- [ ] `Display` トレイトが実装されているか確認した
- [ ] モジュールの可視性を確認した（private サブモジュールにアクセスしていないか）
- [ ] 対象SQL構文がParser対応済みか確認した（未対応構文リストを参照）
