//! JavaScript-friendly AST representations

use tsql_parser::{ParseError, Statement};

/// Conversion result (T-SQL to target dialect)
#[cfg(feature = "wasm")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "status")]
pub enum JsConversionResult {
    /// Successful conversion
    Success {
        /// The converted SQL string
        sql: String,
    },
    /// Conversion error
    Error {
        /// Error message describing what went wrong
        message: String,
    },
}

/// Parse result (success or error)
#[cfg(feature = "wasm")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum JsParseResult {
    /// Successful parse (multiple statements)
    Success(Vec<JsStatement>),
    /// Successful parse (single statement)
    SuccessSingle(JsStatement),
    /// Parse error
    Error(JsParseError),
}

/// JavaScript-friendly parse error
#[cfg(feature = "wasm")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsParseError {
    /// Error message
    pub message: String,
    /// Line number (0 if unknown)
    pub line: u32,
    /// Column number (0 if unknown)
    pub column: u32,
    /// Byte offset
    pub offset: u32,
}

#[cfg(feature = "wasm")]
impl From<ParseError> for JsParseError {
    fn from(err: ParseError) -> Self {
        let pos = err.position();
        Self {
            message: err.to_string(),
            line: pos.line,
            column: pos.column,
            offset: pos.offset,
        }
    }
}

/// JavaScript-friendly statement representation
#[cfg(feature = "wasm")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "statementType")]
pub enum JsStatement {
    /// SELECT statement
    Select {
        /// Column names (simplified representation)
        columns: Vec<String>,
        /// From table name (simplified representation)
        from: Option<String>,
    },
    /// INSERT statement
    Insert {
        /// Target table name (simplified representation)
        table: Option<String>,
    },
    /// UPDATE statement
    Update {
        /// Target table name (simplified representation)
        table: Option<String>,
    },
    /// DELETE statement
    Delete {
        /// Target table name (simplified representation)
        table: Option<String>,
    },
    /// CREATE statement
    Create {
        /// Object type (TABLE, INDEX, VIEW, PROCEDURE, etc.)
        object_type: Option<String>,
        /// Object name (simplified representation)
        name: Option<String>,
    },
    /// BEGIN/END block
    Block,
    /// IF statement
    IfStatement,
    /// WHILE statement
    WhileStatement,
    /// BREAK statement
    Break,
    /// CONTINUE statement
    Continue,
    /// DECLARE statement
    Declare,
    /// SET statement
    Set,
    /// Variable assignment (SELECT @var = expr)
    VariableAssignment,
    /// RETURN statement
    Return,
    /// Batch separator (GO)
    BatchSeparator,
    /// TRY...CATCH statement
    TryCatch,
    /// Transaction control statement
    Transaction,
    /// THROW statement
    Throw,
    /// RAISERROR statement
    Raiserror,
    /// EXEC/EXECUTE statement
    Exec,
    /// ALTER TABLE statement
    AlterTable,
    /// CREATE TRIGGER statement
    Trigger,
}

#[cfg(feature = "wasm")]
impl TryFrom<Statement> for JsStatement {
    type Error = String;

    /// Issue #61: 旧実装はパース済み AST を無視してハードコード stub を返していた。
    /// 本 impl は純粋な delegate であり、実ロジックは
    /// [`crate::ast_convert`] (non-wasm, nextest で検証済み) に委譲する。
    fn try_from(stmt: Statement) -> Result<Self, String> {
        // 先に CREATE TRIGGER を検出して JsStatement::Trigger に redirect する。
        // ast_convert::create_to_js は Trigger を (None, None) として返し、本 impl は
        // Trigger 専用 variant を使用するため、ここで分岐する。
        if matches!(
            stmt,
            Statement::Create(ref c) if matches!(**c, tsql_parser::CreateStatement::Trigger(_))
        ) {
            return Ok(Self::Trigger);
        }

        match stmt {
            Statement::Select(s) => {
                let (columns, from) = crate::ast_convert::select_to_js(&s);
                Ok(Self::Select { columns, from })
            }
            Statement::Insert(s) => Ok(Self::Insert {
                // InsertStatement.table は Identifier。name を直接取り出す。
                table: Some(s.table.name.clone()),
            }),
            Statement::Update(s) => Ok(Self::Update {
                // UpdateStatement.table は TableReference → table_ref_to_name で解決。
                table: crate::ast_convert::table_ref_to_name(&s.table),
            }),
            Statement::Delete(s) => Ok(Self::Delete {
                // DeleteStatement.table は Identifier。name を直接取り出す。
                table: Some(s.table.name.clone()),
            }),
            Statement::Create(s) => {
                let (object_type, name) = crate::ast_convert::create_to_js(&s);
                Ok(Self::Create { object_type, name })
            }
            Statement::Block(_) => Ok(Self::Block),
            Statement::If(_) => Ok(Self::IfStatement),
            Statement::While(_) => Ok(Self::WhileStatement),
            Statement::Break(_) => Ok(Self::Break),
            Statement::Continue(_) => Ok(Self::Continue),
            Statement::Declare(_) => Ok(Self::Declare),
            Statement::Set(_) => Ok(Self::Set),
            Statement::VariableAssignment(_) => Ok(Self::VariableAssignment),
            Statement::Return(_) => Ok(Self::Return),
            Statement::BatchSeparator(_) => Ok(Self::BatchSeparator),
            Statement::TryCatch(_) => Ok(Self::TryCatch),
            Statement::Transaction(_) => Ok(Self::Transaction),
            Statement::Throw(_) => Ok(Self::Throw),
            Statement::Raiserror(_) => Ok(Self::Raiserror),
            Statement::Exec(_) => Ok(Self::Exec),
            Statement::AlterTable(_) => Ok(Self::AlterTable),
        }
    }
}

