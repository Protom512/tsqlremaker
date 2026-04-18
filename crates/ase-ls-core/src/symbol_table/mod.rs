//! Symbol Table Builder
//!
//! ASTからシンボル情報を抽出し、ナビゲーション機能の基盤を提供する。
//! - テーブル定義（CREATE TABLE）からカラム情報を抽出
//! - プロシージャ定義（CREATE PROCEDURE）からパラメータと変数を抽出
//! - ビュー定義（CREATE VIEW）とインデックス定義（CREATE INDEX）を抽出
//! - 変数宣言（DECLARE）から変数情報を抽出

#![allow(missing_docs)]

use lsp_types::{Position, Range};
use std::collections::HashMap;
use tsql_parser::ast::{CreateStatement, DataType, DeclareStatement, Statement};
use tsql_token::Span;

use crate::{offset_to_position, position_to_offset};

/// シンボルテーブル
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    /// テーブル定義 (名前uppercase → TableSymbol)
    pub tables: HashMap<String, TableSymbol>,
    /// プロシージャ定義 (名前uppercase → ProcedureSymbol)
    pub procedures: HashMap<String, ProcedureSymbol>,
    /// ビュー定義 (名前uppercase → ViewSymbol)
    pub views: HashMap<String, ViewSymbol>,
    /// インデックス定義 (名前uppercase → IndexSymbol)
    pub indexes: HashMap<String, IndexSymbol>,
    /// 変数定義 (名前uppercase → VariableSymbol)
    pub variables: HashMap<String, VariableSymbol>,
}

/// テーブルシンボル
#[derive(Debug, Clone)]
pub struct TableSymbol {
    pub name: String,
    pub range: Range,
    pub columns: Vec<ColumnSymbol>,
    pub constraints: Vec<ConstraintInfo>,
    pub is_temporary: bool,
}

/// カラムシンボル
#[derive(Debug, Clone)]
pub struct ColumnSymbol {
    pub name: String,
    pub range: Range,
    pub data_type: DataType,
    pub nullable: Option<bool>,
    pub is_identity: bool,
    pub table_name: String,
}

/// 制約情報
#[derive(Debug, Clone)]
pub struct ConstraintInfo {
    pub name: Option<String>,
    pub kind: ConstraintKind,
}

/// 制約種別
#[derive(Debug, Clone)]
pub enum ConstraintKind {
    PrimaryKey {
        columns: Vec<String>,
    },
    Foreign {
        columns: Vec<String>,
        ref_table: String,
        ref_columns: Vec<String>,
    },
    Unique {
        columns: Vec<String>,
    },
}

/// プロシージャシンボル
#[derive(Debug, Clone)]
pub struct ProcedureSymbol {
    pub name: String,
    pub range: Range,
    pub parameters: Vec<ParameterSymbol>,
    /// プロシージャボディ内の変数
    pub body_variables: Vec<VariableSymbol>,
}

/// パラメータシンボル
#[derive(Debug, Clone)]
pub struct ParameterSymbol {
    pub name: String,
    pub range: Range,
    pub data_type: DataType,
    pub is_output: bool,
}

/// ビューシンボル
#[derive(Debug, Clone)]
pub struct ViewSymbol {
    pub name: String,
    pub range: Range,
}

/// インデックスシンボル
#[derive(Debug, Clone)]
pub struct IndexSymbol {
    pub name: String,
    pub range: Range,
    pub table_name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
}

/// 変数シンボル
#[derive(Debug, Clone)]
pub struct VariableSymbol {
    pub name: String,
    pub range: Range,
    pub data_type: DataType,
}

/// シンボルテーブルビルダー
pub struct SymbolTableBuilder;

impl SymbolTableBuilder {
    /// ソースコードからシンボルテーブルを構築する
    pub fn build(source: &str) -> SymbolTable {
        let mut table = SymbolTable::default();

        let statements = match tsql_parser::Parser::new(source).parse() {
            Ok(s) => s,
            Err(_) => return table,
        };

        for stmt in &statements {
            Self::collect_from_stmt(source, stmt, &mut table);
        }

        table
    }

    /// ソースコードからシンボルテーブルを構築（エラー許容）
    pub fn build_tolerant(source: &str) -> SymbolTable {
        let mut table = SymbolTable::default();

        let statements = match tsql_parser::Parser::new(source).parse_with_errors() {
            Ok((s, _)) => s,
            Err(_) => return table,
        };

        for stmt in &statements {
            Self::collect_from_stmt(source, stmt, &mut table);
        }

        table
    }

