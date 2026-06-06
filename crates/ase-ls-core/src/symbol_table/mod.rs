//! Symbol Table Builder
//!
//! ASTからシンボル情報を抽出し、ナビゲーション機能の基盤を提供する。
//! - テーブル定義（CREATE TABLE）からカラム情報を抽出
//! - プロシージャ定義（CREATE PROCEDURE）からパラメータと変数を抽出
//! - ビュー定義（CREATE VIEW）とインデックス定義（CREATE INDEX）を抽出
//! - 変数宣言（DECLARE）から変数情報を抽出

use lsp_types::{Position, Range};
use std::borrow::Borrow;
use std::collections::HashMap;
use tsql_parser::ast::{CreateStatement, DataType, DeclareStatement, Statement};
use tsql_token::Span;

use crate::line_index::LineIndex;

/// Case-insensitive key for HashMap lookups.
///
/// Stores the uppercase form; hashes/compares case-insensitively.
/// Implements `Borrow<str>` so `HashMap::get("foo")` works directly.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CaseInsensitiveKey {
    /// Uppercase-normalized name used for hashing and comparison.
    upper: String,
}

impl CaseInsensitiveKey {
    /// Create a new case-insensitive key from any string.
    pub fn new(name: &str) -> Self {
        Self {
            upper: name.to_uppercase(),
        }
    }

    /// Returns the uppercase-normalized key as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.upper
    }
}

impl Borrow<str> for CaseInsensitiveKey {
    fn borrow(&self) -> &str {
        &self.upper
    }
}

impl Borrow<String> for CaseInsensitiveKey {
    fn borrow(&self) -> &String {
        &self.upper
    }
}

/// シンボルテーブル
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    /// テーブル定義 (case-insensitive key → TableSymbol)
    pub tables: HashMap<CaseInsensitiveKey, TableSymbol>,
    /// プロシージャ定義 (case-insensitive key → ProcedureSymbol)
    pub procedures: HashMap<CaseInsensitiveKey, ProcedureSymbol>,
    /// ビュー定義 (case-insensitive key → ViewSymbol)
    pub views: HashMap<CaseInsensitiveKey, ViewSymbol>,
    /// インデックス定義 (case-insensitive key → IndexSymbol)
    pub indexes: HashMap<CaseInsensitiveKey, IndexSymbol>,
    /// トリガー定義 (case-insensitive key → TriggerSymbol)
    pub triggers: HashMap<CaseInsensitiveKey, TriggerSymbol>,
    /// 変数定義 (case-insensitive key → VariableSymbol)
    pub variables: HashMap<CaseInsensitiveKey, VariableSymbol>,
}

impl SymbolTable {
    /// Resolve the semantic token type for an identifier name.
    ///
    /// Returns `Some(9)` (CLASS) for tables, views, and indexes,
    /// `Some(2)` (FUNCTION) for procedures,
    /// or `None` if the name is not a known object.
    ///
    /// Uses a single `CaseInsensitiveKey` allocation to check all maps.
    #[must_use]
    pub fn resolve_semantic_type(&self, name: &str) -> Option<u32> {
        let key = CaseInsensitiveKey::new(name);
        if self.tables.contains_key(&key)
            || self.views.contains_key(&key)
            || self.indexes.contains_key(&key)
        {
            return Some(9); // CLASS
        }
        if self.procedures.contains_key(&key) {
            return Some(2); // FUNCTION
        }
        None
    }
}

/// Table symbol extracted from `CREATE TABLE`.
#[derive(Debug, Clone)]
pub struct TableSymbol {
    /// Table name (original casing).
    pub name: String,
    /// LSP range of the table name in source.
    pub range: Range,
    /// Columns defined in the table.
    pub columns: Vec<ColumnSymbol>,
    /// Constraints defined on the table.
    pub constraints: Vec<ConstraintInfo>,
    /// Whether this is a temporary table (`#` or `##` prefix).
    pub is_temporary: bool,
}

