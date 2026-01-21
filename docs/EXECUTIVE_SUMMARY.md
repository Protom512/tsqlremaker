# SAP ASE T-SQL Dialect - Executive Summary

**Date:** January 19, 2026  
**Project:** tsqlremaker (Rust T-SQL Lexer/Parser)  
**Target:** SAP ASE (Sybase Adaptive Server Enterprise)

---

## Critical Findings

### 1. Unique SAP ASE Features (Lexer Impact)

| Feature | Syntax | Lexer Requirement | Priority |
|---------|--------|-------------------|----------|
| **Nested Comments** | `/* /* nested */ */` | Stack-based depth tracking | **HIGH** |
| **Local Variables** | `@variable` | Single `@` prefix detection | **HIGH** |
| **Global Variables** | `@@error`, `@@identity` | Double `@@` prefix detection | **HIGH** |
| **Local Temp Tables** | `#temp_table` | Single `#` prefix detection | **HIGH** |
| **Global Temp Tables** | `##global_temp` | Double `##` prefix detection | **MEDIUM** |
| **Unicode Escapes** | `U&'\xxxx'` | Multi-char prefix `U&` + escape parsing | **MEDIUM** |
| **String Concat** | `'a' + 'b'` or `'a' \|\| 'b'` | Both `+` and `\|\|` operators | **MEDIUM** |
| **Line Comments** | `-- comment` | `--` prefix, newline termination | **HIGH** |

### 2. Differences from MS SQL Server

| Feature | SAP ASE | MS SQL Server | Impact |
|---------|---------|---------------|--------|
| Comments | **Nested supported** | Not nested | **High** - Different parsing |
| Unicode | `U&'\xxxx'` escape | `N'...'` prefix | **High** - Different syntax |
| Variable Scope | Can declare anywhere | Must be at top | **Medium** - Affects parser |
| Deferred Resolution | Table names resolved at runtime | Compile-time | **Medium** - Parser flexibility |

### 3. Critical Global Variables

These are **reset by every statement** (including `PRINT`, `IF`):

- `@@error` - Last error number (0 = success)
- `@@identity` - Last IDENTITY value inserted
- `@@rowcount` - Rows affected by last query

**Implication:** Must check immediately after operation; affects control flow analysis.

---

## Current Codebase Status

### ✅ Implemented
- Basic keyword recognition (SELECT, INSERT, UPDATE, DELETE, CREATE, FROM, WHERE, AS, EXEC, IF)
- Simple operators: `=`, `(`, `)`, `,`
- Identifier reading (alphanumeric, `_`, `.`)
- Integer literals
- Whitespace handling

### ❌ Missing (Blocking Tests)
1. **Block comment parsing** - Present in test but not implemented
2. **Line comment parsing** - No support
3. **Variable prefixes** - Cannot parse `@var`, `@@var`
4. **String literals** - No string parsing
5. **Temp table prefixes** - Cannot parse `#table`, `##table`

### 🐛 Bug Found
**File:** `tsql-lexer/src/lib.rs`, Line 135  
**Issue:** `read_block_comment()` references undefined variable `sql`  
**Fix:** Change `sql.chars()` to `self.input.chars()`

---

## Data Types Reference

### Character Types (UTF-16 = Unique to ASE)

| Type | Max Size | Encoding | Migration Target |
|------|----------|----------|------------------|
| `VARCHAR(n)` | 255 bytes | Server charset | MySQL `VARCHAR(n)` |
| `UNITEXT` | 1GB chars | **UTF-16** | MySQL `TEXT CHARACTER SET utf8mb4` |
| `UNICHAR(n)` | Variable | **UTF-16** | MySQL `CHAR(n) CHARACTER SET utf8mb4` |
| `UNIVARCHAR(n)` | Variable | **UTF-16** | MySQL `VARCHAR(n) CHARACTER SET utf8mb4` |
| `TEXT` | 2GB | Server charset | MySQL `TEXT` |

**Key:** UNITEXT/UNICHAR/UNIVARCHAR use UTF-16 (2-byte), rare in other SQL dialects.

### Numeric Types

| Type | Notes | Migration |
|------|-------|-----------|
| `MONEY` | Fixed-point currency | MySQL `DECIMAL(19,4)` |
| `SMALLMONEY` | Small currency | MySQL `DECIMAL(10,4)` |
| `TINYINT` | **Unsigned (0-255)** | MySQL `TINYINT UNSIGNED` |

**Note:** `%` (modulo) **cannot** be used on MONEY types.

---

## Operator Reference

### String Concatenation (Both Supported)
```sql
SELECT 'Hello' + ' ' + 'World'    -- Primary
SELECT 'Hello' || ' ' || 'World'  -- SQL standard
```

### Bitwise Operators (Integer Only)
```sql
SELECT 5 & 3   -- AND → 1
SELECT 5 | 3   -- OR → 7
SELECT 5 ^ 3   -- XOR → 6
SELECT ~5      -- NOT → -6
```

### Comparison Operators (Extended Set)
```sql
=, !=, <>, <, >, <=, >=   -- Standard
!<, !>                     -- Not less than, not greater than (ASE specific)
```

---

## Migration Challenges (ASE → MySQL)

### High Complexity
1. **Global variables** - No `@@error`, `@@identity` in MySQL
   - Solution: Use `DECLARE ... HANDLER` for errors, `LAST_INSERT_ID()` for identity