    fn collect_from_stmt(source: &str, stmt: &Statement, table: &mut SymbolTable) {
        match stmt {
            Statement::Create(create) => match create.as_ref() {
                CreateStatement::Table(td) => {
                    let name_upper = td.name.name.to_uppercase();
                    let columns: Vec<ColumnSymbol> = td
                        .columns
                        .iter()
                        .map(|col| ColumnSymbol {
                            name: col.name.name.clone(),
                            range: span_to_range(source, col.name.span),
                            data_type: col.data_type.clone(),
                            nullable: col.nullability,
                            is_identity: col.identity,
                            table_name: name_upper.clone(),
                        })
                        .collect();

                    let constraints: Vec<ConstraintInfo> = td
                        .constraints
                        .iter()
                        .filter_map(|c| match c {
                            tsql_parser::ast::TableConstraint::PrimaryKey {
                                name,
                                columns: cols,
                            } => Some(ConstraintInfo {
                                name: name.as_ref().map(|n| n.name.clone()),
                                kind: ConstraintKind::PrimaryKey {
                                    columns: cols.iter().map(|c| c.name.clone()).collect(),
                                },
                            }),
                            tsql_parser::ast::TableConstraint::Foreign {
                                name,
                                columns: cols,
                                ref_table,
                                ref_columns,
                            } => Some(ConstraintInfo {
                                name: name.as_ref().map(|n| n.name.clone()),
                                kind: ConstraintKind::Foreign {
                                    columns: cols.iter().map(|c| c.name.clone()).collect(),
                                    ref_table: ref_table.name.clone(),
                                    ref_columns: ref_columns
                                        .iter()
                                        .map(|c| c.name.clone())
                                        .collect(),
                                },
                            }),
                            tsql_parser::ast::TableConstraint::Unique {
                                name,
                                columns: cols,
                            } => Some(ConstraintInfo {
                                name: name.as_ref().map(|n| n.name.clone()),
                                kind: ConstraintKind::Unique {
                                    columns: cols.iter().map(|c| c.name.clone()).collect(),
                                },
                            }),
                            tsql_parser::ast::TableConstraint::Check { .. } => None,
                        })
                        .collect();

                    table.tables.insert(
                        name_upper,
                        TableSymbol {
                            name: td.name.name.clone(),
                            range: span_to_range(source, td.name.span),
                            columns,
                            constraints,
                            is_temporary: td.temporary,
                        },
                    );
                }
                CreateStatement::Procedure(pd) => {
                    let name_upper = pd.name.name.to_uppercase();
                    let parameters: Vec<ParameterSymbol> = pd
                        .parameters
                        .iter()
                        .map(|p| ParameterSymbol {
                            name: p.name.name.clone(),
                            range: span_to_range(source, p.name.span),
                            data_type: p.data_type.clone(),
                            is_output: p.is_output,
                        })
                        .collect();

                    // プロシージャボディ内の変数を収集
                    let mut body_variables = Vec::new();
                    for body_stmt in &pd.body {
                        Self::collect_variables(source, body_stmt, &mut body_variables);
                    }

                    table.procedures.insert(
                        name_upper,
                        ProcedureSymbol {
                            name: pd.name.name.clone(),
                            range: span_to_range(source, pd.name.span),
                            parameters,
                            body_variables,
                        },
                    );
                }
                CreateStatement::View(vd) => {
                    let name_upper = vd.name.name.to_uppercase();
                    table.views.insert(
                        name_upper,
                        ViewSymbol {
                            name: vd.name.name.clone(),
                            range: span_to_range(source, vd.name.span),
                        },
                    );
                }
                CreateStatement::Index(idx) => {
                    let name_upper = idx.name.name.to_uppercase();
                    table.indexes.insert(
                        name_upper,
                        IndexSymbol {
                            name: idx.name.name.clone(),
                            range: span_to_range(source, idx.name.span),
                            table_name: idx.table.name.clone(),
                            columns: idx.columns.iter().map(|c| c.name.clone()).collect(),
                            is_unique: idx.unique,
                        },
                    );
                }
            },
            Statement::Declare(decl) => {
                Self::collect_declare_variables(source, decl, &mut table.variables);
            }
            Statement::Block(block) => {
                for s in &block.statements {
                    Self::collect_from_stmt(source, s, table);
                }
            }
            Statement::If(if_stmt) => {
                Self::collect_from_stmt(source, &if_stmt.then_branch, table);
                if let Some(else_branch) = &if_stmt.else_branch {
                    Self::collect_from_stmt(source, else_branch, table);
                }
            }
            Statement::While(while_stmt) => {
                Self::collect_from_stmt(source, &while_stmt.body, table);
            }
            Statement::TryCatch(tc) => {
                for s in &tc.try_block.statements {
                    Self::collect_from_stmt(source, s, table);
                }
                for s in &tc.catch_block.statements {
                    Self::collect_from_stmt(source, s, table);
                }
            }
            _ => {}
        }
    }