#[cfg(test)]
#[cfg(feature = "wasm")]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod delegate_tests {
    use super::*;

    // Issue #61: end-to-end delegate 検証。ヘルパ単体テスト (ast_convert.rs) と
    // TryFrom の接続を検証し、旧ハードコード stub (columns=["*"], table=None,
    // name=None) が実際の AST データで置き換えられたことを保証する。

    /// SELECT a, b FROM t → columns=["a","b"], from=Some("t") (旧 stub は ["*"]/None)
    #[test]
    fn delegate_select_populates_real_columns_and_from() {
        let stmts = tsql_parser::parse("SELECT a, b FROM t").unwrap();
        let js = JsStatement::try_from(stmts.into_iter().next().unwrap()).unwrap();
        match js {
            JsStatement::Select { columns, from } => {
                assert_eq!(columns, vec!["a".to_string(), "b".to_string()]);
                assert_eq!(from.as_deref(), Some("t"));
            }
            other => panic!("expected Select, got {other:?}"),
        }
    }

    /// INSERT INTO t (a) VALUES (1) → table=Some("t") (旧 stub は None)
    #[test]
    fn delegate_insert_populates_real_table() {
        let stmts = tsql_parser::parse("INSERT INTO t (a) VALUES (1)").unwrap();
        let js = JsStatement::try_from(stmts.into_iter().next().unwrap()).unwrap();
        match js {
            JsStatement::Insert { table } => {
                assert_eq!(table.as_deref(), Some("t"));
            }
            other => panic!("expected Insert, got {other:?}"),
        }
    }

    /// UPDATE t SET a = 1 → table=Some("t") (旧 stub は None)
    #[test]
    fn delegate_update_populates_real_table() {
        let stmts = tsql_parser::parse("UPDATE t SET a = 1").unwrap();
        let js = JsStatement::try_from(stmts.into_iter().next().unwrap()).unwrap();
        match js {
            JsStatement::Update { table } => {
                assert_eq!(table.as_deref(), Some("t"));
            }
            other => panic!("expected Update, got {other:?}"),
        }
    }

    /// DELETE FROM t WHERE a = 1 → table=Some("t") (旧 stub は None)
    #[test]
    fn delegate_delete_populates_real_table() {
        let stmts = tsql_parser::parse("DELETE FROM t WHERE a = 1").unwrap();
        let js = JsStatement::try_from(stmts.into_iter().next().unwrap()).unwrap();
        match js {
            JsStatement::Delete { table } => {
                assert_eq!(table.as_deref(), Some("t"));
            }
            other => panic!("expected Delete, got {other:?}"),
        }
    }

    /// CREATE TABLE t (id INT) → object_type="TABLE", name=Some("t") (旧 stub は None)
    #[test]
    fn delegate_create_table_populates_object_and_name() {
        let stmts = tsql_parser::parse("CREATE TABLE t (id INT)").unwrap();
        let js = JsStatement::try_from(stmts.into_iter().next().unwrap()).unwrap();
        match js {
            JsStatement::Create { object_type, name } => {
                assert_eq!(object_type.as_deref(), Some("TABLE"));
                assert_eq!(name.as_deref(), Some("t"));
            }
            other => panic!("expected Create, got {other:?}"),
        }
    }

    /// CREATE INDEX idx ON t (c) → object_type="INDEX", name=Some("idx")
    #[test]
    fn delegate_create_index_populates_object_and_name() {
        let stmts = tsql_parser::parse("CREATE INDEX idx ON t (c)").unwrap();
        let js = JsStatement::try_from(stmts.into_iter().next().unwrap()).unwrap();
        match js {
            JsStatement::Create { object_type, name } => {
                assert_eq!(object_type.as_deref(), Some("INDEX"));
                assert_eq!(name.as_deref(), Some("idx"));
            }
            other => panic!("expected Create, got {other:?}"),
        }
    }

    /// CREATE TRIGGER → 専用 Trigger variant (Create ではない)
    #[test]
    fn delegate_create_trigger_routes_to_trigger_variant() {
        let stmts =
            tsql_parser::parse("CREATE TRIGGER tr ON t FOR INSERT AS BEGIN RETURN END").unwrap();
        let js = JsStatement::try_from(stmts.into_iter().next().unwrap()).unwrap();
        assert!(
            matches!(js, JsStatement::Trigger),
            "CREATE TRIGGER must map to Trigger variant, got {js:?}"
        );
    }

    /// SELECT a + b FROM t → BinaryOp は EXPR_PLACEHOLDER へフォールバック
    #[test]
    fn delegate_select_binary_op_falls_back_to_placeholder() {
        let stmts = tsql_parser::parse("SELECT a + b FROM t").unwrap();
        let js = JsStatement::try_from(stmts.into_iter().next().unwrap()).unwrap();
        match js {
            JsStatement::Select { columns, .. } => {
                assert!(
                    columns
                        .iter()
                        .any(|c| c == crate::ast_convert::EXPR_PLACEHOLDER),
                    "BinaryOp must fall back to placeholder, got {columns:?}"
                );
            }
            other => panic!("expected Select, got {other:?}"),
        }
    }
}
