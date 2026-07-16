# Task 1 — Error-Classification Taxonomy (CTO-Approval Gate)

> **Issue:** #139 (bug/LSP): Error handler failures are silent.
> **Status:** Design gate — **APPROVAL REQUIRED before any code (task 2/3/4/5).**
> **Rule:** pre-implementation-checklist.md §6 — the design must fully enumerate the
> branches and not leave "decide later".
> **Scope:** This note contains **no code**. It fixes the taxonomy, the per-handler
> branch mapping, and the `catch_unwind` mechanism decision. Output of task 1 only.

---

## 0. Verified facts (empirical probes, 2026-07-16)

These were confirmed against `master` before classifying — they are the load-bearing
preconditions of the taxonomy, not assertions:

1. **Core library is panic-free in production.** Every `unwrap()` / `expect()` /
   `panic!` / `unreachable!` in `crates/ase-ls-core/src/*.rs` lives under
   `#[cfg(test)]` (e.g. `formatting.rs:136` and `formatting.rs:262` gate the two
   test modules). Non-test code uses only `unwrap_or` / `unwrap_or_default`.
   → `catch_unwind` (task 3) is **defense-in-depth**, not a fix for a live bug.
2. **`futures v0.3.32` is already a transitive dependency** via `tower-lsp 0.20.0`.
   An `async`-wrapper for `catch_unwind` adds **no version conflict**.
3. **`Client::show_message(typ, msg)` and `Client::log_message(typ, msg)` exist in
   tower-lsp 0.20** (`client.rs:128`, `client.rs:163`). `show_message` is already
   used indirectly via `log_message(MessageType::WARNING, …)` at `server.rs:650`
   and `:787`. No new dependency.
4. **Handler count is 22 `async fn`s** in `LanguageServer` impl (incl. lifecycle);
   **18 are request handlers returning `Result<Option<_>>`** (the silent-`None`
   surface). Notification handlers (`did_open`/`did_change`/…) return `()` and
   cannot propagate `None`; their error path is fire-and-forget logging only.
5. **`parse_errors` already lives on `DocumentAnalysis`** (built by
   `parse_with_errors`), so the "parse-error present" signal is observable inside
   every handler without a new field.
6. **Lock poisoning is structurally impossible today**: `tokio::sync::RwLock` does
   not poison on panic (unlike `std::sync`). So branch (b4) "lock-poisoned" is a
   **future-proofing placeholder**, not an active path — documented as such.

---

## 1. The taxonomy — exactly two classes, decided now

### Class A — NORMAL NO-OP (MUST stay silent: no log, no notify)

These are expected, benign states. Logging them would be noise on every keystroke /
every cursor idle. They return `Ok(None)` / empty / passthrough with **zero** side
channels.

| ID | Condition | Where it arises |
|----|-----------|-----------------|
| A1 | **Empty source** — `analysis.source.is_empty()` | `did_open` with empty text; any handler on an empty doc |
| A2 | **Document not open / not loaded** — `get_analysis(uri)` returns `None` | every request handler (single-file mode, stale URI, did_open race) |
| A3 | **No token at cursor** — position outside any token span | goto-def, references, hover, rename, prepare-rename, completion prefix |
| A4 | **Nonexistent / unresolvable symbol** — symbol-table miss, empty result | goto-def → empty `locations`, references → empty, hover → `None` |
| A5 | **Empty query / empty result by contract** | `symbol()` empty query → `None`; `formatting` no edits → `None`; `code_action` no actions → `None` |
| A6 | **Resolve fallthrough by design** — lens/link resolve returns input unchanged when doc unloaded or data malformed | `code_lens_resolve`, `document_link_resolve` |

**Rule:** A-paths are the *contract*. They must NEVER log at WARN/ERROR and must
NEVER `show_message`. UC-3 of the ticket (empty input / unloaded doc / nonexistent
symbol) is fully covered by A1–A4.

### Class B — RECOVERABLE ERROR PATH (MUST log + selectively notify)

These are unexpected; the server stays up but an operator/developer needs a signal.
Every B-path **logs**; only B1 and B3 (see threshold §3) also **notify** the user.