2. **Nested comments** - MySQL doesn't support
   - Solution: Pre-process or strip comments
3. **Stored procedure syntax** - Major differences
   - `EXEC proc @p=val` → `CALL proc(val)`
   - `@variable` → `variable` (no @ in MySQL procedures)
   - `PRINT 'msg'` → `SELECT 'msg'`

### Medium Complexity
4. **Function mapping** - Different names/parameters
   - `GETDATE()` → `NOW()`
   - `CHARINDEX(sub,str)` → `LOCATE(sub,str)` or `INSTR(str,sub)`
   - `REPLICATE(s,n)` → `REPEAT(s,n)`
5. **Data types** - MONEY, UNITEXT need conversion
6. **IDENTITY syntax** - Different column property syntax

### Low Complexity
7. **`SELECT TOP n`** → **`SELECT ... LIMIT n`** (simple rewrite)
8. **Transaction commands** - `BEGIN TRAN` → `START TRANSACTION`

---

## Lexer Implementation Priority

### Immediate (Phase 1)
1. **Fix bug** in `read_block_comment()` (line 135)
2. **Implement nested block comments** - Stack-based depth tracking
3. **Implement line comments** - `--` to newline

### High Priority (Phase 2-3)
4. **Variable prefixes** - `@`, `@@` detection
5. **Temp table prefixes** - `#`, `##` detection
6. **String literals** - Single/double quotes, escape handling
7. **Unicode strings** - `U&'\xxxx'` syntax

### Medium Priority (Phase 4-5)
8. **Extended operators** - Arithmetic, bitwise, comparison
9. **String concatenation** - `||` operator
10. **Extended keywords** - Control flow (BEGIN, END, WHILE, IF, ELSE, RETURN)

### Low Priority (Phase 6)
11. **Enhanced numeric literals** - Decimals, scientific notation, hex

---

## Code Quality Recommendations

### Performance
**Issue:** `lookup_ident()` creates HashMap on every call  
**Solution:** Use `once_cell::sync::Lazy` for static HashMap  
**Impact:** Massive improvement for keyword-heavy SQL

### Architecture
**Issue:** String-based token types, no position tracking  
**Solution:** Consider enum-based `TokenType` with line/column info  
**Benefits:** Type safety, better errors, smaller memory

### Error Handling
**Issue:** `panic!()` on errors  
**Solution:** Use `Result<Token, LexError>` with custom error types  
**Benefits:** Graceful error recovery, better user experience

---

## Common SQL Patterns in ASE

### Stored Procedure Template
```sql
CREATE PROCEDURE proc_name
    @param1 INT,
    @param2 VARCHAR(50) = NULL,
    @output INT OUTPUT
AS
BEGIN
    DECLARE @local_var INT
    
    IF @param2 IS NULL
        SELECT @param2 = 'default'
    
    SELECT @local_var = COUNT(*)
      FROM table1
     WHERE col1 = @param1
    
    IF @@error != 0
    BEGIN
        PRINT 'Error occurred'
        RETURN -1
    END
    
    SELECT @output = @local_var
    RETURN 0
END
```

### Temporary Table Usage
```sql
CREATE TABLE #temp_orders (
    order_id INT,
    customer_id INT,
    total MONEY
)

INSERT INTO #temp_orders
SELECT order_id, customer_id, total
  FROM orders
 WHERE order_date >= @start_date

SELECT customer_id, SUM(total) AS total_sales
  FROM #temp_orders
 GROUP BY customer_id

DROP TABLE #temp_orders
```

### Error Handling Pattern
```sql
INSERT INTO table1 VALUES (1, 'test')
IF @@error != 0
BEGIN
    ROLLBACK TRAN
    RETURN -1
END

UPDATE table2 SET status = 1
IF @@error != 0
BEGIN
    ROLLBACK TRAN
    RETURN -1
END

COMMIT TRAN
RETURN 0
```

---

## Resources

### Documentation
- **Comprehensive Analysis:** `/docs/SAP_ASE_TSQL_DIALECT_ANALYSIS.md` (12,000+ words)
- **Implementation Roadmap:** `/docs/LEXER_ROADMAP.md` (detailed phases)
- **SAP ASE Official Docs:** https://infocenter.sybase.com/

### Test Cases
- Simple SELECT: `tsql-lexer/tests/token.rs`
- Stored procedure: `tsql-lexer/tests/procedure_test.rs`

### Codebase
- Token definitions: `tsql-token/src/lib.rs`
- Lexer implementation: `tsql-lexer/src/lib.rs`

---

## Key Takeaways

1. **SAP ASE has unique features** not found in MS SQL Server (nested comments, `U&` escapes)
2. **Lexer must handle special prefixes** (`@`, `@@`, `#`, `##`) as distinct token types
3. **Current implementation is incomplete** - comments not working despite being in tests
4. **Migration to MySQL is non-trivial** - global variables, procedure syntax, data types all differ
5. **Performance optimization needed** - HashMap recreation on every keyword lookup
6. **Error handling needs improvement** - Replace `panic!()` with `Result` types

**Estimated Complexity:** Medium-High for full SAP ASE T-SQL support  
**Recommended Approach:** Incremental implementation following phased roadmap  
**Critical Path:** Fix comments → Add variable support → Add string literals → Extend operators
