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
}

#[cfg(feature = "wasm")]
impl TryFrom<Statement> for JsStatement {
    type Error = String;

    fn try_from(stmt: Statement) -> Result<Self, String> {
        match stmt {
            Statement::Select(_) => Ok(Self::Select {
                columns: vec!["*".to_string()], // Simplified
                from: None,
            }),
            Statement::Insert(_) => Ok(Self::Insert { table: None }),
            Statement::Update(_) => Ok(Self::Update { table: None }),
            Statement::Delete(_) => Ok(Self::Delete { table: None }),
            Statement::Create(s) => match *s {
                tsql_parser::CreateStatement::Table(_) => Ok(Self::Create {
                    object_type: Some("TABLE".to_string()),
                    name: None,
                }),
                tsql_parser::CreateStatement::Index(_) => Ok(Self::Create {
                    object_type: Some("INDEX".to_string()),
                    name: None,
                }),
                tsql_parser::CreateStatement::View(_) => Ok(Self::Create {
                    object_type: Some("VIEW".to_string()),
                    name: None,
                }),
                tsql_parser::CreateStatement::Procedure(_) => Ok(Self::Create {
                    object_type: Some("PROCEDURE".to_string()),
                    name: None,
                }),
            },
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
        }
    }
}