| ID | Condition | Recovery | Log level | Notify? |
|----|-----------|----------|-----------|---------|
| B1 | **`catch_unwind` caught a panic** inside a handler's core call | return `Ok(None)` (req) / continue (notif) | `ERROR` | **YES** (WARNING) |
| B2 | **`parse_errors` non-empty AND handler produced `None`/empty** — the likely cause of the empty answer is visible | return `Ok(None)`; surface parse errors are already published as diagnostics | `DEBUG` | no |
| B3 | **Broken span encountered AND it produced no result** (span `start >= end`, the `#135` guard fired, and the feature yielded nothing) | return `Ok(None)` | `WARN` | **YES** (WARNING) |
| B4 | **Lock-poisoned** *(placeholder — see fact 6; structurally impossible with tokio RwLock today; reserved for a future `std::sync` migration)* | n/a | — | no |
| B5 | **`spawn_blocking` JoinError** (panic/cancel in the watched-files read task) | drop the batch (existing behaviour) | `WARN` | no |
| B6 | **`Result::Err` from a handler that currently can't fail** (defensive — no live `Err` producer) | convert to `Ok(None)` | `ERROR` | no |

**Rule:** B-paths always log through `tracing` (structured, with `uri`, `handler`,
`position`). `show_message` fires only for B1 and B3 (the two "feature silently
produced nothing *abnormally*" cases). B2 stays log-only because parse errors are
*already user-visible* as diagnostics — double-notifying would be noise (threshold
rationale §3).

---

## 2. Per-handler branch mapping table

`✓` = branch is reachable in this handler. `/` = not applicable.

| Handler (server.rs) | A1 empty | A2 not-open | A3 no-token | A4 no-symbol | A5 empty-by-contract | A6 resolve-fallthrough | B1 panic | B2 parse-err+None | B3 broken-span |
|---|---|---|---|---|---|---|---|---|---|
| `completion` (804) | ✓ | ✓ | ✓ | ✓ | | | ✓ | ✓ | |
| `document_symbol` (828) | ✓ | ✓ | | | | | ✓ | ✓ | |
| `folding_range` (840) | ✓ | ✓ | | | ✓(empty vec) | | ✓ | ✓ | ✓ |
| `semantic_tokens_full` (849) | ✓ | ✓ | | | | | ✓ | ✓ | ✓ |
| `semantic_tokens_range` (863) | ✓ | ✓ | | | | | ✓ | ✓ | ✓ |
| `hover` (878) | ✓ | ✓ | ✓ | ✓ | | | ✓ | ✓ | |
| `formatting` (890) | ✓ | ✓ | | | ✓(no edits) | | ✓ | ✓ | |
| `range_formatting` (906) | ✓ | ✓ | | | ✓ | | ✓ | ✓ | |
| `signature_help` (924) | ✓ | ✓ | ✓ | ✓ | | | ✓ | ✓ | |
| `goto_definition` (936) | ✓ | ✓ | ✓ | ✓ | ✓(empty) | | ✓ | ✓ | ✓ |
| `references` (978) | ✓ | ✓ | ✓ | ✓ | ✓(empty) | | ✓ | ✓ | ✓ |
| `code_action` (1016) | ✓ | ✓ | | | ✓(no actions) | | ✓ | ✓ | |
| `code_lens` (1030) | ✓ | ✓ | | | | | ✓ | ✓ | |
| `code_lens_resolve` (1041) | | ✓(data→uri) | | ✓(symbol gone) | | ✓ | ✓ | ✓ | |
| `document_link` (1068) | ✓ | ✓ | | | ✓(empty vec) | | ✓ | ✓ | |
| `document_link_resolve` (1090) | | ✓(data→uri) | | | | ✓ | ✓ | | |
| `inlay_hint` (1110) | ✓ | ✓ | | | | | ✓ | ✓ | ✓ |
| `rename` (1129) | ✓ | ✓ | ✓ | ✓ | | | ✓ | ✓ | ✓ |
| `prepare_rename` (1143) | ✓ | ✓ | ✓ | ✓ | | | ✓ | ✓ | ✓ |
| `symbol` (1158) | | | | ✓ | ✓(empty query) | | ✓ | | |

**Notification handlers (return `()`, no `None` to return — log-only on B):**
`did_open` (661), `did_change` (675), `did_close` (705),
`did_change_configuration` (725), `did_change_watched_files` (735, already logs B5
at `:787`). These cannot be `Ok(None)`-silent; their only error surface is the log
channel. B1 (panic) applies to all of them via the wrapper.

**Summary of the table:**
- **Every request handler** gets B1 (panic guard) and B2 (parse-error context).
- **B3 (broken-span notify)** is scoped to the 8 handlers that walk statement
  spans: folding, both semantic-tokens, goto-def, references, inlay-hint, rename,
  prepare-rename. These are exactly the handlers that consult the `#135` /
  `resolve_block_end` machinery where `span.start >= span.end` arises.
- **A6** is exclusive to the two `*_resolve` handlers by construction.

---

## 3. Notify threshold (fixed, conservative — CTO condition 3)

```
show_message(WARNING) fires IFF:
    (B1 caught-panic)  OR  (B3 broken-span-yielded-nothing)

log (tracing) fires for: B1(ERROR), B2(DEBUG), B3(WARN), B5(WARN), B6(ERROR)
silent for all A
```

**Why B2 is log-only:** parse errors are *already published to the client as
diagnostics* by `publish_diagnostics_for`. A second `show_message` per parse-error
keystroke would double-report and spam the status bar. The DEBUG log gives
developers the server-side trace they need without touching the user.

**Why B1/B3 notify:** they represent "a feature you asked for silently did nothing,
and the reason is NOT visible in your editor" — precisely the dogfooding (#125)
pain point. One status-bar line per occurrence is the contracted noise budget.

---

## 4. `catch_unwind` mechanism decision (CTO gate condition 1)

**Decision: sync-call wrapper around the pure core function, NOT an async wrapper
around the whole handler.**

### Rationale
- All core entry points delegated to by handlers (`definition_locations`,
  `reference_locations`, `hover_with_analysis`, `resolve_lens`, `inlay_hints`,
  `folding_ranges_with_analysis`, etc.) are **synchronous pure functions**
  (verified: none are `async`). The panic surface is entirely inside these sync
  calls.
- `std::panic::catch_unwind` requires `UnwindSafe`. A sync closure over `&analysis`
  + `&params` satisfies this directly. Wrapping the *async* handler body would
  require `AssertUnwindSafe` over a borrowed `&self` (which holds `Arc<RwLock<…>>`
  — not unwind-safe) and would catch panics in `await` points (lock-release /
  client-IO) that are **outside the bug surface** and already cancellation-safe
  under tokio.
- The sync-call wrapper is **minimal-invasive**: it wraps exactly the one call that
  can panic (the core pure-fn), returns `Option<T>` (panicked → `None`), and the
  handler maps `None → Ok(None)` + B1 log/notify. The async scaffolding (lock
  acquire, `get_analysis`, config read) stays unwrapped — it is already panic-free.

### Chosen shape (descriptive, not code to ship in task 1)
```
// per request handler, conceptually:
let result = std::panic::catch_unwind(|| {
    core::feature_with_analysis(&analysis, ...)
});
match result {
    Ok(value) => /* normal A-path mapping */,
    Err(payload) => {
        tracing::error!(handler=%H, uri=%U, panic=?payload, "caught panic");
        self.client.show_message(MessageType::WARNING, "...").await; // B1
        Ok(None)
    }
}
```
A single helper `fn guarded<F: FnOnce() -> R + UnwindSafe>(f: F) -> Option<R>`
centralizes this so no handler hand-rolls the `match`. (Real implementation is
task 2/3; this note fixes only the *shape* and the sync-vs-async decision.)

### SCOPE-OPTIONAL tag (CTO condition 2)
Because fact 1 proves no live panic exists, **task 3 (the wrapper) + its
panic-injection probe test are marked SCOPE-OPTIONAL**. If schedule pressure hits,
tasks 2 (logging) + 4 (notify) + 5 (tests for those) deliver the ticket's stated
value (visibility = log + notify) on their own, and task 3 can be deferred or the
ticket shrunk M→S. Including task 3 is accepted as defense-in-depth, not required
for acceptance.

---

## 5. Non-goals (fixed now — not "decide later")

- No fix of the underlying LSP feature bugs themselves (this ticket = visibility +
  recovery only).
- No remote/distributed logging backend.
- No fine-grained notification tiering (INFO vs WARNING vs ERROR tuning is a
  future extension; threshold is pinned in §3).
- No migration from `tokio::sync::RwLock` (B4 stays a documented placeholder).
- No change to the A-path `Ok(None)` contract — **response shape is invariant**;
  only side-channels (log/notify) are added.

---

## 6. Acceptance gating for downstream tasks

This note is the gate. Until a CTO approves it:
- task 2 (logging) MUST NOT start — it has no level map without §1/§3.
- task 3 (catch_unwind) MUST NOT start — mechanism is undecided without §4.
- task 4 (notify) MUST NOT start — threshold is undecided without §3.
- Serial execution 2→3→4 on `server.rs` is mandatory (CTO condition 4); they share
  one file and parallel dispatch caused the `#133`/`#177` edit-clash incidents.

**On approval**, tasks 2/3/4/5 proceed serially against this mapping; task 7
quality gate runs workspace-wide clippy + nextest Summary paste + wasm feature
check (CTO condition 5).
