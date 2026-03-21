## Issue from PR Review

Source: PR #11 comment (gemini-code-assist)

### Problem
`parse_table_constraint` only correctly handles `Primary Key` constraints. Other constraint types (`Foreign Key`, `Unique`, `Check`) are all incorrectly processed as `Unique` constraints.

### Current Implementation
The match statement only has an arm for `Primary`, causing all other constraint keywords to fall through to incorrect handling.

### Proposed Solution
Add separate match arms for `Foreign`, `Unique`, and `Check` keywords with proper parsing logic for each constraint type.

### Priority
High

### Files
- `crates/tsql-parser/src/parser.rs` (around line 945)
