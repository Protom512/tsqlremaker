# T-SQL Lexer Implementation Roadmap

**Project:** tsqlremaker  
**Target Dialect:** SAP ASE (Sybase Adaptive Server Enterprise) T-SQL  
**Date:** January 19, 2026

---

## Current Status

### ✅ Implemented (Phase 1 - Partial)

**Token Types** (`tsql-token/src/lib.rs`):
- Keywords: `SELECT`, `UPDATE`, `DELETE`, `INSERT`, `CREATE`, `FROM`, `WHERE`, `AS`, `EXEC`, `IF`
- Operators: `=` (assign), `(`, `)`, `,`
- Special tokens: `EOF`, `ILLEGAL`, `IDENT`, `NUM`
- Function: `lookup_ident()` for keyword matching (case-insensitive)

**Lexer** (`tsql-lexer/src/lib.rs`):
- Character-by-character cursor navigation
- Whitespace handling (`eat_whitespace()`)
- Identifier reading (alphanumeric, `_`, `.`)
- Number reading (integers only)
- Basic token matching

**Tests**:
- Simple SELECT query (`tests/token.rs`)
- Stored procedure with block comment (`tests/procedure_test.rs`)

### ❌ Missing Critical Features

1. **Comment handling** - Block comments `/* */` present in test but not implemented
2. **Variable prefixes** - No `@`, `@@`, `#`, `##` support
3. **String literals** - No string parsing
4. **Extended operators** - Only `=`, `(`, `)`, `,` supported
5. **Extended keywords** - Limited keyword set

### 🐛 Known Issues

**`tsql-lexer/src/lib.rs` Line 135:**
```rust
fn read_block_comment(&self) -> String {
    let mut result = String::new();
    let mut inside_comment = 0;
    let mut chars = sql.chars().peekable();  // ❌ ERROR: `sql` not defined
    // ...
}
```
- Method defined but never called
- References undefined variable `sql`
- Should be `self.input.chars()...`

---

## Implementation Phases

### Phase 1: Comment Support (HIGH PRIORITY)

**Why:** Test files already use block comments, currently failing

**Tasks:**
1. Fix `read_block_comment()` implementation
2. Add nested comment support (stack-based tracking)
3. Add line comment support (`--`)
4. Integrate into `next_token()` matching

**Implementation:**
```rust
pub fn next_token(&mut self) -> Token {
    self.eat_whitespace();
    
    match self.ch {
        '/' => {
            if self.peek_char() == '*' {
                let comment = self.read_block_comment();
                // Option 1: Skip comments, call next_token() recursively
                // Option 2: Return COMMENT token for preservation
                return self.next_token();
            }
            // Handle division operator if needed
        }
        '-' => {
            if self.peek_char() == '-' {
                let comment = self.read_line_comment();
                return self.next_token();
            }
            // Handle minus operator
        }
        // ... rest of matching
    }
}

fn read_block_comment(&mut self) -> String {
    let mut result = String::from("/*");
    let mut depth = 1;
    
    self.read_char(); // consume '/'
    self.read_char(); // consume '*'
    
    while depth > 0 && !self.check_eof() {
        if self.ch == '/' && self.peek_char() == '*' {
            depth += 1;
            result.push(self.ch);
            self.read_char();
            result.push(self.ch);
            self.read_char();
        } else if self.ch == '*' && self.peek_char() == '/' {
            depth -= 1;
            result.push(self.ch);
            self.read_char();
            result.push(self.ch);
            if depth > 0 {
                self.read_char();
            }
        } else {
            result.push(self.ch);
            self.read_char();
        }
    }
    
    if depth > 0 {
        panic!("Unterminated block comment");
    }
    
    result
}

fn read_line_comment(&mut self) -> String {
    let mut result = String::from("--");
    self.read_char(); // consume first '-'
    self.read_char(); // consume second '-'
    
    while self.ch != '\n' && !self.check_eof() {
        result.push(self.ch);
        self.read_char();
    }
    
    result
}
```

**Test Cases:**
```sql
/* Simple block comment */
/* Outer /* nested */ comment */
-- Line comment
SELECT col1, -- inline comment
       col2
/* /* /* triple nested */ */ */
```

### Phase 2: Variable and Special Prefixes (HIGH PRIORITY)

**Why:** Essential for stored procedure parsing

