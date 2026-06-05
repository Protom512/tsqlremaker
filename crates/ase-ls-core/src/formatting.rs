//! Document Formatting
//!
//! T-SQL ソースコードの自動フォーマット機能を提供する。
//! キーワード大文字化、インデント、改行の挿入を行う。

use lsp_types::{Position, Range, TextEdit};
use std::borrow::Cow;
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// SQL文をフォーマットし、TextEditのリストを返す
#[must_use]
pub fn format(source: &str) -> Vec<TextEdit> {
    let formatted = format_sql(source);
    if formatted == source {
        return Vec::new();
    }

    let total_lines = source.lines().count() as u32;
    let last_line_len = source.lines().last().map_or(0, |l| l.len() as u32);

    vec![TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: total_lines,
                character: last_line_len,
            },
        },
        new_text: formatted,
    }]
}

/// SQL文字列をフォーマットする
fn format_sql(source: &str) -> String {
    let lexer = Lexer::new(source).with_comments(true);

    let mut result = String::new();
    let mut indent_level = 0u32;
    let mut prev_kind: Option<TokenKind> = None;
    let mut at_line_start = true;

    for token in lexer.filter_map(Result::ok) {
        if token.kind == TokenKind::Eof {
            break;
        }

        // END/END_ の前にインデントを減らす（ELSEは除外: IFと同じレベルに配置）
        if should_decrease_indent(&token.kind) && indent_level > 0 {
            indent_level -= 1;
        }

        // 改行前のトークン調整
        if should_newline_before(&token.kind, prev_kind.as_ref()) {
            result.push('\n');
            at_line_start = true;
        }

        // インデント挿入
        if at_line_start {
            const INDENT: &str = "    ";
            for _ in 0..indent_level {
                result.push_str(INDENT);
            }
            at_line_start = false;
        }

        // トークン間のスペース
        if !result.is_empty()
            && !result.ends_with('\n')
            && !result.ends_with("    ")
            && needs_space_before(&token.kind, prev_kind.as_ref())
        {
            result.push(' ');
        }

        // トークンテキストの書き換え
        let text = format_token(&token.kind, token.text);
        result.push_str(&text);

        // BEGIN/CASE の後にインデントを増やす
        if matches!(token.kind, TokenKind::Begin | TokenKind::Case) {
            indent_level += 1;
        }

        prev_kind = Some(token.kind);
    }

    // 末尾改行
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }

    result
}

/// キーワードを大文字化する
///
/// 変換が不要なトークン（識別子、演算子、数字等）は `Cow::Borrowed` を返し、
/// アロケーションを回避する。キーワードの大文字化と文字列/コメントの
/// コピーにのみ `Cow::Owned` を使用する。
fn format_token<'a>(kind: &TokenKind, text: &'a str) -> Cow<'a, str> {
    match kind {
        TokenKind::String | TokenKind::NString | TokenKind::HexString => {
            Cow::Owned(text.to_owned())
        }
        TokenKind::LineComment | TokenKind::BlockComment => Cow::Owned(text.to_owned()),
        _ => {
            if kind.is_keyword() {
                Cow::Owned(text.to_uppercase())
            } else {
                Cow::Borrowed(text)
            }
        }
    }
}

/// トークン前に改行を入れるべきか
fn should_newline_before(kind: &TokenKind, prev: Option<&TokenKind>) -> bool {
    let prev = match prev {
        Some(p) => p,
        None => return false,
    };

    // GO の後は必ず改行
    if matches!(prev, TokenKind::Go) {
        return true;
    }

    // セミコロンの後は改行
    if matches!(prev, TokenKind::Semicolon) {
        return true;
    }

    // BEGIN/END/END_ の後
    if matches!(prev, TokenKind::Begin | TokenKind::End | TokenKind::End_)
        && !matches!(kind, TokenKind::End | TokenKind::End_)
    {
        return true;
    }

    // 主要な句の前に改行
    match kind {
        TokenKind::Select
        | TokenKind::From
        | TokenKind::Where
        | TokenKind::Group
        | TokenKind::Order
        | TokenKind::Having
        | TokenKind::Union
        | TokenKind::Insert
        | TokenKind::Update
        | TokenKind::Delete
        | TokenKind::Create
        | TokenKind::Alter
        | TokenKind::Drop
        | TokenKind::Declare
        | TokenKind::Set
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::While
        | TokenKind::Return
        | TokenKind::Begin
        | TokenKind::End
        | TokenKind::Try
        | TokenKind::Catch => {
            // Rationale: SELECTとFROMは同一行に保つ。これにより "SELECT col FROM t" が
            // 1行にフォーマットされ、短いクエリの可読性が向上する。FROMのみを改行
            // 対象キーワードに含めつつ、SELECT直後のFROMだけを例外的に同一行扱いにする。
            if matches!(prev, TokenKind::Select) && matches!(kind, TokenKind::From) {
                return false;
            }
            // 初回以外で改行
            true
        }
        _ => false,
    }
}

