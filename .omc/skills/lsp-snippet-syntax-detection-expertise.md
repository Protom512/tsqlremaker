---
name: lsp-snippet-syntax-detection
description: Detect non-comma SQL function syntax before generating LSP snippets to avoid invalid placeholder generation
triggers:
  - snippet completion generates invalid placeholders
  - CAST shows commas instead of AS
  - IDENTITY gets empty parens appended
  - OBJECT_ID snippet contains raw quotes
  - COUNT snippet shows pipe alternatives
  - InsertTextFormat SNIPPET vs PLAIN_TEXT decision
  - is_comma_separated_syntax
  - build_function_snippet
---

# LSP Snippet Syntax Detection

## The Insight

SQL function syntax is NOT uniformly comma-separated. When generating LSP `InsertTextFormat::SNIPPET` completions from a syntax string, you cannot blindly split by comma and wrap each part in `${N:param}`. Three categories of non-comma syntax exist in ASE T-SQL:

1. **AS-separated**: `CAST(expression AS type)` — comma would produce invalid T-SQL
2. **Quoted parameters**: `OBJECT_ID('object_name')` — quotes become part of placeholder, confusing
3. **Pipe alternatives**: `COUNT([DISTINCT] expression | *)` — pipe is not a real separator
4. **No parentheses**: `IDENTITY` — has no function call syntax at all

The principle: **Always validate the syntax structure before choosing SNIPPET vs PLAIN_TEXT. Default to PLAIN_TEXT (safe) when syntax is ambiguous.**

## Why This Matters

If you skip detection, the LSP editor shows broken completions:
- `CAST(${1:expression}, ${2:type})` — user tabs to get `CAST(expr, int)` which is INVALID T-SQL
- `IDENTITY()` — empty parens appended to a non-function
- `OBJECT_ID(${1:'object_name'})` — raw quotes in placeholder text

This took 4 rounds of review-driven fixes to fully resolve (bracket contamination → CAST → IDENTITY → quotes/pipes).

## Recognition Pattern

This skill applies when:
- Adding function completions to an LSP server
- Converting syntax strings or parameter arrays to LSP snippets
- A code review bot flags "placeholders look wrong" or "brackets in snippet"
- Seeing `InsertTextFormat::SNIPPET` in completion code

## The Approach

1. **Use structured data over string parsing**: Store params as a clean `&[&str]` array (e.g., `DocEntry.params`), not the raw syntax string. The syntax string contains brackets, quotes, and pipes that are documentation, not parameter names.

2. **Detect before deciding**: Before choosing SNIPPET, check the syntax string for:
   - Contains `(` and `)` with content between them
   - Inner content does NOT contain `" AS "`, `'`, or `|`
   - If any check fails → use `InsertTextFormat::PLAIN_TEXT` with the raw syntax

3. **Default to safe (PLAIN_TEXT)**: When in doubt, PLAIN_TEXT produces the raw syntax as a label. The user sees the correct syntax without tab-through. SNIPPET is a nice-to-have, not a requirement.

## Example

```rust
// In completion.rs — the detection function
fn is_comma_separated_syntax(syntax: &str) -> bool {
    if let (Some(open), Some(close)) = (syntax.find('('), syntax.rfind(')')) {
        if open < close {
            let inner = &syntax[open + 1..close];
            return !inner.contains(" AS ") && !inner.contains('\'') && !inner.contains('|');
        }
    }
    false // no valid parens → not a function call syntax
}

// Usage: choose format based on detection
let (insert_text, format) = if is_comma_separated_syntax(entry.syntax) {
    (build_function_snippet(entry.name, &entry.params), InsertTextFormat::SNIPPET)
} else {
    (entry.syntax.to_string(), InsertTextFormat::PLAIN_TEXT)
};
```

### What NOT to do

```rust
// BAD: Parse syntax string by comma
// "CONVERT(type, expression[, style])" → split by comma
//   → ["type", "expression[", "style])"]
//   → snippet: "CONVERT(${1:type}, ${2:expression[}, ${3:style])})"
// The bracket contamination makes the snippet unusable.

// GOOD: Use DocEntry.params directly (clean array: ["type", "expression", "style"])
// And only use it when is_comma_separated_syntax() confirms it's safe.
```
