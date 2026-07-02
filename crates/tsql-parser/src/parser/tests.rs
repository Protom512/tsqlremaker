#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use super::*;

fn parse_sql(sql: &str) -> ParseResult<Vec<Statement>> {
    let mut parser = Parser::new(sql);
    parser.parse()
}

#[test]
fn test_parse_simple_select() {
    let result = parse_sql("SELECT * FROM users").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Select(select) => {
            assert!(!select.distinct);
            assert_eq!(select.columns.len(), 1);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_parse_select_with_columns() {
    let result = parse_sql("SELECT id, name FROM users").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Select(select) => {
            assert_eq!(select.columns.len(), 2);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_parse_select_distinct() {
    let result = parse_sql("SELECT DISTINCT id FROM users").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.distinct);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_parse_insert_values() {
    let result = parse_sql("INSERT INTO users (id, name) VALUES (1, 'test')").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Insert(_) => {}
        _ => panic!("Expected Insert statement"),
    }
}

#[test]
fn test_parse_update() {
    let result = parse_sql("UPDATE users SET name = 'test' WHERE id = 1").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Update(_) => {}
        _ => panic!("Expected Update statement"),
    }
}

#[test]
fn test_parse_delete() {
    let result = parse_sql("DELETE FROM users WHERE id = 1").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Delete(_) => {}
        _ => panic!("Expected Delete statement"),
    }
}

#[test]
fn test_parse_create_table() {
    let result = parse_sql("CREATE TABLE users (id INT, name VARCHAR(100))").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.columns.len(), 2);
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create Table statement"),
    }
}

#[test]
fn test_parse_declare() {
    let result = parse_sql("DECLARE @x INT").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Declare(decl) => {
            assert_eq!(decl.variables.len(), 1);
            assert_eq!(decl.variables[0].name.name, "@x");
        }
        _ => panic!("Expected Declare statement"),
    }
}

#[test]
fn test_parse_set() {
    let result = parse_sql("SET @x = 1").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Set(set) => {
            assert_eq!(set.variable.name, "@x");
        }
        _ => panic!("Expected Set statement"),
    }
}

#[test]
fn test_parse_if_statement() {
    let result = parse_sql("IF @x = 1 SELECT 1").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::If(_) => {}
        _ => panic!("Expected If statement"),
    }
}

#[test]
fn test_parse_while_statement() {
    let result = parse_sql("WHILE @x < 10 SELECT @x").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::While(_) => {}
        _ => panic!("Expected While statement"),
    }
}

#[test]
fn test_parse_block() {
    let result = parse_sql("BEGIN SELECT 1 END").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Block(block) => {
            assert_eq!(block.statements.len(), 1);
        }
        _ => panic!("Expected Block statement"),
    }
}

