# Contributing to TSQLRemaker

Thank you for your interest in contributing to TSQLRemaker! This document provides guidelines and instructions for contributing.

---

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Coding Standards](#coding-standards)
- [Testing Guidelines](#testing-guidelines)
- [Commit Messages](#commit-messages)
- [Pull Request Process](#pull-request-process)

---

## Code of Conduct

Be respectful, inclusive, and collaborative. We aim to maintain a welcoming environment for all contributors.

---

## Getting Started

### Prerequisites

- Rust 1.75 or later
- Git
- A GitHub account

### Initial Setup

```bash
# Fork the repository on GitHub
# Clone your fork
git clone https://github.com/YOUR_USERNAME/tsqlremaker.git
cd tsqlremaker

# Add upstream remote
git remote add upstream https://github.com/protom512/tsqlremaker.git

# Install development tools
cargo install cargo-watch
cargo install cargo-edit
```

### Development Commands

```bash
# Watch for changes and run tests
cargo watch -x test

# Format code
cargo fmt

# Run linter
cargo clippy --workspace -- -D warnings

# Run all tests
cargo test --workspace

# Run tests with output
cargo test --workspace -- --nocapture

# Generate coverage report
cargo llvm-cov --workspace --html
```

---

## Development Workflow

### 1. Find or Create an Issue

- Check [existing issues](https://github.com/protom512/tsqlremaker/issues)
- Comment on an issue you want to work on
- Or create a new issue for bugs or feature requests

### 2. Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/your-bug-fix
```

Branch naming conventions:
- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation changes
- `refactor/` - Code refactoring
- `test/` - Test improvements
- `ci/` - CI/CD changes

### 3. Make Your Changes

- Write code following the [Coding Standards](#coding-standards)
- Add tests for your changes
- Update documentation as needed

### 4. Test Your Changes

```bash
# Run affected tests
cargo test -p tsql-lexer

# Run all workspace tests
cargo test --workspace

# Run clippy
cargo clippy --workspace -- -D warnings

# Check formatting
cargo fmt -- --check
```

### 5. Commit Your Changes

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add support for UNICODE string literals
fix: handle escaped quotes in identifiers
docs: update README with new examples
test: add tests for nested block comments
refactor: simplify cursor implementation
ci: upgrade clippy to latest version
```

### 6. Push and Create Pull Request

```bash
git push origin feature/your-feature-name
```

Then create a Pull Request on GitHub.

---

## Coding Standards

### Rust Guidelines

#### 1. Error Handling

**Never use panic in library code:**

```rust
// ❌ Bad
let token = self.tokens.next().unwrap();

// ✅ Good
let token = self.tokens.next()
    .ok_or(LexError::UnexpectedEof)?;
```

#### 2. Use Result for Fallible Operations

```rust
// ❌ Bad
fn parse(input: &str) -> Statement {
    if input.is_empty() {
        panic!("Empty input");
    }
    // ...
}

// ✅ Good
fn parse(input: &str) -> Result<Statement, ParseError> {
    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    // ...
}
```

#### 3. Prefer Borrowing Over Cloning

```rust
// ❌ Bad
fn tokenize(input: String) -> Vec<Token>

// ✅ Good
fn tokenize(input: &str) -> Vec<Token>
```

#### 4. Use Iterator Methods

```rust
// ❌ Bad
let mut result = Vec::new();
for token in tokens {
    if token.kind != TokenKind::EOF {
        result.push(token);
    }
}

// ✅ Good
let result: Vec<_> = tokens
    .filter(|t| t.kind != TokenKind::EOF)
    .collect();
```

### Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Types | PascalCase | `TokenKind`, `Lexer` |
| Functions | snake_case | `next_token`, `parse_expression` |
| Constants | SCREAMING_SNAKE_CASE | `MAX_KEYWORD_LENGTH` |
| Variables | snake_case | `input_string`, `token_count` |

### Documentation

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
/// use tsql_lexer::Lexer;
///
/// let sql = "SELECT * FROM users";
/// let tokens: Vec<_> = Lexer::new(sql).collect();
/// ```
pub fn tokenize(input: &str) -> impl Iterator<Item = Token> + '_ {
    Lexer::new(input)
}
```

---

## Testing Guidelines

### Coverage Requirements

- **Minimum 80%** line coverage for all modules
- **90%+ coverage** for critical parsing logic

### Test Organization

```
crates/
├── tsql-lexer/
│   ├── src/
│   │   └── lexer.rs       # Include unit tests here
│   └── tests/             # Integration tests
│       └── integration_tests.rs
```

### Test Patterns

#### Table-Driven Tests

```rust
#[test]
fn test_keyword_recognition() {
    let tests = vec![
        ("SELECT", TokenKind::Select),
        ("FROM", TokenKind::From),
        ("select", TokenKind::Select),  // Case insensitive
    ];

    for (input, expected) in tests {
        let mut lexer = Lexer::new(input);
        let token = lexer.next_token().unwrap();
        assert_eq!(token.kind, expected, "Failed for: {}", input);
    }
}
```

#### Error Testing

```rust
#[test]
fn test_unterminated_string() {
    let input = "'unterminated";
    let result = Lexer::new(input).next();

    assert!(result.is_err());
    match result.unwrap_err() {
        LexError::UnterminatedString { .. } => {}
        _ => panic!("Expected UnterminatedString error"),
    }
}
```

### Pre-Commit Checklist

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt -- --check` passes
- [ ] New tests added for new functionality
- [ ] Documentation updated

---

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation changes |
| `style` | Code style changes (formatting) |
| `refactor` | Code refactoring |
| `test` | Test changes |
| `ci` | CI/CD changes |
| `chore` | Other changes |

### Examples

```
feat(lexer): add support for hexadecimal literals

The lexer now recognizes hexadecimal numbers in the format 0xABCD.
This is commonly used for binary data in SAP ASE.

Closes #123
```

```
fix(parser): handle empty JOIN clauses correctly

Previously, empty JOIN clauses would cause a panic. Now they
return a proper ParseError with position information.

Fixes #456
```

---

## Pull Request Process

### Before Submitting

1. **Update documentation** - If your changes affect user-facing behavior
2. **Add tests** - Ensure adequate test coverage
3. **Update CHANGELOG** - Add entries to the Unreleased section
4. **Rebase** - Keep your branch up to date with main

```bash
git fetch upstream
git rebase upstream/master
```

### Pull Request Template

When creating a PR, fill out the template:

```markdown
## Summary
Brief description of changes

## Related Issue
Closes #(issue number)

## Changes
- Change 1
- Change 2

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation

## Testing
- [ ] Tests added/updated
- [ ] All tests pass
- [ ] Coverage adequate

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
```

### Review Process

1. **Automated checks** - CI must pass
2. **Code review** - At least one maintainer approval
3. **Test coverage** - Must meet minimum thresholds
4. **Documentation** - Must be updated if needed

### After Merge

- Your contribution will be credited
- The changelog will be updated with your name
- The next release will include your changes

---

## Architecture Guidelines

See [`.claude/rules/architecture-coupling-balance.md`](.claude/rules/architecture-coupling-balance.md) for detailed architecture rules.

Key principles:
- **Single-direction dependencies** - Lower layers don't depend on upper layers
- **Contract coupling** - Use traits for cross-crate communication
- **High cohesion** - Related code in same crate

---

## Getting Help

- **Documentation**: [Wiki](https://github.com/protom512/tsqlremaker/wiki)
- **Issues**: [Issue Tracker](https://github.com/protom512/tsqlremaker/issues)
- **Discussions**: [GitHub Discussions](https://github.com/protom512/tsqlremaker/discussions)

---

Thank you for contributing to TSQLRemaker!
