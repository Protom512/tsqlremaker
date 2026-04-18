//! # SAP ASE Language Server Core
//!
//! SAP ASE (Sybase) T-SQL 向け Language Server のコアロジック。
//! 既存の `tsql-lexer`, `tsql-parser` を基盤として LSP 機能を提供する。

#![warn(missing_docs)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

pub mod code_actions;
pub mod completion;
pub mod db_docs;
pub mod definition;
pub mod diagnostics;
pub mod folding;
pub mod formatting;
pub mod hover;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod signature_help;
pub mod symbol_table;
pub mod symbols;
pub mod workspace_symbols;

pub use tsql_lexer::Lexer;
pub use tsql_parser::Parser;

/// バイトオフセットから (0-indexed line, 0-indexed character) を計算する
pub(crate) fn offset_to_position(source: &str, offset: u32) -> (u32, u32) {
    let mut line = 0u32;
    let mut last_newline = 0u32;
    let bytes = source.as_bytes();
    let end = (offset as usize).min(bytes.len());
    for (i, &b) in bytes.iter().enumerate().take(end) {
        if b == b'\n' {
            line += 1;
            last_newline = (i + 1) as u32;
        }
    }
    let character = offset.saturating_sub(last_newline);
    (line, character)
}

/// LSP Position (0-indexed) からバイトオフセットを計算する
pub(crate) fn position_to_offset(source: &str, position: lsp_types::Position) -> usize {
    let mut offset = 0;
    let mut current_line = 0u32;

    for ch in source.chars() {
        if current_line == position.line {
            let char_offset = offset;
            let chars_to_target = position.character as usize;
            let mut counted = 0;
            for c in source[char_offset..].chars() {
                if counted >= chars_to_target {
                    return char_offset + counted;
                }
                counted += c.len_utf8();
            }
            return char_offset + counted;
        }
        offset += ch.len_utf8();
        if ch == '\n' {
            current_line += 1;
        }
    }

    offset
}

/// カーソル位置のトークンを特定する（共有ユーティリティ）
///
/// 指定バイトオフセットに含まれるトークンの種類とテキストを返す。
pub(crate) fn find_token_at(
    source: &str,
    offset: usize,
) -> Option<(tsql_token::TokenKind, String)> {
    for token_result in Lexer::new(source) {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };
        let start = token.span.start as usize;
        let end = token.span.end as usize;
        if offset >= start && offset < end {
            return Some((token.kind, token.text.to_string()));
        }
        if start > offset {
            break;
        }
    }
    None
}

/// トークンがシンボル名にマッチするかを判定する（共有ユーティリティ）
///
/// 変数（@var）の場合は `LocalVar` トークンのみマッチ。
/// その他の場合は `Ident` またはSQLキーワードトークンとマッチ。
pub(crate) fn token_matches_symbol(
    kind: tsql_token::TokenKind,
    text: &str,
    search_upper: &str,
    is_var: bool,
) -> bool {
    if is_var {
        kind == tsql_token::TokenKind::LocalVar && text.to_uppercase() == search_upper
    } else {
        (kind == tsql_token::TokenKind::Ident
            || matches!(
                kind,
                tsql_token::TokenKind::Select
                    | tsql_token::TokenKind::From
                    | tsql_token::TokenKind::Insert
                    | tsql_token::TokenKind::Update
                    | tsql_token::TokenKind::Delete
                    | tsql_token::TokenKind::Create
                    | tsql_token::TokenKind::Exec
                    | tsql_token::TokenKind::Procedure
                    | tsql_token::TokenKind::Table
                    | tsql_token::TokenKind::View
                    | tsql_token::TokenKind::Index
            )
            || kind.is_keyword())
            && text.to_uppercase() == search_upper
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_to_position_start() {
        let (line, col) = offset_to_position("SELECT * FROM t", 0);
        assert_eq!(line, 0);
        assert_eq!(col, 0);
    }

    #[test]
    fn test_offset_to_position_mid_line() {
        let (line, col) = offset_to_position("SELECT * FROM t", 7);
        assert_eq!(line, 0);
        assert_eq!(col, 7);
    }

    #[test]
    fn test_offset_to_position_second_line() {
        let source = "SELECT *\nFROM t";
        let (line, col) = offset_to_position(source, 9);
        assert_eq!(line, 1);
        assert_eq!(col, 0);
    }

    #[test]
    fn test_offset_to_position_multiline() {
        let source = "line1\nline2\nline3";
        let (line, col) = offset_to_position(source, 12);
        assert_eq!(line, 2);
        assert_eq!(col, 0);
    }
}
