pub type token_type = String;
#[derive(Debug)]
pub struct Token {
    token_type: token_type,
    token: String,
}

impl Token {
    pub fn new(token_type: token_type, token: String) -> Self {
        Self { token_type, token }
    }

    pub fn set_token_type(&mut self, token_type: token_type) {
        self.token_type = token_type;
    }

    pub fn set_token(&mut self, token: String) {
        self.token = token;
    }

    pub fn token_type(&self) -> &str {
        &self.token_type
    }
}
pub const SELECT: &str = "select";
pub const UPDATE: &str = "update";
pub const DELETE: &str = "delete";
pub const INSERT: &str = "insert";
pub const CREATE: &str = "create";
pub const FROM: &str = "from";
pub const WHERE: &str = "where";
pub const IF: &str = "if";
pub const RPAREN: &str = ")";
pub const LPAREN: &str = "(";
pub const COMMA: &str = ",";
pub const SEMICOLON: &str = ";";
pub const COLON: &str = ":";
pub const ASSIGN: &str = "=";
pub const WHITESPACE: &str = "\n";

pub const ILLEGAL: &str = "ILLEGAL";
pub const EOF: &str = "EOF";
pub const IDENT: &str = "IDENT";
pub const NUM: &str = "NUM";

use std::collections::HashMap;
pub fn lookup_ident(ident: &str) -> token_type {
    dbg!(ident);
    let mut map = HashMap::new();
    map.insert("select".to_lowercase(), SELECT);
    map.insert("if".to_lowercase(), IF);
    map.insert("create".to_lowercase(), CREATE);
    map.insert("update".to_lowercase(), UPDATE);
    map.insert("insert".to_lowercase(), INSERT);
    map.insert("from".to_lowercase(), FROM);
    map.insert("where".to_lowercase(), WHERE);

    let lower_ident = ident.to_lowercase();
    match map.get(&lower_ident) {
        Some(value) => value.to_string(),

        None => IDENT.to_string(),
    }
}
