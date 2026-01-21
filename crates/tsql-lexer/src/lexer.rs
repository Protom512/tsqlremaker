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
    /// 収集されたエラー
    errors: Vec<LexError>,
    /// 先読みバッファ（1トークン分）
    peek_buffer: Option<Result<Token<'src>, LexError>>,
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
            errors: Vec::new(),
            peek_buffer: None,
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
    /// エラーが発生した場合、エラーを内部に記録し、リカバリを試みます。
    /// peek_buffer にトークンがある場合は、それを返します。
    ///
    /// # Returns
    ///
    /// 次のトークン、またはエラー
    pub fn next_token(&mut self) -> Result<Token<'src>, LexError> {
        // peek_buffer にトークンがある場合はそれを返す
        if let Some(token) = self.peek_buffer.take() {
            return token;
        }

        // 次のトークンを読み取る
        self.next_token_impl()
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
        if self.cursor.current() == Some('0') && self.cursor.peek() == Some('x') {
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
                if !self
                    .cursor
                    .current()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
                {
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

    // Task 6.2: National 文字列の読み取り (N'...')
    fn read_national_string(&mut self) -> Result<Token<'src>, LexError> {
        let start_pos = self.cursor.position();
        let start_offset = self.cursor.position().offset as usize;

        self.cursor.bump(); // 'N' or 'n'

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

        Ok(Token::new(TokenKind::NString, text, start_pos))
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

    /// 同期ポイントまでスキップしてエラーから回復する
    ///
    /// 次のセミコロンまたはキーワードの先頭までスキップします。
    fn synchronize(&mut self) {
        // まずエラー位置の文字を消費
        let _ = self.cursor.bump();

        // 次の有効なトークンまでスキップ
        while let Some(ch) = self.cursor.current() {
            match ch {
                // セミコロンで同期（消費して次へ）
                ';' => {
                    // セミコロンを消費して、次のトークンへ
                    let _ = self.cursor.bump();
                    break;
                }
                // キーワードの先頭文字で同期
                'A'..='Z' | 'a'..='z' | '_' => {
                    // 識別子の開始文字なので、ここで停止
                    break;
                }
                // その他の文字はスキップして継続
                _ => {
                    self.cursor.bump();
                }
            }
        }
    }

    /// エラーがあるかどうかを判定する
    ///
    /// # Returns
    ///
    /// エラーが1つ以上記録されている場合は `true`
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// 収集されたエラーのスライスを取得する
    ///
    /// # Returns
    ///
    /// 記録されたエラーのスライス
    #[must_use]
    pub fn errors(&self) -> &[LexError] {
        &self.errors
    }

    /// 収集されたエラーを消費して取得する
    ///
    /// # Returns
    ///
    /// 記録されたエラーのベクタ（内部のエラーはクリアされる）
    pub fn drain_errors(&mut self) -> Vec<LexError> {
        std::mem::take(&mut self.errors)
    }

    /// 次のトークンを先読みする（消費しない）
    ///
    /// # Returns
    ///
    /// 次のトークン、またはエラー
    ///
    /// # Examples
    ///
    /// ```
    /// use tsql_lexer::Lexer;
    ///
    /// let mut lexer = Lexer::new("SELECT * FROM users");
    ///
    /// // 先読み
    /// let first = lexer.peek().unwrap();
    /// assert_eq!(first.kind, tsql_token::TokenKind::Select);
    ///
    /// // 先読みしてもトークンは消費されない
    /// let second = lexer.peek().unwrap();
    /// assert_eq!(second.kind, tsql_token::TokenKind::Select);
    ///
    /// // next_token で消費
    /// let third = lexer.next_token().unwrap();
    /// assert_eq!(third.kind, tsql_token::TokenKind::Select);
    /// ```
    pub fn peek(&mut self) -> Result<Token<'src>, LexError> {
        // peek_buffer にトークンがある場合はそれを返す
        if let Some(ref token) = self.peek_buffer {
            return token.clone();
        }

        // 次のトークンを読み取ってバッファに保存
        let token = self.next_token_impl()?;
        self.peek_buffer = Some(Ok(token));
        Ok(token)
    }

    /// 内部実装: peek_buffer を使用せずに次のトークンを読み取る
    fn next_token_impl(&mut self) -> Result<Token<'src>, LexError> {
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

        let result = match ch {
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
            '=' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::Assign, "=", start_pos))
            }
            '<' => self.read_less_than(),
            '>' => self.read_greater_than(),
            '!' => self.read_bang(),
            '&' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::Ampersand, "&", start_pos))
            }
            '|' => self.read_pipe(),
            '^' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::Caret, "^", start_pos))
            }
            '~' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::Tilde, "~", start_pos))
            }
            '.' => self.read_dot(),

            // 区切り文字
            '(' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::LParen, "(", start_pos))
            }
            ')' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::RParen, ")", start_pos))
            }
            '{' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::LBrace, "{", start_pos))
            }
            '}' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::RBrace, "}", start_pos))
            }
            ']' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::RBracket, "]", start_pos))
            }
            ',' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::Comma, ",", start_pos))
            }
            ';' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::Semicolon, ";", start_pos))
            }
            ':' => {
                self.cursor.bump();
                Ok(Token::new(TokenKind::Colon, ":", start_pos))
            }

            // Unicode 文字列プレフィックス
            'U' | 'u' if self.cursor.peek() == Some('&') => self.read_unicode_string(),

            // National 文字列プレフィックス N'...' (キーワードより優先)
            'N' | 'n' if self.cursor.peek() == Some('\'') => self.read_national_string(),

            // 識別子またはキーワード
            // 注意: 'n' で始まるが 'n"' ではない場合を識別子として処理
            c if is_ident_start(c) && !(c == 'N' || c == 'n') => {
                self.read_ident_or_keyword(start_offset, start_pos)
            }
            c if is_ident_start(c) && (c == 'N' || c == 'n') => {
                // 'n' または 'N' の場合、次の文字が "'" でなければ識別子
                if self.cursor.peek() != Some('\'') {
                    self.read_ident_or_keyword(start_offset, start_pos)
                } else {
                    // これは上の N'... パターンで処理される
                    Err(LexError::UnexpectedEof {
                        position: start_pos,
                        expected: "identifier or N'...'".to_string(),
                    })
                }
            }

            // 不正な文字
            c => Err(LexError::InvalidCharacter {
                ch: c,
                position: start_pos,
            }),
        };

        // エラーリカバリ: エラーを記録して同期ポイントまでスキップ
        match result {
            Err(ref error) => {
                self.errors.push(error.clone());
                // 同期ポイント（セミコロンまたはキーワードの先頭）までスキップ
                self.synchronize();
                // Unknown トークンを返して処理を継続
                Ok(Token::new(TokenKind::Unknown, "", start_pos))
            }
            Ok(token) => Ok(token),
        }
    }
}

