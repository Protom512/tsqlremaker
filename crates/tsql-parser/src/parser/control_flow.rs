//! Control flow statement parsers
//!
//! IF, WHILE, BEGIN...END, BREAK, CONTINUE, RETURN, TRY...CATCH

use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use crate::expression::ExpressionParser;
use tsql_token::{Span, TokenKind};

impl<'src> super::Parser<'src> {
    /// IF文を解析
    pub(super) fn parse_if_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // IF

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let condition = expr_parser.parse()?;

        // 深度チェック
        self.check_depth_before_nesting()?;

        // 深度を増やしてネストされたステートメントをパース
        self.depth += 1;
        let then_branch = self.parse_statement()?;
        self.depth -= 1;

        let else_branch = if self.buffer.check(TokenKind::Else) {
            self.buffer.consume()?;
            self.check_depth_before_nesting()?;
            self.depth += 1;
            let branch = self.parse_statement()?;
            self.depth -= 1;
            Some(branch)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::If(Box::new(IfStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            condition,
            then_branch,
            else_branch,
        })))
    }

    /// WHILE文を解析
    pub(super) fn parse_while_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // WHILE

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let condition = expr_parser.parse()?;

        // 深度チェック
        self.check_depth_before_nesting()?;

        // 深度を増やしてネストされたステートメントをパース
        self.depth += 1;
        let body = self.parse_statement()?;
        self.depth -= 1;

        let end_span = self.buffer.current()?.span;
        Ok(Statement::While(Box::new(WhileStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            condition,
            body,
        })))
    }

    /// BEGIN...ENDブロックを解析
    pub(super) fn parse_block(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // BEGIN

        // 深度チェック
        self.check_depth_before_nesting()?;

        // ブロック内のステートメントは1レベル深いネストとして扱う
        self.depth += 1;
        let mut statements = Vec::new();
        while !self.buffer.check(TokenKind::End) && !self.is_at_eof() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
            // セミコロンを消費
            let _ = self.buffer.consume_if(TokenKind::Semicolon);
        }
        self.depth -= 1;

        if !self.buffer.check(TokenKind::End) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::End],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        let end_span = self.buffer.current()?.span;
        self.buffer.consume()?; // END

        Ok(Statement::Block(Box::new(Block {
            span: Span {
                start,
                end: end_span.end,
            },
            statements,
        })))
    }

    /// BREAK文を解析
    pub(super) fn parse_break_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // BREAK
        Ok(Statement::Break(Box::new(BreakStatement { span })))
    }

    /// CONTINUE文を解析
    pub(super) fn parse_continue_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // CONTINUE
        Ok(Statement::Continue(Box::new(ContinueStatement { span })))
    }

    /// RETURN文を解析
    pub(super) fn parse_return_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // RETURN

        let expression = if self.buffer.check(TokenKind::Semicolon)
            || self.buffer.check(TokenKind::End)
            || self.is_at_eof()
        {
            None
        } else {
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        };

        Ok(Statement::Return(Box::new(ReturnStatement {
            span,
            expression,
        })))
    }

    /// TRY...CATCH ブロックを解析
    ///
    /// T-SQL構文: BEGIN TRY ... END TRY BEGIN CATCH ... END CATCH
    pub(super) fn parse_try_catch_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;

        // BEGIN TRY
        self.buffer.consume()?; // BEGIN
        if !self.buffer.check(TokenKind::Try) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Try],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?; // TRY

        // TRY ブロックの本体をパース
        let try_block = if self.buffer.check(TokenKind::Begin) {
            match self.parse_block()? {
                Statement::Block(block) => block,
                _ => {
                    return Err(ParseError::invalid_syntax(
                        "Expected block statement".to_string(),
                        self.buffer.current()?.position,
                    ))
                }
            }
        } else {
            // 単一の文も許容
            self.check_depth_before_nesting()?;
            self.depth += 1;
            let stmt = self.parse_statement()?;
            self.depth -= 1;
            Box::new(Block {
                span: stmt.span(),
                statements: vec![stmt],
            })
        };

        // END TRY
        if !self.buffer.check(TokenKind::End) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::End],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?; // END

        if !self.buffer.check(TokenKind::Try) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Try],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?; // TRY

        // BEGIN CATCH
        if !self.buffer.check(TokenKind::Begin) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Begin],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?; // BEGIN

        if !self.buffer.check(TokenKind::Catch) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Catch],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?; // CATCH

        // CATCH ブロックの本体をパース
        let catch_block = if self.buffer.check(TokenKind::Begin) {
            match self.parse_block()? {
                Statement::Block(block) => block,
                _ => {
                    return Err(ParseError::invalid_syntax(
                        "Expected block statement".to_string(),
                        self.buffer.current()?.position,
                    ))
                }
            }
        } else {
            // 単一の文も許容
            self.check_depth_before_nesting()?;
            self.depth += 1;
            let stmt = self.parse_statement()?;
            self.depth -= 1;
            Box::new(Block {
                span: stmt.span(),
                statements: vec![stmt],
            })
        };

        // END CATCH
        if !self.buffer.check(TokenKind::End) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::End],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?; // END

        if !self.buffer.check(TokenKind::Catch) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Catch],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?; // CATCH

        let end_span = self.buffer.current()?.span;

        Ok(Statement::TryCatch(Box::new(TryCatchStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            try_block,
            catch_block,
        })))
    }

    /// BEGIN TRY かどうかをチェック
    ///
    /// BEGIN の後ろに TRY が続く場合のみ true
    pub(super) fn check_try_begin(&self) -> bool {
        // 現在のトークンは BEGIN なので、次のトークンをチェック
        self.buffer
            .peek(1)
            .is_ok_and(|t| matches!(t.kind, TokenKind::Try))
    }
}