    /// DECLARE文から変数を収集する
    fn collect_declare_variables(
        source: &str,
        decl: &DeclareStatement,
        variables: &mut HashMap<String, VariableSymbol>,
    ) {
        for var in &decl.variables {
            let name_upper = var.name.name.to_uppercase();
            variables.insert(
                name_upper,
                VariableSymbol {
                    name: var.name.name.clone(),
                    range: span_to_range(source, var.name.span),
                    data_type: var.data_type.clone(),
                },
            );
        }
    }

    /// ステートメント内の変数を再帰的に収集する
    fn collect_variables(source: &str, stmt: &Statement, variables: &mut Vec<VariableSymbol>) {
        match stmt {
            Statement::Declare(decl) => {
                for var in &decl.variables {
                    variables.push(VariableSymbol {
                        name: var.name.name.clone(),
                        range: span_to_range(source, var.name.span),
                        data_type: var.data_type.clone(),
                    });
                }
            }
            Statement::Block(block) => {
                for s in &block.statements {
                    Self::collect_variables(source, s, variables);
                }
            }
            Statement::If(if_stmt) => {
                Self::collect_variables(source, &if_stmt.then_branch, variables);
                if let Some(else_branch) = &if_stmt.else_branch {
                    Self::collect_variables(source, else_branch, variables);
                }
            }
            Statement::While(while_stmt) => {
                Self::collect_variables(source, &while_stmt.body, variables);
            }
            Statement::TryCatch(tc) => {
                for s in &tc.try_block.statements {
                    Self::collect_variables(source, s, variables);
                }
                for s in &tc.catch_block.statements {
                    Self::collect_variables(source, s, variables);
                }
            }
            _ => {}
        }
    }

    /// テーブル名でテーブルを検索
    pub fn find_table<'a>(table: &'a SymbolTable, name: &str) -> Option<&'a TableSymbol> {
        table.tables.get(&name.to_uppercase())
    }

    /// プロシージャ名でプロシージャを検索
    pub fn find_procedure<'a>(table: &'a SymbolTable, name: &str) -> Option<&'a ProcedureSymbol> {
        table.procedures.get(&name.to_uppercase())
    }

    /// 変数名で変数を検索
    pub fn find_variable<'a>(table: &'a SymbolTable, name: &str) -> Option<&'a VariableSymbol> {
        // 変数名は@付きで格納されている
        let search_name = if name.starts_with('@') {
            name.to_uppercase()
        } else {
            format!("@{}", name.to_uppercase())
        };
        table.variables.get(&search_name)
    }

    /// カーソル位置の識別子を特定
    pub fn find_identifier_at(
        source: &str,
        position: Position,
    ) -> Option<(String, lsp_types::Range)> {
        let offset = position_to_offset(source, position);

        for token_result in tsql_lexer::Lexer::new(source) {
            let token = match token_result {
                Ok(t) => t,
                Err(_) => continue,
            };
            let start = token.span.start as usize;
            let end = token.span.end as usize;
            if offset >= start && offset < end {
                if matches!(
                    token.kind,
                    tsql_token::TokenKind::Ident | tsql_token::TokenKind::LocalVar
                ) {
                    return Some((token.text.to_string(), span_to_range(source, token.span)));
                }
                return None;
            }
            if start > offset {
                break;
            }
        }
        None
    }
}