**Token Additions** (`tsql-token/src/lib.rs`):
```rust
pub const LOCAL_VAR: &str = "LOCAL_VAR";        // @variable
pub const GLOBAL_VAR: &str = "GLOBAL_VAR";      // @@variable
pub const TEMP_TABLE: &str = "TEMP_TABLE";      // #table
pub const GLOBAL_TEMP_TABLE: &str = "GLOBAL_TEMP_TABLE"; // ##table
```

**Lexer Updates** (`tsql-lexer/src/lib.rs`):
```rust
match self.ch {
    '@' => {
        if self.peek_char() == '@' {
            // Global variable @@var
            self.read_char();
            self.read_char();
            let name = self.read_identifier();
            Token::new(GLOBAL_VAR, format!("@@{}", name))
        } else {
            // Local variable @var
            self.read_char();
            let name = self.read_identifier();
            Token::new(LOCAL_VAR, format!("@{}", name))
        }
    }
    '#' => {
        if self.peek_char() == '#' {
            // Global temp table ##table
            self.read_char();
            self.read_char();
            let name = self.read_identifier();
            Token::new(GLOBAL_TEMP_TABLE, format!("##{}", name))
        } else {
            // Local temp table #table
            self.read_char();
            let name = self.read_identifier();
            Token::new(TEMP_TABLE, format!("#{}", name))
        }
    }
    // ... existing matches
}
```

**Helper Method Update:**
```rust
// Modify read_identifier() to be reusable
fn read_identifier(&mut self) -> String {
    let start_pos = self.position;
    while self.ch.is_alphanumeric() || self.ch == '_' || self.ch == '.' {
        self.read_char();
    }
    self.input[start_pos..self.position].to_string()
}

// Update read_identity() to use read_identifier()
fn read_identity(&mut self) -> String {
    let literal = self.read_identifier();
    literal
}
```

**Test Cases:**
```sql
DECLARE @count INT, @name VARCHAR(50)
SELECT @count = @@rowcount
IF @@error != 0 RETURN -1
CREATE TABLE #temp (id INT)
CREATE TABLE ##global_temp (id INT)
```

### Phase 3: String Literals (HIGH PRIORITY)

**Token Addition:**
```rust
pub const STRING: &str = "STRING";
pub const UNICODE_STRING: &str = "UNICODE_STRING";
```

**Lexer Implementation:**
```rust
match self.ch {
    '\'' => {
        let literal = self.read_string_literal('\'');
        Token::new(STRING, literal)
    }
    '"' => {
        // Check quoted_identifier setting in real implementation
        let literal = self.read_string_literal('"');
        Token::new(STRING, literal)
    }
    'U' | 'u' => {
        if self.peek_char() == '&' {
            self.read_char(); // U
            self.read_char(); // &
            if self.ch == '\'' || self.ch == '"' {
                let literal = self.read_unicode_string();
                Token::new(UNICODE_STRING, literal)
            } else {
                panic!("Expected string literal after U&");
            }
        } else {
            // Regular identifier
            let literal = self.read_identity();
            Token::new(lookup_ident(&literal), literal)
        }
    }
    // ...
}

fn read_string_literal(&mut self, quote: char) -> String {
    let mut result = String::new();
    result.push(quote);
    self.read_char(); // consume opening quote
    
    while self.ch != quote && !self.check_eof() {
        if self.ch == quote && self.peek_char() == quote {
            // Escaped quote: '' or ""
            result.push(self.ch);
            self.read_char();
            result.push(self.ch);
            self.read_char();
        } else if self.ch == '\\' && self.peek_char() == '\n' {
            // Line continuation
            self.read_char(); // skip \
            self.read_char(); // skip \n
        } else {
            result.push(self.ch);
            self.read_char();
        }
    }
    
    if self.ch == quote {
        result.push(self.ch);
        self.read_char();
    } else {
        panic!("Unterminated string literal");
    }
    
    result
}

fn read_unicode_string(&mut self) -> String {
    let quote = self.ch;
    let mut result = String::from("U&");
    result.push(quote);
    self.read_char(); // consume opening quote
    
    while self.ch != quote && !self.check_eof() {
        if self.ch == '\\' {
            result.push(self.ch);
            self.read_char();
            
            if self.ch == '\\' {
                // Escaped backslash
                result.push(self.ch);
                self.read_char();
            } else if self.ch == '+' {
                // \+yyyyyy format
                result.push(self.ch);
                self.read_char();
                for _ in 0..6 {
                    result.push(self.ch);
                    self.read_char();
                }
            } else {
                // \xxxx format
                for _ in 0..4 {
                    result.push(self.ch);
                    self.read_char();
                }
            }
        } else {
            result.push(self.ch);
            self.read_char();
        }
    }
    
    if self.ch == quote {
        result.push(self.ch);
        self.read_char();
    }
    
    result
}
```

