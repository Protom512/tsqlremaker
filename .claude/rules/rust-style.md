# Rust Coding Style

Rust-specific coding standards for this project.

## Core Principles

1. **Readability First**: Code is read more than written
2. **Idiomatic Rust**: Follow Rust conventions and patterns
3. **Type Safety**: Leverage the type system
4. **Error Handling**: Use `Result`, never `panic!` in library code
5. **Ownership**: Respect borrow checker rules

## Naming Conventions

```rust
// Types: PascalCase
struct Token { }
enum TokenKind { }
trait Visitor { }
type ParseResult<T> = Result<T, Error>;

// Functions and methods: snake_case
fn next_token() { }
fn parse_expression() { }

// Constants: SCREAMING_SNAKE_CASE
const MAX_KEYWORD_LENGTH: usize = 20;
static KEYWORDS: Lazy<HashMap<&str, TokenKind>> = Lazy::new(|| { ... });

// Variables: snake_case
let input_string = "hello";
let token_count = 42;
```

## Structs and Enums

```rust
// Derive common traits
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Select,
    From,
    Ident,
}

// Use field init shorthand
fn new_token(kind: TokenKind, literal: String) -> Token {
    Token { kind, literal }
}

// Use builder pattern for complex construction
impl Token {
    pub fn builder() -> TokenBuilder {
        TokenBuilder::default()
    }
}
```

## Error Handling

### Prefer Result over panic!

```rust
// Bad: panics on error
fn parse(input: &str) -> Statement {
    if input.is_empty() {
        panic!("Input cannot be empty");
    }
    // ...
}

// Good: returns Result
fn parse(input: &str) -> ParseResult<Statement> {
    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    // ...
}
```

### Use ? operator for propagation

```rust
// Good: idiomatic error propagation
fn parse_statement(input: &str) -> ParseResult<Statement> {
    let tokens = tokenize(input)?;
    let stmt = parse_from_tokens(tokens)?;
    Ok(stmt)
}

// Avoid: manual error propagation
fn parse_statement(input: &str) -> ParseResult<Statement> {
    let tokens = match tokenize(input) {
        Ok(t) => t,
        Err(e) => return Err(e),
    };
    // ...
}
```

### Use thiserror for custom errors

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected token: expected {expected:?}, found {found:?}")]
    UnexpectedToken {
        expected: Vec<TokenKind>,
        found: TokenKind,
        #[source] source: Option<Box<dyn std::error::Error>>,
    },

    #[error("invalid syntax at {line}:{column}: {message}")]
    InvalidSyntax { line: usize, column: usize, message: String },

    #[error("unsupported feature: {0}")]
    Unsupported(String),
}
```

## Ownership and Borrowing

### Prefer borrowing over cloning

```rust
// Bad: unnecessary allocation
fn tokenize(input: String) -> Vec<Token> {
    // ...
}

// Good: borrow input
fn tokenize(input: &str) -> Vec<Token> {
    // ...
}

// Even better: return iterator
fn tokenize(input: &str) -> impl Iterator<Item = Token> + '_ {
    // ...
}
```

### Use Cow for conditional ownership

```rust
use std::borrow::Cow;

pub struct Token<'a> {
    pub kind: TokenKind,
    pub literal: Cow<'a, str>,  // Borrowed or owned
}

// If we need to modify/own:
Token {
    kind: TokenKind::STRING,
    literal: Cow::Owned(format!("{}{}", prefix, value)),
}

// If we can borrow:
Token {
    kind: TokenKind::IDENT,
    literal: Cow::Borrowed("identifier"),
}
```

## Iterators

### Prefer iterator methods over loops

```rust
// Bad: imperative loop
let mut result = Vec::new();
for token in tokens {
    if token.kind != TokenKind::EOF {
        result.push(token);
    }
}

// Good: functional style
let result: Vec<_> = tokens
    .filter(|t| t.kind != TokenKind::EOF)
    .collect();
```

### Use impl Iterator for return types

```rust
// Good: Abstract return type
fn tokenize<'a>(input: &'a str) -> impl Iterator<Item = Token<'a>> + 'a {
    Lexer::new(input)
}

// Or explicit if needed
fn tokenize<'a>(input: &'a str) -> Lexer<'a> {
    Lexer::new(input)
}
```

## Pattern Matching

### Match all variants

```rust
match token.kind {
    TokenKind::SELECT => { /* ... */ }
    TokenKind::FROM => { /* ... */ }
    _ => return Err(ParseError::UnexpectedToken),
}
```

### Use if let for single variant matching

```rust
// Good: single variant
if let TokenKind::NUMBER = token.kind {
    // handle number
}

// Good: multiple variants with same handling
if matches!(token.kind, TokenKind::NUMBER | TokenKind::STRING) {
    // handle literal
}
```

## Collections

### Use Vec for sequences

```rust
let mut tokens = Vec::new();
tokens.push(token);
let first = tokens.first();
```

### Use HashMap for lookups

```rust
use std::collections::HashMap;

let mut map = HashMap::new();
map.insert("select", TokenKind::SELECT);

if let Some(&kind) = map.get("select") {
    // ...
}
```

### Use lazy_static/once_cell for static initialization

```rust
use once_cell::sync::Lazy;

static KEYWORDS: Lazy<HashMap<&str, TokenKind>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("select", TokenKind::SELECT);
    m.insert("from", TokenKind::FROM);
    m
});
```

## Macros

### Use macros judiciously

```rust
// Good: reduce repetition for token variants
macro_rules! define_tokens {
    ($($name:ident),*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum TokenKind {
            $($name),*,
            ILLEGAL,
            EOF,
        }
    };
}

define_tokens!(SELECT, FROM, WHERE, INSERT);
```

## Documentation

### Document public APIs

```rust
/// Tokenizes a SQL input string into a stream of tokens.
///
/// # Arguments
///
/// * `input` - The SQL string to tokenize
///
/// # Returns
///
/// An iterator that yields tokens in order.
///
/// # Errors
///
/// Returns an error if the input contains invalid characters or
/// unterminated literals.
///
/// # Examples
///
/// ```
/// use tsql_lexer::tokenize;
///
/// let tokens: Vec<_> = tokenize("SELECT * FROM users").collect();
/// assert_eq!(tokens[0].kind, TokenKind::SELECT);
/// ```
pub fn tokenize(input: &str) -> impl Iterator<Item = Token> + '_ {
    Lexer::new(input)
}
```

### Document modules

```rust
//! # T-SQL Lexer
//!
//! This crate provides a lexer for SAP ASE T-SQL dialect.
//!
//! ## Example
//!
//! ```
//! use tsql_lexer::Lexer;
//!
//! let sql = "SELECT * FROM users";
//! let tokens: Vec<_> = Lexer::new(sql).collect();
//! ```
```

## Performance Guidelines

1. **Avoid allocations in hot paths**: Use references where possible
2. **Use lazy static for static data**: Don't recreate HashMaps
3. **Prefer iterators over collecting**: Process data lazily
4. **Profile before optimizing**: Use criterion for benchmarks

## Formatting

- Use `rustfmt` for all code
- Set max line length to 100 characters
- Run `cargo fmt` before committing

```bash
# Check formatting
cargo fmt -- --check

# Apply formatting
cargo fmt
```

## Linting

- Use `clippy` for additional checks
- Fix all warnings
- Treat warnings as errors in CI

```bash
# Run clippy
cargo clippy -- -D warnings
```
