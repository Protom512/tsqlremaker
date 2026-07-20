//! # schema-diff CLI (T11)
//!
//! Thin publishable binary that wires the schema-diff library into a usable
//! command-line migration generator (design §0.1 / tasks.md Task 11.1). It is
//! the first end-to-end USABLE artifact of the schema-diff work-stream
//! (#162 / Issue #190).
//!
//! ## Usage
//!
//! ```text
//! schema-diff --current <catalog.json> --desired <ddl.sql> --dialect mysql
//! ```
//!
//! - `--current`: path to a catalog JSON dump (wire-format: see
//!   `adapters::json`). Loaded via `JsonCatalogProvider`.
//! - `--desired`: path to a DDL file (CREATE TABLE / CREATE INDEX statements
//!   in SAP ASE T-SQL dialect). Parsed via `build_desired_schema`.
//! - `--dialect`: one of `mysql` | `postgresql` | `sqlite`. Selects the
//!   emitter that renders the migration statements to SQL.
//!
//! STDOUT receives the rendered migration SQL; STDERR receives any
//! `MigrationWarning`s (design §2.6) in a human-readable form. The process
//! exits 0 on success and 1 on any error (catalog parse failure, desired-DDL
//! parse failure, IO error, or unsupported dialect).
//!
//! ## Error contract (no panics)
//!
//! `fn main` returns [`std::process::ExitCode`] (NOT `Result`) and maps every
//! error path to `eprintln!` + `ExitCode::from(1)`. This is mandatory under
//! the workspace `clippy::panic = "deny"` lint (CTO condition #2) — there is
//! no `?` propagation, no `unwrap`/`expect`, and no `panic!` in the binary.
//!
//! ## Argument order invariant (CTO condition #1)
//!
//! The CLI calls `diff_schema(current, desired)` verbatim — it does NOT wrap
//! `diff_schema` in a helper that flips argument order. This is the
//! non-negotiable invariant from the 2026-07-14 #162 CTO judgment; flipping it
//! would invert DROP/CREATE and silently destroy data.

use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use schema_diff::adapters::json::JsonCatalogProvider;
use schema_diff::catalog::CatalogProvider;
use schema_diff::dialect::Dialect as LibDialect;
use schema_diff::warning::MigrationWarning;
use schema_diff::{build_desired_schema, diff_schema, plan_operations, to_statements_for_dialect};

/// The dialect emitted to STDOUT. Each variant dispatches to the matching
/// emitter crate using the emitter's ACTUAL config-type name (CTO condition
/// #3): mysql/sqlite use `EmitterConfig`, postgresql uses `EmissionConfig`.
///
/// This is a clap `ValueEnum` adapter that converts 1:1 into the library's
/// [`LibDialect`] (the single source of truth in `crates/schema-diff/src/dialect.rs`).
/// Keeping the conversion in one place prevents the variant spelling / ordering
/// from drifting between the CLI and the emit layer (T10-4 wiring).
#[derive(Copy, Clone, Debug, ValueEnum)]
enum Dialect {
    /// MySQL / SAP ASE-compatible rendering.
    Mysql,
    /// PostgreSQL rendering.
    Postgresql,
    /// SQLite rendering.
    Sqlite,
}

impl Dialect {
    /// Converts the clap `ValueEnum` into the library's [`LibDialect`].
    ///
    /// Order-preserving 1:1 mapping — both enums declare variants in the same
    /// MySQL → PostgreSQL → SQLite order, so this conversion cannot drift.
    #[must_use]
    const fn to_lib(self) -> LibDialect {
        match self {
            Self::Mysql => LibDialect::Mysql,
            Self::Postgresql => LibDialect::Postgresql,
            Self::Sqlite => LibDialect::Sqlite,
        }
    }
}

/// schema-diff CLI arguments (design §5 / tasks.md Task 11.1).
#[derive(Parser, Debug)]
#[command(
    name = "schema-diff",
    about = "Compute a DDL migration from a catalog JSON dump to a desired DDL schema"
)]
struct Cli {
    /// Path to the current catalog JSON dump (wire-format; see `adapters::json`).
    #[arg(long)]
    current: String,

    /// Path to the desired DDL file (CREATE TABLE / CREATE INDEX statements).
    #[arg(long)]
    desired: String,

