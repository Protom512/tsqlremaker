# Dogfooding Checklist — SAP ASE Language Server + T-SQL Remaker

GitHub Issue #125

## Overview

This checklist is the primary deliverable template for dogfooding sessions.
Each item is derived directly from Issue #125 investigation categories.
Record pass/fail, observed behavior, severity, and link to any filed issue.

### Severity Definitions

| Severity | Definition |
|----------|------------|
| **P0** | Crash, data loss, or complete feature failure — blocks all dogfooding |
| **P1** | Core feature broken or unusable — severely impacts daily use |
| **P2** | Feature works but with significant quality gaps — workaround exists |
| **P3** | Minor annoyance or cosmetic issue — does not block real work |

### Pass/Fail Criteria

| Result | Definition |
|--------|------------|
| **PASS** | Works as expected, no observable problem |
| **FAIL** | Does not work or produces incorrect output |
| **PARTIAL** | Works for some cases but not all |
| **N/A** | Not tested in this session (note reason) |

---

## Category 1: Parse Accuracy (4 items)

| # | Item | UC | Pass/Fail | Observed Behavior | Severity | Filed Issue |
|---|------|----|-----------|-------------------|----------|-------------|
| 1.1 | Everyday SQL constructs parse without errors | UC-1 | | | | |
| 1.2 | Multi-batch scripts with `GO` separator process correctly | UC-2 | | | | |
| 1.3 | Nested control structures (IF > WHILE > TRY...CATCH) parse correctly | UC-1 | | | | |
| 1.4 | Temp table references (`#temp`, `##global`) resolve correctly | UC-1 | | | | |

**Related known issues:** #82 (parser error recovery), #114 (ALTER TABLE), #115 (EXEC), #116 (CREATE TRIGGER)

---

## Category 2: LSP Feature Quality (13 items)

| # | Item | UC | Pass/Fail | Observed Behavior | Severity | Filed Issue |
|---|------|----|-----------|-------------------|----------|-------------|
| 2.1 | Completion returns context-relevant candidates (not all items) | UC-1 | | | | |
| 2.2 | Hover shows variable type info and table schema info | UC-1 | | | | |
| 2.3 | Go-to-Definition jumps to DECLARE variable declaration | UC-1 | | | | |
| 2.4 | References lists all usages of a variable or table name | UC-1 | | | | |
| 2.5 | Rename propagates changes to all occurrences of a variable | UC-1 | | | | |
| 2.6 | Semantic Tokens colorizes keywords, variables, tables, strings correctly | UC-1 | | | | |
| 2.7 | Diagnostics show understandable error messages at accurate positions | UC-1 | | | | |
| 2.8 | Folding collapses BEGIN...END blocks correctly | UC-1 | | | | |
| 2.9 | Formatting produces natural indentation and line breaks | UC-1 | | | | |
| 2.10 | Signature Help displays function parameter information | UC-1 | | | | |
| 2.11 | Code Actions propose useful quick fixes | UC-1 | | | | |
| 2.12 | Document Symbols outline is accurate | UC-1 | | | | |
| 2.13 | Workspace Symbols search finds symbols across workspace | UC-1 | | | | |

**Related known issues:** #54 (context-free completion), #60 (formatting range), #65 (multi-file), #70 (cross-file definition)

---

## Category 3: Performance (4 items)

| # | Item | UC | Pass/Fail | Observed Behavior | Severity | Filed Issue |
|---|------|----|-----------|-------------------|----------|-------------|
| 3.1 | ~100-line SQL: response under 100ms | UC-1 | | | | |
| 3.2 | ~500-line SQL: no perceptible delay (under 500ms) | UC-1 | | | | |
| 3.3 | ~1000-line SQL: no freeze | UC-2 | | | | |
| 3.4 | Character-by-character typing: no perceptible lag during reparse | UC-3 | | | | |

**Related known issues:** #52 (full document sync, no incremental)

---

## Category 4: UX and Usability (4 items)

| # | Item | UC | Pass/Fail | Observed Behavior | Severity | Filed Issue |
|---|------|----|-----------|-------------------|----------|-------------|
| 4.1 | Setup from first launch to active use is straightforward | UC-1 | | | | |
| 4.2 | Error messages are actionable (suggest next steps) | UC-1 | | | | |
| 4.3 | Language Server status is visible (status bar or equivalent) | UC-1 | | | | |
| 4.4 | Log output is useful for diagnosing problems | UC-3 | | | | |

**Related known issues:** #81 (no configuration support)

---

## Category 5: Edge Case Robustness (3 items)

| # | Item | UC | Pass/Fail | Observed Behavior | Severity | Filed Issue |
|---|------|----|-----------|-------------------|----------|-------------|
| 5.1 | Empty file: no errors emitted | UC-3 | | | | |
| 5.2 | Non-SQL text file opened: no crash | UC-3 | | | | |
| 5.3 | Very long single line (10,000+ characters): no problem | UC-3 | | | | |

**Related known issues:** #82 (no error recovery causes cascade failures on incomplete input)

---

## Use Case Cross-Reference

| Use Case | Description | Checklist Items |
|----------|-------------|-----------------|
| UC-1 | Daily stored procedure development (500+ lines) | 1.1, 1.3, 1.4, 2.1-2.13, 3.1-3.2, 4.1-4.4 |
| UC-2 | Migration SQL script conversion (1000+ lines with GO) | 1.2, 2.7, 3.3 |
| UC-3 | Real-time experience during incomplete SQL typing | 3.4, 4.4, 5.1-5.3 |

---

## Known Technical Debt — User-Impact Assessment

After completing the checklist, assess each known issue from a user perspective:

| Issue | Title | User Impact (P0-P3) | Dogfooding Evidence | Recommended Priority |
|-------|-------|---------------------|---------------------|----------------------|
| #52 | Incremental document sync | | | |
| #54 | Context-free completion | | | |
| #60 | Formatting range support | | | |
| #65 | No multi-file support | | | |
| #70 | Definition only current document | | | |
| #81 | No configuration support | | | |
| #82 | Parser no error recovery | | | |
| #114 | ALTER TABLE parser support | | | |
| #115 | EXEC/EXECUTE parser support | | | |
| #116 | CREATE TRIGGER parser support | | | |

---

## Session Log

| Date | Tester | Duration | Files Tested | Summary |
|------|--------|----------|--------------|---------|
| | | | | |

---

## Instructions

1. **Before each session:** Copy this template into a new section in a session log file.
2. **During testing:** Fill in Pass/Fail, Observed Behavior, and Severity columns.
3. **After finding a bug:** File a GitHub Issue and link it in the "Filed Issue" column.
4. **After each session:** Complete the Session Log table above.
5. **After all sessions:** Fill in the Known Technical Debt assessment table with evidence-based priority recommendations.
