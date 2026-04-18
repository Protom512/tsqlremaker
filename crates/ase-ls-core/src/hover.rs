//! Hover 情報の提供
//!
//! T-SQL キーワード、データ型、組み込み関数、変数のホバー情報を提供する。
//! 静的ドキュメントデータは [`crate::db_docs`] モジュールに集約されている。

use crate::{offset_to_position, position_to_offset};
use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Range};
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// Hover情報を生成する
///
/// カーソル位置のトークンを特定し、対応するドキュメントを返す。
/// まずシンボルテーブルを検索し、見つからなければ静的ドキュメントにフォールバックする。
pub fn hover(source: &str, position: Position) -> Option<Hover> {
    let offset = position_to_offset(source, position);

    let mut hovered_token = None;
    for token_result in Lexer::new(source) {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };
        let start = token.span.start as usize;
        let end = token.span.end as usize;
        if offset >= start && offset < end {
            hovered_token = Some((token.kind, token.text.to_string(), start, end));
            break;
        }
        if start > offset {
            break;
        }
    }

    let (kind, text, start, end) = hovered_token?;

    // シンボルテーブルからスキーマ情報を取得
    let symbol_table = crate::symbol_table::SymbolTableBuilder::build_tolerant(source);
    let content = build_schema_hover(&symbol_table, &kind, &text)
        .or_else(|| build_hover_content(&kind, &text))?;

    let (start_line, start_char) = offset_to_position(source, start as u32);
    let (end_line, end_char) = offset_to_position(source, end as u32);

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: content,
        }),
        range: Some(Range {
            start: Position {
                line: start_line,
                character: start_char,
            },
            end: Position {
                line: end_line,
                character: end_char,
            },
        }),
    })
}

/// シンボルテーブルからスキーマ情報のHoverを構築する
fn build_schema_hover(
    symbol_table: &crate::symbol_table::SymbolTable,
    kind: &TokenKind,
    text: &str,
) -> Option<String> {
    let upper = text.to_uppercase();

    match kind {
        TokenKind::LocalVar => {
            // 変数の型情報を表示
            if let Some(var) =
                crate::symbol_table::SymbolTableBuilder::find_variable(symbol_table, text)
            {
                return Some(format!(
                    "```tsql\n{}: {:?}\n```\n\n**Variable** — Declared with `DECLARE {} {:?}`",
                    text, var.data_type, var.name, var.data_type
                ));
            }
            // プロシージャボディ内変数
            for proc in symbol_table.procedures.values() {
                for body_var in &proc.body_variables {
                    if body_var.name.to_uppercase() == upper {
                        return Some(format!(
                            "```tsql\n{}: {:?}\n```\n\n**Variable** in `{}` — `DECLARE {} {:?}`",
                            text, body_var.data_type, proc.name, body_var.name, body_var.data_type
                        ));
                    }
                }
                for param in &proc.parameters {
                    if param.name.to_uppercase() == upper {
                        let output_marker = if param.is_output { " OUTPUT" } else { "" };
                        return Some(format!(
                            "```tsql\n{}: {:?}{}\n```\n\n**Parameter** of `{}`",
                            text, param.data_type, output_marker, proc.name
                        ));
                    }
                }
            }
            None
        }
        TokenKind::Ident => {
            // テーブルのカラム情報を表示
            if let Some(table) = symbol_table.tables.get(&upper) {
                let mut cols = String::new();
                for col in &table.columns {
                    let nullable = match col.nullable {
                        Some(true) => " NULL",
                        Some(false) => " NOT NULL",
                        None => "",
                    };
                    let identity = if col.is_identity { " IDENTITY" } else { "" };
                    cols.push_str(&format!(
                        "\n  `{} {:?}`{}{}",
                        col.name, col.data_type, nullable, identity
                    ));
                }
                return Some(format!(
                    "```tsql\nCREATE TABLE {} ({}\n)\n```\n\n**Table** — {} column{}",
                    table.name,
                    cols,
                    table.columns.len(),
                    if table.columns.len() != 1 { "s" } else { "" }
                ));
            }
            // プロシージャ情報を表示
            if let Some(proc) = symbol_table.procedures.get(&upper) {
                let mut params = String::new();
                for p in &proc.parameters {
                    let output = if p.is_output { " OUTPUT" } else { "" };
                    params.push_str(&format!("\n  `{} {:?}{}`", p.name, p.data_type, output));
                }
                return Some(format!(
                    "```tsql\nCREATE PROCEDURE {} ({}\n)\n```\n\n**Procedure** — {} parameter{}",
                    proc.name,
                    params,
                    proc.parameters.len(),
                    if proc.parameters.len() != 1 { "s" } else { "" }
                ));
            }
            // ビュー情報を表示
            if let Some(_view) = symbol_table.views.get(&upper) {
                return Some(format!("**`{}`** — View", text));
            }
            None
        }
        _ => None,
    }
}

