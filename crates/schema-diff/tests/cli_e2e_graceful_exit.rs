//! T11.5 — UC-3 graceful-exit end-to-end CLI tests (Issue #190 / #162).
//!
//! Black-box integration tests driving the published `schema-diff` binary
//! (T11.2, `src/bin/schema-diff.rs`) at the process boundary. These verify
//! the UC-3 contract and estimate condition #6: every malformed-input / IO
//! failure must surface as a *non-zero process exit* with a human-readable
//! message on STDERR, an *empty STDOUT* (no partial SQL leaks before the
//! failure), and — critically — *no panic*.
//!
//! ## "No panic" proof
//!
//! A Rust panic aborts the process with exit code 101. The CLI's contract
//! (estimate condition #2 / `rust-anti-patterns.md`) is that `fn main`
//! returns `process::ExitCode` (never `Result`/`?` that would `unwrap`-panic
//! on `Err`), mapping each error path to `eprintln!` + `ExitCode::from(1)`.
//! We therefore assert the exit code is NOT 101, which is the literal
//! non-panic invariant.
//!
//! ## Error sources covered (condition #6 — both required)
//!
//! - (a) `CatalogError::ParseFailed`: malformed/truncated current JSON
//!   (`uc3a`) AND a syntactically-valid JSON that violates the
//!   `data_type.kind` wire-format tag contract (`uc3b` — the case named in
//!   the T11.5 task description).
//! - (b) IO error: nonexistent `--current` file path (`uc3c`).
//!
//! This file is intentionally separate from `cli_e2e.rs` (T11.3 UC-1) and
//! `cli_e2e_uc2_uc3.rs` (T11.4/T11.5 sibling) to avoid parallel-edit
//! collisions during concurrent agent work; the UC-3 surface is the union of
//! all three files. Quality gate (default features, no `ase`): `cargo nextest
//! run -p schema-diff`.

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use std::fs;
use std::io::Write;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;

/// Resolves the `schema-diff` binary via the `assert_cmd` cargo-bin helper
/// (stdlib `current_exe`-based resolution; workspace ships Rust 1.95).
fn bin() -> Command {
    Command::new(cargo_bin("schema-diff"))
}

/// Writes `contents` to a fresh temp file and returns its absolute path.
static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn write_temp(name: &str, contents: &str) -> String {
    let id = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "schema-diff-t11-5-{}-{}-{}",
        std::process::id(),
        id,
        name.replace('.', "_")
    ));
    fs::create_dir_all(&dir).expect("temp dir creation must not fail in tests");
    let path = dir.join(name);
    let mut f = fs::File::create(&path).expect("temp file creation must not fail in tests");
    f.write_all(contents.as_bytes())
        .expect("temp file write must not fail in tests");
    path.to_string_lossy().into_owned()
}

/// Asserts the UC-3 graceful-exit invariants for a CLI invocation that is
/// expected to fail. Centralizes the four contracts so every UC-3 case
/// enforces them identically:
/// 1. non-zero exit (the primary contract);
/// 2. exit code is NOT 101 (the literal "no panic" proof);
/// 3. STDOUT is empty (no partial SQL leaked);
/// 4. STDERR is non-empty (human-readable message present).
fn assert_graceful_failure(output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "expected non-zero exit, got status {:?}",
        output.status.code()
    );
    assert_ne!(
        output.status.code(),
        Some(101),
        "CLI must not panic (exit 101) on malformed input"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout must be empty on failure, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        !output.stderr.is_empty(),
        "stderr must carry an error message"
    );
}

// ---------------------------------------------------------------------------
// UC-3 (a): truncated / malformed current JSON → CatalogError::ParseFailed.
// ---------------------------------------------------------------------------

#[test]
fn uc3a_truncated_current_json_exits_nonzero_no_panic() {
    // Truncated mid-object: missing closing brace and value. The eager
    // JsonCatalogProvider parser (adapters/json.rs) rejects this at
    // construction with CatalogError::ParseFailed; the CLI must surface it
    // as a non-zero exit + STDERR message, with no panic.
    let truncated = r#"{ "schema_name": "dbo", "tables": [ { "name":"users "#;
    let current_path = write_temp("current_truncated.json", truncated);
    let desired_path = write_temp("desired.sql", "CREATE TABLE users (id INT)");

    let output = bin()
        .args([
            "--current",
            &current_path,
            "--desired",
            &desired_path,
            "--dialect",
            "mysql",
        ])
        .output()
        .expect("spawning the binary must succeed");

    assert_graceful_failure(&output);
    // The STDERR message must hint at the parse/json failure so operators can
    // diagnose the input without re-running with a debugger.
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("parse") || stderr.contains("json") || stderr.contains("error"),
        "expected a parse/json/error mention on stderr, got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// UC-3 (b): unknown data_type kind → CatalogError::ParseFailed.
//
// Distinct from (a): the JSON is *syntactically* valid, but violates the
// wire-format `data_type.kind` tag contract (adapters/json.rs module docs —
// "Any unrecognized `kind` tag → CatalogError::ParseFailed"). This is the
// specific case named in the T11.5 task description ("unknown data_type
// kind"), exercising the serde discriminant rejection path rather than the
// JSON syntax-error path.
// ---------------------------------------------------------------------------

#[test]
fn uc3b_unknown_data_type_kind_exits_nonzero_no_panic() {
    let json = r#"{
        "schema_name": "dbo",
        "tables": [
            { "name": "t", "columns": [
                { "name": "c", "data_type": { "kind": "NoSuchType" } }
            ], "constraints": [] }
        ],
        "indices": []
    }"#;
    let current_path = write_temp("current_unknown_kind.json", json);
    let desired_path = write_temp("desired.sql", "CREATE TABLE t (c INT)");

    let output = bin()
        .args([
            "--current",
            &current_path,
            "--desired",
            &desired_path,
            "--dialect",
            "postgresql",
        ])
        .output()
        .expect("spawning the binary must succeed");

    assert_graceful_failure(&output);
}

// ---------------------------------------------------------------------------
// UC-3 (c): nonexistent --current file path → IO error.
//
// This is the second error source required by estimate condition #6,
// distinct from ParseFailed: the file does not exist at all, so the CLI's
// file-read step fails before any JSON parsing begins.
// ---------------------------------------------------------------------------

#[test]
fn uc3c_nonexistent_current_file_exits_nonzero_no_panic() {
    // A path that is vanishingly unlikely to exist. Precondition: clean up
    // any leftover from a prior run so the test honestly exercises the
    // IO-failure path.
    let bogus_dir =
        std::env::temp_dir().join(format!("schema-diff-uc3c-missing-{}", std::process::id()));
    let bogus = bogus_dir
        .join("does-not-exist.json")
        .to_string_lossy()
        .into_owned();
    let _ = fs::remove_file(&bogus);
    let _ = fs::remove_dir(&bogus_dir);
    assert!(
        !std::path::Path::new(&bogus).exists(),
        "fixture path must not exist: {bogus}"
    );

    let desired_path = write_temp("desired.sql", "CREATE TABLE users (id INT)");

    let output = bin()
        .args([
            "--current",
            &bogus,
            "--desired",
            &desired_path,
            "--dialect",
            "sqlite",
        ])
        .output()
        .expect("spawning the binary must succeed");

    assert_graceful_failure(&output);
}
