# SAP ASE (Sybase) T-SQL Dialect Analysis
**Research Date:** January 19, 2026  
**Purpose:** Document SAP ASE T-SQL dialect specifics for lexer/parser implementation

---

## Table of Contents
1. [Variable Prefixes and Scoping](#1-variable-prefixes-and-scoping)
2. [Temporary Tables](#2-temporary-tables)
3. [Data Types](#3-data-types)
4. [Operators](#4-operators)
5. [Comment Syntax](#5-comment-syntax)
6. [String Literals and Escaping](#6-string-literals-and-escaping)
7. [Built-in Functions and Global Variables](#7-built-in-functions-and-global-variables)
8. [Control Flow Statements](#8-control-flow-statements)
9. [Stored Procedure Syntax](#9-stored-procedure-syntax)
10. [Key Differences from MS SQL Server](#10-key-differences-from-ms-sql-server)
11. [Migration Challenges to MySQL](#11-migration-challenges-to-mysql)
12. [Lexer Implementation Priorities](#12-lexer-implementation-priorities)

---

## 1. Variable Prefixes and Scoping

### Local Variables: `@`
- **Prefix:** Single `@` sign (e.g., `@variable_name`)
- **Declaration:** `DECLARE @variable datatype [, @variable2 datatype]...`
- **Scope:** 
  - Limited to the batch, procedure, or function where declared
  - In compound statements (`BEGIN...END`), scope limited to that block
  - Can be declared anywhere in the body of a stored procedure
- **Assignment:** 
  - `SELECT @var = value` (Transact-SQL style)
  - `SET @var = value` (SQL Anywhere compatible)
- **Naming Rules:**
  - Must start with `@`
  - Followed by alphabetic character, underscore, or another character
  - Can contain letters, numbers, underscore, period (`.`)
  - Maximum 254 bytes (excluding `@` prefix)

**Example:**
```sql
DECLARE @mult1 INT, @mult2 INT, @result INT
SELECT @mult1 = 12
SELECT @mult2 = 23
SELECT @result = @mult1 * @mult2
```

### Global Variables: `@@`
- **Prefix:** Double `@@` sign (e.g., `@@error`, `@@identity`)
- **Read-only:** Cannot be created or modified by users
- **System-defined:** Automatically updated by the system
- **Reserved:** Cannot use `@@` at start of user-defined objects
- **Examples:**
  - `@@error` - Last error number (0 = success)
  - `@@identity` - Last IDENTITY value inserted
  - `@@rowcount` - Rows affected by last statement
  - `@@trancount` - Transaction nesting level
  - `@@version` - Server version information
  - `@@servername` - Server name

**Important:** Global variables are reset by **every** Transact-SQL statement (including `PRINT` and `IF`), so they must be checked **immediately** after the relevant operation.

### Compatibility Note
- **SAP ASE:** `@` prefix is **required** for variables
- **Sybase IQ:** `@` prefix is **optional** (but recommended for compatibility)

---

## 2. Temporary Tables

### Local Temporary Tables: `#`
- **Prefix:** Single `#` sign (e.g., `#temp_table`)
- **Scope:** Session-specific, visible only to the current connection
- **Lifetime:** Automatically destroyed when session ends
- **Naming:** First 13 bytes (including `#`) must be unique
- **System Suffix:** ASE appends a 17-byte numeric suffix for uniqueness
- **Storage:** Created in `tempdb` database

**Example:**
```sql
CREATE TABLE #customer_temp (
    customer_id INT,
    customer_name VARCHAR(100)
)
```

### Global Temporary Tables: `##`
- **Prefix:** Double `##` sign (e.g., `##global_temp`)
- **Scope:** Visible to all sessions
- **Lifetime:** Exists until explicitly dropped or server restart
- **Storage:** `tempdb` database
- **Use Case:** Sharing data between multiple connections

**Example:**
```sql
CREATE TABLE ##shared_data (
    id INT,
    value VARCHAR(50)
)
```

### Permanent Temporary Tables (No Prefix)
- **Syntax:** `tempdb..table_name` (no `#` prefix)
- **Scope:** Can be shared among sessions
- **Lifetime:** Persist until explicitly dropped or server restart
- **Access:** Must qualify with `tempdb..` prefix

**Example:**
```sql
CREATE TABLE tempdb..permanent_temp (
    id INT,
    data VARCHAR(100)
)
```

---

## 3. Data Types

### Character Data Types

| Type | Storage | Max Size | Encoding | Description |
|------|---------|----------|----------|-------------|
| `CHAR(n)` | Fixed | 255 bytes | Server charset | Fixed-length character |
| `VARCHAR(n)` | Variable | 255 bytes | Server charset | Variable-length character |
| `NCHAR(n)` | Fixed | 255 chars | National charset | National character set |
| `NVARCHAR(n)` | Variable | 255 chars | National charset | Variable national charset |
| `UNICHAR(n)` | Fixed | N/A | UTF-16 (2-byte) | Unicode fixed-length |
| `UNIVARCHAR(n)` | Variable | N/A | UTF-16 (2-byte) | Unicode variable-length |
| `TEXT` | LOB | 2GB - 2 bytes | Server charset | Large text data |
| `UNITEXT` | LOB | 1,073,741,823 chars<br>(2,147,483,646 bytes) | UTF-16 | Unicode large text |

**Notes:**
- `UNITEXT` stores Unicode in UTF-16 encoding (suitable for Windows/Java)
- Empty string is treated as single blank for `VARCHAR` and `UNIVARCHAR`
- `TEXT` and `UNITEXT` share same storage mechanism

### Numeric Data Types

| Type | Range | Storage | Description |
|------|-------|---------|-------------|
| `TINYINT` | 0 to 255 | 1 byte | Unsigned integer |
| `SMALLINT` | -32,768 to 32,767 | 2 bytes | Small integer |
| `INT` / `INTEGER` | -2³¹ to 2³¹-1 | 4 bytes | Standard integer |
| `BIGINT` | -2⁶³ to 2⁶³-1 | 8 bytes | Large integer |
| `DECIMAL(p,s)` | Variable | Variable | Exact numeric |
| `NUMERIC(p,s)` | Variable | Variable | Exact numeric (alias) |
| `FLOAT(p)` | Variable | 4 or 8 bytes | Approximate numeric |
| `REAL` | ±3.4E-38 to ±3.4E+38 | 4 bytes | Single precision |
| `DOUBLE PRECISION` | ±1.7E-308 to ±1.7E+308 | 8 bytes | Double precision |
| `SMALLMONEY` | -214,748.3648 to 214,748.3647 | 4 bytes | Small money |
| `MONEY` | -922,337,203,685,477.5808 to 922,337,203,685,477.5807 | 8 bytes | Money |

**Note:** Modulo operator (`%`) **cannot** be used on `SMALLMONEY` or `MONEY` columns.

### Date/Time Data Types

| Type | Format | Range | Description |
|------|--------|-------|-------------|
| `DATE` | YYYY-MM-DD | 0001-01-01 to 9999-12-31 | Date only |
| `TIME` | HH:MM:SS[.nnnnnnn] | 00:00:00 to 23:59:59.999999 | Time only |
| `DATETIME` | YYYY-MM-DD HH:MM:SS.mmm | 1753-01-01 to 9999-12-31 | Date + Time (3ms precision) |
| `SMALLDATETIME` | YYYY-MM-DD HH:MM:SS | 1900-01-01 to 2079-06-06 | Date + Time (1min precision) |
| `TIMESTAMP` | System-generated | N/A | Row version tracking |
| `SECONDDATE` | YYYY-MM-DD HH:MM:SS | SQLScript specific | Date + Time (1s precision) |

### Binary Data Types

| Type | Max Size | Description |
|------|----------|-------------|
| `BINARY(n)` | 255 bytes | Fixed-length binary |
| `VARBINARY(n)` | 255 bytes | Variable-length binary |
| `IMAGE` | 2GB - 2 bytes | Large binary object |

### Other Types

| Type | Description |
|------|-------------|
| `BIT` | Boolean (0, 1, or NULL) |
| `BOOLEAN` | SQLScript boolean type |

---

## 4. Operators

### Arithmetic Operators

| Operator | Description | Notes |
|----------|-------------|-------|
| `+` | Addition | Also used for string concatenation |
| `-` | Subtraction | Also unary negation |
| `*` | Multiplication | |
| `/` | Division | Integer division if both operands are integers |
| `%` | Modulo | Transact-SQL extension, not for MONEY types |

**Precedence (high to low):**
1. Unary `+`, `-`
2. `*`, `/`, `%`
3. Binary `+`, `-`

### Bitwise Operators
*Operate on integer data types only*

| Operator | Description | Example |
|----------|-------------|---------|
| `&` | Bitwise AND | `5 & 3` → `1` |
| `\|` | Bitwise OR | `5 \| 3` → `7` |
| `^` | Bitwise XOR | `5 ^ 3` → `6` |
| `~` | Bitwise NOT | `~5` → `-6` |

### Comparison Operators

| Operator | Description |
|----------|-------------|
| `=` | Equal to |
| `!=` or `<>` | Not equal to |
| `<` | Less than |
| `>` | Greater than |
| `<=` | Less than or equal to |
| `>=` | Greater than or equal to |
| `!<` | Not less than |
| `!>` | Not greater than |

### String Concatenation Operators

| Operator | Description | Compatibility |
|----------|-------------|---------------|
| `+` | String concatenation | Primary method |
| `\|\|` | String concatenation | SQL standard alternative |

**Example:**
```sql
SELECT 'Hello' + ' ' + 'World'  -- Result: 'Hello World'
SELECT 'Hello' || ' ' || 'World'  -- Result: 'Hello World'
```

### Logical Operators

| Operator | Description |
|----------|-------------|
| `AND` | Logical AND |
| `OR` | Logical OR |
| `NOT` | Logical NOT |
| `LIKE` | Pattern matching |
| `IN` | Value in list |
| `BETWEEN` | Range check |
| `IS NULL` | NULL check |
| `EXISTS` | Subquery existence |

### Assignment Operators
**Note:** Compound assignment operators like `+=`, `*=` are **NOT standard** in SAP ASE T-SQL. Use explicit assignment instead:

```sql
-- NOT standard in ASE
-- @count += 1  

-- Use this instead
SELECT @count = @count + 1
-- or
SET @count = @count + 1
```

---

## 5. Comment Syntax

### Block Comments: `/* ... */`
- **Start:** `/*`
- **End:** `*/`
- **Multi-line:** Yes
- **Nesting:** **YES** - Nested comments are **supported** in SAP ASE
  - This is a key difference from standard SQL and C-style comments
  - Each `/*` must have matching `*/`

**Example:**
```sql
/*****************************************************************************
 * block comment
 * /* nested comment works here */
 * more comment text
 *****************************************************************************/
CREATE PROC proc1 AS
  SELECT clm1, clm2
    FROM db1..table1
```

### Line Comments: `--`
- **Start:** `--` (two hyphens, optionally followed by space)
- **End:** Newline character
- **Multi-line:** No (each line needs `--`)
- **Common Use:** Inline comments

**Example:**
```sql
SELECT col1,  -- This is a comment
       col2   -- Another comment
  FROM table1
-- WHERE clause commented out
-- WHERE col1 > 10
```

### Important Notes
- `--` within `/* */` block is **not recognized** as a comment marker
- Convention: Multi-line `/* */` comments often use `**` at start of subsequent lines

---

## 6. String Literals and Escaping

### Quote Types

| Quote Type | Usage | Setting Dependency |
|------------|-------|-------------------|
| Single quotes `'...'` | String literals | Always works |
| Double quotes `"..."` | String literals or identifiers | If `quoted_identifier` is OFF |
| Double quotes `"..."` | Identifiers only | If `quoted_identifier` is ON |

**Best Practice:** Use **single quotes** for string literals to avoid ambiguity.

### Escaping Quotes

**Method 1: Double the quote character**
```sql
SELECT 'I don''t understand'          -- Single quote in string
SELECT "He said, ""Hello"""           -- Double quote in string (if quoted_identifier OFF)
```

**Method 2: Use opposite quote type**
```sql
SELECT "George said, 'Hello'"         -- Single quote inside double quotes
SELECT 'She said, "Goodbye"'          -- Double quote inside single quotes
```

### Unicode String Literals

**Unicode Escape Sequences:** `U&` or `u&` prefix
- Must have **no whitespace** between prefix and quote
- Escape sequences within the literal:
  - `\xxxx` - Unicode character (4 hex digits)
  - `\+yyyyyy` - Unicode character (6 hex digits)
  - `\\` - Literal backslash

**Example:**
```sql
SELECT U&'\0041\0042\0043'            -- Result: 'ABC'
SELECT U&'Caf\00E9'                   -- Result: 'Café'
SELECT U&'\+01F600'                   -- Unicode emoji
```

**Note:** Unlike MS SQL Server, SAP ASE does **NOT** use `N'...'` prefix for Unicode strings. Use `U&` prefix or Unicode data types instead.

### Line Continuation
Use backslash `\` to continue string literal across multiple lines:

```sql
SELECT 'This is a very long string \
that continues on the next line'
```

### Empty String Handling
- Empty string `''` is interpreted as **single blank** for `VARCHAR` and `UNIVARCHAR` in insert/assignment statements

---

## 7. Built-in Functions and Global Variables

### Common String Functions

| Function | Description | MySQL Equivalent |
|----------|-------------|------------------|
| `ASCII(char)` | ASCII value of character | `ASCII()` |
| `CHAR_LENGTH(str)` | Length in characters | `CHAR_LENGTH()` |
| `LEFT(str, n)` | Leftmost n characters | `LEFT()` |
| `RIGHT(str, n)` | Rightmost n characters | `RIGHT()` |
| `UPPER(str)` | Convert to uppercase | `UPPER()` |
| `LOWER(str)` | Convert to lowercase | `LOWER()` |
| `LTRIM(str)` | Remove leading spaces | `LTRIM()` |
| `RTRIM(str)` | Remove trailing spaces | `RTRIM()` |
| `SUBSTRING(str, start, len)` | Extract substring | `SUBSTRING()` |
| `REVERSE(str)` | Reverse string | `REVERSE()` |
| `REPLICATE(str, n)` | Repeat string n times | `REPEAT()` |
| `CHARINDEX(substr, str)` | Find substring position | `LOCATE()` or `INSTR()` |
| `PATINDEX(pattern, str)` | Pattern match position | No direct equivalent |
| `STUFF(str, start, len, new)` | Replace substring | `INSERT()` |

### Conversion Functions

| Function | Description |
|----------|-------------|
| `CAST(value AS datatype)` | Type conversion |
| `CONVERT(datatype, value)` | Type conversion (Transact-SQL) |
| `TO_BIGINT(value)` | Convert to BIGINT (SQLScript) |
| `TO_VARCHAR(value)` | Convert to VARCHAR (SQLScript) |

### Date/Time Functions

| Function | Description | MySQL Equivalent |
|----------|-------------|------------------|
| `GETDATE()` | Current date/time | `NOW()` |
| `DATEADD(part, n, date)` | Add interval to date | `DATE_ADD()` |
| `DATEDIFF(part, date1, date2)` | Difference between dates | `DATEDIFF()` |
| `DATEPART(part, date)` | Extract date part | `EXTRACT()` |
| `DATENAME(part, date)` | Date part as string | `DATE_FORMAT()` |
| `YEAR(date)` | Extract year | `YEAR()` |
| `MONTH(date)` | Extract month | `MONTH()` |
| `DAY(date)` | Extract day | `DAY()` |

### Aggregate Functions

| Function | Description |
|----------|-------------|
| `COUNT(expr)` | Count rows |
| `SUM(expr)` | Sum values |
| `AVG(expr)` | Average value |
| `MAX(expr)` | Maximum value |
| `MIN(expr)` | Minimum value |
| `STDEV(expr)` | Standard deviation |
| `VARIANCE(expr)` | Variance |

### Critical Global Variables

| Variable | Description | Reset Behavior |
|----------|-------------|----------------|
| `@@error` | Last error number (0 = success) | **Reset by every statement** |
| `@@identity` | Last IDENTITY value inserted | Updated by INSERT/SELECT INTO/BCP |
| `@@rowcount` | Rows affected by last statement | **Reset by every statement** |
| `@@trancount` | Current transaction nesting level | Modified by BEGIN/COMMIT/ROLLBACK TRAN |
| `@@version` | Server version string | Static |
| `@@servername` | Server name | Static |
| `@@spid` | Current session process ID | Static per session |
| `@@connections` | Login attempts since startup | Server-wide |
| `@@cpu_busy` | CPU time in milliseconds | Server-wide |
| `@@idle` | Idle time in milliseconds | Server-wide |
| `@@io_busy` | I/O time in milliseconds | Server-wide |
| `@@pack_received` | Input packets received | Server-wide |
| `@@pack_sent` | Output packets sent | Server-wide |
| `@@total_errors` | Total errors since startup | Server-wide |

**Critical:** `@@error` and `@@rowcount` are reset by **EVERY** statement, including:
- `PRINT` statements
- `IF` tests
- Variable assignments

**Always check immediately after the operation:**
```sql
INSERT INTO table1 VALUES (1, 'test')
IF @@error != 0
    RETURN  -- Check must be immediate!
```

---

## 8. Control Flow Statements

### IF...ELSE

**Syntax:**
```sql
IF logical_expression
    statement | BEGIN...END
[ELSE
    statement | BEGIN...END]
```

**Example:**
```sql
IF @count > 10
BEGIN
    PRINT 'Count is high'
    SELECT @status = 1
END
ELSE
BEGIN
    PRINT 'Count is low'
    SELECT @status = 0
END
```

**Notes:**
- `logical_expression` can involve column names, constants, arithmetic operators, **bitwise operators**, or subqueries
- Single statement doesn't need `BEGIN...END`
- Multiple statements require `BEGIN...END` block

### WHILE and BREAK...CONTINUE

**Syntax:**
```sql
WHILE logical_expression
BEGIN
    statements
    [BREAK]
    [CONTINUE]
END
```

**Example:**
```sql
WHILE @counter < 100
BEGIN
    IF @counter % 10 = 0
        CONTINUE  -- Skip to next iteration
    
    IF @counter = 50
        BREAK     -- Exit loop
    
    SELECT @counter = @counter + 1
END
```

### BEGIN...END

**Syntax:**
```sql
BEGIN
    statements
END
```

**Purpose:**
- Groups multiple SQL statements into a single block
- Required for multiple statements in `IF`, `ELSE`, `WHILE`
- Defines scope for local variables (in some contexts)

### GOTO and Labels

**Syntax:**
```sql
label:
    statements
    
GOTO label
```

**Example:**
```sql
IF @error_flag = 1
    GOTO error_handler

-- normal processing
SELECT * FROM table1
RETURN

error_handler:
    PRINT 'Error occurred'
    RETURN
```

### RETURN

**Syntax:**
```sql
RETURN [integer_expression]
```

**Purpose:**
- Exits unconditionally from batch or procedure
- Optional integer return value (typically 0 = success, non-zero = error)

**Example:**
```sql
IF @@error != 0
    RETURN -1
    
-- continue processing
RETURN 0
```

### WAITFOR

**Syntax:**
```sql
WAITFOR DELAY 'time_string'
WAITFOR TIME 'time_string'
```

**Example:**
```sql
WAITFOR DELAY '00:00:05'  -- Wait 5 seconds
WAITFOR TIME '23:00:00'   -- Wait until 11 PM
```

---

## 9. Stored Procedure Syntax

### CREATE PROCEDURE

**Basic Syntax:**
```sql
CREATE PROCEDURE [owner.]procedure_name
    [@parameter datatype [= default] [OUTPUT]] [, ...]
    [WITH RECOMPILE]
    [WITH EXECUTE AS {CALLER | OWNER}]
AS
    SQL_statements
```

**Example:**
```sql
CREATE PROCEDURE get_customer_orders
    @customer_id INT,
    @start_date DATETIME = NULL,
    @order_count INT OUTPUT
AS
BEGIN
    IF @start_date IS NULL
        SELECT @start_date = '1900-01-01'
    
    SELECT order_id, order_date, total_amount
      FROM orders
     WHERE customer_id = @customer_id
       AND order_date >= @start_date
    
    SELECT @order_count = @@rowcount
    
    RETURN 0
END
```

### Key Features

**Parameters:**
- Prefixed with `@`
- Can have default values: `@param datatype = default_value`
- `OUTPUT` keyword for output parameters
- Data type restrictions: `TEXT`, `UNITEXT`, `IMAGE` can only be input parameters (not OUTPUT)

**WITH RECOMPILE:**
- Forces recompilation each time procedure is executed
- Useful when execution plan may vary significantly

**WITH EXECUTE AS:**
- `OWNER`: Permission checks use procedure owner's permissions
- `CALLER`: Permission checks use caller's permissions (default)

**Deferred Name Resolution:**
- Object references (tables, columns) resolved at **execution time**, not creation time
- Allows procedures to reference objects that don't exist yet

**Procedure Groups:**
- Multiple procedures can share same name with different numbers
- Syntax: `procedure_name;number`
- Dropped as a group with single `DROP PROCEDURE`

### EXECUTE/EXEC

**Syntax:**
```sql
EXECUTE procedure_name [@param = value [, ...]]
EXEC procedure_name [value [, value [, ...]]]
procedure_name  -- Can omit EXEC if first statement in batch
```

**Example:**
```sql
-- Named parameters
EXEC get_customer_orders @customer_id = 123, @order_count = @count OUTPUT

-- Positional parameters
EXEC get_customer_orders 123, '2024-01-01', @count OUTPUT

-- First statement in batch
get_customer_orders @customer_id = 123
```

### Extended Stored Procedures (ESP)

**Purpose:** Execute external procedural code (C/C++ DLLs)
**Syntax:** Same as regular stored procedures
**Naming Convention:** Typically prefixed with `xp_`
**System ESPs:** `xp_cmdshell`, `xp_sendmail`, etc.

---

## 10. Key Differences from MS SQL Server

| Feature | SAP ASE | MS SQL Server | Impact |
|---------|---------|---------------|--------|
| **Variable Assignment** | `SELECT @var = value` (primary) | `SET @var = value` (primary) | Low - both support both syntaxes |
| **Variable Scope** | Can declare anywhere in procedure | Must declare at start of batch/proc | Medium - affects code organization |
| **Unicode Literals** | `U&'\xxxx'` escape syntax | `N'...'` prefix | High - requires lexer support |
| **Nested Comments** | **Supported** (`/* /* */ */`) | **Not supported** | High - lexer must handle nesting |
| **Temp Table Scope** | `#` = session, `##` = global | `#` = session, `##` = global | Low - same behavior |
| **Global Variables** | `@@error`, `@@identity` (reset per statement) | Same but less aggressive reset | Medium - error handling patterns differ |
| **`SELECT *` Expansion** | Resolved at execution time | Resolved at creation time | Medium - deferred resolution |
| **String Concatenation** | `+` or `\|\|` | `+` only | Low - both work |
| **Date/Time Functions** | `GETDATE()` | `GETDATE()` or `SYSDATETIME()` | Low - mostly compatible |
| **`TOP` Clause** | `SELECT TOP n` | `SELECT TOP n` or `TOP n PERCENT` | Low - basic syntax same |
| **Identity Syntax** | `IDENTITY` column property | `IDENTITY(seed, increment)` | Medium - syntax difference |
| **Error Handling** | `@@error` checks | `TRY...CATCH` blocks (modern) | High - different error handling paradigm |
| **NULL Concatenation** | `NULL + 'text'` = `NULL` | Same (if `CONCAT_NULL_YIELDS_NULL` ON) | Low - similar behavior |
| **Modulo Operator** | `%` (not for MONEY types) | `%` | Low - same operator |
| **`PRINT` Statement** | `PRINT expression` | `PRINT expression` | Low - compatible |

### Critical Differences for Parser

1. **Nested Block Comments:** ASE supports `/* /* nested */ */` - requires stack-based comment parsing
2. **Unicode Escape Sequences:** `U&'\xxxx'` syntax requires special lexer handling
3. **Deferred Name Resolution:** Table/column names may not exist at parse time
4. **Variable Declaration Location:** Can be anywhere in procedure body, not just at start

---

## 11. Migration Challenges to MySQL

### Data Type Mapping

| SAP ASE | MySQL | Notes |
|---------|-------|-------|
| `VARCHAR(n)` | `VARCHAR(n)` | MySQL max is 65,535 bytes |
| `NVARCHAR(n)` | `VARCHAR(n) CHARACTER SET utf8mb4` | Charset handling differs |
| `TEXT` | `TEXT` | LOB handling differs |
| `UNITEXT` | `TEXT CHARACTER SET utf8mb4` | UTF-16 → UTF-8 conversion |
| `UNICHAR(n)` | `CHAR(n) CHARACTER SET utf8mb4` | Charset conversion |
| `UNIVARCHAR(n)` | `VARCHAR(n) CHARACTER SET utf8mb4` | Charset conversion |
| `INT` | `INT` | Compatible |
| `BIGINT` | `BIGINT` | Compatible |
| `SMALLINT` | `SMALLINT` | Compatible |
| `TINYINT` | `TINYINT UNSIGNED` | ASE is unsigned, MySQL default is signed |
| `MONEY` | `DECIMAL(19,4)` | MySQL has no MONEY type |
| `SMALLMONEY` | `DECIMAL(10,4)` | MySQL has no SMALLMONEY type |
| `DATETIME` | `DATETIME` | Precision differs |
| `SMALLDATETIME` | `DATETIME` | MySQL has single DATETIME type |
| `IMAGE` | `BLOB` | Binary LOB |

### Function Mapping

| SAP ASE | MySQL | Notes |
|---------|-------|-------|
| `GETDATE()` | `NOW()` | Current timestamp |
| `CHARINDEX(sub, str)` | `LOCATE(sub, str)` or `INSTR(str, sub)` | Parameter order differs |
| `LEN(str)` | `LENGTH(str)` or `CHAR_LENGTH(str)` | Name difference |
| `REPLICATE(str, n)` | `REPEAT(str, n)` | Name difference |
| `DATEADD(part, n, date)` | `DATE_ADD(date, INTERVAL n part)` | Syntax differs |
| `DATEDIFF(part, d1, d2)` | `DATEDIFF(d1, d2)` (days only) | MySQL limited |
| `ISNULL(expr, replace)` | `IFNULL(expr, replace)` or `COALESCE()` | Name difference |
| `CONVERT(type, value)` | `CAST(value AS type)` | Syntax differs |

### Stored Procedure Conversion

**Major Challenges:**

1. **`PRINT` Statement:**
   - **ASE:** `PRINT 'message'`
   - **MySQL:** Use `SELECT 'message'` or application logging

2. **`EXEC` Statement:**
   - **ASE:** `EXEC procedure_name @param = value`
   - **MySQL:** `CALL procedure_name(value)`

3. **Output Parameters:**
   - **ASE:** `@param OUTPUT`
   - **MySQL:** `OUT param` or `INOUT param`

4. **Return Values:**
   - **ASE:** `RETURN integer_value`
   - **MySQL:** Functions have `RETURN`, procedures use `OUT` parameters

5. **Error Handling:**
   - **ASE:** `IF @@error != 0`
   - **MySQL:** `DECLARE ... HANDLER FOR ... `

6. **Variable Declaration:**
   - **ASE:** `DECLARE @var datatype` (anywhere in procedure)
   - **MySQL:** `DECLARE var datatype` (must be at start, no `@` prefix)

7. **Temporary Tables:**
   - **ASE:** `#temp` (session), `##temp` (global)
   - **MySQL:** `CREATE TEMPORARY TABLE temp` (session only)

8. **Transaction Control:**
   - **ASE:** `BEGIN TRAN`, `COMMIT TRAN`, `ROLLBACK TRAN`
   - **MySQL:** `START TRANSACTION`, `COMMIT`, `ROLLBACK`

### Syntax Conversions

| Feature | SAP ASE | MySQL |
|---------|---------|-------|
| **Select Top** | `SELECT TOP 10 *` | `SELECT * LIMIT 10` |
| **String Concat** | `'a' + 'b'` or `'a' \|\| 'b'` | `CONCAT('a', 'b')` or `'a' 'b'` |
| **Identity Column** | `column_name INT IDENTITY` | `column_name INT AUTO_INCREMENT` |
| **Local Variable** | `@variable` | `variable` (no @ in procedures) |
| **Global Variable** | `@@error`, `@@rowcount` | No direct equivalent, use exceptions |
| **IF Statement** | `IF condition statement` | `IF condition THEN statement; END IF;` |
| **WHILE Loop** | `WHILE condition BEGIN ... END` | `WHILE condition DO ... END WHILE;` |

### Critical Migration Issues

1. **No Global Variables:** MySQL doesn't have `@@error`, `@@identity` equivalents
   - Use `DECLARE ... HANDLER` for error handling
   - Use `LAST_INSERT_ID()` for identity values

2. **Different Transaction Semantics:** 
   - ASE has implicit transactions
   - MySQL auto-commits by default

3. **Procedure Execution:**
   - ASE: `EXEC proc_name` or just `proc_name`
   - MySQL: Must use `CALL proc_name()`

4. **No Named Parameters:**
   - ASE: `EXEC proc @param1 = value, @param2 = value`
   - MySQL: `CALL proc(value1, value2)` (positional only)

5. **Comment Nesting:**
   - ASE: Nested `/* /* */ */` supported
   - MySQL: Nested comments **NOT** supported

6. **Unicode Handling:**
   - ASE: UTF-16 storage (UNITEXT), `U&` escape syntax
   - MySQL: UTF-8 storage, standard escaping

---

## 12. Lexer Implementation Priorities

Based on the codebase analysis and dialect research, here are recommended priorities for the lexer:

### Phase 1: Core Token Recognition (Current Focus)

**Status:** ✅ Partially implemented in `tsql-token` and `tsql-lexer`

- [x] Keywords: `SELECT`, `UPDATE`, `DELETE`, `INSERT`, `CREATE`, `FROM`, `WHERE`, `AS`, `EXEC`, `IF`
- [x] Operators: `=`, `(`, `)`, `,`
- [x] Identifiers: Letters, digits, underscore, period
- [x] Numbers: Integer literals
- [x] Whitespace handling
- [ ] **PRIORITY:** Block comments `/* */` with **nesting support**
- [ ] **PRIORITY:** Line comments `--`

### Phase 2: Variable and Special Prefixes

**Next Priority:** Variable handling is essential for procedure parsing

- [ ] Local variables: `@identifier` (single `@` prefix)
- [ ] Global variables: `@@identifier` (double `@@` prefix)
- [ ] Local temp tables: `#identifier` (single `#` prefix)
- [ ] Global temp tables: `##identifier` (double `##` prefix)
- [ ] Unicode escape prefix: `U&` or `u&` (followed by string literal)

**Lexer Strategy:**
```rust
match self.ch {
    '@' => {
        if self.peek_char() == '@' {
            // Global variable @@
            self.read_char();
            self.read_char();
            let name = self.read_identifier();
            Token::new(GLOBAL_VAR, format!("@@{}", name))
        } else {
            // Local variable @
            self.read_char();
            let name = self.read_identifier();
            Token::new(LOCAL_VAR, format!("@{}", name))
        }
    }
    '#' => {
        if self.peek_char() == '#' {
            // Global temp table ##
            self.read_char();
            self.read_char();
            let name = self.read_identifier();
            Token::new(GLOBAL_TEMP_TABLE, format!("##{}", name))
        } else {
            // Local temp table #
            self.read_char();
            let name = self.read_identifier();
            Token::new(LOCAL_TEMP_TABLE, format!("#{}", name))
        }
    }
    // ... rest of matching
}
```

### Phase 3: String Literals and Escaping

**Essential for proper SQL parsing:**

- [ ] Single-quoted strings: `'string'`
- [ ] Double-quoted strings: `"string"` (with `quoted_identifier` consideration)
- [ ] Quote escaping: `''` (double quote within string)
- [ ] Unicode escape sequences: `U&'\xxxx'`, `U&'\+yyyyyy'`
- [ ] Line continuation: `\` at end of line
- [ ] Mixed quote handling: `'It''s'` and `"He said, ""Hi"""`

**Lexer Strategy:**
```rust
'\'' => {
    let literal = self.read_string_literal('\'');
    Token::new(STRING, literal)
}
'U' | 'u' => {
    if self.peek_char() == '&' {
        // Unicode escape string U&'...'
        self.read_char(); // consume 'U' or 'u'
        self.read_char(); // consume '&'
        if self.ch == '\'' {
            let literal = self.read_unicode_string();
            Token::new(UNICODE_STRING, literal)
        } else {
            // Just 'U' or 'u' identifier
            self.read_identifier()
        }
    } else {
        // Regular identifier starting with U/u
        self.read_identifier()
    }
}
```

### Phase 4: Additional Operators

**Needed for full expression support:**

- [ ] Arithmetic: `+`, `-`, `*`, `/`, `%` (modulo)
- [ ] Comparison: `<`, `>`, `<=`, `>=`, `!=`, `<>`, `!<`, `!>`
- [ ] Bitwise: `&`, `|`, `^`, `~`
- [ ] String concatenation: `||` (two-character operator)
- [ ] Assignment: `=` (already implemented)
- [ ] Semicolon: `;` (statement terminator)
- [ ] Colon: `:` (label marker)

**Multi-character operator handling:**
```rust
'<' => {
    match self.peek_char() {
        '=' => {
            self.read_char();
            Token::new(LTE, "<=".to_string())
        }
        '>' => {
            self.read_char();
            Token::new(NOT_EQUAL, "<>".to_string())
        }
        _ => Token::new(LT, "<".to_string())
    }
}
'|' => {
    if self.peek_char() == '|' {
        self.read_char();
        Token::new(CONCAT, "||".to_string())
    } else {
        Token::new(BITWISE_OR, "|".to_string())
    }
}
```

### Phase 5: Extended Keywords

**Add comprehensive keyword support:**

```rust
// Control flow
pub const BEGIN: &str = "begin";
pub const END: &str = "end";
pub const WHILE: &str = "while";
pub const BREAK: &str = "break";
pub const CONTINUE: &str = "continue";
pub const GOTO: &str = "goto";
pub const RETURN: &str = "return";
pub const WAITFOR: &str = "waitfor";
pub const DELAY: &str = "delay";

// DDL
pub const ALTER: &str = "alter";
pub const DROP: &str = "drop";
pub const TABLE: &str = "table";
pub const INDEX: &str = "index";
pub const VIEW: &str = "view";
pub const PROCEDURE: &str = "procedure";
pub const PROC: &str = "proc";
pub const FUNCTION: &str = "function";
pub const TRIGGER: &str = "trigger";

// DML
pub const INTO: &str = "into";
pub const VALUES: &str = "values";
pub const SET: &str = "set";

// Joins
pub const JOIN: &str = "join";
pub const LEFT: &str = "left";
pub const RIGHT: &str = "right";
pub const INNER: &str = "inner";
pub const OUTER: &str = "outer";
pub const CROSS: &str = "cross";
pub const ON: &str = "on";

// Clauses
pub const ORDER: &str = "order";
pub const GROUP: &str = "group";
pub const BY: &str = "by";
pub const HAVING: &str = "having";
pub const DISTINCT: &str = "distinct";
pub const TOP: &str = "top";

// Logical
pub const AND: &str = "and";
pub const OR: &str = "or";
pub const NOT: &str = "not";
pub const IN: &str = "in";
pub const LIKE: &str = "like";
pub const BETWEEN: &str = "between";
pub const IS: &str = "is";
pub const NULL: &str = "null";
pub const EXISTS: &str = "exists";

// Transactions
pub const TRAN: &str = "tran";
pub const TRANSACTION: &str = "transaction";
pub const COMMIT: &str = "commit";
pub const ROLLBACK: &str = "rollback";
pub const SAVE: &str = "save";
pub const SAVEPOINT: &str = "savepoint";

// Procedure-specific
pub const DECLARE: &str = "declare";
pub const OUTPUT: &str = "output";
pub const WITH: &str = "with";
pub const RECOMPILE: &str = "recompile";
pub const EXECUTE: &str = "execute";
pub const PRINT: &str = "print";
pub const RAISERROR: &str = "raiserror";

// Data types (if treating as keywords)
pub const INT: &str = "int";
pub const VARCHAR: &str = "varchar";
pub const CHAR: &str = "char";
pub const DATETIME: &str = "datetime";
pub const DECIMAL: &str = "decimal";
pub const NUMERIC: &str = "numeric";
pub const MONEY: &str = "money";
pub const TEXT: &str = "text";
pub const UNITEXT: &str = "unitext";
pub const BIT: &str = "bit";
```

### Phase 6: Numeric Literals

**Expand numeric token support:**

- [ ] Integers: `123`, `-456`
- [ ] Decimals: `123.45`, `.5`
- [ ] Scientific notation: `1.23E+10`, `5e-3`
- [ ] Negative numbers: Distinguish unary `-` from binary `-`
- [ ] Hex literals: `0x1A2B` (if supported in ASE)
- [ ] Money literals: `$123.45` (if supported)

### Phase 7: Advanced Comment Handling

**Nested block comment implementation:**

```rust
fn read_block_comment(&mut self) -> String {
    let mut result = String::from("/*");
    let mut depth = 1;  // Track nesting depth
    
    self.read_char(); // consume '/'
    self.read_char(); // consume '*'
    
    while depth > 0 {
        if self.ch == '\0' {
            panic!("Unterminated block comment");
        }
        
        if self.ch == '/' && self.peek_char() == '*' {
            // Nested comment start
            depth += 1;
            result.push(self.ch);
            self.read_char();
            result.push(self.ch);
            self.read_char();
        } else if self.ch == '*' && self.peek_char() == '/' {
            // Comment end
            depth -= 1;
            result.push(self.ch);
            self.read_char();
            result.push(self.ch);
            self.read_char();
        } else {
            result.push(self.ch);
            self.read_char();
        }
    }
    
    result
}
```

### Testing Strategy

**Recommended test cases for each phase:**

1. **Variable Prefixes:**
   ```sql
   DECLARE @count INT, @name VARCHAR(50)
   SELECT @count = @@rowcount
   SELECT * FROM #temp_table
   INSERT INTO ##global_temp VALUES (1, 'test')
   ```

2. **Comments:**
   ```sql
   /* Simple block comment */
   /* Outer /* nested */ comment */
   /* /* /* triple nested */ */ */
   -- Line comment
   SELECT col1, -- inline comment
          col2
   ```

3. **String Literals:**
   ```sql
   SELECT 'Simple string'
   SELECT 'String with ''escaped quote'''
   SELECT "Double quoted"
   SELECT U&'\0041\0042\0043'
   SELECT U&'Caf\00E9'
   ```

4. **Operators:**
   ```sql
   SELECT a + b, a - b, a * b, a / b, a % b
   SELECT a & b, a | b, a ^ b, ~a
   SELECT a < b, a > b, a <= b, a >= b, a != b, a <> b
   SELECT 'Hello' || ' ' || 'World'
   ```

5. **Real Stored Procedure:**
   ```sql
   /*****************************************************************************
    * Get customer orders
    *****************************************************************************/
   CREATE PROC get_customer_orders
       @customer_id INT,
       @start_date DATETIME = NULL,
       @order_count INT OUTPUT
   AS
   BEGIN
       DECLARE @temp_count INT
       
       IF @start_date IS NULL
           SELECT @start_date = '1900-01-01'
       
       CREATE TABLE #temp_orders (
           order_id INT,
           order_date DATETIME,
           total MONEY
       )
       
       INSERT INTO #temp_orders
       SELECT order_id, order_date, total_amount
         FROM orders
        WHERE customer_id = @customer_id
          AND order_date >= @start_date
       
       SELECT @order_count = @@rowcount
       
       IF @@error != 0
       BEGIN
           PRINT 'Error retrieving orders'
           RETURN -1
       END
       
       SELECT * FROM #temp_orders
       ORDER BY order_date DESC
       
       DROP TABLE #temp_orders
       
       RETURN 0
   END
   ```

### Performance Considerations

1. **Keyword Lookup Optimization:**
   - Current: HashMap created on every `lookup_ident()` call
   - **Recommended:** Use `lazy_static!` or `once_cell::sync::Lazy` for static HashMap

```rust
use once_cell::sync::Lazy;
use std::collections::HashMap;

static KEYWORDS: Lazy<HashMap<String, &'static str>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("select".to_string(), SELECT);
    map.insert("if".to_string(), IF);
    map.insert("create".to_string(), CREATE);
    // ... all keywords
    map
});

pub fn lookup_ident(ident: &str) -> token_type {
    let lower_ident = ident.to_lowercase();
    KEYWORDS.get(&lower_ident)
        .map(|s| s.to_string())
        .unwrap_or_else(|| IDENT.to_string())
}
```

2. **Character-by-Character Reading:**
   - Current approach is fine for learning
   - For production: Consider using `chars().peekable()` iterator or byte-level parsing with UTF-8 validation

3. **Token Storage:**
   - Current: `token_type` and `token` both stored as `String`
   - **Optimization:** Use `Cow<'static, str>` for keywords (avoid allocation)
   - Or use enum for `token_type` instead of `String`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Keywords
    Select,
    Insert,
    Update,
    Delete,
    // Special
    Ident,
    Number,
    String,
    LocalVar,      // @var
    GlobalVar,     // @@var
    LocalTempTable,  // #table
    GlobalTempTable, // ##table
    // Operators
    Assign,
    Plus,
    // ... etc
}
```

---

## Summary

This analysis covers the essential SAP ASE (Sybase) T-SQL dialect features needed for lexer/parser implementation:

### Critical Lexer Requirements
1. **Nested block comment support** (`/* /* */ */`)
2. **Variable prefix handling** (`@`, `@@`, `#`, `##`)
3. **Unicode escape sequences** (`U&'\xxxx'`)
4. **Multi-character operators** (`||`, `<=`, `!=`, `<>`)
5. **Comprehensive keyword recognition**

### Unique ASE Features
- Nested comments (unlike standard SQL)
- `U&` Unicode escapes (not `N'` like MS SQL)
- Deferred name resolution in procedures
- Aggressive `@@error`/`@@rowcount` reset behavior
- Both `+` and `||` for string concatenation

### Migration Complexity
- **High:** Error handling, global variables, nested comments
- **Medium:** Data types (MONEY, UNITEXT), functions (CHARINDEX, DATEADD), procedure syntax
- **Low:** Basic SQL structure (SELECT, JOIN, WHERE)

This document should serve as a comprehensive reference for implementing a robust SAP ASE T-SQL lexer and parser.
