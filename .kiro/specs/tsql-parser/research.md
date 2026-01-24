# Research & Design Decisions: T-SQL Parser for SAP ASE

---

## Summary
- **Feature**: tsql-parser
- **Discovery Scope**: New Feature (greenfield parser implementation)
- **Key Findings**:
  - Recursive descent parser with Pratt parsing for expressions provides the best balance of maintainability and performance
  - Token lookahead buffer of 3+ tokens is sufficient for T-SQL grammar disambiguation
  - Error recovery via synchronization points enables multi-error reporting per batch
  - Zero-copy design from Lexer can be preserved through lifetime-aware AST nodes

---

## Research Log

### Topic 1: Parser Architecture Selection
- **Context**: Need to choose between recursive descent, parser combinators, and Pratt parser for expression parsing
- **Sources Consulted**:
  - "Parsing Techniques: A Practical Guide" (Dick Grune)
  - sqlparser-rs (Rust SQL parser implementation)
  - postgresql parser implementation patterns
- **Findings**:
  - **Recursive descent**: Best for statement-level parsing (SELECT, INSERT, etc.); intuitive to implement and debug
  - **Pratt parser**: Optimal for expression parsing with operator precedence; handles binary/unary operators elegantly
  - **Parser combinators** (nom/pest): Powerful but harder to debug; error recovery is complex
  - **Hybrid approach**: Recursive descent for statements + Pratt parser for expressions is industry standard for SQL parsers
- **Implications**:
  - Statement parsing uses recursive descent methods (parse_select, parse_insert, etc.)
  - Expression parsing uses Pratt parser with precedence climbing
  - Error recovery can be implemented at statement boundaries

### Topic 2: T-SQL Grammar Quirks (SAP ASE)
- **Context**: SAP ASE has unique syntax requiring special handling
- **Sources Consulted**:
  - SAP ASE 16.0 Reference Manual
  - Transact-SQL syntax documentation
  - Comparison with Microsoft SQL Server T-SQL
- **Findings**:
  - **GO batch separator**: Only recognized when alone on a line (not as identifier like "GO_HOME")
  - **Variable assignment**: Supports both `SET @var = expr` and `SELECT @var = expr`
  - **Temp tables**: `#temp` (local) and `##global_temp` with scoping rules
  - **Bracket identifiers**: `[identifier]` with `]]` escape sequence
  - **UPDATE with FROM**: ASE-specific syntax `UPDATE t SET ... FROM t JOIN ...`
  - **TOP N**: `SELECT TOP N ...` (not `LIMIT` like MySQL)
- **Implications**:
  - Lexer must provide context for GO keyword (line position)
  - Parser needs special mode for variable assignment parsing
  - Temp table references need special AST nodes for scoping

### Topic 3: Expression Operator Precedence
- **Context**: Need to define correct operator precedence for T-SQL expressions
- **Sources Consulted**:
  - SAP ASE Operator Precedence Documentation
  - SQL:2016 Standard
- **Findings**:
  - **Precedence levels (highest to lowest)**:
    1. Unary operators: `+`, `-`, `~`, `NOT`
    2. Multiplicative: `*`, `/`, `%`
    3. Additive: `+`, `-`, `||` (concat)
    4. Comparison: `=`, `<>`, `!=`, `<`, `>`, `<=`, `>=`, `!<`, `!>`
    5. `BETWEEN`, `IN`, `LIKE`, `IS`
    6. `AND`
    7. `OR`
  - All operators are left-associative except `^` (power) which is right-associative
- **Implications**:
  - Pratt parser binding powers: 1-7 corresponding to precedence levels
  - Right-associative operators need special handling in null denotation

### Topic 4: Error Recovery Strategy
- **Context**: Need to parse multiple statements per batch even with syntax errors
- **Sources Consulted**:
  - "Crafting a Compiler" (Charles Fisher)
  - PostgreSQL error recovery patterns
  - GCC error recovery implementation
