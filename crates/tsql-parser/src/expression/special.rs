//! 特殊式の解析（CASE, EXISTS, IS, IN, BETWEEN, LIKE）

use crate::ast::{AstNode, CaseExpression, Expression, InList, IsValue};
use crate::error::{ParseError, ParseResult};
use tsql_token::{Span, TokenKind};

use super::BindingPower;

impl super::ExpressionParser<'_, '_> {
    /// CASE式を解析
    pub(super) fn parse_case_expression(&mut self) -> ParseResult<Expression> {
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
    pub(super) fn parse_exists_expression(&mut self) -> ParseResult<Expression> {
        self.buffer.consume()?; // EXISTS

        // 括弧をチェック
        if !self.buffer.check(TokenKind::LParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::LParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // LParen

        // サブクエリを解析
        let select_stmt = self.parse_subquery_select()?;

        // 閉じ括弧
        if !self.buffer.check(TokenKind::RParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::RParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // RParen

        Ok(Expression::Exists(Box::new(select_stmt)))
    }

    /// IS式を解析
    pub(super) fn parse_is_expression(&mut self, left: Expression) -> ParseResult<Expression> {
        let start = AstNode::span(&left).start;
        self.buffer.consume()?; // IS

        // NOT IS のチェック
        let negated = if self.buffer.check(TokenKind::Not) {
            self.buffer.consume()?;
            true
        } else {
            false
        };

        // 値を解析
        let value = if self.buffer.check(TokenKind::Null) {
            self.buffer.consume()?;
            IsValue::Null
        } else {
            let current = self.buffer.current()?;
            let span = current.span;
            let text = current.text.to_uppercase();
            self.buffer.consume()?;
            match text.as_str() {
                "TRUE" => IsValue::True,
                "FALSE" => IsValue::False,
                "UNKNOWN" => IsValue::Unknown,
                _ => {
                    return Err(ParseError::invalid_syntax(
                        format!(
                            "Expected NULL, TRUE, FALSE, or UNKNOWN after IS, found '{}'",
                            text
                        ),
                        span,
                    ))
                }
            }
        };

        let end_span = self.buffer.current()?.span;
        Ok(Expression::Is {
            expr: Box::new(left),
            negated,
            value,
            span: Span {
                start,
                end: end_span.end,
            },
        })
    }

    /// NOT [IN | BETWEEN | LIKE] 式を解析
    pub(super) fn parse_not_special_expression(
        &mut self,
        left: Expression,
    ) -> ParseResult<Expression> {
        self.buffer.consume()?; // NOT

        match self.buffer.current()?.kind {
            TokenKind::In => {
                self.buffer.consume()?; // IN
                self.parse_in_expression(left, true)
            }
            TokenKind::Between => {
                self.buffer.consume()?; // BETWEEN
                self.parse_between_expression(left, true)
            }
            TokenKind::Like => {
                self.buffer.consume()?; // LIKE
                self.parse_like_expression(left, true)
            }
            _ => Err(ParseError::unexpected_token(
                vec![TokenKind::In, TokenKind::Between, TokenKind::Like],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            )),
        }
    }

    /// IN式を解析
    /// Note: INトークンは呼び出し元で消費されている
    pub(super) fn parse_in_expression(
        &mut self,
        left: Expression,
        negated: bool,
    ) -> ParseResult<Expression> {
        let start = AstNode::span(&left).start;

        // 値リストまたはサブクエリ
        let list = if self.buffer.check(TokenKind::LParen) {
            self.buffer.consume()?; // LEFT PAREN

            let list = if self.buffer.check(TokenKind::Select) {
                // サブクエリ
                let select_stmt = self.parse_subquery_select()?;
                InList::Subquery(Box::new(select_stmt))
            } else {
                // 値リスト
                let mut values = Vec::new();
                while !self.buffer.check(TokenKind::RParen) && !self.buffer.check(TokenKind::Eof) {
                    values.push(self.parse()?);
                    if !self.buffer.consume_if(TokenKind::Comma)? {
                        break;
                    }
                }
                InList::Values(values)
            };

            // 閉じ括弧を確認して消費
            if self.buffer.check(TokenKind::RParen) {
                self.buffer.consume()?; // RIGHT PAREN
            } else {
                return Err(ParseError::unexpected_eof(
                    ")".to_string(),
                    tsql_token::Position {
                        line: 1,
                        column: self.buffer.current()?.span.end,
                        offset: self.buffer.current()?.span.end,
                    },
                ));
            };
            list
        } else {
            // 括弧なしは構文エラー
            return Err(ParseError::unexpected_token(
                vec![TokenKind::LParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        };

        Ok(Expression::In {
            expr: Box::new(left),
            list,
            negated,
            span: Span {
                start,
                end: self.buffer.current()?.span.end,
            },
        })
    }

    /// BETWEEN式を解析
    /// Note: BETWEENトークンは呼び出し元で消費されている
    pub(super) fn parse_between_expression(
        &mut self,
        left: Expression,
        negated: bool,
    ) -> ParseResult<Expression> {
        let start = AstNode::span(&left).start;

        // LogicalAnd より高い結合力でパース
        let low = self.parse_bp(BindingPower::Comparison)?;

        if !self.buffer.check(TokenKind::And) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::And],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // AND

        let high = self.parse_bp(BindingPower::Comparison)?;
        let high_span = AstNode::span(&high);

        Ok(Expression::Between {
            expr: Box::new(left),
            low: Box::new(low),
            high: Box::new(high),
            negated,
            span: Span {
                start,
                end: high_span.end,
            },
        })
    }

    /// LIKE式を解析
    /// Note: LIKEトークンは呼び出し元で消費されている
    pub(super) fn parse_like_expression(
        &mut self,
        left: Expression,
        negated: bool,
    ) -> ParseResult<Expression> {
        let start = AstNode::span(&left).start;

        let pattern = self.parse()?;
        let pattern_span = AstNode::span(&pattern);

        // ESCAPE句の解析
        // Note: ESCAPEトークンがまだ定義されていないため、後で実装
        let escape = None;

        Ok(Expression::Like {
            expr: Box::new(left),
            pattern: Box::new(pattern),
            escape,
            negated,
            span: Span {
                start,
                end: pattern_span.end,
            },
        })
    }
}
