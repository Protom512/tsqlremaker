# SAP ASE T-SQL Quick Reference Card

**Target:** tsqlremaker Lexer Implementation  
**Dialect:** SAP Adaptive Server Enterprise (Sybase)

---

## Token Categories for Lexer

### Prefixes (Must Detect)

| Prefix | Example | Token Type | Usage |
|--------|---------|------------|-------|
| `@` | `@count` | `LOCAL_VAR` | Local variable |
| `@@` | `@@error` | `GLOBAL_VAR` | System global variable |
| `#` | `#temp` | `TEMP_TABLE` | Session temp table |
| `##` | `##global` | `GLOBAL_TEMP_TABLE` | Global temp table |
| `U&` | `U&'\0041'` | `UNICODE_STRING` | Unicode escape literal |

### Comments (Nested!)

| Type | Start | End | Nesting |
|------|-------|-----|---------|
| Block | `/*` | `*/` | **YES** ✅ |
| Line | `--` | Newline | N/A |

**Example:**
```sql
/* Outer /* nested /* deep */ */ still in comment */
```

### String Literals

| Syntax | Description | Escape Quotes |
|--------|-------------|---------------|
| `'...'` | Single-quoted | `''` (double single) |
| `"..."` | Double-quoted* | `""` (double double) |
| `U&'...'` | Unicode escape | `\xxxx` or `\+yyyyyy` |

*\*Depends on `quoted_identifier` setting*

**Examples:**
```sql
'It''s working'              -- Escaped quote
U&'\0041\0042\0043'          -- ABC in Unicode
U&'Caf\00E9'                 -- Café
'Line 1 \                    -- Line continuation
Line 2'
```

### Operators (Multi-char)

| Operator | Description | Example |
|----------|-------------|---------|
| `||` | String concat | `'a' \|\| 'b'` |
| `<=` | Less or equal | `x <= 10` |
| `>=` | Greater or equal | `x >= 5` |
| `!=` | Not equal | `a != b` |
| `<>` | Not equal (alt) | `a <> b` |
| `!<` | Not less than | `a !< b` |
| `!>` | Not greater than | `a !> b` |

---

## Keywords by Category

### DDL (Data Definition)
```
CREATE, ALTER, DROP, TABLE, INDEX, VIEW, PROCEDURE, PROC, FUNCTION, TRIGGER
```

### DML (Data Manipulation)
```
SELECT, INSERT, UPDATE, DELETE, INTO, FROM, WHERE, VALUES, SET
```

### Control Flow
```
IF, ELSE, BEGIN, END, WHILE, BREAK, CONTINUE, GOTO, RETURN, WAITFOR
```

### Joins
```
JOIN, LEFT, RIGHT, INNER, OUTER, CROSS, ON
```

### Clauses
```
WHERE, ORDER, GROUP, BY, HAVING, DISTINCT, TOP, AS
```

### Logical
```
AND, OR, NOT, IN, LIKE, BETWEEN, IS, NULL, EXISTS
```

### Transactions
```
BEGIN, COMMIT, ROLLBACK, TRAN, TRANSACTION, SAVE, SAVEPOINT
```

### Procedure Keywords
```
DECLARE, OUTPUT, EXEC, EXECUTE, PRINT, RAISERROR, WITH, RECOMPILE
```

---

## Data Types (Recognition)

### Character
```
CHAR, VARCHAR, NCHAR, NVARCHAR, UNICHAR, UNIVARCHAR, TEXT, UNITEXT
```

### Numeric
```
INT, INTEGER, BIGINT, SMALLINT, TINYINT, DECIMAL, NUMERIC, FLOAT, REAL, MONEY, SMALLMONEY
```

### Date/Time
```
DATE, TIME, DATETIME, SMALLDATETIME, TIMESTAMP
```

### Binary
```
BINARY, VARBINARY, IMAGE
```

### Other
```
BIT, BOOLEAN
```

---

## Critical Global Variables

| Variable | Description | Reset Behavior |
|----------|-------------|----------------|
| `@@error` | Last error (0=success) | **Every statement** |
| `@@identity` | Last IDENTITY inserted | Insert/BCP only |
| `@@rowcount` | Rows affected | **Every statement** |
| `@@trancount` | Transaction nesting | BEGIN/COMMIT/ROLLBACK |
| `@@version` | Server version | Static |
| `@@servername` | Server name | Static |

**⚠️ Warning:** `@@error` and `@@rowcount` reset after **EVERY** statement, including `PRINT` and `IF`!

