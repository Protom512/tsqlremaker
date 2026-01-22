//! トークンバッファモジュール
//!
//! Lexerからのトークンストリームに対して先読み機能を提供する。

use crate::error::{ParseError, ParseResult};
use tsql_lexer::{Lexer, Token};
use tsql_token::{Span, TokenKind};

/// トークンバッファ
///
/// 固定サイズの循環バッファで、トークンの先読み（peek）を提供する。
pub struct TokenBuffer<'src> {
    /// Lexer
    lexer: Lexer<'src>,
    /// 循環バッファ（サイズ3）
    buffer: [Option<Token<'src>>; 3],
    /// 現在の読み取り位置（0-2）
    cursor: usize,
    /// バッファに読み込まれているトークン数
    filled: usize,
    /// 入力の終わりに達したか
    eof_reached: bool,
}

impl<'src> TokenBuffer<'src> {
    /// 新しいトークンバッファを作成
    ///
    /// # Arguments
    ///
    /// * `lexer` - トークンソースとしてのLexer
    #[must_use]
    pub fn new(mut lexer: Lexer<'src>) -> Self {
        let mut buffer = [None, None, None];
        let mut filled = 0;
        let mut eof_reached = false;

        // 最初のトークンをプリフィル
        for slot in &mut buffer {
            match lexer.next_token() {
                Ok(token) if token.kind == TokenKind::Eof => {
                    eof_reached = true;
                    break;
                }
                Ok(token) => {
                    *slot = Some(token);
                    filled += 1;
                }
                // Lexerエラーは記録されているが、ここでは無視して継続
                Err(_) => {
                    eof_reached = true;
                    break;
                }
            }
        }

        Self {
            lexer,
            buffer,
            cursor: 0,
            filled,
            eof_reached,
        }
    }

