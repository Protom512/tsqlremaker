## Issue from PR Review

Source: PR #11 comment (CodeRabbit)

### Problem
`check_depth()` is called at line 100 of `Parser`, but `self.depth` is never incremented or decremented. This means the recursion limit is ineffective for deeply nested statements (e.g., `IF IF IF ... SELECT`), risking stack overflow.

### Note
The `ExpressionParser` correctly manages depth, but `Parser` does not.

### Proposed Solution
Add depth tracking similar to `ExpressionParser`:
- Increment depth when entering statement parsing
- Decrement depth when exiting
- Check depth before parsing nested statements

### Priority
Major

### Files
- `crates/tsql-parser/src/parser.rs` (parse_statement, check_depth)
