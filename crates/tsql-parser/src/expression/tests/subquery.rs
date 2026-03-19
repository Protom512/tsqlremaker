//! サブクエリ内のFROM句（派生テーブル）のテスト

use crate::buffer::TokenBuffer;
use crate::expression::ExpressionParser;
use crate::ast::{SelectItem, TableReference};
use tsql_lexer::Lexer;

/// 派生テーブルを含むINサブクエリのテスト
#[test]
fn test_subquery_from_clause_in_in_subquery() {
    let sql = "id IN (SELECT t.user_id FROM (SELECT user_id FROM orders) AS t)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    // 式をパース（IN式）
    let expr = parser.parse().unwrap();

    // IN式であることを確認
    match expr {
        crate::ast::Expression::In { list, .. } => {
            match list {
                crate::ast::InList::Subquery(subquery) => {
                    // サブクエリ内にFROM句と派生テーブルがあることを確認
                    assert!(subquery.from.is_some());
                    let from_clause = subquery.from.as_ref().unwrap();
                    assert_eq!(from_clause.tables.len(), 1);
                    match &from_clause.tables[0] {
                        TableReference::Subquery { alias, .. } => {
                            assert_eq!(alias.as_ref().unwrap().name, "t");
                        }
                        _ => panic!("INサブクエリ内に派生テーブルがあること"),
                    }
                }
                _ => panic!("INリストがサブクエリであること"),
            }
        }
        _ => panic!("IN式であること"),
    }
}

/// EXISTSサブクエリ内の派生テーブルのテスト
#[test]
fn test_subquery_from_clause_in_exists_subquery() {
    let sql = "EXISTS (SELECT 1 FROM (SELECT user_id FROM orders) AS t WHERE t.user_id > 10)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    // 式をパース（EXISTS式）
    let expr = parser.parse().unwrap();

    // EXISTS式であることを確認
    match expr {
        crate::ast::Expression::Exists(subquery) => {
            // EXISTSサブクエリ内にFROM句と派生テーブルがあることを確認
            assert!(subquery.from.is_some());
            let from_clause = subquery.from.as_ref().unwrap();
            assert_eq!(from_clause.tables.len(), 1);
            match &from_clause.tables[0] {
                TableReference::Subquery { alias, query } => {
                    assert_eq!(alias.as_ref().unwrap().name, "t");
                    // WHERE句があることを確認
                    assert!(query.where_clause.is_some());
                }
                _ => panic!("EXISTSサブクエリ内に派生テーブルがあること"),
            }
        }
        _ => panic!("EXISTS式であること"),
    }
}

/// 派生テーブル内にGROUP BYを持つサブクエリのテスト
#[test]
fn test_subquery_from_clause_with_group_by() {
    let sql = "EXISTS (SELECT 1 FROM (SELECT user_id, COUNT(*) FROM orders GROUP BY user_id) AS t)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    let expr = parser.parse().unwrap();

    match expr {
        crate::ast::Expression::Exists(subquery) => {
            assert!(subquery.from.is_some());
            let from_clause = subquery.from.as_ref().unwrap();
            match &from_clause.tables[0] {
                TableReference::Subquery { query, .. } => {
                    // 派生テーブル内にGROUP BYがあることを確認
                    assert!(!query.group_by.is_empty());
                }
                _ => panic!("派生テーブルであること"),
            }
        }
        _ => panic!("EXISTS式であること"),
    }
}

/// 派生テーブル内にHAVINGを持つサブクエリのテスト
#[test]
fn test_subquery_from_clause_with_having() {
    let sql = "EXISTS (SELECT 1 FROM (SELECT user_id, COUNT(*) AS cnt FROM orders GROUP BY user_id HAVING COUNT(*) > 5) AS t)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    let expr = parser.parse().unwrap();

    match expr {
        crate::ast::Expression::Exists(subquery) => {
            assert!(subquery.from.is_some());
            let from_clause = subquery.from.as_ref().unwrap();
            match &from_clause.tables[0] {
                TableReference::Subquery { query, .. } => {
                    // 派生テーブル内にGROUP BYとHAVINGがあることを確認
                    assert!(!query.group_by.is_empty());
                    assert!(query.having.is_some());
                }
                _ => panic!("派生テーブルであること"),
            }
        }
        _ => panic!("EXISTS式であること"),
    }
}

/// 派生テーブル内にORDER BYを持つサブクエリのテスト
#[test]
fn test_subquery_from_clause_with_order_by() {
    let sql = "EXISTS (SELECT 1 FROM (SELECT user_id FROM orders ORDER BY user_id) AS t)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    let expr = parser.parse().unwrap();

    match expr {
        crate::ast::Expression::Exists(subquery) => {
            assert!(subquery.from.is_some());
            let from_clause = subquery.from.as_ref().unwrap();
            match &from_clause.tables[0] {
                TableReference::Subquery { query, .. } => {
                    // 派生テーブル内にORDER BYがあることを確認
                    assert!(!query.order_by.is_empty());
                }
                _ => panic!("派生テーブルであること"),
            }
        }
        _ => panic!("EXISTS式であること"),
    }
}