- **Findings**:
  - **Synchronization points** for T-SQL:
    - Semicolon `;`
    - Keywords: `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `ALTER`, `DROP`, `EXEC`
    - `END` (for blocks)
    - `GO` (batch separator)
  - **Panic mode recovery**: Skip tokens until synchronization point, then resume
  - **Error collection**: Continue parsing to find additional errors
- **Implications**:
  - Parser maintains error vector
  - Each statement-level parse function returns `Result<Statement, ParseError>`
  - Batch parsing continues even after individual statement errors

### Topic 5: AST Ownership and Lifetime Design
- **Context**: Lexer uses zero-copy design with references to source; need to decide AST ownership
- **Sources Consulted**:
  - Rust ownership and lifetime documentation
  - sqlparser-rs AST design
  - serde serialization patterns
- **Findings**:
  - **Option A (Lifetime-aware AST)**: `AstNode<'a>` references source directly
    - Pros: Zero-copy, minimal memory
    - Cons: Complex lifetimes, harder to serialize
  - **Option B (Owned AST)**: `String`-owned literals in AST nodes
    - Pros: Simple ownership, easy serialization, thread-safe
    - Cons: Memory allocation for each identifier/literal
  - **Option C (Hybrid)**: Cow<str> for flexible ownership
    - Pros: Best of both worlds
    - Cons: Slightly more complex API
- **Selected Approach**: Option B (Owned AST) for initial implementation
  - Simplifies API significantly
  - Memory overhead is acceptable given typical SQL file sizes
  - Enables easy serialization and cross-thread passing
  - Can optimize to Cow<str> later if profiling shows need

### Topic 6: Common SQL AST Integration
- **Context**: Need to define interface for dialect-specific AST to common AST conversion
- **Sources Consulted**:
  - Apache Calcite architecture (relational algebra representation)
  - SQL Glue (SQL query abstraction layer)
- **Findings**:
  - **Separation of concerns**: Dialect-specific features should be preserved, not lost in conversion
  - **Visitor pattern**: Enables traversing AST without coupling to concrete node types
  - **Trait-based conversion**: `TryInto<CommonSqlNode>` for convertible nodes
- **Implications**:
  - AST nodes implement `ToCommonAst` trait
  - ASE-specific nodes wrapped in `DialectSpecific` variant
  - Source span preserved through all conversions

### Topic 7: GO Batch Processing
- **Context**: SAP ASE uses GO keyword as batch separator; different from standard SQL
- **Sources Consulted**:
  - SAP ASE utility command documentation
  - sqlcmd and isql behavior
- **Findings**:
  - **GO N syntax**: Repeats batch N times (execution-time feature, not parsing)
  - **GO recognition rules**:
    - Must be at start of line (ignoring whitespace)
    - Cannot be part of identifier (e.g., "GO_HOME" is identifier)
    - Cannot be inside string or comment
    - Case-insensitive
  - **Non-GO mode**: Some tools disable GO as separator
- **Implications**:
  - Parser mode: `BatchMode` vs `SingleStatementMode`
  - Lexer provides line start position for GO detection
  - Batch repeat count stored in AST node (execution concern)

---

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| **Recursive Descent + Pratt** | Top-down parsing with precedence climbing for expressions | Clear structure, easy debug, good error recovery | Manual parser implementation | Industry standard for SQL |
| Parser Combinators (nom/pest) | Declarative parsing using combinator functions | Concise code, composable | Complex error recovery, hard to debug | Not chosen for this project |
| Table-Driven (LL(k)) | Generic engine with parse tables | Compact grammar representation | Complex tooling, poor errors | Overkill for T-SQL |
| Handwritten LR | Bottom-up parsing with shift-reduce | Handles all CFG grammars | Very complex, poor error messages | Not suitable |

**Selected**: Recursive Descent + Pratt Parser
- Rationale: Best balance of maintainability, debuggability, and error handling
- Alignment: Follows patterns from sqlparser-rs and PostgreSQL

---

## Design Decisions

### Decision 1: AST Ownership Model
- **Context**: Need to balance memory efficiency with API simplicity
- **Alternatives Considered**:
  1. Lifetime-aware references (`&'a str`)
  2. Fully owned strings (`String`)
  3. Borrowed or owned (`Cow<'a, str>`)
- **Selected Approach**: Fully owned strings (`String`)
- **Rationale**:
  - Simplifies API (no lifetime parameters on parser)
  - Enables easy serialization and cloning
  - Acceptable memory overhead for typical SQL files (<10MB)
  - Can optimize later if profiling shows bottleneck
- **Trade-offs**:
  - (+) Simple API, no lifetime annotations
  - (+) Thread-safe by default
  - (+) Easy to serialize/deserialize
  - (-) More memory allocations
  - (-) Potential for slower parsing
- **Follow-up**: Benchmark with large SQL files; if memory >3x input size, consider Cow<str>

### Decision 2: Expression Parsing Strategy
- **Context**: Expressions have complex precedence and associativity rules
- **Alternatives Considered**:
  1. Pure recursive descent with precedence methods
  2. Pratt parser (precedence climbing)
  3. Shunting-yard algorithm