---

## Lexer State Machine (Simplified)

```
START
  ├─ [a-zA-Z_]        → read_identifier() → lookup_ident()
  ├─ [0-9]            → read_number() → NUM
  ├─ @                → check @@ → LOCAL_VAR or GLOBAL_VAR
  ├─ #                → check ## → TEMP_TABLE or GLOBAL_TEMP_TABLE
  ├─ ' or "           → read_string_literal() → STRING
  ├─ U or u           → check &' → UNICODE_STRING or IDENT
  ├─ /                → check * → read_block_comment() (recursive)
  ├─ -                → check - → read_line_comment()
  ├─ <                → check =,> → LT, LTE, NOT_EQUAL_ALT
  ├─ >                → check = → GT, GTE
  ├─ !                → check =,<,> → NOT_EQUAL, NOT_LT, NOT_GT
  ├─ |                → check | → BITWISE_OR or CONCAT
  ├─ Single char      → =, (, ), ,, ;, +, -, *, /, %, &, ^, ~
  └─ Whitespace       → skip
```

---

## Common Patterns (Examples)

### Variable Declaration
```sql
DECLARE @count INT, @name VARCHAR(50), @result INT OUTPUT
```

### Variable Assignment (Two Ways)
```sql
SELECT @count = COUNT(*) FROM table1  -- Transact-SQL style
SET @count = (SELECT COUNT(*) FROM table1)  -- ANSI style
```

### Temp Table Creation
```sql
CREATE TABLE #temp_results (
    id INT,
    value MONEY
)

INSERT INTO #temp_results
SELECT id, total FROM orders
```

### Error Handling
```sql
UPDATE table1 SET col1 = @value
IF @@error != 0
BEGIN
    ROLLBACK TRAN
    RETURN -1
END
```

### Stored Procedure
```sql
CREATE PROCEDURE get_data
    @id INT,
    @count INT OUTPUT
AS
BEGIN
    SELECT * FROM table1 WHERE id = @id
    SELECT @count = @@rowcount
    RETURN 0
END
```

### Control Flow
```sql
IF @count > 0
BEGIN
    PRINT 'Found records'
    SELECT @status = 1
END
ELSE
BEGIN
    PRINT 'No records'
    SELECT @status = 0
END

WHILE @counter < 100
BEGIN
    IF @counter % 10 = 0
        CONTINUE
    
    -- process
    SELECT @counter = @counter + 1
END
```

---

## Lexer Implementation Checklist

### Phase 1: Comments ✅ (HIGH PRIORITY)
- [ ] Block comment `/* */` with nesting depth tracking
- [ ] Line comment `--` to newline
- [ ] Handle comments inside strings (ignore)
- [ ] Test: `/* /* nested */ */`

### Phase 2: Variables ✅ (HIGH PRIORITY)
- [ ] Single `@` → `LOCAL_VAR`
- [ ] Double `@@` → `GLOBAL_VAR`
- [ ] Test: `@count`, `@@error`, `@@rowcount`

### Phase 3: Temp Tables ✅ (HIGH PRIORITY)
- [ ] Single `#` → `TEMP_TABLE`
- [ ] Double `##` → `GLOBAL_TEMP_TABLE`
- [ ] Test: `#temp`, `##global`

### Phase 4: Strings ✅ (HIGH PRIORITY)
- [ ] Single-quoted: `'...'`
- [ ] Double-quoted: `"..."` (setting-dependent)
- [ ] Escape handling: `''` within string
- [ ] Line continuation: `\` at end of line
- [ ] Test: `'It''s working'`

### Phase 5: Unicode Strings ✅ (MEDIUM PRIORITY)
- [ ] Detect `U&` or `u&` prefix
- [ ] Parse escape sequences: `\xxxx`, `\+yyyyyy`, `\\`
- [ ] Test: `U&'\0041\0042\0043'`, `U&'Caf\00E9'`

### Phase 6: Operators ✅ (MEDIUM PRIORITY)
- [ ] Multi-char: `||`, `<=`, `>=`, `!=`, `<>`, `!<`, `!>`
- [ ] Arithmetic: `+`, `-`, `*`, `/`, `%`
- [ ] Bitwise: `&`, `|`, `^`, `~`
- [ ] Punctuation: `;`, `:`, `.`

### Phase 7: Keywords ✅ (MEDIUM PRIORITY)
- [ ] Extend `lookup_ident()` with all keywords
- [ ] Optimize with lazy static HashMap
- [ ] Case-insensitive matching

### Phase 8: Numbers ✅ (LOW PRIORITY)
- [ ] Integers: `123`
- [ ] Decimals: `123.45`, `.5`
- [ ] Scientific: `1.23E+10`, `5e-3`
- [ ] Negative: `-123`

---

## Testing Checklist

### Unit Tests per Token Type
```rust
// Variables
test_local_variable()        // @count
test_global_variable()       // @@error
test_temp_table()            // #temp
test_global_temp_table()     // ##global

