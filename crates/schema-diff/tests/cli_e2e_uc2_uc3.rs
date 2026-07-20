//! T11.4 / T11.5 — `schema-diff` CLI UC-2 (add column) and UC-3 (graceful exit)
//! end-to-end integration tests.
//!
//! These complement `cli_e2e.rs` (T11.3 UC-1) and share the same black-box
//! file-path convention (`--current <path>` / `--desired <path>`), driving the
//! compiled binary via `assert_cmd::cargo::cargo_bin`. Default features only
//! (design §0.1 — no `ase`).
//!
//! - UC-2: current table narrower than desired → additive ALTER/CREATE emitted,
//!   no destructive warning on STDERR.
//! - UC-3a: malformed (truncated) current JSON → non-zero exit, empty STDOUT,
//!   error on STDERR, no panic (CatalogError::ParseFailed).
//! - UC-3b: nonexistent `--current` file path → non-zero exit, empty STDOUT,
//!   error on STDERR, no panic (IO error).
//! - AC guards: missing required flag + invalid dialect → non-zero exit.
//!
//! Issue #190 / tasks.md Task 11.1; estimate conditions #4/#5/#6.

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use std::fs;
use std::io::Write;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;

fn bin() -> Command {
    Command::new(cargo_bin("schema-diff"))
}

/// Writes `contents` to a fresh temp file and returns its path. Matches the
/// helper convention in `cli_e2e.rs` (T11.3) so both files stay aligned.
fn write_temp(name: &str, contents: &str) -> String {
    let dir = std::env::temp_dir().join(format!(
        "schema-diff-t11-45-{}-{}",
        std::process::id(),
        name
    ));
    fs::create_dir_all(&dir).expect("temp dir creation must not fail in tests");
    let path = dir.join(name);
    let mut f = fs::File::create(&path).expect("temp file creation must not fail in tests");
    f.write_all(contents.as_bytes())
        .expect("temp file write must not fail in tests");
    path.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// UC-2: add column — current users(id) → desired users(id, email).
// ---------------------------------------------------------------------------

#[test]
fn uc2_added_column_emits_users_ddl_with_no_destructive_warning() {
    // current: a single-column users table.
    let current_json = r#"{
        "schema_name": "dbo",
        "tables": [
            { "name": "users", "columns": [
                { "name": "id", "data_type": { "kind": "Int" }, "nullable": false }
            ], "constraints": [] }
        ],
        "indices": []
    }"#;
    let current_path = write_temp("current.json", current_json);

    // desired: same table plus an email column (purely additive).
    let desired_ddl = "CREATE TABLE users (id INT NOT NULL, email VARCHAR(255) NULL)";
    let desired_path = write_temp("desired.sql", desired_ddl);

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
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Additive change must reference the table; ALTER TABLE is the canonical
    // shape but we accept any statement that mentions `users`.
    assert!(
        stdout.to_lowercase().contains("users"),
        "expected 'users' in stdout for additive change, got: {stdout}"
    );

    // No destructive warning should be emitted for a purely additive change.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.to_lowercase().contains("warning"),
        "did not expect a migration warning for additive change, stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// UC-3a: malformed (truncated) current JSON → non-zero exit, no panic.
// ---------------------------------------------------------------------------

#[test]
fn uc3a_malformed_current_json_exits_nonzero_with_empty_stdout() {
    let truncated = r#"{ "schema_name": "dbo", "tables": [ { "name":"users "#;
    let current_path = write_temp("current_bad.json", truncated);
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
        .unwrap();

    assert!(
        !output.status.success(),
        "malformed JSON must exit non-zero; got status {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("error") || stderr.to_lowercase().contains("parse"),
        "expected an error/parse mention on stderr, got: {stderr}"
    );
    // No partial SQL may leak to STDOUT before the failure.
    assert!(
        output.stdout.is_empty(),
        "stdout must be empty on error, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ---------------------------------------------------------------------------
// UC-3b: nonexistent --current file → IO error → non-zero exit, no panic.
// ---------------------------------------------------------------------------

#[test]
fn uc3b_nonexistent_current_file_exits_nonzero_with_empty_stdout() {
    // A path guaranteed not to exist (and cleaned up if a prior run left it).
    let bogus_dir =
        std::env::temp_dir().join(format!("schema-diff-uc3b-missing-{}", std::process::id()));
    let bogus = bogus_dir
        .join("does-not-exist.json")
        .to_string_lossy()
        .into_owned();
    let _ = fs::remove_file(&bogus);
    let _ = fs::remove_dir(&bogus_dir);

    let desired_path = write_temp("desired.sql", "CREATE TABLE users (id INT)");

    let output = bin()
        .args([
            "--current",
            &bogus,
            "--desired",
            &desired_path,
            "--dialect",
            "mysql",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "nonexistent current file must exit non-zero; got status {:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("error") || stderr.to_lowercase().contains("no such"),
        "expected an IO error mention on stderr, got: {stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout must be empty on IO error, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ---------------------------------------------------------------------------
// AC guards: missing required flag + invalid dialect → non-zero exit.
// ---------------------------------------------------------------------------

#[test]
fn missing_required_flags_exits_nonzero() {
    // No flags at all — clap rejects missing required args with a non-zero code.
    let output = bin().output().unwrap();
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}

#[test]
fn invalid_dialect_exits_nonzero() {
    let current_path = write_temp("current.json", r#"{"tables":[],"indices":[]}"#);
    let desired_path = write_temp("desired.sql", "CREATE TABLE users (id INT)");

    let output = bin()
        .args([
            "--current",
            &current_path,
            "--desired",
            &desired_path,
            "--dialect",
            "oracle",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
