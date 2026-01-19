//! 字句解析器の統合テスト

use tsql_lexer::{LexError, Lexer, TokenKind};

/// 単純な SELECT クエリのテスト
#[test]
fn test_simple_select_query() {
    let sql = "SELECT * FROM users";
    let mut lexer = Lexer::new(sql);

    let token1 = lexer.next_token().unwrap();
    assert_eq!(token1.kind, TokenKind::Select);
    assert_eq!(token1.text, "SELECT");

    let token2 = lexer.next_token().unwrap();
    assert_eq!(token2.kind, TokenKind::Star);
    assert_eq!(token2.text, "*");

    let token3 = lexer.next_token().unwrap();
    assert_eq!(token3.kind, TokenKind::From);
    assert_eq!(token3.text, "FROM");

    let token4 = lexer.next_token().unwrap();
    assert_eq!(token4.kind, TokenKind::Ident);
    assert_eq!(token4.text, "users");

    let token5 = lexer.next_token().unwrap();
    assert_eq!(token5.kind, TokenKind::Eof);
}

/// キーワードの大文字小文字非区別テスト
#[test]
fn test_case_insensitive_keywords() {
    let sql = "select From wheRE";
    let mut lexer = Lexer::new(sql);

    assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Select);
    assert_eq!(lexer.next_token().unwrap().kind, TokenKind::From);
    assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Where);
}

/// 演算子のテスト
#[test]
fn test_operators() {
    // Test operators without spaces to ensure they're parsed correctly
    let tests = vec![
        ("+", TokenKind::Plus),
        ("-", TokenKind::Minus),
        ("*", TokenKind::Star),
        ("/", TokenKind::Slash),
        ("%", TokenKind::Percent),
        ("=", TokenKind::Assign),
        ("<", TokenKind::Lt),
        (">", TokenKind::Gt),
        ("<=", TokenKind::Le),
        (">=", TokenKind::Ge),
        ("<>", TokenKind::NeAlt),
        ("!=", TokenKind::Ne),
        ("!<", TokenKind::NotLt),
        ("!>", TokenKind::NotGt),
        ("&", TokenKind::Ampersand),
        ("|", TokenKind::Pipe),
        ("^", TokenKind::Caret),
        ("~", TokenKind::Tilde),
        ("..", TokenKind::DotDot),
        (".", TokenKind::Dot),
        ("||", TokenKind::Concat),
    ];

    for (sql, expected) in tests {
        let mut lexer = Lexer::new(sql);
        let token = lexer.next_token().unwrap();
        assert_eq!(token.kind, expected, "Failed for: {}", sql);
    }
}

/// 数値リテラルのテスト
#[test]
fn test_number_literals() {
    let sql = "123 456.78 1.5e10 0xABCD";
    let mut lexer = Lexer::new(sql);

    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::Number);
    assert_eq!(t1.text, "123");

    let t2 = lexer.next_token().unwrap();
    assert_eq!(t2.kind, TokenKind::FloatLiteral);
    assert_eq!(t2.text, "456.78");

    let t3 = lexer.next_token().unwrap();
    assert_eq!(t3.kind, TokenKind::FloatLiteral);
    assert_eq!(t3.text, "1.5e10");

    let t4 = lexer.next_token().unwrap();
    assert_eq!(t4.kind, TokenKind::HexString);
    assert_eq!(t4.text, "0xABCD");
}

/// 文字列リテラルのテスト
#[test]
fn test_string_literals() {
    // Test simple strings first
    let mut lexer = Lexer::new("'hello'");
    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::String);
    assert_eq!(t1.text, "'hello'");

    // Test escaped quote
    let mut lexer2 = Lexer::new("'it''s'");
    let t2 = lexer2.next_token().unwrap();
    assert_eq!(t2.kind, TokenKind::String);
    assert_eq!(t2.text, "'it''s'");
}

/// 変数のテスト
#[test]
fn test_variables() {
    let sql = "@var @@global";
    let mut lexer = Lexer::new(sql);

    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::LocalVar);
    assert_eq!(t1.text, "@var");

    let t2 = lexer.next_token().unwrap();
    assert_eq!(t2.kind, TokenKind::GlobalVar);
    assert_eq!(t2.text, "@@global");
}

/// 一時テーブルのテスト
#[test]
fn test_temp_tables() {
    let sql = "#temp ##global_temp";
    let mut lexer = Lexer::new(sql);

    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::TempTable);
    assert_eq!(t1.text, "#temp");

    let t2 = lexer.next_token().unwrap();
    assert_eq!(t2.kind, TokenKind::GlobalTempTable);
    assert_eq!(t2.text, "##global_temp");
}

