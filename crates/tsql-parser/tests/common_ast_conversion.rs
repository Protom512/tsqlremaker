//! Common SQL AST 変換テスト
//!
//! T-SQL AST から Common SQL AST への変換を検証するテストスイート。

// テストコードでは unwrap/panic/expect を許可
#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]
#![allow(clippy::single_match)]
#![allow(clippy::len_zero)]

use tsql_parser::{common::ToCommonAst, parse};

/// 基本的な SELECT 文の変換テスト
#[test]
fn test_select_statement_conversion() {
    let sql = "SELECT id, name FROM users WHERE id = 1";
    let statements = parse(sql).unwrap();

    assert_eq!(statements.len(), 1);

    let converted = statements[0].to_common_ast();
    assert!(converted.is_some(), "SELECT文をCommon ASTに変換できること");

    let common_stmt = converted.unwrap();
    match common_stmt {
        tsql_parser::common::CommonStatement::Select(select) => {
            assert_eq!(select.columns.len(), 2, "カラムが2つであること");
            assert!(!select.distinct, "DISTINCTではないこと");
            assert_eq!(select.from.len(), 1, "FROM句に1つのテーブルがあること");
            assert!(select.where_clause.is_some(), "WHERE句があること");
        }
        _ => panic!("SELECT文に変換されるべき"),
    }
}

/// DISTINCT付き SELECT 文の変換テスト
#[test]
fn test_select_distinct_conversion() {
    let sql = "SELECT DISTINCT name FROM products";
    let statements = parse(sql).unwrap();

    let converted = statements[0].to_common_ast();
    assert!(converted.is_some());

    let common_stmt = converted.unwrap();
    match common_stmt {
        tsql_parser::common::CommonStatement::Select(select) => {
            assert!(select.distinct, "DISTINCTがtrueであること");
        }
        _ => panic!("SELECT文に変換されるべき"),
    }
}

/// ワイルドカードを含む SELECT 文の変換テスト
#[test]
fn test_select_wildcard_conversion() {
    let sql = "SELECT * FROM users";
    let statements = parse(sql).unwrap();

    let converted = statements[0].to_common_ast();
    assert!(converted.is_some());

    let common_stmt = converted.unwrap();
    match common_stmt {
        tsql_parser::common::CommonStatement::Select(select) => {
            assert_eq!(select.columns.len(), 1);
            match &select.columns[0] {
                tsql_parser::common::CommonSelectItem::Wildcard => {
                    // OK
                }
                _ => panic!("ワイルドカードであること"),
            }
        }
        _ => panic!("SELECT文に変換されるべき"),
    }
}

/// 修飾付きワイルドカードの変換テスト
#[test]
fn test_select_qualified_wildcard_conversion() {
    let sql = "SELECT users.* FROM users";
    let statements = parse(sql).unwrap();

    let converted = statements[0].to_common_ast();
    assert!(converted.is_some());

    let common_stmt = converted.unwrap();
    match common_stmt {
        tsql_parser::common::CommonStatement::Select(select) => {
            assert_eq!(select.columns.len(), 1);
            // パーサーは users.* を ColumnReference として解析する
            match &select.columns[0] {
                tsql_parser::common::CommonSelectItem::Expression(
                    tsql_parser::common::CommonExpression::ColumnReference(col),
                    _,
                ) => {
                    assert_eq!(col.table.as_ref().unwrap(), "users");
                    assert_eq!(col.column, "*");
                }
                other => panic!("カラム参照であること, got {:?}", other),
            }
        }
        _ => panic!("SELECT文に変換されるべき"),
    }
}

/// INSERT VALUES 文の変換テスト
#[test]
fn test_insert_values_conversion() {
    let sql = "INSERT INTO users (id, name) VALUES (1, 'test')";
    let statements = parse(sql).unwrap();

    let converted = statements[0].to_common_ast();
    assert!(converted.is_some(), "INSERT文をCommon ASTに変換できること");

    let common_stmt = converted.unwrap();
    match common_stmt {
        tsql_parser::common::CommonStatement::Insert(insert) => {
            assert_eq!(insert.table, "users");
            assert_eq!(insert.columns.len(), 2);
            match &insert.source {
                tsql_parser::common::CommonInsertSource::Values(rows) => {
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].len(), 2);
                }
                _ => panic!("VALUESであること"),
            }
        }
        _ => panic!("INSERT文に変換されるべき"),
    }
}

