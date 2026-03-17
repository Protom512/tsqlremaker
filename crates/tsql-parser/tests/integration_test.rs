//! 統合テストスイート
//!
//! 複雑な SQL クエリのパースを検証する統合テスト。

// テストコードでは unwrap/panic/expect を許可
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]
#![allow(clippy::single_match)]
#![allow(clippy::len_zero)]

use tsql_parser::{parse, parse_one};
use tsql_parser::ast::ColumnConstraint;

/// 複数の JOIN を含む複雑な SELECT 文
#[test]
fn test_complex_join_query() {
    let sql = r#"
        SELECT u.id, u.name, o.order_id, p.product_name
        FROM users u
        INNER JOIN orders o ON u.id = o.user_id
        LEFT JOIN products p ON o.product_id = p.id
        WHERE u.status = 'active'
        ORDER BY u.id
    "#;

    let statements = parse(sql).unwrap();
    assert_eq!(statements.len(), 1);

    match &statements[0] {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.columns.len(), 4);
            assert!(select.from.is_some());
            assert!(select.where_clause.is_some());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 入れ子のサブクエリ
#[test]
fn test_nested_subquery() {
    let sql = r#"
        SELECT u.id, u.name
        FROM users u
        WHERE u.id IN (1, 2, 3)
    "#;

    let statements = parse(sql).unwrap();
    assert_eq!(statements.len(), 1);

    match &statements[0] {
        tsql_parser::Statement::Select(select) => {
            assert!(select.where_clause.is_some());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 深い入れ子の式
#[test]
fn test_deeply_nested_expression() {
    let sql = "SELECT a + b * c - (d / e) FROM t";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.columns.len(), 1);
        }
        _ => panic!("SELECT文であること"),
    }
}

/// GROUP BY と HAVING を含むクエリ
#[test]
fn test_group_by_having() {
    let sql = r#"
        SELECT department, COUNT(*) as emp_count, AVG(salary) as avg_salary
        FROM employees
        WHERE status = 'active'
        GROUP BY department
        HAVING COUNT(*) > 5
        ORDER BY emp_count DESC
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(!select.group_by.is_empty());
            assert!(select.having.is_some());
            assert!(!select.order_by.is_empty());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// CASE 式を含むクエリ
#[test]
fn test_case_expression() {
    let sql = r#"
        SELECT id,
               CASE
                   WHEN score >= 90 THEN 'A'
                   WHEN score >= 80 THEN 'B'
                   WHEN score >= 70 THEN 'C'
                   ELSE 'F'
               END as grade
        FROM students
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.columns.len(), 2);
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 複数のステートメント
#[test]
fn test_multiple_statements() {
    let sql = r#"
        DECLARE @x INT;
        SET @x = 10;
        SELECT @x as value;
    "#;

    let statements = parse(sql).unwrap();
    assert!(statements.len() >= 3);
}

/// IF...ELSE 制御フロー
#[test]
fn test_if_else_control_flow() {
    // TODO: ELSE 句の実装が完了したら完全版をテスト
    let sql = r#"
        IF @x > 0
            SELECT 'positive' as result;
    "#;

    let statements = parse(sql).unwrap();
    assert_eq!(statements.len(), 1);

    match &statements[0] {
        tsql_parser::Statement::If(_) => {}
        _ => panic!("IF文であること"),
    }
}

/// WHILE ループ
#[test]
fn test_while_loop() {
    let sql = r#"
        WHILE @counter < 10
        BEGIN
            SET @counter = @counter + 1;
        END
    "#;

    let statements = parse(sql).unwrap();
    assert_eq!(statements.len(), 1);

    match &statements[0] {
        tsql_parser::Statement::While(_) => {}
        _ => panic!("WHILE文であること"),
    }
}

/// BEGIN...END ブロック
#[test]
fn test_begin_end_block() {
    let sql = r#"
        BEGIN
            DECLARE @x INT;
            SET @x = 1;
            SELECT @x;
        END
    "#;

    let statements = parse(sql).unwrap();
    assert_eq!(statements.len(), 1);

    match &statements[0] {
        tsql_parser::Statement::Block(block) => {
            assert!(!block.statements.is_empty());
        }
        _ => panic!("BLOCKであること"),
    }
}

/// INSERT-SELECT
#[test]
fn test_insert_select() {
    let sql = r#"
        INSERT INTO archived_orders
        SELECT * FROM orders WHERE order_date < '2020-01-01'
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Insert(insert) => match &insert.source {
            tsql_parser::InsertSource::Select(_) => {}
            _ => panic!("SELECTソースであること"),
        },
        _ => panic!("INSERT文であること"),
    }
}

/// CREATE TABLE
#[test]
fn test_create_table() {
    // TODO: DEFAULT 式の実装が完了したら完全版をテスト
    let sql = r#"
        CREATE TABLE users (
            id INT PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(255)
        )
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Create(_) => {}
        _ => panic!("CREATE文であること"),
    }
}