/// Column symbol within a table definition.
#[derive(Debug, Clone)]
pub struct ColumnSymbol {
    /// Column name.
    pub name: String,
    /// LSP range of the column name in source.
    pub range: Range,
    /// SQL data type of the column.
    pub data_type: DataType,
    /// Whether the column is nullable (`None` means unspecified).
    pub nullable: Option<bool>,
    /// Whether the column is an IDENTITY column.
    pub is_identity: bool,
    /// Owning table name (for cross-reference).
    pub table_name: String,
}

/// Constraint information attached to a table.
#[derive(Debug, Clone)]
pub struct ConstraintInfo {
    /// Optional constraint name.
    pub name: Option<String>,
    /// Kind of constraint (primary key, foreign key, unique).
    pub kind: ConstraintKind,
}

/// Kind of table constraint.
#[derive(Debug, Clone)]
pub enum ConstraintKind {
    /// `PRIMARY KEY (columns...)`
    PrimaryKey {
        /// Column names in the primary key.
        columns: Vec<String>,
    },
    /// `FOREIGN KEY (columns...) REFERENCES ref_table (ref_columns)`
    Foreign {
        /// Column names in the foreign key.
        columns: Vec<String>,
        /// Referenced table name.
        ref_table: String,
        /// Referenced column names.
        ref_columns: Vec<String>,
    },
    /// `UNIQUE (columns...)`
    Unique {
        /// Column names in the unique constraint.
        columns: Vec<String>,
    },
}

/// Procedure symbol extracted from `CREATE PROCEDURE`.
#[derive(Debug, Clone)]
pub struct ProcedureSymbol {
    /// Procedure name (original casing).
    pub name: String,
    /// LSP range of the procedure name in source.
    pub range: Range,
    /// Parameters declared in the procedure signature.
    pub parameters: Vec<ParameterSymbol>,
    /// Variables declared inside the procedure body.
    pub body_variables: Vec<VariableSymbol>,
}

/// Parameter symbol in a procedure signature.
#[derive(Debug, Clone)]
pub struct ParameterSymbol {
    /// Parameter name (includes `@` prefix).
    pub name: String,
    /// LSP range of the parameter name in source.
    pub range: Range,
    /// SQL data type of the parameter.
    pub data_type: DataType,
    /// Whether the parameter is declared with OUTPUT.
    pub is_output: bool,
}

/// View symbol extracted from `CREATE VIEW`.
#[derive(Debug, Clone)]
pub struct ViewSymbol {
    /// View name (original casing).
    pub name: String,
    /// LSP range of the view name in source.
    pub range: Range,
}

/// Index symbol extracted from `CREATE [UNIQUE] INDEX`.
#[derive(Debug, Clone)]
pub struct IndexSymbol {
    /// Index name (original casing).
    pub name: String,
    /// LSP range of the index name in source.
    pub range: Range,
    /// Target table name.
    pub table_name: String,
    /// Column names covered by the index.
    pub columns: Vec<String>,
    /// Whether the index is unique.
    pub is_unique: bool,
}

/// Variable symbol extracted from `DECLARE`.
#[derive(Debug, Clone)]
pub struct VariableSymbol {
    /// Variable name (includes `@` prefix).
    pub name: String,
    /// LSP range of the variable name in source.
    pub range: Range,
    /// SQL data type of the variable.
    pub data_type: DataType,
}

/// トリガーシンボル
#[derive(Debug, Clone)]
pub struct TriggerSymbol {
    /// トリガー名
    pub name: String,
    /// 定義位置
    pub range: Range,
    /// 対象テーブル名
    pub table_name: String,
    /// トリガーイベント（INSERT, UPDATE, DELETE）
    pub events: Vec<String>,
}

/// シンボルテーブルビルダー
pub struct SymbolTableBuilder;

impl SymbolTableBuilder {
    /// ソースコードからシンボルテーブルを構築する
    pub fn build(source: &str) -> SymbolTable {
        let mut table = SymbolTable::default();
        let line_index = LineIndex::new(source);

        let statements = match tsql_parser::Parser::new(source).parse() {
            Ok(s) => s,
            Err(_) => return table,
        };

        for stmt in &statements {
            Self::collect_from_stmt(&line_index, stmt, &mut table);
        }

        table
    }

