# Implementation Tasks: T-SQL Parser for SAP ASE

---

## Task Overview

| Category | Task Count | Estimate |
|----------|-----------|----------|
| Core Infrastructure | 4 | 10-12h |
| Statement Parsers | 6 | 18-24h |
| Expression & Join | 3 | 12-18h |
| Batch Processing | 2 | 8-12h |
| Error Handling | 2 | 6-9h |
| AST & Conversion | 3 | 9-13h |
| Testing | 5 | 15-20h |
| Integration | 1 | 4-6h |
| **Total** | **26 major tasks** | **82-114h** |

---

## Task 1: Project Foundation and Crate Setup

- [ ] 1.1 (P) Create tsql-parser crate structure with Cargo.toml
  - Create new crate under `crates/tsql-parser/` directory
  - Configure Cargo.toml with dependencies: tsql-lexer, tsql-token, thiserror, once_cell
  - Set dev-dependencies: rstest, criterion
  - Enable Rust 2021 edition
  - Configure lib.rs for public API exports
  - _Requirements: 1.1_

- [ ] 1.2 (P) Define error types and Result alias
  - Create ParseError enum with variants: UnexpectedToken, UnexpectedEOF, InvalidSyntax, RecursionLimitExceeded, BatchError
  - Implement std::fmt::Display and std::error::Error for all error types
  - Include span/position information in all error variants
  - Create ParseResult<T> type alias for Result<T, ParseError>
  - Add error constructors for each variant
  - _Requirements: 13.1, 13.2, 13.3, 13.6_

- [ ] 1.3 (P) Define ParserMode and configuration types
  - Create ParserMode enum: BatchMode, SingleStatement
  - Create ParserConfig struct for parser configuration
  - Implement builder pattern for Parser construction
  - Define default configuration (batch mode)
  - _Requirements: 18.1, 18.2, 18.5_

- [ ] 1.4 Create AST module structure with base traits
  - Create ast/ module directory with mod.rs
  - Define AstNode trait with span() method
  - Create Statement enum as root AST node type
  - Set up module structure for statement, expression, and data_type submodules
  - _Requirements: 14.4_

---

## Task 2: Token Buffer and Lexer Integration

- [ ] 2.1 Implement TokenStream trait for Lexer abstraction
  - Define TokenStream trait with methods: current(), peek(n), consume()
  - Implement trait for tsql_lexer::Lexer
  - Add EOF handling to return TokenKind::EOF when exhausted
  - Create token reference lifetime management
  - _Requirements: 1.1, 1.2, 1.4_

- [ ] 2.2 Implement TokenBuffer with lookahead capability
  - Create TokenBuffer struct with circular buffer storage (size 3+)
  - Implement new() that pre-fills buffer from lexer
  - Implement current() to return current token without consuming
  - Implement peek(n) to return nth lookahead token (0 = current)
  - Implement consume() to advance to next token
  - Add check() method to test token kind without consuming
  - Add consume_if() for conditional token consumption
  - _Requirements: 1.1, 1.2, 1.3, 1.5_

---

## Task 3: Core Parser Engine

- [ ] 3.1 Implement Parser struct with basic parsing loop
  - Create Parser struct with fields: lexer, buffer, errors, mode, recursion_depth
  - Implement new() constructor that initializes TokenBuffer
  - Implement with_mode() for mode configuration
  - Create parse() method for full input parsing
  - Create parse_statement() for single statement parsing
  - Add errors() and drain_errors() methods for error access
  - _Requirements: 1.1, 1.4_

- [ ] 3.2 Implement statement dispatcher
  - Create parse_statement_internal() that routes based on keyword
  - Add keyword detection logic for SELECT, INSERT, UPDATE, DELETE, CREATE, DECLARE, SET, IF, WHILE, BEGIN, etc.
  - Handle unknown keywords with UnexpectedToken error
  - Implement statement termination detection (semicolon or keyword boundary)
  - _Requirements: 1.1_

- [ ] 3.3 Implement recursion depth tracking
  - Add depth counter to Parser struct
  - Create RecursionLimitExceeded error variant
  - Check depth before recursive calls
  - Implement depth guard pattern for all recursive methods
  - Set limit to 1000 levels
  - _Requirements: 13.2_

