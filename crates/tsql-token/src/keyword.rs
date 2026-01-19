//! 静的キーワードマップ
//!
//! 大文字小文字を区別せずにキーワードを解決するための静的な HashMap を提供する。

use once_cell::sync::Lazy;
use std::collections::HashMap;

use crate::TokenKind;

/// 静的キーワードマップ（プログラム起動時に1回のみ初期化）
///
/// SAP ASE T-SQL のすべての予約語を小文字キーでマッピングする。
static KEYWORDS: Lazy<HashMap<&'static str, TokenKind>> = Lazy::new(|| {
    let mut m = HashMap::with_capacity(150);

    // DML Keywords
    m.insert("select", TokenKind::Select);
    m.insert("insert", TokenKind::Insert);
    m.insert("update", TokenKind::Update);
    m.insert("delete", TokenKind::Delete);
    m.insert("merge", TokenKind::Merge);
    m.insert("from", TokenKind::From);
    m.insert("where", TokenKind::Where);
    m.insert("join", TokenKind::Join);
    m.insert("inner", TokenKind::Inner);
    m.insert("outer", TokenKind::Outer);
    m.insert("left", TokenKind::Left);
    m.insert("right", TokenKind::Right);
    m.insert("full", TokenKind::Full);
    m.insert("cross", TokenKind::Cross);
    m.insert("on", TokenKind::On);
    m.insert("and", TokenKind::And);
    m.insert("or", TokenKind::Or);
    m.insert("not", TokenKind::Not);
    m.insert("in", TokenKind::In);
    m.insert("exists", TokenKind::Exists);
    m.insert("between", TokenKind::Between);
    m.insert("like", TokenKind::Like);
    m.insert("is", TokenKind::Is);
    m.insert("null", TokenKind::Null);
    m.insert("order", TokenKind::Order);
    m.insert("by", TokenKind::By);
    m.insert("asc", TokenKind::Asc);
    m.insert("desc", TokenKind::Desc);
    m.insert("group", TokenKind::Group);
    m.insert("having", TokenKind::Having);
    m.insert("union", TokenKind::Union);
    m.insert("intersect", TokenKind::Intersect);
    m.insert("except", TokenKind::Except);
    m.insert("distinct", TokenKind::Distinct);
    m.insert("all", TokenKind::All);
    m.insert("top", TokenKind::Top);
    m.insert("limit", TokenKind::Limit);
    m.insert("offset", TokenKind::Offset);
    m.insert("first", TokenKind::First);
    m.insert("next", TokenKind::Next);
    m.insert("rows", TokenKind::Rows);
    m.insert("only", TokenKind::Only);

    // DDL Keywords
    m.insert("create", TokenKind::Create);
    m.insert("alter", TokenKind::Alter);
    m.insert("drop", TokenKind::Drop);
    m.insert("truncate", TokenKind::Truncate);
    m.insert("table", TokenKind::Table);
    m.insert("index", TokenKind::Index);
    m.insert("view", TokenKind::View);
    m.insert("procedure", TokenKind::Procedure);
    m.insert("proc", TokenKind::Proc);
    m.insert("function", TokenKind::Function);
    m.insert("trigger", TokenKind::Trigger);
    m.insert("database", TokenKind::Database);
    m.insert("schema", TokenKind::Schema);
    m.insert("constraint", TokenKind::Constraint);
    m.insert("primary", TokenKind::Primary);
    m.insert("foreign", TokenKind::Foreign);
    m.insert("key", TokenKind::Key);
    m.insert("references", TokenKind::References);
    m.insert("unique", TokenKind::Unique);
    m.insert("check", TokenKind::Check);
    m.insert("default", TokenKind::Default);
    m.insert("identity", TokenKind::Identity);
    m.insert("autoincrement", TokenKind::Autoincrement);

    // Control Flow Keywords
    m.insert("if", TokenKind::If);
    m.insert("else", TokenKind::Else);
    m.insert("begin", TokenKind::Begin);
    m.insert("end", TokenKind::End);
    m.insert("while", TokenKind::While);
    m.insert("return", TokenKind::Return);
    m.insert("break", TokenKind::Break);
    m.insert("continue", TokenKind::Continue);
    m.insert("case", TokenKind::Case);
    m.insert("when", TokenKind::When);
    m.insert("then", TokenKind::Then);
    m.insert("try", TokenKind::Try);
    m.insert("catch", TokenKind::Catch);
    m.insert("throw", TokenKind::Throw);
    m.insert("raiserror", TokenKind::Raiserror);

    // Transaction Keywords
    m.insert("commit", TokenKind::Commit);
    m.insert("rollback", TokenKind::Rollback);
    m.insert("transaction", TokenKind::Transaction);
    m.insert("tran", TokenKind::Tran);
    m.insert("save", TokenKind::Save);
    m.insert("savepoint", TokenKind::Savepoint);

    // Type Keywords
    m.insert("int", TokenKind::Int);
    m.insert("integer", TokenKind::Integer);
    m.insert("smallint", TokenKind::Smallint);
    m.insert("tinyint", TokenKind::Tinyint);
    m.insert("bigint", TokenKind::Bigint);
    m.insert("real", TokenKind::Real);
    m.insert("double", TokenKind::Double);
    m.insert("decimal", TokenKind::Decimal);
    m.insert("numeric", TokenKind::Numeric);
    m.insert("money", TokenKind::Money);
    m.insert("smallmoney", TokenKind::Smallmoney);
    m.insert("char", TokenKind::Char);
    m.insert("varchar", TokenKind::Varchar);
    m.insert("text", TokenKind::Text);
    m.insert("nchar", TokenKind::Nchar);
    m.insert("nvarchar", TokenKind::Nvarchar);
    m.insert("ntext", TokenKind::Ntext);
    m.insert("unichar", TokenKind::Unichar);
    m.insert("univarchar", TokenKind::Univarchar);
    m.insert("unitext", TokenKind::Unitext);
    m.insert("binary", TokenKind::Binary);
    m.insert("varbinary", TokenKind::Varbinary);
    m.insert("image", TokenKind::Image);
    m.insert("date", TokenKind::Date);
    m.insert("time", TokenKind::Time);
    m.insert("datetime", TokenKind::Datetime);
    m.insert("smalldatetime", TokenKind::Smalldatetime);
    m.insert("timestamp", TokenKind::Timestamp);
    m.insert("bigdatetime", TokenKind::Bigdatetime);
    m.insert("bit", TokenKind::Bit);
    m.insert("uniqueidentifier", TokenKind::Uniqueidentifier);

    // Misc Keywords
    m.insert("as", TokenKind::As);
    m.insert("set", TokenKind::Set);
    m.insert("declare", TokenKind::Declare);
    m.insert("exec", TokenKind::Exec);
    m.insert("execute", TokenKind::Execute);
    m.insert("into", TokenKind::Into);
    m.insert("values", TokenKind::Values);
    m.insert("output", TokenKind::Output);
    m.insert("cursor", TokenKind::Cursor);
    m.insert("open", TokenKind::Open);
    m.insert("close", TokenKind::Close);
    m.insert("deallocate", TokenKind::Deallocate);
    m.insert("grant", TokenKind::Grant);
    m.insert("revoke", TokenKind::Revoke);
    m.insert("deny", TokenKind::Deny);
    m.insert("print", TokenKind::Print);
    m.insert("waitfor", TokenKind::Waitfor);
    m.insert("goto", TokenKind::Goto);
    m.insert("label", TokenKind::Label);

    m
});

