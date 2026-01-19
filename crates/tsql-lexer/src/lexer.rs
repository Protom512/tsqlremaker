//! 字句解析器本体
//!
//! SQL ソースコードをトークンストリームに変換する。

use crate::cursor::Cursor;
use crate::error::LexError;
use tsql_token::{Position, Span, TokenKind};

/// トークン
///
/// Zero-copy でソースコードへの参照を保持するトークン。
#[derive(Debug, Clone, Copy)]
pub struct Token<'src> {
    /// トークン種別
    pub kind: TokenKind,
    /// ソースコードへの参照（コピーなし）
    pub text: &'src str,
    /// 位置情報
    pub span: Span,
}

impl<'src> Token<'src> {
    /// 新しい Token を作成する
    ///
    /// # Arguments
    ///
    /// * `kind` - トークン種別
    /// * `text` - ソースコードへの参照
    /// * `position` - 開始位置
    #[must_use]
    pub const fn new(kind: TokenKind, text: &'src str, position: Position) -> Self {
        let len = text.len() as u32;
        Self {
            kind,
            text,
            span: Span {
                start: position.offset,
                end: position.offset + len,
            },
        }
    }

    /// EOF トークンを作成する
    #[must_use]
    pub const fn eof() -> Self {
        Self {
            kind: TokenKind::Eof,
            text: "",
            span: Span { start: 0, end: 0 },
        }
    }
}

/// 字句解析器
///
/// ソースコードをトークンストリームに変換する。
pub struct Lexer<'src> {
    input: &'src str,
    cursor: Cursor<'src>,
    preserve_comments: bool,
}

impl<'src> Lexer<'src> {
    /// 新しい Lexer を作成する
    ///
    /// # Arguments
    ///
    /// * `input` - 字句解析するソースコード
    #[must_use]
    pub fn new(input: &'src str) -> Self {
        Self {
            input,
            cursor: Cursor::new(input),
            preserve_comments: false,
        }
    }

    /// コメントを保持するか設定する
    ///
    /// # Arguments
    ///
    /// * `preserve` - true の場合、コメントトークンを保持する
    pub fn with_comments(mut self, preserve: bool) -> Self {
        self.preserve_comments = preserve;
        self
    }

    /// 次のトークンを取得する
    ///
    /// # Returns
    ///
    /// 次のトークン、またはエラー
    pub fn next_token(&mut self) -> Result<Token<'src>, LexError> {
        self.skip_whitespace();

        if self.cursor.is_eof() {
            return Ok(Token::eof());
        }

        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        let ch = self.cursor.current().ok_or(LexError::UnexpectedEof {
            position: start_pos,
            expected: "token".to_string(),
        })?;