**Test Cases:**
```sql
SELECT 'Simple string'
SELECT 'String with ''escaped quote'''
SELECT "Double quoted string"
SELECT 'It''s a test'
SELECT U&'\0041\0042\0043'  -- ABC
SELECT U&'Caf\00E9'         -- Café
SELECT U&'\+01F600'         -- Emoji
```

### Phase 4: Extended Operators (MEDIUM PRIORITY)

**Token Additions:**
```rust
// Arithmetic
pub const PLUS: &str = "+";
pub const MINUS: &str = "-";
pub const MULTIPLY: &str = "*";
pub const DIVIDE: &str = "/";
pub const MODULO: &str = "%";

// Comparison
pub const LT: &str = "<";
pub const GT: &str = ">";
pub const LTE: &str = "<=";
pub const GTE: &str = ">=";
pub const NOT_EQUAL: &str = "!=";
pub const NOT_EQUAL_ALT: &str = "<>";
pub const NOT_LT: &str = "!<";
pub const NOT_GT: &str = "!>";

// Bitwise
pub const BITWISE_AND: &str = "&";
pub const BITWISE_OR: &str = "|";
pub const BITWISE_XOR: &str = "^";
pub const BITWISE_NOT: &str = "~";

// String
pub const CONCAT: &str = "||";

// Others
pub const SEMICOLON: &str = ";";
pub const DOT: &str = ".";
pub const COLON: &str = ":";
```

**Lexer Implementation:**
```rust
match self.ch {
    '+' => Token::new(PLUS, "+".to_string()),
    '-' => {
        if self.peek_char() == '-' {
            // Line comment (already handled above)
        } else {
            Token::new(MINUS, "-".to_string())
        }
    },
    '*' => Token::new(MULTIPLY, "*".to_string()),
    '/' => {
        if self.peek_char() == '*' {
            // Block comment (already handled)
        } else {
            Token::new(DIVIDE, "/".to_string())
        }
    },
    '%' => Token::new(MODULO, "%".to_string()),
    '<' => {
        match self.peek_char() {
            '=' => {
                self.read_char();
                Token::new(LTE, "<=".to_string())
            }
            '>' => {
                self.read_char();
                Token::new(NOT_EQUAL_ALT, "<>".to_string())
            }
            _ => Token::new(LT, "<".to_string())
        }
    }
    '>' => {
        if self.peek_char() == '=' {
            self.read_char();
            Token::new(GTE, ">=".to_string())
        } else {
            Token::new(GT, ">".to_string())
        }
    }
    '!' => {
        match self.peek_char() {
            '=' => {
                self.read_char();
                Token::new(NOT_EQUAL, "!=".to_string())
            }
            '<' => {
                self.read_char();
                Token::new(NOT_LT, "!<".to_string())
            }
            '>' => {
                self.read_char();
                Token::new(NOT_GT, "!>".to_string())
            }
            _ => Token::new(ILLEGAL, "!".to_string())
        }
    }
    '&' => Token::new(BITWISE_AND, "&".to_string()),
    '|' => {
        if self.peek_char() == '|' {
            self.read_char();
            Token::new(CONCAT, "||".to_string())
        } else {
            Token::new(BITWISE_OR, "|".to_string())
        }
    }
    '^' => Token::new(BITWISE_XOR, "^".to_string()),
    '~' => Token::new(BITWISE_NOT, "~".to_string()),
    ';' => Token::new(SEMICOLON, ";".to_string()),
    ':' => Token::new(COLON, ":".to_string()),
    '.' => Token::new(DOT, ".".to_string()),
    // ... existing matches
}
```

### Phase 5: Extended Keywords (MEDIUM PRIORITY)

**Add to `tsql-token/src/lib.rs`:**
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
pub const ELSE: &str = "else";

