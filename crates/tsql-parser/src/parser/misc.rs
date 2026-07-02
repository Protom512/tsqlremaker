//! Miscellaneous statement parsing methods.
//!
//! DECLARE, SET, TRANSACTION, THROW, RAISERROR, EXEC/EXECUTE, GO batch separator,
//! and BEGIN TRANSACTION detection.

use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use crate::expression::ExpressionParser;
use tsql_token::{Span, TokenKind};

impl<'src> super::Parser<'src> {
    /// DECLARE文を解析
    pub(super) fn parse_declare_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // DECLARE

        let mut variables = Vec::new();
        loop {
            let name = self.parse_identifier()?;
            let data_type = self.parse_data_type()?;

            let default_value =
                if self.buffer.check(TokenKind::Eq) || self.buffer.check(TokenKind::Assign) {
                    self.buffer.consume()?;
                    let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                    Some(expr_parser.parse()?)
                } else {
                    None
                };

            variables.push(VariableDeclaration {
                name,
                data_type,
                default_value,
            });

            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Declare(Box::new(DeclareStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            variables,
        })))
    }

    /// SET文を解析
    pub(super) fn parse_set_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // SET

        let variable = self.parse_identifier()?;

        if !self.buffer.check(TokenKind::Eq) && !self.buffer.check(TokenKind::Assign) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Eq, TokenKind::Assign],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?;

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let value = expr_parser.parse()?;

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Set(Box::new(SetStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            variable,
            value,
        })))
    }

    /// トランザクション制御文を解析
    ///
    /// T-SQL構文: BEGIN TRANSACTION [name], COMMIT TRANSACTION [name],
    ///             ROLLBACK TRANSACTION [name], SAVE TRANSACTION name
    pub(super) fn parse_transaction_statement(&mut self) -> ParseResult<Statement> {
        let kind = self.buffer.current()?.kind;
        let start = self.buffer.current()?.span.start;

        match kind {
            // BEGIN TRANSACTION [name]
            TokenKind::Begin => {
                self.buffer.consume()?; // BEGIN

                if !self.buffer.check(TokenKind::Transaction) && !self.buffer.check(TokenKind::Tran)
                {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::Transaction, TokenKind::Tran],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.position,
                    ));
                }
                self.buffer.consume()?; // TRANSACTION | TRAN

                let name = if self.buffer.check(TokenKind::Ident)
                    || self.buffer.check(TokenKind::QuotedIdent)
                {
                    Some(self.parse_identifier()?)
                } else {
                    None
                };

                let end_span = self.buffer.current()?.span;

                Ok(Statement::Transaction(TransactionStatement::Begin {
                    span: Span {
                        start,
                        end: end_span.end,
                    },
                    name,
                }))
            }

            // COMMIT TRANSACTION [name]
            TokenKind::Commit => {
                self.buffer.consume()?; // COMMIT

                let (name, end_span) = if self.buffer.check(TokenKind::Transaction)
                    || self.buffer.check(TokenKind::Tran)
                {
                    self.buffer.consume()?; // TRANSACTION | TRAN
                    (
                        if self.buffer.check(TokenKind::Ident)
                            || self.buffer.check(TokenKind::QuotedIdent)
                        {
                            Some(self.parse_identifier()?)
                        } else {
                            None
                        },
                        self.buffer.current()?.position,
                    )
                } else {
                    // COMMIT だけの場合（TRANSACTION 省略）
                    (None, self.buffer.current()?.position)
                };

                Ok(Statement::Transaction(TransactionStatement::Commit {
                    span: Span {
                        start,
                        end: end_span.offset,
                    },
                    name,
                }))
            }

            // ROLLBACK TRANSACTION [name]
            TokenKind::Rollback => {
                self.buffer.consume()?; // ROLLBACK

                let (name, end_span) = if self.buffer.check(TokenKind::Transaction)
                    || self.buffer.check(TokenKind::Tran)
                {
                    self.buffer.consume()?; // TRANSACTION | TRAN
                    (
                        if self.buffer.check(TokenKind::Ident)
                            || self.buffer.check(TokenKind::QuotedIdent)
                        {
                            Some(self.parse_identifier()?)
                        } else {
                            None
                        },
                        self.buffer.current()?.position,
                    )
                } else {
                    // ROLLBACK だけの場合（TRANSACTION 省略）
                    (None, self.buffer.current()?.position)
                };

                Ok(Statement::Transaction(TransactionStatement::Rollback {
                    span: Span {
                        start,
                        end: end_span.offset,
                    },
                    name,
                }))
            }

            // SAVE TRANSACTION name
            TokenKind::Save => {
                self.buffer.consume()?; // SAVE

                if !self.buffer.check(TokenKind::Transaction) && !self.buffer.check(TokenKind::Tran)
                {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::Transaction, TokenKind::Tran],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.position,
                    ));
                }
                self.buffer.consume()?; // TRANSACTION | TRAN

                let name = self.parse_identifier()?;
                let end_span = self.buffer.current()?.span;

                Ok(Statement::Transaction(TransactionStatement::Save {
                    span: Span {
                        start,
                        end: end_span.end,
                    },
                    name,
                }))
            }

            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Begin,
                    TokenKind::Commit,
                    TokenKind::Rollback,
                    TokenKind::Save,
                ],
                kind,
                self.buffer.current()?.position,
            )),
        }
    }

    /// THROW 文を解析
    ///
    /// T-SQL構文: THROW [error_number, message, state]
    pub(super) fn parse_throw_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // THROW

        let error_number = if self.buffer.check(TokenKind::Semicolon)
            || self.buffer.check(TokenKind::End)
            || self.is_at_eof()
        {
            None
        } else {
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        };

        let message = if error_number.is_some() && (self.buffer.check(TokenKind::Comma)) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let state = if message.is_some() && self.buffer.check(TokenKind::Comma) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;

        Ok(Statement::Throw(Box::new(ThrowStatement {
            span: Span {
                start: span.start,
                end: end_span.end,
            },
            error_number,
            message,
            state,
        })))
    }

    /// RAISERROR 文を解析
    ///
    /// T-SQL構文: RAISERROR(message, severity, state)
    pub(super) fn parse_raiserror_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // RAISERROR

        // 左括弧
        if !self.buffer.check(TokenKind::LParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::LParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?;

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let message = expr_parser.parse()?;

        let severity = if self.buffer.check(TokenKind::Comma) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let state = if severity.is_some() && self.buffer.check(TokenKind::Comma) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        // 右括弧
        if !self.buffer.check(TokenKind::RParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::RParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?;

        let end_span = self.buffer.current()?.span;

        Ok(Statement::Raiserror(Box::new(RaiserrorStatement {
            span: Span {
                start: span.start,
                end: end_span.end,
            },
            message,
            severity,
            state,
        })))
    }

    /// EXEC/EXECUTE文を解析
    pub(super) fn parse_exec_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // EXEC or EXECUTE

        let procedure = self.parse_identifier()?;

        let mut arguments = Vec::new();

        // Parse arguments: first arg has no comma, subsequent args require comma
        // Supports: EXEC proc value1, @p1 = val, @p2
        let mut expect_arg = false;
        loop {
            if expect_arg {
                // After comma, next argument is required — propagate parse errors
                if self.buffer.check(TokenKind::LocalVar) {
                    let param_name = self.parse_identifier()?;
                    if self.buffer.check(TokenKind::Eq) || self.buffer.check(TokenKind::Assign) {
                        self.buffer.consume()?; // =
                        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                        let value = expr_parser.parse()?;
                        arguments.push(ExecArgument::Named {
                            name: param_name,
                            value,
                        });
                    } else {
                        arguments
                            .push(ExecArgument::Positional(Expression::Identifier(param_name)));
                    }
                } else {
                    let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                    let value = expr_parser.parse()?;
                    arguments.push(ExecArgument::Positional(value));
                }
            } else {
                // First arg (no preceding comma) — optional, break if not parseable
                if self.buffer.check(TokenKind::Comma) {
                    self.buffer.consume()?;
                    expect_arg = true;
                    continue;
                }
                if self.buffer.check(TokenKind::LocalVar) {
                    let param_name = self.parse_identifier()?;
                    if self.buffer.check(TokenKind::Eq) || self.buffer.check(TokenKind::Assign) {
                        self.buffer.consume()?; // =
                        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                        let value = expr_parser.parse()?;
                        arguments.push(ExecArgument::Named {
                            name: param_name,
                            value,
                        });
                    } else {
                        arguments
                            .push(ExecArgument::Positional(Expression::Identifier(param_name)));
                    }
                } else {
                    let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                    match expr_parser.parse() {
                        Ok(value) => arguments.push(ExecArgument::Positional(value)),
                        Err(_) => break, // No first arg — that's fine (e.g., EXEC sp_who)
                    }
                }
            }

            // Check for comma before next argument
            if self.buffer.check(TokenKind::Comma) {
                self.buffer.consume()?;
                expect_arg = true;
            } else {
                break;
            }
        }

        // Compute span end from the last meaningful token
        let resolve_end = |s: &Span| if s.end > 0 { s.end } else { s.start };
        let end = arguments
            .last()
            .map(|arg| match arg {
                ExecArgument::Positional(expr) => resolve_end(&expr.span()),
                ExecArgument::Named { value, .. } => resolve_end(&value.span()),
            })
            .unwrap_or_else(|| resolve_end(&procedure.span));

        Ok(Statement::Exec(Box::new(ExecStatement {
            span: Span { start, end },
            procedure,
            arguments,
        })))
    }

    /// BEGIN TRANSACTION かどうかをチェック
    ///
    /// BEGIN の後ろに TRANSACTION または TRAN が続く場合のみ true
    pub(super) fn check_transaction_begin(&self) -> bool {
        // 現在のトークンは BEGIN なので、次のトークンをチェック
        self.buffer
            .peek(1)
            .is_ok_and(|t| matches!(t.kind, TokenKind::Transaction | TokenKind::Tran))
    }

    /// バッチ区切り（GO）を解析
    pub(super) fn parse_batch_separator(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // GO

        let repeat_count = if self.buffer.check(TokenKind::Number) {
            let n = self.buffer.current()?.text.parse().ok();
            self.buffer.consume()?;
            n
        } else {
            None
        };

        Ok(Statement::BatchSeparator(BatchSeparator {
            span,
            repeat_count,
        }))
    }
}