---

## Task 4: Expression Parser (Pratt Algorithm)

- [ ] 4.1 Implement BindingPower enum and precedence table
  - Define BindingPower enum with levels: Lowest, LogicalOr, LogicalAnd, Comparison, Is, Additive, Multiplicative, Unary, Primary
  - Create operator precedence mapping using once_cell Lazy<HashMap>
  - Define left/right binding powers for infix operators
  - Define prefix binding powers for unary operators
  - _Requirements: 9.1, 9.2, 9.3_

- [ ] 4.2 Implement primary expression parsing
  - Create parse_primary() method for literals, identifiers, and parentheses
  - Handle string, number, boolean, NULL literals
  - Handle column references with optional table qualification
  - Handle parenthesized sub-expressions
  - _Requirements: 9.6, 9.7, 9.9_

- [ ] 4.3 Implement prefix operator parsing (null denotation)
  - Create parse_prefix() method for unary operators
  - Handle +, -, ~, NOT prefix operators
  - Delegate to parse_primary() for non-prefix tokens
  - Create appropriate AST nodes (UnaryOp)
  - _Requirements: 9.1, 9.3_

- [ ] 4.4 Implement infix operator parsing (left denotation)
  - Create parse_infix() method using Pratt parser algorithm
  - Handle all arithmetic operators: +, -, *, /, %
  - Handle all comparison operators: =, <>, !=, <, >, <=, >=, !<, !>
  - Handle logical operators: AND, OR
  - Handle concatenation operator: ||
  - Create BinaryOp AST nodes with proper structure
  - _Requirements: 9.1, 9.2, 9.3_

- [ ] 4.5 Implement function call parsing
  - Create parse_function_call() method
  - Detect function name followed by parentheses
  - Parse argument list (comma-separated expressions)
  - Handle DISTINCT keyword for aggregate functions
  - Support wildcard arguments for COUNT(*)
  - Create FunctionCall AST node
  - _Requirements: 9.4_

- [ ] 4.6 Implement CASE expression parsing
  - Create parse_case_expression() method
  - Handle CASE WHEN...THEN...ELSE...END syntax
  - Parse multiple WHEN branches
  - Handle optional ELSE clause
  - Create CaseExpression AST node with branches vector
  - _Requirements: 9.5_

- [ ] 4.7 Implement special expression types
  - Create parse_subquery_expression() for scalar subqueries
  - Create parse_exists_expression() for EXISTS subqueries
  - Create parse_in_expression() for IN lists and subqueries
  - Create parse_between_expression() for BETWEEN ranges
  - Create parse_like_expression() for LIKE patterns
  - Create parse_is_expression() for IS NULL/TRUE/FALSE
  - Handle negated forms (NOT IN, NOT LIKE, IS NOT NULL)
  - _Requirements: 9.8, 9.9, 9.10, 9.11, 9.12_

---

## Task 5: SELECT Statement Parser

- [ ] 5.1 Implement SELECT clause parsing
  - Create parse_select_statement() method
  - Handle DISTINCT keyword
  - Handle TOP N clause
  - Parse column list with SelectItem variants (expression, wildcard, qualified wildcard)
  - Support column aliases (AS keyword or implicit)
  - _Requirements: 2.1, 2.2, 2.8, 2.9_

- [ ] 5.2 Implement FROM clause parsing
  - Create parse_from_clause() method
  - Parse table references with optional aliases
  - Handle subqueries in FROM clause (derived tables)
  - Support AS keyword for aliases
  - Create FromClause AST node
  - _Requirements: 2.3_

- [ ] 5.3 Implement JOIN parsing
  - Create parse_join() method for individual JOIN clauses
  - Parse all JOIN types: INNER, LEFT/LEFT OUTER, RIGHT/RIGHT OUTER, FULL/FULL OUTER, CROSS
  - Handle ON clause with condition expression
  - Handle USING clause with column list
  - Create Join AST node with type, table, and condition
  - Implement parse_joins() to collect multiple joins
  - _Requirements: 2.5, 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 10.8_

