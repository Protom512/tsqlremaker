## Issue from PR Review

Source: PR #11 comment (gemini-code-assist)

### Problem
The definition of `is_synchronization_point` in `design.md` is inconsistent:
- Lines 621-633: Does NOT include `TokenKind::Alter` and `TokenKind::Drop`
- Lines 1300-1315 (Supporting References section): DOES include them

### Impact
Documentation inaccuracy could lead to confusion about parser synchronization behavior.

### Proposed Solution
Unify the definitions by ensuring both sections list the same tokens.

### Priority
Medium

### Files
- `.kiro/specs/tsql-parser/design.md`
