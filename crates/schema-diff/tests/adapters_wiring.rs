//! T8.2 wiring test (red-first / green).
//!
//! Verifies the module re-export surface only — that the `adapters` module
//! and its `json` submodule are reachable via the public path
//! `schema_diff::adapters::json` without any feature flag (default features).
//! The full `JsonCatalogProvider` implementation (DTO + `CatalogProvider`
//! trait impl + `load_schema`) is T8.3; this test only locks down the module
//! wiring declared by T8.2 so T8.3 can be a drop-in.
//!
//! Design §3.5 / tasks.md Task 8.1: the `ase` feature gate is intentionally
//! NOT applied to `json` — it must compile on the default feature set
//! (T9 adds the `#[cfg(feature = "ase")] pub mod ase;` line, never `json`).

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

#[test]
fn adapters_json_module_is_reachable_on_default_features() {
    // Compile-time reachability: the `adapters` module and `json` submodule
    // must be public on the default feature set (no `--features ase`).
    use schema_diff::adapters::json;

    // The `json` submodule must be a real module (not a private detail):
    // referencing its path keeps the `pub mod json;` declaration honest.
    let _ = std::any::TypeId::of::<json::JsonCatalogProvider>();
}

#[test]
fn adapters_module_is_publicly_listed() {
    // Sanity: `adapters` is a public top-level module of the crate, not a
    // private/internal one. The fully-qualified path must type-check.
    let _ = std::any::TypeId::of::<schema_diff::adapters::json::JsonCatalogProvider>();
}
