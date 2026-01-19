//! トークン種別の定義
//!
//! SAP ASE T-SQL のすべてのトークン種別を列挙型として定義する。

/// トークン種別の列挙型
///
/// SAP ASE T-SQL のすべての予約語、識別子、演算子、リテラルを表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // ==================== Keywords ====================
    // DML
    Select,
    Insert,
    Update,
    Delete,
    Merge,
    From,
    Where,
    Join,
    Inner,
    Outer,
    Left,
    Right,
    Full,
    Cross,
    On,
    And,
    Or,
    Not,
    In,
    Exists,
    Between,
    Like,
    Is,
    Null,
    Order,
    By,
    Asc,
    Desc,
    Group,
    Having,
    Union,
    Intersect,
    Except,
    Distinct,
    All,
    Top,
    Limit,
    Offset,
    First,
    Next,
    Rows,
    Only,

    // DDL
    Create,
    Alter,
    Drop,
    Truncate,
    Table,
    Index,
    View,
    Procedure,
    Proc,
    Function,
    Trigger,
    Database,
    Schema,
    Constraint,
    Primary,
    Foreign,
    Key,
    References,
    Unique,
    Check,
    Default,
    Identity,
    Autoincrement,

    // Control Flow
    If,
    Else,
    Begin,
    End,
    While,
    Return,
    Break,
    Continue,
    Case,
    When,
    Then,
    Else_,
    End_,
    Try,
    Catch,
    Throw,
    Raiserror,

    // Transaction
    Commit,
    Rollback,
    Transaction,
    Tran,
    Save,
    Savepoint,

    // Types
    Int,
    Integer,
    Smallint,
    Tinyint,
    Bigint,
    Real,
    Double,
    Decimal,
    Numeric,
    Money,
    Smallmoney,
    Char,
    Varchar,
    Text,
    Nchar,
    Nvarchar,
    Ntext,
    Unichar,
    Univarchar,
    Unitext,
    Binary,
    Varbinary,
    Image,
    Date,
    Time,
    Datetime,
    Smalldatetime,
    Timestamp,
    Bigdatetime,
    Bit,
    Uniqueidentifier,

    // Misc Keywords
    As,
    Set,
    Declare,
    Exec,
    Execute,
    Into,
    Values,
    Output,
    Cursor,
    Open,
    Close,
    Deallocate,
    Grant,
    Revoke,
    Deny,
    Print,
    Waitfor,
    Goto,
    Label,

    // ==================== Literals ====================
    Ident,
    QuotedIdent,
    Number,
    FloatLiteral,
    String,
    NString,
    UnicodeString,
    HexString,

    // ==================== Operators ====================
    // Comparison
    Eq,
    Ne,
    NeAlt,
    Lt,
    Gt,
    Le,
    Ge,
    NotLt,
    NotGt,

    // Arithmetic
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    // Bitwise
    Ampersand,
    Pipe,
    Caret,
    Tilde,

    // Assignment
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,

    // String
    Concat,

    // ==================== Punctuation ====================
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Colon,
    Dot,
    DotDot,

    // Variable prefixes
    LocalVar,
    GlobalVar,
    TempTable,
    GlobalTempTable,

    // ==================== Special ====================
    Whitespace,
    Newline,
    LineComment,
    BlockComment,

    Eof,
    Unknown,
}

impl TokenKind {
    /// このトークン種別がキーワードかどうかを判定する
    #[must_use]
    pub const fn is_keyword(self) -> bool {
        matches!(
            self,
            Self::Select
                | Self::Insert
                | Self::Update
                | Self::Delete
                | Self::Merge
                | Self::From
                | Self::Where
                | Self::Join
                | Self::Inner
                | Self::Outer
                | Self::Left
                | Self::Right
                | Self::Full
                | Self::Cross
                | Self::On
                | Self::And
                | Self::Or
                | Self::Not
                | Self::In
                | Self::Exists
                | Self::Between
                | Self::Like
                | Self::Is
                | Self::Null
                | Self::Order
                | Self::By
                | Self::Asc
                | Self::Desc
                | Self::Group
                | Self::Having
                | Self::Union
                | Self::Intersect
                | Self::Except
                | Self::Distinct
                | Self::All
                | Self::Top
                | Self::Limit
                | Self::Offset
                | Self::First
                | Self::Next
                | Self::Rows
                | Self::Only
                | Self::Create
                | Self::Alter
                | Self::Drop
                | Self::Truncate
                | Self::Table
                | Self::Index
                | Self::View
                | Self::Procedure
                | Self::Proc
                | Self::Function
                | Self::Trigger
                | Self::Database
                | Self::Schema
                | Self::Constraint
                | Self::Primary
                | Self::Foreign
                | Self::Key
                | Self::References
                | Self::Unique
                | Self::Check
                | Self::Default
                | Self::Identity
                | Self::Autoincrement
                | Self::If
                | Self::Else
                | Self::Begin
                | Self::End
                | Self::While
                | Self::Return
                | Self::Break
                | Self::Continue
                | Self::Case
                | Self::When
                | Self::Then
                | Self::Else_
                | Self::End_
                | Self::Try
                | Self::Catch
                | Self::Throw
                | Self::Raiserror
                | Self::Commit
                | Self::Rollback
                | Self::Transaction
                | Self::Tran
                | Self::Save
                | Self::Savepoint
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_detection() {
        assert!(TokenKind::Select.is_keyword());
        assert!(TokenKind::From.is_keyword());
        assert!(TokenKind::Where.is_keyword());
        assert!(TokenKind::If.is_keyword());
        assert!(TokenKind::Create.is_keyword());
    }

    #[test]
    fn test_non_keyword_detection() {
        assert!(!TokenKind::Ident.is_keyword());
        assert!(!TokenKind::Number.is_keyword());
        assert!(!TokenKind::String.is_keyword());
        assert!(!TokenKind::Eq.is_keyword());
        assert!(!TokenKind::Plus.is_keyword());
    }
}
