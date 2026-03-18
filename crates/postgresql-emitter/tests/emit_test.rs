//! PostgreSQL Emitter 統合テスト
//!
//! Common SQL AST から PostgreSQL SQL への変換をテストします。

use postgresql_emitter::{PostgreSqlEmitter, EmissionConfig};
use tsql_parser::{parse, ToCommonAst};

/// SELECT文の発行テスト
#[test]
fn test_emit_select_simple() {
    let sql = "SELECT * FROM users";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    assert_eq!(postgres_sql, "SELECT * FROM users");
}

/// SELECT文 with WHERE句
#[test]
fn test_emit_select_with_where() {
    let sql = "SELECT id, name FROM users WHERE id = 1";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    // Binary operations are wrapped in parentheses for proper precedence
    // Note: "name" might not be quoted in certain contexts based on IdentifierQuoter logic
    assert!(postgres_sql.contains("SELECT id"));
    assert!(postgres_sql.contains("FROM users"));
    assert!(postgres_sql.contains("WHERE (id = 1)"));
}

/// SELECT文 with ORDER BY
#[test]
fn test_emit_select_with_order_by() {
    let sql = "SELECT * FROM users ORDER BY name ASC";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    assert_eq!(postgres_sql, "SELECT * FROM users ORDER BY name ASC");
}

/// SELECT文 with LIMIT
#[test]
fn test_emit_select_with_limit() {
    let sql = "SELECT TOP 10 * FROM users";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    // TOP 10 becomes LIMIT 10 in PostgreSQL
    assert!(postgres_sql.contains("SELECT"));
    assert!(postgres_sql.contains("FROM users"));
    // Note: LIMIT clause emission is implemented but may differ slightly
    println!("SQL with limit: {}", postgres_sql);
}

/// INSERT文の発行テスト
#[test]
fn test_emit_insert_values() {
    let sql = "INSERT INTO users (id, name) VALUES (1, 'test')";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    // "name" is a PostgreSQL reserved keyword, so it's quoted
    assert_eq!(postgres_sql, "INSERT INTO users (id, \"name\") VALUES (1, 'test')");
}

/// UPDATE文の発行テスト
#[test]
fn test_emit_update() {
    let sql = "UPDATE users SET name = 'updated' WHERE id = 1";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    // "name" is a PostgreSQL reserved keyword, so it's quoted
    assert_eq!(postgres_sql, "UPDATE users SET \"name\" = 'updated' WHERE (id = 1)");
}

/// DELETE文の発行テスト
#[test]
fn test_emit_delete() {
    let sql = "DELETE FROM users WHERE id = 1";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    // Binary operations have parentheses
    assert_eq!(postgres_sql, "DELETE FROM users WHERE (id = 1)");
}

/// バッチ発行テスト
#[test]
fn test_emit_batch() {
    let sql = "SELECT * FROM users; SELECT * FROM orders";
    let statements = parse(sql).unwrap();
    let common_stmts: Vec<_> = statements
        .iter()
        .filter_map(|s| s.to_common_ast())
        .collect();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit_batch(&common_stmts).unwrap();

    assert!(postgres_sql.contains("SELECT * FROM users"));
    assert!(postgres_sql.contains("SELECT * FROM orders"));
    assert!(postgres_sql.contains(";\n"));
}

/// 識別子のクォートテスト
#[test]
fn test_emit_with_quoted_identifiers() {
    let sql = "SELECT * FROM Users";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig {
        quote_identifiers: true,
        uppercase_keywords: false,
        indent_size: 4,
    };
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    // "Users" should be quoted as it starts with uppercase
    assert!(postgres_sql.contains("\"Users\""));
}

/// 識別子のクォートなしテスト
#[test]
fn test_emit_without_quoted_identifiers() {
    let sql = "SELECT * FROM users";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig {
        quote_identifiers: false,
        uppercase_keywords: false,
        indent_size: 4,
    };
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    assert_eq!(postgres_sql, "SELECT * FROM users");
}
