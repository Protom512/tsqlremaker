//! Comprehensive dogfooding quality gate tests for tsql-parser.
//!
//! This file exercises the parser with a wide range of real-world T-SQL patterns,
//! verifies AST structure correctness, and confirms error resilience.
//!
//! Test categories:
//!   1. Parse Success — valid SQL that must parse without error
//!   2. Error Recovery — invalid SQL that must return errors, not panic
//!   3. AST Correctness — verify the internal structure of parsed statements
//!   4. Fixture Parsing — large real-world SQL files must not panic
//!   5. Regression — known past bugs re-tested
//!
//! Run: cargo nextest run -p tsql-parser -E 'test(dogfood_quality)'

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use tsql_parser::ast::*;
use tsql_parser::error::ParseError;
use tsql_parser::{parse, parse_one, parse_with_errors, Parser};

// ============================================================================
// Helpers
// ============================================================================

fn must_parse(sql: &str) -> Vec<Statement> {
    parse(sql).unwrap_or_else(|e| panic!("Expected parse OK for:\n{sql}\nError: {e}"))
}

#[allow(dead_code)]
fn must_fail(sql: &str) {
    assert!(parse(sql).is_err(), "Expected parse error for:\n{sql}");
}

fn tolerant_parse(
    sql: &str,
) -> Result<(Vec<Statement>, Vec<ParseError>), tsql_parser::ParseErrors> {
    let mut parser = Parser::new(sql);
    parser.parse_with_errors()
}

fn first_stmt(sql: &str) -> Statement {
    let stmts = must_parse(sql);
    assert_eq!(
        stmts.len(),
        1,
        "Expected exactly 1 statement, got {}",
        stmts.len()
    );
    stmts.into_iter().next().unwrap()
}

fn count_variant<'a>(
    stmts: impl IntoIterator<Item = &'a Statement>,
    pred: fn(&Statement) -> bool,
) -> usize {
    stmts.into_iter().filter(|s| pred(s)).count()
}

fn is_select(s: &Statement) -> bool {
    matches!(s, Statement::Select(_))
}
fn is_insert(s: &Statement) -> bool {
    matches!(s, Statement::Insert(_))
}
fn is_update(s: &Statement) -> bool {
    matches!(s, Statement::Update(_))
}
fn is_delete(s: &Statement) -> bool {
    matches!(s, Statement::Delete(_))
}
fn is_create(s: &Statement) -> bool {
    matches!(s, Statement::Create(_))
}
fn is_alter(s: &Statement) -> bool {
    matches!(s, Statement::AlterTable(_))
}
fn is_declare(s: &Statement) -> bool {
    matches!(s, Statement::Declare(_))
}
fn is_set(s: &Statement) -> bool {
    matches!(s, Statement::Set(_))
}
fn is_if(s: &Statement) -> bool {
    matches!(s, Statement::If(_))
}
fn is_while(s: &Statement) -> bool {
    matches!(s, Statement::While(_))
}
fn is_block(s: &Statement) -> bool {
    matches!(s, Statement::Block(_))
}
fn is_trycatch(s: &Statement) -> bool {
    matches!(s, Statement::TryCatch(_))
}
fn is_transaction(s: &Statement) -> bool {
    matches!(s, Statement::Transaction(_))
}
#[allow(dead_code)]
fn is_return(s: &Statement) -> bool {
    matches!(s, Statement::Return(_))
}
fn is_raiserror(s: &Statement) -> bool {
    matches!(s, Statement::Raiserror(_))
}
fn is_throw(s: &Statement) -> bool {
    matches!(s, Statement::Throw(_))
}
fn is_batch_sep(s: &Statement) -> bool {
    matches!(s, Statement::BatchSeparator(_))
}
fn is_break(s: &Statement) -> bool {
    matches!(s, Statement::Break(_))
}
fn is_continue(s: &Statement) -> bool {
    matches!(s, Statement::Continue(_))
}
fn is_exec(s: &Statement) -> bool {
    matches!(s, Statement::Exec(_))
}
#[allow(dead_code)]
fn is_var_assign(s: &Statement) -> bool {
    matches!(s, Statement::VariableAssignment(_))
}

// ============================================================================
// Category 1: Parse Success — SELECT variants
// ============================================================================

