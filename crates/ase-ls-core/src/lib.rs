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
pub mod code_lens;
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
/// その他の場合は `Ident`、SQLキーワード、または一時テーブル識別子トークンとマッチ。
///
/// # 一時テーブル識別子（#temp / ##global）
///
/// ASE では `CREATE TABLE #temp` や `SELECT * FROM #temp` の `#temp` は
/// `TokenKind::TempTable`（ローカル）、`##global` は `TokenKind::GlobalTempTable`
/// （グローバル）として字句化される。これらは `is_keyword()`=false かつ `Ident`
/// でもないため、明示的にマッチ対象に含めないと FROM 句の `#temp` が
/// 定義シンボルとマッチせず、goto-definition / references / rename が一斉に失敗する。
/// レキサーはトークン text に `#`/`##` プレフィックスを含めた状態で生成し、
/// シンボルテーブルも同名で登録するため、プレフィックス込みで比較する。
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
        // TempTable/GlobalTempTable は lexer が text に "#"/"##" プレフィックスを
        // 含めた状態で生成し (lexer.rs:294)、ddl.rs:189 が同 text をシンボル名として
        // 登録するため、これらの kind もオブジェクト名としてマッチ対象に含める。
        // is_keyword() の明示リストには含まれないため、ここで個別に許容する。
        kind == tsql_token::TokenKind::Ident
            || kind.is_keyword()
            || matches!(
                kind,
                tsql_token::TokenKind::TempTable | tsql_token::TokenKind::GlobalTempTable
            )
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

    #[test]
    fn test_token_matches_symbol_temp_table_matches() {
        // lexer.rs:294 が '#' プレフィックスを含む text を生成するため、
        // トークン text は "#temp" になる（find_token_at_position の戻り値形状）。
        // search_upper はシンボルテーブル登録名（ddl.rs:189 が同 text を name に格納）。
        assert!(token_matches_symbol(
            tsql_token::TokenKind::TempTable,
            "#temp",
            "#TEMP",
            false
        ));
        // 大文字小文字混在もケース無視でマッチすること
        assert!(token_matches_symbol(
            tsql_token::TokenKind::TempTable,
            "#Temp",
            "#TEMP",
            false
        ));
    }

    #[test]
    fn test_token_matches_symbol_global_temp_table_matches() {
        // ## プレフィックス（グローバル一時テーブル）もマッチすること
        assert!(token_matches_symbol(
            tsql_token::TokenKind::GlobalTempTable,
            "##global",
            "##GLOBAL",
            false
        ));
        assert!(token_matches_symbol(
            tsql_token::TokenKind::GlobalTempTable,
            "##Global",
            "##GLOBAL",
            false
        ));
    }

    #[test]
    fn test_token_matches_symbol_temp_table_name_mismatch() {
        // TempTable トークンであっても名前が異なればマッチしないこと
        assert!(!token_matches_symbol(
            tsql_token::TokenKind::TempTable,
            "#temp",
            "#OTHER",
            false
        ));
        // GlobalTempTable も同様
        assert!(!token_matches_symbol(
            tsql_token::TokenKind::GlobalTempTable,
            "##global",
            "##OTHER",
            false
        ));
    }

    #[test]
    fn test_token_matches_symbol_temp_table_not_matched_as_variable() {
        // 変数検索(is_var=true)では TempTable トークンが誤ヒットしないこと。
        // LocalVar 専用ブランチのため、TempTable は弾かれる必要がある。
        assert!(!token_matches_symbol(
            tsql_token::TokenKind::TempTable,
            "#temp",
            "#TEMP",
            true
        ));
        assert!(!token_matches_symbol(
            tsql_token::TokenKind::GlobalTempTable,
            "##global",
            "##GLOBAL",
            true
        ));
    }
}
