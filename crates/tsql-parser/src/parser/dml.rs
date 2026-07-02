//! DML (Data Manipulation Language) parsing methods.
//!
//! INSERT, UPDATE, DELETE statements and SELECT variable assignment.

use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use crate::expression::ExpressionParser;
use tsql_token::{Span, TokenKind};

impl<'src> super::Parser<'src> {
    /// INSERT文を解析
    pub(super) fn parse_insert_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // INSERT

        if !self.buffer.check(TokenKind::Into) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Into],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?; // INTO

        let table = self.parse_identifier()?;

        // カラムリスト
        let mut columns = Vec::new();
        if self.buffer.check(TokenKind::LParen) {
            self.buffer.consume()?;
            while !self.buffer.check(TokenKind::RParen) {
                columns.push(self.parse_identifier()?);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
            if !self.buffer.check(TokenKind::RParen) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::RParen],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
            self.buffer.consume()?;
        }

        // VALUES or SELECT
        let source = if self.buffer.check(TokenKind::Values) {
            self.buffer.consume()?;
            let mut rows = Vec::new();
            loop {
                if !self.buffer.check(TokenKind::LParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::LParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.position,
                    ));
                }
                self.buffer.consume()?; // LEFT PAREN
                let mut values = Vec::new();
                while !self.buffer.check(TokenKind::RParen) {
                    let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                    values.push(expr_parser.parse()?);
                    if !self.buffer.consume_if(TokenKind::Comma)? {
                        break;
                    }
                }
                if !self.buffer.check(TokenKind::RParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::RParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.position,
                    ));
                }
                self.buffer.consume()?;
                rows.push(values);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
            InsertSource::Values(rows)
        } else if self.buffer.check(TokenKind::Default) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::Values) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::Values],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
            self.buffer.consume()?;
            InsertSource::DefaultValues
        } else {
            // SELECT
            let select_stmt = self.parse_select_statement()?;
            match select_stmt {
                Statement::Select(s) => InsertSource::Select(s),
                _ => {
                    return Err(ParseError::invalid_syntax(
                        "Expected SELECT statement".to_string(),
                        self.buffer.current()?.position,
                    ))
                }
            }
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Insert(Box::new(InsertStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            table,
            columns,
            source,
        })))
    }

    /// UPDATE文を解析
    pub(super) fn parse_update_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // UPDATE

        let table_ref = self.parse_table_reference()?;

        // SET
        if !self.buffer.check(TokenKind::Set) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Set],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?;

        // 代入リスト
        let mut assignments = Vec::new();
        loop {
            let column = self.parse_identifier()?;
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
            assignments.push(ColumnAssignment { column, value });
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        // FROM（オプション）
        let from_clause = if self.buffer.check(TokenKind::From) {
            Some(self.parse_from_clause()?)
        } else {
            None
        };

        // WHERE（オプション）
        let where_clause = if self.buffer.check(TokenKind::Where) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Update(Box::new(UpdateStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            table: table_ref,
            assignments,
            from_clause,
            where_clause,
        })))
    }

    /// DELETE文を解析
    pub(super) fn parse_delete_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // DELETE

        let table = if self.buffer.check(TokenKind::From) {
            self.buffer.consume()?;
            self.parse_identifier()?
        } else {
            // FROMなしの形式
            self.parse_identifier()?
        };

        // FROM（結合用、オプション）
        let from_clause = if self.buffer.check(TokenKind::From) {
            Some(self.parse_from_clause()?)
        } else {
            None
        };

        // WHERE（オプション）
        let where_clause = if self.buffer.check(TokenKind::Where) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Delete(Box::new(DeleteStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            table,
            from_clause,
            where_clause,
        })))
    }

    /// 変数代入パターンかチェックする
    ///
    /// `SELECT @var = expr` または `SELECT @var1 = expr1, @var2 = expr2` のパターンを検出する。
    pub(super) fn is_variable_assignment_pattern(&self) -> ParseResult<bool> {
        // 現在のトークンが LocalVar (@var) であるか確認
        if !matches!(self.buffer.current()?.kind, TokenKind::LocalVar) {
            return Ok(false);
        }

        // 次のトークンが代入演算子(=)であるか確認
        match self.buffer.peek(1) {
            Ok(next_token) => Ok(matches!(next_token.kind, TokenKind::Eq | TokenKind::Assign)),
            Err(_) => Ok(false),
        }
    }

    /// SELECT変数代入文を解析
    ///
    /// `SELECT @var = expr` または `SELECT @var1 = expr1, @var2 = expr2` の構文を解析する。
    pub(super) fn parse_variable_assignment(&mut self, start: u32) -> ParseResult<Statement> {
        use crate::ast::{Assignment, VariableAssignment};

        let mut assignments = Vec::new();

        loop {
            // 変数名 (@variable)
            if !matches!(self.buffer.current()?.kind, TokenKind::LocalVar) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::LocalVar],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }

            let variable = self.parse_identifier()?;

            // 代入演算子 (= または :=)
            if !matches!(
                self.buffer.current()?.kind,
                TokenKind::Eq | TokenKind::Assign
            ) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::Eq, TokenKind::Assign],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
            self.buffer.consume()?;

            // 式
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            let value = expr_parser.parse()?;

            assignments.push(Assignment { variable, value });

            // カンマで区切られた複数代入
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }

            // 次も変数代入パターンか確認
            if !self.is_variable_assignment_pattern()? {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::LocalVar],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
        }

        let end_span = self.buffer.current()?.span;
        Ok(Statement::VariableAssignment(Box::new(
            VariableAssignment {
                span: Span {
                    start,
                    end: end_span.end,
                },
                assignments,
            },
        )))
    }
}
