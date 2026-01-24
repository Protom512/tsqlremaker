//! リテラルと識別子のパーサーテスト

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
fn test_parse_literal_number() {
    let expr = parse_expr("123").unwrap();
    match expr {
        Expression::Literal(Literal::Number(n, _)) => assert_eq!(n, "123"),
        _ => panic!("Expected Number literal"),
    }
}

#[test]
fn test_parse_literal_string() {
    let expr = parse_expr("'hello'").unwrap();
    match expr {
        Expression::Literal(Literal::String(s, _)) => assert_eq!(s, "hello"),
        _ => panic!("Expected String literal"),
    }
}

#[test]
fn test_parse_literal_null() {
    let expr = parse_expr("NULL").unwrap();
    match expr {
        Expression::Literal(Literal::Null(_)) => {}
        _ => panic!("Expected Null literal"),
    }
}

#[test]
fn test_parse_identifier() {
    let expr = parse_expr("column_name").unwrap();
    match expr {
        Expression::Identifier(ident) => assert_eq!(ident.name, "column_name"),
        _ => panic!("Expected Identifier"),
    }
}

#[test]
fn test_parse_column_reference() {
    let expr = parse_expr("tbl.column").unwrap();
    match expr {
        Expression::ColumnReference(col) => {
            assert_eq!(col.table.as_ref().unwrap().name, "tbl");
            assert_eq!(col.column.name, "column");
        }
        _ => panic!("Expected ColumnReference"),
    }
}

#[test]
fn test_parse_qualified_column_with_table() {
    let expr = parse_expr("tbl.column").unwrap();
    match expr {
        Expression::ColumnReference(col) => {
            assert_eq!(col.column.name, "column");
            assert!(col.table.is_some());
        }
        _ => panic!("Expected ColumnReference"),
    }
}
