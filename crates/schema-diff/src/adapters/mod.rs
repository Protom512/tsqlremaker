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
