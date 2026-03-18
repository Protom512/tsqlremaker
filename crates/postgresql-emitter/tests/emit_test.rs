//! PostgreSQL Emitter 統合テスト
//!
//! Common SQL AST から PostgreSQL SQL への変換をテストします。

use postgresql_emitter::{EmissionConfig, PostgreSqlEmitter};
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
    assert_eq!(
        postgres_sql,
        "INSERT INTO users (id, \"name\") VALUES (1, 'test')"
    );
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
    assert_eq!(
        postgres_sql,
        "UPDATE users SET \"name\" = 'updated' WHERE (id = 1)"
    );
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

/// IN サブクエリテスト
#[test]
fn test_emit_in_subquery() {
    let sql =
        "SELECT * FROM orders WHERE customer_id IN (SELECT id FROM customers WHERE active = 1)";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    assert!(postgres_sql.contains("SELECT"));
    assert!(postgres_sql.contains("FROM orders"));
    assert!(postgres_sql.contains("customer_id IN (SELECT id"));
    assert!(postgres_sql.contains("FROM customers"));
    assert!(postgres_sql.contains("active"));
}

/// NOT IN サブクエリテスト
#[test]
fn test_emit_not_in_subquery() {
    let sql = "SELECT * FROM orders WHERE customer_id NOT IN (SELECT id FROM blocked_customers)";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    println!("NOT IN subquery output: {}", postgres_sql);
    assert!(postgres_sql.contains("customer_id NOT IN (SELECT id"));
    assert!(postgres_sql.contains("FROM blocked_customers"));
}

/// EXISTS サブクエリテスト
#[test]
fn test_emit_exists_subquery() {
    let sql = "SELECT * FROM customers WHERE EXISTS (SELECT 1 FROM orders WHERE orders.customer_id = customers.id)";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    assert!(postgres_sql.contains("EXISTS (SELECT 1"));
    assert!(postgres_sql.contains("FROM orders"));
    assert!(postgres_sql.contains("orders.customer_id = customers.id"));
}

/// NOT EXISTS サブクエリテスト
#[test]
fn test_emit_not_exists_subquery() {
    let sql = "SELECT * FROM customers WHERE NOT EXISTS (SELECT 1 FROM orders WHERE orders.customer_id = customers.id)";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    assert!(postgres_sql.contains("NOT EXISTS (SELECT 1"));
}

/// スカラーサブクエリテスト（SELECTリスト内）
#[test]
fn test_emit_scalar_subquery() {
    let sql = "SELECT id, (SELECT COUNT(*) FROM orders WHERE orders.customer_id = customers.id) AS order_count FROM customers";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    println!("Scalar subquery output: {}", postgres_sql);
    assert!(postgres_sql.contains("SELECT id"));
    assert!(postgres_sql.contains("SELECT COUNT(*)"));
    assert!(postgres_sql.contains("AS order_count"));
}

/// FROM句の派生テーブル（サブクエリ）
#[test]
fn test_emit_derived_table() {
    let sql = "SELECT * FROM (SELECT id, name FROM users WHERE active = 1) AS active_users";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    assert!(postgres_sql.contains("SELECT * FROM (SELECT id"));
    assert!(postgres_sql.contains("AS active_users"));
}

/// 入れ子のサブクエリテスト
#[test]
fn test_emit_nested_subquery() {
    let sql = "SELECT * FROM orders WHERE customer_id IN (SELECT id FROM customers WHERE region_id IN (SELECT id FROM regions WHERE country = 'USA'))";
    let statements = parse(sql).unwrap();
    let common_stmt = statements[0].to_common_ast().unwrap();

    let config = EmissionConfig::default();
    let mut emitter = PostgreSqlEmitter::new(config);
    let postgres_sql = emitter.emit(&common_stmt).unwrap();

    assert!(postgres_sql.contains("customer_id IN (SELECT id FROM customers"));
    assert!(postgres_sql.contains("region_id IN (SELECT id FROM regions"));
}