/// UPDATE 文の変換テスト
#[test]
fn test_update_conversion() {
    let sql = "UPDATE users SET name = 'test' WHERE id = 1";
    let statements = parse(sql).unwrap();

    let converted = statements[0].to_common_ast();
    assert!(converted.is_some(), "UPDATE文をCommon ASTに変換できること");

    let common_stmt = converted.unwrap();
    match common_stmt {
        tsql_parser::common::CommonStatement::Update(update) => {
            assert_eq!(update.table, "users");
            assert_eq!(update.assignments.len(), 1);
            assert_eq!(update.assignments[0].column, "name");
            assert!(update.where_clause.is_some(), "WHERE句があること");
        }
        _ => panic!("UPDATE文に変換されるべき"),
    }
}

/// DELETE 文の変換テスト
#[test]
fn test_delete_conversion() {
    let sql = "DELETE FROM users WHERE id = 1";
    let statements = parse(sql).unwrap();

    let converted = statements[0].to_common_ast();
    assert!(converted.is_some(), "DELETE文をCommon ASTに変換できること");

    let common_stmt = converted.unwrap();
    match common_stmt {
        tsql_parser::common::CommonStatement::Delete(delete) => {
            assert_eq!(delete.table, "users");
            assert!(delete.where_clause.is_some(), "WHERE句があること");
        }
        _ => panic!("DELETE文に変換されるべき"),
    }
}