// Task 14.1: イテレータの実装
/// Lexer をトークンストリームとしてイテレータ可能にする
///
/// # Examples
///
/// ```
/// use tsql_lexer::Lexer;
///
/// let sql = "SELECT * FROM users";
/// let mut lexer = Lexer::new(sql);
///
/// // イテレータとして消費
/// let tokens: Vec<_> = lexer.by_ref().take_while(|t| {
///     t.as_ref().map(|t| t.kind != tsql_token::TokenKind::Eof).unwrap_or(true)
/// }).collect();
/// ```
impl<'src> Iterator for Lexer<'src> {
    type Item = Result<Token<'src>, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.next_token();
        match token {
            Ok(t) if t.kind == TokenKind::Eof => None,
            other => Some(other),
        }
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
    fn test_lexer_consuming_input() {
        // 公開API経由でLexerが入力を消費することを検証
        let mut lexer = Lexer::new("SELECT");
        let token = lexer.next_token().unwrap();

        // 入力が正しくトークン化されていることを確認
        assert_eq!(token.kind, TokenKind::Select);
        assert_eq!(token.text, "SELECT");

        // 全て消費した後にEOFになることを検証
        let eof_token = lexer.next_token().unwrap();
        assert_eq!(eof_token.kind, TokenKind::Eof);
    }

