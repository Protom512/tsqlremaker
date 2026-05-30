---
name: db-docs-lookup-priority
description: Handle SQL keyword/function name collisions in db_docs lookup by using separate lookup functions for different contexts
triggers:
  - LEFT JOIN hover shows function info instead of keyword
  - RIGHT JOIN completion shows function signature
  - hover shows wrong category for a token
  - lookup returns keyword when function expected
  - lookup_function
  - db_docs.rs lookup priority
  - keyword-function name collision
---

# db_docs Lookup Priority for SQL Keyword/Function Collisions

## The Insight

In ASE T-SQL, some names are both keywords AND functions. `LEFT` is a keyword (LEFT JOIN) and a function (`LEFT(string, length)`). `RIGHT` is the same. When a single `lookup()` function serves both hover (needs keyword info for JOIN context) and signature_help (needs function info for call context), the priority order of the lookup determines which result the user sees.

The principle: **A single lookup function cannot serve all contexts correctly. Provide separate lookup functions: one keyword-first (for hover/completion) and one function-only (for signature_help).**

## Why This Matters

Before this fix:
- Hovering over `LEFT` in `LEFT JOIN users` showed the LEFT() function documentation (string function) instead of the JOIN keyword info
- `signature_help` for `SUBSTRING(` worked, but the `lookup()` would have returned a keyword entry if one existed with the same name

This took discovering that `LEFT JOIN` hover showed wrong info → tracing through `lookup()` → realizing the priority was function-first when it should be keyword-first for hover → but function-only for signature_help.

## Recognition Pattern

This skill applies when:
- Adding a new keyword or function entry to `db_docs.rs` that shares a name with an existing entry
- Hover shows unexpected category (Function when expecting Keyword, or vice versa)
- A token that works in multiple SQL contexts (JOIN clause AND function call) gets wrong documentation
- You see `lookup()` being used in hover.rs, completion.rs, AND signature_help.rs

## The Approach

1. **Maintain separate lookup maps**: `FUNCTION_LOOKUP` for function entries, `OTHER_LOOKUP` for keywords/system variables. Never merge them into a single map.

2. **Keyword-first for general lookup**: `lookup()` should check `OTHER_LOOKUP` first, then fall back to `FUNCTION_LOOKUP`. This ensures hover on `LEFT JOIN` shows keyword info, not function info.

3. **Function-only for signature_help**: `lookup_function()` checks only `FUNCTION_LOOKUP`. When the cursor is inside a function call `LEFT(`, you want the function signature, not the keyword.

4. **When adding new entries**: If the name could be ambiguous, check BOTH maps:
   ```rust
   // Before adding, verify no collision
   assert!(FUNCTION_LOOKUP.get("LEFT").is_none()); // if adding keyword
   assert!(OTHER_LOOKUP.get("LEFT").is_none());    // if adding function
   ```

## Example

```rust
// db_docs.rs — two separate lookups

/// Keyword-first lookup (for hover, completion)
pub fn lookup(name: &str) -> Option<&'static DocEntry> {
    OTHER_LOOKUP.get(name).copied()      // keywords, system variables first
        .or_else(|| FUNCTION_LOOKUP.get(name).copied())  // then functions
}

/// Function-only lookup (for signature_help)
pub fn lookup_function(name: &str) -> Option<&'static DocEntry> {
    FUNCTION_LOOKUP.get(name).copied()   // functions only
}

// Usage in signature_help.rs:
let entry = crate::db_docs::lookup_function(name.as_str())?;  // NOT lookup()

// Usage in hover.rs:
let entry = crate::db_docs::lookup(name)?;  // keyword-first is correct here
```

### Known collisions in this codebase

| Name | Keyword Role | Function Role |
|------|-------------|---------------|
| LEFT | LEFT JOIN | LEFT(string, length) |
| RIGHT | RIGHT JOIN | RIGHT(string, length) |
| COUNT | (not a keyword) | COUNT(expr) |
| IDENTITY | (column property) | IDENTITY function |

### What NOT to do

```rust
// BAD: Single merged lookup with function-first priority
pub fn lookup(name: &str) -> Option<&'static DocEntry> {
    FUNCTION_LOOKUP.get(name).copied()
        .or_else(|| OTHER_LOOKUP.get(name).copied())
}
// This causes LEFT JOIN hover to show LEFT() function info

// BAD: Using lookup() for signature_help
let entry = crate::db_docs::lookup("LEFT")?;  // Returns keyword, not function!
// signature_help should use lookup_function("LEFT")
```
