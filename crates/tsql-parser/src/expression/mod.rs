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

use crate::ast::{AstNode, BinaryOperator, Expression};
use crate::buffer::TokenBuffer;
use crate::error::{ParseError, ParseResult};
use tsql_token::TokenKind;

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
}
