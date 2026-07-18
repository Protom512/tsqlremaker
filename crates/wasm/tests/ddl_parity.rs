//! T2.1c (schema-diff §0.5): wasm `convert_to` downstream parity regression tests.
//!
//! These tests pin the wasm public contract change caused by T2.1b
//! (parser→common-sql DDL 橋渡し: `to_common_sql` `Statement::Create(_) => None`
//! → `Some(SqlStmt::CreateTable(...))` 等). See `.kiro/specs/schema-diff/design.md`
//! §0.5 "wasm `convert_to` の DDL 混在バッチ挙動 — silent-drop → hard-fail 反転".
//!
//! ## Expected intermediate state (T2.1b done, T3/T4/T5 NOT done)
//!
//! Once T2.1b converts DDL to `Some(...)`, DDL statements reach the emitter
//! instead of being silently dropped by `filter_map`. The 3 emitters still
//! return `EmitError::UnsupportedStatement` for DDL (T3/T4/T5 pending), so a
//! mixed DDL+DML batch hard-fails with an **emitter-origin** message — NOT the
//! old pre-T2 `"Statement contains unsupported features for conversion"`
//! converter-drop message. After T3/T4/T5 land, DDL emits normally and these
//! R1 assertions will flip to success (documented below).
//!
//! ## R2 (pure DML non-regression)
//!
//! `SELECT 1;` (no DDL) must produce identical output before and after T2.1b —
//! the DML conversion path is untouched.

#![cfg(feature = "wasm")]

use tsql_remaker_wasm::{convert_to, TargetDialect};

// ===========================================================================
// R1: mixed DDL+DML batch — intermediate hard-fail (emitter-origin error)
// ===========================================================================

