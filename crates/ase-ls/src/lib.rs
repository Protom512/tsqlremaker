//! SAP ASE Language Server — tower-lsp based LSP implementation.
//!
//! Provides T-SQL language intelligence for SAP ASE (Sybase) in VSCode and Zed.

pub mod config;
pub mod error_taxonomy;
pub mod panic_recovery;
pub mod server;

pub use server::AseLanguageServer;
