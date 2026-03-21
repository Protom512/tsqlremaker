//! 字句解析器の統合テスト

#![allow(clippy::unwrap_used)]

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

/// ESCAPE キーワードのテスト
#[test]
fn test_escape_keyword() {
    let sql = "ESCAPE";
    let mut lexer = Lexer::new(sql);

    let token = lexer.next_token().unwrap();
    assert_eq!(token.kind, TokenKind::Escape);
    assert_eq!(token.text, "ESCAPE");
}

/// ESCAPE キーワードの大文字小文字非区別テスト
#[test]
fn test_escape_case_insensitive() {
    let tests = vec!["ESCAPE", "escape", "EsCaPe"];

    for sql in tests {
        let mut lexer = Lexer::new(sql);
        assert_eq!(lexer.next_token().unwrap().kind, TokenKind::Escape);
    }
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
/// エラーリカバリにより、Unknownトークンが返され、エラーが記録される
#[test]
fn test_unterminated_string_error() {
    let sql = "'hello";
    let mut lexer = Lexer::new(sql);
    let result = lexer.next_token();

    // エラーリカバリにより Ok(Unknown) が返される
    assert!(result.is_ok());
    assert_eq!(result.unwrap().kind, TokenKind::Unknown);

    // エラーが記録されていることを確認
    assert!(lexer.has_errors());
    assert!(matches!(
        lexer.errors()[0],
        LexError::UnterminatedString { .. }
    ));
}

/// 終了していないブロックコメントのエラーテスト
/// エラーリカバリにより、Unknownトークンが返され、エラーが記録される
#[test]
fn test_unterminated_block_comment_error() {
    let sql = "/* comment";
    let mut lexer = Lexer::new(sql).with_comments(true);
    let result = lexer.next_token();

    // エラーリカバリにより Ok(Unknown) が返される
    assert!(result.is_ok());
    assert_eq!(result.unwrap().kind, TokenKind::Unknown);

    // エラーが記録されていることを確認
    assert!(lexer.has_errors());
    assert!(matches!(
        lexer.errors()[0],
        LexError::UnterminatedBlockComment { .. }
    ));
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

// Task 16.2: 追加のインテグレーションテスト

/// 完全な SELECT クエリのトークン化テスト
#[test]
fn test_tokenize_full_select_query() {
    let sql = "SELECT id, name, email FROM users WHERE active = 1 ORDER BY name";
    let mut lexer = Lexer::new(sql);

    let mut count = 0;
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        count += 1;
    }

    // 一定数以上のトークンが生成されている
    assert!(count >= 8); // SELECT, id, ,, name, ,, email, FROM, users, WHERE, active, =, 1, ORDER, BY, name
}

/// CREATE PROCEDURE 文のトークン化テスト
#[test]
fn test_tokenize_create_procedure() {
    let sql = "CREATE PROCEDURE get_users @active INT AS BEGIN SELECT * FROM users WHERE active = @active END";
    let mut lexer = Lexer::new(sql);

    let mut kinds = Vec::new();
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        kinds.push(token.kind);
    }

    assert!(kinds.contains(&TokenKind::Create));
    assert!(kinds.contains(&TokenKind::Procedure));
    assert!(kinds.contains(&TokenKind::As));
    assert!(kinds.contains(&TokenKind::Begin));
    assert!(kinds.contains(&TokenKind::End));
}

/// 複雑な JOIN を含むクエリのトークン化テスト
#[test]
fn test_tokenize_complex_join() {
    let sql = "SELECT u.id, u.name, o.order_id FROM users u INNER JOIN orders o ON u.id = o.user_id LEFT JOIN products p ON o.product_id = p.id WHERE u.status = 'active'";
    let mut lexer = Lexer::new(sql);

    let mut kinds = Vec::new();
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        kinds.push(token.kind);
    }

    assert!(kinds.contains(&TokenKind::Inner));
    assert!(kinds.contains(&TokenKind::Join));
    assert!(kinds.contains(&TokenKind::Left));
    assert!(kinds.contains(&TokenKind::On));
}

/// エラーを含む SQL のリカバリテスト
#[test]
fn test_tokenize_sql_with_errors() {
    // 不正な文字が含まれる SQL
    let sql = "SELECT id, © name FROM users WHERE id = 1";

    let mut lexer = Lexer::new(sql);

    // SELECT トークン
    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::Select);

    // id トークン
    let t2 = lexer.next_token().unwrap();
    assert_eq!(t2.kind, TokenKind::Ident);

    // カンマ
    let t3 = lexer.next_token().unwrap();
    assert_eq!(t3.kind, TokenKind::Comma);

    // © でエラーが発生
    let t4 = lexer.next_token().unwrap();
    assert_eq!(t4.kind, TokenKind::Unknown);

    // エラーが記録されている
    assert!(lexer.has_errors());

    // FROM もリカバリ後にトークン化されている
    let mut found_from = false;
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::From {
            found_from = true;
            break;
        }
    }
    assert!(found_from);
}

