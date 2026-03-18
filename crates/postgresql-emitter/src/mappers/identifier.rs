//! PostgreSQL 識別子クォーター
//!
//! 識別子を PostgreSQL のクォート規則に従って処理します。

/// PostgreSQL 識別子クォーター
#[derive(Debug, Clone, Copy)]
pub struct IdentifierQuoter;

impl IdentifierQuoter {
    /// PostgreSQL の予約語セット
    const RESERVED_WORDS: &'static [&'static str] = &[
        // SQL 予約語
        "ALL", "ANALYSE", "ANALYZE", "AND", "ANY", "ARRAY", "AS", "ASC", "ASYMMETRIC", "AUTHORIZATION",
        "BETWEEN", "BINARY", "BOTH", "CASE", "CAST", "CHECK", "COLLATE", "COLLATION", "COLUMN", "CONCURRENTLY",
        "CONSTRAINT", "CREATE", "CROSS", "CURRENT_CATALOG", "CURRENT_DATE", "CURRENT_ROLE", "CURRENT_SCHEMA",
        "CURRENT_TIME", "CURRENT_TIMESTAMP", "CURRENT_USER", "DEFAULT", "DEFERRABLE", "DESC", "DISTINCT",
        "DO", "ELSE", "END", "EXCEPT", "EXCLUDE", "EXISTS", "FALSE", "FETCH", "FILTER", "FIRST", "FOR",
        "FOREIGN", "FROM", "FULL", "GRANT", "GROUP", "HAVING", "IN", "INITIALLY", "INNER", "INSERT", "INTERSECT",
        "INTO", "JOIN", "LATERAL", "LEADING", "LEFT", "LIKE", "LIMIT", "LOCAL", "LOCALTIME", "LOCALTIMESTAMP",
        "NATURAL", "NOT", "NULL", "OFFSET", "ON", "ONLY", "OR", "ORDER", "OUTER", "OVERLAPS", "PLACING", "PRIMARY",
        "REFERENCES", "RETURNING", "RIGHT", "ROW", "SELECT", "SESSION_USER", "SIMILAR", "SOME", "SYMMETRIC",
        "TABLE", "TABLESAMPLE", "THEN", "TO", "TRAILING", "TRUE", "UNION", "UNIQUE", "USER", "USING", "VARIADIC",
        "VERBOSE", "WHEN", "WHERE", "WINDOW", "WITH",
        // PostgreSQL 固有
        "ACL", "ADMIN", "AGGREGATE", "ALSO", "ALTER", " ALWAYS", "ASSERTION", "ASSIGNMENT", "AT", "ATTRIBUTE",
        "BACKWARD", "BEFORE", "BEGIN", "BY", "CACHE", "CALLED", "CASCADE", "CASCADED", "CATALOG", "CHAIN",
        "CHARACTERISTICS", "COMMENT", "COMMENTS", "COMMIT", "COMMITTED", "CONFIGURATION", "CONNECTION",
        "CONSTRAINTS", "CONTENT", "CONTINUE", "CONVERSION", "COPY", "COST", "CSV", "CURSOR", "CYCLE",
        "DATA", "DATABASE", "DAY", "DEALLOCATE", "DECLARE", "DEFAULTS", "DEFERRED", "DEFINER", "DELETE",
        "DELIMITER", "DELIMITERS", "DICTIONARY", "DISABLE", "DISCARD", "DOCUMENT", "DOMAIN", "DOUBLE", "DROP",
        "EACH", "ENABLE", "ENCODING", "ENCRYPTED", "ENUM", "ESCAPE", "EVENT", "EXCLUDE", "EXCLUDING",
        "EXCLUSIVE", "EXECUTE", "EXPLAIN", "EXTENSION", "EXTERNAL", "FAMILY", "FIRST", "FOLLOWING", "FORCE",
        "FORWARD", "FUNCTION", "FUNCTIONS", "GLOBAL", "GRANTED", "GREATEST", "GROUPING", "HANDLER", "HEADER",
        "HOLD", "HOUR", "IDENTITY", "IF", "IMMEDIATE", "IMMUTABLE", "IMPLICIT", "IMPORT", "INCLUSIVE", "INCREMENT",
        "INDEX", "INDEXES", "INHERIT", "INHERITS", "INLINE", "INSENSITIVE", "INSTEAD", "INVOKER", "ISOLATION",
        "KEY", "LABEL", "LANGUAGE", "LARGE", "LAST", "LEAKPROOF", "LEAST", "LEVEL", "LISTEN", "LOAD", "LOCAL",
        "LOCATION", "LOCK", "LOCKED", "LOGGED", "MAPPING", "MATCH", "MATERIALIZED", "MAXVALUE", "MINUTE",
        "MINVALUE", "MODE", "MONTH", "MOVE", "NAME", "NAMES", "NATIONAL", "NATURAL", "NCHAR", "NEXT", "NO",
        "NONE", "NOSUPERUSER", "NOTHING", "NOTIFY", "NOWAIT", "NULLIF", "NULLS", "NUMERIC", "OBJECT", "OF",
        "OFF", "OIDS", "OPERATOR", "OPTION", "OPTIONS", "ORDINALITY", "OTHERS", "OUT", "OVER", "OWNER", "PARTIAL",
        "PARTITION", "PASSING", "PLANNER", "POLICY", "POSITION", "PRECEDING", "PRECISION", "PREPARE", "PREPARED",
        "PRESERVE", "PRIOR", "PRIVILEGES", "PROCEDURAL", "PROCEDURE", "PROGRAM", "QUOTE", "RANGE", "READ",
        "REASSIGN", "RECHECK", "RECURSIVE", "REF", "REFERENCES", "REFRESH", "REINDEX", "RELATIVE", "RELEASE",
        "RENAME", "REPEATABLE", "REPLACE", "REPLICA", "RESET", "RESTART", "RESTRICT", "RETURNS", "REVOKE",
        "ROLE", "ROLLBACK", "ROLLUP", "ROUTINE", "ROUTINES", "RULE", "SAVEPOINT", "SCHEMA", "SCHEMAS", "SCROLL",
        "SEARCH", "SECOND", "SECURITY", "SEQUENCE", "SEQUENCES", "SERIALIZABLE", "SERVER", "SESSION", "SET",
        "SETOF", "SHARE", "SHOW", "SKIP", "SNAPSHOT", "STABLE", "STANDALONE", "START", "STATEMENT", "STATISTICS",
        "STDIN", "STDOUT", "STORAGE", "STRICT", "STRIP", "SUBSCRIPTION", "SUPPORT", "SYSID", "SYSTEM", "TABLES",
        "TABLESPACE", "TEMP", "TEMPLATE", "TEMPORARY", "TRANSACTION", "TREAT", "TRIGGER", "TRUNCATE", "TRUSTED",
        "TYPE", "TYPES", "UNBOUNDED", "UNCOMMITTED", "UNENCRYPTED", "UNKNOWN", "UNLISTEN", "UNLOGGED", "UNTIL",
        "UPDATE", "VACUUM", "VALID", "VALIDATE", "VALIDATOR", "VALUE", "VARYING", "VERSION", "VIEW", "VIEWS",
        "VOLATILE", "WHITESPACE", "WITHIN", "WITHOUT", "WORK", "WRAPPER", "WRITE", "XMLATTRIBUTES", "XMLCONCAT",
        "XMLELEMENT", "XMLEXISTS", "XMLFOREST", "XMLNAMESPACES", "XMLPARSE", "XMLPI", "XMLROOT", "XMLSERIALIZE",
        "XMLTABLE", "ZONE",
    ];

    /// 識別子がクォートが必要かどうかを判定
    ///
    /// # Arguments
    ///
    /// * `identifier` - 識別子文字列
    ///
    /// # Returns
    ///
    /// クォートが必要な場合は true
    ///
    /// # Note
    ///
    /// 以下の場合にクォートが必要:
    /// - 予約語と一致する
    /// - 小文字で始まり、小文字・数字・アンダースコアのみでない
    /// - 数字で始まる
    /// - 空文字列
    pub fn needs_quoting(identifier: &str) -> bool {
        if identifier.is_empty() {
            return true;
        }

        // 大文字に変換して予約語チェック
        let upper = identifier.to_uppercase();
        if Self::RESERVED_WORDS.contains(&upper.as_str()) {
            return true;
        }

        // 識別子のルールチェック
        // PostgreSQL ではクォートなしの識別子は小文字に変換される
        // クォートなしで有効なのは: [a-z_][a-z0-9_]*
        let bytes = identifier.as_bytes();
        if bytes.is_empty() {
            return true;
        }

        // 先頭文字チェック
        let first = bytes[0];
        if !first.is_ascii_lowercase() && first != b'_' {
            return true;
        }

        // 2文字目以降のチェック
        for &b in &bytes[1..] {
            if !b.is_ascii_lowercase() && !b.is_ascii_digit() && b != b'_' {
                return true;
            }
        }

        false
    }

    /// 識別子をクォートする
    ///
    /// # Arguments
    ///
    /// * `identifier` - 識別子文字列
    ///
    /// # Returns
    ///
    /// クォートされた識別子
    ///
    /// # Note
    ///
    /// - クォートが必要な場合はダブルクォートで囲む
    /// - ダブルクォートは二重にエスケープ（`"` → `""`）
    pub fn quote(identifier: &str) -> String {
        if Self::needs_quoting(identifier) {
            // ダブルクォートを二重にエスケープ
            let escaped = identifier.replace('"', "\"\"");
            format!("\"{}\"", escaped)
        } else {
            // クォート不要（PostgreSQL は自動的に小文字に変換）
            identifier.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_quoting_reserved_words() {
        assert!(IdentifierQuoter::needs_quoting("SELECT"));
        assert!(IdentifierQuoter::needs_quoting("FROM"));
        assert!(IdentifierQuoter::needs_quoting("WHERE"));
        assert!(IdentifierQuoter::needs_quoting("TABLE"));
        assert!(IdentifierQuoter::needs_quoting("CREATE"));
        assert!(IdentifierQuoter::needs_quoting("INSERT"));
        assert!(IdentifierQuoter::needs_quoting("UPDATE"));
        assert!(IdentifierQuoter::needs_quoting("DELETE"));
    }

    #[test]
    fn test_needs_quoting_case_sensitive() {
        assert!(IdentifierQuoter::needs_quoting("Users"));  // 大文字開始
        assert!(IdentifierQuoter::needs_quoting("UserName"));  // 大文字を含む
        assert!(IdentifierQuoter::needs_quoting("user-name"));  // ハイフンを含む
        assert!(IdentifierQuoter::needs_quoting("user.name"));  // ドットを含む
    }

    #[test]
    fn test_needs_quoting_starts_with_digit() {
        assert!(IdentifierQuoter::needs_quoting("1column"));
        assert!(IdentifierQuoter::needs_quoting("123users"));
    }

    #[test]
    fn test_needs_quoting_empty() {
        assert!(IdentifierQuoter::needs_quoting(""));
    }

    #[test]
    fn test_needs_quoting_valid_lowercase() {
        assert!(!IdentifierQuoter::needs_quoting("users"));
        assert!(!IdentifierQuoter::needs_quoting("user_name"));
        assert!(!IdentifierQuoter::needs_quoting("_users"));
        assert!(!IdentifierQuoter::needs_quoting("user123"));
        assert!(!IdentifierQuoter::needs_quoting("_123"));
    }

    #[test]
    fn test_quote_reserved_word() {
        assert_eq!(IdentifierQuoter::quote("SELECT"), "\"SELECT\"");
        assert_eq!(IdentifierQuoter::quote("from"), "\"from\"");  // 小文字でも予約語
    }

    #[test]
    fn test_quote_case_sensitive() {
        assert_eq!(IdentifierQuoter::quote("Users"), "\"Users\"");
        assert_eq!(IdentifierQuoter::quote("UserName"), "\"UserName\"");
    }

    #[test]
    fn test_quote_valid_lowercase() {
        assert_eq!(IdentifierQuoter::quote("users"), "users");
        assert_eq!(IdentifierQuoter::quote("user_name"), "user_name");
        assert_eq!(IdentifierQuoter::quote("_users"), "_users");
    }

    #[test]
    fn test_quote_with_double_quote() {
        assert_eq!(IdentifierQuoter::quote("my\"column"), "\"my\"\"column\"");
        assert_eq!(IdentifierQuoter::quote("col\"\"umn"), "\"col\"\"\"\"umn\"");
    }

    #[test]
    fn test_quote_special_chars() {
        assert_eq!(IdentifierQuoter::quote("user-name"), "\"user-name\"");
        assert_eq!(IdentifierQuoter::quote("user.name"), "\"user.name\"");
        assert_eq!(IdentifierQuoter::quote("user name"), "\"user name\"");
    }

    #[test]
    fn test_quote_starts_with_digit() {
        assert_eq!(IdentifierQuoter::quote("1column"), "\"1column\"");
        assert_eq!(IdentifierQuoter::quote("123users"), "\"123users\"");
    }

    #[test]
    fn test_postgres_specific_reserved_words() {
        assert!(IdentifierQuoter::needs_quoting("ANALYZE"));
        assert!(IdentifierQuoter::needs_quoting("CONCURRENTLY"));
        assert!(IdentifierQuoter::needs_quoting("EXCLUDE"));
        assert!(IdentifierQuoter::needs_quoting("MATERIALIZED"));
        assert!(IdentifierQuoter::needs_quoting("TABLESPACE"));
        assert!(IdentifierQuoter::needs_quoting("VACUUM"));
    }
}
