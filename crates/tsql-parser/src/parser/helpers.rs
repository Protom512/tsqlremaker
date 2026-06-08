//! Parser helper/utility methods
//!
//! 識別子のパース、同期/エラー回復、再帰深度チェック、
//! カンマ区切りリストのパースなどの汎用ヘルパーメソッド。

use crate::ast::Identifier;
use crate::error::{ParseError, ParseResult};
use tsql_token::TokenKind;

impl<'src> super::Parser<'src> {
    /// 識別子を解析
    pub(super) fn parse_identifier(&mut self) -> ParseResult<Identifier> {
        let current = self.buffer.current()?;
        let span = current.span;

        // キーワードが識別子として使用可能かチェック
        let can_keyword_be_identifier = matches!(
            current.kind,
            TokenKind::Table
                | TokenKind::View
                | TokenKind::Proc
                | TokenKind::Function
                | TokenKind::Index
                | TokenKind::Key
                | TokenKind::Constraint
                | TokenKind::Trigger
                | TokenKind::Go
                | TokenKind::Goto
                | TokenKind::Label
        );

        let name = if current.kind == TokenKind::QuotedIdent {
            // [name] の形式
            &current.text[1..current.text.len() - 1]
        } else if current.kind == TokenKind::Ident
            || current.kind == TokenKind::LocalVar
            || current.kind == TokenKind::GlobalVar
            || current.kind == TokenKind::TempTable
            || current.kind == TokenKind::GlobalTempTable
            || can_keyword_be_identifier
        {
            current.text
        } else {
            return Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Ident,
                    TokenKind::QuotedIdent,
                    TokenKind::LocalVar,
                    TokenKind::TempTable,
                    TokenKind::GlobalTempTable,
                ],
                current.kind,
                current.position,
            ));
        };

        self.buffer.consume()?;

        Ok(Identifier {
            name: name.to_string(),
            span,
        })
    }

    /// EOFに達したか判定
    pub(super) fn is_at_eof(&self) -> bool {
        self.buffer.check(TokenKind::Eof)
    }

    /// 同期ポイントまでスキップしてエラー回復
    pub(super) fn synchronize(&mut self) {
        while !self.is_at_eof() {
            let kind = self.buffer.current().map(|t| t.kind);
            if matches!(
                kind,
                Ok(TokenKind::Semicolon)
                    | Ok(TokenKind::Select)
                    | Ok(TokenKind::Insert)
                    | Ok(TokenKind::Update)
                    | Ok(TokenKind::Delete)
                    | Ok(TokenKind::Create)
                    | Ok(TokenKind::End)
            ) {
                break;
            }
            let _ = self.buffer.consume();
        }
    }

    /// 再帰深度をチェック（ネストされる前に呼び出す）
    pub(super) fn check_depth_before_nesting(&self) -> ParseResult<()> {
        if self.depth + 1 > self.max_depth {
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

    /// カンマ区切りのリストをパース
    ///
    /// # Arguments
    ///
    /// * `parse_item` - 各アイテムをパースするクロージャ
    ///
    /// # Returns
    ///
    /// パースされたアイテムのベクター
    pub(super) fn parse_comma_separated<T, F>(&mut self, mut parse_item: F) -> ParseResult<Vec<T>>
    where
        F: FnMut(&mut Self) -> ParseResult<T>,
    {
        let mut items = Vec::new();
        loop {
            items.push(parse_item(self)?);
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }
        Ok(items)
    }

    /// エラーを消費して取得
    pub fn drain_errors(&mut self) -> Vec<ParseError> {
        std::mem::take(&mut self.errors)
    }
}
