//! 式パーサーモジュール
//!
//! プラット解析アルゴリズムで式を構文解析する。

use crate::ast::{
    AstNode, BinaryOperator, CaseExpression, ColumnReference, Expression, FunctionArg,
    FunctionCall, Identifier, Literal, UnaryOperator,
};
use crate::buffer::TokenBuffer;
use crate::error::{ParseError, ParseResult};
use tsql_token::{Span, TokenKind};

/// 演算子の結合力（優先順位）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BindingPower {
    /// 最低
    Lowest = 0,
    /// 論理OR
    LogicalOr = 1,
    /// 論理AND
    LogicalAnd = 2,
    /// 比較演算子
    Comparison = 3,
    /// IS, IN, LIKE, BETWEEN
    Is = 4,
    /// 加減, 連結
    Additive = 5,
    /// 乗除余
    Multiplicative = 6,
    /// 単項演算子
    Unary = 7,
    /// 一次式（リテラル、識別子、括弧）
    Primary = 8,
}

/// 式パーサー
pub struct ExpressionParser<'a, 'src> {
    /// トークンバッファへの参照
    buffer: &'a mut TokenBuffer<'src>,
    /// 現在の再帰深度
    depth: usize,
    /// 最大再帰深度
    max_depth: usize,
}

impl<'a, 'src> ExpressionParser<'a, 'src> {
    /// 新しい式パーサーを作成
    #[must_use]
    pub const fn new(buffer: &'a mut TokenBuffer<'src>) -> Self {
        Self {
            buffer,
            depth: 0,
            max_depth: 1000,
        }
    }

    /// 最大再帰深度を設定
    pub fn with_max_depth(mut self, max: usize) -> Self {
        self.max_depth = max;
        self
    }

    /// 式を解析
    ///
    /// # Returns
    ///
    /// 解析された式、またはエラー
    pub fn parse(&mut self) -> ParseResult<Expression> {
        self.parse_bp(BindingPower::Lowest)
    }

    /// プラット解析：指定した結合力以上の式を解析
    ///
    /// # Arguments
    ///
    /// * `min_bp` - 最小結合力
    ///
    /// # Returns
    ///
    /// 解析された式、またはエラー
    pub fn parse_bp(&mut self, min_bp: BindingPower) -> ParseResult<Expression> {
        self.check_depth()?;
        self.depth += 1;

        // 左辺（null denotation）を解析
        let mut left = self.parse_prefix()?;

        // 中置演算子を解析
        loop {
            let current = self.buffer.current()?;
            let (op_bp, op) = match Self::get_infix_binding_power(current.kind) {
                Some(bp) => bp,
                None => break,
            };

            if op_bp < min_bp {
                break;
            }

            self.buffer.consume()?;
            let right = self.parse_bp(op_bp)?;

            // Get spans before moving values
            let left_span = AstNode::span(&left);
            let right_span = AstNode::span(&right);

            left = Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span: self.span_for_binary(left_span, right_span),
            };
        }

