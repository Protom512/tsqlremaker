//! Defense-in-depth panic recovery at the server boundary (#139).
//!
//! The core library functions delegated to by LSP handlers
//! (`hover::hover_with_analysis`, `definition::definition_locations`, …) are
//! verified panic-free in production — every `unwrap` / `expect` / `panic!` in
//! `ase-ls-core` lives under `#[cfg(test)]`. A future regression or an
//! unhandled AST shape could still panic, however, and an unwrapped panic in
//! an `async fn` handler tears down the whole language-server process,
//! leaving the editor with "no response" and no clue why.
//!
//! [`guarded`] wraps a *synchronous* pure-function call in
//! [`std::panic::catch_unwind`]. A panic is caught, logged at `ERROR` with the
//! feature name, and surfaced as [`CaughtPanic`]; the handler then maps that
//! to a safe `Ok(None)` response and a user notification instead of crashing.
//!
//! ## Why a sync wrapper, not an async one
//!
//! All core entry points are synchronous pure functions (verified), so the
//! entire panic surface sits inside one sync call. `catch_unwind` over a sync
//! closure satisfies [`UnwindSafe`] directly when the closure borrows only
//! `&DocumentAnalysis` and `Copy` request params. Wrapping the *async* handler
//! body would require [`AssertUnwindSafe`] over a borrowed `&self` (which owns
//! `Arc<RwLock<…>>`) and would catch panics in `await` points (lock-release /
//! client-IO) that are already cancellation-safe under tokio — strictly
//! worse. Callers whose closure borrows a lock guard wrap it in
//! [`AssertUnwindSafe`] explicitly (see `goto_definition` / `references`):
//! that is safe because the core fns are pure and tokio's `RwLock` never
//! poisons.

use std::panic::{catch_unwind, UnwindSafe};

/// Sentinel returned by [`guarded`] when the wrapped closure panicked.
///
/// Carries no payload (the panic is logged at the catch site); it exists only
/// so the caller can branch into the "feature recovered from a panic" path
/// (taxonomy B1 — notify the user).
#[derive(Debug)]
pub struct CaughtPanic;

/// Run a synchronous pure-function closure with panic recovery.
///
/// On success returns `Ok(value)`. On panic, logs at `ERROR` (with `feature`
/// and a best-effort rendering of the panic payload) and returns
/// `Err(CaughtPanic)`; the caller turns that into a safe `Ok(None)` LSP
/// response and a `window/showMessage` notification.
///
/// `f` must be [`UnwindSafe`]. Closures borrowing only `&DocumentAnalysis`
/// and `Copy` request params satisfy this directly; closures borrowing a lock
/// guard must be wrapped in [`std::panic::AssertUnwindSafe`] by the caller.
pub fn guarded<R>(
    feature: &'static str,
    f: impl FnOnce() -> R + UnwindSafe,
) -> Result<R, CaughtPanic> {
    match catch_unwind(f) {
        Ok(value) => Ok(value),
        Err(payload) => {
            tracing::error!(
                feature,
                panic = %format_panic_payload(&payload),
                "caught panic in LSP handler core call; recovering with Ok(None)",
            );
            Err(CaughtPanic)
        }
    }
}

/// Best-effort stringification of a `catch_unwind` payload (`Box<dyn Any + Send>`).
///
/// Panics raised by `panic!("literal")`, `panic!("{}", x)` and the standard
/// library produce `&'static str` or `String` payloads; anything else falls
/// back to a placeholder rather than risking a second panic here.
fn format_panic_payload(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&'static str>() {
        (*msg).to_string()
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn guarded_returns_value_on_success() {
        let result = guarded("test", || 42);
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn guarded_catches_panic_and_returns_caught() {
        let result: Result<i32, CaughtPanic> = guarded("test", || panic!("boom"));
        assert!(matches!(result, Err(CaughtPanic)));
    }

    #[test]
    fn guarded_propagates_option_inner_value() {
        // Mirrors how handlers use it: the core fn returns Option<T>.
        let some: Result<Option<u32>, CaughtPanic> = guarded("test", || Some(7));
        assert_eq!(some.unwrap(), Some(7));

        let none: Result<Option<u32>, CaughtPanic> = guarded("test", || None);
        assert_eq!(none.unwrap(), None);
    }

    #[test]
    fn format_panic_payload_handles_str_and_string() {
        let s: Box<dyn std::any::Any + Send> = Box::new("static");
        assert_eq!(format_panic_payload(&s), "static");

        let st: Box<dyn std::any::Any + Send> = Box::new(String::from("owned"));
        assert_eq!(format_panic_payload(&st), "owned");

        let other: Box<dyn std::any::Any + Send> = Box::new(123u32);
        assert_eq!(format_panic_payload(&other), "<non-string panic payload>");
    }
}