    /// Target dialect for the emitted migration SQL.
    #[arg(long, value_enum)]
    dialect: Dialect,
}

/// Entry point. Returns `ExitCode` so no error path can panic (workspace
/// `clippy::panic = "deny"`).
fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("schema-diff: {message}");
            ExitCode::from(1)
        }
    }
}

/// Runs the full pipeline against `cli`. Returns `Err(String)` so `main` can
/// stay panic-free: the binary is a thin adapter whose only job is to surface
/// a human-readable failure to STDERR and exit non-zero — there is no
/// structured recovery at this layer, so a `String` message suffices.
fn run(cli: &Cli) -> Result<(), String> {
    // ---- current: load catalog JSON via JsonCatalogProvider ----
    let current_json = std::fs::read_to_string(&cli.current)
        .map_err(|e| format!("failed to read --current {}: {e}", cli.current))?;
    let provider = JsonCatalogProvider::new(&current_json)
        .map_err(|e| format!("failed to parse --current catalog: {e}"))?;
    let current = provider
        .load_schema()
        .map_err(|e| format!("failed to load --current catalog: {e}"))?;

    // ---- desired: parse DDL into a CatalogSchema ----
    let desired_ddl = std::fs::read_to_string(&cli.desired)
        .map_err(|e| format!("failed to read --desired {}: {e}", cli.desired))?;
    let desired = build_desired_schema(&desired_ddl)
        .map_err(|e| format!("failed to parse --desired DDL: {e}"))?;

    // ---- diff: diff_schema(current, desired) — argument order invariant ----
    let diff = diff_schema(&current, &desired);

    // Warnings go to STDERR as human-readable lines (design §2.6 / §5).
    for warning in &diff.warnings {
        eprintln!("warning: {}", render_warning(warning));
    }

    // ---- plan: lift the diff into dialect-neutral AlterOperations ----
    let ops = plan_operations(&diff);

    // ---- partition: drop SQLite-unsupported actions, surface as warnings ----
    // design §0.4 / tasks.md Task 10.1: SQLite's native ALTER TABLE limits mean
    // `AlterColumn` (type change) and `DropConstraint` cannot be emitted as-is.
    // `to_statements_for_dialect` partitions per-action (AddColumn survives,
    // AlterColumn is warned + stripped) rather than dropping the whole ALTER.
    // The returned `dialect_warnings` are merged into the same STDERR stream
    // as `diff.warnings` (design §5 / §2.6).
    let (stmts, dialect_warnings) = to_statements_for_dialect(&ops, cli.dialect.to_lib());

    // Dialect-unsupported warnings go to STDERR alongside diff-derived warnings.
    for warning in &dialect_warnings {
        eprintln!("warning: {}", render_warning(warning));
    }

    // ---- emit: dispatch to the selected dialect emitter ----
    let sql = match cli.dialect {
        Dialect::Mysql => {
            let mut emitter =
                mysql_emitter::MySqlEmitter::new(mysql_emitter::EmitterConfig::default());
            emitter
                .emit_batch(&stmts)
                .map_err(|e| format!("mysql emission failed: {e}"))?
        }
        Dialect::Postgresql => {
            let mut emitter = postgresql_emitter::PostgreSqlEmitter::new(
                postgresql_emitter::EmissionConfig::default(),
            );
            emitter
                .emit_batch(&stmts)
                .map_err(|e| format!("postgresql emission failed: {e}"))?
        }
        Dialect::Sqlite => {
            let mut emitter =
                sqlite_emitter::SqliteEmitter::new(sqlite_emitter::EmitterConfig::default());
            emitter
                .emit_batch(&stmts)
                .map_err(|e| format!("sqlite emission failed: {e}"))?
        }
    };

    // STDOUT receives the rendered migration SQL.
    println!("{sql}");
    Ok(())
}

/// Renders a `MigrationWarning` to a single human-readable line for STDERR.
///
/// `MigrationWarning` variants are owned-`String` (no lifetimes) so this
/// formatter has no borrow entanglement — it is safe to call from the binary.
fn render_warning(w: &MigrationWarning) -> String {
    match w {
        MigrationWarning::Destructive { target, detail } => {
            format!("destructive change at {target}: {detail}")
        }
        MigrationWarning::UnsupportedDialect { dialect, operation } => {
            format!("unsupported in {dialect}: {operation}")
        }
    }
}
