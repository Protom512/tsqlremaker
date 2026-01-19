# Rust Testing Rules

Mandatory testing guidelines for Rust development in this project.

## Coverage Requirements

- **Minimum 80%** line coverage for all modules
- **90%+ coverage** for critical parsing logic (lexer, parser)
- Use `cargo tarpaulin` or `cargo-llvm-cov` for coverage reports

```bash
# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage/

# Run with threshold
cargo tarpaulin --threshold 80 --fail-under
```

## Test Organization

### Directory Structure

```
crates/
├── tsql-lexer/
│   ├── src/
│   │   └── lib.rs
│   └── tests/              # Integration tests
│       ├── lexer_tests.rs
│       └── fixtures/
│           └── samples.sql
│
└── common-sql/
    ├── src/
    │   └── ast/
    │       └── mod.rs
    └── tests/              # AST-specific tests
        └── ast_tests.rs
```

### Unit Tests (in src/)

Place unit tests in the same file as the code being tested:

```rust
// In src/lib.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        // ...
    }
}
```

### Integration Tests (in tests/)

Use integration tests for API-level testing:

```rust
// In tests/lexer_tests.rs
use tsql_lexer::Lexer;

#[test]
fn test_full_statement() {
    let sql = "SELECT * FROM users WHERE id = 1";
    let tokens: Vec<_> = Lexer::new(sql).collect();

    assert!(tokens.iter().any(|t| t.kind == TokenKind::SELECT));
}
```

## Testing Patterns

### 1. Table-Driven Tests

```rust
#[test]
fn test_keyword_recognition() {
    let tests = vec![
        ("SELECT", TokenKind::SELECT),
        ("FROM", TokenKind::FROM),
        ("WHERE", TokenKind::WHERE),
        ("select", TokenKind::SELECT),  // Case insensitive
        ("Select", TokenKind::SELECT),
    ];

    for (input, expected) in tests {
        let mut lexer = Lexer::new(input);
        let token = lexer.next_token().unwrap();
        assert_eq!(token.kind, expected, "Failed for input: {}", input);
    }
}
```

### 2. Error Testing

```rust
#[test]
fn test_unterminated_string() {
    let input = "'unterminated";
    let result = Lexer::new(input).next_token();

    assert!(result.is_err());
    match result.unwrap_err() {
        LexError::UnterminatedString { pos, .. } => {
            assert_eq!(pos.column, 1);
        }
        _ => panic!("Expected UnterminatedString error"),
    }
}
```

### 3. Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_tokenize_valid_identifiers(ident in "[a-zA-Z_][a-zA-Z0-9_]*") {
        let mut lexer = Lexer::new(&ident);
        let token = lexer.next_token().unwrap();

        assert_eq!(token.kind, TokenKind::IDENT);
        assert_eq!(token.literal, ident);
    }
}
```

## Test Naming

- Use descriptive names: `test_tokenizes_nested_comments`
- For negative tests: `test_errors_on_x`
- For edge cases: `test_handles_empty_string`

## Mandatory Test Categories

### Lexer Tests

- [x] All keywords (case insensitive)
- [x] Identifiers
- [x] String literals (including escaped quotes)
- [x] Number literals (integer, decimal, scientific)
- [x] Operators (all variants)
- [x] Comments (block, line, nested)
- [x] Variables (local `@`, global `@@`)
- [x] Temp tables (`#`, `##`)
- [x] Whitespace handling
- [x] Error cases (unterminated strings, invalid characters)

### Parser Tests

- [x] All statement types (SELECT, INSERT, UPDATE, DELETE, CREATE)
- [x] Expression precedence
- [x] Nested expressions
- [x] JOIN variants
- [x] Subqueries
- [x] Error cases (missing tokens, invalid syntax)
- [x] Error recovery

### AST Tests

- [x] Node construction
- [x] Visitor pattern
- [x] Type conversions

## Pre-Commit Checklist

Before committing code:

```bash
# 1. Run all tests
cargo test

# 2. Run with --release for additional checks
cargo test --release

# 3. Check clippy
cargo clippy -- -D warnings

# 4. Check formatting
cargo fmt -- --check

# 5. Run coverage (if making significant changes)
cargo tarpaulin --threshold 80
```

## CI Requirements

The CI pipeline must:

1. Run `cargo test` on all crates
2. Run `cargo clippy` with strict warnings
3. Check formatting with `cargo fmt --check`
4. Generate coverage report (for PR review)

## Prohibited Practices

- ❌ Commenting out tests instead of fixing them
- ❌ Using `#[ignore]` without a GitHub issue reference
- ❌ Testing implementation details (test behavior)
- ❌ Hardcoding test values without explanation
- ❌ Skipping error path testing