- **Selected Approach**: Pratt parser
- **Rationale**:
  - Elegant handling of infix, prefix, postfix operators
  - Easy to add new operators
  - Proven pattern in sqlparser-rs
- **Trade-offs**:
  - (+) Concise implementation
  - (+) Extensible for new operators
  - (+) Natural handling of precedence
  - (-) Slightly higher learning curve
- **Follow-up**: Create comprehensive expression tests covering all precedence levels

### Decision 3: Error Recovery Granularity
- **Context**: Need to balance error quality with recovery capability
- **Alternatives Considered**:
  1. Stop at first error
  2. Panic mode with statement-level recovery
  3. Full synchronized recovery with error correction
- **Selected Approach**: Panic mode with statement-level recovery
- **Rationale**:
  - Most errors are localized to single statement
  - Statement boundaries provide natural synchronization points
  - Continues parsing to find more errors
- **Trade-offs**:
  - (+) Finds multiple errors per parse
  - (+) Predictable recovery behavior
  - (-) May produce cascading errors
  - (-) Limited error correction
- **Follow-up**: Monitor cascading error rate; add heuristics if >30% errors cascade

### Decision 4: Token Lookahead Buffer Size
- **Context**: Need sufficient lookahead for ambiguity resolution
- **Alternatives Considered**:
  1. Single token (LL(1))
  2. Two tokens (LL(2))
  3. Three tokens (LL(3))
  4. Unlimited backtrack
- **Selected Approach**: Three token lookahead buffer
- **Rationale**:
  - LL(3) sufficient for T-SSQL grammar ambiguities
  - `SELECT (SELECT ...)` needs 2 tokens for subquery detection
  - `CREATE TABLE ...` vs `CREATE INDEX ...` needs 2 tokens
  - Fixed size enables efficient circular buffer
- **Trade-offs**:
  - (+) Predictable memory usage
  - (+) Covers all T-SQL ambiguities
  - (+) Simple implementation
  - (-) Not suitable for grammars needing more lookahead
- **Follow-up**: Verify during testing; if grammar needs more, extend to LL(4)

### Decision 5: Common SQL AST Integration
- **Context**: Need to interface with future common-sql crate
- **Alternatives Considered**:
  1. Direct conversion to common AST
  2. Trait-based conversion interface
  3. Separate AST transformation pass
- **Selected Approach**: Trait-based conversion interface
- **Rationale**:
  - Decouples parser from common AST design
  - Allows incremental implementation
  - Preserves dialect-specific information
- **Trade-offs**:
  - (+) Flexible architecture
  - (+) Can evolve independently
  - (+) Dialect features preserved
  - (-) Additional conversion step
  - (-) More boilerplate code
- **Follow-up**: Define `ToCommonAst` trait early for interface stability

---

## Risks & Mitigations

### Risk 1: Expression Precedence Bug Complexity
- **Probability**: Medium
- **Impact**: High (incorrect parsing)
- **Mitigation**: Comprehensive table-driven tests for all operator combinations
- **Detection**: Property-based testing with proptest

### Risk 2: Stack Overflow on Deeply Nested Expressions
- **Probability**: Low
- **Impact**: Medium (crash)
- **Mitigation**: Implement recursion depth limit (1000 levels)
- **Detection**: Fuzz testing with deeply nested inputs

### Risk 3: GO Keyword False Positives
- **Probability**: Medium
- **Impact**: High (incorrect batch separation)
- **Mitigation**: Strict GO detection rules (line start, not in string/comment)
- **Detection**: Regression tests with edge cases

### Risk 4: Memory Blowout on Large Inputs
- **Probability**: Low
- **Impact**: High (OOM)
- **Mitigation**: Input size limit (100MB), memory profiling
- **Detection**: Benchmark with 10MB+ SQL files

### Risk 5: Ambiguous Grammar for ASE-Specific Syntax
- **Probability**: Medium
- **Impact**: Medium (parse errors)
- **Mitigation**: Document all ASE-specific parsing rules, reference ASE manual
- **Detection**: Test with real SAP ASE SQL scripts

---

## References
- [SAP ASE 16.0 Transact-SQL User's Guide](https://help.sap.com/docs/SAP_ASE)
- [sqlparser-rs GitHub Repository](https://github.com/sqlparser-rs/sqlparser-rs) - Reference implementation
- "Crafting a Compiler" by Charles Fischer - Error recovery strategies
- "Parsing Techniques: A Practical Guide" by Dick Grune - Parser architecture patterns
- [PostgreSQL Parser Implementation](https://github.com/postgres/postgres/tree/master/src/backend/parser) - Reference for recursive descent patterns