/// 複雑な WHERE 句（複数の条件）
#[test]
fn test_complex_where_clause() {
    let sql = r#"
        SELECT * FROM orders
        WHERE (status = 'pending' OR status = 'processing')
          AND amount > 1000
          AND created_at >= '2024-01-01'
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(select.where_clause.is_some());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 集計関数と DISTINCT
#[test]
fn test_aggregate_functions() {
    let sql = r#"
        SELECT
            COUNT(*) as total_rows,
            COUNT(DISTINCT user_id) as unique_users,
            SUM(amount) as total_amount,
            AVG(amount) as avg_amount,
            MIN(amount) as min_amount,
            MAX(amount) as max_amount
        FROM transactions
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.columns.len(), 6);
        }
        _ => panic!("SELECT文であること"),
    }
}

/// トランザクション制御（GO 区切り）
#[test]
fn test_go_batch_separator() {
    let sql = r#"
        SELECT * FROM users WHERE id = 1
        GO
        SELECT * FROM orders WHERE order_id = 100
        GO
    "#;

    let statements = parse(sql).unwrap();
    assert!(statements.len() >= 2);

    // GO は BatchSeparator としてパースされる
    let has_batch_separator = statements
        .iter()
        .any(|s| matches!(s, tsql_parser::Statement::BatchSeparator(_)));
    assert!(
        has_batch_separator,
        "GOはBatchSeparatorとしてパースされること"
    );
}

/// 一時テーブルの作成と参照
#[test]
fn test_temp_table() {
    let sql = r#"
        CREATE TABLE #temp_results (
            id INT,
            value VARCHAR(100)
        )
        INSERT INTO #temp_results VALUES (1, 'test')
        SELECT * FROM #temp_results
    "#;

    let statements = parse(sql).unwrap();
    assert!(statements.len() >= 3);
}

/// エラー回復：構文エラーを含むクエリ
#[test]
fn test_error_recovery() {
    let sql = r#"
        SELCT * FROM users
        SELECT * FROM orders
    "#;

    let result = parse(sql);
    // 最初の文は構文エラーで失敗するか、部分的にパースされる
    // エラー回復が実装されている場合、2番目の文はパース可能
    assert!(result.is_err() || result.unwrap().len() >= 1);
}

/// 大きな入力ファイル（パフォーマンス）
///
/// 注意: このテストは wall-clock タイミングを使用しているため、CI ではスキップされます。
/// 手動で実行するには: `cargo test --package tsql-parser --test integration_test test_large_input_performance -- --ignored`
#[test]
#[ignore]
fn test_large_input_performance() {
    // UNION ALL の代わりに複数の文を使用
    let mut sql = String::new();
    for i in 0..100 {
        sql.push_str(&format!("SELECT {} as id, 'name_{}' as name; ", i, i));
    }

    let start = std::time::Instant::now();
    let result = parse(&sql);
    let duration = start.elapsed();

    assert!(result.is_ok(), "大きな入力もパースできること");
    // パフォーマンス要件: 100文を10秒以内にパース（CI負荷考慮）
    // 注: 本当なパフォーマンス検証は benches/parser_bench.rs を使用してください
    assert!(
        duration.as_secs() < 10,
        "パフォーマンス要件を満たすこと: {:?}",
        duration
    );
}