- [ ] 5.4 Implement WHERE clause parsing
  - Create parse_where_clause() method
  - Parse condition expression using expression parser
  - Create Optional<Expression> field in SelectStatement
  - _Requirements: 2.4_

- [ ] 5.5 Implement GROUP BY and HAVING parsing
  - Create parse_group_by_clause() method
  - Parse group key expression list
  - Create parse_having_clause() method for HAVING condition
  - _Requirements: 2.6_

- [ ] 5.6 Implement ORDER BY and LIMIT parsing
  - Create parse_order_by_clause() method
  - Parse ordering expressions with ASC/DESC direction
  - Handle LIMIT/OFFSET clauses if present (non-standard but common)
  - _Requirements: 2.7_

---

## Task 6: DML Statement Parsers (INSERT, UPDATE, DELETE)

- [ ] 6.1 (P) Implement INSERT statement parser
  - Create parse_insert_statement() method
  - Parse INSERT INTO with target table name
  - Parse optional column list in parentheses
  - Handle VALUES clause with row value lists
  - Handle INSERT-SELECT form
  - Handle DEFAULT VALUES clause
  - Create InsertStatement AST node
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6_

- [ ] 6.2 (P) Implement UPDATE statement parser
  - Create parse_update_statement() method
  - Parse UPDATE with target table
  - Parse SET clause with Assignment list (column = expression)
  - Handle multiple comma-separated assignments
  - Parse optional FROM clause (ASE-specific)
  - Parse optional WHERE clause
  - Create UpdateStatement AST node
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_

- [ ] 6.3 (P) Implement DELETE statement parser
  - Create parse_delete_statement() method
  - Parse DELETE FROM with target table
  - Parse optional WHERE clause
  - Set warning flag when WHERE clause is absent
  - Create DeleteStatement AST node
  - _Requirements: 5.1, 5.2, 5.3, 5.4_

---

## Task 7: DDL Statement Parser