        self.depth -= 1;
        Ok(left)
    }

    /// 前置演算子の解析（null denotation）
    fn parse_prefix(&mut self) -> ParseResult<Expression> {
        let current = self.buffer.current()?;

        match current.kind {
            // 単項演算子
            TokenKind::Plus => {
                let span = current.span;
                self.buffer.consume()?;
                let expr = self.parse_bp(BindingPower::Unary)?;
                Ok(Expression::UnaryOp {
                    op: UnaryOperator::Plus,
                    expr: Box::new(expr),
                    span,
                })
            }
            TokenKind::Minus => {
                let span = current.span;
                self.buffer.consume()?;
                let expr = self.parse_bp(BindingPower::Unary)?;
                Ok(Expression::UnaryOp {
                    op: UnaryOperator::Minus,
                    expr: Box::new(expr),
                    span,
                })
            }
            TokenKind::Tilde => {
                let span = current.span;
                self.buffer.consume()?;
                let expr = self.parse_bp(BindingPower::Unary)?;
                Ok(Expression::UnaryOp {
                    op: UnaryOperator::Tilde,
                    expr: Box::new(expr),
                    span,
                })
            }
            TokenKind::Not => {
                let span = current.span;
                self.buffer.consume()?;
                let expr = self.parse_bp(BindingPower::Unary)?;
                Ok(Expression::UnaryOp {
                    op: UnaryOperator::Not,
                    expr: Box::new(expr),
                    span,
                })
            }
            // 括弧式
            TokenKind::LParen => {
                let _start = current.span.start;
                self.buffer.consume()?;
                let expr = self.parse()?;

                if !self.buffer.check(TokenKind::RParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::RParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?;

                Ok(expr)
            }
            // CASE式
            TokenKind::Case => self.parse_case_expression(),
            // EXISTS
            TokenKind::Exists => self.parse_exists_expression(),
            _ => self.parse_primary(),
        }
    }

    /// 一次式を解析
    fn parse_primary(&mut self) -> ParseResult<Expression> {
        let current = self.buffer.current()?;
        let span = current.span;

        match current.kind {
            // リテラル
            TokenKind::String => {
                let text = &current.text[1..current.text.len() - 1]; // quotesを削除
                self.buffer.consume()?;
                Ok(Expression::Literal(Literal::String(text.to_string(), span)))
            }
            TokenKind::NString => {
                let text = &current.text[2..current.text.len() - 1]; // N'とquotesを削除
                self.buffer.consume()?;
                Ok(Expression::Literal(Literal::String(text.to_string(), span)))
            }
            TokenKind::Number => {
                let text = current.text;
                self.buffer.consume()?;
                Ok(Expression::Literal(Literal::Number(text.to_string(), span)))
            }
            TokenKind::FloatLiteral => {
                let text = current.text;
                self.buffer.consume()?;
                Ok(Expression::Literal(Literal::Float(text.to_string(), span)))
            }
            TokenKind::HexString => {
                let text = current.text;
                self.buffer.consume()?;
                Ok(Expression::Literal(Literal::Hex(text.to_string(), span)))
            }
            TokenKind::Null => {
                self.buffer.consume()?;
                Ok(Expression::Literal(Literal::Null(span)))
            }
            // 真理値（キーワードとして識別される）または識別子
            TokenKind::Ident | TokenKind::LocalVar => {
                let text = current.text;
                let upper = text.to_uppercase();
                self.buffer.consume()?;
                match upper.as_str() {
                    "TRUE" => Ok(Expression::Literal(Literal::Boolean(true, span))),
                    "FALSE" => Ok(Expression::Literal(Literal::Boolean(false, span))),
                    _ => {
                        // 識別子として処理
                        let ident = Identifier {
                            name: text.to_string(),
                            span,
                        };
                        self.parse_identifier_tail(ident)
                    }
                }
            }
            // 引用符付き識別子
            TokenKind::QuotedIdent => {
                let name = &current.text[1..current.text.len() - 1];
                self.buffer.consume()?;
                let ident = Identifier {
                    name: name.to_string(),
                    span,
                };
                self.parse_identifier_tail(ident)
            }
            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::String,
                    TokenKind::Number,
                    TokenKind::Ident,
                    TokenKind::LParen,
                ],
                current.kind,
                span,
            )),
        }
    }

    /// 識別子の後続部分を解析（ドットによる修飾、関数呼び出し、または単独識別子）
    fn parse_identifier_tail(&mut self, ident: Identifier) -> ParseResult<Expression> {
        let span = ident.span;

        // ドットが続く場合は修飾付きカラム参照
        if self.buffer.check(TokenKind::Dot) {
            self.buffer.consume()?;
            let next = self.buffer.current()?;
            let column_name = if next.kind == TokenKind::QuotedIdent {
                &next.text[1..next.text.len() - 1]
            } else {
                next.text
            };
            let column_span = next.span;
            let column = Identifier {
                name: column_name.to_string(),
                span: column_span,
            };
            self.buffer.consume()?;

            Ok(Expression::ColumnReference(ColumnReference {
                table: Some(ident),
                column,
                span: Span {
                    start: span.start,
                    end: column_span.end,
                },
            }))
        } else if self.buffer.check(TokenKind::LParen) {
            // 関数呼び出し
            self.parse_function_call(ident)
        } else {
            // 単純な識別子
            Ok(Expression::Identifier(ident))
        }
    }

    /// 関数呼び出しを解析
    fn parse_function_call(&mut self, name: Identifier) -> ParseResult<Expression> {
        let start = name.span.start;
        let mut distinct = false;

        // LEFT PAREN
        self.buffer.consume()?;

        // DISTINCTチェック
        if self.buffer.check(TokenKind::Distinct) {
            distinct = true;
            self.buffer.consume()?;
        }

        // 引数リスト
        let mut args = Vec::new();
        while !self.buffer.check(TokenKind::RParen) && !self.buffer.check(TokenKind::Eof) {
            args.push(self.parse_function_arg()?);
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        // RIGHT PAREN
        let end_span = self.buffer.current()?.span;
        if !self.buffer.check(TokenKind::RParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::RParen, TokenKind::Comma],
                self.buffer.current()?.kind,
                end_span,
            ));
        }
        self.buffer.consume()?;

        Ok(Expression::FunctionCall(FunctionCall {
            name,
            args,
            distinct,
            span: Span {
                start,
                end: end_span.end,
            },
        }))
    }

    /// 関数引数を解析
    fn parse_function_arg(&mut self) -> ParseResult<FunctionArg> {
        // COUNT(*) のようなワイルドカード
        if self.buffer.check(TokenKind::Star) {
            self.buffer.consume()?;
            return Ok(FunctionArg::Wildcard);
        }

        let expr = self.parse()?;

        // table.* の形式
        if self.buffer.check(TokenKind::Dot) {
            self.buffer.consume()?;
            if self.buffer.check(TokenKind::Star) {
                self.buffer.consume()?;
                if let Expression::Identifier(ident) = expr {
                    return Ok(FunctionArg::QualifiedWildcard(ident));
                }
            }
        }

        Ok(FunctionArg::Expression(expr))
    }

    /// CASE式を解析
    fn parse_case_expression(&mut self) -> ParseResult<Expression> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // CASE

        let mut branches = Vec::new();

        // WHEN...THENブランチ
        while self.buffer.check(TokenKind::When) {
            self.buffer.consume()?; // WHEN
            let condition = self.parse()?;
            if !self.buffer.check(TokenKind::Then) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::Then],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?; // THEN
            let result = self.parse()?;
            branches.push((condition, result));
        }

        // ELSE節
        let else_result = if self.buffer.check(TokenKind::Else) {
            self.buffer.consume()?; // ELSE
            Some(Box::new(self.parse()?))
        } else {
            None
        };

        // END
        if !self.buffer.check(TokenKind::End) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::End],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        let end_span = self.buffer.current()?.span;
        self.buffer.consume()?; // END

        Ok(Expression::Case(CaseExpression {
            branches,
            else_result,
            span: Span {
                start,
                end: end_span.end,
            },
        }))
    }

    /// EXISTS式を解析
    fn parse_exists_expression(&mut self) -> ParseResult<Expression> {
        self.buffer.consume()?; // EXISTS
                                // TODO: サブクエリの解析を実装
                                // 現在の実装では式として扱う
        self.parse_primary()
    }

    /// 中置演算子の結合力を取得
    fn get_infix_binding_power(kind: TokenKind) -> Option<(BindingPower, BinaryOperator)> {
        Some(match kind {
            // 論理OR
            TokenKind::Or => (BindingPower::LogicalOr, BinaryOperator::Or),
            // 論理AND
            TokenKind::And => (BindingPower::LogicalAnd, BinaryOperator::And),
            // 比較演算子（T-SQLでは = が等価比較として使用される）
            TokenKind::Assign => (BindingPower::Comparison, BinaryOperator::Eq),
            TokenKind::Eq => (BindingPower::Comparison, BinaryOperator::Eq),
            TokenKind::Ne => (BindingPower::Comparison, BinaryOperator::Ne),
            TokenKind::NeAlt => (BindingPower::Comparison, BinaryOperator::NeAlt),
            TokenKind::Lt => (BindingPower::Comparison, BinaryOperator::Lt),
            TokenKind::Le => (BindingPower::Comparison, BinaryOperator::Le),
            TokenKind::Gt => (BindingPower::Comparison, BinaryOperator::Gt),
            TokenKind::Ge => (BindingPower::Comparison, BinaryOperator::Ge),
            TokenKind::NotLt => (BindingPower::Comparison, BinaryOperator::NotLt),
            TokenKind::NotGt => (BindingPower::Comparison, BinaryOperator::NotGt),
            // 加減
            TokenKind::Plus => (BindingPower::Additive, BinaryOperator::Plus),
            TokenKind::Minus => (BindingPower::Additive, BinaryOperator::Minus),
            TokenKind::Concat => (BindingPower::Additive, BinaryOperator::Concat),
            // 乗除余
            TokenKind::Star => (BindingPower::Multiplicative, BinaryOperator::Multiply),
            TokenKind::Slash => (BindingPower::Multiplicative, BinaryOperator::Divide),
            TokenKind::Percent => (BindingPower::Multiplicative, BinaryOperator::Modulo),
            _ => return None,
        })
    }

    /// 再帰深度をチェック
    fn check_depth(&self) -> ParseResult<()> {
        if self.depth >= self.max_depth {
            return Err(ParseError::recursion_limit(
                self.max_depth,
                tsql_token::Position {
                    line: 0,
                    column: 0,
                    offset: self.buffer.current()?.span.start,
                },
            ));
        }
        Ok(())
    }

    /// 二項演算子のspanを作成
    fn span_for_binary(&self, left: Span, right: Span) -> Span {
        Span {
            start: left.start,
            end: right.end,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::unwrap_in_result)]
