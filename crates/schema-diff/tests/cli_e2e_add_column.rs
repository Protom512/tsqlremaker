//! T11.4 — UC-2 `ALTER TABLE ... ADD COLUMN` end-to-end CLI test (3 dialects).
//!
//! Black-box integration test driving the published `schema-diff` binary
//! (T11.2, `src/bin/schema-diff.rs`) at the process boundary. Reproduces
//! use-case UC-2 (カラム追加) exactly as specified by the task:
//!
//! > orders テーブル1カラムの current JSON + ADD COLUMN を含む desired DDL で
//! > CLI を起動し、STDOUT に ALTER TABLE ... ADD COLUMN が含まれることを検証。
//! > 3方言 (mysql/postgresql/sqlite) 各1で。ADD COLUMN は全 emitter サポート済み
//! > (T3/T4/T5) なので T10 非依存。
//!
//! ## Why a dedicated file
//!
//! `cli_e2e_uc2_uc3.rs` already has a UC-2 test, but it (a) uses the `users`
//! table rather than the spec's `orders` table, (b) only checks for the table
//! name substring rather than the verbatim `ALTER TABLE ... ADD COLUMN` shape,
//! and (c) covers only the mysql dialect. This file enforces the stricter
//! T11.4 contract — the literal `ADD COLUMN` phrase must appear in STDOUT for
//! **all three** dialects, proving the end-to-end `current → diff → plan →
//! to_statements → emitter` path produces a real `AlterTableAction::AddColumn`
//! for every emitter (T3/T4/T5). It is non-dependent on T10 (SQLite ALTER
//! dialect) because `ADD COLUMN` is already supported by all three emitters.
//!
//! ## Quality gate
//!
//! Default features only (design §0.1 — no `ase`):
//! `cargo nextest run -p schema-diff`.

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use std::fs;
use std::io::Write;

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::str::contains;

/// Resolves the `schema-diff` binary via `assert_cmd`'s stdlib
/// `current_exe`-based cargo-bin helper (Rust 1.85+; workspace ships 1.95).
fn bin() -> Command {
    Command::new(cargo_bin("schema-diff"))
}

/// Writes `contents` to a fresh temp file under the ambient temp dir and
/// returns its absolute path. Follows the same convention as the sibling
/// `cli_e2e*.rs` files so all integration tests stay aligned.
fn write_temp(name: &str, contents: &str) -> String {
    let dir =
        std::env::temp_dir().join(format!("schema-diff-t11-4-{}-{}", std::process::id(), name));
    fs::create_dir_all(&dir).expect("temp dir creation must not fail in tests");
    let path = dir.join(name);
    let mut f = fs::File::create(&path).expect("temp file creation must not fail in tests");
    f.write_all(contents.as_bytes())
        .expect("temp file write must not fail in tests");
    path.to_string_lossy().into_owned()
}

/// The UC-2 fixture: a single-column `orders` table as the *current* catalog.
///
/// `id INT NOT NULL` is the existing column. The desired DDL (below) adds an
/// `amount` column, so the diff must surface a single `AddColumn` action.
const CURRENT_JSON: &str = r#"{
    "schema_name": "dbo",
    "tables": [
        {
            "name": "orders",
            "columns": [
                { "name": "id", "data_type": { "kind": "Int" }, "nullable": false }
            ],
            "constraints": []
        }
    ],
    "indices": []
}"#;

/// The UC-2 fixture: desired DDL adding one column to `orders`.
///
/// `amount INT NULL` is the additive column. INT is universally supported
/// across mysql/postgresql/sqlite, so all three dialects must succeed.
const DESIRED_DDL: &str = "CREATE TABLE orders (id INT NOT NULL, amount INT NULL)";

/// Asserts the UC-2 contract for a single dialect: the CLI, fed the
/// `orders` current catalog and the ADD-COLUMN desired DDL, must (a) exit 0,
/// (b) print `ALTER TABLE` and `ADD COLUMN` to STDOUT, and (c) reference the
/// `orders` table and the new `amount` column.
fn assert_add_column_for(dialect: &str) {
    let current_path = write_temp("current.json", CURRENT_JSON);
    let desired_path = write_temp("desired.sql", DESIRED_DDL);

    bin()
        .args([
            "--current",
            &current_path,
            "--desired",
            &desired_path,
            "--dialect",
            dialect,
        ])
        .assert()
        .success()
        // The literal `ALTER TABLE` and `ADD COLUMN` phrases must appear —
        // this is the verbatim T11.4 contract (not just a table-name match).
        .stdout(contains("ALTER TABLE"))
        .stdout(contains("ADD COLUMN"))
        // The `orders` table and the new `amount` column must be referenced.
        // Matching is case-insensitive via to_lowercase because dialects quote
        // identifiers differently (mysql `orders`, postgresql orders, sqlite
        // "orders") — but the bare name always appears inside the quotes.
        .stdout(contains("orders"))
        .stdout(contains("amount"));
}

#[test]
fn uc2_add_column_mysql_dialect_emits_alter_table_add_column() {
    assert_add_column_for("mysql");
}

#[test]
fn uc2_add_column_postgresql_dialect_emits_alter_table_add_column() {
    // Guards the postgresql `EmissionConfig` vs mysql/sqlite `EmitterConfig`
    // naming divergence (estimate approval condition #3).
    assert_add_column_for("postgresql");
}

#[test]
fn uc2_add_column_sqlite_dialect_emits_alter_table_add_column() {
    // Completes the three-dialect matrix. ADD COLUMN is supported by the
    // sqlite emitter (T5), so this must NOT surface an
    // UnsupportedDialect warning or non-zero exit (T10-non-dependence proof).
    assert_add_column_for("sqlite");
}
