# TSQLRemaker

<div align="center">

  **SAP ASE T-SQL → Other SQL Dialects Converter**

  [![CI](https://github.com/protom512/tsqlremaker/actions/workflows/ci.yml/badge.svg)](https://github.com/protom512/tsqlremaker/actions/workflows/ci.yml)
  [![Coverage](https://codecov.io/gh/protom512/tsqlremaker/branch/main/graph/badge.svg)](https://codecov.io/gh/protom512/tsqlremaker)
  [![Crates.io](https://img.shields.io/crates/v/tsqlremaker)](https://crates.io/crates/tsqlremaker)
  [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

  [Documentation](https://github.com/protom512/tsqlremaker/wiki) •
  [Contributing](#contributing) •
  [Changelog](CHANGELOG.md)

</div>

---

## Overview

**TSQLRemaker** is a SQL dialect converter that transforms SAP ASE (Sybase Adaptive Server Enterprise) T-SQL code into other SQL dialects such as MySQL, PostgreSQL, and more.

### Key Features

- **Full SAP ASE T-SQL Lexer Support** - Parses all ASE-specific syntax including:
  - Nested block comments (`/* /* */ */`)
  - Variables (`@local`, `@@global`)
  - Temporary tables (`#temp`, `##global_temp`)
  - Quoted identifiers (`[identifier]`, `"identifier"`)
  - Unicode strings (`N'string'`, `U&'string'`)
  - All ASE keywords and operators

- **High Performance** - Zero-copy tokenization, processes 1MB+ SQL files in <100ms

- **Type-Safe** - Written in Rust with comprehensive error handling

- **Architecture** - Clean separation of concerns with modular crate design

---

## Project Status

| Component | Status | Coverage |
|-----------|--------|----------|
| **Lexer** | ✅ Implemented | 90%+ |
| **Parser** | 🚧 In Progress | - |
| **Common SQL AST** | 🚧 In Progress | - |
| **MySQL Emitter** | 📝 Planned | - |
| **PostgreSQL Emitter** | 📝 Planned | - |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        TSQLRemaker                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐ │
│  │   Source    │───▶│    Lexer    │───▶│   Token Stream      │ │
│  │ (SAP ASE    │    │ (tsql-lexer│    │   (tsql-token)       │ │
│  │  T-SQL)     │    │             │    │                     │ │
│  └─────────────┘    └─────────────┘    └─────────────────────┘ │
│                                                  │              │
│                                                  ▼              │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Parser                               │   │
│  │                 (tsql-parser)                          │   │
│  │                      │                                  │   │
│  │                      ▼                                  │   │
│  │            ┌───────────────────┐                        │   │
│  │            │  SAP ASE AST      │                        │   │
│  │            └───────────────────┘                        │   │
│  │                      │                                  │   │
│  │                      ▼                                  │   │
│  │            ┌───────────────────┐                        │   │
│  │            │  Common SQL AST   │                        │   │
│  │            └───────────────────┘                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                            │                                    │
│                            ▼                                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Emitters                             │   │
│  │  ┌───────────┐  ┌─────────────┐  ┌─────────────────┐   │   │
│  │  │   MySQL   │  │ PostgreSQL  │  │   Other         │   │   │
│  │  │  Emitter  │  │   Emitter   │  │   Emitters      │   │   │
│  │  └───────────┘  └─────────────┘  └─────────────────┘   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/protom512/tsqlremaker.git
cd tsqlremaker

# Build
cargo build --release

# The binary will be available at target/release/tsqlremaker
```

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
tsql-lexer = "0.1"
tsql-token = "0.1"
```

---

## Usage

### Command Line (Future)

```bash
# Convert ASE T-SQL to MySQL
tsqlremaker convert --input query.sql --dialect mysql

# Convert entire directory
tsqlremaker convert --dir ./sql --dialect postgres --output ./converted
```

### As a Library

```rust
use tsql_lexer::Lexer;

fn main() {
    let sql = r#"
        SELECT TOP 10 *
        FROM users
        WHERE @status = 'active'
    "#;

    let lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.collect();

    for token in tokens {
        println!("{:?}", token);
    }
}
```

#### With Comments Preserved

```rust
use tsql_lexer::Lexer;

let sql = "SELECT * FROM users /* active users only */";
let lexer = Lexer::new(sql).with_comments(true);

// Comments will be included in the token stream
```

---

## Development

### Prerequisites

- Rust 1.75 or later
- Git

### Setup

```bash
# Clone repository
git clone https://github.com/protom512/tsqlremaker.git
cd tsqlremaker

# Run tests
cargo test --workspace

# Run with coverage
cargo llvm-cov --workspace --html

# Format code
cargo fmt

# Lint
cargo clippy --workspace -- -D warnings
```

### Project Structure

```
tsqlremaker/
├── crates/
│   ├── tsql-token/        # Token definitions (TokenKind, Span, Position)
│   ├── tsql-lexer/        # SAP ASE T-SQL lexer
│   ├── tsql-parser/       # Parser (planned)
│   ├── common-sql/        # Common SQL AST (planned)
│   └── mysql-emitter/     # MySQL code generator (planned)
├── .github/
│   ├── workflows/         # CI/CD configurations
│   └── ISSUE_TEMPLATE/    # Issue templates
├── .claude/
│   └── rules/             # Development guidelines
├── .kiro/
│   ├── specs/             # Feature specifications
│   └── steering/          # Project-wide context
└── docs/                  # Additional documentation
```

---

## SAP ASE T-SQL Support

### Supported Syntax

| Feature | ASE Syntax | Status |
|---------|------------|--------|
| Nested block comments | `/* /* */ */` | ✅ |
| Line comments | `-- comment` | ✅ |
| Local variables | `@variable` | ✅ |
| Global variables | `@@variable` | ✅ |
| Temp tables | `#table` | ✅ |
| Global temp tables | `##table` | ✅ |
| Bracket identifiers | `[identifier]` | ✅ |
| Quoted identifiers | `"identifier"` | ✅ |
| Unicode strings | `N'string'` | ✅ |
| Unicode escape | `U&'\+XXXXXX'` | ✅ |
| Hex numbers | `0xABCD` | ✅ |
| TOP clause | `SELECT TOP 10` | ✅ |
| Comparison operators | `!<`, `!>`, `<>` | ✅ |

### Token Examples

```sql
-- All of these are correctly tokenized

SELECT * FROM [table-name]        -- Quoted identifier
DECLARE @counter INT                -- Local variable
SELECT @@identity                   -- Global variable
CREATE TABLE #temp (id INT)         -- Temp table
/* /* Nested comment */ */          -- Nested block comment
```

---

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

### Development Workflow

1. Check existing [Issues](https://github.com/protom512/tsqlremaker/issues)
2. Fork the repository
3. Create a feature branch (`git checkout -b feature/amazing-feature`)
4. Make your changes
5. Run tests (`cargo test --workspace`)
6. Commit your changes ([Conventional Commits](https://www.conventionalcommits.org/))
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

### Code Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting
- No panics in library code (use `Result` instead)
- 80%+ test coverage required

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Inspired by SQL parser and transpiler projects
- Architecture guided by ["Balanced Coupling"](https://www.youtube.com/watch?v=hWrGbdq2OQ0) principles

---

## Links

- [Documentation](https://github.com/protom512/tsqlremaker/wiki)
- [Issue Tracker](https://github.com/protom512/tsqlremaker/issues)
- [Discussions](https://github.com/protom512/tsqlremaker/discussions)
- [Changelog](CHANGELOG.md)