/// 引用符付き識別子のテスト
#[test]
fn test_quoted_identifiers() {
    // Test bracket identifiers with escape sequences
    let mut lexer = Lexer::new("[my table]");
    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::QuotedIdent);
    assert_eq!(t1.text, "[my table]");

    // Test double quote identifiers
    let mut lexer2 = Lexer::new("\"schema\"");
    let t2 = lexer2.next_token().unwrap();
    assert_eq!(t2.kind, TokenKind::QuotedIdent);
    assert_eq!(t2.text, "\"schema\"");

    // Test escaped closing bracket
    // In T-SQL, ]] is the escape for a literal ] character
    // So [my]]] means: [ + my + ]] (literal ]) + ] (closing) = identifier "my]"
    let mut lexer3 = Lexer::new("[my]]]");
    let t3 = lexer3.next_token().unwrap();
    assert_eq!(t3.kind, TokenKind::QuotedIdent);
    assert_eq!(t3.text, "[my]]]");

    // Test escaped double quote
    // In T-SQL, "" is the escape for a literal " character
    // So "table""" means: " + table + "" (literal ") + " (closing) = identifier "table""
    let mut lexer4 = Lexer::new("\"table\"\"\"");
    let t4 = lexer4.next_token().unwrap();
    assert_eq!(t4.kind, TokenKind::QuotedIdent);
    assert_eq!(t4.text, "\"table\"\"\"");
}

/// ブロックコメントのテスト
#[test]
fn test_block_comment() {
    let sql = "/* comment */ SELECT";
    let mut lexer = Lexer::new(sql);

    // デフォルトではコメントはスキップされる
    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::Select);
}

/// ネストされたブロックコメントのテスト
#[test]
fn test_nested_block_comment() {
    let sql = "/* outer /* inner */ */ SELECT";
    let mut lexer = Lexer::new(sql);

    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::Select);
}

/// ラインコメントのテスト
#[test]
fn test_line_comment() {
    let sql = "-- comment\nSELECT";
    let mut lexer = Lexer::new(sql);

    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::Select);
}

/// コメント保持モードのテスト
#[test]
fn test_preserve_comments() {
    let sql = "/* comment */ SELECT";
    let mut lexer = Lexer::new(sql).with_comments(true);

    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::BlockComment);
    assert_eq!(t1.text, "/* comment */");
}

/// 終了していない文字列のエラーテスト
#[test]
fn test_unterminated_string_error() {
    let sql = "'hello";
    let mut lexer = Lexer::new(sql);
    let result = lexer.next_token();
    assert!(matches!(result, Err(LexError::UnterminatedString { .. })));
}

/// 終了していないブロックコメントのエラーテスト
#[test]
fn test_unterminated_block_comment_error() {
    let sql = "/* comment";
    let mut lexer = Lexer::new(sql).with_comments(true);
    let result = lexer.next_token();
    assert!(matches!(result, Err(LexError::UnterminatedBlockComment { .. })));
}

/// 複合代入演算子のテスト
#[test]
fn test_compound_assignment_operators() {
    let sql = "+= -= *= /=";
    let mut lexer = Lexer::new(sql);

    assert_eq!(lexer.next_token().unwrap().kind, TokenKind::PlusAssign);
    assert_eq!(lexer.next_token().unwrap().kind, TokenKind::MinusAssign);
    assert_eq!(lexer.next_token().unwrap().kind, TokenKind::StarAssign);
    assert_eq!(lexer.next_token().unwrap().kind, TokenKind::SlashAssign);
}

/// 区切りのテスト
#[test]
fn test_delimiters() {
    // Test each delimiter individually to isolate issues
    let tests = vec![
        ("(", TokenKind::LParen),
        (")", TokenKind::RParen),
        ("{", TokenKind::LBrace),
        ("}", TokenKind::RBrace),
        ("[", TokenKind::LBracket),
        ("]", TokenKind::RBracket),
        (",", TokenKind::Comma),
        (";", TokenKind::Semicolon),
        (":", TokenKind::Colon),
    ];

    for (sql, expected) in tests {
        let mut lexer = Lexer::new(sql);
        let token = lexer.next_token().unwrap();
        assert_eq!(token.kind, expected, "Failed for: {}", sql);
    }
}

/// Unicode 文字列のテスト
#[test]
fn test_unicode_string() {
    let sql = "U&'test' u&'test'";
    let mut lexer = Lexer::new(sql);

    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::UnicodeString);
    assert_eq!(t1.text, "U&'test'");

    let t2 = lexer.next_token().unwrap();
    assert_eq!(t2.kind, TokenKind::UnicodeString);
    assert_eq!(t2.text, "u&'test'");
}