- [ ] 7.1 Implement CREATE TABLE parser
  - Create parse_create_table() method
  - Parse table name and temporary flag (# and ## prefixes)
  - Parse column definitions in parentheses
  - Parse table constraints (PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK)
  - Create CreateTableStatement AST node
  - _Requirements: 6.1, 6.2, 6.3, 12.1, 12.2_

- [ ] 7.2 Implement CREATE INDEX parser
  - Create parse_create_index() method
  - Parse index name, table name, and column list
  - Handle UNIQUE constraint option
  - Create CreateIndexStatement AST node
  - _Requirements: 6.4_

- [ ] 7.3 Implement CREATE VIEW parser
  - Create parse_create_view() method
  - Parse view name and SELECT query
  - Create CreateViewStatement AST node
  - _Requirements: 6.5_

- [ ] 7.4 Implement CREATE PROCEDURE parser
  - Create parse_create_procedure() method
  - Parse procedure name
  - Parse optional parameters
  - Parse procedure body (AS block with statements)
  - Create CreateProcedureStatement AST node
  - _Requirements: 6.6_

---

## Task 8: Variable and Control Flow Parsers

- [ ] 8.1 (P) Implement DECLARE statement parser
  - Create parse_declare_statement() method
  - Parse @variable_name identifiers
  - Parse data types for each variable
  - Handle comma-separated multiple variable declarations
  - Parse optional DEFAULT values
  - Create DeclareStatement AST node
  - _Requirements: 7.1, 7.2, 7.5_

- [ ] 8.2 (P) Implement SET statement parser
  - Create parse_set_statement() method
  - Parse SET @variable = expression syntax
  - Create SetStatement AST node
  - _Requirements: 7.3_

- [ ] 8.3 Implement SELECT variable assignment parser
  - Extend expression parser to handle SELECT @var = expr syntax
  - Distinguish from regular SELECT statements
  - Create VariableAssignment AST node
  - _Requirements: 7.4_

- [ ] 8.4 Implement IF...ELSE statement parser
  - Create parse_if_statement() method
  - Parse IF condition expression
  - Parse THEN branch (can be statement or block)
  - Parse optional ELSE branch
  - Create IfStatement AST node
  - _Requirements: 8.1, 8.2_

- [ ] 8.5 Implement WHILE statement parser
  - Create parse_while_statement() method
  - Parse WHILE condition expression
  - Parse loop body (statement or block)
  - Create WhileStatement AST node
  - _Requirements: 8.3_

- [ ] 8.6 Implement BEGIN...END block parser
  - Create parse_block() method
  - Parse BEGIN keyword
  - Parse inner statement list
  - Parse END keyword
  - Create Block AST node
  - _Requirements: 8.6_

- [ ] 8.7 Implement BREAK, CONTINUE, RETURN parsers
  - Create parse_break_statement() method
  - Create parse_continue_statement() method
  - Create parse_return_statement() method with optional expression
  - _Requirements: 8.4, 8.5, 8.7_

---

## Task 9: Data Type Parser

- [ ] 9.1 Implement data type parser
  - Create parse_data_type() method
  - Parse integer types: INT, INTEGER, SMALLINT, TINYINT, BIGINT
  - Parse string types: VARCHAR(n), CHAR(n), TEXT
  - Parse decimal types: DECIMAL(p,s), NUMERIC(p,s)
  - Parse float types: FLOAT, REAL, DOUBLE
  - Parse date/time types: DATE, TIME, DATETIME, TIMESTAMP
  - Parse bit type: BIT
  - Handle NULL/NOT NULL constraints
  - Handle IDENTITY/AUTOINCREMENT property
  - Create DataType AST node variants
  - _Requirements: 15.1, 15.2, 15.3, 15.4, 15.5, 15.6, 15.7_

---

## Task 10: Batch Processing and GO Separator

- [ ] 10.1 Implement GO keyword detection
  - Create is_go_keyword() method with line position checking
  - Verify GO is alone on line (ignoring whitespace)
  - Exclude GO inside strings and comments
  - Exclude GO as part of identifier (e.g., GO_HOME)
  - _Requirements: 16.5, 16.6_

- [ ] 10.2 Implement batch processing logic
  - Create parse_batches() method for batch mode
  - Parse GO N repeat count syntax
  - Create BatchSeparator AST node with repeat count
  - Collect statements into Batch AST nodes
  - Handle empty batches (consecutive GO)
  - Create BatchList result type
  - _Requirements: 16.1, 16.2, 16.3, 16.4_

- [ ] 10.3 Implement single statement mode
  - Modify parse() to respect SingleStatement mode
  - Treat GO as regular identifier in single statement mode
  - Return single Statement instead of BatchList
  - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5, 18.1, 18.2, 18.4, 18.5_

---

## Task 11: Error Recovery and Synchronization

- [ ] 11.1 Implement synchronization point detection
  - Define is_synchronization_point() function
  - Recognize: semicolon, SELECT, INSERT, UPDATE, DELETE, CREATE, END, GO keywords
  - _Requirements: 13.4_

- [ ] 11.2 Implement panic mode error recovery
  - Create synchronize() method
  - Skip tokens until synchronization point found
  - Record errors before skipping
  - Continue parsing after recovery
  - _Requirements: 13.4, 13.5_

- [ ] 11.3 Implement error collection and reporting
  - Maintain Vec<ParseError> in Parser struct
  - Add source code snippet extraction for error positions
  - Format error messages with expected/actual tokens
  - Implement batch-specific error wrapping (BatchError)
  - _Requirements: 13.1, 13.2, 13.3, 13.5, 13.6, 19.1, 19.3, 19.4, 19.5_

---

## Task 12: Temp Table and Subquery Support

- [ ] 12.1 Implement temporary table reference detection
  - Modify table reference parser to recognize #temp and ##global_temp syntax
  - Create TempTableReference AST node
  - Set scope information in AST
  - _Requirements: 12.1, 12.2, 12.3, 12.4_

- [ ] 12.2 Implement subquery parsing integration
  - Handle scalar subqueries in expression lists
  - Handle derived tables in FROM clause
  - Handle EXISTS and IN subqueries
  - Assign subquery aliases when present
  - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5_

---

## Task 13: Common SQL AST Integration

- [ ] 13.1 Define ToCommonAst trait
  - Create trait with to_common_ast() method
  - Define Result type for conversion
  - Add trait to ast module
  - _Requirements: 14.1_

- [ ] 13.2 Implement ToCommonAst for core statement types
  - Implement trait for SelectStatement
  - Implement trait for InsertStatement, UpdateStatement, DeleteStatement
  - Handle conversion failures with DialectSpecific variant
  - _Requirements: 14.1, 14.3_

- [ ] 13.3 Implement ToCommonAst for expressions
  - Implement trait for Expression enum
  - Handle ASE-specific operators
  - Preserve source span through conversion
  - _Requirements: 14.2, 14.3, 14.4_

---

## Task 14: AST Node Definitions

- [ ] 14.1 Define Expression AST nodes
  - Create Expression enum with all variants
  - Define Literal enum (String, Number, Float, Hex, Null, Boolean)
  - Define Identifier struct with name and span
  - Define ColumnReference with optional table qualifier
  - Define UnaryOperator and BinaryOperator enums
  - Define FunctionCall struct with args vector
  - Define CaseExpression with branches vector
  - _Requirements: 9.6, 9.7, 9.8, 9.10, 9.11, 9.12_

- [ ] 14.2 Define Statement AST nodes
  - Define SelectStatement with all clauses
  - Define InsertStatement with source enum
  - Define UpdateStatement with assignments
  - Define DeleteStatement
  - Define CreateStatement enum variants
  - Define DeclareStatement, SetStatement
  - Define IfStatement, WhileStatement, Block
  - Define BreakStatement, ContinueStatement, ReturnStatement
  - _Requirements: 2.1, 3.1, 4.1, 5.1, 6.1, 7.1, 8.1_

- [ ] 14.3 Define JOIN and FROM AST nodes
  - Define JoinType enum (Inner, Left, LeftOuter, Right, RightOuter, Full, FullOuter, Cross)
  - Define Join struct with type, table, condition, using columns
  - Define FromClause with tables and joins
  - Define TableReference enum (Table, Subquery, Joined)
  - Define SelectItem enum (Expression with alias, Wildcard, QualifiedWildcard)
  - _Requirements: 10.1, 10.6, 10.7, 10.8_

---

## Task 15: Public API and Library Interface

- [ ] 15.1 Design and implement public API
  - Export Parser struct and key types from lib.rs
  - Export Statement, Expression, and other AST nodes
  - Export ParseError and ParseResult types
  - Export ParserMode enum for configuration
  - Add documentation examples to public API
  - _Requirements: 1.1, 14.1_

---

## Task 16: Unit Tests - Token Buffer and Core

- [ ] 16.1 (P) Write TokenBuffer unit tests
  - Test current() returns current token
  - Test peek(n) returns lookahead tokens
  - Test consume() advances buffer correctly
  - Test consume_if() conditional consumption
  - Test EOF handling
  - Test buffer refill when exhausted
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5_

- [ ] 16.2 (P) Write error type unit tests
  - Test UnexpectedToken error creation
  - Test UnexpectedEOF error creation
  - Test error Display formatting
  - Test error span information
  - _Requirements: 13.1, 13.2, 13.3_

---

## Task 17: Unit Tests - Expression Parser

- [ ] 17.1 Write expression operator precedence tests
  - Test arithmetic precedence (*, /, % vs +, -)
  - Test comparison precedence vs arithmetic
  - Test logical precedence (NOT > AND > OR)
  - Test parentheses override precedence
  - Test operator associativity
  - _Requirements: 9.1, 9.2, 9.3_

- [ ] 17.2 Write function call and special expression tests
  - Test function calls with multiple arguments
  - Test aggregate functions with DISTINCT
  - Test CASE expressions with multiple branches
  - Test IN with value list and subquery
  - Test LIKE with pattern and escape
  - Test EXISTS subquery
  - Test BETWEEN expressions
  - Test IS NULL/TRUE/FALSE
  - _Requirements: 9.4, 9.5, 9.8, 9.9, 9.10, 9.11, 9.12_

---

## Task 18: Unit Tests - Statement Parsers

- [ ] 18.1 (P) Write SELECT statement tests
  - Test simple SELECT with columns
  - Test SELECT with DISTINCT
  - Test SELECT with TOP N
  - Test SELECT with FROM clause
  - Test SELECT with WHERE clause
  - Test SELECT with JOIN
  - Test SELECT with GROUP BY and HAVING
  - Test SELECT with ORDER BY
  - Test subqueries in FROM clause
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9, 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7, 10.8, 11.2_

- [ ] 18.2 (P) Write DML statement tests
  - Test INSERT with VALUES
  - Test INSERT with column list
  - Test INSERT-SELECT
  - Test INSERT DEFAULT VALUES
  - Test UPDATE with SET clause
  - Test UPDATE with FROM clause (ASE-specific)
  - Test DELETE with and without WHERE
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 5.1, 5.2, 5.3, 5.4_

- [ ] 18.3 (P) Write DDL and control flow tests
  - Test CREATE TABLE with columns
  - Test CREATE TABLE with constraints
  - Test CREATE INDEX
  - Test CREATE VIEW
  - Test DECLARE statements
  - Test SET variable assignment
  - Test IF...ELSE statements
  - Test WHILE loops
  - Test BEGIN...END blocks
  - Test BREAK, CONTINUE, RETURN
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 7.1, 7.2, 7.3, 7.4, 7.5, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 15.1, 15.2, 15.3, 15.4, 15.5, 15.6, 15.7_

---

## Task 19: Unit Tests - Batch Processing

- [ ] 19.1 Write batch processing tests
  - Test GO keyword detection at line start
  - Test GO not detected in strings/comments
  - Test GO not detected as part of identifier
  - Test GO N repeat count parsing
  - Test multiple batches in single input
  - Test empty batches
  - Test single statement mode (non-GO)
  - Test mode switching
  - _Requirements: 16.1, 16.2, 16.3, 16.4, 16.5, 16.6, 17.1, 17.2, 17.3, 17.4, 17.5, 18.1, 18.2, 18.3, 18.4, 18.5_

---

## Task 20: Unit Tests - Error Recovery

- [ ] 20.1 Write error recovery tests
  - Test unexpected token error
  - Test unexpected EOF error
  - Test synchronization at semicolon
  - Test synchronization at keywords
  - Test multiple errors in single batch
  - Test batch-specific error reporting
  - Test error position reporting
  - Test source code snippet extraction
  - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5, 13.6, 19.1, 19.2, 19.3, 19.4, 19.5_

---

## Task 21: Unit Tests - Temp Tables and Data Types

- [ ] 21.1 Write temporary table and data type tests
  - Test #temp table reference
  - Test ##global_temp table reference
  - Test temporary table in all DML statements
  - Test integer data types
  - Test string data types with length
  - Test decimal/numeric with precision and scale
  - Test NULL/NOT NULL constraints
  - Test IDENTITY property
  - _Requirements: 12.1, 12.2, 12.3, 12.4, 15.1, 15.2, 15.3, 15.4, 15.5, 15.6, 15.7_

---

## Task 22: Unit Tests - Common SQL AST Conversion

- [ ] 22.1 Write ToCommonAst conversion tests
  - Test SelectStatement conversion
  - Test InsertStatement conversion
  - Test Expression conversion
  - Test DialectSpecific handling for ASE features
  - Test span preservation through conversion
  - _Requirements: 14.1, 14.2, 14.3, 14.4_

---

## Task 23: Integration Tests

- [ ] 23.1 Create integration test suite
  - Set up tests/ directory with test modules
  - Test full SQL file parsing
  - Test complex multi-join queries
  - Test nested subqueries
  - Test deep expression nesting
  - Test large input files (performance)
  - Test error recovery across multiple statements
  - _Requirements: All_

---

## Task 24: Performance Benchmarks

- [ ] 24.1 Create criterion benchmark suite
  - Set up benches/ directory
  - Benchmark 1MB SQL file parsing (target: <=500ms)
  - Benchmark 100MB SQL file parsing (target: <=60s)
  - Benchmark expression parsing
  - Benchmark SELECT statement parsing
  - Measure memory usage
  - _Requirements: NFR-001-01, NFR-001-02_

---

## Task 25: Documentation and Examples

- [ ] 25.1 Write API documentation
  - Add rustdoc comments to all public types
  - Add module-level documentation
  - Add usage examples to lib.rs
  - Document error handling patterns
  - _Requirements: NFR-003-03_

---

## Task 26: Final Integration and Validation

- [ ] 26.1 Perform final integration and quality checks
  - Run full test suite with coverage measurement (target: >=80% overall, >=90% core)
  - Run clippy with -D warnings (target: 0 warnings)
  - Run rustfmt --check (target: pass)
  - Run cargo doc --no-deps (target: no errors)
  - Verify all requirements are covered
  - Create example SQL files demonstrating parser capabilities
  - _Requirements: All, NFR-002-01, NFR-003-01, NFR-004-01, NFR-004-02, NFR-004-03, NFR-005-01, NFR-005-02, NFR-005-03, NFR-006-01, NFR-006-02, NFR-006-03, NFR-006-04_

---

## Requirements Coverage Matrix

| Requirement | Tasks | Status |
|-------------|-------|--------|
| 1. Lexer Integration | 1.1, 2.1, 2.2, 3.1, 3.2, 3.3, 16.1 | Covered |
| 2. SELECT Statement | 4.2-4.7, 5.1-5.6, 18.1 | Covered |
| 3. INSERT Statement | 6.1, 18.2 | Covered |
| 4. UPDATE Statement | 6.2, 18.2 | Covered |
| 5. DELETE Statement | 6.3, 18.2 | Covered |
| 6. CREATE Statement | 7.1-7.4, 18.3 | Covered |
| 7. Variables | 8.1-8.3, 18.3 | Covered |
| 8. Control Flow | 8.4-8.7, 18.3 | Covered |
| 9. Expression Parsing | 4.1-4.7, 14.1, 17.1, 17.2 | Covered |
| 10. JOIN Syntax | 5.3, 14.3, 18.1 | Covered |
| 11. Subquery | 5.2, 5.6, 12.2, 14.1 | Covered |
| 12. Temp Tables | 7.1, 12.1, 21.1 | Covered |
| 13. Error Handling | 1.2, 3.3, 11.1-11.3, 16.2, 20.1 | Covered |
| 14. Common SQL AST | 1.4, 13.1-13.3, 22.1 | Covered |
| 15. Data Types | 9.1, 18.3, 21.1 | Covered |
| 16. GO Batch Separator | 10.1, 10.2, 19.1 | Covered |
| 17. Non-GO SQL | 10.3, 19.1 | Covered |
| 18. Mode Switching | 1.3, 3.1, 10.3, 19.1 | Covered |
| 19. GO Error Handling | 10.2, 11.3, 20.1 | Covered |
| **All Functional** | **All 19 requirements** | **100%** |
| **All NFRs** | **24, 26.1** | **Covered** |

---

## Dependencies and Parallel Execution Notes

### Parallel-Capable Tasks (P)
Tasks marked with `(P)` can be executed concurrently as they:
- Operate on separate files/modules
- Have no data dependencies on other pending tasks
- Share no mutable resources

### Sequential Dependencies
- Task 2 must complete before Task 3 (TokenBuffer needed by Parser)
- Task 3 must complete before Tasks 4-13 (Parser core needed by all parsers)
- Task 4 must complete before Tasks 5-6 (Expression parser needed by statement parsers)
- Tasks 7-13 can partially parallelize after their prerequisites
- Task 14 must complete before Task 15
- Tasks 16-22 (testing) can parallelize after corresponding implementation
- Task 23-26 must execute sequentially at the end

### Suggested Execution Order
1. Group 0: Tasks 1.x, 2.x, 14.x (setup and AST)
2. Group 1: Tasks 3.x, 4.x (core parser and expressions)
3. Group 2: Tasks 5.x, 6.1-6.3, 7.x, 8.x, 9.x, 10.x, 11.x (statement parsers - partial parallel)
4. Group 3: Tasks 12.x, 13.x, 15.x (special features and API)
5. Group 4: Tasks 16.x-22.x (tests - can parallelize)
6. Group 5: Tasks 23.x-26.x (integration and validation - sequential)
