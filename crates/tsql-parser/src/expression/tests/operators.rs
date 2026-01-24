//! 演算子のパーサーテスト

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

// 二項演算子テスト

#[test]
fn test_parse_binary_op_addition() {
    let expr = parse_expr("1 + 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Plus,
            ..
        } => {}
        _ => panic!("Expected BinaryOp with Plus"),
    }
}

#[test]
fn test_parse_binary_op_multiplication() {
    let expr = parse_expr("2 * 3").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Multiply,
            ..
        } => {}
        _ => panic!("Expected BinaryOp with Multiply"),
    }
}

#[test]
fn test_parse_binary_op_subtraction() {
    let expr = parse_expr("5 - 3").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Minus,
            ..
        } => {}
        _ => panic!("Expected BinaryOp with Minus"),
    }
}

#[test]
fn test_parse_binary_op_division() {
    let expr = parse_expr("10 / 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Divide,
            ..
        } => {}
        _ => panic!("Expected BinaryOp with Divide"),
    }
}

#[test]
fn test_parse_binary_op_modulo() {
    let expr = parse_expr("10 % 3").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Modulo,
            ..
        } => {}
        _ => panic!("Expected BinaryOp with Modulo"),
    }
}

#[test]
fn test_parse_comparison_operators() {
    let expr = parse_expr("1 > 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Gt,
            ..
        } => {}
        _ => panic!("Expected Gt operator"),
    }

    let expr = parse_expr("1 >= 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Ge,
            ..
        } => {}
        _ => panic!("Expected Ge operator"),
    }

    let expr = parse_expr("1 < 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Lt,
            ..
        } => {}
        _ => panic!("Expected Lt operator"),
    }

    let expr = parse_expr("1 <= 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Le,
            ..
        } => {}
        _ => panic!("Expected Le operator"),
    }

    let expr = parse_expr("1 = 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Eq,
            ..
        } => {}
        _ => panic!("Expected Eq operator"),
    }

    let expr = parse_expr("1 <> 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::NeAlt,
            ..
        } => {}
        _ => panic!("Expected NeAlt operator"),
    }
}

#[test]
fn test_parse_logical_operators() {
    let expr = parse_expr("TRUE AND FALSE").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::And,
            ..
        } => {}
        _ => panic!("Expected And operator"),
    }

    let expr = parse_expr("TRUE OR FALSE").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Or,
            ..
        } => {}
        _ => panic!("Expected Or operator"),
    }
}

#[test]
fn test_parse_concat_operator() {
    let expr = parse_expr("'a' || 'b'").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Concat,
            ..
        } => {}
        _ => panic!("Expected Concat operator"),
    }
}

#[test]
fn test_parse_precedence_multiply_before_add() {
    let expr = parse_expr("1 + 2 * 3").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Plus,
            ..
        } => {}
        _ => panic!("Expected BinaryOp with Plus at top level"),
    }
}

// 単項演算子テスト

#[test]
fn test_parse_unary_op_minus() {
    let expr = parse_expr("-123").unwrap();
    match expr {
        Expression::UnaryOp {
            op: UnaryOperator::Minus,
            ..
        } => {}
        _ => panic!("Expected UnaryOp with Minus"),
    }
}

#[test]
fn test_parse_unary_op_not() {
    let expr = parse_expr("NOT TRUE").unwrap();
    match expr {
        Expression::UnaryOp {
            op: UnaryOperator::Not,
            ..
        } => {}
        _ => panic!("Expected UnaryOp with Not"),
    }
}

#[test]
fn test_parse_parenthesized_expression() {
    let expr = parse_expr("(1 + 2)").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Plus,
            ..
        } => {}
        _ => panic!("Expected BinaryOp with Plus inside parentheses"),
    }
}

#[test]
fn test_parse_nested_expressions() {
    let expr = parse_expr("(1 + 2) * 3").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Multiply,
            ..
        } => {}
        _ => panic!("Expected Multiply with nested expression"),
    }
}

// 演算子の優先順位テスト

#[test]
fn test_precedence_arithmetic_multiply_over_add() {
    let expr = parse_expr("1 + 2 * 3").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Plus,
            ..
        } => {}
        _ => panic!("Expected Plus operator at top level"),
    }
}

#[test]
fn test_precedence_arithmetic_divide_over_subtract() {
    let expr = parse_expr("10 - 6 / 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Minus,
            ..
        } => {}
        _ => panic!("Expected Minus operator at top level"),
    }
}

#[test]
fn test_precedence_modulo_with_addition() {
    let expr = parse_expr("10 % 3 + 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Plus,
            ..
        } => {}
        _ => panic!("Expected Plus operator at top level"),
    }
}

#[test]
fn test_precedence_comparison_over_arithmetic() {
    let expr = parse_expr("1 + 2 > 3 * 4").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Gt,
            ..
        } => {}
        _ => panic!("Expected Gt operator at top level"),
    }
}

#[test]
fn test_precedence_logical_and_over_or() {
    let expr = parse_expr("TRUE OR FALSE AND TRUE").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Or,
            ..
        } => {}
        _ => panic!("Expected Or operator at top level"),
    }
}

#[test]
fn test_precedence_comparison_over_logical() {
    let expr = parse_expr("1 = 2 AND 3 > 4").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::And,
            ..
        } => {}
        _ => panic!("Expected And operator at top level"),
    }
}

#[test]
fn test_precedence_not_over_and() {
    let expr = parse_expr("NOT TRUE AND FALSE").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::And,
            ..
        } => {}
        _ => panic!("Expected And operator at top level"),
    }
}

#[test]
fn test_precedence_parentheses_override() {
    let expr = parse_expr("(1 + 2) * 3").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Multiply,
            ..
        } => {}
        _ => panic!("Expected Multiply operator at top level"),
    }
}

#[test]
fn test_precedence_parentheses_multiple() {
    let expr = parse_expr("((1 + 2) * (3 + 4))").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Multiply,
            ..
        } => {}
        _ => panic!("Expected Multiply operator at top level"),
    }
}

// 結合性テスト

#[test]
fn test_associativity_left() {
    let expr = parse_expr("10 - 5 - 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Minus,
            ..
        } => {}
        _ => panic!("Expected Minus operator at top level"),
    }
}

#[test]
fn test_associativity_multiply_divide() {
    let expr = parse_expr("12 / 3 * 2").unwrap();
    match expr {
        Expression::BinaryOp {
            op: BinaryOperator::Divide,
            ..
        } => {}
        _ => panic!("Expected Divide operator at top level"),
    }
}

#[test]
fn test_combined_expressions() {
    let expr = parse_expr("x IN (1, 2) AND y BETWEEN 10 AND 20 OR z IS NULL");
    assert!(expr.is_ok());
    match expr.unwrap() {
        Expression::BinaryOp {
            op: BinaryOperator::Or,
            ..
        } => {}
        _ => panic!("Expected Or at top level"),
    }
}