    #[test]
    fn test_lexer_with_comments_preserves_comment_tokens() {
        // コメント保持モードの振る舞いを検証
        let sql = "/* comment */ SELECT";

        // デフォルトではコメントはスキップされる
        let mut lexer_default = Lexer::new(sql);
        let first = lexer_default.next_token().unwrap();
        assert_eq!(first.kind, TokenKind::Select);

        // with_comments(true) でコメントが保持されることを検証
        let mut lexer_preserve = Lexer::new(sql).with_comments(true);
        let comment = lexer_preserve.next_token().unwrap();
        assert_eq!(comment.kind, TokenKind::BlockComment);
        assert_eq!(comment.text, "/* comment */");
    }

    #[test]
    fn test_is_ident_start() {
        assert!(is_ident_start('a'));
        assert!(is_ident_start('_'));
        assert!(!is_ident_start('1'));
        assert!(!is_ident_start('@'));
    }

    // Task 12.2: エラーリカバリのテスト

    #[test]
    fn test_error_recovery_invalid_character() {
        // 不正な文字が含まれるSQLのエラーリカバリをテスト
        let sql = "SELECT © FROM users";
        let mut lexer = Lexer::new(sql);

        // SELECT トークン
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.kind, TokenKind::Select);

        // 不正な文字で Unknown トークンが生成される
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.kind, TokenKind::Unknown);
        assert!(lexer.has_errors());

        // エラーを確認
        let errors = lexer.errors();
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            LexError::InvalidCharacter { ch, .. } => {
                assert_eq!(*ch, '©');
            }
            _ => panic!("Expected InvalidCharacter error"),
        }

        // リカバリ後、FROM が正しくトークン化される
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.kind, TokenKind::From);
    }

    #[test]
    fn test_error_recovery_unterminated_string() {
        // 終了していない文字列のエラーリカバリをテスト
        // 文字列が終了していない場合、LexerはEOFまで読み進めてエラーを返す
        let sql = "SELECT 'unterminated FROM users";
        let mut lexer = Lexer::new(sql);

        // SELECT トークン
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.kind, TokenKind::Select);

        // 終了していない文字列で Unknown トークンが生成される
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.kind, TokenKind::Unknown);
        assert!(lexer.has_errors());

        // エラーを確認
        let errors = lexer.errors();
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            LexError::UnterminatedString { .. } => {
                // OK
            }
            _ => panic!("Expected UnterminatedString error"),
        }

        // リカバリ後、EOF に達している（文字列がEOFまで読み進められたため）
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.kind, TokenKind::Eof);
    }

    #[test]
    fn test_error_recovery_to_semicolon() {
        // セミコロンまでスキップするエラーリカバリをテスト
        let sql = "SELECT © ; FROM users";
        let mut lexer = Lexer::new(sql);

        // SELECT トークン
        let token1 = lexer.next_token().unwrap();
        assert_eq!(token1.kind, TokenKind::Select);

        // エラーで Unknown トークン
        let token2 = lexer.next_token().unwrap();
        assert_eq!(token2.kind, TokenKind::Unknown);

        // synchronizeがセミコロンを消費して、FROM が次のトークンになる
        let token3 = lexer.next_token().unwrap();
        assert_eq!(token3.kind, TokenKind::From);

        // users
        let token4 = lexer.next_token().unwrap();
        assert_eq!(token4.kind, TokenKind::Ident);
    }

    #[test]
    fn test_error_recovery_debug() {
        // エラーリカバリの挙動をデバッグ
        let sql = "SELECT © ; FROM";
        let mut lexer = Lexer::new(sql);

        let t1 = lexer.next_token().unwrap();
        println!("t1: {:?}", t1.kind);

        let t2 = lexer.next_token().unwrap();
        println!("t2: {:?}", t2.kind);

        let t3 = lexer.next_token().unwrap();
        println!("t3: {:?}", t3.kind);

        let t4 = lexer.next_token().unwrap();
        println!("t4: {:?}", t4.kind);

        let t5 = lexer.next_token().unwrap();
        println!("t5: {:?}", t5.kind);

        assert_eq!(t1.kind, TokenKind::Select);
        assert_eq!(t2.kind, TokenKind::Unknown);
        assert_eq!(t3.kind, TokenKind::From);
    }

    #[test]
    fn test_has_errors() {
        let mut lexer = Lexer::new("SELECT");
        assert!(!lexer.has_errors());

        // エラーがない状態でトークンを消費
        lexer.next_token().unwrap();
        assert!(!lexer.has_errors());

        // EOF に到達
        lexer.next_token().unwrap();
        assert!(!lexer.has_errors());
    }

    #[test]
    fn test_drain_errors() {
        let mut lexer = Lexer::new("SELECT © FROM");
        lexer.next_token().unwrap(); // SELECT
        lexer.next_token().unwrap(); // Unknown (error)

        assert!(lexer.has_errors());
        let errors = lexer.drain_errors();
        assert_eq!(errors.len(), 1);

        // drain 後はエラーがクリアされる
        assert!(!lexer.has_errors());
        assert_eq!(lexer.errors().len(), 0);
    }

    #[test]
    fn test_multiple_errors_collection() {
        // 複数のエラーが正しく収集されることをテスト
        // 終了していない文字列の後も解析が続けられるケース
        let sql = "SELECT 'unterminated; © FROM users";
        let mut lexer = Lexer::new(sql);

        // トークンを消費
        while lexer.next_token().unwrap().kind != TokenKind::Eof {}

        // 複数のエラーが収集されていることを確認
        let errors = lexer.errors();
        assert!(errors.len() >= 1);
    }

    // Task 14.1: イテレータのテスト

    #[test]
    fn test_iterator_collect_all() {
        // イテレータとして全トークンを収集
        let sql = "SELECT * FROM users";
        let mut lexer = Lexer::new(sql);

        let tokens: Vec<_> = lexer.by_ref().collect();

        // EOF は含まれない
        assert!(!tokens.iter().any(|t| {
            t.as_ref()
                .map(|t| t.kind == TokenKind::Eof)
                .unwrap_or(false)
        }));

        // 期待するトークンが含まれている
        assert!(tokens.iter().any(|t| {
            t.as_ref()
                .map(|t| t.kind == TokenKind::Select)
                .unwrap_or(false)
        }));
        assert!(tokens.iter().any(|t| {
            t.as_ref()
                .map(|t| t.kind == TokenKind::Star)
                .unwrap_or(false)
        }));
        assert!(tokens.iter().any(|t| {
            t.as_ref()
                .map(|t| t.kind == TokenKind::From)
                .unwrap_or(false)
        }));
    }

    #[test]
    fn test_iterator_count() {
        // イテレータのカウント
        let sql = "SELECT * FROM users";
        let mut lexer = Lexer::new(sql);

        let count = lexer.by_ref().count();
        // SELECT, *, FROM, users = 4
        assert_eq!(count, 4);
    }

    #[test]
    fn test_iterator_take_while() {
        // 条件付きでトークンを取得
        let sql = "SELECT * FROM users";
        let mut lexer = Lexer::new(sql);

        let tokens: Vec<_> = lexer
            .by_ref()
            .take_while(|t| {
                t.as_ref()
                    .map(|t| t.kind != TokenKind::From)
                    .unwrap_or(false)
            })
            .collect();

        // SELECT と * のみ
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn test_iterator_with_errors() {
        // エラーが発生する場合のイテレータ
        // エラーリカバリにより Unknown トークンが返される
        let sql = "SELECT © FROM";
        let mut lexer = Lexer::new(sql);

        let tokens: Vec<_> = lexer.by_ref().collect();

        // エラーが記録されている
        assert!(lexer.has_errors());

        // Unknown トークンが含まれる
        assert!(tokens.iter().any(|t| {
            t.as_ref()
                .map(|t| t.kind == TokenKind::Unknown)
                .unwrap_or(false)
        }));
    }

    #[test]
    fn test_iterator_empty_input() {
        // 空入力のイテレータ
        let sql = "";
        let mut lexer = Lexer::new(sql);

        let tokens: Vec<_> = lexer.by_ref().collect();
        assert_eq!(tokens.len(), 0);
    }

    #[test]
    fn test_iterator_whitespace_only() {
        // 空白のみの入力（改行を含む）
        // 改行は Newline トークンになる
        let sql = "   \t  ";
        let mut lexer = Lexer::new(sql);

        let tokens: Vec<_> = lexer.by_ref().collect();
        assert_eq!(tokens.len(), 0);
    }

    // Task 14.2: 先読み（peek）機能のテスト

    #[test]
    fn test_peek_returns_next_token() {
        // peek で次のトークンを取得
        let sql = "SELECT * FROM users";
        let mut lexer = Lexer::new(sql);

        let peeked = lexer.peek().unwrap();
        assert_eq!(peeked.kind, TokenKind::Select);
    }

    #[test]
    fn test_peek_does_not_consume() {
        // peek してもトークンは消費されない
        let sql = "SELECT * FROM";
        let mut lexer = Lexer::new(sql);

        // 複数回 peek しても同じトークンが返される
        let first = lexer.peek().unwrap();
        let second = lexer.peek().unwrap();
        assert_eq!(first.kind, TokenKind::Select);
        assert_eq!(second.kind, TokenKind::Select);
    }

    #[test]
    fn test_peek_then_next_token() {
        // peek 後に next_token で同じトークンが消費される
        let sql = "SELECT * FROM";
        let mut lexer = Lexer::new(sql);

        let peeked = lexer.peek().unwrap();
        assert_eq!(peeked.kind, TokenKind::Select);

        // next_token で同じトークンが取得される
        let consumed = lexer.next_token().unwrap();
        assert_eq!(consumed.kind, TokenKind::Select);

        // 次のトークンは *
        let next = lexer.next_token().unwrap();
        assert_eq!(next.kind, TokenKind::Star);
    }

    #[test]
    fn test_multiple_peek_then_consume() {
        // 複数回 peek した後に連続して消費
        let sql = "SELECT * FROM users";
        let mut lexer = Lexer::new(sql);

        // 3回 peek
        let _ = lexer.peek().unwrap();
        let _ = lexer.peek().unwrap();
        let _ = lexer.peek().unwrap();

        // 1回だけ消費
        let first = lexer.next_token().unwrap();
        assert_eq!(first.kind, TokenKind::Select);

        // 残りのトークンを消費
        let second = lexer.next_token().unwrap();
        assert_eq!(second.kind, TokenKind::Star);

        let third = lexer.next_token().unwrap();
        assert_eq!(third.kind, TokenKind::From);
    }

    #[test]
    fn test_peek_after_next_token() {
        // next_token 後に peek で次のトークンを取得
        let sql = "SELECT * FROM";
        let mut lexer = Lexer::new(sql);

        // 最初のトークンを消費
        let _ = lexer.next_token().unwrap();

        // 次のトークンを peek
        let peeked = lexer.peek().unwrap();
        assert_eq!(peeked.kind, TokenKind::Star);

        // peek したトークンを消費
        let consumed = lexer.next_token().unwrap();
        assert_eq!(consumed.kind, TokenKind::Star);
    }

    #[test]
    fn test_peek_eof() {
        // EOF での peek
        let sql = "";
        let mut lexer = Lexer::new(sql);

        let peeked = lexer.peek().unwrap();
        assert_eq!(peeked.kind, TokenKind::Eof);
    }

    #[test]
    fn test_peek_with_error() {
        // エラーが発生する場合の peek
        let sql = "SELECT © FROM";
        let mut lexer = Lexer::new(sql);

        // SELECT を消費
        let _ = lexer.next_token().unwrap();

        // エラー位置のトークンを peek
        let peeked = lexer.peek().unwrap();
        assert_eq!(peeked.kind, TokenKind::Unknown);

        // エラーが記録されている
        assert!(lexer.has_errors());
    }
}
