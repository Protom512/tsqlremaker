## Issue from PR Review

Source: PR #11 comment (gemini-code-assist)

### Problem
The `TokenBuffer` implementation, especially the buffer recentering logic in `refill_buffer`, is very complex. The logic copies the buffer to keep `cursor` and `filled` counts from growing too large, which is inefficient and error-prone.

### Proposed Solution
Consider using `std::collections::VecDeque` from the standard library for the lookahead buffer. `VecDeque` supports efficient removal from the front and addition to the back, making it ideal for this use case.

### Priority
Medium (refactoring)

### Files
- `crates/tsql-parser/src/buffer.rs` (around line 194)