/// トークンの種類に応じてHover内容を構築する（静的ドキュメント）
///
/// [`crate::db_docs`] からエントリを検索し、マークダウン形式で返す。
fn build_hover_content(kind: &TokenKind, text: &str) -> Option<String> {
    let upper = text.to_uppercase();

    match kind {
        TokenKind::LocalVar => {
            let var_name = text.trim_start_matches('@');
            Some(format!(
                "```tsql\n{text}: VARIABLE\n```\n\nLocal variable — Declare with `DECLARE @{var_name} TYPE`"
            ))
        }
        _ => {
            if let Some(entry) = crate::db_docs::lookup(upper.as_str()) {
                Some(format!(
                    "```tsql\n{}\n```\n\n**`{}`** — {}",
                    entry.syntax, upper, entry.description
                ))
            } else if kind.is_keyword() {
                Some(format!("**`{upper}`** — T-SQL Keyword"))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_keyword_select() {
        let result = hover(
            "SELECT * FROM t",
            Position {
                line: 0,
                character: 2,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("SELECT"));
                assert!(mc.value.contains("Retrieves data"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_keyword_from() {
        let result = hover(
            "SELECT * FROM t",
            Position {
                line: 0,
                character: 10,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("FROM"));
                assert!(mc.value.contains("source tables"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_datatype_varchar() {
        let result = hover(
            "CREATE TABLE t (col VARCHAR(100))",
            Position {
                line: 0,
                character: 25,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("VARCHAR"));
                assert!(mc.value.contains("Variable-length"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_function_getdate() {
        let result = hover(
            "SELECT GETDATE()",
            Position {
                line: 0,
                character: 9,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("GETDATE"));
                assert!(mc.value.contains("Current"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_variable() {
        let result = hover(
            "SELECT @var",
            Position {
                line: 0,
                character: 8,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("@var"));
                assert!(mc.value.contains("variable"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_whitespace_returns_none() {
        let result = hover(
            "SELECT  FROM t",
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_hover_has_range() {
        let result = hover(
            "SELECT * FROM t",
            Position {
                line: 0,
                character: 2,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        assert!(h.range.is_some());
        let range = h.range.unwrap();
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.character, 6);
    }

    #[test]
    fn test_hover_table_shows_columns() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT * FROM users";
        let result = hover(
            source,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("users"));
                assert!(mc.value.contains("id"));
                assert!(mc.value.contains("name"));
                assert!(mc.value.contains("Table"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_variable_shows_type() {
        let source = "DECLARE @count INT\nSET @count = 1";
        let result = hover(
            source,
            Position {
                line: 1,
                character: 5,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("@count"));
                assert!(mc.value.contains("Int"));
                assert!(mc.value.contains("Variable"));
            }
            _ => panic!("Expected Markup content"),
        }
    }

    #[test]
    fn test_hover_procedure_shows_params() {
        let source =
            "CREATE PROCEDURE my_proc @p1 INT, @p2 VARCHAR(50) OUTPUT AS BEGIN RETURN 1 END";
        let result = hover(
            source,
            Position {
                line: 0,
                character: 18,
            },
        );
        assert!(result.is_some());
        let h = result.unwrap();
        match &h.contents {
            HoverContents::Markup(mc) => {
                assert!(mc.value.contains("my_proc"));
                assert!(mc.value.contains("@p1"));
                assert!(mc.value.contains("@p2"));
                assert!(mc.value.contains("Procedure"));
            }
            _ => panic!("Expected Markup content"),
        }
    }
}