    /// ソースコードからシンボルテーブルを構築（エラー許容）
    pub fn build_tolerant(source: &str) -> SymbolTable {
        let mut table = SymbolTable::default();
        let line_index = LineIndex::new(source);

        let statements = match tsql_parser::Parser::new(source).parse_with_errors() {
            Ok((s, _)) => s,
            Err(_) => return table,
        };

        for stmt in &statements {
            Self::collect_from_stmt(&line_index, stmt, &mut table);
        }

        table
    }

    fn collect_from_stmt(line_index: &LineIndex, stmt: &Statement, table: &mut SymbolTable) {
        match stmt {
            Statement::Create(create) => match create.as_ref() {
                CreateStatement::Table(td) => {
                    let name_upper = CaseInsensitiveKey::new(&td.name.name);
                    let columns: Vec<ColumnSymbol> = td
                        .columns
                        .iter()
                        .map(|col| ColumnSymbol {
                            name: col.name.name.clone(),
                            range: span_to_range(line_index, col.name.span),
                            data_type: col.data_type.clone(),
                            nullable: col.nullability,
                            is_identity: col.identity,
                            table_name: td.name.name.clone(),
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
                            range: span_to_range(line_index, td.name.span),
                            columns,
                            constraints,
                            is_temporary: td.temporary,
                        },
                    );
                }
                CreateStatement::Procedure(pd) => {
                    let name_upper = CaseInsensitiveKey::new(&pd.name.name);
                    let parameters: Vec<ParameterSymbol> = pd
                        .parameters
                        .iter()
                        .map(|p| ParameterSymbol {
                            name: p.name.name.clone(),
                            range: span_to_range(line_index, p.name.span),
                            data_type: p.data_type.clone(),
                            is_output: p.is_output,
                        })
                        .collect();

                    // プロシージャボディ内の変数を収集
                    let mut body_variables = Vec::new();
                    for body_stmt in &pd.body {
                        Self::collect_variables(line_index, body_stmt, &mut body_variables);
                    }

                    table.procedures.insert(
                        name_upper,
                        ProcedureSymbol {
                            name: pd.name.name.clone(),
                            range: span_to_range(line_index, pd.name.span),
                            parameters,
                            body_variables,
                        },
                    );
                }
                CreateStatement::View(vd) => {
                    let name_upper = CaseInsensitiveKey::new(&vd.name.name);
                    table.views.insert(
                        name_upper,
                        ViewSymbol {
                            name: vd.name.name.clone(),
                            range: span_to_range(line_index, vd.name.span),
                        },
                    );
                }
                CreateStatement::Index(idx) => {
                    let name_upper = CaseInsensitiveKey::new(&idx.name.name);
                    table.indexes.insert(
                        name_upper,
                        IndexSymbol {
                            name: idx.name.name.clone(),
                            range: span_to_range(line_index, idx.name.span),
                            table_name: idx.table.name.clone(),
                            columns: idx.columns.iter().map(|c| c.name.clone()).collect(),
                            is_unique: idx.unique,
                        },
                    );
                }
                CreateStatement::Trigger(trigger) => {
                    let range = span_to_range(line_index, trigger.span);
                    let events = trigger.events.iter().map(|e| format!("{e:?}")).collect();
                    table.triggers.insert(
                        CaseInsensitiveKey::new(&trigger.name.name),
                        TriggerSymbol {
                            name: trigger.name.name.clone(),
                            range,
                            table_name: trigger.table.name.clone(),
                            events,
                        },
                    );
                }
            },
            Statement::Declare(decl) => {
                Self::collect_declare_variables(line_index, decl, &mut table.variables);
            }
            Statement::Block(block) => {
                for s in &block.statements {
                    Self::collect_from_stmt(line_index, s, table);
                }
            }
            Statement::If(if_stmt) => {
                Self::collect_from_stmt(line_index, &if_stmt.then_branch, table);
                if let Some(else_branch) = &if_stmt.else_branch {
                    Self::collect_from_stmt(line_index, else_branch, table);
                }
            }
            Statement::While(while_stmt) => {
                Self::collect_from_stmt(line_index, &while_stmt.body, table);
            }
            Statement::TryCatch(tc) => {
                for s in &tc.try_block.statements {
                    Self::collect_from_stmt(line_index, s, table);
                }
                for s in &tc.catch_block.statements {
                    Self::collect_from_stmt(line_index, s, table);
                }
            }
            // Flat statements — no nested definitions to extract
            Statement::Select(_)
            | Statement::Insert(_)
            | Statement::Update(_)
            | Statement::Delete(_)
            | Statement::Set(_)
            | Statement::VariableAssignment(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Return(_)
            | Statement::Transaction(_)
            | Statement::Throw(_)
            | Statement::Raiserror(_)
            | Statement::AlterTable(_)
            | Statement::Exec(_)
            | Statement::BatchSeparator(_) => {}
        }
    }

    /// DECLARE文から変数を収集する
    fn collect_declare_variables(
        line_index: &LineIndex,
        decl: &DeclareStatement,
        variables: &mut HashMap<CaseInsensitiveKey, VariableSymbol>,
    ) {
        for var in &decl.variables {
            let name_upper = CaseInsensitiveKey::new(&var.name.name);
            variables.insert(
                name_upper,
                VariableSymbol {
                    name: var.name.name.clone(),
                    range: span_to_range(line_index, var.name.span),
                    data_type: var.data_type.clone(),
                },
            );
        }
    }

    /// ステートメント内の変数を再帰的に収集する
    fn collect_variables(
        line_index: &LineIndex,
        stmt: &Statement,
        variables: &mut Vec<VariableSymbol>,
    ) {
        match stmt {
            Statement::Declare(decl) => {
                for var in &decl.variables {
                    variables.push(VariableSymbol {
                        name: var.name.name.clone(),
                        range: span_to_range(line_index, var.name.span),
                        data_type: var.data_type.clone(),
                    });
                }
            }
            Statement::Block(block) => {
                for s in &block.statements {
                    Self::collect_variables(line_index, s, variables);
                }
            }
            Statement::If(if_stmt) => {
                Self::collect_variables(line_index, &if_stmt.then_branch, variables);
                if let Some(else_branch) = &if_stmt.else_branch {
                    Self::collect_variables(line_index, else_branch, variables);
                }
            }
            Statement::While(while_stmt) => {
                Self::collect_variables(line_index, &while_stmt.body, variables);
            }
            Statement::TryCatch(tc) => {
                for s in &tc.try_block.statements {
                    Self::collect_variables(line_index, s, variables);
                }
                for s in &tc.catch_block.statements {
                    Self::collect_variables(line_index, s, variables);
                }
            }
            // Flat statements — no nested variable declarations
            Statement::Select(_)
            | Statement::Insert(_)
            | Statement::Update(_)
            | Statement::Delete(_)
            | Statement::Create(_)
            | Statement::AlterTable(_)
            | Statement::Set(_)
            | Statement::VariableAssignment(_)
            | Statement::Break(_)
            | Statement::Continue(_)
            | Statement::Return(_)
            | Statement::Transaction(_)
            | Statement::Throw(_)
            | Statement::Raiserror(_)
            | Statement::Exec(_)
            | Statement::BatchSeparator(_) => {}
        }
    }

    /// テーブル名でテーブルを検索 (case-insensitive)
    #[must_use]
    pub fn find_table<'a>(table: &'a SymbolTable, name: &str) -> Option<&'a TableSymbol> {
        let key = CaseInsensitiveKey::new(name);
        table.tables.get::<str>(key.borrow())
    }

    /// プロシージャ名でプロシージャを検索 (case-insensitive)
    #[must_use]
    pub fn find_procedure<'a>(table: &'a SymbolTable, name: &str) -> Option<&'a ProcedureSymbol> {
        let key = CaseInsensitiveKey::new(name);
        table.procedures.get::<str>(key.borrow())
    }

    /// ビュー名でビューを検索 (case-insensitive)
    #[must_use]
    pub fn find_view<'a>(table: &'a SymbolTable, name: &str) -> Option<&'a ViewSymbol> {
        let key = CaseInsensitiveKey::new(name);
        table.views.get::<str>(key.borrow())
    }

    /// インデックス名でインデックスを検索 (case-insensitive)
    #[must_use]
    pub fn find_index<'a>(table: &'a SymbolTable, name: &str) -> Option<&'a IndexSymbol> {
        let key = CaseInsensitiveKey::new(name);
        table.indexes.get::<str>(key.borrow())
    }

    /// トリガー名でトリガーを検索 (case-insensitive)
    #[must_use]
    pub fn find_trigger<'a>(table: &'a SymbolTable, name: &str) -> Option<&'a TriggerSymbol> {
        let key = CaseInsensitiveKey::new(name);
        table.triggers.get::<str>(key.borrow())
    }

    /// 変数名で変数を検索 (case-insensitive, @prefix auto-added)
    #[must_use]
    pub fn find_variable<'a>(table: &'a SymbolTable, name: &str) -> Option<&'a VariableSymbol> {
        let search_name = if name.starts_with('@') {
            CaseInsensitiveKey::new(name)
        } else {
            CaseInsensitiveKey::new(&format!("@{}", name))
        };
        table.variables.get::<str>(search_name.borrow())
    }

    /// カーソル位置の識別子を特定
    pub fn find_identifier_at(
        source: &str,
        position: Position,
    ) -> Option<(String, lsp_types::Range)> {
        let line_index = LineIndex::new(source);
        let offset = line_index.position_to_offset(source, position);

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
                    return Some((
                        token.text.to_string(),
                        span_to_range(&line_index, token.span),
                    ));
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
fn span_to_range(line_index: &LineIndex, span: Span) -> Range {
    line_index.offset_to_range(span.start, span.end)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_case_insensitive_key_equality() {
        let a = CaseInsensitiveKey::new("Users");
        let b = CaseInsensitiveKey::new("users");
        let c = CaseInsensitiveKey::new("USERS");
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn test_case_insensitive_key_hash_match() {
        let mut map: HashMap<CaseInsensitiveKey, i32> = HashMap::new();
        map.insert(CaseInsensitiveKey::new("Users"), 1);
        // Lookup works with uppercase str (same as stored form)
        assert_eq!(map.get("USERS"), Some(&1));
        assert_eq!(map.get("unknown"), None);
        // Lookup with pre-computed uppercase String via Borrow<String>
        let upper = "USERS".to_string();
        assert_eq!(map.get(&upper), Some(&1));
    }

    #[test]
    fn test_case_insensitive_key_borrow_string() {
        let mut map: HashMap<CaseInsensitiveKey, i32> = HashMap::new();
        map.insert(CaseInsensitiveKey::new("Tbl"), 42);
        let upper = "TBL".to_string();
        assert_eq!(map.get(&upper), Some(&42));
    }

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

    // --- Explicit variant coverage tests (#56) ---

    #[test]
    fn test_variable_after_set_statement() {
        // SET @var = expr (VariableAssignment) is parsed separately from SET option
        let source = "DECLARE @x INT\nSET @x = 1\nDECLARE @y INT";
        let table = SymbolTableBuilder::build_tolerant(source);
        assert!(table.variables.contains_key("@X"), "@x should be tracked");
        assert!(
            table.variables.contains_key("@Y"),
            "@y after SET should still be tracked"
        );
    }

    #[test]
    fn test_variable_after_transaction() {
        let source = "BEGIN TRAN\nDECLARE @x INT\nCOMMIT";
        let table = SymbolTableBuilder::build_tolerant(source);
        assert!(
            table.variables.contains_key("@X"),
            "Variable inside transaction context should be tracked"
        );
    }

    #[test]
    fn test_variable_after_select_statement() {
        let source = "SELECT * FROM t\nDECLARE @result INT";
        let table = SymbolTableBuilder::build_tolerant(source);
        assert!(
            table.variables.contains_key("@RESULT"),
            "Variable after SELECT should be tracked"
        );
    }

    #[test]
    fn test_table_and_procedure_in_multi_batch() {
        // GO separates batches — both should still be found
        let source = "CREATE TABLE t (id INT)\nGO\nCREATE PROCEDURE p AS BEGIN RETURN 1 END";
        let table = SymbolTableBuilder::build_tolerant(source);
        assert!(
            table.tables.contains_key("T"),
            "Table before GO should be tracked"
        );
        assert!(
            table.procedures.contains_key("P"),
            "Procedure after GO should be tracked"
        );
    }

    #[test]
    fn test_empty_source_returns_empty_table() {
        let table = SymbolTableBuilder::build_tolerant("");
        assert!(table.tables.is_empty());
        assert!(table.procedures.is_empty());
        assert!(table.variables.is_empty());
        assert!(table.views.is_empty());
        assert!(table.indexes.is_empty());
    }

    #[test]
    fn test_invalid_sql_returns_empty_table() {
        let table = SymbolTableBuilder::build_tolerant("NOT VALID SQL AT ALL");
        assert!(
            table.tables.is_empty(),
            "Invalid SQL should produce empty symbol table"
        );
    }

    #[test]
    fn test_variable_in_if_block() {
        let source = "IF 1 = 1\nBEGIN\n    DECLARE @x INT\n    SET @x = 1\nEND";
        let table = SymbolTableBuilder::build_tolerant(source);
        assert!(
            table.variables.contains_key("@X"),
            "Variable inside IF/BEGIN block should be tracked"
        );
    }

    #[test]
    fn test_case_insensitive_table_lookup() {
        let source = "CREATE TABLE MyTable (id INT)";
        let table = SymbolTableBuilder::build_tolerant(source);
        // CaseInsensitiveKey stores uppercase; contains_key borrows &str
        // and the HashMap uses Eq on the borrowed str
        assert!(
            table.tables.contains_key("MYTABLE"),
            "Should find table via uppercase key"
        );
    }

    #[test]
    fn test_index_extraction() {
        let source = "CREATE INDEX idx_name ON users (id)";
        let table = SymbolTableBuilder::build_tolerant(source);
        assert!(
            table.indexes.contains_key("IDX_NAME"),
            "Index should be tracked: {:?}",
            table.indexes.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_trigger_extraction() {
        let source = "CREATE TRIGGER tr_test ON users FOR INSERT AS BEGIN SELECT 1 END";
        let table = SymbolTableBuilder::build_tolerant(source);
        assert!(
            table.triggers.contains_key("TR_TEST"),
            "Trigger should be tracked: {:?}",
            table.triggers.keys().collect::<Vec<_>>()
        );
        let trigger = table.triggers.get("TR_TEST").expect("trigger exists");
        assert_eq!(trigger.name, "tr_test");
        assert_eq!(trigger.table_name, "users");
        assert!(trigger.events.contains(&"Insert".to_string()));
    }

    #[test]
    fn test_trigger_multiple_events() {
        let source =
            "CREATE TRIGGER tr_multi ON users FOR INSERT, UPDATE, DELETE AS BEGIN SELECT 1 END";
        let table = SymbolTableBuilder::build_tolerant(source);
        let trigger = table.triggers.get("TR_MULTI").expect("trigger exists");
        assert_eq!(trigger.events.len(), 3);
    }

    #[test]
    fn test_trigger_definition_found() {
        let source = "CREATE TRIGGER tr_test ON users FOR INSERT AS BEGIN SELECT 1 END";
        // Verify trigger is tracked in symbol table
        let table = SymbolTableBuilder::build(source);
        assert!(
            table.triggers.contains_key("TR_TEST"),
            "Trigger should be tracked before testing definition"
        );
        let results = crate::definition::definition_ranges_with_analysis(
            &crate::analysis::DocumentAnalysis::new(source),
            Position {
                line: 0,
                character: 18,
            },
        );
        assert!(
            !results.is_empty(),
            "Go to Definition should find trigger definition"
        );
    }
}
