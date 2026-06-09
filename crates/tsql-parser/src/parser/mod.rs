//! パーサーモジュール
//!
//! T-SQLの構文解析を行うメインパーサー。

mod control_flow;
mod ddl;
mod dml;
mod helpers;
mod misc;
mod select;

use crate::ast::*;
use crate::buffer::TokenBuffer;
use crate::error::{ParseError, ParseErrors, ParseResult, ParseResultWithErrors};
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// デフォルトの最大再帰深度
const DEFAULT_MAX_DEPTH: usize = 1000;

/// パーサーモード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParserMode {
    /// バッチモード（GOをバッチ区切りとして認識）
    #[default]
    BatchMode,
    /// 単一文モード（GOを識別子として扱う）
    SingleStatement,
}

/// パーサー構造
pub struct Parser<'src> {
    /// トークンバッファ
    buffer: TokenBuffer<'src>,
    /// パーサーモード
    mode: ParserMode,
    /// 収集されたエラー
    errors: Vec<ParseError>,
    /// 現在の再帰深度
    depth: usize,
    /// 最大再帰深度
    max_depth: usize,
}

impl<'src> Parser<'src> {
    /// 新しいパーサーを作成
    ///
    /// # Arguments
    ///
    /// * `input` - 解析するSQLソースコード
    #[must_use]
    pub fn new(input: &'src str) -> Self {
        let lexer = Lexer::new(input);
        let buffer = TokenBuffer::new(lexer);
        Self {
            buffer,
            mode: ParserMode::default(),
            errors: Vec::new(),
            depth: 0,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// パーサーモードを設定
    ///
    /// # Arguments
    ///
    /// * `mode` - パーサーモード
    #[must_use]
    pub const fn with_mode(mut self, mode: ParserMode) -> Self {
        self.mode = mode;
        self
    }

    /// 入力全体を解析
    ///
    /// # Returns
    ///
    /// 文のリスト、またはエラー
    pub fn parse(&mut self) -> ParseResult<Vec<Statement>> {
        let mut statements = Vec::new();

        while !self.is_at_eof() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    self.errors.push(e.clone());
                    self.synchronize();
                }
            }

            // セミコロンを消費
            let _ = self.buffer.consume_if(TokenKind::Semicolon);
        }

        // エラーがあった場合は最初のエラーを返す
        if !self.errors.is_empty() {
            return Err(self.errors[0].clone());
        }

        Ok(statements)
    }

    /// 入力全体を解析（エラー回復付き）
    ///
    /// エラー回復機能を使用して、構文エラーがあってもパースを継続し、
    /// 複数のエラーを一度に報告する。
    ///
    /// # Returns
    ///
    /// 文のリストとエラーリスト、または単一のエラー
    pub fn parse_with_errors(
        &mut self,
    ) -> ParseResultWithErrors<(Vec<Statement>, Vec<ParseError>)> {
        let mut statements = Vec::new();

        while !self.is_at_eof() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    self.errors.push(e.clone());
                    self.synchronize();
                    // Safety: always advance past the current token to prevent
                    // infinite loops when synchronize stops at a sync-point
                    // (e.g., END) that is not a valid statement starter.
                    if !self.is_at_eof() {
                        let _ = self.buffer.consume();
                    }
                }
            }

            // セミコロンを消費
            let _ = self.buffer.consume_if(TokenKind::Semicolon);
        }

        // エラーリストをクローンして返す
        let errors = self.errors.clone();

        // エラーがあった場合はParseErrorsを返す
        if !errors.is_empty() {
            return Err(ParseErrors::new(errors));
        }

        Ok((statements, Vec::new()))
    }

    /// 収集されたエラーを返す
    ///
    /// # Returns
    ///
    /// エラーリストへの参照を返す
    #[must_use]
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// エラーがあるかどうかを確認
    ///
    /// # Returns
    ///
    /// エラーがある場合はtrue
    #[must_use]
    pub const fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 単一の文を解析
    ///
    /// # Returns
    ///
    /// 文、またはエラー
    pub fn parse_statement(&mut self) -> ParseResult<Statement> {
        match self.buffer.current()?.kind {
            // SELECT文
            TokenKind::Select => self.parse_select_statement(),
            // INSERT文
            TokenKind::Insert => self.parse_insert_statement(),
            // UPDATE文
            TokenKind::Update => self.parse_update_statement(),
            // DELETE文
            TokenKind::Delete => self.parse_delete_statement(),
            // CREATE文
            TokenKind::Create => self.parse_create_statement(),
            // ALTER TABLE文
            TokenKind::Alter => self.parse_alter_statement(),
            // DECLARE文
            TokenKind::Declare => self.parse_declare_statement(),
            // SET文
            TokenKind::Set => self.parse_set_statement(),
            // IF文
            TokenKind::If => self.parse_if_statement(),
            // WHILE文
            TokenKind::While => self.parse_while_statement(),
            // BEGINブロック（BEGIN TRY は TRY...CATCH、BEGIN TRANSACTION はトランザクション）
            TokenKind::Begin => {
                if self.check_try_begin() {
                    self.parse_try_catch_statement()
                } else if self.check_transaction_begin() {
                    self.parse_transaction_statement()
                } else {
                    self.parse_block()
                }
            }
            // BREAK文
            TokenKind::Break => self.parse_break_statement(),
            // CONTINUE文
            TokenKind::Continue => self.parse_continue_statement(),
            // RETURN文
            TokenKind::Return => self.parse_return_statement(),
            // トランザクション制御（COMMIT, ROLLBACK, SAVE）
            TokenKind::Commit | TokenKind::Rollback | TokenKind::Save => {
                self.parse_transaction_statement()
            }
            // THROW 文
            TokenKind::Throw => self.parse_throw_statement(),
            // RAISERROR 文
            TokenKind::Raiserror => self.parse_raiserror_statement(),
            // EXEC/EXECUTE 文
            TokenKind::Exec | TokenKind::Execute => self.parse_exec_statement(),
            // GOバッチ区切り（BatchModeのみ）
            TokenKind::Go => {
                if matches!(self.mode, ParserMode::BatchMode) {
                    self.parse_batch_separator()
                } else {
                    // SingleStatementモードではGOを識別子として扱う
                    Err(ParseError::unexpected_token(
                        vec![
                            TokenKind::Select,
                            TokenKind::Insert,
                            TokenKind::Update,
                            TokenKind::Delete,
                            TokenKind::Create,
                            TokenKind::Declare,
                        ],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.position,
                    ))
                }
            }
            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Select,
                    TokenKind::Insert,
                    TokenKind::Update,
                    TokenKind::Delete,
                    TokenKind::Create,
                    TokenKind::Declare,
                ],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            )),
        }
    }
}

#[cfg(test)]
mod tests;
