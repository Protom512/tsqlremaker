# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Features
- Implement SAP ASE T-SQL Parser with full grammar support
  - Recursive descent parser with Pratt expression parsing
  - Support for DML: SELECT (DISTINCT, TOP, JOINs, WHERE, GROUP BY, HAVING, ORDER BY)
  - Support for DML: INSERT (VALUES, INSERT-SELECT), UPDATE, DELETE
  - Support for DDL: CREATE TABLE/INDEX/VIEW/PROCEDURE
  - Support for control flow: IF...ELSE, WHILE, BEGIN...END, BREAK, CONTINUE, RETURN
  - Support for variables: DECLARE, SET with LocalVar (@variable)
  - Support for batch processing: GO separator detection
  - Expression parsing with operator precedence (arithmetic, comparison, logical, bitwise)
  - CASE expressions, function calls, subqueries
  - Error recovery with synchronization points
  - Span tracking for all AST nodes
- Implement SAP ASE T-SQL Lexer with full token support
- Add TokenKind enum with 190+ variants for all T-SQL keywords
- Add Position and Span for source location tracking
- Support nested block comments (T-SQL specific)
- Support quoted identifiers with brackets and double quotes
- Support variables (local @, global @@) and temp tables (#, ##)
- Support string literals (string, N-string, Unicode string)
- Support numeric literals (integer, float, hex)

### Bug Fixes
- Fixed handling of escaped quotes in string literals
- Fixed position tracking with tab expansion (8 spaces)

### Documentation
- Added architecture rules based on "Balanced Coupling" principles
- Added Rust coding style guidelines
- Added Rust testing rules with 80% coverage requirement
- Added Rust anti-patterns rules (no panic in library code)

### CI/CD
- Set up GitHub Actions CI with lint, test, coverage
- Set up automated releases with cross-platform binaries
- Set up Dependabot for dependency updates

### Testing
- 44 unit tests passing (parser: 44, lexer: 21)
- Table-driven tests for keyword recognition
- Error case tests for unterminated literals
- Expression parsing tests with operator precedence
- Statement parsing tests for all T-SQL constructs
