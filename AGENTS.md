# AGENTS.md

## Build, Test, and Lint Commands

### Build
```bash
cargo build                    # Build all workspace crates
cargo build --package tsql-lexer     # Build specific crate
cargo check                    # Check without building artifacts
cargo clean                    # Clean build artifacts
```

### Testing
```bash
cargo test                    # Run all tests
cargo test --verbose          # Verbose output
cargo test --package tsql-lexer       # Test specific package
cargo test --package tsql-lexer -- <test_name>  # Run single test
cargo tarpaulin -- --test-threads 1    # Coverage report
```

### Code Quality
```bash
cargo fmt                     # Format code
cargo fmt --check             # Check formatting
cargo clippy                  # Run linter
cargo clippy -- -D warnings   # Warnings as errors
```

## Code Style Guidelines

### Naming Conventions
- **Structs/Enums:** `PascalCase` (e.g., `Lexer`, `Token`)
- **Functions/Methods:** `snake_case` (e.g., `next_token`, `read_identity`)
- **Variables:** `snake_case` (e.g., `ch`, `input`, `position`)
- **Constants:** `SCREAMING_SNAKE_CASE` (e.g., `SELECT`, `EOF`, `IDENT`)
- **Type Aliases:** `snake_case` (e.g., `pub type token_type = String;`)

### Imports
- Group imports by crate (e.g., `use tsql_token::{...};`)
- Keep related constants on separate import lines or grouped
- Standard library imports after workspace imports

### Formatting
- Use standard `rustfmt` defaults (no custom config)
- 4-space indentation
- Struct fields: one per line for public structs
- Match arms: braces on same line, body on next line

### Types and Lifetimes
- Use explicit lifetime parameters for references (`'a`)
- Derive `Debug` on all public structs for debugging
- Use `&str` for string references, `String` for owned strings

### Error Handling
- Current: `panic!()` for invalid tokens
- Future: migrate to `Result<T, E>` types for better error handling
- Avoid `.unwrap()` where possible, use pattern matching

### Visibility
- Mark all public items with `pub`
- Internal helper methods: private (no `pub`)
- Test utilities: in `tests/` directory

### Comments
- Multilingual: Japanese and English comments are acceptable
- Use `///` for public API documentation
- Use `//` for inline comments
- Document non-obvious logic

### Token Definitions
- Keywords as module-level `pub const` with lowercase string values
- Use `HashMap` in `lookup_ident()` for keyword resolution
- Case-insensitive matching (`.to_lowercase()`)
- Return `IDENT` token type for unrecognized identifiers

### Lexer Implementation
- Cursor-based approach with `ch`, `position`, `read_position`
- Methods: `next_token()`, `read_char()`, `peek_char()`, `eat_whitespace()`
- Skip whitespace before reading tokens
- Support: numbers, identifiers (letters, digits, `_`, `.`), operators, punctuation

### Workspace Structure
- `tsql-token/`: Token type definitions and lookup
- `tsql-lexer/`: Lexical analysis implementation
- Both crates target `cdylib` (C dynamic lib) and `rlib` (Rust lib)

### CI/CD
- Run on `master` branch pushes and PRs
- Uses `cargo-tarpaulin` for coverage
- Single-threaded tests (`--test-threads 1`)

### Notes
- No existing Cursor/Copilot rules files
- Early development phase - patterns still evolving
- Target platforms: Linux, macOS, future WASM/ARM support