/// 式の変換テスト - リテラル
#[test]
fn test_expression_literal_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT 1, 'test', NULL, TRUE";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            // 各カラムの式を確認
            for item in &select_stmt.columns {
                match item {
                    tsql_parser::SelectItem::Expression(expr, _) => {
                        let common_expr = expr.to_common_expression();
                        assert!(common_expr.is_some(), "式をCommon ASTに変換できること");
                    }
                    _ => {}
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - カラム参照
#[test]
fn test_expression_column_reference_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT id, name FROM users";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            match &select_stmt.columns[0] {
                tsql_parser::SelectItem::Expression(expr, _) => {
                    let common_expr = expr.to_common_expression();
                    assert!(common_expr.is_some(), "式をCommon ASTに変換できること");

                    let common_expr = common_expr.unwrap();
                    match common_expr {
                        // 単一の識別子は Identifier として変換される
                        tsql_parser::common::CommonExpression::Identifier(id) => {
                            assert_eq!(id.name, "id");
                        }
                        tsql_parser::common::CommonExpression::ColumnReference(col) => {
                            assert_eq!(col.column, "id");
                            assert!(col.table.is_none());
                        }
                        _ => panic!(
                            "識別子またはカラム参照に変換されるべき, got {:?}",
                            common_expr
                        ),
                    }
                }
                _ => panic!("式であること"),
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - 修飾付きカラム参照
#[test]
fn test_expression_qualified_column_reference_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT users.id FROM users";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => match &select_stmt.columns[0] {
            tsql_parser::SelectItem::Expression(expr, _) => {
                let common_expr = expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::ColumnReference(col) => {
                        assert_eq!(col.column, "id");
                        assert_eq!(col.table.as_ref().unwrap(), &"users".to_string());
                    }
                    _ => panic!("修飾付きカラム参照に変換されるべき"),
                }
            }
            _ => panic!("式であること"),
        },
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - 二項演算子
#[test]
fn test_expression_binary_op_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT a + b FROM t";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => match &select_stmt.columns[0] {
            tsql_parser::SelectItem::Expression(expr, _) => {
                let common_expr = expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::BinaryOp { op, .. } => {
                        assert_eq!(op, tsql_parser::common::CommonBinaryOperator::Plus);
                    }
                    _ => panic!("二項演算子に変換されるべき"),
                }
            }
            _ => panic!("式であること"),
        },
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - 比較演算子
#[test]
fn test_expression_comparison_op_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT id FROM users WHERE id = 1";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common_expr = where_expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::BinaryOp { op, .. } => {
                        assert_eq!(op, tsql_parser::common::CommonBinaryOperator::Eq);
                    }
                    _ => panic!("比較演算子に変換されるべき"),
                }
            } else {
                panic!("WHERE句があるはず");
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - 論理演算子
#[test]
fn test_expression_logical_op_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM users WHERE active = TRUE AND verified = TRUE";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common_expr = where_expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::BinaryOp { op, .. } => {
                        assert_eq!(op, tsql_parser::common::CommonBinaryOperator::And);
                    }
                    _ => panic!("論理演算子に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - 関数呼び出し
#[test]
fn test_expression_function_call_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT COUNT(*) FROM users";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => match &select_stmt.columns[0] {
            tsql_parser::SelectItem::Expression(expr, _) => {
                let common_expr = expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::FunctionCall(f) => {
                        assert_eq!(f.name, "COUNT");
                    }
                    _ => panic!("関数呼び出しに変換されるべき"),
                }
            }
            _ => {}
        },
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - IN 式
#[test]
fn test_expression_in_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM users WHERE id IN (1, 2, 3)";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common_expr = where_expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::In { negated, list, .. } => {
                        assert!(!negated, "NOT INではない");
                        match list {
                            tsql_parser::common::CommonInList::Values(values) => {
                                assert_eq!(values.len(), 3);
                            }
                            tsql_parser::common::CommonInList::Subquery(_) => {
                                panic!("サブクエリではない");
                            }
                        }
                    }
                    _ => panic!("IN式に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - NOT IN 式
#[test]
fn test_expression_not_in_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM users WHERE id NOT IN (1, 2, 3)";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common_expr = where_expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::In { negated, .. } => {
                        assert!(negated, "NOT INである");
                    }
                    _ => panic!("IN式に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - BETWEEN 式
#[test]
fn test_expression_between_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM products WHERE price BETWEEN 100 AND 200";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common_expr = where_expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::Between { negated, .. } => {
                        assert!(!negated, "NOT BETWEENではない");
                    }
                    _ => panic!("BETWEEN式に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - LIKE 式
#[test]
fn test_expression_like_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM users WHERE name LIKE 'John%'";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common_expr = where_expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::Like { negated, .. } => {
                        assert!(!negated, "NOT LIKEではない");
                    }
                    _ => panic!("LIKE式に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - IS NULL 式
#[test]
fn test_expression_is_null_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM users WHERE name IS NULL";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common_expr = where_expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::IsNull { negated, .. } => {
                        assert!(!negated, "IS NOT NULLではない");
                    }
                    _ => panic!("IS NULL式に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 式の変換テスト - IS NOT NULL 式
#[test]
fn test_expression_is_not_null_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM users WHERE name IS NOT NULL";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common_expr = where_expr.to_common_expression();
                assert!(common_expr.is_some());

                let common_expr = common_expr.unwrap();
                match common_expr {
                    tsql_parser::common::CommonExpression::IsNull { negated, .. } => {
                        assert!(negated, "IS NOT NULLである");
                    }
                    _ => panic!("IS NOT NULL式に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// Span 情報の保持テスト
#[test]
fn test_span_preservation() {
    use tsql_parser::{common::CommonStatement, parse};

    let sql = "SELECT id FROM users";
    let statements = parse(sql).unwrap();

    let converted = statements[0].to_common_ast();
    assert!(converted.is_some());

    let common_stmt = converted.unwrap();
    let span = match &common_stmt {
        CommonStatement::Select(s) => s.span,
        _ => panic!("SELECT文であること"),
    };

    // Spanが設定されていることを確認（開始と終了が同じまたは開始 < 終了）
    assert!(
        span.start <= span.end,
        "Spanに有効な範囲があること: start={}, end={}",
        span.start,
        span.end
    );
}

/// 方言固有構文の変換テスト - DECLARE文
#[test]
fn test_dialect_specific_statement_conversion() {
    use tsql_parser::{common::CommonStatement, parse};

    let sql = "DECLARE @x INT";
    let statements = parse(sql).unwrap();

    let converted = statements[0].to_common_ast();
    assert!(converted.is_some());

    let common_stmt = converted.unwrap();
    match common_stmt {
        CommonStatement::DialectSpecific { description, .. } => {
            assert!(
                description.contains("DECLARE") || description.contains("variable"),
                "方言固有としてマークされること"
            );
        }
        _ => panic!("方言固有としてマークされるべき"),
    }
}

/// INSERT SELECT 文の変換テスト
#[test]
fn test_insert_select_conversion() {
    use tsql_parser::common::{CommonInsertSource, CommonStatement};

    let sql = "INSERT INTO users (id, name) SELECT id, name FROM temp_users";
    let statements = parse(sql).unwrap();

    let common_stmt = statements[0].to_common_ast().unwrap();
    match common_stmt {
        CommonStatement::Insert(insert) => {
            assert_eq!(insert.table, "users");
            assert_eq!(insert.columns.len(), 2);
            match &insert.source {
                CommonInsertSource::Select(_) => {} // OK
                _ => panic!("INSERT SELECTであること"),
            }
        }
        _ => panic!("INSERT文に変換されるべき"),
    }
}

/// CASE 式の変換テスト
#[test]
fn test_case_expression_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT CASE WHEN id > 10 THEN 'big' ELSE 'small' END FROM t";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => match &select_stmt.columns[0] {
            tsql_parser::SelectItem::Expression(expr, _) => {
                let common = expr.to_common_expression();
                assert!(common.is_some());
                match common.unwrap() {
                    tsql_parser::common::CommonExpression::Case(case) => {
                        assert_eq!(case.branches.len(), 1);
                        assert!(case.else_result.is_some());
                    }
                    _ => panic!("CASE式に変換されるべき"),
                }
            }
            _ => panic!("式であること"),
        },
        _ => panic!("SELECT文であること"),
    }
}