// Comments
test_block_comment()         // /* comment */
test_nested_comment()        // /* /* nested */ */
test_line_comment()          // -- comment
test_inline_comment()        // SELECT col1, -- comment

// Strings
test_string_literal()        // 'test'
test_escaped_quote()         // 'It''s'
test_double_quoted()         // "test"
test_unicode_string()        // U&'\0041'
test_unicode_escape()        // U&'\+01F600'

// Operators
test_concat_operator()       // ||
test_comparison_ops()        // <=, >=, !=, <>
test_bitwise_ops()           // &, |, ^, ~
test_arithmetic_ops()        // +, -, *, /, %
```

### Integration Tests (Real SQL)
```rust
test_simple_select()         // SELECT with WHERE
test_stored_procedure()      // Full procedure definition
test_temp_table_usage()      // CREATE, INSERT, SELECT from #temp
test_control_flow()          // IF/ELSE/WHILE
test_error_handling()        // @@error checks
```

---

## Performance Tips

### 1. Lazy Static HashMap
```rust
use once_cell::sync::Lazy;
static KEYWORDS: Lazy<HashMap<&str, &str>> = Lazy::new(|| { ... });
```

### 2. Avoid String Allocation
```rust
// Current: Token { token_type: String, token: String }
// Better: Token { token_type: TokenType, lexeme: &'a str }
```

### 3. Use Peekable Iterator
```rust
let mut chars = input.chars().peekable();
while let Some(ch) = chars.next() {
    match ch {
        '/' if chars.peek() == Some(&'*') => { /* block comment */ }
        // ...
    }
}
```

---

## Migration Gotchas (ASE → MySQL)

| ASE | MySQL | Note |
|-----|-------|------|
| `@@error` | No equivalent | Use `DECLARE ... HANDLER` |
| `@@identity` | `LAST_INSERT_ID()` | Different scope |
| `@@rowcount` | `ROW_COUNT()` | Function vs variable |
| `PRINT 'msg'` | `SELECT 'msg'` | Different output |
| `EXEC proc @p=val` | `CALL proc(val)` | No named params |
| `@variable` | `variable` | No @ in MySQL procedures |
| `#temp` | `CREATE TEMPORARY TABLE` | Different syntax |
| `SELECT TOP 10` | `SELECT ... LIMIT 10` | Different clause |
| `GETDATE()` | `NOW()` | Different function |
| `CHARINDEX(a,b)` | `LOCATE(a,b)` or `INSTR(b,a)` | Param order differs |

---

## Quick Lexer Bug Fixes

### Current Bug (Line 135)
```rust
// ❌ WRONG
fn read_block_comment(&self) -> String {
    let mut chars = sql.chars().peekable();  // sql undefined!
    
// ✅ FIX
fn read_block_comment(&mut self) -> String {
    let mut chars = self.input.chars().peekable();
```

### Missing Implementation
```rust
// In next_token():
match self.ch {
    '/' => {
        if self.peek_char() == '*' {
            self.read_block_comment();
            return self.next_token();  // Skip comment, get next token
        }
    }
    // ... rest
}
```

---

## Resources

- **Full Analysis:** `/docs/SAP_ASE_TSQL_DIALECT_ANALYSIS.md`
- **Roadmap:** `/docs/LEXER_ROADMAP.md`
- **Executive Summary:** `/docs/EXECUTIVE_SUMMARY.md`
- **SAP Docs:** https://infocenter.sybase.com/help/topic/com.sybase.infocenter.dc36272.1600/

---

## This Card Covers

✅ All special prefixes (`@`, `@@`, `#`, `##`, `U&`)  
✅ Comment syntax (nested block comments)  
✅ Operators (including multi-char)  
✅ String literal variants  
✅ Complete keyword list  
✅ Common SQL patterns  
✅ Implementation checklist  
✅ Testing strategy  
✅ Performance optimization  
✅ Migration gotchas  
✅ Current bug fix  

**Print this for quick reference during implementation!**