/// 派生テーブル内にDISTINCTを持つサブクエリのテスト
#[test]
fn test_subquery_from_clause_with_distinct() {
    let sql = "EXISTS (SELECT 1 FROM (SELECT DISTINCT user_id FROM orders) AS t)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    let expr = parser.parse().unwrap();

    match expr {
        crate::ast::Expression::Exists(subquery) => {
            assert!(subquery.from.is_some());
            let from_clause = subquery.from.as_ref().unwrap();
            match &from_clause.tables[0] {
                TableReference::Subquery { query, .. } => {
                    // 派生テーブル内にDISTINCTがあることを確認
                    assert!(query.distinct);
                }
                _ => panic!("派生テーブルであること"),
            }
        }
        _ => panic!("EXISTS式であること"),
    }
}

/// 派生テーブル内にWHERE句を持つサブクエリのテスト
#[test]
fn test_subquery_from_clause_with_where() {
    let sql = "EXISTS (SELECT 1 FROM (SELECT user_id FROM orders WHERE amount > 100) AS t)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    let expr = parser.parse().unwrap();

    match expr {
        crate::ast::Expression::Exists(subquery) => {
            assert!(subquery.from.is_some());
            let from_clause = subquery.from.as_ref().unwrap();
            match &from_clause.tables[0] {
                TableReference::Subquery { query, .. } => {
                    // 派生テーブル内にWHERE句があることを確認
                    assert!(query.where_clause.is_some());
                }
                _ => panic!("派生テーブルであること"),
            }
        }
        _ => panic!("EXISTS式であること"),
    }
}

/// 派生テーブルの別名（ASなし）のテスト
#[test]
fn test_subquery_from_clause_without_as_keyword() {
    let sql = "EXISTS (SELECT 1 FROM (SELECT user_id FROM orders) t)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    let expr = parser.parse().unwrap();

    match expr {
        crate::ast::Expression::Exists(subquery) => {
            assert!(subquery.from.is_some());
            let from_clause = subquery.from.as_ref().unwrap();
            match &from_clause.tables[0] {
                TableReference::Subquery { alias, .. } => {
                    assert_eq!(alias.as_ref().unwrap().name, "t");
                }
                _ => panic!("派生テーブルであること"),
            }
        }
        _ => panic!("EXISTS式であること"),
    }
}

/// 複数のカラムを持つ派生テーブルのテスト
#[test]
fn test_subquery_from_clause_multiple_columns() {
    let sql = "EXISTS (SELECT 1 FROM (SELECT user_id, order_id, amount FROM orders) AS t)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    let expr = parser.parse().unwrap();

    match expr {
        crate::ast::Expression::Exists(subquery) => {
            assert!(subquery.from.is_some());
            let from_clause = subquery.from.as_ref().unwrap();
            match &from_clause.tables[0] {
                TableReference::Subquery { query, .. } => {
                    // 派生テーブル内に3つのカラムがあることを確認
                    assert_eq!(query.columns.len(), 3);
                    // 全てExpression型であることを確認
                    match &query.columns[0] {
                        SelectItem::Expression(_, _) => {}
                        _ => panic!("カラムはExpression型であること"),
                    }
                }
                _ => panic!("派生テーブルであること"),
            }
        }
        _ => panic!("EXISTS式であること"),
    }
}

/// ワイルドカードを持つ派生テーブルのテスト
#[test]
fn test_subquery_from_clause_wildcard() {
    let sql = "EXISTS (SELECT 1 FROM (SELECT * FROM orders) AS t)";
    let mut lexer = Lexer::new(sql);
    let tokens: Vec<_> = lexer.filter_map(|t| t.ok()).collect();

    let mut buffer = TokenBuffer::new(&tokens);
    let mut parser = ExpressionParser::new(&mut buffer);

    let expr = parser.parse().unwrap();

    match expr {
        crate::ast::Expression::Exists(subquery) => {
            assert!(subquery.from.is_some());
            let from_clause = subquery.from.as_ref().unwrap();
            match &from_clause.tables[0] {
                TableReference::Subquery { query, .. } => {
                    // 派生テーブル内にワイルドカードがあることを確認
                    assert_eq!(query.columns.len(), 1);
                    match &query.columns[0] {
                        SelectItem::Wildcard => {}
                        _ => panic!("カラムはWildcardであること"),
                    }
                }
                _ => panic!("派生テーブルであること"),
            }
        }
        _ => panic!("EXISTS式であること"),
    }
}