// DDL
pub const ALTER: &str = "alter";
pub const DROP: &str = "drop";
pub const TABLE: &str = "table";
pub const PROCEDURE: &str = "procedure";
pub const PROC: &str = "proc";
pub const FUNCTION: &str = "function";

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

// Procedure-specific
pub const DECLARE: &str = "declare";
pub const OUTPUT: &str = "output";
pub const PRINT: &str = "print";
```

**Update `lookup_ident()` with all keywords**

### Phase 6: Enhanced Number Parsing (LOW PRIORITY)

**Current:** Only integer literals  
**Target:** Decimals, scientific notation, negative numbers

```rust
fn read_number(&mut self) -> (String, &'static str) {
    let start_pos = self.position;
    let mut has_decimal = false;
    let mut has_exponent = false;
    
    // Handle negative sign
    if self.ch == '-' {
        self.read_char();
    }
    
    // Integer part
    while self.ch.is_numeric() {
        self.read_char();
    }
    
    // Decimal part
    if self.ch == '.' && self.peek_char().is_numeric() {
        has_decimal = true;
        self.read_char(); // consume '.'
        while self.ch.is_numeric() {
            self.read_char();
        }
    }
    
    // Exponent part
    if self.ch == 'e' || self.ch == 'E' {
        has_exponent = true;
        self.read_char();
        if self.ch == '+' || self.ch == '-' {
            self.read_char();
        }
        while self.ch.is_numeric() {
            self.read_char();
        }
    }
    
    let literal = self.input[start_pos..self.position].to_string();
    let token_type = if has_decimal || has_exponent {
        "FLOAT"
    } else {
        "INT"
    };
    
    (literal, token_type)
}
```

---

## Performance Optimization

### Issue: HashMap Recreation on Every Lookup

**Current Code:**
```rust
pub fn lookup_ident(ident: &str) -> token_type {
    let mut map = HashMap::new();  // ❌ Created every call!
    map.insert("select".to_lowercase(), SELECT);
    map.insert("if".to_lowercase(), IF);
    // ...
}
```

**Solution: Use Lazy Static HashMap**

Add to `Cargo.toml`:
```toml
[dependencies]
once_cell = "1.19"
```

Update `tsql-token/src/lib.rs`:
```rust
use once_cell::sync::Lazy;
use std::collections::HashMap;

static KEYWORDS: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert("select", SELECT);
    map.insert("if", IF);
    map.insert("create", CREATE);
    map.insert("update", UPDATE);
    map.insert("insert", INSERT);
    map.insert("delete", DELETE);
    map.insert("from", FROM);
    map.insert("where", WHERE);
    map.insert("as", AS);
    map.insert("exec", EXEC);
    map.insert("begin", BEGIN);
    map.insert("end", END);
    // ... add all keywords
    map
});

pub fn lookup_ident(ident: &str) -> token_type {
    let lower_ident = ident.to_lowercase();
    KEYWORDS
        .get(lower_ident.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| IDENT.to_string())
}
```

**Impact:** Massive performance improvement, especially for keyword-heavy SQL

---

## Testing Strategy

### Unit Tests per Phase

**Phase 1 (Comments):**
```rust
#[test]
fn test_block_comment() {
    let sql = "/* comment */ SELECT";
    let mut lexer = Lexer::new(sql);
    let token = lexer.next_token();
    assert_eq!(token.token_type(), SELECT);
}

#[test]
fn test_nested_comment() {
    let sql = "/* outer /* nested */ still comment */ SELECT";
    let mut lexer = Lexer::new(sql);
    let token = lexer.next_token();
    assert_eq!(token.token_type(), SELECT);
}

#[test]
fn test_line_comment() {
    let sql = "-- comment\nSELECT";
    let mut lexer = Lexer::new(sql);
    let token = lexer.next_token();
    assert_eq!(token.token_type(), SELECT);
}
```

**Phase 2 (Variables):**
```rust
#[test]
fn test_local_variable() {
    let sql = "@count";
    let mut lexer = Lexer::new(sql);
    let token = lexer.next_token();
    assert_eq!(token.token_type(), LOCAL_VAR);
    assert_eq!(token.token(), "@count");
}

