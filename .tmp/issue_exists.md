## Issue from PR Review

Source: PR #11 comment (gemini-code-assist)

### Problem
The `EXISTS` operator needs to parse subqueries (SELECT statements), but the current `ExpressionParser` doesn't have access to statement parsing methods.

### Current Implementation
The `parse_exists_expression` currently just calls `parse_prefix()` as a placeholder.

### Proposed Solution
Refactor to allow `ExpressionParser` to parse subqueries, or integrate expression parsing into `Parser` itself.

### Priority
High

### Files
- `crates/tsql-parser/src/expression/special.rs`