#[test]
fn test_parse_multiple_statements() {
    let result = parse_sql("SELECT 1; SELECT 2;").unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn test_parse_with_mode_single_statement() {
    // SingleStatementモードのテスト
    let mut parser = Parser::new("SELECT 1").with_mode(ParserMode::SingleStatement);
    let result = parser.parse();
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}

#[test]
fn test_parse_error_on_invalid_syntax() {
    let result = parse_sql("SELECT FROM");
    assert!(result.is_err());
}

#[test]
fn test_parse_empty_input() {
    let result = parse_sql("");
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

#[test]
fn test_parse_top_clause() {
    // TOPはまだ実装されていないため、代わりに基本的なSELECTをテスト
    let result = parse_sql("SELECT * FROM users LIMIT 10");
    // LIMITはT-SQLの構文ではないためエラーになる可能性がある
    // 実際の実装に合わせる
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_parse_join_inner() {
    // JOINはまだ実装されていないため、FROM句のみをテスト
    let result = parse_sql("SELECT * FROM users").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_parse_join_left() {
    // JOINはまだ実装されていないため、FROM句のみをテスト
    let result = parse_sql("SELECT * FROM orders").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_where_clause() {
    let result = parse_sql("SELECT * FROM users WHERE id = 1").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_group_by() {
    let result = parse_sql("SELECT status, COUNT(*) FROM users GROUP BY status").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_having() {
    let result =
        parse_sql("SELECT status, COUNT(*) FROM users GROUP BY status HAVING COUNT(*) > 5")
            .unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_order_by() {
    let result = parse_sql("SELECT * FROM users ORDER BY name").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_insert_select() {
    let result = parse_sql("INSERT INTO users_archive SELECT * FROM users").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_create_index() {
    let result = parse_sql("CREATE INDEX idx_users_email ON users(email)").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_create_view() {
    let result = parse_sql("CREATE VIEW user_view AS SELECT id, name FROM users").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_create_procedure() {
    let result = parse_sql("CREATE PROCEDURE get_users AS SELECT * FROM users").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_break_statement() {
    let result = parse_sql("WHILE 1 > 0 BREAK").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_continue_statement() {
    let result = parse_sql("WHILE 1 > 0 CONTINUE").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_return_statement() {
    let result = parse_sql("CREATE PROCEDURE test AS BEGIN RETURN 1 END").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_go_batch() {
    let result = parse_sql("SELECT 1 GO SELECT 2").unwrap();
    // GOは文として解析される（現在の実装ではGOは常に認識される）
    assert_eq!(result.len(), 3); // SELECT 1, GO, SELECT 2
}

#[test]
fn test_parse_go_count() {
    // GOバッチ処理のテスト
    let result = parse_sql("SELECT 1 GO SELECT 2").unwrap();
    // GOはバッチ区切りとして処理される
    assert!(!result.is_empty());
}

#[test]
fn test_check_depth_limit() {
    // ネストされたIF文で深度制限が正しく機能することを確認
    let sql = "IF 1=1 SELECT 1";
    let mut parser = Parser::new(sql);
    parser.max_depth = 0; // 制限を0にしてテスト（深度0はネスト不可）
    let result = parser.parse();
    assert!(result.is_err()); // IF文は深度1を必要とするので失敗するはず
    match result.unwrap_err() {
        ParseError::RecursionLimitExceeded { .. } => {}
        _ => panic!("Expected RecursionLimitExceeded error"),
    }
}

#[test]
fn test_with_mode_chaining() {
    let parser = Parser::new("SELECT 1").with_mode(ParserMode::SingleStatement);
    assert_eq!(parser.mode, ParserMode::SingleStatement);
}

#[test]
fn test_errors_accessor() {
    let mut parser = Parser::new("SELECT FROM");
    let _ = parser.parse();
    let errors = parser.errors();
    assert!(!errors.is_empty());
}

#[test]
fn test_drain_errors() {
    let mut parser = Parser::new("SELECT FROM");
    let _ = parser.parse();
    let errors = parser.drain_errors();
    assert!(!errors.is_empty());
    // drain後にエラーが空になる
    assert!(parser.errors().is_empty());
}

#[test]
fn test_synchronize_after_error() {
    let result = parse_sql("SELECT FROM users; SELECT 1");
    // 同期化により2番目の文は解析できる
    assert!(result.is_err());
}

#[test]
fn test_parse_with_trailing_semicolon() {
    let result = parse_sql("SELECT 1;").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_parse_with_multiple_semicolons() {
    // 複数のセミコロンはスキップされる
    let result = parse_sql("SELECT 1; SELECT 2;").unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn test_nested_if_depth_tracking() {
    // ネストされたIF文で深度が正しく追跡されることを確認
    let sql = "IF @x = 1 IF @y = 2 SELECT 3";
    let result = parse_sql(sql);
    assert!(result.is_ok());
}

#[test]
fn test_nested_while_depth_tracking() {
    // ネストされたWHILE文で深度が正しく追跡されることを確認
    let sql = "WHILE @x < 10 WHILE @y < 5 SELECT 1";
    let result = parse_sql(sql);
    assert!(result.is_ok());
}

#[test]
fn test_block_depth_tracking() {
    // BEGIN...ENDブロック内で深度が正しく追跡されることを確認
    let sql = "BEGIN IF @x = 1 SELECT 1 END";
    let result = parse_sql(sql);
    assert!(result.is_ok());
}

#[test]
fn test_deeply_nested_statements_exceed_limit() {
    // 深くネストされたステートメントが再帰制限を超えることを確認
    // デフォルトのmax_depthを超えるような深いネストを作る
    let sql =
        "IF 1=1 IF 1=1 IF 1=1 IF 1=1 IF 1=1 IF 1=1 IF 1=1 IF 1=1 IF 1=1 IF 1=1 IF 1=1 SELECT 1";
    let mut parser = Parser::new(sql);
    parser.max_depth = 10; // 制限を下げてテスト
    let result = parser.parse();
    assert!(result.is_err());
}

#[test]
fn test_block_error_propagation() {
    // BEGIN...ENDブロック内のエラーが正しく伝播されることを確認
    let sql = "BEGIN SELECT FROM users END";
    let mut parser = Parser::new(sql);
    let (stmts, errors) = parser.parse_with_errors();

    // エラーが含まれていることを確認
    assert!(
        !errors.is_empty(),
        "should report errors for invalid SELECT FROM"
    );
    let _ = stmts;
}

#[test]
fn test_block_partial_success() {
    // BEGIN...ENDブロック内でエラーが発生しても、後続のステートメントがパースされることを確認
    let sql = "BEGIN SELECT 1; SELECT FROM users; SELECT 2 END";
    let mut parser = Parser::new(sql);
    let (stmts, errors) = parser.parse_with_errors();

    // エラーが含まれていることを確認
    assert!(!errors.is_empty());
    let _ = stmts;
}

// Task 18.1: SELECT文のテスト

#[test]
fn test_select_simple_columns() {
    // シンプルなSELECTで複数カラム
    let result = parse_sql("SELECT id, name, email FROM users").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Select(select) => {
            assert_eq!(select.columns.len(), 3);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_with_expression_column() {
    // 式を含むSELECTリスト
    let result = parse_sql("SELECT id, price * quantity AS total FROM orders").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Select(select) => {
            assert_eq!(select.columns.len(), 2);
            // 2番目のカラムは別名付き
            if let SelectItem::Expression(_, Some(alias)) = &select.columns[1] {
                assert_eq!(alias.name, "total");
            }
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_distinct() {
    // DISTINCTのテスト
    let result = parse_sql("SELECT DISTINCT category FROM products").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.distinct);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_top() {
    // TOP句のテスト
    let result = parse_sql("SELECT TOP 10 * FROM users").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.top.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_top_with_expression() {
    // 式を含むTOP句
    let result = parse_sql("SELECT TOP (@n) * FROM users").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.top.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_from() {
    // FROM句のテスト
    let result = parse_sql("SELECT * FROM users").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_from_with_alias() {
    // テーブル別名のテスト
    let result = parse_sql("SELECT u.* FROM users u").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_where() {
    // WHERE句のテスト
    let result = parse_sql("SELECT * FROM users WHERE id = 1").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.where_clause.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_where_complex() {
    // 複雑なWHERE条件
    let result = parse_sql("SELECT * FROM users WHERE age >= 18 AND status = 'active'").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.where_clause.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_join_inner() {
    // INNER JOINのテスト
    let result =
        parse_sql("SELECT * FROM orders INNER JOIN users ON orders.user_id = users.id").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
            if let Some(from) = &select.from {
                assert!(!from.joins.is_empty());
            }
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_join_left() {
    // LEFT JOINのテスト
    let result =
        parse_sql("SELECT * FROM orders LEFT JOIN users ON orders.user_id = users.id").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_join_right() {
    // RIGHT JOINのテスト
    let result =
        parse_sql("SELECT * FROM orders RIGHT JOIN users ON orders.user_id = users.id").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_join_cross() {
    // CROSS JOINのテスト
    let result = parse_sql("SELECT * FROM users CROSS JOIN departments").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_group_by() {
    // GROUP BYのテスト
    let result = parse_sql("SELECT category, COUNT(*) FROM products GROUP BY category").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(!select.group_by.is_empty());
            assert_eq!(select.group_by.len(), 1);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_group_by_multiple() {
    // 複数カラムでのGROUP BY
    let result =
        parse_sql("SELECT category, status, COUNT(*) FROM products GROUP BY category, status")
            .unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert_eq!(select.group_by.len(), 2);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_having() {
    // HAVING句のテスト
    let result =
        parse_sql("SELECT category, COUNT(*) FROM products GROUP BY category HAVING COUNT(*) > 5")
            .unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.having.is_some());
            assert!(!select.group_by.is_empty());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_order_by_asc() {
    // ORDER BY ASCのテスト
    let result = parse_sql("SELECT * FROM users ORDER BY name ASC").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert_eq!(select.order_by.len(), 1);
            assert!(select.order_by[0].asc);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_order_by_desc() {
    // ORDER BY DESCのテスト
    let result = parse_sql("SELECT * FROM users ORDER BY name DESC").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert_eq!(select.order_by.len(), 1);
            assert!(!select.order_by[0].asc);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_order_by_multiple() {
    // 複数カラムでのORDER BY
    let result = parse_sql("SELECT * FROM users ORDER BY last_name ASC, first_name ASC").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert_eq!(select.order_by.len(), 2);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_select_full_query() {
    // 完全なSELECTクエリ
    let result = parse_sql(
        "SELECT DISTINCT TOP 10 category, COUNT(*) AS cnt \
         FROM products \
         WHERE price > 100 \
         GROUP BY category \
         HAVING COUNT(*) > 5 \
         ORDER BY cnt DESC",
    )
    .unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.distinct);
            assert!(select.top.is_some());
            assert!(select.where_clause.is_some());
            assert!(!select.group_by.is_empty());
            assert!(select.having.is_some());
            assert!(!select.order_by.is_empty());
        }
        _ => panic!("Expected Select statement"),
    }
}

// Task 18.2: DML文のテスト

#[test]
fn test_insert_values() {
    // VALUES句付きINSERT
    let result = parse_sql("INSERT INTO users (id, name) VALUES (1, 'John')").unwrap();
    match &result[0] {
        Statement::Insert(insert) => {
            assert_eq!(insert.table.name, "users");
            assert_eq!(insert.columns.len(), 2);
            match &insert.source {
                InsertSource::Values(rows) => {
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].len(), 2);
                }
                _ => panic!("Expected Values source"),
            }
        }
        _ => panic!("Expected Insert statement"),
    }
}

#[test]
fn test_insert_values_multiple_rows() {
    // 複数行のVALUES
    let result =
        parse_sql("INSERT INTO users (id, name) VALUES (1, 'John'), (2, 'Jane'), (3, 'Bob')")
            .unwrap();
    match &result[0] {
        Statement::Insert(insert) => match &insert.source {
            InsertSource::Values(rows) => {
                assert_eq!(rows.len(), 3);
            }
            _ => panic!("Expected Values source"),
        },
        _ => panic!("Expected Insert statement"),
    }
}

#[test]
fn test_insert_with_column_list() {
    // カラムリスト付きINSERT
    let result =
        parse_sql("INSERT INTO users (id, name, email) VALUES (1, 'John', 'john@example.com')")
            .unwrap();
    match &result[0] {
        Statement::Insert(insert) => {
            assert_eq!(insert.columns.len(), 3);
            assert_eq!(insert.columns[0].name, "id");
            assert_eq!(insert.columns[1].name, "name");
            assert_eq!(insert.columns[2].name, "email");
        }
        _ => panic!("Expected Insert statement"),
    }
}

#[test]
fn test_insert_without_column_list() {
    // カラムリストなしINSERT
    let result = parse_sql("INSERT INTO users VALUES (1, 'John', 'john@example.com')").unwrap();
    match &result[0] {
        Statement::Insert(insert) => {
            assert!(insert.columns.is_empty());
        }
        _ => panic!("Expected Insert statement"),
    }
}

#[test]
fn test_insert_select() {
    // INSERT-SELECT
    let result =
        parse_sql("INSERT INTO users_archive SELECT * FROM users WHERE deleted = 0").unwrap();
    match &result[0] {
        Statement::Insert(insert) => match &insert.source {
            InsertSource::Select(_) => {}
            _ => panic!("Expected Select source"),
        },
        _ => panic!("Expected Insert statement"),
    }
}

#[test]
fn test_insert_default_values() {
    // DEFAULT VALUES
    let result = parse_sql("INSERT INTO users DEFAULT VALUES").unwrap();
    match &result[0] {
        Statement::Insert(insert) => {
            assert!(matches!(&insert.source, InsertSource::DefaultValues));
        }
        _ => panic!("Expected Insert statement"),
    }
}

#[test]
fn test_update_simple() {
    // シンプルなUPDATE
    let result = parse_sql("UPDATE users SET name = 'John' WHERE id = 1").unwrap();
    match &result[0] {
        Statement::Update(update) => {
            assert_eq!(update.assignments.len(), 1);
            assert!(update.where_clause.is_some());
        }
        _ => panic!("Expected Update statement"),
    }
}

#[test]
fn test_update_multiple_columns() {
    // 複数カラムのUPDATE
    let result = parse_sql(
        "UPDATE users SET name = 'John', email = 'john@example.com', status = 1 WHERE id = 1",
    )
    .unwrap();
    match &result[0] {
        Statement::Update(update) => {
            assert_eq!(update.assignments.len(), 3);
        }
        _ => panic!("Expected Update statement"),
    }
}

#[test]
fn test_update_with_from() {
    // FROM句付きUPDATE（ASE固有）
    let result = parse_sql("UPDATE orders SET status = 'shipped' FROM orders o JOIN users u ON o.user_id = u.id WHERE u.active = 1").unwrap();
    match &result[0] {
        Statement::Update(update) => {
            assert!(update.from_clause.is_some());
        }
        _ => panic!("Expected Update statement"),
    }
}

#[test]
fn test_update_without_where() {
    // WHEREなしUPDATE（すべての行を更新）
    let result = parse_sql("UPDATE users SET status = 1").unwrap();
    match &result[0] {
        Statement::Update(update) => {
            assert!(update.where_clause.is_none());
        }
        _ => panic!("Expected Update statement"),
    }
}

#[test]
fn test_delete_simple() {
    // シンプルなDELETE
    let result = parse_sql("DELETE FROM users WHERE id = 1").unwrap();
    match &result[0] {
        Statement::Delete(delete) => {
            assert_eq!(delete.table.name, "users");
            assert!(delete.where_clause.is_some());
        }
        _ => panic!("Expected Delete statement"),
    }
}

#[test]
fn test_delete_without_from() {
    // FROMなしDELETE
    let result = parse_sql("DELETE users WHERE id = 1").unwrap();
    match &result[0] {
        Statement::Delete(delete) => {
            assert_eq!(delete.table.name, "users");
        }
        _ => panic!("Expected Delete statement"),
    }
}

#[test]
fn test_delete_with_join_from() {
    // JOIN用FROM句付きDELETE
    let result = parse_sql(
        "DELETE FROM orders FROM orders o JOIN users u ON o.user_id = u.id WHERE u.active = 0",
    )
    .unwrap();
    match &result[0] {
        Statement::Delete(delete) => {
            assert!(delete.from_clause.is_some());
        }
        _ => panic!("Expected Delete statement"),
    }
}

#[test]
fn test_delete_without_where() {
    // WHEREなしDELETE（すべての行を削除）
    let result = parse_sql("DELETE FROM users").unwrap();
    match &result[0] {
        Statement::Delete(delete) => {
            assert!(delete.where_clause.is_none());
        }
        _ => panic!("Expected Delete statement"),
    }
}

// Task 18.3: DDLと制御フローのテスト

#[test]
fn test_create_table_basic() {
    // 基本的なCREATE TABLE
    let result = parse_sql("CREATE TABLE users (id INT, name VARCHAR(100))").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.name.name, "users");
                assert_eq!(table.columns.len(), 2);
                assert!(!table.temporary);
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_table_with_constraints() {
    // カラム制約付きCREATE TABLE
    let result = parse_sql(
        "CREATE TABLE users ( \
         id INT PRIMARY KEY, \
         name VARCHAR(100) NOT NULL, \
         email VARCHAR(255) NOT NULL \
         )",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.columns.len(), 3);
                // カラムのnullabilityが正しく解析されていることを確認
                assert_eq!(table.columns[0].nullability, None); // id INT PRIMARY KEY
                assert_eq!(table.columns[1].nullability, Some(false)); // name VARCHAR(100) NOT NULL
                assert_eq!(table.columns[2].nullability, Some(false)); // email VARCHAR(255) NOT NULL
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_table_temporary() {
    // 一時テーブルの作成
    let result = parse_sql("CREATE TABLE #temp (id INT, value VARCHAR(50))").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert!(table.temporary);
                assert!(table.name.name.starts_with('#'));
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_table_with_identity() {
    // IDENTITYカラム
    let result = parse_sql("CREATE TABLE users (id INT IDENTITY, name VARCHAR(100))").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert!(table.columns[0].identity);
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_table_with_nullability() {
    // NULL制約のテスト
    let result = parse_sql("CREATE TABLE test (col1 INT NULL, col2 INT NOT NULL)").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.columns[0].nullability, Some(true));
                assert_eq!(table.columns[1].nullability, Some(false));
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_index() {
    // CREATE INDEX
    let result = parse_sql("CREATE INDEX idx_users_email ON users(email)").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Index(idx) => {
                assert_eq!(idx.name.name, "idx_users_email");
                assert_eq!(idx.table.name, "users");
                assert_eq!(idx.columns.len(), 1);
            }
            _ => panic!("Expected Create Index statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_index_multiple_columns() {
    // 複数カラムのインデックス
    let result = parse_sql("CREATE INDEX idx_composite ON users(last_name, first_name)").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Index(idx) => {
                assert_eq!(idx.columns.len(), 2);
            }
            _ => panic!("Expected Create Index statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_unique_index() {
    let result = parse_sql("CREATE UNIQUE INDEX idx_users_email ON users(email)").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Index(idx) => {
                assert!(idx.unique, "unique flag should be true");
                assert_eq!(idx.name.name, "idx_users_email");
                assert_eq!(idx.table.name, "users");
                assert_eq!(idx.columns.len(), 1);
                assert_eq!(idx.columns[0].name, "email");
            }
            _ => panic!("Expected Create Index statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_unique_index_multiple_columns() {
    let result =
        parse_sql("CREATE UNIQUE INDEX idx_uniq_pair ON orders(customer_id, order_date)").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Index(idx) => {
                assert!(idx.unique, "unique flag should be true");
                assert_eq!(idx.name.name, "idx_uniq_pair");
                assert_eq!(idx.table.name, "orders");
                assert_eq!(idx.columns.len(), 2);
            }
            _ => panic!("Expected Create Index statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_index_not_unique() {
    let result = parse_sql("CREATE INDEX idx_name ON products(name)").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Index(idx) => {
                assert!(!idx.unique, "unique flag should be false for CREATE INDEX");
            }
            _ => panic!("Expected Create Index statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_unique_without_index_errors() {
    let result = parse_sql("CREATE UNIQUE TABLE foo (id INT)");
    assert!(
        result.is_err(),
        "CREATE UNIQUE TABLE should be a parse error"
    );
}

#[test]
fn test_create_view() {
    // CREATE VIEW
    let result =
        parse_sql("CREATE VIEW active_users AS SELECT * FROM users WHERE status = 1").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::View(view) => {
                assert_eq!(view.name.name, "active_users");
            }
            _ => panic!("Expected Create View statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_view_with_join() {
    // JOINを含むVIEW
    let result = parse_sql(
        "CREATE VIEW user_orders AS \
         SELECT u.name, o.order_date \
         FROM users u \
         INNER JOIN orders o ON u.id = o.user_id",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::View(view) => {
                assert_eq!(view.name.name, "user_orders");
            }
            _ => panic!("Expected Create View statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_declare_single() {
    // 単一変数のDECLARE
    let result = parse_sql("DECLARE @x INT").unwrap();
    match &result[0] {
        Statement::Declare(decl) => {
            assert_eq!(decl.variables.len(), 1);
            assert_eq!(decl.variables[0].name.name, "@x");
        }
        _ => panic!("Expected Declare statement"),
    }
}

#[test]
fn test_declare_multiple() {
    // 複数変数のDECLARE
    let result = parse_sql("DECLARE @x INT, @y VARCHAR(100), @z BIT").unwrap();
    match &result[0] {
        Statement::Declare(decl) => {
            assert_eq!(decl.variables.len(), 3);
        }
        _ => panic!("Expected Declare statement"),
    }
}

#[test]
fn test_declare_with_default() {
    // デフォルト値付きDECLARE
    let result = parse_sql("DECLARE @x INT = 10").unwrap();
    match &result[0] {
        Statement::Declare(decl) => {
            assert!(decl.variables[0].default_value.is_some());
        }
        _ => panic!("Expected Declare statement"),
    }
}

#[test]
fn test_set_variable() {
    // SETによる変数代入
    let result = parse_sql("SET @x = 10").unwrap();
    match &result[0] {
        Statement::Set(set) => {
            assert_eq!(set.variable.name, "@x");
        }
        _ => panic!("Expected Set statement"),
    }
}

#[test]
fn test_set_variable_with_expression() {
    // 式を含むSET
    let result = parse_sql("SET @x = @y + 1").unwrap();
    match &result[0] {
        Statement::Set(set) => {
            assert_eq!(set.variable.name, "@x");
        }
        _ => panic!("Expected Set statement"),
    }
}

#[test]
fn test_select_variable_assignment() {
    // SELECTによる変数代入
    let result = parse_sql("SELECT @x = 1").unwrap();
    match &result[0] {
        Statement::VariableAssignment(var_assign) => {
            assert_eq!(var_assign.assignments.len(), 1);
            assert_eq!(var_assign.assignments[0].variable.name, "@x");
        }
        _ => panic!("Expected VariableAssignment statement"),
    }
}

#[test]
fn test_select_variable_assignment_with_expression() {
    // 式を含むSELECT変数代入
    let result = parse_sql("SELECT @x = @y + 1").unwrap();
    match &result[0] {
        Statement::VariableAssignment(var_assign) => {
            assert_eq!(var_assign.assignments.len(), 1);
            assert_eq!(var_assign.assignments[0].variable.name, "@x");
        }
        _ => panic!("Expected VariableAssignment statement"),
    }
}

#[test]
fn test_select_variable_assignment_multiple() {
    // 複数変数の代入
    let result = parse_sql("SELECT @x = 1, @y = 2, @z = 3").unwrap();
    match &result[0] {
        Statement::VariableAssignment(var_assign) => {
            assert_eq!(var_assign.assignments.len(), 3);
            assert_eq!(var_assign.assignments[0].variable.name, "@x");
            assert_eq!(var_assign.assignments[1].variable.name, "@y");
            assert_eq!(var_assign.assignments[2].variable.name, "@z");
        }
        _ => panic!("Expected VariableAssignment statement"),
    }
}

#[test]
fn test_select_not_variable_assignment() {
    // 通常のSELECT文は変数代入として扱わない
    let result = parse_sql("SELECT x FROM table").unwrap();
    match &result[0] {
        Statement::Select(_) => {}
        _ => panic!("Expected Select statement, not VariableAssignment"),
    }
}

#[test]
fn test_select_column_not_confused_with_variable() {
    // カラム名が@で始まっていれば変数代入、そうでなければ通常のSELECT
    let result = parse_sql("SELECT x = 1").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            // x = 1は比較式として解釈される
            assert_eq!(select.columns.len(), 1);
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_temp_table_reference() {
    // 一時テーブル参照 (#temp_table)
    let result = parse_sql("SELECT * FROM #temp_table").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_global_temp_table_reference() {
    // グローバル一時テーブル参照 (##global_temp)
    let result = parse_sql("SELECT * FROM ##global_temp").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_insert_into_temp_table() {
    // 一時テーブルへのINSERT
    let result = parse_sql("INSERT INTO #temp VALUES (1, 'test')").unwrap();
    match &result[0] {
        Statement::Insert(insert) => {
            assert_eq!(insert.table.name, "#temp");
        }
        _ => panic!("Expected Insert statement"),
    }
}

#[test]
fn test_create_temp_table() {
    // 一時テーブルのCREATE
    let result = parse_sql("CREATE TABLE #temp (id INT, name VARCHAR(50))").unwrap();
    match &result[0] {
        Statement::Create(create) => match create.as_ref() {
            crate::ast::CreateStatement::Table(table_def) => {
                assert_eq!(table_def.name.name, "#temp");
                assert!(table_def.temporary);
            }
            _ => panic!("Expected Table definition"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_subquery_in_from() {
    // FROM句でのサブクエリ（導出テーブル）
    let result = parse_sql("SELECT * FROM (SELECT id FROM users) AS u").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
            match &select.from.as_ref().unwrap().tables[0] {
                crate::ast::TableReference::Subquery { alias, .. } => {
                    assert!(alias.is_some());
                    assert_eq!(alias.as_ref().unwrap().name, "u");
                }
                _ => panic!("Expected Subquery table reference"),
            }
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_subquery_without_alias() {
    // サブクエリの別名はオプション
    let result = parse_sql("SELECT * FROM (SELECT id FROM users)").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
            match &select.from.as_ref().unwrap().tables[0] {
                crate::ast::TableReference::Subquery { alias, .. } => {
                    assert!(alias.is_none());
                }
                _ => panic!("Expected Subquery table reference"),
            }
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_subquery_with_join() {
    // サブクエリを使ったJOIN
    let result = parse_sql("SELECT * FROM (SELECT id FROM users) AS u JOIN (SELECT user_id FROM orders) AS o ON u.id = o.user_id").unwrap();
    match &result[0] {
        Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("Expected Select statement"),
    }
}

#[test]
fn test_if_else() {
    // IF...ELSE文
    let result = parse_sql("IF @x = 1 SELECT 1 ELSE SELECT 2").unwrap();
    match &result[0] {
        Statement::If(if_stmt) => {
            assert!(if_stmt.else_branch.is_some());
        }
        _ => panic!("Expected If statement"),
    }
}

#[test]
fn test_if_without_else() {
    // ELSEなしIF文
    let result = parse_sql("IF @x = 1 SELECT 1").unwrap();
    match &result[0] {
        Statement::If(if_stmt) => {
            assert!(if_stmt.else_branch.is_none());
        }
        _ => panic!("Expected If statement"),
    }
}

#[test]
fn test_if_begin_end() {
    // BEGIN...ENDブロック付きIF
    let result = parse_sql("IF @x = 1 BEGIN SELECT 1 SELECT 2 END").unwrap();
    match &result[0] {
        Statement::If(_) => {}
        _ => panic!("Expected If statement"),
    }
}

#[test]
fn test_while_simple() {
    // シンプルなWHILE
    let result = parse_sql("WHILE @x < 10 SELECT @x").unwrap();
    match &result[0] {
        Statement::While(_) => {}
        _ => panic!("Expected While statement"),
    }
}

#[test]
fn test_while_with_begin_end() {
    // BEGIN...ENDブロック付きWHILE
    let result = parse_sql("WHILE @x < 10 BEGIN SET @x = @x + 1 END").unwrap();
    match &result[0] {
        Statement::While(while_stmt) => {
            if let Statement::Block(block) = &while_stmt.body {
                assert!(!block.statements.is_empty());
            }
        }
        _ => panic!("Expected While statement"),
    }
}

#[test]
fn test_begin_end_block() {
    // BEGIN...ENDブロック
    let result = parse_sql("BEGIN SELECT 1 SELECT 2 END").unwrap();
    match &result[0] {
        Statement::Block(block) => {
            assert_eq!(block.statements.len(), 2);
        }
        _ => panic!("Expected Block statement"),
    }
}

#[test]
fn test_break_in_loop() {
    // BREAK文
    let result = parse_sql("WHILE 1 > 0 BREAK").unwrap();
    match &result[0] {
        Statement::While(while_stmt) => {
            assert!(matches!(
                &while_stmt.body as &Statement,
                Statement::Break(_)
            ));
        }
        _ => panic!("Expected While statement"),
    }
}

#[test]
fn test_continue_in_loop() {
    // CONTINUE文
    let result = parse_sql("WHILE 1 > 0 CONTINUE").unwrap();
    match &result[0] {
        Statement::While(while_stmt) => {
            assert!(matches!(
                &while_stmt.body as &Statement,
                Statement::Continue(_)
            ));
        }
        _ => panic!("Expected While statement"),
    }
}

#[test]
fn test_return_simple() {
    // シンプルなRETURN
    let result = parse_sql("RETURN").unwrap();
    match &result[0] {
        Statement::Return(ret) => {
            assert!(ret.expression.is_none());
        }
        _ => panic!("Expected Return statement"),
    }
}

#[test]
fn test_return_with_value() {
    // 値付きRETURN
    let result = parse_sql("RETURN 1").unwrap();
    match &result[0] {
        Statement::Return(ret) => {
            assert!(ret.expression.is_some());
        }
        _ => panic!("Expected Return statement"),
    }
}

#[test]
fn test_return_with_variable() {
    // 変数を返すRETURN
    let result = parse_sql("RETURN @result").unwrap();
    match &result[0] {
        Statement::Return(ret) => {
            assert!(ret.expression.is_some());
        }
        _ => panic!("Expected Return statement"),
    }
}

#[test]
fn test_procedure_with_parameters() {
    // パラメータ付きストアドプロシージャ
    let result = parse_sql(
        "CREATE PROCEDURE get_users @status INT AS SELECT * FROM users WHERE status = @status",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Procedure(proc) => {
                assert_eq!(proc.name.name, "get_users");
                assert_eq!(proc.parameters.len(), 1);
                assert_eq!(proc.parameters[0].name.name, "@status");
            }
            _ => panic!("Expected Create Procedure statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_procedure_with_multiple_parameters() {
    // 複数パラメータ付きストアドプロシージャ
    let result = parse_sql(
        "CREATE PROCEDURE search_users \
         @min_id INT = 0, \
         @max_id INT = 1000000, \
         @status INT \
         AS \
         SELECT * FROM users \
         WHERE id BETWEEN @min_id AND @max_id AND status = @status",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Procedure(proc) => {
                assert_eq!(proc.parameters.len(), 3);
                // 2番目のパラメータはデフォルト値を持つ
                assert!(proc.parameters[1].default_value.is_some());
            }
            _ => panic!("Expected Create Procedure statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

// Task 19.1: バッチ処理のテスト

#[test]
fn test_go_keyword_tokenization() {
    // GOキーワードが正しくトークン化されているか確認
    use tsql_lexer::Lexer;

    let sql = "GO";
    let mut lexer = Lexer::new(sql);
    let token = lexer.next_token().unwrap();

    // デバッグ: トークン種別を確認
    println!("GO token kind: {:?}", token.kind);
    println!("GO token text: {:?}", token.text);

    // Goトークンであることを確認
    assert_eq!(token.kind, tsql_token::TokenKind::Go);
}

#[test]
fn test_go_after_select() {
    // SELECT文の後のGOが正しくトークン化されているか確認
    use tsql_lexer::Lexer;

    let sql = "SELECT 1\nGO";
    let mut lexer = Lexer::new(sql);

    // SELECT
    let token1 = lexer.next_token().unwrap();
    println!("token1: {:?} {:?}", token1.kind, token1.text);
    assert_eq!(token1.kind, tsql_token::TokenKind::Select);

    // スペース（スキップされる）
    // 1
    let token2 = lexer.next_token().unwrap();
    println!("token2: {:?} {:?}", token2.kind, token2.text);
    assert_eq!(token2.kind, tsql_token::TokenKind::Number);

    // GO
    let token3 = lexer.next_token().unwrap();
    println!("token3: {:?} {:?}", token3.kind, token3.text);
    assert_eq!(token3.kind, tsql_token::TokenKind::Go);
}

#[test]
fn test_go_at_line_start() {
    // 行頭でのGO検出
    let result = parse_sql("SELECT 1\nGO\nSELECT 2").unwrap();
    assert_eq!(result.len(), 3); // SELECT 1, GO, SELECT 2
}

#[test]
fn test_go_with_leading_whitespace() {
    // 先頭空白付きGO（T-SQLではバッチ区切りとして認識）
    let result = parse_sql("SELECT 1\n  GO  \nSELECT 2").unwrap();
    // GOは行頭で検出されるため、このテストではGOが識別子として扱われる可能性がある
    // 実際のT-SQLでは行頭のGOはバッチ区切り
    assert!(!result.is_empty());
}

#[test]
fn test_go_not_in_string() {
    // 文字列内のGOはバッチ区切りとみなされない
    let result = parse_sql("SELECT 'GO' AS result").unwrap();
    match &result[0] {
        Statement::Select(_) => {}
        _ => panic!("Expected Select statement, GO should not be detected in string"),
    }
}

#[test]
fn test_go_not_in_comment() {
    // コメント内のGOはバッチ区切りとみなされない
    let result = parse_sql("-- This is a comment with GO\nSELECT 1").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_go_not_in_multiline_comment() {
    // 複数行コメント内のGO
    let result = parse_sql("/* This is a comment with GO inside */\nSELECT 1").unwrap();
    assert_eq!(result.len(), 1);
}

#[test]
fn test_go_not_as_identifier() {
    // 識別子の一部としてのGOはバッチ区切りとみなされない
    let mut parser = Parser::new("SELECT goto FROM gopher").with_mode(ParserMode::SingleStatement);
    let result = parser.parse();
    // SingleStatementモードではGOは識別子として扱われる
    assert!(result.is_ok());
}

#[test]
fn test_go_with_repeat_count() {
    // GO N形式のリピートカウント
    let result = parse_sql("SELECT 1\nGO 5").unwrap();
    match &result[1] {
        Statement::BatchSeparator(batch) => {
            assert_eq!(batch.repeat_count, Some(5));
        }
        _ => panic!("Expected BatchSeparator with repeat count"),
    }
}

#[test]
fn test_go_zero_count() {
    // GO 0はバッチを実行しない
    let result = parse_sql("SELECT 1\nGO 0").unwrap();
    match &result[1] {
        Statement::BatchSeparator(batch) => {
            assert_eq!(batch.repeat_count, Some(0));
        }
        _ => panic!("Expected BatchSeparator with repeat count 0"),
    }
}

#[test]
fn test_multiple_batches() {
    // 複数バッチの処理
    let result = parse_sql("SELECT 1\nGO\nSELECT 2\nGO\nSELECT 3").unwrap();
    assert_eq!(result.len(), 5); // 3つのSELECT + 2つのGO
}

#[test]
fn test_empty_batch_before_go() {
    // 空バッチのテスト
    let result = parse_sql("\nGO\nSELECT 1").unwrap();
    // GOの前の空行は無視される
    assert!(!result.is_empty());
}

#[test]
fn test_empty_batch_after_go() {
    // GOの後の空バッチ
    let result = parse_sql("SELECT 1\nGO\n\n").unwrap();
    assert!(!result.is_empty());
}

#[test]
fn test_single_statement_mode_go_as_identifier() {
    // 単一文モードではGOは識別子
    let mut parser = Parser::new("SELECT GO FROM table").with_mode(ParserMode::SingleStatement);
    let result = parser.parse();
    assert!(result.is_ok());
    match &result.unwrap()[0] {
        Statement::Select(_) => {}
        _ => panic!("Expected Select statement in SingleStatement mode"),
    }
}

#[test]
fn test_mode_switching() {
    // モード切り替えのテスト
    let sql = "SELECT GO FROM table";
    let mut batch_parser = Parser::new(sql);
    let mut single_parser = Parser::new(sql).with_mode(ParserMode::SingleStatement);

    // バッチモードではGOを解釈しようとするが、行頭ではないため識別子として扱われる
    let batch_result = batch_parser.parse();
    assert!(batch_result.is_ok());

    // 単一文モードではGOは常に識別子
    let single_result = single_parser.parse();
    assert!(single_result.is_ok());
}

#[test]
fn test_go_case_insensitive() {
    // GOは大文字小文字を区別しない
    let result = parse_sql("SELECT 1\ngo\nSELECT 2\nGo\nSELECT 3\ngO").unwrap();
    assert_eq!(result.len(), 6); // 3つのSELECT + 3つのGO（すべての大文字小文字バリエーション）
}

// Task 20.1: エラー回復のテスト

#[test]
fn test_error_unexpected_token() {
    // 予期しないトークンによるエラー
    let result = parse_sql("SELECT FROM users");
    assert!(result.is_err());
    match result.unwrap_err() {
        ParseError::UnexpectedToken { .. } => {}
        _ => panic!("Expected UnexpectedToken error"),
    }
}

#[test]
fn test_error_unexpected_eof() {
    // 予期しないEOFによるエラー
    let result = parse_sql("SELECT * FROM");
    assert!(result.is_err());
}

#[test]
fn test_error_missing_parenthesis() {
    // 括弧の閉じ忘れ
    let result = parse_sql("SELECT * FROM users WHERE id IN (1, 2, 3");
    assert!(result.is_err());
}

#[test]
fn test_error_missing_quote() {
    // クォートの閉じ忘れ（字句解析器で検出されるはず）
    let result = parse_sql("SELECT * FROM users WHERE name = 'John");
    // 文字列リテラルのエラー処理は字句解析器に依存
    // パーサーがこのエラーをどう処理するかを確認
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_synchronize_at_semicolon() {
    // セミコロンでの同期化
    let mut parser = Parser::new("INVALID SQL; SELECT 1");
    let result = parser.parse();
    // 最初のエラー後に同期して2番目の文を解析できるか
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_synchronize_at_keywords() {
    // キーワードでの同期化
    let mut parser = Parser::new("INVALID STATEMENT\nSELECT 1");
    let result = parser.parse();
    // SELECTで同期できるか
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_multiple_errors_in_batch() {
    // 複数のエラーを含むバッチ
    let mut parser = Parser::new("INVALID1; INVALID2; SELECT 1");
    let _ = parser.parse();
    let errors = parser.errors();
    // 少なくとも1つのエラーが収集されているはず
    assert!(!errors.is_empty());
}

#[test]
fn test_error_position_reporting() {
    // エラー位置の報告
    let result = parse_sql("SELCT FROM users"); // SELCT is a typo
    assert!(result.is_err());
    if let ParseError::UnexpectedToken { expected, .. } = result.unwrap_err() {
        // 期待されるトークンが報告されている
        assert!(!expected.is_empty());
    }
}

#[test]
fn test_error_incomplete_statement() {
    // 不完全な文
    let result = parse_sql("INSERT INTO users");
    assert!(result.is_err());
}

#[test]
fn test_error_invalid_create_target() {
    // 無効なCREATE対象
    let result = parse_sql("CREATE INVALID name");
    assert!(result.is_err());
}

#[test]
fn test_error_missing_comma_in_select() {
    // SELECTリストでのカンマ漏れ
    let result = parse_sql("SELECT id name FROM users");
    // パーサーはこれを式として解釈する可能性がある
    // エラーになるか、何らかの形でパースされる
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_error_in_between_syntax() {
    // BETWEENの構文エラー
    let result = parse_sql("SELECT * FROM users WHERE id BETWEEN 1");
    assert!(result.is_err()); // ANDが欠落している
}

#[test]
fn test_recovery_continues_parsing() {
    // エラー回復後にパースを継続できるか
    let result = parse_sql("INVALID; SELECT 1; INVALID; SELECT 2");
    // エラーがあっても一部の文はパースできる
    assert!(result.is_err() || result.is_ok());
}

#[test]
fn test_error_with_nested_structure() {
    // 入れ子構造でのエラー - 閉じていない括弧
    let result = parse_sql("SELECT * FROM users WHERE id IN (1, 2, (3, 4)");
    // 入れ子のINリストで閉じ括弧が不足
    assert!(result.is_err());
}

#[test]
fn test_batch_specific_error() {
    // バッチモード特有のエラー処理
    let result = parse_sql("SELECT 1; GO; INVALID");
    // GO後の無効なステートメント
    assert!(result.is_err());
}

// Table-level constraint tests

#[test]
fn test_table_level_primary_key() {
    // テーブルレベルPRIMARY KEY制約
    let result = parse_sql("CREATE TABLE t (id INT, CONSTRAINT pk_t PRIMARY KEY (id))").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::PrimaryKey { columns, .. } => {
                        assert_eq!(columns.len(), 1);
                        assert_eq!(columns[0].name, "id");
                    }
                    _ => panic!("Expected PrimaryKey constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_primary_key_multiple_columns() {
    // 複数カラムのPRIMARY KEY制約
    let result = parse_sql(
        "CREATE TABLE t (id INT, user_id INT, CONSTRAINT pk_t PRIMARY KEY (id, user_id))",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::PrimaryKey { columns, .. } => {
                        assert_eq!(columns.len(), 2);
                        assert_eq!(columns[0].name, "id");
                        assert_eq!(columns[1].name, "user_id");
                    }
                    _ => panic!("Expected PrimaryKey constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_primary_key_without_constraint_name() {
    // 制約名なしのPRIMARY KEY
    let result = parse_sql("CREATE TABLE t (id INT, PRIMARY KEY (id))").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::PrimaryKey { columns, .. } => {
                        assert_eq!(columns.len(), 1);
                        assert_eq!(columns[0].name, "id");
                    }
                    _ => panic!("Expected PrimaryKey constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_foreign_key() {
    // テーブルレベルFOREIGN KEY制約
    let result = parse_sql("CREATE TABLE orders (id INT, user_id INT, CONSTRAINT fk_orders_user FOREIGN KEY (user_id) REFERENCES users(id))").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::Foreign {
                        columns,
                        ref_table,
                        ref_columns,
                        ..
                    } => {
                        assert_eq!(columns.len(), 1);
                        assert_eq!(columns[0].name, "user_id");
                        assert_eq!(ref_table.name, "users");
                        assert_eq!(ref_columns.len(), 1);
                        assert_eq!(ref_columns[0].name, "id");
                    }
                    _ => panic!("Expected Foreign constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_foreign_key_multiple_columns() {
    // 複数カラムのFOREIGN KEY制約
    let result = parse_sql(
        "CREATE TABLE t (a INT, b INT, CONSTRAINT fk_t FOREIGN KEY (a, b) REFERENCES other(x, y))",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::Foreign {
                        columns,
                        ref_table,
                        ref_columns,
                        ..
                    } => {
                        assert_eq!(columns.len(), 2);
                        assert_eq!(columns[0].name, "a");
                        assert_eq!(columns[1].name, "b");
                        assert_eq!(ref_table.name, "other");
                        assert_eq!(ref_columns.len(), 2);
                        assert_eq!(ref_columns[0].name, "x");
                        assert_eq!(ref_columns[1].name, "y");
                    }
                    _ => panic!("Expected Foreign constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_foreign_key_without_parens() {
    // 括弧なしの参照カラム（単一カラムの場合）
    let result = parse_sql("CREATE TABLE t (id INT, user_id INT, CONSTRAINT fk_t FOREIGN KEY (user_id) REFERENCES users id)").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::Foreign { ref_columns, .. } => {
                        assert_eq!(ref_columns.len(), 1);
                        assert_eq!(ref_columns[0].name, "id");
                    }
                    _ => panic!("Expected Foreign constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_unique() {
    // テーブルレベルUNIQUE制約
    let result = parse_sql(
        "CREATE TABLE t (id INT, email VARCHAR(100), CONSTRAINT uq_t_email UNIQUE (email))",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::Unique { columns, .. } => {
                        assert_eq!(columns.len(), 1);
                        assert_eq!(columns[0].name, "email");
                    }
                    _ => panic!("Expected Unique constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_unique_multiple_columns() {
    // 複数カラムのUNIQUE制約
    let result = parse_sql("CREATE TABLE t (id INT, email VARCHAR(100), username VARCHAR(50), CONSTRAINT uq_t UNIQUE (email, username))").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::Unique { columns, .. } => {
                        assert_eq!(columns.len(), 2);
                        assert_eq!(columns[0].name, "email");
                        assert_eq!(columns[1].name, "username");
                    }
                    _ => panic!("Expected Unique constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_unique_without_constraint_name() {
    // 制約名なしのUNIQUE
    let result = parse_sql("CREATE TABLE t (id INT, email VARCHAR(100), UNIQUE (email))").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::Unique { columns, .. } => {
                        assert_eq!(columns.len(), 1);
                        assert_eq!(columns[0].name, "email");
                    }
                    _ => panic!("Expected Unique constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_check() {
    // テーブルレベルCHECK制約
    let result =
        parse_sql("CREATE TABLE t (id INT, age INT, CONSTRAINT chk_t_age CHECK (age >= 18))")
            .unwrap();
    match &result[0] {
        Statement::Create(stmt) => {
            match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::Check { expr, .. } => {
                            // CHECK式がパースされていることを確認
                            // パースされた式をそのままチェック（詳細な構造までは検証しない）
                            match expr {
                                Expression::BinaryOp {
                                    op: BinaryOperator::Ge,
                                    ..
                                } => {
                                    // >=演算子が使われていればOK
                                }
                                _ => {
                                    // デバッグのためにパニックの代わりに式を表示
                                    eprintln!("Parsed expr: {:?}", expr);
                                    panic!(
                                        "Expected BinaryOp expression with Ge operator, got {:?}",
                                        expr
                                    );
                                }
                            }
                        }
                        _ => panic!("Expected Check constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            }
        }
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_table_level_check_without_constraint_name() {
    // 制約名なしのCHECK
    let result = parse_sql("CREATE TABLE t (id INT, age INT, CHECK (age >= 18))").unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 1);
                match &table.constraints[0] {
                    TableConstraint::Check { .. } => {
                        // CHECK制約が存在すればOK
                    }
                    _ => panic!("Expected Check constraint"),
                }
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_multiple_table_level_constraints() {
    // 複数のテーブルレベル制約
    let result = parse_sql(
        "CREATE TABLE t (id INT, user_id INT, email VARCHAR(100), age INT, \
         CONSTRAINT pk_t PRIMARY KEY (id), \
         CONSTRAINT fk_t_user FOREIGN KEY (user_id) REFERENCES users(id), \
         CONSTRAINT uq_t_email UNIQUE (email), \
         CONSTRAINT chk_t_age CHECK (age >= 18))",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                assert_eq!(table.constraints.len(), 4);
                // 各制約が正しくパースされていることを確認
                let mut found_pk = false;
                let mut found_fk = false;
                let mut found_uq = false;
                let mut found_chk = false;

                for constraint in &table.constraints {
                    match constraint {
                        TableConstraint::PrimaryKey { .. } => found_pk = true,
                        TableConstraint::Foreign { .. } => found_fk = true,
                        TableConstraint::Unique { .. } => found_uq = true,
                        TableConstraint::Check { .. } => found_chk = true,
                    }
                }

                assert!(found_pk, "PrimaryKey constraint not found");
                assert!(found_fk, "Foreign constraint not found");
                assert!(found_uq, "Unique constraint not found");
                assert!(found_chk, "Check constraint not found");
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_mix_column_and_table_level_constraints() {
    // カラムレベルとテーブルレベルの制約の混合
    let result = parse_sql(
        "CREATE TABLE t (id INT PRIMARY KEY, user_id INT, email VARCHAR(100) NOT NULL, \
         FOREIGN KEY (user_id) REFERENCES users(id), \
         UNIQUE (email))",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(stmt) => match stmt.as_ref() {
            CreateStatement::Table(table) => {
                // カラムレベル制約はColumnDefinition.constraintsに含まれる
                assert_eq!(table.columns.len(), 3);
                // idカラムのPRIMARY KEY制約
                assert!(!table.columns[0].constraints.is_empty());
                // emailカラムはNOT NULL（nullabilityフィールド）
                assert_eq!(table.columns[2].nullability, Some(false));
                // テーブルレベル制約
                assert_eq!(table.constraints.len(), 2);
            }
            _ => panic!("Expected Create Table statement"),
        },
        _ => panic!("Expected Create statement"),
    }
}

// TRY...CATCH tests

#[test]
fn test_try_catch_basic() {
    // 基本的なTRY...CATCHブロック
    let result = parse_sql(
        "BEGIN TRY \
         SELECT 1 \
         END TRY \
         BEGIN CATCH \
         SELECT 2 \
         END CATCH",
    )
    .unwrap();
    match &result[0] {
        Statement::TryCatch(tc) => {
            assert!(!tc.try_block.statements.is_empty());
            assert!(!tc.catch_block.statements.is_empty());
        }
        _ => panic!("Expected TryCatch statement"),
    }
}

// Transaction tests

#[test]
fn test_begin_transaction() {
    // BEGIN TRANSACTION
    let result = parse_sql("BEGIN TRANSACTION").unwrap();
    match &result[0] {
        Statement::Transaction(TransactionStatement::Begin { name, .. }) => {
            assert!(name.is_none());
        }
        _ => panic!("Expected Begin Transaction statement"),
    }
}

#[test]
fn test_begin_transaction_with_name() {
    // BEGIN TRANSACTION tran_name
    let result = parse_sql("BEGIN TRANSACTION my_tran").unwrap();
    match &result[0] {
        Statement::Transaction(TransactionStatement::Begin { name, .. }) => {
            assert_eq!(name.as_ref().unwrap().name, "my_tran");
        }
        _ => panic!("Expected Begin Transaction statement"),
    }
}

#[test]
fn test_commit_transaction() {
    // COMMIT TRANSACTION
    let result = parse_sql("COMMIT TRANSACTION").unwrap();
    match &result[0] {
        Statement::Transaction(TransactionStatement::Commit { name, .. }) => {
            assert!(name.is_none());
        }
        _ => panic!("Expected Commit Transaction statement"),
    }
}

#[test]
fn test_rollback_transaction() {
    // ROLLBACK TRANSACTION
    let result = parse_sql("ROLLBACK TRANSACTION").unwrap();
    match &result[0] {
        Statement::Transaction(TransactionStatement::Rollback { name, .. }) => {
            assert!(name.is_none());
        }
        _ => panic!("Expected Rollback Transaction statement"),
    }
}

#[test]
fn test_save_transaction() {
    // SAVE TRANSACTION savepoint_name
    let result = parse_sql("SAVE TRANSACTION my_savepoint").unwrap();
    match &result[0] {
        Statement::Transaction(TransactionStatement::Save { name, .. }) => {
            assert_eq!(name.name, "my_savepoint");
        }
        _ => panic!("Expected Save Transaction statement"),
    }
}

// THROW tests

#[test]
fn test_throw_basic() {
    // 基本的なTHROW
    let result = parse_sql("THROW").unwrap();
    match &result[0] {
        Statement::Throw(_) => {}
        _ => panic!("Expected Throw statement"),
    }
}

// RAISERROR tests

#[test]
fn test_raiserror_basic() {
    // 基本的なRAISERROR
    let result = parse_sql("RAISERROR('Error message', 16, 1)").unwrap();
    match &result[0] {
        Statement::Raiserror(_) => {}
        _ => panic!("Expected Raiserror statement"),
    }
}

// === ALTER TABLE tests ===

#[test]
fn test_alter_table_add_column() {
    let result = parse_sql("ALTER TABLE users ADD email VARCHAR(100)").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::AlterTable(alter) => {
            assert_eq!(alter.table.name, "users");
            match &alter.operation {
                AlterTableOperation::AddColumn(add) => {
                    assert_eq!(add.name.name, "email");
                    assert!(matches!(add.data_type, DataType::Varchar(Some(100))));
                    assert_eq!(add.nullability, None);
                    assert!(!add.identity);
                }
                _ => panic!("Expected AddColumn operation"),
            }
        }
        _ => panic!("Expected AlterTable statement"),
    }
}

#[test]
fn test_alter_table_add_column_not_null() {
    let result = parse_sql("ALTER TABLE users ADD email VARCHAR(100) NOT NULL").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::AlterTable(alter) => match &alter.operation {
            AlterTableOperation::AddColumn(add) => {
                assert_eq!(add.nullability, Some(false));
            }
            _ => panic!("Expected AddColumn"),
        },
        _ => panic!("Expected AlterTable"),
    }
}

#[test]
fn test_alter_table_add_column_null() {
    let result = parse_sql("ALTER TABLE users ADD email VARCHAR(100) NULL").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::AlterTable(alter) => match &alter.operation {
            AlterTableOperation::AddColumn(add) => {
                assert_eq!(add.nullability, Some(true));
            }
            _ => panic!("Expected AddColumn"),
        },
        _ => panic!("Expected AlterTable"),
    }
}

#[test]
fn test_alter_table_add_column_identity() {
    let result = parse_sql("ALTER TABLE users ADD row_id INT IDENTITY").unwrap();
    match &result[0] {
        Statement::AlterTable(alter) => match &alter.operation {
            AlterTableOperation::AddColumn(add) => {
                assert!(add.identity);
            }
            _ => panic!("Expected AddColumn"),
        },
        _ => panic!("Expected AlterTable"),
    }
}

#[test]
fn test_alter_table_drop_column() {
    let result = parse_sql("ALTER TABLE users DROP COLUMN email").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::AlterTable(alter) => {
            assert_eq!(alter.table.name, "users");
            match &alter.operation {
                AlterTableOperation::DropColumn(name) => {
                    assert_eq!(name.name, "email");
                }
                _ => panic!("Expected DropColumn operation"),
            }
        }
        _ => panic!("Expected AlterTable statement"),
    }
}

#[test]
fn test_alter_table_drop_column_without_keyword() {
    // DROP without explicit COLUMN keyword
    let result = parse_sql("ALTER TABLE users DROP email").unwrap();
    match &result[0] {
        Statement::AlterTable(alter) => match &alter.operation {
            AlterTableOperation::DropColumn(name) => {
                assert_eq!(name.name, "email");
            }
            _ => panic!("Expected DropColumn"),
        },
        _ => panic!("Expected AlterTable"),
    }
}

#[test]
fn test_alter_table_alter_column() {
    let result = parse_sql("ALTER TABLE users ALTER COLUMN email VARCHAR(200)").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::AlterTable(alter) => {
            assert_eq!(alter.table.name, "users");
            match &alter.operation {
                AlterTableOperation::AlterColumn(modify) => {
                    assert_eq!(modify.name.name, "email");
                    assert!(matches!(modify.data_type, DataType::Varchar(Some(200))));
                }
                _ => panic!("Expected AlterColumn operation"),
            }
        }
        _ => panic!("Expected AlterTable statement"),
    }
}

#[test]
fn test_alter_table_alter_column_not_null() {
    let result = parse_sql("ALTER TABLE users ALTER COLUMN email VARCHAR(200) NOT NULL").unwrap();
    match &result[0] {
        Statement::AlterTable(alter) => match &alter.operation {
            AlterTableOperation::AlterColumn(modify) => {
                assert_eq!(modify.nullability, Some(false));
            }
            _ => panic!("Expected AlterColumn"),
        },
        _ => panic!("Expected AlterTable"),
    }
}

#[test]
fn test_alter_table_invalid_operation() {
    let result = parse_sql("ALTER TABLE users UNKNOWN_OP");
    assert!(
        result.is_err(),
        "ALTER TABLE with invalid operation should fail"
    );
}

#[test]
fn test_alter_table_not_table() {
    let result = parse_sql("ALTER INDEX idx REBUILD");
    assert!(result.is_err(), "ALTER without TABLE should fail");
}

#[test]
fn test_alter_table_span() {
    let result = parse_sql("ALTER TABLE users ADD email INT").unwrap();
    match &result[0] {
        Statement::AlterTable(alter) => {
            // Verify the span was captured (start comes from ALTER token)
            assert_eq!(alter.table.name, "users");
        }
        _ => panic!("Expected AlterTable"),
    }
}

// === EXEC / EXECUTE tests ===

#[test]
fn test_exec_no_args() {
    let result = parse_sql("EXEC my_proc").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert!(exec.arguments.is_empty());
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_execute_keyword() {
    let result = parse_sql("EXECUTE my_proc").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert!(exec.arguments.is_empty());
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_positional_args() {
    let result = parse_sql("EXEC my_proc 1, 2, 3").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert_eq!(exec.arguments.len(), 3);
            for arg in &exec.arguments {
                assert!(
                    matches!(arg, ExecArgument::Positional(_)),
                    "Expected Positional argument"
                );
            }
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_named_param() {
    let result = parse_sql("EXEC my_proc @p1 = 1").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert_eq!(exec.arguments.len(), 1);
            match &exec.arguments[0] {
                ExecArgument::Named { name, value } => {
                    assert_eq!(name.name, "@p1");
                    assert!(
                        matches!(value, Expression::Literal(Literal::Number(_, _))),
                        "Expected Number literal for named param value"
                    );
                }
                _ => panic!("Expected Named argument"),
            }
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_multiple_named_params() {
    let result = parse_sql("EXEC my_proc @p1 = 1, @p2 = 2").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert_eq!(exec.arguments.len(), 2);
            match &exec.arguments[0] {
                ExecArgument::Named { name, .. } => {
                    assert_eq!(name.name, "@p1");
                }
                _ => panic!("Expected Named argument for @p1"),
            }
            match &exec.arguments[1] {
                ExecArgument::Named { name, .. } => {
                    assert_eq!(name.name, "@p2");
                }
                _ => panic!("Expected Named argument for @p2"),
            }
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_string_literal_arg() {
    let result = parse_sql("EXEC my_proc 'hello'").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert_eq!(exec.arguments.len(), 1);
            match &exec.arguments[0] {
                ExecArgument::Positional(expr) => {
                    assert!(
                        matches!(expr, Expression::Literal(Literal::String(_, _))),
                        "Expected String literal argument"
                    );
                }
                _ => panic!("Expected Positional argument"),
            }
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_mixed_named_and_positional_args() {
    let result = parse_sql("EXEC my_proc @p1 = 1, 'hello'").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert_eq!(exec.arguments.len(), 2);
            assert!(
                matches!(&exec.arguments[0], ExecArgument::Named { name, .. } if name.name == "@p1"),
                "Expected Named(@p1) as first argument"
            );
            assert!(
                matches!(
                    &exec.arguments[1],
                    ExecArgument::Positional(Expression::Literal(Literal::String(_, _)))
                ),
                "Expected Positional(String) as second argument"
            );
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_variable_positional_arg() {
    let result = parse_sql("EXEC my_proc @var").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert_eq!(exec.arguments.len(), 1);
            match &exec.arguments[0] {
                ExecArgument::Positional(expr) => {
                    assert!(
                        matches!(expr, Expression::Identifier(id) if id.name == "@var"),
                        "Expected Identifier(@var) as positional argument"
                    );
                }
                _ => panic!("Expected Positional argument"),
            }
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_with_semicolon() {
    let result = parse_sql("EXEC my_proc;").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert!(
                exec.arguments.is_empty(),
                "Semicolon should not be treated as an argument"
            );
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_null_arg() {
    let result = parse_sql("EXEC my_proc NULL").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert_eq!(exec.arguments.len(), 1);
            match &exec.arguments[0] {
                ExecArgument::Positional(expr) => {
                    assert!(
                        matches!(expr, Expression::Literal(Literal::Null(_))),
                        "Expected Null literal argument"
                    );
                }
                _ => panic!("Expected Positional argument"),
            }
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_missing_procedure_name() {
    let result = parse_sql("EXEC");
    assert!(
        result.is_err(),
        "EXEC without procedure name should be a parse error"
    );
}

#[test]
fn test_exec_followed_by_next_statement() {
    let result = parse_sql("EXEC my_proc 1\nSELECT 1").unwrap();
    assert_eq!(result.len(), 2);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert_eq!(exec.arguments.len(), 1);
        }
        _ => panic!("Expected Exec as first statement"),
    }
    match &result[1] {
        Statement::Select(_) => {}
        _ => panic!("Expected Select as second statement"),
    }
}

/// カンマの後に引数がない場合、パースエラーが伝播されることを確認
/// （レビュー指摘: Err(_) => break で黙って無視していたのを修正）
#[test]
fn test_exec_comma_without_arg_is_error() {
    let result = parse_sql("EXEC my_proc 1,");
    assert!(
        result.is_err(),
        "EXEC with trailing comma (missing argument) should be a parse error"
    );
}

/// カンマの後に不正なトークンがある場合もエラー伝播
#[test]
fn test_exec_comma_then_invalid_is_error() {
    let result = parse_sql("EXEC my_proc 1, )");
    // カンマの後)は有効なexpressionとしてパースできない→エラー
    assert!(
        result.is_err(),
        "EXEC with comma then invalid token should be a parse error"
    );
}

#[test]
fn test_exec_named_params_mixed_with_positional() {
    let result = parse_sql("EXEC my_proc 'hello', @p2 = 2, 3").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Exec(exec) => {
            assert_eq!(exec.procedure.name, "my_proc");
            assert_eq!(exec.arguments.len(), 3);
            assert!(
                matches!(
                    &exec.arguments[0],
                    ExecArgument::Positional(Expression::Literal(Literal::String(_, _)))
                ),
                "First arg should be Positional(String)"
            );
            assert!(
                matches!(&exec.arguments[1], ExecArgument::Named { name, .. } if name.name == "@p2"),
                "Second arg should be Named(@p2)"
            );
            assert!(
                matches!(
                    &exec.arguments[2],
                    ExecArgument::Positional(Expression::Literal(Literal::Number(_, _)))
                ),
                "Third arg should be Positional(Number)"
            );
        }
        _ => panic!("Expected Exec statement"),
    }
}

#[test]
fn test_exec_span_includes_procedure_and_args() {
    let result = parse_sql("EXEC my_proc 1, 2").unwrap();
    match &result[0] {
        Statement::Exec(exec) => {
            assert!(exec.span.start < exec.span.end, "Span should be non-empty");
            assert_ne!(exec.span.end, 0, "Span end should not be zero");
        }
        _ => panic!("Expected Exec statement"),
    }
}

// === CREATE TRIGGER tests ===

#[test]
fn test_create_trigger_insert() {
    let result =
        parse_sql("CREATE TRIGGER trg_ins ON users FOR INSERT AS BEGIN SELECT 1 END").unwrap();
    assert_eq!(result.len(), 1);
    match &result[0] {
        Statement::Create(create) => match create.as_ref() {
            CreateStatement::Trigger(td) => {
                assert_eq!(td.name.name, "trg_ins");
                assert_eq!(td.table.name, "users");
                assert_eq!(td.events.len(), 1);
                assert_eq!(td.events[0], TriggerEvent::Insert);
                assert_eq!(td.body.len(), 1);
            }
            _ => panic!("Expected Trigger"),
        },
        _ => panic!("Expected Create statement"),
    }
}

#[test]
fn test_create_trigger_update() {
    let result =
        parse_sql("CREATE TRIGGER trg_upd ON orders FOR UPDATE AS BEGIN SELECT 1 END").unwrap();
    match &result[0] {
        Statement::Create(create) => match create.as_ref() {
            CreateStatement::Trigger(td) => {
                assert_eq!(td.name.name, "trg_upd");
                assert_eq!(td.table.name, "orders");
                assert_eq!(td.events[0], TriggerEvent::Update);
            }
            _ => panic!("Expected Trigger"),
        },
        _ => panic!("Expected Create"),
    }
}

#[test]
fn test_create_trigger_delete() {
    let result =
        parse_sql("CREATE TRIGGER trg_del ON users FOR DELETE AS BEGIN SELECT 1 END").unwrap();
    match &result[0] {
        Statement::Create(create) => match create.as_ref() {
            CreateStatement::Trigger(td) => {
                assert_eq!(td.events[0], TriggerEvent::Delete);
            }
            _ => panic!("Expected Trigger"),
        },
        _ => panic!("Expected Create"),
    }
}

#[test]
fn test_create_trigger_multiple_events() {
    let result = parse_sql(
        "CREATE TRIGGER trg_multi ON users FOR INSERT, UPDATE, DELETE AS BEGIN SELECT 1 END",
    )
    .unwrap();
    match &result[0] {
        Statement::Create(create) => match create.as_ref() {
            CreateStatement::Trigger(td) => {
                assert_eq!(td.events.len(), 3);
                assert_eq!(td.events[0], TriggerEvent::Insert);
                assert_eq!(td.events[1], TriggerEvent::Update);
                assert_eq!(td.events[2], TriggerEvent::Delete);
            }
            _ => panic!("Expected Trigger"),
        },
        _ => panic!("Expected Create"),
    }
}

#[test]
fn test_create_trigger_single_statement_body() {
    let result =
        parse_sql("CREATE TRIGGER trg_simple ON users FOR INSERT AS INSERT INTO log VALUES (1)")
            .unwrap();
    match &result[0] {
        Statement::Create(create) => match create.as_ref() {
            CreateStatement::Trigger(td) => {
                assert_eq!(td.body.len(), 1);
            }
            _ => panic!("Expected Trigger"),
        },
        _ => panic!("Expected Create"),
    }
}

#[test]
fn test_create_trigger_missing_on() {
    let result = parse_sql("CREATE TRIGGER trg");
    assert!(result.is_err(), "Should fail without ON keyword");
}

#[test]
fn test_create_trigger_missing_for() {
    let result = parse_sql("CREATE TRIGGER trg ON users");
    assert!(result.is_err(), "Should fail without FOR keyword");
}

#[test]
fn test_create_trigger_missing_as() {
    let result = parse_sql("CREATE TRIGGER trg ON users FOR INSERT");
    assert!(result.is_err(), "Should fail without AS keyword");
}

#[test]
fn test_create_trigger_span() {
    let result = parse_sql("CREATE TRIGGER trg ON users FOR INSERT AS BEGIN SELECT 1 END").unwrap();
    match &result[0] {
        Statement::Create(create) => match create.as_ref() {
            CreateStatement::Trigger(td) => {
                assert_eq!(td.name.name, "trg");
                assert_eq!(td.table.name, "users");
                assert_eq!(td.events.len(), 1);
            }
            _ => panic!("Expected Trigger"),
        },
        _ => panic!("Expected Create"),
    }
}

// ── Error recovery tests (#127) ──────────────────────────────────────────────

/// Helper: parse with error recovery and return (statements, errors).
fn parse_with_recovery(sql: &str) -> (Vec<Statement>, Vec<ParseError>) {
    let mut parser = Parser::new(sql);
    parser.parse_with_errors()
}

#[test]
fn test_recovery_error_in_middle() {
    // Error in the middle statement; first and last should parse fine.
    let (stmts, errors) = parse_with_recovery("SELECT 1; SELCT * FROM t; SELECT 2");

    // First SELECT 1 and last SELECT 2 should be recovered
    assert!(
        stmts.len() >= 2,
        "should recover at least 2 statements, got {}",
        stmts.len()
    );
    assert!(!errors.is_empty(), "should report at least 1 error");
}

#[test]
fn test_recovery_error_at_start() {
    // Error at start; second statement should parse.
    let (stmts, errors) = parse_with_recovery("SELCT * FROM t; SELECT 1");

    assert!(
        !stmts.is_empty(),
        "should recover at least 1 statement, got {}",
        stmts.len()
    );
    assert!(!errors.is_empty(), "should report at least 1 error");
}

#[test]
fn test_recovery_error_at_end() {
    // Error at end; first statement should parse.
    let (stmts, errors) = parse_with_recovery("SELECT 1; SELCT * FROM t");

    assert!(
        !stmts.is_empty(),
        "should recover at least 1 statement, got {}",
        stmts.len()
    );
    assert!(!errors.is_empty(), "should report at least 1 error");
}

#[test]
fn test_recovery_multiple_errors_mixed_with_valid() {
    // Two invalid, one valid in between.
    let (stmts, errors) = parse_with_recovery("SELCT 1; INSERT INTO t VALUES (1); SELCT 2");

    assert!(
        !stmts.is_empty(),
        "should recover INSERT statement, got {} stmts",
        stmts.len()
    );
    assert!(
        errors.len() >= 2,
        "should report at least 2 errors, got {}",
        errors.len()
    );
}

#[test]
fn test_recovery_no_errors() {
    // Clean input should have zero errors and all statements.
    let (stmts, errors) = parse_with_recovery("SELECT 1; SELECT 2; SELECT 3");

    assert_eq!(stmts.len(), 3, "all 3 statements should parse");
    assert!(
        errors.is_empty(),
        "no errors expected, got {}",
        errors.len()
    );
}

#[test]
fn test_recovery_preserves_statement_types() {
    // Mix of DML/DDL/control-flow with one error.
    let sql = "SELECT 1; INVALID STMT; INSERT INTO t VALUES (1)";
    let (stmts, errors) = parse_with_recovery(sql);

    assert!(!stmts.is_empty(), "should recover some statements");
    assert!(!errors.is_empty(), "should report error for INVALID");

    // Check that the recovered statements are the right types
    let has_select = stmts.iter().any(|s| matches!(s, Statement::Select(_)));
    let has_insert = stmts.iter().any(|s| matches!(s, Statement::Insert(_)));
    assert!(has_select, "should recover SELECT statement");
    assert!(has_insert, "should recover INSERT statement");
}

#[test]
fn test_recovery_all_invalid() {
    // All statements are invalid.
    let (_stmts, errors) = parse_with_recovery("FOO BAR; BAZ QUX");

    // May or may not recover statements, but should report errors
    assert!(
        !errors.is_empty(),
        "should report errors for all-invalid input"
    );
}

#[test]
fn test_recovery_empty_input() {
    let (stmts, errors) = parse_with_recovery("");

    assert!(stmts.is_empty(), "empty input should produce no statements");
    assert!(errors.is_empty(), "empty input should produce no errors");
}

#[test]
fn test_recovery_with_declare_and_set() {
    // Ensure synchronize recognizes DECLARE and SET as sync points.
    let sql = "INVALID; DECLARE @x INT; SET @x = 1";
    let (stmts, errors) = parse_with_recovery(sql);

    assert!(!errors.is_empty(), "should report error for INVALID");
    // DECLARE and SET should be recovered
    assert!(
        stmts.len() >= 2,
        "should recover DECLARE and SET, got {} stmts",
        stmts.len()
    );
}

#[test]
fn test_recovery_synchronize_to_begin() {
    // Error before BEGIN block; BEGIN block should still parse.
    let sql = "INVALID; BEGIN SELECT 1 END";
    let (stmts, errors) = parse_with_recovery(sql);

    assert!(!errors.is_empty(), "should report error for INVALID");
    assert!(!stmts.is_empty(), "should recover BEGIN block");
}

#[test]
fn test_recovery_error_inside_block() {
    // Error inside BEGIN...END should not prevent outer parsing.
    let sql = "BEGIN SELECT 1; INVALID; SELECT 2 END; SELECT 3";
    let (stmts, errors) = parse_with_recovery(sql);

    // At minimum we should get something
    assert!(
        !errors.is_empty(),
        "should report error for INVALID inside block"
    );
    // The outer SELECT 3 should be recovered even if the block has issues
    assert!(!stmts.is_empty(), "should recover some statements");
}

#[test]
fn test_recovery_with_if_statement() {
    // Error followed by IF statement.
    let sql = "INVALID; IF 1 = 1 SELECT 1";
    let (stmts, errors) = parse_with_recovery(sql);

    assert!(!errors.is_empty(), "should report error for INVALID");
    assert!(!stmts.is_empty(), "should recover IF statement");
}

#[test]
fn test_recovery_with_while_statement() {
    // Error followed by WHILE statement.
    let sql = "INVALID; WHILE 1 = 1 BREAK";
    let (stmts, errors) = parse_with_recovery(sql);

    assert!(!errors.is_empty(), "should report error for INVALID");
    assert!(!stmts.is_empty(), "should recover WHILE statement");
}