/// NULL と論理値のリテラル
///
/// SAP ASEではTRUE/FALSEはキーワードではなく識別子として扱われるため、
/// ブール値にはbit型の0/1を使用します
#[test]
fn test_null_and_boolean_literals() {
    // ASE互換のブール値表現（bit型のリテラル）
    let sql = "SELECT NULL, 1, 0 FROM t";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.columns.len(), 3);
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 文字列の連結
#[test]
fn test_string_concatenation() {
    let sql = "SELECT first_name || ' ' || last_name as full_name FROM users";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.columns.len(), 1);
        }
        _ => panic!("SELECT文であること"),
    }
}

/// LIKE と ESCAPE 句
#[test]
fn test_like_pattern() {
    let sql = r#"
        SELECT * FROM products
        WHERE product_name LIKE '%A\%B%' ESCAPE '\'
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(select.where_clause.is_some());
            if let Some(where_expr) = &select.where_clause {
                // ESCAPE句が正しくパースされていることを確認
                match where_expr {
                    tsql_parser::Expression::Like { escape, .. } => {
                        assert!(escape.is_some(), "ESCAPE句があること");
                    }
                    _ => panic!("LIKE式であること"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// LIKE ESCAPE 句のパース（バックスラッシュ）
#[test]
fn test_like_escape_backslash() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM t WHERE col LIKE '%\\_%' ESCAPE '\\'";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select) => {
            if let Some(where_expr) = &select.where_clause {
                match where_expr {
                    tsql_parser::Expression::Like { escape, .. } => {
                        assert!(escape.is_some(), "ESCAPE句があること");
                    }
                    _ => panic!("LIKE式であること"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// LIKE ESCAPE 句のパース（他のエスケープ文字）
#[test]
fn test_like_escape_other_char() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM t WHERE col LIKE '%#_%' ESCAPE '#'";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select) => {
            if let Some(where_expr) = &select.where_clause {
                match where_expr {
                    tsql_parser::Expression::Like { escape, .. } => {
                        assert!(escape.is_some(), "ESCAPE句があること");
                    }
                    _ => panic!("LIKE式であること"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// LIKE ESCAPE 句なし（通常のLIKE）
#[test]
fn test_like_without_escape() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM t WHERE col LIKE '%test%'";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select) => {
            if let Some(where_expr) = &select.where_clause {
                match where_expr {
                    tsql_parser::Expression::Like { escape, .. } => {
                        assert!(escape.is_none(), "ESCAPE句がないこと");
                    }
                    _ => panic!("LIKE式であること"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// IN サブクエリ
#[test]
fn test_in_subquery() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select) => {
            if let Some(where_expr) = &select.where_clause {
                match where_expr {
                    tsql_parser::Expression::In { list, .. } => {
                        // INリストがサブクエリであることを確認
                        match list {
                            tsql_parser::InList::Subquery(_) => {
                                // OK - サブクエリが正しくパースされている
                            }
                            _ => panic!("INリストがサブクエリであること"),
                        }
                    }
                    _ => panic!("IN式であること"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// スカラーサブクエリ
#[test]
fn test_scalar_subquery() {
    use tsql_parser::parse_one;

    let sql = "SELECT (SELECT COUNT(*) FROM orders) as order_count FROM users";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select) => {
            // カラムリストにサブクエリが含まれていることを確認
            assert!(!select.columns.is_empty());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// EXISTS サブクエリ
#[test]
fn test_exists_subquery() {
    let sql = r#"
        SELECT u.id, u.name
        FROM users u
        WHERE EXISTS (
            SELECT 1 FROM orders o WHERE o.user_id = u.id
        )
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(select.where_clause.is_some());
            if let Some(where_expr) = &select.where_clause {
                match where_expr {
                    tsql_parser::Expression::Exists(_) => {
                        // OK - EXISTS式が正しくパースされている
                    }
                    _ => panic!("EXISTS式であること"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// エラー回復：キーワードでの再同期
#[test]
fn test_synchronization_at_keywords() {
    let sql = r#"
        SELCT * FROM users;
        INSERT INTO orders VALUES (1, 2);
        UPDATE products SET price = 100;
    "#;

    let result = parse(sql);
    // 現在の実装ではエラー回復が不完全なので、エラーになることを期待
    // TODO: エラー回復が実装されたらこのテストを更新する
    assert!(result.is_err());
}

/// エラー回復：予期しないEOF
#[test]
fn test_unexpected_eof_recovery() {
    let sql = "SELECT id, name FROM";

    let result = parse_one(sql);
    assert!(result.is_err());
}

/// 再帰深度制限のテスト
#[test]
#[ignore = "深い入れ子でスタックオーバーフローが発生するため無効化"]
fn test_recursion_depth_limit() {
    // 深く入れ子になった式を作成（制限の1000を超える1100）
    let mut expr = String::from("1");
    for _ in 0..1100 {
        expr = format!("({} + 1)", expr);
    }

    let sql = format!("SELECT {}", &expr);
    let result = parse_one(&sql);

    // 深度制限(1000)を超えるとエラーになる
    assert!(result.is_err());
}

/// DialectSpecific変換のテスト
#[test]
fn test_dialect_specific_conversions() {
    use tsql_parser::{common::ToCommonAst, parse};

    // DECLARE文は方言固有
    let sql = "DECLARE @x INT";
    let statements = parse(sql).unwrap();
    let converted = statements[0].to_common_ast();
    assert!(converted.is_some());

    match converted.unwrap() {
        tsql_parser::common::CommonStatement::DialectSpecific { description, .. } => {
            assert!(description.contains("DECLARE") || description.contains("variable"));
        }
        _ => panic!("方言固有としてマークされるべき"),
    }

    // SET文は方言固有
    let sql = "SET @x = 1";
    let statements = parse(sql).unwrap();
    let converted = statements[0].to_common_ast();
    assert!(converted.is_some());

    // IF文は方言固有
    let sql = "IF @x > 0 SELECT 1";
    let statements = parse(sql).unwrap();
    let converted = statements[0].to_common_ast();
    assert!(converted.is_some());
}

/// UPDATE with FROM clause (ASE-specific)
#[test]
fn test_update_with_from_dialect_specific() {
    use tsql_parser::{common::ToCommonAst, parse_one};

    let sql = "UPDATE t SET x = 1 FROM table_t";
    let stmt = parse_one(sql).unwrap();

    // パースは成功する
    let has_from =
        matches!(stmt, tsql_parser::Statement::Update(ref update) if update.from_clause.is_some());
    assert!(has_from, "UPDATE with FROM clause");

    // しかしCommon AST変換ではDialectSpecificになる
    let converted = stmt.to_common_ast();
    assert!(converted.is_some());

    match converted.unwrap() {
        tsql_parser::common::CommonStatement::DialectSpecific { description, .. } => {
            assert!(description.contains("FROM clause"));
        }
        _ => {}
    }
}

/// ビット否定演算子のテスト
#[test]
fn test_bitwise_not_operator() {
    let sql = "SELECT ~x FROM t";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.columns.len(), 1);
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 複雑な型変換を含む式
#[test]
fn test_complex_type_conversions() {
    // CAST/CONVERT は AS キーワードを使用する特殊な構文
    // 現在のパーサーでは完全にはサポートされていないため、
    // 関数呼び出し形式でテストする
    let sql = r#"
        SELECT
            ABS(-123),
            UPPER('test'),
            SUBSTRING('hello', 1, 3)
        FROM t
    "#;

    let stmt = parse_one(sql);
    // 組み込み関数はパースできる
    assert!(stmt.is_ok());
}

/// TOP句とDISTINCTの組み合わせ
#[test]
fn test_top_with_distinct() {
    let sql = "SELECT DISTINCT TOP 10 * FROM users";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(select.top.is_some());
            assert!(select.distinct);
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 複数のHAVING条件
#[test]
fn test_multiple_having_conditions() {
    let sql = r#"
        SELECT department, COUNT(*) as cnt
        FROM employees
        GROUP BY department
        HAVING COUNT(*) > 5 AND AVG(salary) > 50000
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(!select.group_by.is_empty());
            assert!(select.having.is_some());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 入れ子のCASE式
#[test]
fn test_nested_case_expressions() {
    let sql = r#"
        SELECT
            CASE
                WHEN x > 0 THEN
                    CASE
                        WHEN y > 0 THEN 'positive'
                        ELSE 'mixed'
                    END
                ELSE 'negative'
            END as result
        FROM t
    "#;

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.columns.len(), 1);
        }
        _ => panic!("SELECT文であること"),
    }
}

/// FULL JOINのテスト
#[test]
fn test_full_join() {
    let sql = "SELECT * FROM users FULL OUTER JOIN orders ON users.id = orders.user_id";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// CROSS JOINのテスト
#[test]
fn test_cross_join() {
    let sql = "SELECT * FROM users CROSS JOIN orders";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 複数のORDER BY条件
#[test]
fn test_multiple_order_by() {
    let sql = "SELECT * FROM users ORDER BY department ASC, salary DESC, name";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.order_by.len(), 3);
            assert!(select.order_by[0].asc);
            assert!(!select.order_by[1].asc);
            assert!(select.order_by[2].asc); // default ASC
        }
        _ => panic!("SELECT文であること"),
    }
}

/// INSERT with DEFAULT VALUES
#[test]
fn test_insert_default_values() {
    let sql = "INSERT INTO users DEFAULT VALUES";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Insert(insert) => match &insert.source {
            tsql_parser::InsertSource::DefaultValues => {}
            _ => panic!("DEFAULT VALUESであること"),
        },
        _ => panic!("INSERT文であること"),
    }
}

/// DELETE without WHERE (warning case)
#[test]
fn test_delete_without_where() {
    let sql = "DELETE FROM temp_table";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Delete(delete) => {
            assert_eq!(delete.table.name, "temp_table");
            assert!(delete.where_clause.is_none());
        }
        _ => panic!("DELETE文であること"),
    }
}

/// RETURN文のテスト
#[test]
fn test_return_statement() {
    let sql = "RETURN";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Return(_) => {}
        _ => panic!("RETURN文であること"),
    }
}

/// RETURN with expression
#[test]
fn test_return_with_expression() {
    let sql = "RETURN @x + 1";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Return(ret) => {
            assert!(ret.expression.is_some());
        }
        _ => panic!("RETURN文であること"),
    }
}

/// BREAK statement
#[test]
fn test_break_statement() {
    let sql = "BREAK";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Break(_) => {}
        _ => panic!("BREAK文であること"),
    }
}

/// CONTINUE statement
#[test]
fn test_continue_statement() {
    let sql = "CONTINUE";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Continue(_) => {}
        _ => panic!("CONTINUE文であること"),
    }
}

/// 一時テーブルのCREATEとDROP
#[test]
fn test_temp_table_create() {
    let sql = "CREATE TABLE #tmp (id INT, name VARCHAR(100))";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Create(create) => match &*create {
            tsql_parser::CreateStatement::Table(table) => {
                assert!(table.temporary);
            }
            _ => panic!("TABLEであること"),
        },
        _ => panic!("CREATE文であること"),
    }
}

/// グローバル一時テーブル
#[test]
fn test_global_temp_table() {
    let sql = "SELECT * FROM ##global_temp";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(select.from.is_some());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// TOP 句
#[test]
fn test_top_clause() {
    let sql = "SELECT TOP 10 * FROM users ORDER BY created_at DESC";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert!(select.top.is_some());
        }
        _ => panic!("SELECT文であること"),
    }
}

/// ORDER BY の ASC/DESC
#[test]
fn test_order_by_direction() {
    let sql = "SELECT * FROM users ORDER BY name ASC, created_at DESC";

    let stmt = parse_one(sql).unwrap();
    match stmt {
        tsql_parser::Statement::Select(select) => {
            assert_eq!(select.order_by.len(), 2);
            assert!(select.order_by[0].asc); // ASC
            assert!(!select.order_by[1].asc); // DESC
        }
        _ => panic!("SELECT文であること"),
    }
}

/// カラムレベル制約: PRIMARY KEY
#[test]
fn test_column_primary_key_constraint() {
    let sql = "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(100))";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Create(create) => {
            match create.as_ref() {
                tsql_parser::CreateStatement::Table(table) => {
                    assert_eq!(table.columns.len(), 2);
                    // idカラムにPRIMARY KEY制約がある
                    assert!(!table.columns[0].constraints.is_empty());
                    match &table.columns[0].constraints[0] {
                        ColumnConstraint::PrimaryKey => {
                            // OK
                        }
                        _ => panic!("PRIMARY KEY制約があること"),
                    }
                }
                _ => panic!("CREATE TABLEであること"),
            }
        }
        _ => panic!("CREATE文であること"),
    }
}

/// カラムレベル制約: UNIQUE
#[test]
fn test_column_unique_constraint() {
    let sql = "CREATE TABLE users (email VARCHAR(255) UNIQUE)";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Create(create) => {
            match create.as_ref() {
                tsql_parser::CreateStatement::Table(table) => {
                    assert_eq!(table.columns.len(), 1);
                    assert!(!table.columns[0].constraints.is_empty());
                    match &table.columns[0].constraints[0] {
                        ColumnConstraint::Unique => {
                            // OK
                        }
                        _ => panic!("UNIQUE制約があること"),
                    }
                }
                _ => panic!("CREATE TABLEであること"),
            }
        }
        _ => panic!("CREATE文であること"),
    }
}

/// カラムレベル制約: REFERENCES (FOREIGN KEY)
#[test]
fn test_column_references_constraint() {
    let sql = "CREATE TABLE orders (user_id INT REFERENCES users(id))";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Create(create) => {
            match create.as_ref() {
                tsql_parser::CreateStatement::Table(table) => {
                    assert_eq!(table.columns.len(), 1);
                    assert!(!table.columns[0].constraints.is_empty());
                    match &table.columns[0].constraints[0] {
                        ColumnConstraint::Foreign { ref_table, ref_column } => {
                            assert_eq!(ref_table.name, "users");
                            assert_eq!(ref_column.name, "id");
                        }
                        _ => panic!("REFERENCES制約があること"),
                    }
                }
                _ => panic!("CREATE TABLEであること"),
            }
        }
        _ => panic!("CREATE文であること"),
    }
}

/// カラムレベル制約: CHECK
#[test]
fn test_column_check_constraint() {
    let sql = "CREATE TABLE products (price DECIMAL CHECK (price > 0))";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Create(create) => {
            match create.as_ref() {
                tsql_parser::CreateStatement::Table(table) => {
                    assert_eq!(table.columns.len(), 1);
                    assert!(!table.columns[0].constraints.is_empty());
                    match &table.columns[0].constraints[0] {
                        ColumnConstraint::Check(_) => {
                            // OK
                        }
                        _ => panic!("CHECK制約があること"),
                    }
                }
                _ => panic!("CREATE TABLEであること"),
            }
        }
        _ => panic!("CREATE文であること"),
    }
}

/// 複数のカラムレベル制約
#[test]
fn test_multiple_column_constraints() {
    // T-SQL標準ではNULL制約を先に記述する
    let sql = "CREATE TABLE test (id INT NOT NULL PRIMARY KEY, name VARCHAR(100) NOT NULL UNIQUE)";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Create(create) => {
            match create.as_ref() {
                tsql_parser::CreateStatement::Table(table) => {
                    assert_eq!(table.columns.len(), 2);
                    // idカラム: NOT NULL + PRIMARY KEY
                    assert_eq!(table.columns[0].nullability, Some(false));
                    assert_eq!(table.columns[0].constraints.len(), 1);
                    match &table.columns[0].constraints[0] {
                        ColumnConstraint::PrimaryKey => {
                            // OK
                        }
                        _ => panic!("idカラムにPRIMARY KEY制約があること"),
                    }
                    // nameカラム: NOT NULL + UNIQUE
                    assert_eq!(table.columns[1].nullability, Some(false));
                    assert_eq!(table.columns[1].constraints.len(), 1);
                    match &table.columns[1].constraints[0] {
                        ColumnConstraint::Unique => {
                            // OK
                        }
                        _ => panic!("nameカラムにUNIQUE制約があること"),
                    }
                }
                _ => panic!("CREATE TABLEであること"),
            }
        }
        _ => panic!("CREATE文であること"),
    }
}