/// ORDER BY / GROUP BY / HAVING の変換テスト
#[test]
fn test_select_order_group_having_conversion() {
    use tsql_parser::common::CommonStatement;

    let sql = "SELECT category, COUNT(*) FROM products GROUP BY category HAVING COUNT(*) > 5 ORDER BY category";
    let statements = parse(sql).unwrap();

    let common_stmt = statements[0].to_common_ast().unwrap();
    match common_stmt {
        CommonStatement::Select(select) => {
            assert!(!select.group_by.is_empty(), "GROUP BYがあること");
            assert!(select.having.is_some(), "HAVINGがあること");
            assert!(!select.order_by.is_empty(), "ORDER BYがあること");
        }
        _ => panic!("SELECT文に変換されるべき"),
    }
}

/// EXISTS 式の変換テスト
#[test]
fn test_exists_expression_conversion() {
    use tsql_parser::parse_one;

    let sql =
        "SELECT * FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE orders.user_id = users.id)";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common = where_expr.to_common_expression();
                assert!(common.is_some());
                match common.unwrap() {
                    tsql_parser::common::CommonExpression::Exists { negated, .. } => {
                        assert!(!negated, "NOT EXISTSではない");
                    }
                    _ => panic!("EXISTS式に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// CREATE 文は方言固有として変換されること
#[test]
fn test_create_statement_dialect_specific() {
    use tsql_parser::common::CommonStatement;

    let sql = "CREATE TABLE users (id INT, name VARCHAR(100))";
    let statements = parse(sql).unwrap();

    let common_stmt = statements[0].to_common_ast().unwrap();
    match common_stmt {
        CommonStatement::DialectSpecific { description, .. } => {
            assert!(
                description.contains("CREATE"),
                "CREATE文は方言固有: {description}"
            );
        }
        _ => panic!("CREATE文は方言固有として変換されるべき"),
    }
}

/// UPDATE with FROM clause は方言固有として変換されること
#[test]
fn test_update_with_from_dialect_specific() {
    use tsql_parser::common::CommonStatement;

    // ASE supports UPDATE with FROM clause
    let sql = "UPDATE users SET name = 'test' FROM users WHERE id = 1";
    let statements = parse(sql).unwrap();

    let common_stmt = statements[0].to_common_ast().unwrap();
    match common_stmt {
        CommonStatement::DialectSpecific { description, .. } => {
            assert!(
                description.contains("FROM"),
                "UPDATE with FROMは方言固有: {description}"
            );
        }
        _ => panic!("UPDATE with FROMは方言固有として変換されるべき"),
    }
}

/// DELETE with FROM clause は方言固有として変換されること
#[test]
fn test_delete_with_from_dialect_specific() {
    use tsql_parser::common::CommonStatement;

    let sql = "DELETE users FROM users WHERE id = 1";
    let statements = parse(sql).unwrap();

    let common_stmt = statements[0].to_common_ast().unwrap();
    match common_stmt {
        CommonStatement::DialectSpecific { description, .. } => {
            assert!(
                description.contains("FROM"),
                "DELETE with FROMは方言固有: {description}"
            );
        }
        _ => panic!("DELETE with FROMは方言固有として変換されるべき"),
    }
}

/// 単項マイナス演算子の変換テスト
#[test]
fn test_unary_minus_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT -price FROM products";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => match &select_stmt.columns[0] {
            tsql_parser::SelectItem::Expression(expr, _) => {
                let common = expr.to_common_expression();
                assert!(common.is_some());
                match common.unwrap() {
                    tsql_parser::common::CommonExpression::UnaryOp { op, .. } => {
                        assert_eq!(
                            op,
                            tsql_parser::common::CommonUnaryOperator::Minus,
                            "マイナス演算子であること"
                        );
                    }
                    _ => panic!("単項演算子に変換されるべき"),
                }
            }
            _ => panic!("式であること"),
        },
        _ => panic!("SELECT文であること"),
    }
}