/// Span → LSP Range 変換
fn span_to_range(source: &str, span: Span) -> Range {
    let (start_line, start_char) = offset_to_position(source, span.start);
    let (end_line, end_char) = offset_to_position(source, span.end);
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
    fn test_build_table_symbol() {
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))";
        let table = SymbolTableBuilder::build(source);

        let users = table.tables.get("USERS").expect("table should exist");
        assert_eq!(users.name, "users");
        assert_eq!(users.columns.len(), 2);
        assert_eq!(users.columns[0].name, "id");
        assert_eq!(users.columns[1].name, "name");
        assert!(!users.is_temporary);
    }

    #[test]
    fn test_build_procedure_symbol() {
        let source =
            "CREATE PROCEDURE my_proc @p1 INT, @p2 VARCHAR(50) OUTPUT AS BEGIN DECLARE @x INT SET @x = 1 END";
        let table = SymbolTableBuilder::build(source);

        let proc = table.procedures.get("MY_PROC").expect("proc should exist");
        assert_eq!(proc.name, "my_proc");
        assert_eq!(proc.parameters.len(), 2);
        assert_eq!(proc.parameters[0].name, "@p1");
        assert_eq!(proc.parameters[1].name, "@p2");
        assert!(proc.parameters[1].is_output);
        assert_eq!(proc.body_variables.len(), 1);
        assert_eq!(proc.body_variables[0].name, "@x");
    }

    #[test]
    fn test_build_view_symbol() {
        let source = "CREATE VIEW active_users AS SELECT * FROM users";
        let table = SymbolTableBuilder::build(source);

        let view = table.views.get("ACTIVE_USERS").expect("view should exist");
        assert_eq!(view.name, "active_users");
    }

    #[test]
    fn test_build_index_symbol() {
        let source = "CREATE INDEX idx_users_id ON users (id)";
        let table = SymbolTableBuilder::build(source);

        let idx = table
            .indexes
            .get("IDX_USERS_ID")
            .expect("index should exist");
        assert_eq!(idx.name, "idx_users_id");
        assert_eq!(idx.table_name, "users");
        assert!(!idx.is_unique);
        assert_eq!(idx.columns, vec!["id"]);
    }

    #[test]
    fn test_build_declare_variables() {
        let source = "DECLARE @count INT\nDECLARE @name VARCHAR(100)";
        let table = SymbolTableBuilder::build(source);

        let count_var = table.variables.get("@COUNT").expect("var should exist");
        assert_eq!(count_var.name, "@count");
        assert!(matches!(count_var.data_type, DataType::Int));

        let name_var = table.variables.get("@NAME").expect("var should exist");
        assert_eq!(name_var.name, "@name");
    }

    #[test]
    fn test_find_table() {
        let source = "CREATE TABLE users (id INT)";
        let table = SymbolTableBuilder::build(source);

        assert!(SymbolTableBuilder::find_table(&table, "users").is_some());
        assert!(SymbolTableBuilder::find_table(&table, "USERS").is_some());
        assert!(SymbolTableBuilder::find_table(&table, "nonexistent").is_none());
    }

    #[test]
    fn test_find_variable() {
        let source = "DECLARE @count INT";
        let table = SymbolTableBuilder::build(source);

        assert!(SymbolTableBuilder::find_variable(&table, "@count").is_some());
        assert!(SymbolTableBuilder::find_variable(&table, "@COUNT").is_some());
        assert!(SymbolTableBuilder::find_variable(&table, "count").is_some());
        assert!(SymbolTableBuilder::find_variable(&table, "@other").is_none());
    }

    #[test]
    fn test_variable_in_nested_block() {
        let source = "CREATE PROCEDURE test_proc AS BEGIN DECLARE @x INT IF 1 = 1 BEGIN DECLARE @y INT END END";
        let table = SymbolTableBuilder::build(source);

        let proc = table
            .procedures
            .get("TEST_PROC")
            .expect("proc should exist");
        assert_eq!(proc.body_variables.len(), 2);
        let names: Vec<&str> = proc
            .body_variables
            .iter()
            .map(|v| v.name.as_str())
            .collect();
        assert!(names.contains(&"@x"));
        assert!(names.contains(&"@y"));
    }

    #[test]
    fn test_table_constraints() {
        let source = "CREATE TABLE orders (
            id INT,
            user_id INT,
            CONSTRAINT pk_orders PRIMARY KEY (id),
            CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users (id)
        )";
        let table = SymbolTableBuilder::build(source);

        let orders = table.tables.get("ORDERS").expect("table should exist");
        assert_eq!(orders.constraints.len(), 2);
        assert!(matches!(
            &orders.constraints[0].kind,
            ConstraintKind::PrimaryKey { columns } if columns[0] == "id"
        ));
        assert!(matches!(
            &orders.constraints[1].kind,
            ConstraintKind::Foreign { ref_table, .. } if ref_table == "users"
        ));
    }

    #[test]
    fn test_empty_source() {
        let source = "";
        let table = SymbolTableBuilder::build(source);
        assert!(table.tables.is_empty());
        assert!(table.procedures.is_empty());
        assert!(table.views.is_empty());
    }

    #[test]
    fn test_find_identifier_at() {
        let source = "SELECT * FROM users";
        let result = SymbolTableBuilder::find_identifier_at(
            source,
            Position {
                line: 0,
                character: 15,
            },
        );
        assert!(result.is_some());
        let (name, _range) = result.unwrap();
        assert_eq!(name, "users");
    }
}