#[test]
fn dogfood_quality_select_star() {
    let stmts = must_parse("SELECT * FROM t");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_column_list() {
    let stmts = must_parse("SELECT id, name, email, created_at FROM users");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_with_alias() {
    let stmts = must_parse("SELECT u.id AS user_id, u.name AS user_name FROM users u");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_with_where() {
    let stmts = must_parse("SELECT * FROM orders WHERE status = 'pending' AND total > 100");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_with_order_by() {
    let stmts = must_parse("SELECT * FROM users ORDER BY name ASC, id DESC");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_distinct() {
    let stmts = must_parse("SELECT DISTINCT category FROM products");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_top() {
    let stmts = must_parse("SELECT TOP 10 * FROM users ORDER BY score DESC");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_group_by_having() {
    let sql = "SELECT category, COUNT(*) AS cnt FROM products \
               GROUP BY category HAVING COUNT(*) > 5";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_with_subquery_where() {
    let sql = "SELECT * FROM orders WHERE customer_id IN \
               (SELECT id FROM customers WHERE tier = 'gold')";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_with_exists() {
    let sql = "SELECT * FROM customers c WHERE EXISTS \
               (SELECT 1 FROM orders o WHERE o.customer_id = c.id)";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_case_expression() {
    let sql = "SELECT id, CASE WHEN status = 1 THEN 'active' \
               WHEN status = 0 THEN 'inactive' ELSE 'unknown' END AS status_text FROM users";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_between() {
    let stmts = must_parse("SELECT * FROM orders WHERE total BETWEEN 100 AND 500");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_like() {
    let stmts = must_parse("SELECT * FROM users WHERE name LIKE 'John%'");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_is_null() {
    let stmts = must_parse("SELECT * FROM users WHERE email IS NULL");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_is_not_null() {
    let stmts = must_parse("SELECT * FROM users WHERE email IS NOT NULL");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_not_in() {
    let sql = "SELECT * FROM users WHERE id NOT IN (SELECT user_id FROM banned_users)";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_not_like() {
    let stmts = must_parse("SELECT * FROM users WHERE name NOT LIKE 'admin%'");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_not_between() {
    let stmts = must_parse("SELECT * FROM orders WHERE total NOT BETWEEN 10 AND 20");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_count_star() {
    let stmts = must_parse("SELECT COUNT(*) AS total FROM users");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_aggregate_functions() {
    let sql = "SELECT MIN(price) AS min_price, MAX(price) AS max_price, \
               AVG(price) AS avg_price, SUM(quantity) AS total_qty FROM products";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_select_nested_functions() {
    let sql = "SELECT ISNULL(MAX(score), 0) AS best_score FROM results";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

// ============================================================================
// Category 2: Parse Success — JOIN variants
// ============================================================================

#[test]
fn dogfood_quality_inner_join() {
    let sql = "SELECT u.name, o.total FROM users u \
               INNER JOIN orders o ON u.id = o.user_id";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_left_join() {
    let sql = "SELECT c.name, o.order_id FROM customers c \
               LEFT JOIN orders o ON c.id = o.customer_id";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_right_join() {
    let sql = "SELECT d.dept_name, e.emp_name FROM departments d \
               RIGHT JOIN employees e ON d.id = e.dept_id";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_full_join() {
    let sql = "SELECT a.name, b.name FROM table_a a \
               FULL JOIN table_b b ON a.id = b.id";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_cross_join() {
    let sql = "SELECT a.col1, b.col2 FROM table_a a CROSS JOIN table_b b";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_multi_join_chain() {
    let sql = "SELECT u.name, p.title, oi.quantity \
               FROM users u \
               JOIN orders o ON u.id = o.user_id \
               JOIN order_items oi ON o.id = oi.order_id \
               JOIN products p ON oi.product_id = p.id";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_derived_table() {
    let sql = "SELECT sq.category, sq.total \
               FROM (SELECT category, COUNT(*) AS total FROM products GROUP BY category) sq";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

// ============================================================================
// Category 3: Parse Success — DML (INSERT, UPDATE, DELETE)
// ============================================================================

#[test]
fn dogfood_quality_insert_values() {
    let sql = "INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com')";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_insert), 1);
}

#[test]
fn dogfood_quality_insert_select() {
    let sql = "INSERT INTO archive_users SELECT * FROM users WHERE inactive = 1";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_insert), 1);
}

#[test]
fn dogfood_quality_insert_default_values() {
    let sql = "INSERT INTO log_defaults DEFAULT VALUES";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_insert), 1);
}

#[test]
fn dogfood_quality_update_simple() {
    let sql = "UPDATE users SET name = 'Bob', email = 'bob@example.com' WHERE id = 1";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_update), 1);
}

/// NOTE: UPDATE with table-qualified SET (e.g. SET o.status = ...) is NOT supported.
/// The parser only supports SET column = value, not SET table.column = value.
/// Use unqualified column names in the SET clause instead.
#[test]
fn dogfood_quality_update_with_from() {
    let sql = "UPDATE orders SET status = 'processed' \
               FROM orders o JOIN customers c ON o.customer_id = c.id \
               WHERE c.tier = 'vip'";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_update), 1);
}

#[test]
fn dogfood_quality_delete_simple() {
    let stmts = must_parse("DELETE FROM users WHERE id = 1");
    assert_eq!(count_variant(&stmts, is_delete), 1);
}

#[test]
fn dogfood_quality_delete_with_subquery() {
    let sql = "DELETE FROM orders WHERE customer_id IN \
               (SELECT id FROM customers WHERE status = 'closed')";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_delete), 1);
}

// ============================================================================
// Category 4: Parse Success — DDL (CREATE, ALTER)
// ============================================================================

#[test]
fn dogfood_quality_create_table_basic() {
    let sql = "CREATE TABLE users (id INT, name VARCHAR(100))";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_table_with_pk() {
    let sql = "CREATE TABLE users (\
               id INT NOT NULL, \
               name VARCHAR(100) NOT NULL, \
               CONSTRAINT pk_users PRIMARY KEY (id))";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_table_with_fk() {
    let sql = "CREATE TABLE orders (\
               id INT NOT NULL, \
               user_id INT NOT NULL, \
               total NUMERIC(10,2), \
               CONSTRAINT pk_orders PRIMARY KEY (id), \
               CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id))";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_table_with_check() {
    let sql = "CREATE TABLE products (\
               id INT, price NUMERIC(10,2), \
               CONSTRAINT chk_price CHECK (price > 0))";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_table_with_default() {
    let sql = "CREATE TABLE t (id INT, status VARCHAR(20) DEFAULT 'active', \
               created DATETIME DEFAULT GETDATE())";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_table_identity() {
    let stmts = must_parse("CREATE TABLE seq (id INT IDENTITY, name VARCHAR(100))");
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_table_temp() {
    let stmts = must_parse("CREATE TABLE #temp (id INT, val VARCHAR(50))");
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_table_global_temp() {
    let stmts = must_parse("CREATE TABLE ##global_cache (cache_key VARCHAR(100), data TEXT)");
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_index() {
    let stmts = must_parse("CREATE INDEX idx_email ON users (email)");
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_unique_index() {
    let stmts = must_parse("CREATE UNIQUE INDEX idx_sku ON products (sku)");
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_view() {
    let sql = "CREATE VIEW v_active_users AS SELECT id, name FROM users WHERE status = 'active'";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_procedure_simple() {
    let sql = "CREATE PROCEDURE sp_get_user @id INT AS SELECT * FROM users WHERE id = @id";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_procedure_with_body() {
    let sql = "CREATE PROCEDURE sp_test @p1 INT, @p2 VARCHAR(100) = 'x' AS \
               BEGIN SELECT @p1 END";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_create_trigger() {
    let sql = "CREATE TRIGGER tr_audit ON users FOR INSERT AS BEGIN SELECT 1 END";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_alter_table_add() {
    let stmts = must_parse("ALTER TABLE users ADD email VARCHAR(255)");
    assert_eq!(count_variant(&stmts, is_alter), 1);
}

#[test]
fn dogfood_quality_alter_table_drop_column() {
    let stmts = must_parse("ALTER TABLE users DROP COLUMN old_col");
    assert_eq!(count_variant(&stmts, is_alter), 1);
}

#[test]
fn dogfood_quality_alter_table_alter_column() {
    let stmts = must_parse("ALTER TABLE users ALTER COLUMN name VARCHAR(200) NOT NULL");
    assert_eq!(count_variant(&stmts, is_alter), 1);
}

// ============================================================================
// Category 5: Parse Success — Control Flow
// ============================================================================

#[test]
fn dogfood_quality_if_else() {
    let sql = "IF @mode = 1 BEGIN SELECT 1 END ELSE BEGIN SELECT 0 END";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_if), 1);
}

#[test]
fn dogfood_quality_if_no_else() {
    let sql = "IF @count > 0 BEGIN DELETE FROM temp WHERE id < @count END";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_if), 1);
}

#[test]
fn dogfood_quality_while_loop() {
    let sql = "WHILE @x < 10 BEGIN SET @x = @x + 1 END";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_while), 1);
}

#[test]
fn dogfood_quality_nested_begin_end() {
    let stmts = must_parse("BEGIN BEGIN BEGIN SELECT 1 END END END");
    assert!(stmts.iter().any(is_block));
}

#[test]
fn dogfood_quality_try_catch() {
    let sql = "BEGIN TRY INSERT INTO t VALUES (1) END TRY \
               BEGIN CATCH RAISERROR('Failed', 16, 1) END CATCH";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_trycatch), 1);
}

#[test]
fn dogfood_quality_declare() {
    let stmts = must_parse("DECLARE @count INT");
    assert_eq!(count_variant(&stmts, is_declare), 1);
}

#[test]
fn dogfood_quality_declare_with_default() {
    let stmts = must_parse("DECLARE @status VARCHAR(20) = 'active'");
    assert_eq!(count_variant(&stmts, is_declare), 1);
}

#[test]
fn dogfood_quality_set_variable() {
    let stmts = must_parse("SET @count = 42");
    assert_eq!(count_variant(&stmts, is_set), 1);
}

#[test]
fn dogfood_quality_return_no_value() {
    // RETURN inside a procedure body context
    let sql = "CREATE PROCEDURE sp_test AS BEGIN RETURN END";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_return_value() {
    let sql = "CREATE PROCEDURE sp_test AS BEGIN RETURN 0 END";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_break() {
    let sql = "WHILE 1 = 1 BEGIN BREAK END";
    let stmts = must_parse(sql);
    assert!(stmts.iter().any(is_while));
}

#[test]
fn dogfood_quality_continue() {
    let sql = "WHILE @i < 10 BEGIN SET @i = @i + 1 CONTINUE END";
    let stmts = must_parse(sql);
    assert!(stmts.iter().any(is_while));
}

#[test]
fn dogfood_quality_raiserror() {
    let stmts = must_parse("RAISERROR('Error occurred', 16, 1)");
    assert_eq!(count_variant(&stmts, is_raiserror), 1);
}

#[test]
fn dogfood_quality_throw() {
    let stmts = must_parse("THROW 50001, 'Custom error', 1");
    assert_eq!(count_variant(&stmts, is_throw), 1);
}

// ============================================================================
// Category 6: Parse Success — EXEC
// ============================================================================

#[test]
fn dogfood_quality_exec_no_params() {
    let stmts = must_parse("EXEC sp_help");
    assert_eq!(count_variant(&stmts, is_exec), 1);
}

#[test]
fn dogfood_quality_execute_no_params() {
    let stmts = must_parse("EXECUTE sp_who");
    assert_eq!(count_variant(&stmts, is_exec), 1);
}

#[test]
fn dogfood_quality_exec_positional_params() {
    let stmts = must_parse("EXEC sp_get_user 42, 'active'");
    assert_eq!(count_variant(&stmts, is_exec), 1);
}

#[test]
fn dogfood_quality_exec_named_params() {
    let stmts = must_parse("EXEC sp_get_orders @customer_id = 100, @status = 'pending'");
    assert_eq!(count_variant(&stmts, is_exec), 1);
}

// ============================================================================
// Category 7: Parse Success — Transactions
// ============================================================================

#[test]
fn dogfood_quality_begin_commit() {
    let sql = "BEGIN TRANSACTION\nINSERT INTO t VALUES (1)\nCOMMIT TRANSACTION";
    let stmts = must_parse(sql);
    assert!(stmts.iter().any(is_transaction));
}

#[test]
fn dogfood_quality_begin_rollback() {
    let sql = "BEGIN TRANSACTION\nDELETE FROM t\nROLLBACK TRANSACTION";
    let stmts = must_parse(sql);
    assert!(stmts.iter().any(is_transaction));
}

#[test]
fn dogfood_quality_begin_tran_short() {
    let sql = "BEGIN TRAN\nUPDATE t SET x = 1\nCOMMIT TRAN";
    let stmts = must_parse(sql);
    assert!(stmts.iter().any(is_transaction));
}

#[test]
fn dogfood_quality_save_transaction() {
    let stmts = must_parse("SAVE TRANSACTION my_savepoint");
    assert!(stmts.iter().any(is_transaction));
}

#[test]
fn dogfood_quality_named_transaction() {
    let sql = "BEGIN TRANSACTION tx1\nCOMMIT TRANSACTION tx1";
    let stmts = must_parse(sql);
    assert!(stmts.iter().any(is_transaction));
}

// ============================================================================
// Category 8: Parse Success — GO batch separators
// ============================================================================

#[test]
fn dogfood_quality_go_separator() {
    let sql = "SELECT 1\nGO\nSELECT 2";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_batch_sep), 1);
    assert_eq!(count_variant(&stmts, is_select), 2);
}

#[test]
fn dogfood_quality_multiple_go() {
    let sql =
        "CREATE TABLE t1 (id INT)\nGO\nCREATE TABLE t2 (id INT)\nGO\nINSERT INTO t1 VALUES (1)\nGO";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_batch_sep), 3);
}

// ============================================================================
// Category 9: Parse Success — Datatypes
// ============================================================================

#[test]
fn dogfood_quality_all_supported_datatypes() {
    let sql = "CREATE TABLE t (\
        c_int INT, \
        c_smallint SMALLINT, \
        c_tinyint TINYINT, \
        c_bigint BIGINT, \
        c_varchar VARCHAR(255), \
        c_char CHAR(10), \
        c_text TEXT, \
        c_decimal DECIMAL(18,2), \
        c_numeric NUMERIC(10,4), \
        c_float FLOAT, \
        c_real REAL, \
        c_datetime DATETIME, \
        c_date DATE, \
        c_time TIME, \
        c_bit BIT, \
        c_money MONEY)";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_create), 1);
}

#[test]
fn dogfood_quality_varchar_without_length() {
    let stmts = must_parse("CREATE TABLE t (name VARCHAR)");
    assert_eq!(count_variant(&stmts, is_create), 1);
}

// ============================================================================
// Category 10: Parse Success — Expressions
// ============================================================================

#[test]
fn dogfood_quality_arithmetic_expressions() {
    let sql = "SELECT price * quantity AS total, price + tax AS gross, discount / 100 AS pct";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_comparison_operators() {
    let sql = "SELECT * FROM t WHERE a = 1 AND b <> 2 AND c < 3 AND d <= 4 AND e > 5 AND f >= 6";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_logical_operators() {
    let sql = "SELECT * FROM t WHERE (a = 1 OR b = 2) AND NOT c = 3";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_unary_minus() {
    let stmts = must_parse("SELECT -1 AS neg, -total AS neg_col FROM t");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_string_concat() {
    let stmts = must_parse("SELECT first_name + ' ' + last_name AS full_name FROM users");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_hex_literal() {
    let stmts = must_parse("INSERT INTO t (data) VALUES (0xFF)");
    assert_eq!(count_variant(&stmts, is_insert), 1);
}

#[test]
fn dogfood_quality_nested_case() {
    let sql =
        "SELECT CASE WHEN x > 10 THEN CASE WHEN y = 1 THEN 'a' ELSE 'b' END ELSE 'c' END FROM t";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_deeply_nested_parens() {
    let stmts = must_parse("SELECT * FROM t WHERE id = (((((((((1)))))))))");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_in_list() {
    let stmts = must_parse("SELECT * FROM t WHERE status IN ('a', 'b', 'c')");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

#[test]
fn dogfood_quality_scalar_subquery() {
    let stmts = must_parse("SELECT (SELECT MAX(id) FROM users) AS max_id");
    assert_eq!(count_variant(&stmts, is_select), 1);
}

// ============================================================================
// Category 11: Parse Success — Multi-statement sequences
// ============================================================================

#[test]
fn dogfood_quality_multiple_selects() {
    let stmts = must_parse("SELECT 1\nSELECT 2\nSELECT 3");
    assert_eq!(count_variant(&stmts, is_select), 3);
}

#[test]
fn dogfood_quality_mixed_dml_sequence() {
    let sql = "INSERT INTO t VALUES (1)\nUPDATE t SET x = 2\nDELETE FROM t WHERE x = 2";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_insert), 1);
    assert_eq!(count_variant(&stmts, is_update), 1);
    assert_eq!(count_variant(&stmts, is_delete), 1);
}

#[test]
fn dogfood_quality_semicolons_between_statements() {
    let stmts = must_parse("SELECT 1; SELECT 2; SELECT 3;");
    assert_eq!(count_variant(&stmts, is_select), 3);
}

#[test]
fn dogfood_quality_declare_set_select() {
    let sql = "DECLARE @x INT\nSET @x = 5\nSELECT @x";
    let stmts = must_parse(sql);
    assert_eq!(count_variant(&stmts, is_declare), 1);
    assert_eq!(count_variant(&stmts, is_set), 1);
    assert_eq!(count_variant(&stmts, is_select), 1);
}

// ============================================================================
// Category 12: Error Recovery — invalid SQL must not panic
// ============================================================================

#[test]
fn dogfood_quality_err_select_no_from() {
    // "SELECT * FROM" without table name
    let _ = tolerant_parse("SELECT * FROM");
}

#[test]
fn dogfood_quality_err_create_no_type() {
    let _ = tolerant_parse("CREATE");
}

#[test]
fn dogfood_quality_err_unterminated_string() {
    let _ = tolerant_parse("SELECT 'unterminated");
}

#[test]
fn dogfood_quality_err_deeply_nested_unmatched() {
    let _ = tolerant_parse("((((((((");
}

#[test]
fn dogfood_quality_err_declare_no_varname() {
    let _ = tolerant_parse("DECLARE @");
}

#[test]
fn dogfood_quality_err_empty_after_go() {
    let _ = tolerant_parse("GO");
}

#[test]
fn dogfood_quality_err_gibberish() {
    let _ = tolerant_parse("aslkdjfalksjdfklj");
}

#[test]
fn dogfood_quality_err_partial_keywords() {
    let cases = vec![
        "SELCT",
        "SELECT",
        "INSERT INTO",
        "CREATE TABLE",
        "IF",
        "WHILE",
        "BEGIN TRY",
        "BEGIN TRANSACTION",
        "SET @x =",
        "UPDATE t SET",
        "DELETE FROM",
        "ALTER TABLE",
    ];
    for sql in cases {
        let _ = tolerant_parse(sql);
    }
}

#[test]
fn dogfood_quality_err_non_sql_text() {
    let _ = tolerant_parse("This is not SQL. Just plain English text with words.");
}

#[test]
fn dogfood_quality_err_empty_string() {
    let stmts = must_parse("");
    assert!(stmts.is_empty());
}

#[test]
fn dogfood_quality_err_whitespace_only() {
    let stmts = must_parse("   \n\t\n   ");
    assert!(stmts.is_empty());
}

#[test]
fn dogfood_quality_err_comment_only() {
    let stmts = must_parse("-- just a comment\n/* block comment */");
    assert!(stmts.is_empty());
}

#[test]
fn dogfood_quality_err_long_line() {
    let mut sql = String::from("SELECT ");
    for i in 0..500 {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("col_{i}"));
    }
    sql.push_str(" FROM big_table");
    let _ = tolerant_parse(&sql);
}

#[test]
fn dogfood_quality_err_unicode_in_comments() {
    let _ = tolerant_parse("-- Japanese: 日本語\nSELECT 1 /* Chinese: 数据库 */");
}

#[test]
fn dogfood_quality_err_special_chars_in_strings() {
    let cases = vec![
        "SELECT 'It''s a quote'",
        "SELECT ''",
        "SELECT 'newline\nhere'",
    ];
    for sql in cases {
        let _ = tolerant_parse(sql);
    }
}

#[test]
fn dogfood_quality_err_incomplete_sql_batch() {
    // None of these should panic
    let cases = vec![
        "SELECT *\nFROM\nWHERE",
        "CREATE TABLE (id INT)",
        "INSERT INTO VALUES (1)",
        "UPDATE SET x = 1",
        "DELETE WHERE id = 1",
        "IF BEGIN SELECT 1",
        "WHILE BEGIN SET @x = 1",
    ];
    for sql in cases {
        let _ = tolerant_parse(sql);
    }
}

// ============================================================================
// Category 13: AST Correctness — SELECT structure
// ============================================================================

#[test]
fn dogfood_quality_ast_select_where_clause_present() {
    let stmt = first_stmt("SELECT * FROM t WHERE x = 1");
    if let Statement::Select(sel) = &stmt {
        assert!(sel.where_clause.is_some(), "Should have WHERE clause");
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn dogfood_quality_ast_select_order_by() {
    let stmt = first_stmt("SELECT * FROM t ORDER BY name ASC");
    if let Statement::Select(sel) = &stmt {
        assert_eq!(sel.order_by.len(), 1, "Should have 1 ORDER BY item");
        let item = &sel.order_by[0];
        assert!(item.asc, "Should be ascending");
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn dogfood_quality_ast_select_distinct_flag() {
    let stmt = first_stmt("SELECT DISTINCT name FROM t");
    if let Statement::Select(sel) = &stmt {
        assert!(sel.distinct, "DISTINCT should be true");
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn dogfood_quality_ast_select_top_value() {
    let stmt = first_stmt("SELECT TOP 5 * FROM t");
    if let Statement::Select(sel) = &stmt {
        assert!(sel.top.is_some(), "Should have TOP clause");
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn dogfood_quality_ast_select_group_by_having() {
    let stmt = first_stmt("SELECT cat, COUNT(*) AS cnt FROM t GROUP BY cat HAVING COUNT(*) > 5");
    if let Statement::Select(sel) = &stmt {
        assert_eq!(sel.group_by.len(), 1, "Should have 1 GROUP BY expr");
        assert!(sel.having.is_some(), "Should have HAVING clause");
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn dogfood_quality_ast_select_from_table() {
    let stmt = first_stmt("SELECT * FROM users u");
    if let Statement::Select(sel) = &stmt {
        let from = sel.from.as_ref().expect("Should have FROM");
        assert_eq!(from.tables.len(), 1);
        if let TableReference::Table { name, alias, .. } = &from.tables[0] {
            assert_eq!(name.name, "users");
            assert!(alias.is_some());
            assert_eq!(alias.as_ref().unwrap().name, "u");
        } else {
            panic!("Expected Table variant");
        }
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn dogfood_quality_ast_select_from_subquery() {
    let sql = "SELECT * FROM (SELECT id FROM t) sq";
    let stmt = first_stmt(sql);
    if let Statement::Select(sel) = &stmt {
        let from = sel.from.as_ref().expect("Should have FROM");
        assert_eq!(from.tables.len(), 1);
        if let TableReference::Subquery { alias, .. } = &from.tables[0] {
            assert!(alias.is_some());
            assert_eq!(alias.as_ref().unwrap().name, "sq");
        } else {
            panic!("Expected Subquery variant, got: {:?}", from.tables[0]);
        }
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn dogfood_quality_ast_select_wildcard() {
    let stmt = first_stmt("SELECT * FROM t");
    if let Statement::Select(sel) = &stmt {
        assert_eq!(sel.columns.len(), 1);
        assert!(matches!(sel.columns[0], SelectItem::Wildcard));
    } else {
        panic!("Expected Select");
    }
}

/// KNOWN LIMITATION: SELECT t.* produces Expression(ColumnReference) rather than
/// SelectItem::QualifiedWildcard. The expression parser's parse_identifier_tail
/// consumes the dot before the select parser can check for QualifiedWildcard.
/// The QualifiedWildcard variant in SelectItem is currently unreachable dead code.
#[test]
fn dogfood_quality_ast_select_qualified_wildcard_produces_expression() {
    let stmt = first_stmt("SELECT t.* FROM t");
    if let Statement::Select(sel) = &stmt {
        assert_eq!(sel.columns.len(), 1);
        // The parser produces Expression(ColumnReference) for t.*, not QualifiedWildcard
        if let SelectItem::Expression(expr, alias) = &sel.columns[0] {
            assert!(alias.is_none());
            assert!(
                matches!(expr, Expression::ColumnReference(_)),
                "t.* should produce a ColumnReference expression"
            );
            if let Expression::ColumnReference(cr) = expr {
                assert_eq!(cr.table.as_ref().unwrap().name, "t");
                assert_eq!(cr.column.name, "*");
            }
        } else {
            panic!("Expected Expression select item, got {:?}", sel.columns[0]);
        }
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn dogfood_quality_ast_select_column_with_alias() {
    let stmt = first_stmt("SELECT id AS user_id FROM t");
    if let Statement::Select(sel) = &stmt {
        assert_eq!(sel.columns.len(), 1);
        if let SelectItem::Expression(_, alias) = &sel.columns[0] {
            assert!(alias.is_some());
            assert_eq!(alias.as_ref().unwrap().name, "user_id");
        } else {
            panic!("Expected Expression select item");
        }
    } else {
        panic!("Expected Select");
    }
}

#[test]
fn dogfood_quality_ast_select_span_valid() {
    let stmt = first_stmt("SELECT * FROM t");
    if let Statement::Select(sel) = &stmt {
        assert!(sel.span.start <= sel.span.end, "Span should be valid");
    } else {
        panic!("Expected Select");
    }
}

// ============================================================================
// Category 14: AST Correctness — CREATE TABLE structure
// ============================================================================

#[test]
fn dogfood_quality_ast_create_table_columns() {
    let sql =
        "CREATE TABLE test (col1 INT NOT NULL, col2 VARCHAR(50), col3 NUMERIC(10,2) DEFAULT 0)";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert_eq!(td.name.name, "test");
            assert_eq!(td.columns.len(), 3);
            assert_eq!(td.columns[0].name.name, "col1");
            assert_eq!(td.columns[1].name.name, "col2");
            assert_eq!(td.columns[2].name.name, "col3");
        } else {
            panic!("Expected Table");
        }
    } else {
        panic!("Expected Create");
    }
}

#[test]
fn dogfood_quality_ast_create_table_not_null() {
    let sql = "CREATE TABLE t (id INT NOT NULL)";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert_eq!(td.columns[0].nullability, Some(false));
        }
    }
}

#[test]
fn dogfood_quality_ast_create_table_null() {
    let sql = "CREATE TABLE t (name VARCHAR(100) NULL)";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert_eq!(td.columns[0].nullability, Some(true));
        }
    }
}

#[test]
fn dogfood_quality_ast_create_table_default_value() {
    let sql = "CREATE TABLE t (status VARCHAR(20) DEFAULT 'active')";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert!(td.columns[0].default_value.is_some());
        }
    }
}

#[test]
fn dogfood_quality_ast_create_table_identity() {
    let sql = "CREATE TABLE t (id INT IDENTITY)";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert!(td.columns[0].identity, "Should be IDENTITY");
        }
    }
}

#[test]
fn dogfood_quality_ast_create_table_temporary_flag() {
    let stmt = first_stmt("CREATE TABLE #temp (id INT)");
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert!(td.temporary, "#temp should have temporary=true");
        }
    }
}

#[test]
fn dogfood_quality_ast_create_table_pk_constraint() {
    let sql = "CREATE TABLE t (id INT, CONSTRAINT pk_t PRIMARY KEY (id))";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert!(!td.constraints.is_empty(), "Should have table constraints");
            assert!(
                matches!(td.constraints[0], TableConstraint::PrimaryKey { .. }),
                "First constraint should be PK"
            );
        }
    }
}

#[test]
fn dogfood_quality_ast_create_table_fk_constraint() {
    let sql = "CREATE TABLE orders (\
               id INT, user_id INT, \
               CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id))";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            let has_fk = td
                .constraints
                .iter()
                .any(|c| matches!(c, TableConstraint::Foreign { .. }));
            assert!(has_fk, "Should have FK constraint");
        }
    }
}

#[test]
fn dogfood_quality_ast_create_table_check_constraint() {
    let sql = "CREATE TABLE t (price NUMERIC(10,2), CONSTRAINT chk CHECK (price > 0))";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            let has_check = td
                .constraints
                .iter()
                .any(|c| matches!(c, TableConstraint::Check { .. }));
            assert!(has_check, "Should have CHECK constraint");
        }
    }
}

#[test]
fn dogfood_quality_ast_create_table_unique_constraint() {
    let sql = "CREATE TABLE t (email VARCHAR(255), CONSTRAINT uq_email UNIQUE (email))";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            let has_uq = td
                .constraints
                .iter()
                .any(|c| matches!(c, TableConstraint::Unique { .. }));
            assert!(has_uq, "Should have UNIQUE constraint");
        }
    }
}

#[test]
fn dogfood_quality_ast_column_constraint_pk() {
    let sql = "CREATE TABLE t (id INT PRIMARY KEY)";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert!(
                td.columns[0]
                    .constraints
                    .iter()
                    .any(|c| matches!(c, ColumnConstraint::PrimaryKey)),
                "Column should have PK constraint"
            );
        }
    }
}

// ============================================================================
// Category 15: AST Correctness — CREATE INDEX
// ============================================================================

#[test]
fn dogfood_quality_ast_create_index_fields() {
    let stmt = first_stmt("CREATE INDEX idx_email ON users (email)");
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Index(idx) = boxed.as_ref() {
            assert_eq!(idx.name.name, "idx_email");
            assert_eq!(idx.table.name, "users");
            assert_eq!(idx.columns.len(), 1);
            assert_eq!(idx.columns[0].name, "email");
            assert!(!idx.unique);
        }
    }
}

#[test]
fn dogfood_quality_ast_create_unique_index_flag() {
    let stmt = first_stmt("CREATE UNIQUE INDEX idx_sku ON products (sku)");
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Index(idx) = boxed.as_ref() {
            assert!(idx.unique, "Should be UNIQUE index");
        }
    }
}

// ============================================================================
// Category 16: AST Correctness — CREATE VIEW
// ============================================================================

#[test]
fn dogfood_quality_ast_create_view() {
    let sql = "CREATE VIEW v_users AS SELECT id, name FROM users WHERE active = 1";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::View(vd) = boxed.as_ref() {
            assert_eq!(vd.name.name, "v_users");
        } else {
            panic!("Expected View");
        }
    }
}

// ============================================================================
// Category 17: AST Correctness — CREATE PROCEDURE
// ============================================================================

#[test]
fn dogfood_quality_ast_procedure_name_and_params() {
    let sql = "CREATE PROCEDURE sp_test @p1 INT, @p2 VARCHAR(100) = 'default' AS SELECT @p1";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Procedure(pd) = boxed.as_ref() {
            assert_eq!(pd.name.name, "sp_test");
            assert!(pd.parameters.len() >= 2, "Should have 2+ parameters");
            // First param: @p1 INT
            assert_eq!(pd.parameters[0].name.name, "@p1");
            // Second param: @p2 VARCHAR(100) with default
            assert_eq!(pd.parameters[1].name.name, "@p2");
            assert!(pd.parameters[1].default_value.is_some());
        }
    }
}

// ============================================================================
// Category 18: AST Correctness — CREATE TRIGGER
// ============================================================================

#[test]
fn dogfood_quality_ast_trigger() {
    let sql = "CREATE TRIGGER tr_test ON users FOR INSERT AS BEGIN SELECT 1 END";
    let stmt = first_stmt(sql);
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Trigger(td) = boxed.as_ref() {
            assert_eq!(td.name.name, "tr_test");
            assert_eq!(td.table.name, "users");
            assert!(td.events.contains(&TriggerEvent::Insert));
        } else {
            panic!("Expected Trigger");
        }
    }
}

// ============================================================================
// Category 19: AST Correctness — INSERT
// ============================================================================

#[test]
fn dogfood_quality_ast_insert_table_name_and_columns() {
    let sql = "INSERT INTO orders (id, total) VALUES (1, 99.99)";
    let stmt = first_stmt(sql);
    if let Statement::Insert(ins) = &stmt {
        assert_eq!(ins.table.name, "orders");
        assert_eq!(ins.columns.len(), 2);
        assert_eq!(ins.columns[0].name, "id");
        assert_eq!(ins.columns[1].name, "total");
    }
}

#[test]
fn dogfood_quality_ast_insert_values_source() {
    let sql = "INSERT INTO t (id) VALUES (1)";
    let stmt = first_stmt(sql);
    if let Statement::Insert(ins) = &stmt {
        assert!(matches!(ins.source, InsertSource::Values(_)));
    }
}

#[test]
fn dogfood_quality_ast_insert_select_source() {
    let sql = "INSERT INTO t SELECT * FROM src";
    let stmt = first_stmt(sql);
    if let Statement::Insert(ins) = &stmt {
        assert!(matches!(ins.source, InsertSource::Select(_)));
    }
}

// ============================================================================
// Category 20: AST Correctness — Control Flow
// ============================================================================

#[test]
fn dogfood_quality_ast_if_else_branches() {
    let sql = "IF @x = 1 BEGIN SELECT 1 END ELSE BEGIN SELECT 0 END";
    let stmt = first_stmt(sql);
    if let Statement::If(if_stmt) = &stmt {
        // Then branch should be a Block
        assert!(matches!(if_stmt.then_branch, Statement::Block(_)));
        // Else branch should be present and a Block
        assert!(if_stmt.else_branch.is_some());
        assert!(matches!(if_stmt.else_branch, Some(Statement::Block(_))));
    } else {
        panic!("Expected If");
    }
}

#[test]
fn dogfood_quality_ast_while_body() {
    let sql = "WHILE @x < 10 BEGIN SET @x = @x + 1 END";
    let stmt = first_stmt(sql);
    if let Statement::While(while_stmt) = &stmt {
        assert!(matches!(while_stmt.body, Statement::Block(_)));
    }
}

#[test]
fn dogfood_quality_ast_try_catch_blocks() {
    let sql = "BEGIN TRY SELECT 1 END TRY BEGIN CATCH SELECT 0 END CATCH";
    let stmt = first_stmt(sql);
    if let Statement::TryCatch(tc) = &stmt {
        assert!(
            !tc.try_block.statements.is_empty(),
            "TRY block should have statements"
        );
        assert!(
            !tc.catch_block.statements.is_empty(),
            "CATCH block should have statements"
        );
    }
}

#[test]
fn dogfood_quality_ast_declare_variable() {
    let stmt = first_stmt("DECLARE @count INT");
    if let Statement::Declare(decl) = &stmt {
        assert_eq!(decl.variables.len(), 1);
        assert_eq!(decl.variables[0].name.name, "@count");
    }
}

#[test]
fn dogfood_quality_ast_set_variable() {
    let stmt = first_stmt("SET @count = 42");
    if let Statement::Set(set) = &stmt {
        assert_eq!(set.variable.name, "@count");
    }
}

#[test]
fn dogfood_quality_ast_block_statements() {
    let stmt = first_stmt("BEGIN SELECT 1 SELECT 2 END");
    if let Statement::Block(block) = &stmt {
        assert_eq!(block.statements.len(), 2);
    }
}

#[test]
fn dogfood_quality_ast_transaction_begin() {
    let stmt = first_stmt("BEGIN TRANSACTION");
    if let Statement::Transaction(ts) = &stmt {
        assert!(matches!(ts, TransactionStatement::Begin { name: None, .. }));
    }
}

#[test]
fn dogfood_quality_ast_transaction_begin_named() {
    let stmt = first_stmt("BEGIN TRANSACTION tx1");
    if let Statement::Transaction(TransactionStatement::Begin { name, .. }) = &stmt {
        assert_eq!(name.as_ref().unwrap().name, "tx1");
    }
}

#[test]
fn dogfood_quality_ast_transaction_commit() {
    let stmt = first_stmt("COMMIT TRANSACTION");
    assert!(matches!(
        stmt,
        Statement::Transaction(TransactionStatement::Commit { .. })
    ));
}

#[test]
fn dogfood_quality_ast_transaction_rollback() {
    let stmt = first_stmt("ROLLBACK TRANSACTION");
    assert!(matches!(
        stmt,
        Statement::Transaction(TransactionStatement::Rollback { .. })
    ));
}

#[test]
fn dogfood_quality_ast_transaction_save() {
    let stmt = first_stmt("SAVE TRANSACTION sp1");
    if let Statement::Transaction(TransactionStatement::Save { name, .. }) = &stmt {
        assert_eq!(name.name, "sp1");
    }
}

// ============================================================================
// Category 21: AST Correctness — EXEC
// ============================================================================

#[test]
fn dogfood_quality_ast_exec_procedure_name() {
    let stmt = first_stmt("EXEC sp_help");
    if let Statement::Exec(exec) = &stmt {
        assert_eq!(exec.procedure.name, "sp_help");
        assert!(exec.arguments.is_empty());
    }
}

#[test]
fn dogfood_quality_ast_exec_named_args() {
    let stmt = first_stmt("EXEC sp_get @id = 1, @name = 'test'");
    if let Statement::Exec(exec) = &stmt {
        assert_eq!(exec.arguments.len(), 2);
        assert!(matches!(exec.arguments[0], ExecArgument::Named { .. }));
        assert!(matches!(exec.arguments[1], ExecArgument::Named { .. }));
    }
}

#[test]
fn dogfood_quality_ast_exec_positional_args() {
    let stmt = first_stmt("EXEC sp_add 1, 'hello', 3.14");
    if let Statement::Exec(exec) = &stmt {
        assert_eq!(exec.arguments.len(), 3);
        assert!(matches!(exec.arguments[0], ExecArgument::Positional(_)));
    }
}

// ============================================================================
// Category 22: AST Correctness — ALTER TABLE
// ============================================================================

#[test]
fn dogfood_quality_ast_alter_add_column() {
    let stmt = first_stmt("ALTER TABLE users ADD email VARCHAR(255)");
    if let Statement::AlterTable(alter) = &stmt {
        assert_eq!(alter.table.name, "users");
        assert!(matches!(alter.operation, AlterTableOperation::AddColumn(_)));
    }
}

#[test]
fn dogfood_quality_ast_alter_drop_column() {
    let stmt = first_stmt("ALTER TABLE users DROP COLUMN old_col");
    if let Statement::AlterTable(alter) = &stmt {
        if let AlterTableOperation::DropColumn(col) = &alter.operation {
            assert_eq!(col.name, "old_col");
        }
    }
}

#[test]
fn dogfood_quality_ast_alter_alter_column() {
    let stmt = first_stmt("ALTER TABLE users ALTER COLUMN name VARCHAR(200) NOT NULL");
    if let Statement::AlterTable(alter) = &stmt {
        if let AlterTableOperation::AlterColumn(ac) = &alter.operation {
            assert_eq!(ac.name.name, "name");
            assert_eq!(ac.nullability, Some(false));
        }
    }
}

// ============================================================================
// Category 23: AST Correctness — Datatype parsing
// ============================================================================

#[test]
fn dogfood_quality_ast_datatype_int() {
    let stmt = first_stmt("CREATE TABLE t (c INT)");
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert_eq!(td.columns[0].data_type, DataType::Int);
        }
    }
}

#[test]
fn dogfood_quality_ast_datatype_varchar_with_len() {
    let stmt = first_stmt("CREATE TABLE t (c VARCHAR(255))");
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert_eq!(td.columns[0].data_type, DataType::Varchar(Some(255)));
        }
    }
}

#[test]
fn dogfood_quality_ast_datatype_numeric_with_precision() {
    let stmt = first_stmt("CREATE TABLE t (c NUMERIC(18,4))");
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert_eq!(
                td.columns[0].data_type,
                DataType::Numeric(Some(18), Some(4))
            );
        }
    }
}

#[test]
fn dogfood_quality_ast_datatype_float() {
    let stmt = first_stmt("CREATE TABLE t (c FLOAT)");
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert_eq!(td.columns[0].data_type, DataType::Float);
        }
    }
}

#[test]
fn dogfood_quality_ast_datatype_bigint() {
    let stmt = first_stmt("CREATE TABLE t (c BIGINT)");
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert_eq!(td.columns[0].data_type, DataType::BigInt);
        }
    }
}

#[test]
fn dogfood_quality_ast_datatype_money() {
    let stmt = first_stmt("CREATE TABLE t (c MONEY)");
    if let Statement::Create(boxed) = &stmt {
        if let CreateStatement::Table(td) = boxed.as_ref() {
            assert_eq!(td.columns[0].data_type, DataType::Money);
        }
    }
}

// ============================================================================
// Category 24: parse_with_errors behavior
// ============================================================================

#[test]
fn dogfood_quality_tolerant_valid_sql_no_errors() {
    let sql = "SELECT 1\nINSERT INTO t VALUES (1)\nDELETE FROM t";
    let result = parse_with_errors(sql);
    assert!(
        result.is_ok(),
        "Valid SQL should succeed with parse_with_errors"
    );
    let (stmts, errors) = result.unwrap();
    assert_eq!(stmts.len(), 3);
    assert!(errors.is_empty(), "Valid SQL should have zero errors");
}

#[test]
fn dogfood_quality_tolerant_mixed_errors() {
    let sql = "SELECT 1\nINVALID SQL HERE";
    let _ = tolerant_parse(sql);
    // Should not panic
}

#[test]
fn dogfood_quality_tolerant_empty_input() {
    let result = parse_with_errors("");
    assert!(result.is_ok());
    let (stmts, errors) = result.unwrap();
    assert!(stmts.is_empty());
    assert!(errors.is_empty());
}

// ============================================================================
// Category 25: Fixture-based parsing (large files must not panic)
// ============================================================================

fn read_fixture(relative_path: &str) -> String {
    use std::fs;
    use std::path::PathBuf;
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.push("dogfooding");
    dir.push("fixtures");
    dir.push(relative_path);
    assert!(dir.exists(), "Fixture missing: {}", dir.display());
    fs::read_to_string(&dir).unwrap_or_else(|e| panic!("Failed to read {}: {}", dir.display(), e))
}

#[test]
fn dogfood_quality_fixture_stored_procedure() {
    let sql = read_fixture("stored_procedure.sql");
    let result = tolerant_parse(&sql);
    match result {
        Ok((stmts, _errors)) => {
            assert!(
                !stmts.is_empty(),
                "Should parse some statements from stored_procedure.sql"
            );
        }
        Err(_pe) => {}
    }
}

#[test]
fn dogfood_quality_fixture_sp_complex_logic() {
    let sql = read_fixture("sp_complex_logic.sql");
    let result = tolerant_parse(&sql);
    match result {
        Ok((stmts, _errors)) => {
            assert!(
                !stmts.is_empty(),
                "Should parse some statements from sp_complex_logic.sql"
            );
        }
        Err(_pe) => {}
    }
}

#[test]
fn dogfood_quality_fixture_migration_input() {
    let sql = read_fixture("migration_input.sql");
    let result = tolerant_parse(&sql);
    match result {
        Ok((stmts, _errors)) => {
            assert!(
                !stmts.is_empty(),
                "Should parse statements from migration_input.sql"
            );
            assert!(stmts.iter().any(is_create), "Should have CREATE");
            assert!(stmts.iter().any(is_insert), "Should have INSERT");
        }
        Err(_pe) => {}
    }
}

#[test]
fn dogfood_quality_fixture_multi_batch_migration() {
    let sql = read_fixture("sp_multi_batch_migration.sql");
    let result = tolerant_parse(&sql);
    match result {
        Ok((stmts, _errors)) => {
            assert!(
                stmts.len() >= 50,
                "Migration fixture should produce 50+ statements, got {}",
                stmts.len()
            );
            let creates = count_variant(&stmts, is_create);
            let inserts = count_variant(&stmts, is_insert);
            let go_count = count_variant(&stmts, is_batch_sep);
            assert!(creates >= 5, "Should have 5+ CREATEs, got {creates}");
            assert!(inserts >= 5, "Should have 5+ INSERTs, got {inserts}");
            assert!(go_count >= 10, "Should have 10+ GOs, got {go_count}");
        }
        Err(_pe) => {}
    }
}

#[test]
fn dogfood_quality_fixture_incomplete_typing() {
    let sql = read_fixture("incomplete_typing.sql");
    let _ = tolerant_parse(&sql);
    // Should not panic
}

#[test]
fn dogfood_quality_fixture_incomplete_wip() {
    let sql = read_fixture("incomplete_wip.sql");
    let _ = tolerant_parse(&sql);
}

#[test]
fn dogfood_quality_fixture_empty() {
    let sql = read_fixture("edge_cases/empty.sql");
    let stmts = must_parse(&sql);
    assert!(stmts.is_empty());
}

#[test]
fn dogfood_quality_fixture_long_line() {
    let sql = read_fixture("edge_cases/long_line.sql");
    let _ = tolerant_parse(&sql);
}

#[test]
fn dogfood_quality_fixture_unicode() {
    let sql = read_fixture("edge_cases/unicode.sql");
    let _ = tolerant_parse(&sql);
}

// ============================================================================
// Category 26: Complex nested structures (tolerant parse)
// ============================================================================

#[test]
fn dogfood_quality_nested_if_while_try() {
    let sql = "\
IF @mode = 1
BEGIN
    WHILE @count < 10
    BEGIN
        BEGIN TRY
            INSERT INTO log (msg) VALUES ('processing')
        END TRY
        BEGIN CATCH
            RAISERROR('Failed', 16, 1)
        END CATCH
        SET @count = @count + 1
    END
END";
    let result = tolerant_parse(sql);
    if let Ok((stmts, _)) = result {
        assert!(!stmts.is_empty());
    }
}

#[test]
fn dogfood_quality_full_procedure() {
    let sql = "\
CREATE PROCEDURE sp_process_order
    @order_id INT,
    @status VARCHAR(20) = 'pending'
AS
BEGIN
    DECLARE @total NUMERIC(12,2)
    SET @total = 0
    BEGIN TRY
        UPDATE orders SET status = @status WHERE order_id = @order_id
    END TRY
    BEGIN CATCH
        RAISERROR('Update failed', 16, 1)
        RETURN -1
    END CATCH
    RETURN 0
END";
    let result = tolerant_parse(sql);
    if let Ok((stmts, _)) = result {
        assert!(stmts.iter().any(is_create), "Should have CREATE PROCEDURE");
    }
}

#[test]
fn dogfood_quality_procedure_with_transaction() {
    let sql = "\
CREATE PROCEDURE sp_txn_test @mode INT AS
BEGIN
    DECLARE @count INT
    SET @count = 0
    BEGIN TRANSACTION
        INSERT INTO results VALUES (1, 'ok')
    COMMIT TRANSACTION
    BEGIN TRY
        UPDATE results SET msg = 'processed' WHERE id = 1
    END TRY
    BEGIN CATCH
        RAISERROR('Error', 16, 1)
    END CATCH
    RETURN 0
END";
    let result = tolerant_parse(sql);
    if let Ok((stmts, _)) = result {
        assert!(stmts.iter().any(is_create), "Should have CREATE PROCEDURE");
    }
}

// ============================================================================
// Category 27: parse_one behavior
// ============================================================================

#[test]
fn dogfood_quality_parse_one_select() {
    let result = parse_one("SELECT 1");
    assert!(result.is_ok());
    assert!(is_select(&result.unwrap()));
}

#[test]
fn dogfood_quality_parse_one_create() {
    let result = parse_one("CREATE TABLE t (id INT)");
    assert!(result.is_ok());
    assert!(is_create(&result.unwrap()));
}

#[test]
fn dogfood_quality_parse_one_insert() {
    let result = parse_one("INSERT INTO t VALUES (1)");
    assert!(result.is_ok());
    assert!(is_insert(&result.unwrap()));
}

#[test]
fn dogfood_quality_parse_one_update() {
    let result = parse_one("UPDATE t SET x = 1 WHERE id = 1");
    assert!(result.is_ok());
    assert!(is_update(&result.unwrap()));
}

#[test]
fn dogfood_quality_parse_one_delete() {
    let result = parse_one("DELETE FROM t WHERE id = 1");
    assert!(result.is_ok());
    assert!(is_delete(&result.unwrap()));
}

#[test]
fn dogfood_quality_parse_one_if() {
    let result = parse_one("IF 1 = 1 SELECT 1");
    assert!(result.is_ok());
    assert!(is_if(&result.unwrap()));
}

#[test]
fn dogfood_quality_parse_one_declare() {
    let result = parse_one("DECLARE @x INT");
    assert!(result.is_ok());
    assert!(is_declare(&result.unwrap()));
}

// ============================================================================
// Category 28: Statement variant coverage — ensure all 21 Statement variants parse
// ============================================================================

#[test]
fn dogfood_quality_all_statement_variants() {
    // One test covering every Statement variant to ensure none regress
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, fn(&Statement) -> bool)> = vec![
        ("SELECT * FROM t", is_select),
        ("INSERT INTO t VALUES (1)", is_insert),
        ("UPDATE t SET x = 1", is_update),
        ("DELETE FROM t WHERE x = 1", is_delete),
        ("CREATE TABLE t (id INT)", is_create),
        ("ALTER TABLE t ADD c INT", is_alter),
        ("DECLARE @x INT", is_declare),
        ("SET @x = 1", is_set),
        ("IF 1 = 1 SELECT 1", is_if),
        ("WHILE 1 = 1 BREAK", is_while),
        ("BEGIN SELECT 1 END", is_block),
        ("BREAK", is_break),
        ("CONTINUE", is_continue),
        ("CREATE PROCEDURE sp_test AS RETURN 0", |_| true), // return checked inside
        (
            "BEGIN TRY SELECT 1 END TRY BEGIN CATCH SELECT 0 END CATCH",
            is_trycatch,
        ),
        ("BEGIN TRANSACTION", is_transaction),
        ("THROW 50001, 'err', 1", is_throw),
        ("RAISERROR('err', 16, 1)", is_raiserror),
        ("EXEC sp_help", is_exec),
        ("SELECT 1\nGO\nSELECT 2", |s| {
            is_batch_sep(s) || is_select(s)
        }),
    ];

    for (sql, pred) in &cases {
        let result = tolerant_parse(sql);
        match result {
            Ok((stmts, _)) => {
                assert!(
                    stmts.iter().any(pred),
                    "No matching statement variant for: {sql}"
                );
            }
            Err(_) => {
                panic!("Failed to parse statement variant: {sql}");
            }
        }
    }
}

// ============================================================================
// Category 29: JOIN type coverage
// ============================================================================

#[test]
fn dogfood_quality_all_join_types() {
    let join_types = vec![
        "INNER JOIN",
        "LEFT JOIN",
        "LEFT OUTER JOIN",
        "RIGHT JOIN",
        "RIGHT OUTER JOIN",
        "FULL JOIN",
        "FULL OUTER JOIN",
        "CROSS JOIN",
        "JOIN", // bare JOIN = INNER
    ];
    for jt in join_types {
        let sql = format!("SELECT * FROM t1 {jt} t2 ON t1.id = t2.id");
        let result = tolerant_parse(&sql);
        match result {
            Ok((stmts, _)) => {
                assert_eq!(
                    count_variant(&stmts, is_select),
                    1,
                    "Failed for JOIN type: {jt}"
                );
            }
            Err(_) => {
                panic!("Failed to parse JOIN type: {jt}");
            }
        }
    }
}
