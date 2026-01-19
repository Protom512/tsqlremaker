# Rust Parser/Lexer Development Guide

Comprehensive guide for implementing lexers and parsers in Rust, specifically for SQL dialects.

## Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│  Source String  │───▶│     Lexer       │───▶│  Token Stream   │
│                 │    │                 │    │  (Iterator)     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                                      │
                                                      ▼
                                              ┌─────────────────┐
                                              │     Parser      │
                                              │                 │
                                              └─────────────────┘
                                                      │
                                                      ▼
                                              ┌─────────────────┐
                                              │      AST        │
                                              │  (Typed nodes)  │
                                              └─────────────────┘
```

## Token Design

### TokenKind Enum (Preferred)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Keywords
    Select, From, Where, Insert, Update, Delete, Create,
    // Operators
    Plus, Minus, Asterisk, Slash, Percent,
    Eq, Ne, Lt, Gt, Le, Ge,
    // Delimiters
    LParen, RParen, Comma, Semicolon, Dot,
    // Literals
    Ident,
    StringLiteral,
    Number,
    // Special
    EOF,
    ILLEGAL,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub literal: String,
    pub span: Span,  // line: usize, column: usize
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}
```

### Keyword Lookup (Optimized)

```rust
use once_cell::sync::Lazy;
use std::collections::HashMap;

static KEYWORDS: Lazy<HashMap<&'static str, TokenKind>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("select", TokenKind::Select);
    m.insert("from", TokenKind::From);
    m.insert("where", TokenKind::Where);
    // ... more keywords
    m
});

pub fn lookup_keyword(ident: &str) -> TokenKind {
    KEYWORDS.get(ident.to_lowercase().as_str())
        .copied()
        .unwrap_or(TokenKind::Ident)
}
```

## Lexer Implementation

### Core Lexer Struct

```rust
pub struct Lexer<'a> {
    input: &'a str,
    chars: Chars<'a>,
    ch: Option<char>,
    pos: Position,
    next_pos: Position,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Self {
            input,
            mut chars: input.chars(),
            ch: None,
            pos: Position { line: 1, column: 0, offset: 0 },
            next_pos: Position { line: 1, column: 0, offset: 0 },
        };
        lexer.read_char();
        lexer
    }

    fn read_char(&mut self) {
        self.ch = self.chars.next();
        self.pos = self.next_pos;

        if self.ch.is_some() {
            self.next_pos.offset += 1;
            self.next_pos.column += 1;
            if self.ch == Some('\n') {
                self.next_pos.line += 1;
                self.next_pos.column = 0;
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.clone().next()
    }
}
```

### Token Iteration

```rust
impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespace();

        let token = match self.ch {
            None => self.eof_token(),
            Some('/') if self.peek_char() == Some('*') => self.read_block_comment(),
            Some('-') if self.peek_char() == Some('-') => self.read_line_comment(),
            Some('\'') => self.read_string_literal(),
            Some('@') => self.read_variable(),
            Some(c) if c.is_alphabetic() => self.read_identifier(),
            Some(c) if c.is_numeric() => self.read_number(),
            Some(c) => self.read_single_char_token(c),
        };

        if token.kind == TokenKind::EOF {
            None
        } else {
            Some(token)
        }
    }
}
```

### Handling Special Cases

```rust
// Nested comments (SAP ASE specific)
fn read_block_comment(&mut self) -> Token {
    let start = self.pos;
    let mut depth = 1;

    self.read_char(); // '/'
    self.read_char(); // '*'

    while depth > 0 && self.ch.is_some() {
        match (self.ch, self.peek_char()) {
            (Some('/'), Some('*')) => {
                depth += 1;
                self.read_char();
                self.read_char();
            }
            (Some('*'), Some('/')) => {
                depth -= 1;
                self.read_char();
                self.read_char();
            }
            _ => {
                self.read_char();
            }
        }
    }

    Token {
        kind: TokenKind::COMMENT,
        literal: self.input[start.offset..self.pos.offset].to_string(),
        span: Span { start, end: self.pos },
    }
}

// Variables: @local, @@global
fn read_variable(&mut self) -> Token {
    let start = self.pos;
    self.read_char(); // '@'

    let is_global = self.ch == Some('@');
    if is_global {
        self.read_char();
    }

    let name = self.read_identifier();
    let literal = format!("{}{}", if is_global { "@@" } else { "@" }, name);

    Token {
        kind: if is_global { TokenKind::GlobalVar } else { TokenKind::LocalVar },
        literal,
        span: Span { start, end: self.pos },
    }
}
```

