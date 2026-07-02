//! SELECT statement parsing methods.

use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use crate::expression::ExpressionParser;
use tsql_token::{Span, TokenKind};

impl<'src> super::Parser<'src> {
    /// SELECT文を解析
    pub(super) fn parse_select_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // SELECT

        let mut distinct = false;
        let mut top = None;

        // DISTINCT
        if self.buffer.consume_if(TokenKind::Distinct)? {
            distinct = true;
        }

        // TOP
        if self.buffer.check(TokenKind::Top) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            // TOP句は原子式のみを許可（中置演算子を含まない）
            top = Some(expr_parser.parse_atomic()?);
        }

        // 変数代入パターンの検出: SELECT @var = expr
        // 先読みして @var = パターンかチェック
        if self.is_variable_assignment_pattern()? {
            return self.parse_variable_assignment(start);
        }

        // カラムリスト
        let columns = self.parse_comma_separated(|this| this.parse_select_item())?;

        // FROM
        let from = if self.buffer.check(TokenKind::From) {
            Some(self.parse_from_clause()?)
        } else {
            None
        };

        // WHERE
        let where_clause = if self.buffer.check(TokenKind::Where) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        // GROUP BY
        let group_by = if self.buffer.check(TokenKind::Group) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::By) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::By],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
            self.buffer.consume()?;
            self.parse_comma_separated(|this| {
                let mut expr_parser = ExpressionParser::new(&mut this.buffer);
                expr_parser.parse()
            })?
        } else {
            Vec::new()
        };

        // HAVING
        let having = if self.buffer.check(TokenKind::Having) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        // ORDER BY
        let order_by = if self.buffer.check(TokenKind::Order) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::By) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::By],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
            self.buffer.consume()?;
            self.parse_comma_separated(|this| this.parse_order_by_item())?
        } else {
            Vec::new()
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Select(Box::new(SelectStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            distinct,
            top,
            columns,
            from,
            where_clause,
            group_by,
            having,
            order_by,
            limit: None,
        })))
    }

    /// SELECTアイテムを解析
    pub(super) fn parse_select_item(&mut self) -> ParseResult<SelectItem> {
        // ワイルドカード
        if self.buffer.check(TokenKind::Star) {
            self.buffer.consume()?;
            return Ok(SelectItem::Wildcard);
        }

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let expr = expr_parser.parse()?;

        // table.* の形式
        if self.buffer.check(TokenKind::Dot) {
            self.buffer.consume()?;
            if self.buffer.check(TokenKind::Star) {
                self.buffer.consume()?;
                if let Expression::Identifier(ident) = expr {
                    return Ok(SelectItem::QualifiedWildcard(ident));
                }
            }
        }

        // 別名（ASまたは暗黙）
        let alias = if self.buffer.check(TokenKind::As) {
            self.buffer.consume()?;
            Some(self.parse_identifier()?)
        } else if self.buffer.check(TokenKind::Ident) {
            // 暗黙の別名
            Some(self.parse_identifier()?)
        } else {
            None
        };

        Ok(SelectItem::Expression(expr, alias))
    }

    /// FROM句を解析
    pub(super) fn parse_from_clause(&mut self) -> ParseResult<FromClause> {
        self.buffer.consume()?; // FROM

        // テーブル参照リスト（カンマ区切り）
        let mut tables = Vec::new();
        loop {
            tables.push(self.parse_table_reference()?);

            // カンマで区切られた複数テーブル
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        // JOINを解析
        let mut joins = Vec::new();
        while self.is_join_keyword() {
            joins.push(self.parse_join()?);
        }

        Ok(FromClause { tables, joins })
    }

    /// 現在のトークンがJOINキーワードか判定
    pub(super) fn is_join_keyword(&self) -> bool {
        match self.buffer.current() {
            Ok(token) => {
                matches!(
                    token.kind,
                    TokenKind::Inner
                        | TokenKind::Left
                        | TokenKind::Right
                        | TokenKind::Full
                        | TokenKind::Cross
                        | TokenKind::Join
                )
            }
            Err(_) => false,
        }
    }

    /// JOINを解析
    pub(super) fn parse_join(&mut self) -> ParseResult<Join> {
        let start = self.buffer.current()?.span.start;

        // JOIN種別を判定
        let join_type = self.parse_join_type()?;

        // JOINキーワードを消費（INNER/LEFT/... JOIN の場合）
        if !self.buffer.check(TokenKind::Join) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Join],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?;

        // 結合するテーブル
        let table = self.parse_table_reference()?;

        // ON条件
        let on_condition = if self.buffer.check(TokenKind::On) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        // USING句のパース
        let using_columns = if self.buffer.check(TokenKind::Using) {
            self.buffer.consume()?; // USING
                                    // USING (col1, col2, ...)
            if !self.buffer.check(TokenKind::LParen) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::LParen],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
            self.buffer.consume()?; // LParen

            let mut columns = Vec::new();
            loop {
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
            self.buffer.consume()?; // RParen

            columns
        } else {
            Vec::new()
        };

        let end_span = self.buffer.current()?.span;
        Ok(Join {
            join_type,
            table,
            on_condition,
            using_columns,
            span: Span {
                start,
                end: end_span.end,
            },
        })
    }

    /// JOIN種別を解析
    pub(super) fn parse_join_type(&mut self) -> ParseResult<JoinType> {
        let current = self.buffer.current()?;
        match current.kind {
            TokenKind::Inner => {
                self.buffer.consume()?;
                Ok(JoinType::Inner)
            }
            TokenKind::Left => {
                self.buffer.consume()?;
                if self.buffer.check(TokenKind::Outer) {
                    self.buffer.consume()?;
                    Ok(JoinType::LeftOuter)
                } else {
                    Ok(JoinType::Left)
                }
            }
            TokenKind::Right => {
                self.buffer.consume()?;
                if self.buffer.check(TokenKind::Outer) {
                    self.buffer.consume()?;
                    Ok(JoinType::RightOuter)
                } else {
                    Ok(JoinType::Right)
                }
            }
            TokenKind::Full => {
                self.buffer.consume()?;
                if self.buffer.check(TokenKind::Outer) {
                    self.buffer.consume()?;
                    Ok(JoinType::FullOuter)
                } else {
                    Ok(JoinType::Full)
                }
            }
            TokenKind::Cross => {
                self.buffer.consume()?;
                Ok(JoinType::Cross)
            }
            TokenKind::Join => {
                // 単独のJOINはINNER JOINとして扱う
                Ok(JoinType::Inner)
            }
            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Inner,
                    TokenKind::Left,
                    TokenKind::Right,
                    TokenKind::Full,
                    TokenKind::Cross,
                    TokenKind::Join,
                ],
                current.kind,
                current.position,
            )),
        }
    }

    /// テーブル参照を解析
    pub(super) fn parse_table_reference(&mut self) -> ParseResult<TableReference> {
        let start = self.buffer.current()?.span.start;

        // サブクエリ（導出テーブル）の検出
        if self.buffer.check(TokenKind::LParen) {
            self.buffer.consume()?; // LParen

            // サブクエリを解析
            let select_stmt = match self.parse_select_statement()? {
                Statement::Select(select) => select,
                _ => {
                    return Err(ParseError::invalid_syntax(
                        "Expected SELECT statement in subquery".to_string(),
                        self.buffer.current()?.position,
                    ))
                }
            };

            // 右括弧を期待
            if !self.buffer.check(TokenKind::RParen) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::RParen],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
            self.buffer.consume()?; // RParen

            // オプションの別名
            let alias = if self.buffer.check(TokenKind::As) {
                self.buffer.consume()?;
                Some(self.parse_identifier()?)
            } else if self.buffer.check(TokenKind::Ident) {
                Some(self.parse_identifier()?)
            } else {
                None
            };

            let end_span = self.buffer.current()?.span;
            return Ok(TableReference::Subquery {
                query: select_stmt,
                alias,
                span: Span {
                    start,
                    end: end_span.end,
                },
            });
        }

        // 通常のテーブル参照
        let name = self.parse_identifier()?;

        let alias = if self.buffer.check(TokenKind::As) {
            self.buffer.consume()?;
            Some(self.parse_identifier()?)
        } else if self.buffer.check(TokenKind::Ident) {
            Some(self.parse_identifier()?)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;
        Ok(TableReference::Table {
            name,
            alias,
            span: Span {
                start,
                end: end_span.end,
            },
        })
    }

    /// ORDER BYアイテムを解析
    pub(super) fn parse_order_by_item(&mut self) -> ParseResult<OrderByItem> {
        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let expr = expr_parser.parse()?;

        let asc = if self.buffer.check(TokenKind::Asc) {
            self.buffer.consume()?;
            true
        } else if self.buffer.check(TokenKind::Desc) {
            self.buffer.consume()?;
            false
        } else {
            true // デフォルトはASC
        };

        Ok(OrderByItem { expr, asc })
    }
}
