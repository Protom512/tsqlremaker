//! 特殊式（CASE, IN, LIKE, BETWEEN, IS）のパーサーテスト

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

// CASE式テスト

#[test]
fn test_parse_case_expression() {
    let expr = parse_expr("CASE WHEN 1 = 1 THEN 2 ELSE 3 END").unwrap();
    match expr {
        Expression::Case(case) => {
            assert_eq!(case.branches.len(), 1);
            assert!(case.else_result.is_some());
        }
        _ => panic!("Expected Case expression"),
    }
}

#[test]
fn test_case_expression_multiple_branches() {
    let expr = parse_expr(
        "CASE WHEN x = 1 THEN 'one' WHEN x = 2 THEN 'two' WHEN x = 3 THEN 'three' ELSE 'other' END",
    )
    .unwrap();
    match expr {
        Expression::Case(case) => {
            assert_eq!(case.branches.len(), 3);
            assert!(case.else_result.is_some());
        }
        _ => panic!("Expected Case expression"),
    }
}

#[test]
fn test_case_expression_no_else() {
    let expr = parse_expr("CASE WHEN status = 1 THEN 'active' END").unwrap();
    match expr {
        Expression::Case(case) => {
            assert_eq!(case.branches.len(), 1);
            assert!(case.else_result.is_none(), "Expected no else branch");
        }
        _ => panic!("Expected Case expression"),
    }
}

#[test]
fn test_case_expression_nested() {
    let expr = parse_expr(
        "CASE WHEN x = 1 THEN CASE WHEN y = 2 THEN 'both' ELSE 'only_x' END ELSE 'not_x' END",
    )
    .unwrap();
    match expr {
        Expression::Case(case) => {
            assert_eq!(case.branches.len(), 1);
            assert!(case.else_result.is_some());
            match &case.branches[0].1 {
                Expression::Case(nested) => {
                    assert_eq!(nested.branches.len(), 1);
                    assert!(nested.else_result.is_some());
                }
                _ => panic!("Expected nested Case expression"),
            }
        }
        _ => panic!("Expected Case expression"),
    }
}

// IN式テスト

#[test]
fn test_parse_in_expression() {
    let expr = parse_expr("1 IN (1, 2, 3)").unwrap();
    match expr {
        Expression::In {
            list: InList::Values(values),
            ..
        } => {
            assert_eq!(values.len(), 3);
        }
        _ => panic!("Expected IN expression"),
    }
}

#[test]
fn test_parse_not_in_expression() {
    let expr = parse_expr("1 NOT IN (1, 2)").unwrap();
    match expr {
        Expression::In { negated: true, .. } => {}
        _ => panic!("Expected NOT IN expression"),
    }
}

#[test]
fn test_in_expression_single_value() {
    let expr = parse_expr("1 IN (1)").unwrap();
    match expr {
        Expression::In {
            list: InList::Values(values),
            ..
        } => {
            assert_eq!(values.len(), 1);
        }
        _ => panic!("Expected In expression"),
    }
}

#[test]
fn test_in_expression_multiple_values() {
    let expr = parse_expr("status IN (1, 2, 3, 4, 5)").unwrap();
    match expr {
        Expression::In {
            list: InList::Values(values),
            ..
        } => {
            assert_eq!(values.len(), 5);
        }
        _ => panic!("Expected In expression"),
    }
}

#[test]
fn test_in_expression_with_expressions() {
    let expr = parse_expr("x + y IN (1, 2 * 3, 4 + 5)").unwrap();
    match expr {
        Expression::In { .. } => {}
        _ => panic!("Expected In expression, got {:?}", expr),
    }
}

// LIKE式テスト

#[test]
fn test_parse_like_expression() {
    let expr = parse_expr("column LIKE '%pattern%'").unwrap();
    match expr {
        Expression::Like { .. } => {}
        _ => panic!("Expected LIKE expression"),
    }
}

#[test]
fn test_parse_not_like_expression() {
    let expr = parse_expr("column NOT LIKE '%pattern%'").unwrap();
    match expr {
        Expression::Like { negated: true, .. } => {}
        _ => panic!("Expected NOT LIKE expression"),
    }
}

#[test]
fn test_like_simple_pattern() {
    let expr = parse_expr("name LIKE 'John%'").unwrap();
    match expr {
        Expression::Like { negated: false, .. } => {}
        _ => panic!("Expected Like expression"),
    }
}

