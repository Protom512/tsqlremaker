use tsql_token::{lookup_ident, Token, ASSIGN, COMMA, IDENT, ILLEGAL, LPAREN, RPAREN};
use tsql_token::{EOF, NUM};
#[derive(Debug)]
pub struct Lexer<'a> {
    ch: char,
    /// 入力された文字列
    input: &'a str,
    position: usize,
    /// 現在読んでいる
    read_position: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut l = Self {
            input,
            ch: input.chars().nth(0).unwrap(),
            position: 0,
            read_position: 0,
        };
        l
    }

    pub fn ch(&self) -> char {
        self.ch
    }

    pub fn set_ch(&mut self, ch: char) {
        self.ch = ch;
    }

    pub fn ch_mut(&mut self) -> &mut char {
        &mut self.ch
    }
    pub fn next_token(&mut self) -> Token {
        self.eat_whitespace();
        if self.check_eof() {
            return Token::new(EOF.to_string(), "".to_string());
        }
        match self.ch {
            '=' => {
                let token = Token::new(ASSIGN.to_string(), self.ch.to_string());
                self.read_char();
                token
            }
            '(' => {
                let token = Token::new(LPAREN.to_string(), self.ch.to_string());
                self.read_char();
                token
            }
            ')' => {
                let token = Token::new(RPAREN.to_string(), self.ch.to_string());
                self.read_char();
                token
            }
            ',' => {
                let token = Token::new(COMMA.to_string(), self.ch.to_string());
                self.read_char();
                token
            }
            _ => {
                if self.ch.is_numeric() {
                    let token = Token::new(NUM.to_string(), self.read_number());
                    self.read_char();

                    token
                } else if self.ch.is_alphabetic() {
                    let literal = self.read_identity();
                    Token::new(lookup_ident(&literal), literal)
                } else {
                    Token::new(ILLEGAL.to_string(), self.ch.to_string());
                    panic!("{:#?}", &self);
                }
            }
        }
    }

    fn read_number(&mut self) -> String {
        let start_pos = self.position;
        while self.ch.is_numeric() {
            self.read_char(); // 次の文字を読み込む
        }
        let identifier = &self.input[start_pos as usize..self.position as usize];
        identifier.to_string() // 文字列を返す
    }
    fn read_identity(&mut self) -> String {
        let start_pos = self.position;
        while self.ch.is_alphanumeric() || self.ch == '_' || self.ch == '.' {
            self.read_char(); // 次の文字を読み込む
        }
        let identifier = &self.input[start_pos as usize..self.position as usize];
        identifier.to_string() // 文字列を返す
    }
    fn peek_char(&mut self) -> char {
        if self.read_position >= self.input.len() {
            return '\0';
        } else {
            let _char_at_nth = match self.input.chars().nth(self.read_position.into()) {
                Some(n) => return n,
                None => {
                    dbg!(&self);
                    return '\0';
                }
            };
        }
    }
    /// スペース系をスキップしてpositionを勧める
    fn eat_whitespace(&mut self) {
        while self.ch == ' ' || self.ch == '\t' || self.ch == '\n' || self.ch == '\r' {
            self.read_char();
        }
    }
    fn read_char(&mut self) {
        let tmpch = match self.input.chars().nth(self.read_position.into()) {
            Some(n) => n,
            None => '\0',
        };
        self.set_ch(tmpch);
        self.position = self.read_position;
        self.read_position += 1;
    }

    fn check_eof(&self) -> bool {
        return self.read_position >= self.input.len();
    }
}
