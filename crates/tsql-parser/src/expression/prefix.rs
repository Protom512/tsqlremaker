//! 前置演算子と一次式の解析

use crate::ast::{Expression, Identifier, Literal, UnaryOperator};
use crate::error::{ParseError, ParseResult};
use tsql_token::TokenKind;

use super::BindingPower;

impl super::ExpressionParser<'_, '_> {
    /// 前置演算子の解析（null denotation）
    pub(crate) fn parse_prefix(&mut self) -> ParseResult<Expression> {
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
            // 括弧式またはスカラサブクエリ
            TokenKind::LParen => {
                self.buffer.consume()?;

                // 先読きしてSELECTの場合はサブクエリとして処理
                if self.buffer.check(TokenKind::Select) {
                    let stmt = self.parse_subquery_select()?;

                    if !self.buffer.check(TokenKind::RParen) {
                        return Err(ParseError::unexpected_token(
                            vec![TokenKind::RParen],
                            self.buffer.current()?.kind,
                            self.buffer.current()?.span,
                        ));
                    }
                    self.buffer.consume()?;

                    return Ok(Expression::Subquery(Box::new(stmt)));
                }

                // 通常の括弧式
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
            // キーワードで識別子として使用可能なもの
            _ if Self::can_keyword_be_identifier(current.kind) => {
                let text = current.text;
                let span = current.span;
                self.buffer.consume()?;
                let ident = Identifier {
                    name: text.to_string(),
                    span,
                };
                self.parse_identifier_tail(ident)
            }
            _ => self.parse_primary(),
        }
    }

    /// 一次式を解析
    pub(super) fn parse_primary(&mut self) -> ParseResult<Expression> {
        let current = self.buffer.current()?;
        let span = current.span;

        match current.kind {
            // リテラル
            TokenKind::String => {
                let text = &current.text[1..current.text.len() - 1];
                self.buffer.consume()?;
                Ok(Expression::Literal(Literal::String(text.to_string(), span)))
            }
            TokenKind::NString => {
                let text = &current.text[2..current.text.len() - 1];
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
            // 真理値または識別子
            TokenKind::Ident
            | TokenKind::LocalVar
            | TokenKind::GlobalVar
            | TokenKind::TempTable
            | TokenKind::GlobalTempTable => {
                let text = current.text;
                let upper = text.to_uppercase();
                self.buffer.consume()?;
                match upper.as_str() {
                    "TRUE" => Ok(Expression::Literal(Literal::Boolean(true, span))),
                    "FALSE" => Ok(Expression::Literal(Literal::Boolean(false, span))),
                    _ => {
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

    /// キーワードを識別子として解析可能かチェック
    fn can_keyword_be_identifier(kind: TokenKind) -> bool {
        matches!(
            kind,
            // 型名
            TokenKind::Int | TokenKind::Varchar | TokenKind::Char | TokenKind::Date |
            TokenKind::Datetime | TokenKind::Bit | TokenKind::Text | TokenKind::Binary |
            // 制御フローキーワード
            TokenKind::Goto | TokenKind::Label | TokenKind::Begin | TokenKind::End |
            // GOは式の文脈では識別子として使用可能
            TokenKind::Go |
            // その他識別子として使用されやすいキーワード
            TokenKind::Table | TokenKind::Index | TokenKind::Key |
            TokenKind::Constraint | TokenKind::View | TokenKind::Proc | TokenKind::Function
        )
    }
}