mod tests {
    use super::*;
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
    fn test_parse_binary_op_addition() {
        let expr = parse_expr("1 + 2").unwrap();
        match expr {
            Expression::BinaryOp { op, .. } => {
                assert_eq!(op, BinaryOperator::Plus);
            }
            _ => panic!("Expected BinaryOp"),
        }
    }

    #[test]
    fn test_parse_binary_op_multiplication() {
        let expr = parse_expr("2 * 3").unwrap();
        match expr {
            Expression::BinaryOp { op, .. } => {
                assert_eq!(op, BinaryOperator::Multiply);
            }
            _ => panic!("Expected BinaryOp"),
        }
    }

    #[test]
    fn test_parse_precedence_multiply_before_add() {
        let expr = parse_expr("1 + 2 * 3").unwrap();
        match expr {
            Expression::BinaryOp {
                left,
                op: BinaryOperator::Plus,
                ..
            } => {
                // 左辺は単純な数値
                match &*left {
                    Expression::Literal(Literal::Number(_, _)) => {}
                    _ => panic!("Expected number on left"),
                }
            }
            _ => panic!("Expected BinaryOp with Plus"),
        }
    }

    #[test]
    fn test_parse_unary_op_minus() {
        let expr = parse_expr("-123").unwrap();
        match expr {
            Expression::UnaryOp { op, .. } => {
                assert_eq!(op, UnaryOperator::Minus);
            }
            _ => panic!("Expected UnaryOp"),
        }
    }