        match ch {
            // コメント
            '/' if self.cursor.peek() == Some('*') => self.read_block_comment(),
            '-' if self.cursor.peek() == Some('-') => self.read_line_comment(),

            // 変数プレフィックス
            '@' => self.read_at_variable(),
            '#' => self.read_hash_temp(),

            // 引用符付き識別子
            '[' => self.read_bracket_ident(),
            '"' => self.read_quoted_ident(),

            // 文字列リテラル
            '\'' => self.read_string(),

            // 数値
            '0'..='9' => self.read_number(),

            // 演算子
            '+' => self.read_plus(),
            '-' => self.read_minus(),
            '*' => self.read_star(),
            '/' => self.read_slash(),
            '%' => self.read_percent(),
            '=' => Ok(Token::new(TokenKind::Assign, "=", start_pos)),
            '<' => self.read_less_than(),
            '>' => self.read_greater_than(),
            '!' => self.read_bang(),
            '&' => Ok(Token::new(TokenKind::Ampersand, "&", start_pos)),
            '|' => self.read_pipe(),
            '^' => Ok(Token::new(TokenKind::Caret, "^", start_pos)),
            '~' => Ok(Token::new(TokenKind::Tilde, "~", start_pos)),
            '.' => self.read_dot(),

            // 区切り文字
            '(' => Ok(Token::new(TokenKind::LParen, "(", start_pos)),
            ')' => Ok(Token::new(TokenKind::RParen, ")", start_pos)),
            '{' => Ok(Token::new(TokenKind::LBrace, "{", start_pos)),
            '}' => Ok(Token::new(TokenKind::RBrace, "}", start_pos)),
            ']' => Ok(Token::new(TokenKind::RBracket, "]", start_pos)),
            ',' => Ok(Token::new(TokenKind::Comma, ",", start_pos)),
            ';' => Ok(Token::new(TokenKind::Semicolon, ";", start_pos)),
            ':' => Ok(Token::new(TokenKind::Colon, ":", start_pos)),

            // Unicode 文字列プレフィックス
            'U' | 'u' if self.cursor.peek() == Some('&') => self.read_unicode_string(),

            // 識別子またはキーワード
            c if is_ident_start(c) => self.read_ident_or_keyword(start_offset, start_pos),

            // 不正な文字
            c => Err(LexError::InvalidCharacter {
                ch: c,
                position: start_pos,
            }),
        }
    }

    /// 空白をスキップする
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.cursor.current() {
            if ch.is_whitespace() && ch != '\n' && ch != '\r' {
                self.cursor.bump();
            } else {
                break;
            }
        }
    }

    // 識別子またはキーワードの読み取り
    fn read_ident_or_keyword(
        &mut self,
        start_offset: usize,
        start_pos: Position,
    ) -> Result<Token<'src>, LexError> {
        while let Some(ch) = self.cursor.current() {
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                self.cursor.bump();
            } else {
                break;
            }
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];
        let kind = TokenKind::from_ident(text);

        Ok(Token::new(kind, text, start_pos))
    }

    // Task 4.1: ブロックコメントの読み取り（ネスト対応）
    fn read_block_comment(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        self.cursor.bump(); // '/'
        self.cursor.bump(); // '*'

        let mut depth = 1;

        while !self.cursor.is_eof() {
            match (self.cursor.current(), self.cursor.peek()) {
                (Some('/'), Some('*')) => {
                    // ネスト開始
                    depth += 1;
                    self.cursor.bump();
                    self.cursor.bump();
                }
                (Some('*'), Some('/')) => {
                    // ネスト終了
                    depth -= 1;
                    self.cursor.bump();
                    self.cursor.bump();
                    if depth == 0 {
                        break;
                    }
                }
                _ => {
                    self.cursor.bump();
                }
            }
        }

        if depth > 0 {
            return Err(LexError::UnterminatedBlockComment {
                start: start_pos,
                depth,
            });
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];

        if self.preserve_comments {
            Ok(Token::new(TokenKind::BlockComment, text, start_pos))
        } else {
            // コメントをスキップして次のトークンを返す
            self.next_token()
        }
    }

    // Task 4.2: ラインコメントの読み取り
    fn read_line_comment(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        self.cursor.bump(); // first '-'
        self.cursor.bump(); // second '-'

        while !self.cursor.is_eof() && self.cursor.current() != Some('\n') {
            self.cursor.bump();
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];

        if self.preserve_comments {
            Ok(Token::new(TokenKind::LineComment, text, start_pos))
        } else {
            // 改行を消費して次のトークンを返す
            if self.cursor.current() == Some('\n') {
                self.cursor.bump();
            }
            self.next_token()
        }
    }

    // Task 5.1: @ 変数プレフィックスの読み取り
    fn read_at_variable(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        self.cursor.bump(); // '@'

        let is_global = self.cursor.current() == Some('@');
        if is_global {
            self.cursor.bump(); // second '@'
        }

        // 識別子部分を読み取る
        while let Some(ch) = self.cursor.current() {
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                self.cursor.bump();
            } else {
                break;
            }
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];

        let kind = if is_global {
            TokenKind::GlobalVar
        } else {
            TokenKind::LocalVar
        };

        Ok(Token::new(kind, text, start_pos))
    }

    // Task 5.2: # 一時テーブルプレフィックスの読み取り
    fn read_hash_temp(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        self.cursor.bump(); // '#'

        let is_global = self.cursor.current() == Some('#');
        if is_global {
            self.cursor.bump(); // second '#'
        }

        // 識別子部分を読み取る
        while let Some(ch) = self.cursor.current() {
            if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                self.cursor.bump();
            } else {
                break;
            }
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];

        let kind = if is_global {
            TokenKind::GlobalTempTable
        } else {
            TokenKind::TempTable
        };

        Ok(Token::new(kind, text, start_pos))
    }

    // Task 9.1: 角括弧付き識別子の読み取り
    fn read_bracket_ident(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();

        self.cursor.bump(); // '['

        // Check if this is actually a quoted identifier or just a lone bracket
        // If the next char is ']' immediately, or we're at EOF, treat '[' as a delimiter
        match self.cursor.current() {
            None => {
                // EOF after '[', just return LBracket
                Ok(Token::new(TokenKind::LBracket, "[", start_pos))
            }
            Some(']') => {
                // Empty brackets [], treat '[' as a delimiter
                // The ']' will be parsed as RBracket in the next call
                Ok(Token::new(TokenKind::LBracket, "[", start_pos))
            }
            _ => {
                // Has content after '[', this is a quoted identifier
                let start_offset = start_pos.offset as usize;
                let mut found_closing = false;

                while !self.cursor.is_eof() {
                    match self.cursor.current() {
                        Some(']') => {
                            if self.cursor.peek() == Some(']') {
                                // エスケープ ]]
                                self.cursor.bump();
                                self.cursor.bump();
                            } else {
                                self.cursor.bump();
                                found_closing = true;
                                break;
                            }
                        }
                        _ => {
                            self.cursor.bump();
                        }
                    }
                }

                if !found_closing {
                    return Err(LexError::UnterminatedIdentifier {
                        start: start_pos,
                        bracket_type: crate::error::BracketType::Square,
                    });
                }

                let end_offset = self.cursor.position().offset as usize;
                let text = &self.input[start_offset..end_offset];

                Ok(Token::new(TokenKind::QuotedIdent, text, start_pos))
            }
        }
    }

    // Task 9.2: 二重引用符付き識別子の読み取り
    fn read_quoted_ident(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        self.cursor.bump(); // '"'

        let mut found_closing = false;

        while !self.cursor.is_eof() {
            match self.cursor.current() {
                Some('"') => {
                    if self.cursor.peek() == Some('"') {
                        // エスケープ ""
                        self.cursor.bump();
                        self.cursor.bump();
                    } else {
                        self.cursor.bump();
                        found_closing = true;
                        break;
                    }
                }
                _ => {
                    self.cursor.bump();
                }
            }
        }

        if !found_closing {
            return Err(LexError::UnterminatedIdentifier {
                start: start_pos,
                bracket_type: crate::error::BracketType::DoubleQuote,
            });
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];

        Ok(Token::new(TokenKind::QuotedIdent, text, start_pos))
    }

    // Task 6.1: 通常文字列リテラルの読み取り
    fn read_string(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        self.cursor.bump(); // opening quote

        let mut found_closing_quote = false;

        while !self.cursor.is_eof() {
            match self.cursor.current() {
                Some('\'') => {
                    // エスケープチェック（''）
                    if self.cursor.peek() == Some('\'') {
                        self.cursor.bump();
                        self.cursor.bump();
                    } else {
                        // 終了
                        self.cursor.bump();
                        found_closing_quote = true;
                        break;
                    }
                }
                _ => {
                    self.cursor.bump();
                }
            }
        }

        if !found_closing_quote {
            return Err(LexError::UnterminatedString {
                start: start_pos,
                quote_char: '\'',
            });
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];

        Ok(Token::new(TokenKind::String, text, start_pos))
    }

    // Task 7.1: 数値リテラルの読み取り
    fn read_number(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        // 16進数チェック
        if self.cursor.current() == Some('0')
            && self.cursor.peek() == Some('x')
        {
            return self.read_hex_number(start_pos, start_offset);
        }

        let mut has_dot = false;
        let mut has_exponent = false;

        while let Some(ch) = self.cursor.current() {
            if ch.is_ascii_digit() {
                self.cursor.bump();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                self.cursor.bump();
                // ドットの後に数字がない場合は範囲演算子
                if !self.cursor.current().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    // ドットを戻して整数として処理
                    break;
                }
            } else if (ch == 'e' || ch == 'E') && !has_exponent {
                has_exponent = true;
                self.cursor.bump();
                // 符号を許容
                if self.cursor.current() == Some('+') || self.cursor.current() == Some('-') {
                    self.cursor.bump();
                }
            } else {
                break;
            }
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];

        let kind = if has_dot || has_exponent {
            TokenKind::FloatLiteral
        } else {
            TokenKind::Number
        };

        Ok(Token::new(kind, text, start_pos))
    }

    // 16進数リテラルの読み取り（Task 7.1 の一部）
    fn read_hex_number(
        &mut self,
        start_pos: Position,
        start_offset: usize,
    ) -> Result<Token<'src>, LexError> {
        self.cursor.bump(); // '0'
        self.cursor.bump(); // 'x'

        while let Some(ch) = self.cursor.current() {
            if ch.is_ascii_hexdigit() {
                self.cursor.bump();
            } else {
                break;
            }
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];

        Ok(Token::new(TokenKind::HexString, text, start_pos))
    }

    // Task 6.2: Unicode 文字列の読み取り (U&'...')
    fn read_unicode_string(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        self.cursor.bump(); // 'U' or 'u'
        self.cursor.bump(); // '&'

        let quote = self.cursor.current().ok_or(LexError::UnexpectedEof {
            position: self.cursor.position(),
            expected: "quote".to_string(),
        })?;

        if quote != '\'' && quote != '"' {
            return Err(LexError::InvalidCharacter {
                ch: quote,
                position: self.cursor.position(),
            });
        }

        self.cursor.bump(); // opening quote

        while !self.cursor.is_eof() {
            match self.cursor.current() {
                Some(q) if q == quote => {
                    if self.cursor.peek() != Some(q) {
                        self.cursor.bump();
                        break;
                    }
                    // エスケープ
                    self.cursor.bump();
                    self.cursor.bump();
                }
                Some('\\') => {
                    // Unicode エスケープシーケンス
                    self.cursor.bump();
                    if self.cursor.current() == Some('+') {
                        self.cursor.bump();
                        // \+XXXXXX (6 hex digits)
                        for _ in 0..6 {
                            self.cursor.bump();
                        }
                    } else {
                        // \XXXX (4 hex digits)
                        for _ in 0..4 {
                            self.cursor.bump();
                        }
                    }
                }
                _ => {
                    self.cursor.bump();
                }
            }
        }

        let end_offset = self.cursor.position().offset as usize;
        let text = &self.input[start_offset..end_offset];

        Ok(Token::new(TokenKind::UnicodeString, text, start_pos))
    }

    fn read_plus(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        if self.cursor.current() == Some('=') {
            self.cursor.bump();
            Ok(Token::new(TokenKind::PlusAssign, "+=", pos))
        } else {
            Ok(Token::new(TokenKind::Plus, "+", pos))
        }
    }

    fn read_minus(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        if self.cursor.current() == Some('=') {
            self.cursor.bump();
            Ok(Token::new(TokenKind::MinusAssign, "-=", pos))
        } else {
            Ok(Token::new(TokenKind::Minus, "-", pos))
        }
    }

    fn read_star(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        if self.cursor.current() == Some('=') {
            self.cursor.bump();
            Ok(Token::new(TokenKind::StarAssign, "*=", pos))
        } else {
            Ok(Token::new(TokenKind::Star, "*", pos))
        }
    }

    fn read_slash(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        if self.cursor.current() == Some('=') {
            self.cursor.bump();
            Ok(Token::new(TokenKind::SlashAssign, "/=", pos))
        } else {
            Ok(Token::new(TokenKind::Slash, "/", pos))
        }
    }

    fn read_less_than(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        match self.cursor.current() {
            Some('=') => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::Le, "<=", pos))
            }
            Some('>') => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::NeAlt, "<>", pos))
            }
            _ => Ok(Token::new(TokenKind::Lt, "<", pos)),
        }
    }

    fn read_greater_than(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        if self.cursor.current() == Some('=') {
            self.cursor.bump();
            Ok(Token::new(TokenKind::Ge, ">=", pos))
        } else {
            Ok(Token::new(TokenKind::Gt, ">", pos))
        }
    }

    fn read_bang(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        match self.cursor.current() {
            Some('=') => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::Ne, "!=", pos))
            }
            Some('<') => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::NotLt, "!<", pos))
            }
            Some('>') => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::NotGt, "!>", pos))
            }
            _ => Err(LexError::InvalidCharacter {
                ch: '!',
                position: pos,
            }),
        }
    }

    fn read_pipe(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        if self.cursor.current() == Some('|') {
            self.cursor.bump();
            Ok(Token::new(TokenKind::Concat, "||", pos))
        } else {
            Ok(Token::new(TokenKind::Pipe, "|", pos))
        }
    }

    fn read_dot(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        if self.cursor.current() == Some('.') {
            self.cursor.bump();
            Ok(Token::new(TokenKind::DotDot, "..", pos))
        } else {
            Ok(Token::new(TokenKind::Dot, ".", pos))
        }
    }

    fn read_percent(&mut self) -> Result<Token<'src>, LexError> {
        let pos = self.cursor.position();
        self.cursor.bump();
        // %= is not a valid SQL operator, so we just return Percent
        Ok(Token::new(TokenKind::Percent, "%", pos))
    }
}

/// 識別子の開始文字かどうかを判定する
fn is_ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_new() {
        let pos = Position::new(1, 1, 0);
        let token = Token::new(TokenKind::Select, "SELECT", pos);
        assert_eq!(token.kind, TokenKind::Select);
        assert_eq!(token.text, "SELECT");
        assert_eq!(token.span.start, 0);
        assert_eq!(token.span.end, 6);
    }

    #[test]
    fn test_token_eof() {
        let eof = Token::eof();
        assert_eq!(eof.kind, TokenKind::Eof);
        assert_eq!(eof.text, "");
    }

    #[test]
    fn test_lexer_new() {
        let lexer = Lexer::new("SELECT * FROM users");
        assert!(!lexer.cursor.is_eof());
    }

    #[test]
    fn test_lexer_with_comments() {
        let lexer = Lexer::new("SELECT * FROM users").with_comments(true);
        assert!(lexer.preserve_comments);
    }

    #[test]
    fn test_is_ident_start() {
        assert!(is_ident_start('a'));
        assert!(is_ident_start('_'));
        assert!(!is_ident_start('1'));
        assert!(!is_ident_start('@'));
    }
}
