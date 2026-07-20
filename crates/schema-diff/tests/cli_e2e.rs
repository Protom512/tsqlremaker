//! T11.3 UC-1 end-to-end black-box integration test for the `schema-diff` CLI.
//!
//! Reproduces use-case UC-1 (新規テーブル作成) at the process boundary:
//! the CLI is fed an *empty* current catalog JSON (`{"tables":[],"indices":[]}`)
//! and a `CREATE TABLE users (id INT)` desired DDL, and the test asserts the
//! STDOUT contains a `CREATE TABLE` for `users` and the process exits 0.
//!
//! This is a black-box test (design §5 / tasks.md Task 11.1): it drives the
//! published binary via `std::process::Command`-based `assert_cmd::cargo_bin`
//! and inspects only STDOUT / STDERR / exit-code — never the lib API. It is
//! the first USABLE end-to-end artifact of the schema-diff work-stream
//! (#162 / Issue #190, acceptance criterion AC-1).
//!
//! Quality gate (default features — no `ase`): `cargo nextest run -p
//! schema-diff`. The CLI builds and runs entirely on the default feature set
//! per design §0.1 (JsonCatalogProvider only, no ase-rs dependency).

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use std::fs;
use std::io::Write;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::str::contains;

/// Path to the `schema-diff` binary, resolved via the `assert_cmd` cargo-bin
/// helper (stdlib `current_exe`-based resolution; works on Rust 1.85+, the
/// workspace ships 1.95). Resolved once per test process.
fn bin() -> Command {
    Command::new(cargo_bin("schema-diff"))
}

/// Writes `contents` to a fresh temp file under the ambient temp dir and
/// returns its absolute path. Used to materialize both the `--current` JSON
/// and `--desired` DDL fixtures without depending on a third-party temp crate.
fn write_temp(name: &str, contents: &str) -> String {
    let dir =
        std::env::temp_dir().join(format!("schema-diff-t11-3-{}-{}", std::process::id(), name));
    fs::create_dir_all(&dir).expect("temp dir creation must not fail in tests");
    let path = dir.join(name);
    let mut f = fs::File::create(&path).expect("temp file creation must not fail in tests");
    f.write_all(contents.as_bytes())
        .expect("temp file write must not fail in tests");
    path.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// UC-1: 新規テーブル作成 — empty current + CREATE TABLE desired
// ---------------------------------------------------------------------------

#[test]
fn uc1_create_table_from_empty_catalog_emits_create_table_and_exits_zero() {
    // Empty current catalog: no tables, no indices. This is the "greenfield"
    // baseline — every table in `desired` must surface as a CREATE TABLE.
    let current_json = r#"{"schema_name":"dbo","tables":[],"indices":[]}"#;
    let current_path = write_temp("current.json", current_json);

    // Desired DDL: a single new table. INT is a universally-supported type
    // across all three target dialects (mysql / postgresql / sqlite), so UC-1
    // must succeed for every `--dialect`.
    let desired_ddl = "CREATE TABLE users (id INT)";
    let desired_path = write_temp("desired.sql", desired_ddl);

    // Drive the binary end-to-end via the mysql dialect (the spec's first
    // listed dialect). The UC-1 contract is dialect-agnostic at this level:
    // any supported dialect must emit a CREATE TABLE and exit 0.
    bin()
        .args([
            "--current",
            &current_path,
            "--desired",
            &desired_path,
            "--dialect",
            "mysql",
        ])
        .assert()
        .success()
        .stdout(contains("CREATE TABLE"))
        .stdout(contains("users"));
}

#[test]
fn uc1_create_table_works_for_postgresql_dialect() {
    // Same UC-1 scenario, postgresql dialect — guards against dialect-dispatch
    // regressions (the postgres `EmissionConfig` vs mysql/sqlite `EmitterConfig`
    // naming divergence flagged in the estimate approval, condition #3).
    let current_path = write_temp("current.json", r#"{"tables":[],"indices":[]}"#);
    let desired_path = write_temp("desired.sql", "CREATE TABLE users (id INT)");

    bin()
        .args([
            "--current",
            &current_path,
            "--desired",
            &desired_path,
            "--dialect",
            "postgresql",
        ])
        .assert()
        .success()
        .stdout(contains("CREATE TABLE"))
        .stdout(contains("users"));
}

#[test]
fn uc1_create_table_works_for_sqlite_dialect() {
    // Same UC-1 scenario, sqlite dialect — completes the three-dialect matrix.
    let current_path = write_temp("current.json", r#"{"tables":[],"indices":[]}"#);
    let desired_path = write_temp("desired.sql", "CREATE TABLE users (id INT)");

    bin()
        .args([
            "--current",
            &current_path,
            "--desired",
            &desired_path,
            "--dialect",
            "sqlite",
        ])
        .assert()
        .success()
        .stdout(contains("CREATE TABLE"))
        .stdout(contains("users"));
}