#[test]
fn test_global_variable() {
    let sql = "@@rowcount";
    let mut lexer = Lexer::new(sql);
    let token = lexer.next_token();
    assert_eq!(token.token_type(), GLOBAL_VAR);
    assert_eq!(token.token(), "@@rowcount");
}
```

**Phase 3 (Strings):**
```rust
#[test]
fn test_string_literal() {
    let sql = "'Hello World'";
    let mut lexer = Lexer::new(sql);
    let token = lexer.next_token();
    assert_eq!(token.token_type(), STRING);
    assert_eq!(token.token(), "'Hello World'");
}

#[test]
fn test_escaped_quote() {
    let sql = "'It''s working'";
    let mut lexer = Lexer::new(sql);
    let token = lexer.next_token();
    assert_eq!(token.token(), "'It''s working'");
}

#[test]
fn test_unicode_string() {
    let sql = r"U&'\0041\0042\0043'";
    let mut lexer = Lexer::new(sql);
    let token = lexer.next_token();
    assert_eq!(token.token_type(), UNICODE_STRING);
}
```

### Integration Tests

**Real stored procedure from test file:**
```rust
#[test]
fn test_complete_procedure() {
    let sql = r"
/*****************************************************************************
 * block comment
 *
 *****************************************************************************/
create proc proc1 as
  select clm1,
         clm2
    from db1..table1
";
    let mut lexer = Lexer::new(sql);
    let mut tokens = Vec::new();
    
    loop {
        let token = lexer.next_token();
        if token.token_type() == EOF {
            break;
        }
        tokens.push(token);
    }
    
    // Validate token sequence
    assert_eq!(tokens[0].token_type(), CREATE);
    assert_eq!(tokens[1].token_type(), PROC);
    assert_eq!(tokens[2].token_type(), IDENT);
    assert_eq!(tokens[2].token(), "proc1");
    // ... more assertions
}
```

---

## Recommendations

### Priority Order (Next Steps)

1. **FIX:** `read_block_comment()` bug (line 135)
2. **IMPLEMENT:** Phase 1 (Comments) - Blocking existing tests
3. **IMPLEMENT:** Phase 2 (Variables) - Essential for procedures
4. **IMPLEMENT:** Phase 3 (Strings) - Essential for real SQL
5. **OPTIMIZE:** Lazy static keyword HashMap
6. **IMPLEMENT:** Phase 4 (Operators) - Medium priority
7. **IMPLEMENT:** Phase 5 (Extended Keywords) - Medium priority
8. **IMPLEMENT:** Phase 6 (Enhanced Numbers) - Low priority

### Architecture Improvements

**Consider Token Enum:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    // Keywords
    Select, Insert, Update, Delete, Create, From, Where,
    // Variables
    LocalVar(String),   // @var
    GlobalVar(String),  // @@var
    // Literals
    Ident(String),
    Integer(i64),
    Float(f64),
    String(String),
    // Operators
    Assign, Plus, Minus, LParen, RParen,
    // Special
    Eof, Illegal,
}

pub struct Token {
    token_type: TokenType,
    line: usize,
    column: usize,
}
```

**Benefits:**
- Type safety
- Better pattern matching
- Smaller memory footprint
- Easier to extend

### Error Handling

**Current:** `panic!()` on errors  
**Target:** Proper error types

```rust
#[derive(Debug)]
pub enum LexError {
    UnterminatedString { line: usize, column: usize },
    UnterminatedComment { line: usize, column: usize },
    InvalidCharacter { ch: char, line: usize, column: usize },
    UnexpectedEof,
}

impl Lexer<'_> {
    pub fn next_token(&mut self) -> Result<Token, LexError> {
        // ...
    }
}
```

---

## Resources

- **Dialect Analysis:** `/docs/SAP_ASE_TSQL_DIALECT_ANALYSIS.md`
- **SAP ASE Documentation:** https://infocenter.sybase.com/
- **Test Files:** `tsql-lexer/tests/`
- **Current Implementation:** `tsql-lexer/src/lib.rs`, `tsql-token/src/lib.rs`

---

## Success Criteria

**Phase 1 Complete:** All existing tests pass (procedure_test.rs)  
**Phase 2 Complete:** Can tokenize stored procedures with variables  
**Phase 3 Complete:** Can tokenize string literals in SELECT statements  
**Phase 4 Complete:** Can tokenize arithmetic and comparison expressions  
**Phase 5 Complete:** Can tokenize full control flow statements  
**Phase 6 Complete:** Can tokenize all numeric literal formats

**Final Goal:** Lexer can tokenize any valid SAP ASE T-SQL code without errors.
