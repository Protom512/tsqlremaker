//! Catalog adapters (design §3.5 / tasks.md Group E).
//!
//! Concrete implementations of the [`crate::catalog::CatalogProvider`] trait.
//!
//! - [`json`]: `JsonCatalogProvider` — parses a catalog JSON dump string into
//!   a [`crate::catalog::CatalogSchema`]. Always compiled (no feature gate);
//!   it is the default provider used by the T11 CLI and the T8.4 round-trip
//!   tests.
//!
//! T9 will add a `#[cfg(feature = "ase")] pub mod ase;` line below for the
//! ase-rs-backed `AseCatalogProvider`. The `json` module is deliberately NOT
//! feature-gated — design §0.1 requires the JSON provider (and therefore the
//! publishable CLI) to build on the default feature set.

pub mod json;

// T9: ASE live-catalog adapter. The module is ALWAYS compiled so that its
// pure, upstream-free type-label → DataType mapping (`map_ase_type_name`)
// stays unit-testable on the default build (the CI-verifiable surface for
// T9.2). Only the items inside `ase.rs` that touch `ase_types` /
// `ase_driver` (`map_ase_type`, `AseCatalogProvider`) are individually
// `#[cfg(feature = "ase")]`-gated — and the `ase-types` / `ase-driver`
// optional git deps stay behind `default = []` so the default publishable
// build never resolves the private upstream (design §0.1 / AC-5 / AC-6).
pub mod ase;