/// NOT BETWEEN の変換テスト
#[test]
fn test_not_between_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM products WHERE price NOT BETWEEN 100 AND 200";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common = where_expr.to_common_expression();
                assert!(common.is_some());
                match common.unwrap() {
                    tsql_parser::common::CommonExpression::Between { negated, .. } => {
                        assert!(negated, "NOT BETWEENであること");
                    }
                    _ => panic!("BETWEEN式に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// NOT LIKE の変換テスト
#[test]
fn test_not_like_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT * FROM users WHERE name NOT LIKE 'John%'";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => {
            if let Some(where_expr) = &select_stmt.where_clause {
                let common = where_expr.to_common_expression();
                assert!(common.is_some());
                match common.unwrap() {
                    tsql_parser::common::CommonExpression::Like { negated, .. } => {
                        assert!(negated, "NOT LIKEであること");
                    }
                    _ => panic!("LIKE式に変換されるべき"),
                }
            }
        }
        _ => panic!("SELECT文であること"),
    }
}

/// 16進数リテラルは変換不可 (Noneを返す)
#[test]
fn test_hex_literal_not_converted() {
    use tsql_parser::parse_one;

    let sql = "SELECT 0xFF";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => match &select_stmt.columns[0] {
            tsql_parser::SelectItem::Expression(expr, _) => {
                let common = expr.to_common_expression();
                assert!(common.is_none(), "16進数リテラルは変換不可であること");
            }
            _ => {}
        },
        _ => panic!("SELECT文であること"),
    }
}

/// BatchSeparator (GO) は None を返す
#[test]
fn test_batch_separator_returns_none() {
    use tsql_parser::common::ToCommonAst;

    let sql = "SELECT 1\nGO";
    let statements = parse(sql).unwrap();

    // GO is parsed as BatchSeparator
    for stmt in &statements {
        if matches!(stmt, tsql_parser::Statement::BatchSeparator(_)) {
            assert!(
                stmt.to_common_ast().is_none(),
                "BatchSeparatorはNoneを返すべき"
            );
        }
    }
}

/// Plus演算子 (+) の変換テスト
/// T-SQLでは + は算術加算としてパースされ、Common ASTでもPlusに変換される。
/// 文字列連結としての + は型推論が必要なため、パーサーレベルでは区別されない。
#[test]
fn test_plus_operator_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT price + 100 FROM products";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => match &select_stmt.columns[0] {
            tsql_parser::SelectItem::Expression(expr, _) => {
                let common = expr.to_common_expression();
                assert!(common.is_some());
                match common.unwrap() {
                    tsql_parser::common::CommonExpression::BinaryOp { op, .. } => {
                        assert_eq!(
                            op,
                            tsql_parser::common::CommonBinaryOperator::Plus,
                            "Plus演算子であること"
                        );
                    }
                    _ => panic!("二項演算子に変換されるべき"),
                }
            }
            _ => panic!("式であること"),
        },
        _ => panic!("SELECT文であること"),
    }
}

/// サブクエリ式の変換テスト
#[test]
fn test_subquery_expression_conversion() {
    use tsql_parser::parse_one;

    let sql = "SELECT (SELECT MAX(id) FROM users) AS max_id";
    let stmt = parse_one(sql).unwrap();

    match stmt {
        tsql_parser::Statement::Select(select_stmt) => match &select_stmt.columns[0] {
            tsql_parser::SelectItem::Expression(expr, _) => {
                let common = expr.to_common_expression();
                assert!(common.is_some());
                match common.unwrap() {
                    tsql_parser::common::CommonExpression::Subquery { .. } => {}
                    _ => panic!("サブクエリ式に変換されるべき"),
                }
            }
            _ => panic!("式であること"),
        },
        _ => panic!("SELECT文であること"),
    }
}

/// 制御フロー文 (IF, WHILE, TRY...CATCH等) は方言固有としてマークされること
#[test]
fn test_control_flow_dialect_specific() {
    use tsql_parser::common::CommonStatement;

    let sql = "IF 1 = 1 BEGIN SELECT 1 END";
    let statements = parse(sql).unwrap();

    let common_stmt = statements[0].to_common_ast().unwrap();
    match common_stmt {
        CommonStatement::DialectSpecific { .. } => {} // OK
        _ => panic!("IF文は方言固有としてマークされるべき"),
    }
}