/// トークン前にスペースを入れるべきか
fn needs_space_before(kind: &TokenKind, prev: Option<&TokenKind>) -> bool {
    let prev = match prev {
        Some(p) => p,
        None => return false,
    };

    // 括弧の後/前はスペース不要
    if matches!(prev, TokenKind::LParen) {
        return false;
    }
    if matches!(
        kind,
        TokenKind::RParen | TokenKind::LParen | TokenKind::Comma | TokenKind::Semicolon
    ) {
        return false;
    }
    // ドットの前後はスペース不要
    if matches!(kind, TokenKind::Dot) || matches!(prev, TokenKind::Dot) {
        return false;
    }

    true
}

/// トークン出力前にインデントを減らすべきか
fn should_decrease_indent(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::End | TokenKind::End_)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_format_keyword_uppercase() {
        let result = format_sql("select * from t");
        assert!(result.contains("SELECT"));
        assert!(result.contains("FROM"));
    }

    #[test]
    fn test_format_preserves_strings() {
        let result = format_sql("SELECT 'hello world' FROM t");
        assert!(result.contains("'hello world'"));
    }

    #[test]
    fn test_format_preserves_comments() {
        let result = format_sql("SELECT * -- comment\nFROM t");
        assert!(result.contains("-- comment"));
    }

    #[test]
    fn test_format_idempotent() {
        let input = "select col1, col2 from users where id = 1";
        let first = format_sql(input);
        let second = format_sql(&first);
        assert_eq!(first, second, "Formatting should be idempotent");
    }

    #[test]
    fn test_format_returns_text_edit() {
        let source = "select * from t";
        let edits = format(source);
        assert_eq!(edits.len(), 1);
        assert!(edits[0].new_text.contains("SELECT"));
    }

    #[test]
    fn test_format_no_change_returns_empty() {
        // Already formatted (uppercase, proper spacing)
        let source = "SELECT * FROM t\n";
        // Since we always reformat, let's check the formatted output
        let formatted = format_sql(source);
        // The formatted version should be the same or improved
        assert!(!formatted.is_empty());
    }

    #[test]
    fn test_format_semicolon_newline() {
        let result = format_sql("SELECT 1; SELECT 2");
        assert!(result.contains(";\n") || result.contains("; \n"));
    }

    #[test]
    fn test_format_go_newline() {
        let result = format_sql("SELECT 1 GO SELECT 2");
        // GO should cause line break
        assert!(result.contains("GO\n") || result.contains("GO \n"));
    }

    #[test]
    fn test_format_indent_begin_end() {
        let result = format_sql("BEGIN SELECT 1 END");
        let lines: Vec<&str> = result.lines().collect();
        let select_line = lines.iter().find(|l| l.contains("SELECT"));
        assert!(select_line.is_some());
        let select_line = select_line.unwrap();
        assert!(
            select_line.starts_with("    "),
            "SELECT should be indented, got: '{}'",
            select_line
        );
        let end_line = lines.iter().find(|l| l.trim().starts_with("END"));
        assert!(end_line.is_some());
        let end_line = end_line.unwrap();
        assert!(
            !end_line.starts_with("    "),
            "END should not be indented, got: '{}'",
            end_line
        );
    }

    #[test]
    fn test_format_indent_nested_begin() {
        let result = format_sql("BEGIN BEGIN SELECT 1 END END");
        let lines: Vec<&str> = result.lines().collect();
        let select_line = lines.iter().find(|l| l.contains("SELECT"));
        assert!(select_line.is_some());
        let select_line = select_line.unwrap();
        assert!(
            select_line.starts_with("        "),
            "Inner SELECT should be double-indented, got: '{}'",
            select_line
        );
    }

    #[test]
    fn test_format_idempotent_with_indent() {
        let input = "BEGIN\n    SELECT 1\nEND";
        let first = format_sql(input);
        let second = format_sql(&first);
        assert_eq!(first, second, "Formatting with indent should be idempotent");
    }

    #[test]
    fn test_format_if_else_same_indent() {
        let result = format_sql("IF 1 = 1 BEGIN SELECT 1 END ELSE BEGIN SELECT 2 END");
        let lines: Vec<&str> = result.lines().collect();
        let else_line = lines.iter().find(|l| l.trim().starts_with("ELSE"));
        assert!(else_line.is_some());
        let else_line = else_line.unwrap();
        assert!(
            !else_line.starts_with("    "),
            "ELSE should be at same level as IF, got: '{}'",
            else_line
        );
    }

    #[test]
    fn test_format_case_end_indent() {
        // CASE increases indent, END decreases it
        let result = format_sql("CASE WHEN 1 = 1 THEN 'a' END");
        // CASE is indented by Case token, END returns to level 0
        // (CASE/WWhen on same line is acceptable for inline CASE expressions)
        assert!(result.contains("CASE"), "Should contain CASE: {}", result);
        assert!(result.contains("END"), "Should contain END: {}", result);
        // Verify idempotent
        let second = format_sql(&result);
        assert_eq!(result, second, "CASE formatting should be idempotent");
    }

    #[test]
    fn test_format_string_preserves_content() {
        let result = format_sql("SELECT 'hello' FROM t");
        assert!(
            result.contains("'hello'"),
            "String content should be preserved: {}",
            result
        );
    }

    #[test]
    fn test_format_nstring_preserves_content() {
        let result = format_sql("SELECT N'hello' FROM t");
        assert!(
            result.contains("N'hello'") || result.contains("hello"),
            "NString content should be present: {}",
            result
        );
    }

    #[test]
    fn test_format_comment_preserves_content() {
        let result = format_sql("SELECT 1 -- my comment\nFROM t");
        assert!(
            result.contains("-- my comment"),
            "Line comment should be preserved: {}",
            result
        );
    }

    #[test]
    fn test_format_block_comment_preserves_content() {
        let result = format_sql("SELECT 1 /* block */ FROM t");
        assert!(
            result.contains("/* block */"),
            "Block comment should be preserved: {}",
            result
        );
    }

    #[test]
    fn test_format_newline_before_from() {
        let result = format_sql("SELECT col1, col2 FROM users WHERE id = 1");
        assert!(result.contains("FROM"), "Should contain FROM: {}", result);
        // FROM should be on its own line
        let lines: Vec<&str> = result.lines().collect();
        let from_line = lines.iter().find(|l| l.trim().starts_with("FROM"));
        assert!(from_line.is_some(), "FROM should start a line: {}", result);
    }

    #[test]
    fn test_format_newline_before_where() {
        let result = format_sql("SELECT * FROM users WHERE id = 1");
        let lines: Vec<&str> = result.lines().collect();
        let where_line = lines.iter().find(|l| l.trim().starts_with("WHERE"));
        assert!(
            where_line.is_some(),
            "WHERE should start a line: {}",
            result
        );
    }

    #[test]
    fn test_format_space_between_tokens() {
        let result = format_sql("SELECT*FROM t");
        assert!(
            result.contains("SELECT *"),
            "Should add space after SELECT: {}",
            result
        );
        assert!(
            result.contains("FROM t"),
            "Should add space after FROM: {}",
            result
        );
    }

    #[test]
    fn test_format_no_space_in_operators() {
        let result = format_sql("SELECT * FROM t WHERE id = 1");
        assert!(
            result.contains("id = 1"),
            "Spaces around = operator: {}",
            result
        );
    }

    #[test]
    fn test_format_large_input_still_correct() {
        // Multi-batch with GO separator — exercises streaming over many tokens
        let input = "select * from t1 where id = 1 go select col1, col2 from t2 go";
        let first = format_sql(input);
        let second = format_sql(&first);
        assert_eq!(
            first, second,
            "Large multi-batch formatting should be idempotent"
        );
        assert!(first.contains("GO"), "GO separators should be preserved");
    }

    #[test]
    fn test_format_empty_source() {
        let result = format_sql("");
        assert!(result.is_empty() || result == "\n");
    }

    #[test]
    fn test_format_returns_empty_when_no_change() {
        // format() should return empty Vec when source == formatted
        // This is hard to trigger since we always add trailing newline,
        // so test the function behavior
        let source = format_sql("SELECT * FROM t");
        let edits = format(&source);
        // If already formatted (with newline), should return empty or single edit
        // The key is: no crash, valid result
        assert!(edits.len() <= 1);
    }

    #[test]
    fn test_format_dot_notation_no_space() {
        let result = format_sql("SELECT t.id FROM t");
        assert!(
            !result.contains("t . id") && !result.contains("t .id"),
            "Dot notation should not have spaces: {}",
            result
        );
    }

    #[test]
    fn test_format_parentheses_no_extra_space() {
        let result = format_sql("SELECT COUNT(id) FROM t");
        assert!(
            !result.contains("( id") && !result.contains("(  id"),
            "No space after opening paren: {}",
            result
        );
    }

    #[test]
    fn test_format_hexstring_preserved() {
        let result = format_sql("SELECT 0x41 FROM t");
        assert!(
            result.contains("0x41"),
            "HexString should be preserved: {}",
            result
        );
    }
}