/// R1 / PostgreSQL: mixed batch hard-fails with emitter-origin message after
/// T2.1b. Pre-T2 this returned a partial success (DDL silently dropped, only
/// the SELECT emitted) — see design §0.5.
///
/// # Status (intermediate)
/// - RED pre-T2.1b: returns `"unsupported features"` (silent-drop path).
/// - GREEN post-T2.1b (this test's target): returns `Error` with emitter-origin
///   message (e.g. `"Emit error: ..."`), NOT `"unsupported features"`.
/// - Future (post-T3/T4/T5): flip to assert `Success` containing both
///   `CREATE TABLE` and `SELECT`.
#[test]
fn r1_mixed_batch_postgresql_emitter_origin_hard_fail() {
    let input = "CREATE TABLE users (id INT); SELECT * FROM users;";
    let result = convert_to(input, TargetDialect::PostgreSQL);
    let result_str = result.as_string().expect("result must be a string");

    // Must be an error result (not success): the DDL reaches the emitter and
    // the emitter returns EmitError::UnsupportedStatement (T3/T4/T5 pending).
    assert!(
        !result_str.contains(r#""status":"success""#) && !result_str.contains("Success"),
        "mixed DDL+DML batch must NOT emit a partial success after T2.1b (silent-drop is abolished): {result_str}"
    );
    assert!(
        result_str.contains("Error") || result_str.contains(r#""status":"error""#),
        "expected an error result for mixed batch: {result_str}"
    );

    // The error must be emitter-origin (NOT the old converter silent-drop
    // message). This is the core §0.5 contract assertion.
    assert!(
        !result_str.contains("unsupported features for conversion"),
        "post-T2.1b error must be emitter-origin, not the old converter silent-drop message: {result_str}"
    );
    assert!(
        result_str.contains("Emit error") || result_str.contains("emit"),
        "expected emitter-origin error message after T2.1b: {result_str}"
    );
}

/// R1 / MySQL: same intermediate hard-fail contract as PostgreSQL.
#[test]
fn r1_mixed_batch_mysql_emitter_origin_hard_fail() {
    let input = "CREATE TABLE users (id INT); SELECT * FROM users;";
    let result = convert_to(input, TargetDialect::MySQL);
    let result_str = result.as_string().expect("result must be a string");

    assert!(
        !result_str.contains(r#""status":"success""#) && !result_str.contains("Success"),
        "mixed DDL+DML batch must NOT emit a partial success after T2.1b: {result_str}"
    );
    assert!(
        !result_str.contains("unsupported features for conversion"),
        "post-T2.1b error must be emitter-origin, not the old converter silent-drop message: {result_str}"
    );
    assert!(
        result_str.contains("Emit error") || result_str.contains("emit"),
        "expected emitter-origin error message after T2.1b: {result_str}"
    );
}

/// R1 / SQLite: same intermediate hard-fail contract as PostgreSQL/MySQL.
#[test]
fn r1_mixed_batch_sqlite_emitter_origin_hard_fail() {
    let input = "CREATE TABLE users (id INT); SELECT * FROM users;";
    let result = convert_to(input, TargetDialect::SQLite);
    let result_str = result.as_string().expect("result must be a string");

    assert!(
        !result_str.contains(r#""status":"success""#) && !result_str.contains("Success"),
        "mixed DDL+DML batch must NOT emit a partial success after T2.1b: {result_str}"
    );
    assert!(
        !result_str.contains("unsupported features for conversion"),
        "post-T2.1b error must be emitter-origin, not the old converter silent-drop message: {result_str}"
    );
    assert!(
        result_str.contains("Emit error") || result_str.contains("emit"),
        "expected emitter-origin error message after T2.1b: {result_str}"
    );
}

// ===========================================================================
// R2: pure DML batch — non-regression (output unchanged before/after T2.1b)
// ===========================================================================

/// R2 / PostgreSQL: a pure DML batch (no DDL) converts successfully both
/// before and after T2.1b. The DML conversion path is untouched by the DDL
/// 橋渡し change, so the output must be a success containing the SELECT.
#[test]
fn r2_pure_dml_postgresql_non_regression() {
    let input = "SELECT 1;";
    let result = convert_to(input, TargetDialect::PostgreSQL);
    let result_str = result.as_string().expect("result must be a string");

    assert!(
        result_str.contains(r#""status":"success""#) || result_str.contains("Success"),
        "pure DML batch must convert successfully (non-regression): {result_str}"
    );
    assert!(
        result_str.contains("SELECT"),
        "pure DML output must contain the emitted SELECT: {result_str}"
    );
    // The converter silent-drop message must never appear for a pure DML batch
    // that parses and converts cleanly.
    assert!(
        !result_str.contains("unsupported features for conversion"),
        "pure DML batch must not surface the converter silent-drop message: {result_str}"
    );
}

/// R2 / MySQL: pure DML non-regression (same as PostgreSQL).
#[test]
fn r2_pure_dml_mysql_non_regression() {
    let input = "SELECT 1;";
    let result = convert_to(input, TargetDialect::MySQL);
    let result_str = result.as_string().expect("result must be a string");

    assert!(
        result_str.contains(r#""status":"success""#) || result_str.contains("Success"),
        "pure DML batch must convert successfully (non-regression): {result_str}"
    );
    assert!(
        result_str.contains("SELECT"),
        "pure DML output must contain the emitted SELECT: {result_str}"
    );
    assert!(
        !result_str.contains("unsupported features for conversion"),
        "pure DML batch must not surface the converter silent-drop message: {result_str}"
    );
}

/// R2 / SQLite: pure DML non-regression (same as PostgreSQL/MySQL).
#[test]
fn r2_pure_dml_sqlite_non_regression() {
    let input = "SELECT 1;";
    let result = convert_to(input, TargetDialect::SQLite);
    let result_str = result.as_string().expect("result must be a string");

    assert!(
        result_str.contains(r#""status":"success""#) || result_str.contains("Success"),
        "pure DML batch must convert successfully (non-regression): {result_str}"
    );
    assert!(
        result_str.contains("SELECT"),
        "pure DML output must contain the emitted SELECT: {result_str}"
    );
    assert!(
        !result_str.contains("unsupported features for conversion"),
        "pure DML batch must not surface the converter silent-drop message: {result_str}"
    );
}