    /// 現在のトークンを返す（消費しない）
    ///
    /// # Returns
    ///
    /// 現在のトークン、またはエラー
    pub fn current(&self) -> ParseResult<&Token<'src>> {
        self.peek(0)
    }

    /// n番目の先読みトークンを返す（0=現在）
    ///
    /// # Arguments
    ///
    /// * `n` - 先読み位置
    ///
    /// # Returns
    ///
    /// n番目のトークン、またはエラー
    pub fn peek(&self, n: usize) -> ParseResult<&Token<'src>> {
        if self.cursor + n < self.filled {
            let idx = (self.cursor + n) % 3;
            self.buffer[idx]
                .as_ref()
                .ok_or_else(|| ParseError::unexpected_eof("token".to_string(), position_at_eof()))
        } else {
            // EOFトークンを返す
            Ok(&Token {
                kind: TokenKind::Eof,
                text: "",
                span: Span { start: 0, end: 0 },
            })
        }
    }

    /// 現在のトークンを消費して次に進む
    ///
    /// # Returns
    ///
    /// 消費したトークン、またはエラー
    pub fn consume(&mut self) -> ParseResult<Token<'src>> {
        if self.cursor < self.filled {
            let idx = self.cursor % 3;
            self.cursor += 1;

            // バッファから取り出し
            let token = self.buffer[idx].take().ok_or_else(|| {
                ParseError::unexpected_eof("token".to_string(), position_at_eof())
            })?;

            // バッファをリフレーム
            self.refill_buffer()?;

            Ok(token)
        } else {
            // EOF
            Ok(Token {
                kind: TokenKind::Eof,
                text: "",
                span: Span { start: 0, end: 0 },
            })
        }
    }

    /// 現在のトークンが指定された種別かチェック
    ///
    /// # Arguments
    ///
    /// * `kind` - チェックするトークン種別
    ///
    /// # Returns
    ///
    /// 一致する場合はtrue
    #[must_use]
    pub fn check(&self, kind: TokenKind) -> bool {
        self.current().map(|t| t.kind == kind).unwrap_or(false)
    }

    /// 指定された種別の場合に消費
    ///
    /// # Arguments
    ///
    /// * `kind` - チェックするトークン種別
    ///
    /// # Returns
    ///
    /// 消費した場合はtrue、そうでない場合はfalse
    pub fn consume_if(&mut self, kind: TokenKind) -> ParseResult<bool> {
        if self.check(kind) {
            self.consume()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// バッファをリフレーム
    fn refill_buffer(&mut self) -> ParseResult<()> {
        while !self.eof_reached && self.filled - self.cursor < 3 {
            match self.lexer.next_token() {
                Ok(token) if token.kind == TokenKind::Eof => {
                    self.eof_reached = true;
                    break;
                }
                Ok(token) => {
                    let idx = self.filled % 3;
                    self.buffer[idx] = Some(token);
                    self.filled += 1;
                }
                Err(_) => {
                    self.eof_reached = true;
                    break;
                }
            }
        }

        // カーソルがバッファサイズを超えた場合、バッファをシフト
        if self.cursor >= 3 {
            let shift = self.cursor / 3 * 3;
            // 新しいバッファを作成してコピー
            let mut new_buffer = [None, None, None];
            for (i, slot) in new_buffer.iter_mut().enumerate() {
                if self.cursor + i < self.filled {
                    let old_idx = (self.cursor + i) % 3;
                    *slot = self.buffer[old_idx].take();
                }
            }
            self.buffer = new_buffer;
            self.filled -= shift;
            self.cursor -= shift;
        }

        Ok(())
    }
}

/// EOF時のダミー位置
fn position_at_eof() -> tsql_token::Position {
    tsql_token::Position {
        line: 0,
        column: 0,
        offset: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsql_lexer::Lexer;

    #[test]
    fn test_buffer_returns_current_token() {
        let lexer = Lexer::new("SELECT * FROM users");
        let buffer = TokenBuffer::new(lexer);

        let current = buffer.current().unwrap();
        assert_eq!(current.kind, TokenKind::Select);
    }

    #[test]
    fn test_buffer_peek_returns_lookahead() {
        let lexer = Lexer::new("SELECT * FROM");
        let buffer = TokenBuffer::new(lexer);

        // peek(0) = current
        assert_eq!(buffer.peek(0).unwrap().kind, TokenKind::Select);
        // peek(1) = next
        assert_eq!(buffer.peek(1).unwrap().kind, TokenKind::Star);
        // peek(2) = next next
        assert_eq!(buffer.peek(2).unwrap().kind, TokenKind::From);
    }

    #[test]
    fn test_buffer_consume_advances() {
        let lexer = Lexer::new("SELECT * FROM");
        let mut buffer = TokenBuffer::new(lexer);

        // 最初はSELECT
        assert_eq!(buffer.current().unwrap().kind, TokenKind::Select);

        // 消費して次へ
        buffer.consume().unwrap();
        assert_eq!(buffer.current().unwrap().kind, TokenKind::Star);

        // もう一度消費
        buffer.consume().unwrap();
        assert_eq!(buffer.current().unwrap().kind, TokenKind::From);
    }

    #[test]
    fn test_buffer_check_kind() {
        let lexer = Lexer::new("SELECT");
        let buffer = TokenBuffer::new(lexer);

        assert!(buffer.check(TokenKind::Select));
        assert!(!buffer.check(TokenKind::From));
    }

    #[test]
    fn test_buffer_consume_if() {
        let lexer = Lexer::new("SELECT FROM");
        let mut buffer = TokenBuffer::new(lexer);

        // 一致するので消費
        assert!(buffer.consume_if(TokenKind::Select).unwrap());
        assert_eq!(buffer.current().unwrap().kind, TokenKind::From);

        // 一致しないので消費しない
        assert!(!buffer.consume_if(TokenKind::Where).unwrap());
        assert_eq!(buffer.current().unwrap().kind, TokenKind::From);
    }

    #[test]
    fn test_buffer_eof_handling() {
        let lexer = Lexer::new("SELECT");
        let mut buffer = TokenBuffer::new(lexer);

        buffer.consume().unwrap(); // SELECT
        let eof_token = buffer.consume().unwrap();
        assert_eq!(eof_token.kind, TokenKind::Eof);

        // 複数回EOFを取得
        let eof_token2 = buffer.consume().unwrap();
        assert_eq!(eof_token2.kind, TokenKind::Eof);
    }
}
