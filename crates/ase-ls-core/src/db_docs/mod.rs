//! ASE Documentation Data — SAP ASE ドメインデータの単一ソース
//!
//! キーワード、データ型、組み込み関数のドキュメントデータを提供する。
//! hover, signature_help, completion の各モジュールはこのデータを参照する。
//!
//! ## 構成
//!
//! - `keywords` — SQLキーワード（SELECT, FROM, WHERE 等）
//! - `datatypes` — データ型（INT, VARCHAR, DATETIME 等）
//! - `functions` — 組み込み関数（SUBSTRING, GETDATE 等）
//! - `sysvars` — システム変数（@@VERSION, @@ROWCOUNT 等）

mod datatypes;
mod functions;
mod keywords;
mod sysvars;

use std::collections::HashMap;
use std::sync::LazyLock;

pub use datatypes::DATATYPE_ENTRIES;
pub use functions::FUNCTION_ENTRIES;
pub use keywords::KEYWORD_ENTRIES;
pub use sysvars::SYSTEM_VARIABLE_ENTRIES;

/// ドキュメントエントリのカテゴリ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocCategory {
    /// SQLキーワード
    Keyword,
    /// データ型
    DataType,
    /// 組み込み関数
    Function,
    /// システム変数
    SystemVariable,
}

/// ASE組み込みドキュメントエントリ
#[derive(Debug, Clone)]
pub struct DocEntry {
    /// 名前（大文字）
    pub name: &'static str,
    /// 説明文
    pub description: &'static str,
    /// 構文例
    pub syntax: &'static str,
    /// パラメータ名リスト
    pub params: &'static [&'static str],
    /// カテゴリ
    pub category: DocCategory,
}

// ---------------------------------------------------------------------------
// Lookup helpers — O(1) by name
// ---------------------------------------------------------------------------

/// 関数エントリの名前で検索できるHashMap（関数優先）
static FUNCTION_LOOKUP: LazyLock<HashMap<&'static str, &'static DocEntry>> =
    LazyLock::new(|| FUNCTION_ENTRIES.iter().map(|e| (e.name, e)).collect());

/// キーワード・データ型・システム変数エントリの名前で検索できるHashMap
static OTHER_LOOKUP: LazyLock<HashMap<&'static str, &'static DocEntry>> = LazyLock::new(|| {
    KEYWORD_ENTRIES
        .iter()
        .chain(DATATYPE_ENTRIES.iter())
        .chain(SYSTEM_VARIABLE_ENTRIES.iter())
        .map(|e| (e.name, e))
        .collect()
});

/// 名前（大文字）で関数 DocEntry を検索する
#[must_use]
pub fn lookup_function(name: &str) -> Option<&'static DocEntry> {
    FUNCTION_LOOKUP.get(name).copied()
}

/// 名前（大文字）で DocEntry を検索する
/// キーワードと関数で名前が重複する場合（例: LEFT）、キーワードを優先する
#[must_use]
pub fn lookup(name: &str) -> Option<&'static DocEntry> {
    OTHER_LOOKUP
        .get(name)
        .copied()
        .or_else(|| FUNCTION_LOOKUP.get(name).copied())
}

/// キーワードエントリのスライスを返す
#[must_use]
pub fn keywords() -> &'static [DocEntry] {
    KEYWORD_ENTRIES
}

/// データ型エントリのスライスを返す
#[must_use]
pub fn datatypes() -> &'static [DocEntry] {
    DATATYPE_ENTRIES
}

/// 関数エントリのスライスを返す
#[must_use]
pub fn functions() -> &'static [DocEntry] {
    FUNCTION_ENTRIES
}

/// システム変数エントリのスライスを返す
#[must_use]
pub fn system_variables() -> &'static [DocEntry] {
    SYSTEM_VARIABLE_ENTRIES
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_keyword() {
        let entry = lookup("SELECT").unwrap();
        assert_eq!(entry.name, "SELECT");
        assert_eq!(entry.category, DocCategory::Keyword);
        assert!(!entry.description.is_empty());
    }

    #[test]
    fn test_lookup_datatype() {
        let entry = lookup("VARCHAR").unwrap();
        assert_eq!(entry.name, "VARCHAR");
        assert_eq!(entry.category, DocCategory::DataType);
    }

    #[test]
    fn test_lookup_function() {
        let entry = lookup("SUBSTRING").unwrap();
        assert_eq!(entry.name, "SUBSTRING");
        assert_eq!(entry.category, DocCategory::Function);
        assert_eq!(entry.params, &["expression", "start", "length"]);
    }

    #[test]
    fn test_lookup_keyword_right() {
        // RIGHT はキーワード（RIGHT JOIN）として存在
        let entry = lookup("RIGHT").unwrap();
        assert_eq!(entry.name, "RIGHT");
        assert_eq!(entry.category, DocCategory::Keyword);
    }

    #[test]
    fn test_lookup_case_sensitive() {
        // lookup is case-sensitive (uppercase keys)
        assert!(lookup("select").is_none());
        assert!(lookup("SELECT").is_some());
    }

    #[test]
    fn test_lookup_not_found() {
        assert!(lookup("NONEXISTENT").is_none());
    }

    #[test]
    fn test_no_duplicate_names() {
        // 各カテゴリ内で重複がないことを確認
        let mut keyword_names = std::collections::HashSet::new();
        for e in KEYWORD_ENTRIES.iter() {
            assert!(
                !keyword_names.contains(e.name),
                "Duplicate keyword name: {}",
                e.name
            );
            keyword_names.insert(e.name);
        }

        let mut datatype_names = std::collections::HashSet::new();
        for e in DATATYPE_ENTRIES.iter() {
            assert!(
                !datatype_names.contains(e.name),
                "Duplicate datatype name: {}",
                e.name
            );
            datatype_names.insert(e.name);
        }

        let mut function_names = std::collections::HashSet::new();
        for e in FUNCTION_ENTRIES.iter() {
            assert!(
                !function_names.contains(e.name),
                "Duplicate function name: {}",
                e.name
            );
            function_names.insert(e.name);
        }

        // カテゴリ間での重複は許容（例: IDENTITY はキーワードとしても関数としても存在）
        // lookup() は関数を優先して返す
    }

    #[test]
    fn test_all_entries_have_description() {
        for e in KEYWORD_ENTRIES
            .iter()
            .chain(DATATYPE_ENTRIES.iter())
            .chain(FUNCTION_ENTRIES.iter())
        {
            assert!(
                !e.description.is_empty(),
                "Missing description for: {}",
                e.name
            );
            assert!(!e.syntax.is_empty(), "Missing syntax for: {}", e.name);
        }
    }

    #[test]
    fn test_entry_counts() {
        assert!(!KEYWORD_ENTRIES.is_empty());
        assert!(!DATATYPE_ENTRIES.is_empty());
        assert!(!FUNCTION_ENTRIES.is_empty());
        // Spot-check counts are in reasonable range
        assert!(KEYWORD_ENTRIES.len() >= 50);
        assert!(DATATYPE_ENTRIES.len() >= 25);
        assert!(FUNCTION_ENTRIES.len() >= 35);
    }
}