impl TokenKind {
    /// 識別子からキーワードを解決（大文字小文字非区別）
    ///
    /// # Arguments
    ///
    /// * `s` - 解決する識別子文字列
    ///
    /// # Returns
    ///
    /// 対応するキーワードの `TokenKind`、または `TokenKind::Ident`
    #[must_use]
    pub fn from_ident(s: &str) -> Self {
        // 小文字に変換して検索（大文字小文字非区別）
        let lower = s.to_ascii_lowercase();
        KEYWORDS.get(lower.as_str()).copied().unwrap_or(TokenKind::Ident)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_lookup_lowercase() {
        assert_eq!(TokenKind::from_ident("select"), TokenKind::Select);
        assert_eq!(TokenKind::from_ident("from"), TokenKind::From);
        assert_eq!(TokenKind::from_ident("where"), TokenKind::Where);
        assert_eq!(TokenKind::from_ident("create"), TokenKind::Create);
        assert_eq!(TokenKind::from_ident("table"), TokenKind::Table);
    }

    #[test]
    fn test_keyword_lookup_uppercase() {
        assert_eq!(TokenKind::from_ident("SELECT"), TokenKind::Select);
        assert_eq!(TokenKind::from_ident("FROM"), TokenKind::From);
        assert_eq!(TokenKind::from_ident("WHERE"), TokenKind::Where);
        assert_eq!(TokenKind::from_ident("CREATE"), TokenKind::Create);
        assert_eq!(TokenKind::from_ident("TABLE"), TokenKind::Table);
    }

    #[test]
    fn test_keyword_lookup_mixed_case() {
        assert_eq!(TokenKind::from_ident("Select"), TokenKind::Select);
        assert_eq!(TokenKind::from_ident("FrOm"), TokenKind::From);
        assert_eq!(TokenKind::from_ident("WhErE"), TokenKind::Where);
        assert_eq!(TokenKind::from_ident("CrEaTe"), TokenKind::Create);
        assert_eq!(TokenKind::from_ident("TaBlE"), TokenKind::Table);
    }

    #[test]
    fn test_ident_returns_ident() {
        assert_eq!(TokenKind::from_ident("mytable"), TokenKind::Ident);
        assert_eq!(TokenKind::from_ident("foo"), TokenKind::Ident);
        assert_eq!(TokenKind::from_ident("bar123"), TokenKind::Ident);
    }

    #[test]
    fn test_all_dml_keywords() {
        assert_eq!(TokenKind::from_ident("insert"), TokenKind::Insert);
        assert_eq!(TokenKind::from_ident("update"), TokenKind::Update);
        assert_eq!(TokenKind::from_ident("delete"), TokenKind::Delete);
        assert_eq!(TokenKind::from_ident("merge"), TokenKind::Merge);
        assert_eq!(TokenKind::from_ident("join"), TokenKind::Join);
    }

    #[test]
    fn test_all_control_flow_keywords() {
        assert_eq!(TokenKind::from_ident("if"), TokenKind::If);
        assert_eq!(TokenKind::from_ident("else"), TokenKind::Else);
        assert_eq!(TokenKind::from_ident("begin"), TokenKind::Begin);
        assert_eq!(TokenKind::from_ident("end"), TokenKind::End);
        assert_eq!(TokenKind::from_ident("while"), TokenKind::While);
        assert_eq!(TokenKind::from_ident("case"), TokenKind::Case);
    }

    #[test]
    fn test_all_type_keywords() {
        assert_eq!(TokenKind::from_ident("int"), TokenKind::Int);
        assert_eq!(TokenKind::from_ident("varchar"), TokenKind::Varchar);
        assert_eq!(TokenKind::from_ident("datetime"), TokenKind::Datetime);
        assert_eq!(TokenKind::from_ident("bit"), TokenKind::Bit);
    }
}