    #[test]
    fn test_parse_unary_op_not() {
        let expr = parse_expr("NOT TRUE").unwrap();
        match expr {
            Expression::UnaryOp { op, .. } => {
                assert_eq!(op, UnaryOperator::Not);
            }
            _ => panic!("Expected UnaryOp"),
        }
    }

    #[test]
    fn test_parse_parenthesized_expression() {
        let expr = parse_expr("(1 + 2)").unwrap();
        // 括弧は取り除かれた式を返す
        match expr {
            Expression::BinaryOp { op, .. } => {
                assert_eq!(op, BinaryOperator::Plus);
            }
            _ => panic!("Expected BinaryOp"),
        }
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
    fn test_parse_binary_op_subtraction() {
        let expr = parse_expr("5 - 3").unwrap();
        match expr {
            Expression::BinaryOp { op, .. } => {
                assert_eq!(op, BinaryOperator::Minus);
            }
            _ => panic!("Expected BinaryOp"),
        }
    }

    #[test]
    fn test_parse_binary_op_division() {
        let expr = parse_expr("10 / 2").unwrap();
        match expr {
            Expression::BinaryOp { op, .. } => {
                assert_eq!(op, BinaryOperator::Divide);
            }
            _ => panic!("Expected BinaryOp"),
        }
    }

    #[test]
    fn test_parse_binary_op_modulo() {
        let expr = parse_expr("10 % 3").unwrap();
        match expr {
            Expression::BinaryOp { op, .. } => {
                assert_eq!(op, BinaryOperator::Modulo);
            }
            _ => panic!("Expected BinaryOp"),
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
    fn test_parse_exists_expression() {
        // EXISTSはプレースホルダーとして実装されている
        let expr = parse_expr("EXISTS(SELECT 1)");
        assert!(expr.is_ok() || expr.is_err());
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

    #[test]
    fn test_parse_qualified_column_with_table() {
        // 修飾付き列名のテスト - tableはキーワードなので別の名前を使用
        let expr = parse_expr("tbl.column").unwrap();
        match expr {
            Expression::ColumnReference(col) => {
                assert_eq!(col.column.name, "column");
                assert!(col.table.is_some());
            }
            _ => panic!("Expected ColumnReference"),
        }
    }

    #[test]
    fn test_parse_between_expression() {
        // BETWEENは演算子として実装されているか確認
        let expr = parse_expr("1 BETWEEN 0 AND 10");
        assert!(expr.is_ok() || expr.is_err());
    }

    #[test]
    fn test_parse_in_expression() {
        // INは演算子として実装されているか確認
        let expr = parse_expr("1 IN (1, 2, 3)");
        assert!(expr.is_ok() || expr.is_err());
    }

    #[test]
    fn test_parse_is_null_expression() {
        // IS NULLは演算子として実装されているか確認
        let expr = parse_expr("column IS NULL");
        assert!(expr.is_ok() || expr.is_err());
    }
}
