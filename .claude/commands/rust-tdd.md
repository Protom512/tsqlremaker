# Rust TDD Command

Invoke the Rust TDD workflow for test-driven development in Rust projects.

## Usage

```
/rust-tdd [module-name]
```

## Description

This command guides you through the TDD cycle for Rust development:
1. Write failing tests first
2. Implement minimal code to pass
3. Refactor while keeping tests green

## Examples

```bash
# Start TDD for a new module
/rust-tdd lexer

# TDD for a specific feature
/rust-tdd parser-expressions
```

## Workflow

The agent will:

1. **Understand requirements**: Ask clarifying questions if needed
2. **Write tests**: Create comprehensive tests covering:
   - Happy path
   - Edge cases
   - Error cases
3. **Run tests**: Execute `cargo test` to verify failure
4. **Implement**: Write minimal code to pass tests
5. **Verify**: Run tests again to ensure passing
6. **Refactor**: Clean up code while maintaining green tests
7. **Quality checks**: Run clippy and fmt

## Commands Used During Workflow

```bash
cargo test                           # Run tests
cargo test -- --nocapture            # With output
cargo clippy -- -D warnings          # Lint check
cargo fmt                            # Format code
cargo tarpaulin --threshold 80       # Coverage check
```

## Output

At the end, you'll have:
- Well-tested code (80%+ coverage)
- No clippy warnings
- Formatted code
- Tests that serve as documentation
