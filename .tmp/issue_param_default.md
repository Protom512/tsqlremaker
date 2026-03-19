## Issue from PR Review

Source: PR #11 comment (gemini-code-assist)

### Problem
`parse_parameter_definition` checks for `TokenKind::Default` when parsing stored procedure parameter defaults, but T-SQL stored procedure parameter syntax doesn't use the `DEFAULT` keyword.

### Correct Syntax
```
@parameter type = default_value
```

NOT:
```
@parameter type DEFAULT default_value  -- Incorrect for proc params
```

### Current Implementation
The check for `TokenKind::Default` appears to be confusing `CREATE TABLE` column default syntax with stored procedure parameter syntax.

### Proposed Solution
Remove the `TokenKind::Default` check and only check for assignment operators (`=` or `:=`).

### Priority
High

### Files
- `crates/tsql-parser/src/parser.rs` (parse_parameter_definition)
