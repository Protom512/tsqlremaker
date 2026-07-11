//! # SAP ASE Language Server Core
//!
//! SAP ASE (Sybase) T-SQL 向け Language Server のコアロジック。
//! 既存の `tsql-lexer`, `tsql-parser` を基盤として LSP 機能を提供する。

#![warn(missing_docs)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

pub mod analysis;
pub mod code_actions;
pub mod completion;
pub mod config;
pub mod db_docs;
pub mod definition;
pub mod diagnostics;
pub mod folding;
pub mod formatting;
pub mod hover;
pub mod incremental;
pub mod line_index;
pub mod references;
pub mod rename;
pub mod semantic_tokens;
pub mod signature_help;
pub mod span_resolve;
pub mod symbol_store;
pub mod symbol_table;
pub mod symbols;
pub mod workspace_index;
pub mod workspace_symbols;

pub use tsql_parser::Parser;

/// トークンがシンボル名にマッチするかを判定する（共有ユーティリティ）
///
/// 変数（@var）の場合は `LocalVar` トークンのみマッチ。
/// その他の場合は `Ident` またはSQLキーワードトークンとマッチ。
///
/// # なぜキーワードを明示的に列挙するか
///
/// T-SQLでは `CREATE TABLE users` の `users` は `Ident` トークンだが、
/// `CREATE PROCEDURE my_proc` の `my_proc` も同様に `Ident` になる。
/// しかし `CREATE TABLE table_name` で `table` と書いた場合、
/// レキサーはこれを `Table` キーワードトークンとして扱う。
///
/// このため、オブジェクト名として頻出する以下のキーワードを
/// シンボル名としても認識する必要がある:
/// - DDL文脈: `SELECT`, `FROM`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`
/// - プロシージャ: `EXEC`, `PROCEDURE`
/// - オブジェクト種別: `TABLE`, `VIEW`, `INDEX`
///
/// `kind.is_keyword()` は残りのキーワードのフォールバックとして機能する。
#[inline]
pub(crate) fn token_matches_symbol(
    kind: tsql_token::TokenKind,
    text: &str,
    search_upper: &str,
    is_var: bool,
) -> bool {
    let kind_ok = if is_var {
        kind == tsql_token::TokenKind::LocalVar
    } else {
        // Ident and any keyword can be an object name in T-SQL
        // (e.g., table named "Select", "Create", etc.)
        kind == tsql_token::TokenKind::Ident || kind.is_keyword()
    };
    // search_upper is already uppercase, so ascii-case-insensitive compare
    // avoids allocating a new String via text.to_uppercase() on every token.
    kind_ok && text.eq_ignore_ascii_case(search_upper)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_token_matches_symbol_variable() {
        assert!(token_matches_symbol(
            tsql_token::TokenKind::LocalVar,
            "@count",
            "@COUNT",
            true
        ));
        assert!(!token_matches_symbol(
            tsql_token::TokenKind::Ident,
            "count",
            "@COUNT",
            true
        ));
    }

    #[test]
    fn test_token_matches_symbol_identifier() {
        assert!(token_matches_symbol(
            tsql_token::TokenKind::Ident,
            "users",
            "USERS",
            false
        ));
        assert!(!token_matches_symbol(
            tsql_token::TokenKind::Ident,
            "users",
            "ORDERS",
            false
        ));
    }

    #[test]
    fn test_token_matches_symbol_keyword_as_name() {
        assert!(token_matches_symbol(
            tsql_token::TokenKind::Select,
            "select",
            "SELECT",
            false
        ));
        assert!(token_matches_symbol(
            tsql_token::TokenKind::Table,
            "table",
            "TABLE",
            false
        ));
    }

    #[test]
    fn test_token_matches_symbol_case_insensitive() {
        assert!(token_matches_symbol(
            tsql_token::TokenKind::Ident,
            "Users",
            "USERS",
            false
        ));
        assert!(token_matches_symbol(
            tsql_token::TokenKind::LocalVar,
            "@Count",
            "@COUNT",
            true
        ));
    }

    #[test]
    fn test_token_matches_symbol_exec_keyword() {
        // EXEC is listed explicitly as a keyword that can be an object name
        assert!(token_matches_symbol(
            tsql_token::TokenKind::Exec,
            "exec",
            "EXEC",
            false
        ));
    }

    #[test]
    fn test_token_matches_symbol_string_not_matched() {
        assert!(!token_matches_symbol(
            tsql_token::TokenKind::String,
            "users",
            "USERS",
            false
        ));
    }
}
