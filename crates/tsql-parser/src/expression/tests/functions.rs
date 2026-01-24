//! 関数呼び出しのパーサーテスト

use crate::ast::*;
use crate::buffer::TokenBuffer;
use crate::error::ParseResult;
use crate::expression::ExpressionParser;
use tsql_lexer::Lexer;

fn parse_expr(sql: &str) -> ParseResult<Expression> {
    let lexer = Lexer::new(sql);
    let mut buffer = TokenBuffer::new(lexer);
    let mut parser = ExpressionParser::new(&mut buffer);
    parser.parse()
}

#[test]
fn test_parse_function_call_no_args() {
    let expr = parse_expr("COUNT(*)").unwrap();
    match expr {
        Expression::FunctionCall(func) => {
            assert_eq!(func.name.name, "COUNT");
            assert_eq!(func.args.len(), 1);
        }
        _ => panic!("Expected FunctionCall"),
    }
}

#[test]
fn test_parse_function_call_with_args() {
    let expr = parse_expr("SUM(amount)").unwrap();
    match expr {
        Expression::FunctionCall(func) => {
            assert_eq!(func.name.name, "SUM");
            assert_eq!(func.args.len(), 1);
        }
        _ => panic!("Expected FunctionCall"),
    }
}

#[test]
fn test_parse_function_call_with_multiple_args() {
    let expr = parse_expr("CONCAT(a, b, c)").unwrap();
    match expr {
        Expression::FunctionCall(func) => {
            assert_eq!(func.name.name, "CONCAT");
            assert_eq!(func.args.len(), 3);
        }
        _ => panic!("Expected FunctionCall"),
    }
}

#[test]
fn test_function_call_multiple_args() {
    let expr = parse_expr("CONCAT(a, b, c, d)").unwrap();
    match expr {
        Expression::FunctionCall(func) => {
            assert_eq!(func.name.name, "CONCAT");
            assert_eq!(func.args.len(), 4);
        }
        _ => panic!("Expected FunctionCall"),
    }
}

#[test]
fn test_function_call_nested() {
    let expr = parse_expr("CONCAT(UPPER(name), 'suffix')").unwrap();
    match expr {
        Expression::FunctionCall(func) => {
            assert_eq!(func.name.name, "CONCAT");
            assert_eq!(func.args.len(), 2);
            match &func.args[0] {
                FunctionArg::Expression(Expression::FunctionCall(inner)) => {
                    assert_eq!(inner.name.name, "UPPER");
                }
                _ => panic!("Expected FunctionCall as first argument"),
            }
        }
        _ => panic!("Expected FunctionCall"),
    }
}

#[test]
fn test_aggregate_function_distinct() {
    let expr = parse_expr("COUNT(DISTINCT user_id)").unwrap();
    match expr {
        Expression::FunctionCall(func) => {
            assert_eq!(func.name.name, "COUNT");
            assert!(func.distinct, "Expected distinct to be true");
            assert_eq!(func.args.len(), 1);
        }
        _ => panic!("Expected FunctionCall"),
    }
}

#[test]
fn test_aggregate_sum_distinct() {
    let expr = parse_expr("SUM(DISTINCT amount)").unwrap();
    match expr {
        Expression::FunctionCall(func) => {
            assert_eq!(func.name.name, "SUM");
            assert!(func.distinct);
        }
        _ => panic!("Expected FunctionCall"),
    }
}

#[test]
fn test_function_with_all_arg_types() {
    let expr = parse_expr("COALESCE(col1, col2, 'default')").unwrap();
    match expr {
        Expression::FunctionCall(func) => {
            assert_eq!(func.name.name, "COALESCE");
            assert_eq!(func.args.len(), 3);
        }
        _ => panic!("Expected FunctionCall"),
    }
}
