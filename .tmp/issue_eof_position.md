## Issue from PR Review

Source: PR #11 comment (CodeRabbit)

### Problem
`position_at_eof()` returns a zeroed position (line 0, column 0), so EOF-related diagnostics point to an unhelpful location.

### Proposed Solution
Track the last token span (or lexer position) and use that for EOF tokens/errors to improve error location accuracy.

### Priority
Minor (UX improvement)

### Files
- `crates/tsql-parser/src/buffer.rs` (around line 206)