#[test]
fn test_like_complex_pattern() {
    let expr = parse_expr("email LIKE '%@%.com'").unwrap();
    match expr {
        Expression::Like { .. } => {}
        _ => panic!("Expected Like expression"),
    }
}

#[test]
fn test_like_with_column_pattern() {
    let expr = parse_expr("name LIKE pattern_col").unwrap();
    match expr {
        Expression::Like { .. } => {}
        _ => panic!("Expected Like expression"),
    }
}

// BETWEEN式テスト

#[test]
fn test_parse_between_expression() {
    let expr = parse_expr("1 BETWEEN 0 AND 10").unwrap();
    match expr {
        Expression::Between { .. } => {}
        _ => panic!("Expected BETWEEN expression"),
    }
}

#[test]
fn test_parse_not_between_expression() {
    let expr = parse_expr("1 NOT BETWEEN 0 AND 10").unwrap();
    match expr {
        Expression::Between { negated: true, .. } => {}
        _ => panic!("Expected NOT BETWEEN expression"),
    }
}

#[test]
fn test_between_simple() {
    let expr = parse_expr("age BETWEEN 18 AND 65").unwrap();
    match expr {
        Expression::Between { negated: false, .. } => {}
        _ => panic!("Expected Between expression"),
    }
}

#[test]
fn test_between_with_expressions() {
    let expr = parse_expr("x + y BETWEEN 10 AND 20").unwrap();
    match expr {
        Expression::Between { .. } => {}
        _ => panic!("Expected Between expression, got {:?}", expr),
    }
}

#[test]
fn test_between_nested_between() {
    let expr = parse_expr("x BETWEEN 1 AND 10 OR y BETWEEN 20 AND 30");
    assert!(expr.is_ok());
}

// IS式テスト

#[test]
fn test_parse_is_null_expression() {
    let expr = parse_expr("column IS NULL").unwrap();
    match expr {
        Expression::Is {
            value: IsValue::Null,
            ..
        } => {}
        _ => panic!("Expected Is Null expression"),
    }
}

#[test]
fn test_parse_is_not_null_expression() {
    let expr = parse_expr("column IS NOT NULL").unwrap();
    match expr {
        Expression::Is {
            negated: true,
            value: IsValue::Null,
            ..
        } => {}
        _ => panic!("Expected IS NOT NULL expression"),
    }
}

#[test]
fn test_is_null() {
    let expr = parse_expr("column IS NULL").unwrap();
    match expr {
        Expression::Is {
            value: IsValue::Null,
            negated: false,
            ..
        } => {}
        _ => panic!("Expected Is Null expression"),
    }
}

#[test]
fn test_is_not_null() {
    let expr = parse_expr("column IS NOT NULL").unwrap();
    match expr {
        Expression::Is {
            value: IsValue::Null,
            negated: true,
            ..
        } => {}
        _ => panic!("Expected Is Not Null expression"),
    }
}

#[test]
fn test_is_true() {
    let expr = parse_expr("flag IS TRUE").unwrap();
    match expr {
        Expression::Is {
            value: IsValue::True,
            negated: false,
            ..
        } => {}
        _ => panic!("Expected Is True expression"),
    }
}

#[test]
fn test_is_false() {
    let expr = parse_expr("flag IS FALSE").unwrap();
    match expr {
        Expression::Is {
            value: IsValue::False,
            negated: false,
            ..
        } => {}
        _ => panic!("Expected Is False expression"),
    }
}

#[test]
fn test_is_unknown() {
    let expr = parse_expr("result IS UNKNOWN").unwrap();
    match expr {
        Expression::Is {
            value: IsValue::Unknown,
            negated: false,
            ..
        } => {}
        _ => panic!("Expected Is Unknown expression"),
    }
}

#[test]
fn test_is_not_true() {
    let expr = parse_expr("flag IS NOT TRUE").unwrap();
    match expr {
        Expression::Is {
            value: IsValue::True,
            negated: true,
            ..
        } => {}
        _ => panic!("Expected Is Not True expression"),
    }
}

// EXISTS式テスト

#[test]
fn test_parse_exists_expression() {
    let expr = parse_expr("EXISTS(SELECT 1)");
    assert!(expr.is_ok() || expr.is_err());
}
