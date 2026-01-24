//! 二項演算子の処理

use crate::ast::BinaryOperator;
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

impl super::ExpressionParser<'_, '_> {
    /// 中置演算子の結合力を取得
    pub(super) fn get_infix_binding_power(
        kind: TokenKind,
    ) -> Option<(BindingPower, BinaryOperator)> {
        Some(match kind {
            // 論理OR
            TokenKind::Or => (BindingPower::LogicalOr, BinaryOperator::Or),
            // 論理AND
            TokenKind::And => (BindingPower::LogicalAnd, BinaryOperator::And),
            // IN, BETWEEN は比較演算子より低い優先順位
            TokenKind::In => (BindingPower::Is, BinaryOperator::In),
            TokenKind::Between => (BindingPower::Is, BinaryOperator::Between),
            // 比較演算子
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

    /// 二項演算子のspanを作成
    pub(super) fn span_for_binary(&self, left: Span, right: Span) -> Span {
        Span {
            start: left.start,
            end: right.end,
        }
    }
}
