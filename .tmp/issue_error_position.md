## Issue from PR Review

Source: PR #11 comment (gemini-code-assist)

### Problem
`ParseError::UnexpectedToken`'s `position()` method returns a dummy `Position` with line 0 and column 0. This makes error messages less useful for users.

### Current State
`Token` only contains `Span`, but position information (line, column) is available at lexing time.

### Proposed Solution
Either:
1. Pass more accurate position information when constructing `ParseError`
2. Add functionality to `Lexer` to convert `Span` to `Position`
3. Include position tracking in `TokenBuffer`

### Priority
Medium (user experience)

### Files
- `crates/tsql-parser/src/error.rs` (around line 107)
