//! 式パーサーモジュール
//!
//! プラット解析アルゴリズムで式を構文解析する。

mod binary;
mod function;
mod prefix;
mod special;

#[cfg(test)]
mod tests;

pub use binary::BindingPower;

use crate::ast::{AstNode, BinaryOperator, Expression, SelectItem, SelectStatement};
use crate::buffer::TokenBuffer;
use crate::error::{ParseError, ParseResult};
use tsql_token::{Span, TokenKind};

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

    /// 単純式を解析（前置演算子のみ、中置演算子を含まない）
    ///
    /// TOP句など、中置演算子を含まない単純な式を解析する場合に使用します。
    ///
    /// # Returns
    ///
    /// 解析された式、またはエラー
    pub fn parse_simple(&mut self) -> ParseResult<Expression> {
        self.check_depth()?;
        self.parse_prefix()
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

        // 中置演算子と特殊式を解析
        loop {
            let current = self.buffer.current()?;

            // NOT [IN | BETWEEN | LIKE] は特殊式として処理
            if self.buffer.check(TokenKind::Not) {
                // 先読みして NOT の後に何が来るかチェック
                if self.buffer.peek(1).is_ok_and(|t| {
                    matches!(t.kind, TokenKind::In | TokenKind::Between | TokenKind::Like)
                }) {
                    left = self.parse_not_special_expression(left)?;
                    continue;
                }
            }

            // ISは特殊式として処理（ISトークンはparse_is_expressionで消費）
            if self.buffer.check(TokenKind::Is) {
                left = self.parse_is_expression(left)?;
                continue;
            }
            // LIKEは特殊式として処理（LIKEトークンを先に消費）
            if self.buffer.check(TokenKind::Like) {
                self.buffer.consume()?;
                left = self.parse_like_expression(left, false)?;
                continue;
            }

            let (op_bp, op) = match Self::get_infix_binding_power(current.kind) {
                Some(bp) => bp,
                None => break,
            };

            if op_bp < min_bp {
                break;
            }

            // BETWEENは特殊な3項演算子として処理
            if op == BinaryOperator::Between {
                self.buffer.consume()?;
                left = self.parse_between_expression(left, false)?;
                continue;
            }

            // INは特殊な演算子として処理
            if matches!(op, BinaryOperator::In) {
                self.buffer.consume()?;
                left = self.parse_in_expression(left, false)?;
                continue;
            }

            self.buffer.consume()?;
            let right = self.parse_bp(op_bp)?;

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

    /// サブクエリとしてSELECT文を解析
    ///
    /// スカラサブクエリ、EXISTS、INサブクエリなどで使用されます。
    fn parse_subquery_select(&mut self) -> ParseResult<SelectStatement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // SELECT

        let mut distinct = false;

        // DISTINCT
        if self.buffer.consume_if(TokenKind::Distinct)? {
            distinct = true;
        }

        // カラムリスト
        let mut columns = Vec::new();
        loop {
            columns.push(self.parse_subquery_select_item()?);

            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        // FROM
        let from = if self.buffer.check(TokenKind::From) {
            Some(self.parse_subquery_from_clause()?)
        } else {
            None
        };

        // WHERE
        let where_clause = if self.buffer.check(TokenKind::Where) {
            self.buffer.consume()?;
            Some(self.parse()?)
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
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;

            let mut items = Vec::new();
            loop {
                items.push(self.parse()?);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
            items
        } else {
            Vec::new()
        };

        // HAVING
        let having = if self.buffer.check(TokenKind::Having) {
            self.buffer.consume()?;
            Some(self.parse()?)
        } else {
            None
        };

        // ORDER BY (サブクエリでは通常使用されないが、実装)
        let order_by = if self.buffer.check(TokenKind::Order) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::By) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::By],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;

            let mut items = Vec::new();
            loop {
                let expr = self.parse()?;
                let asc = if self.buffer.check(TokenKind::Asc) {
                    self.buffer.consume()?;
                    true
                } else if self.buffer.check(TokenKind::Desc) {
                    self.buffer.consume()?;
                    false
                } else {
                    true
                };
                items.push(crate::ast::OrderByItem { expr, asc });

                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
            items
        } else {
            Vec::new()
        };

        let end_span = self.buffer.current()?.span;
        Ok(SelectStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            distinct,
            top: None, // サブクエリではTOPは非対応
            columns,
            from,
            where_clause,
            group_by,
            having,
            order_by,
            limit: None,
        })
    }

    /// サブクエリとしてSELECT文を解析（Statementラッパー）
    fn parse_subquery_select_statement(&mut self) -> ParseResult<crate::ast::Statement> {
        let select_stmt = self.parse_subquery_select()?;
        Ok(crate::ast::Statement::Select(Box::new(select_stmt)))
    }

    /// サブクエリ内のSELECTアイテムを解析
    fn parse_subquery_select_item(&mut self) -> ParseResult<SelectItem> {
        // ワイルドカード
        if self.buffer.check(TokenKind::Star) {
            self.buffer.consume()?;
            return Ok(SelectItem::Wildcard);
        }

        let expr = self.parse()?;

        // オプションの別名
        let alias = if self.buffer.check(TokenKind::As) {
            self.buffer.consume()?;
            let text = self.buffer.current()?.text;
            let span = self.buffer.current()?.span;
            self.buffer.consume()?;
            Some(crate::ast::Identifier {
                name: text.to_string(),
                span,
            })
        } else if self.buffer.check(TokenKind::Ident) {
            let text = self.buffer.current()?.text;
            let span = self.buffer.current()?.span;
            self.buffer.consume()?;
            Some(crate::ast::Identifier {
                name: text.to_string(),
                span,
            })
        } else {
            None
        };

        Ok(SelectItem::Expression(expr, alias))
    }

    /// サブクエリ内のFROM句を解析
    fn parse_subquery_from_clause(&mut self) -> ParseResult<crate::ast::FromClause> {
        self.buffer.consume()?; // FROM

        let mut tables = Vec::new();
        loop {
            // 派生テーブル（サブクエリ）の検出
            if self.buffer.check(TokenKind::LParen) {
                let start = self.buffer.current()?.span.start;
                self.buffer.consume()?; // LParen

                // サブクエリを解析
                let select_stmt = match self.parse_subquery_select_statement()? {
                    crate::ast::Statement::Select(select) => select,
                    _ => {
                        return Err(ParseError::invalid_syntax(
                            "Expected SELECT statement in subquery".to_string(),
                            self.buffer.current()?.span,
                        ))
                    }
                };

                // 右括弧を期待
                if !self.buffer.check(TokenKind::RParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::RParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?; // RParen

                // オプションの別名
                let alias = if self.buffer.check(TokenKind::As) {
                    self.buffer.consume()?;
                    let text = self.buffer.current()?.text;
                    let span = self.buffer.current()?.span;
                    self.buffer.consume()?;
                    Some(crate::ast::Identifier {
                        name: text.to_string(),
                        span,
                    })
                } else if self.buffer.check(TokenKind::Ident) {
                    let text = self.buffer.current()?.text;
                    let span = self.buffer.current()?.span;
                    self.buffer.consume()?;
                    Some(crate::ast::Identifier {
                        name: text.to_string(),
                        span,
                    })
                } else {
                    None
                };

                let end_span = self.buffer.current()?.span;
                tables.push(crate::ast::TableReference::Subquery {
                    query: select_stmt,
                    alias,
                    span: Span {
                        start,
                        end: end_span.end,
                    },
                });
            } else {
                // 通常のテーブル参照
                let start = self.buffer.current()?.span.start;
                let text = self.buffer.current()?.text;
                let span = self.buffer.current()?.span;
                self.buffer.consume()?;

                let name = crate::ast::Identifier {
                    name: text.to_string(),
                    span,
                };

                let alias = if self.buffer.check(TokenKind::As) {
                    self.buffer.consume()?;
                    let text = self.buffer.current()?.text;
                    let span = self.buffer.current()?.span;
                    self.buffer.consume()?;
                    Some(crate::ast::Identifier {
                        name: text.to_string(),
                        span,
                    })
                } else if self.buffer.check(TokenKind::Ident) {
                    let text = self.buffer.current()?.text;
                    let span = self.buffer.current()?.span;
                    self.buffer.consume()?;
                    Some(crate::ast::Identifier {
                        name: text.to_string(),
                        span,
                    })
                } else {
                    None
                };

                tables.push(crate::ast::TableReference::Table {
                    name,
                    alias,
                    span: Span {
                        start,
                        end: self.buffer.current()?.span.end,
                    },
                });
            }

            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        Ok(crate::ast::FromClause {
            tables,
            joins: Vec::new(),
        })
    }
}
