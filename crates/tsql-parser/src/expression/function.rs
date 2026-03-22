//! 関数呼び出しの解析

use crate::ast::{ColumnReference, Expression, FunctionArg, FunctionCall, Identifier};
use crate::error::{ParseError, ParseResult};
use tsql_token::{Span, TokenKind};

impl super::ExpressionParser<'_, '_> {
    /// 識別子の後続部分を解析（ドットによる修飾、関数呼び出し、または単独識別子）
    pub(super) fn parse_identifier_tail(&mut self, ident: Identifier) -> ParseResult<Expression> {
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
                self.buffer.current()?.position,
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
}
