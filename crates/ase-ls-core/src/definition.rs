//! Go to Definition provider
//!
//! カーソル位置のシンボルから定義箇所へナビゲーションを提供する。
//! - 変数参照 → DECLARE文
//! - テーブル参照 → CREATE TABLE定義
//! - プロシージャ参照 → CREATE PROCEDURE定義
//! - ビュー参照 → CREATE VIEW定義

use crate::{offset_to_position, position_to_offset};
use lsp_types::{Position, Range};
use tsql_lexer::Lexer;
use tsql_parser::ast::{CreateStatement, Statement};
use tsql_token::TokenKind;

/// カーソル位置のシンボルの定義箇所を検索する
///
/// 戻り値は定義箇所のRangeのリスト。空の場合は定義なし。
pub fn definition_ranges(source: &str, position: Position) -> Vec<Range> {
    let offset = position_to_offset(source, position);

    let (target_kind, target_text) = match find_token_at(source, offset) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let statements = match tsql_parser::Parser::new(source).parse_with_errors() {
        Ok((s, _)) => s,
        Err(_) => return Vec::new(),
    };

    let definitions = collect_definitions(source, &statements);

    let search_name = if target_kind == TokenKind::LocalVar {
        target_text.trim_start_matches('@').to_uppercase()
    } else {
        target_text.to_uppercase()
    };

    let is_var = target_kind == TokenKind::LocalVar;

    definitions
        .into_iter()
        .filter(|def| match def {
            Definition::Variable { name, .. } if is_var => *name == search_name,
            Definition::Table { name, .. } if !is_var => *name == search_name,
            Definition::Procedure { name, .. } if !is_var => *name == search_name,
            Definition::View { name, .. } if !is_var => *name == search_name,
            Definition::Index { name, .. } if !is_var => *name == search_name,
            _ => false,
        })
        .map(|def| match def {
            Definition::Variable { range, .. } => range,
            Definition::Table { range, .. } => range,
            Definition::Procedure { range, .. } => range,
            Definition::View { range, .. } => range,
            Definition::Index { range, .. } => range,
        })
        .collect()
}

/// シンボル定義の情報
#[derive(Debug)]
enum Definition {
    Variable { name: String, range: Range },
    Table { name: String, range: Range },
    Procedure { name: String, range: Range },
    View { name: String, range: Range },
    Index { name: String, range: Range },
}

/// AST内の全定義を収集する
fn collect_definitions(source: &str, statements: &[Statement]) -> Vec<Definition> {
    let mut defs = Vec::new();
    for stmt in statements {
        collect_defs_from_stmt(source, stmt, &mut defs);
    }
    defs
}

/// 単一Statementから定義を再帰的に収集する
fn collect_defs_from_stmt(source: &str, stmt: &Statement, defs: &mut Vec<Definition>) {
    match stmt {
        Statement::Declare(decl) => {
            for var in &decl.variables {
                defs.push(Definition::Variable {
                    name: var
                        .name
                        .name
                        .to_uppercase()
                        .trim_start_matches('@')
                        .to_string(),
                    range: span_to_range(source, var.name.span.start, var.name.span.end),
                });
            }
        }
        Statement::Create(create) => match create.as_ref() {
            CreateStatement::Table(td) => {
                defs.push(Definition::Table {
                    name: td.name.name.to_uppercase(),
                    range: span_to_range(source, td.name.span.start, td.name.span.end),
                });
            }
            CreateStatement::Procedure(pd) => {
                defs.push(Definition::Procedure {
                    name: pd.name.name.to_uppercase(),
                    range: span_to_range(source, pd.name.span.start, pd.name.span.end),
                });
                for body_stmt in &pd.body {
                    collect_defs_from_stmt(source, body_stmt, defs);
                }
            }
            CreateStatement::View(vd) => {
                defs.push(Definition::View {
                    name: vd.name.name.to_uppercase(),
                    range: span_to_range(source, vd.name.span.start, vd.name.span.end),
                });
            }
            CreateStatement::Index(idx) => {
                defs.push(Definition::Index {
                    name: idx.name.name.to_uppercase(),
                    range: span_to_range(source, idx.name.span.start, idx.name.span.end),
                });
            }
        },
        Statement::Block(block) => {
            for s in &block.statements {
                collect_defs_from_stmt(source, s, defs);
            }
        }
        Statement::If(if_stmt) => {
            collect_defs_from_stmt(source, &if_stmt.then_branch, defs);
            if let Some(else_branch) = &if_stmt.else_branch {
                collect_defs_from_stmt(source, else_branch, defs);
            }
        }
        Statement::While(while_stmt) => {
            collect_defs_from_stmt(source, &while_stmt.body, defs);
        }
        Statement::TryCatch(tc) => {
            for s in &tc.try_block.statements {
                collect_defs_from_stmt(source, s, defs);
            }
            for s in &tc.catch_block.statements {
                collect_defs_from_stmt(source, s, defs);
            }
        }
        _ => {}
    }
}

/// カーソル位置のトークンを特定する
fn find_token_at(source: &str, offset: usize) -> Option<(TokenKind, String)> {
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

/// バイトオフセット範囲からLSP Rangeを生成
fn span_to_range(source: &str, start: u32, end: u32) -> Range {
    let (start_line, start_char) = offset_to_position(source, start);
    let (end_line, end_char) = offset_to_position(source, end);
    Range {
        start: Position {
            line: start_line,
            character: start_char,
        },
        end: Position {
            line: end_line,
            character: end_char,
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_goto_variable_definition() {
        let source = "DECLARE @count INT\nSET @count = 1\nSELECT @count";
        // Cursor on @count in SET statement (line 1, char 5)
        let ranges = definition_ranges(
            source,
            Position {
                line: 1,
                character: 5,
            },
        );
        assert_eq!(ranges.len(), 1);
        // Should point to @count in DECLARE (line 0)
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_goto_table_definition() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT * FROM users";
        // Cursor on "users" in SELECT FROM (line 1, char 15)
        let ranges = definition_ranges(
            source,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert_eq!(ranges.len(), 1);
        // Should point to "users" in CREATE TABLE (line 0)
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_goto_procedure_definition() {
        let source = "CREATE PROCEDURE my_proc @p1 INT AS BEGIN RETURN @p1 END";
        // Cursor on "my_proc" in CREATE PROCEDURE (line 0)
        let ranges = definition_ranges(
            source,
            Position {
                line: 0,
                character: 18,
            },
        );
        assert_eq!(ranges.len(), 1);
        // Should point to procedure name
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_goto_no_definition_found() {
        let source = "SELECT * FROM users";
        let ranges = definition_ranges(
            source,
            Position {
                line: 0,
                character: 15,
            },
        );
        // No CREATE TABLE for "users" in this file
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_goto_variable_in_procedure_body() {
        let source = "CREATE PROCEDURE test_proc AS BEGIN DECLARE @x INT SET @x = 1 END";
        // Cursor on @x in SET (after "DECLARE @x INT SET ")
        let set_pos = source.find("SET @x").unwrap() + 5;
        let (line, char) = offset_to_position(source, set_pos as u32);
        let ranges = definition_ranges(
            source,
            Position {
                line,
                character: char,
            },
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_goto_view_definition() {
        let source = "CREATE VIEW active_users AS SELECT * FROM users\nSELECT * FROM active_users";
        // Cursor on "active_users" in SELECT (line 1)
        let ranges = definition_ranges(
            source,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_goto_whitespace_returns_empty() {
        let source = "SELECT  FROM t";
        let ranges = definition_ranges(
            source,
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(ranges.is_empty());
    }
}
