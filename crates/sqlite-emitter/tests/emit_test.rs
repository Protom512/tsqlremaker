//! SQLite Emitter integration tests
//!
//! Common SQL AST → SQLite SQL 変換のテスト。

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use sqlite_emitter::{EmitterConfig, SqliteEmitter};
use tsql_parser::{parse, ToCommonAst};

/// SELECT * FROM の基本的な発行テスト
#[test]
fn test_emit_select_star() {
    let sql = "SELECT * FROM users";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmitterConfig::default();
    let mut emitter = SqliteEmitter::new(config);
    let sqlite_sql = emitter.emit(&common_stmt).unwrap();

    assert!(sqlite_sql.contains("SELECT *"));
    assert!(sqlite_sql.contains("users"));
}

/// WHERE句付きSELECTの発行テスト
#[test]
fn test_emit_select_with_where() {
    let sql = "SELECT id, name FROM users WHERE id = 1";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmitterConfig::default();
    let mut emitter = SqliteEmitter::new(config);
    let sqlite_sql = emitter.emit(&common_stmt).unwrap();

    assert!(sqlite_sql.contains("SELECT"));
    assert!(sqlite_sql.contains("FROM"));
    assert!(sqlite_sql.contains("WHERE"));
}

/// UPDATE文の発行テスト
#[test]
fn test_emit_update() {
    let sql = "UPDATE users SET name = 'Bob' WHERE id = 1";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmitterConfig::default();
    let mut emitter = SqliteEmitter::new(config);
    let sqlite_sql = emitter.emit(&common_stmt).unwrap();

    assert!(sqlite_sql.contains("UPDATE"));
    assert!(sqlite_sql.contains("SET"));
    assert!(sqlite_sql.contains("WHERE"));
}

/// emit_batch で複数ステートメントを発行
#[test]
fn test_emit_batch() {
    let sql = "SELECT * FROM t1\nSELECT * FROM t2";
    let statements = parse(sql).unwrap();
    let common_stmts: Vec<_> = statements
        .iter()
        .filter_map(|s| s.to_common_ast())
        .collect();

    assert!(
        common_stmts.len() >= 2,
        "Should parse at least 2 statements"
    );

    let config = EmitterConfig::default();
    let mut emitter = SqliteEmitter::new(config);
    let sqlite_sql = emitter.emit_batch(&common_stmts).unwrap();

    assert!(
        sqlite_sql.contains(";\n"),
        "Batch should be semicolon-separated"
    );
}

/// ORDER BY付きSELECTの発行テスト
#[test]
fn test_emit_select_order_by() {
    let sql = "SELECT * FROM users ORDER BY name ASC";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmitterConfig::default();
    let mut emitter = SqliteEmitter::new(config);
    let sqlite_sql = emitter.emit(&common_stmt).unwrap();

    assert!(sqlite_sql.contains("ORDER BY"));
    assert!(sqlite_sql.contains("ASC"));
}

/// SELECT DISTINCTの発行テスト
#[test]
fn test_emit_select_distinct() {
    let sql = "SELECT DISTINCT id FROM users";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmitterConfig::default();
    let mut emitter = SqliteEmitter::new(config);
    let sqlite_sql = emitter.emit(&common_stmt).unwrap();

    assert!(sqlite_sql.contains("DISTINCT"));
}
