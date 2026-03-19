## Issue from PR Review

Source: PR #11 comment (CodeRabbit)

### Problem
When parsing statements inside a `BEGIN...END` block, errors are pushed to `self.errors` and parsing continues, but the function returns `Ok(...)` even when errors occurred. The caller receives a potentially incomplete block without knowing about internal parse failures.

### Current Implementation
Errors are collected but not propagated to the caller.

### Proposed Solution
Either:
1. Propagate the first error and fail the block parsing
2. Return a result type that indicates partial success
3. Add a method to check if errors were collected during parsing

### Priority
Major

### Files
- `crates/tsql-parser/src/parser.rs` (parse_block, around line 1529)
