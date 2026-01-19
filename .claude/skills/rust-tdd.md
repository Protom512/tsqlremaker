# Rust TDD Workflow

Test-Driven Development for Rust projects with emphasis on lexer/parser/AST development.

## Philosophy

- **Test First**: Always write tests before implementation
- **Red-Green-Refactor**: Follow the TDD cycle
- **80%+ Coverage**: Maintain high test coverage
- **Property-Based Testing**: Use proptest for parser validation

## Workflow

### 1. Red Phase - Write Failing Test

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizes_select_keyword() {
        let input = "SELECT";
        let mut lexer = Lexer::new(input);
        let token = lexer.next_token();

        assert_eq!(token.kind, TokenKind::SELECT);
        assert_eq!(token.literal, "SELECT");
    }

    #[test]
    fn test_tokenizes_identifier() {
        let input = "table_name";
        let mut lexer = Lexer::new(input);
        let token = lexer.next_token();

        assert_eq!(token.kind, TokenKind::IDENT);
        assert_eq!(token.literal, "table_name");
    }
}
```

### 2. Green Phase - Minimal Implementation

```rust
impl Lexer {
    pub fn next_token(&mut self) -> Token {
        // Minimal implementation to pass tests
        match self.input.chars().next() {
            Some('S') => Token { kind: TokenKind::SELECT, literal: "SELECT".to_string() },
            Some(c) if c.is_alphabetic() => Token { kind: TokenKind::IDENT, literal: self.read_identifier() },
            _ => Token { kind: TokenKind::EOF, literal: String::new() },
        }
    }
}
```

### 3. Refactor Phase - Improve Code

```rust
impl Lexer {
    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        match self.ch {
            Some(c) if c.is_alphabetic() => self.read_identifier_or_keyword(),
            Some(c) if c.is_numeric() => self.read_number(),
            _ => self.eof_token(),
        }
    }
}
```

## Testing Patterns

### Unit Tests for Lexer

```rust
#[test]
fn test_handles_whitespace() {
    let input = "  SELECT   *  ";
    let tokens: Vec<_> = Lexer::new(input).collect();

    assert_eq!(tokens[0].kind, TokenKind::SELECT);
    assert_eq!(tokens[1].kind, TokenKind::ASTERISK);
    assert_eq!(tokens[2].kind, TokenKind::EOF);
}

#[test]
fn test_tokenizes_string_literal() {
    let input = "'hello ''world''";
    let mut lexer = Lexer::new(input);
    let token = lexer.next_token();

    assert_eq!(token.kind, TokenKind::STRING);
    assert_eq!(token.literal, "'hello ''world'''");
}
```

### Unit Tests for Parser

```rust
#[test]
fn test_parses_simple_select() {
    let input = "SELECT col1, col2 FROM table1";
    let stmt = parse_statement(input).unwrap();

    match stmt {
        Statement::Select(select) => {
            assert_eq!(select.columns.len(), 2);
            assert_eq!(select.from.name, "table1");
        }
        _ => panic!("Expected SELECT statement"),
    }
}

#[test]
fn test_parse_error_recovery() {
    let input = "SELECT FROM table1";  // Missing columns
    let result = parse_statement(input);

    assert!(result.is_err());
    match result.unwrap_err() {
        ParseError::Expected { expected, found } => {
            assert_eq!(expected, "identifier or *");
        }
        _ => panic!("Expected Expected error"),
    }
}
```

### Property-Based Tests

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_roundtrip(sql in "[a-zA-Z0-9_ ,]+") {
        let tokens: Vec<_> = Lexer::new(&sql).collect();
        let reconstructed = reconstruct_from_tokens(&tokens);

        // After normalizing whitespace, should be equivalent
        assert_eq!(normalize_whitespace(&sql), normalize_whitespace(&reconstructed));
    }
}
```

## Coverage Requirements

- **Minimum 80%** line coverage
- **100%** coverage for critical parsing logic
- Use `cargo tarpaulin` for coverage reports:

```bash
cargo tarpaulin --out Html --output-dir coverage
```

## Test Organization

```
src/
├── lib.rs
├── lexer.rs
├── parser.rs
└── ast.rs

tests/
├── lexer_tests.rs      # Lexer unit tests
├── parser_tests.rs     # Parser unit tests
├── integration_tests.rs # Full SQL statement tests
└── fixtures/           # Test SQL files
    ├── simple_select.sql
    └── complex_procedure.sql
```

## Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_tokenizes_select

# Run tests with coverage
cargo tarpaulin

# Run clippy (linter)
cargo clippy -- -D warnings
```

## Checklist

- [ ] Write failing test first
- [ ] Run `cargo test` to verify failure
- [ ] Implement minimal code to pass
- [ ] Run `cargo test` to verify pass
- [ ] Refactor while maintaining green tests
- [ ] Run `cargo clippy` for lint checks
- [ ] Run `cargo fmt` for formatting
- [ ] Verify coverage >= 80%