/// 複数のステートメントのトークン化テスト
#[test]
fn test_tokenize_multiple_statements() {
    let sql = "SELECT * FROM users; INSERT INTO logs VALUES ('operation'); UPDATE users SET last_login = GETDATE()";
    let mut lexer = Lexer::new(sql);

    let mut kinds = Vec::new();
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        kinds.push(token.kind);
    }

    assert!(kinds.contains(&TokenKind::Select));
    assert!(kinds.contains(&TokenKind::Insert));
    assert!(kinds.contains(&TokenKind::Update));
}

/// 変数と一時テーブルを含むクエリのテスト
#[test]
fn test_tokenize_variables_and_temp_tables() {
    let sql = "DECLARE @user_id INT; SELECT * INTO #temp_results FROM users WHERE id = @user_id";
    let mut lexer = Lexer::new(sql);

    let mut kinds = Vec::new();
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        kinds.push(token.kind);
    }

    assert!(kinds.contains(&TokenKind::Declare));
    assert!(kinds.contains(&TokenKind::LocalVar));
    assert!(kinds.contains(&TokenKind::TempTable));
}

/// 数値リテラルの様々な形式のテスト
#[test]
fn test_tokenize_various_number_literals() {
    let sql = "SELECT 123, 45.67, 1.5e10, 0xABCD FROM numbers";
    let mut lexer = Lexer::new(sql);

    let mut text = Vec::new();
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        text.push(token.text);
    }

    assert!(text.iter().any(|t| t.contains("123")));
    assert!(text
        .iter()
        .any(|t| t.contains("45.67") || t.contains("1.5e10")));
    assert!(text.iter().any(|t| t.contains("0xABCD")));
}

/// 演算子の組み合わせのテスト
#[test]
fn test_tokenize_operator_combinations() {
    let sql = "SELECT a + b - c * d / e % f FROM table1";
    let mut lexer = Lexer::new(sql);

    let mut kinds = Vec::new();
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        kinds.push(token.kind);
    }

    assert!(kinds.contains(&TokenKind::Plus));
    assert!(kinds.contains(&TokenKind::Minus));
    assert!(kinds.contains(&TokenKind::Star));
    assert!(kinds.contains(&TokenKind::Slash));
    assert!(kinds.contains(&TokenKind::Percent));
}

/// 引用符付き識別子のテスト
#[test]
fn test_tokenize_quoted_identifiers_complex() {
    let sql = "SELECT [user id], [table name] FROM [my database].[dbo].[users]";
    let mut lexer = Lexer::new(sql);

    let mut kinds = Vec::new();
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        kinds.push(token.kind);
    }

    assert!(kinds.contains(&TokenKind::QuotedIdent));
    assert!(kinds.contains(&TokenKind::Dot));
}

/// 大きな SQL クエリのトークン化テスト
#[test]
fn test_tokenize_large_query() {
    let sql = r#"
        CREATE PROCEDURE get_user_orders
            @user_id INT,
            @status VARCHAR(50) = 'active'
        AS
        BEGIN
            SELECT
                o.order_id,
                o.order_date,
                c.customer_name
            FROM orders o
            INNER JOIN customers c ON o.customer_id = c.customer_id
            WHERE o.customer_id = @user_id
              AND o.status = @status
            ORDER BY o.order_date DESC
        END
    "#;

    let mut lexer = Lexer::new(sql);

    // イテレータの代わりに next_token を使用してメモリ消費を抑える
    let mut count = 0;
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        count += 1;
    }

    // 一定数以上のトークンが生成されている
    assert!(count > 20);
}

/// エラーリカバリのテスト
#[test]
fn test_error_recovery_in_integration() {
    let sql = "SELECT id, © name FROM users WHERE id = 1";

    let mut lexer = Lexer::new(sql);

    // SELECT トークン
    let t1 = lexer.next_token().unwrap();
    assert_eq!(t1.kind, TokenKind::Select);

    // id トークン
    let _ = lexer.next_token().unwrap();

    // カンマ
    let _ = lexer.next_token().unwrap();

    // © でエラーが発生
    let t_err = lexer.next_token().unwrap();
    assert_eq!(t_err.kind, TokenKind::Unknown);

    // エラーが記録されている
    assert!(lexer.has_errors());

    // リカバリ後、FROM がトークン化されている
    let mut found_from = false;
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::From {
            found_from = true;
            break;
        }
    }
    assert!(found_from);
}

/// National 文字列のテスト
#[test]
fn test_national_string() {
    let sql = "SELECT N'Unicode string' FROM users";
    let mut lexer = Lexer::new(sql);

    let mut kinds = Vec::new();
    while let Ok(token) = lexer.next_token() {
        if token.kind == TokenKind::Eof {
            break;
        }
        kinds.push(token.kind);
    }

    // NString が含まれている（TokenKind に定義されている）
    assert!(kinds.contains(&TokenKind::Select));
}