## Parser Implementation

### Result Type

```rust
pub type ParseResult<T> = Result<T, ParseError>;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedToken {
        expected: Vec<String>,
        found: String,
        span: Span,
    },
    Expected {
        expected: &'static str,
        found: String,
    },
    InvalidSyntax {
        message: String,
        span: Span,
    },
}
```

### Parser Struct

```rust
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    peek_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Self {
        let current_token = lexer.next().unwrap_or_else(|| eof_token());
        let peek_token = lexer.next().unwrap_or_else(|| eof_token());

        Self {
            lexer,
            current_token,
            peek_token,
        }
    }

    fn next_token(&mut self) {
        self.current_token = self.peek_token.clone();
        self.peek_token = self.lexer.next().unwrap_or_else(|| eof_token());
    }

    fn expect_token(&mut self, kind: TokenKind) -> ParseResult<()> {
        if self.current_token.kind == kind {
            self.next_token();
            Ok(())
        } else {
            Err(ParseError::Expected {
                expected: format!("{:?}", kind),
                found: self.current_token.literal.clone(),
            })
        }
    }
}
```

### Pratt Parsing for Expressions

```rust
// Operator precedence
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Precedence {
    Lowest,
    Equals,      // ==, !=
    LessGreater, // <, >, <=, >=
    Sum,         // +, -
    Product,     // *, /
    Prefix,      // -X, !X
    Call,        // my_function(X)
}

fn token_precedence(kind: &TokenKind) -> Precedence {
    match kind {
        TokenKind::Eq | TokenKind::Ne => Precedence::Equals,
        TokenKind::Lt | TokenKind::Gt | TokenKind::Le | TokenKind::Ge => Precedence::LessGreater,
        TokenKind::Plus | TokenKind::Minus => Precedence::Sum,
        TokenKind::Asterisk | TokenKind::Slash => Precedence::Product,
        _ => Precedence::Lowest,
    }
}

impl<'a> Parser<'a> {
    fn parse_expression(&mut self, precedence: Precedence) -> ParseResult<Expression> {
        let mut left = self.parse_prefix_expression()?;

        while token_precedence(&self.peek_token.kind) > precedence {
            self.next_token();
            let op = self.current_token.kind.clone();
            let right = self.parse_expression(token_precedence(&op))?;
            left = Expression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }
}
```

## AST Design

### Statement Enum

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    Create(CreateStatement),
    Drop(DropStatement),
    Alter(AlterStatement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub columns: Vec<SelectItem>,
    pub from: Option<TableSource>,
    pub where_clause: Option<Expression>,
    pub group_by: Vec<Expression>,
    pub having: Option<Expression>,
    pub order_by: Vec<OrderByExpression>,
    pub limit: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    Wildcard,
    Expression(Expression),
    Qualified { expr: Expression, alias: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableSource {
    Table { name: ObjectName, alias: Option<String> },
    Join { left: Box<Self>, op: JoinOperator, right: Box<Self> },
    Subquery { query: Box<Statement>, alias: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Column(ObjectName),
    Binary { op: TokenKind, left: Box<Expression>, right: Box<Expression> },
    Unary { op: TokenKind, expr: Box<Expression> },
    Call { func: ObjectName, args: Vec<Expression> },
    Cast { expr: Box<Expression>, type_name: DataType },
    Case { conditions: Vec<(Expression, Expression)>, else_result: Option<Box<Expression>> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Number(i64),
    Float(f64),
    Boolean(bool),
    Null,
}
```

## Error Handling Best Practices

```rust
// Custom error with source location
impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedToken { expected, found, span } => {
                write!(f, "{}:{}: expected one of {}, found '{}'",
                    span.start.line, span.start.column,
                    expected.join(", "), found)
            }
            ParseError::InvalidSyntax { message, span } => {
                write!(f, "{}:{}: {}", span.start.line, span.start.column, message)
            }
            _ => write!(f, "parse error"),
        }
    }
}

impl std::error::Error for ParseError {}
```

## Performance Tips

1. **Use `Cow<str>` for strings** - Avoid allocation when borrowing input
2. **Lazy static keyword map** - Don't recreate HashMap on every lookup
3. **Peek tokens efficiently** - Store 2-3 lookahead tokens, not clone entire iterator
4. **Arena allocation** - Consider using typed_arena for AST nodes if performance critical
5. **Avoid `String::clone()`** - Use `&str` references where possible

## Testing Strategy

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test integration

# Benchmarks
cargo bench

# Fuzz testing
cargo install cargo-fuzz
cargo fuzz run lexer_parse
```
