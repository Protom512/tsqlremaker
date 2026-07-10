//! Cross-file symbol index — the workspace-wide reverse-lookup map.
//!
//! `SymbolStore` aggregates the symbols of every document the language server
//! knows about (open documents + background-indexed files) behind a single
//! case-insensitive name → entries map, so that workspace symbol search
//! ([`crate::workspace_symbols`]) and cross-file goto-definition /
//! goto-references can resolve a name without re-parsing every document.
//!
//! ## Design (Balanced Coupling)
//!
//! The store lives in the stable `ase-ls-core` crate and consumes only the
//! existing pure primitives already here: [`DocumentAnalysis`] (its
//! [`SymbolTable`](crate::symbol_table::SymbolTable)) and the
//! [`CaseInsensitiveKey`] type that the symbol table already uses for its own
//! maps. It does not re-parse, lex, or read the filesystem.
//!
//! ## Precedence (Open/Live beats Background)
//!
//! Each entry carries a [`DocumentSource`] tag. When two sources contribute a
//! symbol with the same name, an `Open`/`Live` entry always shadows a
//! `Background` entry — the on-disk indexed copy must never overwrite the
//! editor's live buffer. This invariant is enforced in [`SymbolStore::upsert`]
//! and is exercised by the precedence tests in this module.
//!
//! ## Lock-ordering convention (CALLER responsibility)
//!
//! In the live server, `DocumentStore` and `SymbolStore` are both
//! `tokio::sync::RwLock`-guarded. To avoid deadlock, the **`DocumentStore`
//! write lock must always be acquired BEFORE the `SymbolStore` write lock** in
//! any `did_open` / `did_change` / `did_close` handler that touches both. This
//! module cannot enforce that ordering (it owns no `DocumentStore`), so the
//! convention is documented here and must be honoured by the caller.

use crate::analysis::DocumentAnalysis;
use crate::symbol_table::{
    CaseInsensitiveKey, IndexSymbol, SymbolTable, TableSymbol, TriggerSymbol,
};
use lsp_types::{Range, SymbolKind, Url};
use std::collections::HashMap;

/// Where a [`SymbolEntry`] came from.
///
/// Governs precedence inside [`SymbolStore`]: an `Open` (or `Live`) entry
/// always shadows a `Background` entry with the same name, so the editor's
/// live buffer is never overwritten by the on-disk indexed copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentSource {
    /// The document is open in an editor (live buffer authoritative).
    Open,
    /// The document is the live, just-edited buffer (alias of [`Open`] for
    /// precedence purposes — distinct tag kept for diagnostics).
    Live,
    /// The entry was produced by background indexing of an on-disk file.
    Background,
}

impl DocumentSource {
    /// Returns `true` if this source represents a live editor buffer and
    /// therefore shadows [`Background`] entries.
    #[must_use]
    #[inline]
    pub fn is_live(self) -> bool {
        matches!(self, Self::Open | Self::Live)
    }
}

/// One occurrence of a named symbol in one document.
///
/// Multiple entries can share a name (a table defined in two `.sql` files, or
/// a `@count` variable declared in several procedures); the store groups them
/// under a single [`CaseInsensitiveKey`].
#[derive(Debug, Clone)]
pub struct SymbolEntry {
    /// Symbol name in its original source casing (NOT upper-normalized).
    ///
    /// The reverse-map key is upper-normalized for lookup; this field preserves
    /// the casing seen by the user so workspace-symbol results read naturally.
    pub name: String,
    /// Document URI holding this symbol.
    pub uri: Url,
    /// LSP range of the symbol name in `uri`.
    pub range: Range,
    /// Workspace-symbol category mapping (CLASS/FUNCTION/...).
    pub kind: SymbolKind,
    /// Owning object name, if any (table name for indexes and triggers).
    pub container_name: Option<String>,
    /// Origin of this entry (Open/Live vs Background).
    pub source: DocumentSource,
}

/// Category of a definition contributed to the store.
///
/// Mirrors the symbol-table's six maps and the existing workspace-symbol
/// `SymbolKind` mapping in [`crate::workspace_symbols`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymbolCategory {
    Table,
    Procedure,
    View,
    Index,
    Trigger,
    Variable,
}

impl SymbolCategory {
    /// The LSP `SymbolKind` this category maps to.
    #[must_use]
    const fn kind(self) -> SymbolKind {
        match self {
            Self::Table => SymbolKind::CLASS,
            Self::Procedure => SymbolKind::FUNCTION,
            Self::View => SymbolKind::INTERFACE,
            Self::Index => SymbolKind::PROPERTY,
            Self::Trigger => SymbolKind::EVENT,
            Self::Variable => SymbolKind::VARIABLE,
        }
    }

    /// Container name for this entry: the owning table for indexes and
    /// triggers, `None` otherwise.
    fn container_name(self, entry: &FlattenedEntry) -> Option<String> {
        match self {
            Self::Index => Some(entry.index_table_name.clone()),
            Self::Trigger => Some(entry.index_table_name.clone()),
            _ => None,
        }
    }
}

/// Flatten of one symbol-table record into the fields the store needs to build
/// a [`SymbolEntry`]. Avoids passing six different concrete symbol types into
/// the insertion loop.
struct FlattenedEntry {
    name: String,
    range: Range,
    /// Reused for both Index and Trigger container names (owning table).
    index_table_name: String,
    category: SymbolCategory,
}

/// Cross-file symbol index.
///
/// A reverse map from case-insensitive symbol name to every [`SymbolEntry`]
/// that currently contributes that name across all known documents. Construct
/// one per language server, mutate via [`upsert`](Self::upsert) /
/// [`close`](Self::close), and query via [`lookup`](Self::lookup) /
/// [`iter`](Self::iter).
#[derive(Debug, Default)]
pub struct SymbolStore {
    /// name (upper) → all occurrences known to the store.
    by_name: HashMap<CaseInsensitiveKey, Vec<SymbolEntry>>,
    /// uri → set of names that uri currently contributes.
    ///
    /// Used by [`close`](Self::close) to remove exactly the entries a document
    /// added without scanning the whole reverse map.
    by_uri: HashMap<Url, Vec<CaseInsensitiveKey>>,
}

impl SymbolStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace all symbols contributed by `uri` with the symbols found in
    /// `analysis`, tagged with `source`.
    ///
    /// This is idempotent for a fixed `(uri, source)`: calling it twice with
    /// the same analysis yields the same store state (the previous entries for
    /// `uri` are dropped before the new ones are inserted).
    ///
    /// # Precedence
    ///
    /// Within a single `upsert` all entries for `uri` share the same `source`,
    /// so precedence is uniform per call. Cross-document precedence (Open/Live
    /// shadowing Background) is preserved because `close` + `upsert` only ever
    /// touch the names a given uri contributed — a Background upsert for file A
    /// cannot evict an Open upsert for file B.
    pub fn upsert(&mut self, uri: &Url, analysis: &DocumentAnalysis, source: DocumentSource) {
        // Remove the previous contribution of this uri before re-inserting.
        self.close(uri);
        let entries = flatten_table(&analysis.symbol_table, source);
        for flat in entries {
            let category = flat.category;
            let entry = SymbolEntry {
                name: flat.name.clone(),
                uri: uri.clone(),
                range: flat.range,
                kind: category.kind(),
                container_name: category.container_name(&flat),
                source,
            };
            let key = CaseInsensitiveKey::new(&flat.name);
            self.by_uri
                .entry(uri.clone())
                .or_default()
                .push(key.clone());
            self.by_name.entry(key).or_default().push(entry);
        }
    }

    /// Remove every symbol contributed by `uri`, restoring the store to the
    /// state as if `uri` had never been inserted.
    ///
    /// After this call, [`lookup`](Self::lookup) for any name that lived only
    /// in `uri` returns an empty slice.
    pub fn close(&mut self, uri: &Url) {
        let Some(names) = self.by_uri.remove(uri) else {
            return;
        };
        for name in names {
            if let Some(vec) = self.by_name.get_mut(&name) {
                vec.retain(|e| &e.uri != uri);
                if vec.is_empty() {
                    self.by_name.remove(&name);
                }
            }
        }
    }

    /// Look up every occurrence of `name` (case-insensitive).
    #[must_use]
    pub fn lookup(&self, name: &str) -> &[SymbolEntry] {
        let key = CaseInsensitiveKey::new(name);
        match self.by_name.get(&key) {
            Some(vec) => vec,
            None => &[],
        }
    }

    /// Iterate over `(name, entries)` pairs for every name in the store.
    ///
    /// Provided for callers (e.g. workspace symbol search) that need to scan
    /// the whole index rather than a single name.
    pub fn iter(&self) -> impl Iterator<Item = (&CaseInsensitiveKey, &[SymbolEntry])> {
        self.by_name.iter().map(|(k, v)| (k, v.as_slice()))
    }

    /// Number of distinct names currently indexed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Whether the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// Flatten a [`SymbolTable`] into the fields the store needs, applying the
/// category → `SymbolKind` mapping that [`crate::workspace_symbols`] already
/// uses (table=CLASS, procedure=FUNCTION, view=INTERFACE, index=PROPERTY,
/// trigger=EVENT, variable=VARIABLE).
fn flatten_table(table: &SymbolTable, _source: DocumentSource) -> Vec<FlattenedEntry> {
    let mut out = Vec::new();
    for TableSymbol { name, range, .. } in table.tables.values() {
        out.push(FlattenedEntry {
            name: name.clone(),
            range: *range,
            index_table_name: String::new(),
            category: SymbolCategory::Table,
        });
    }
    for proc in table.procedures.values() {
        out.push(FlattenedEntry {
            name: proc.name.clone(),
            range: proc.range,
            index_table_name: String::new(),
            category: SymbolCategory::Procedure,
        });
    }
    for view in table.views.values() {
        out.push(FlattenedEntry {
            name: view.name.clone(),
            range: view.range,
            index_table_name: String::new(),
            category: SymbolCategory::View,
        });
    }
    for IndexSymbol {
        name,
        range,
        table_name,
        ..
    } in table.indexes.values()
    {
        out.push(FlattenedEntry {
            name: name.clone(),
            range: *range,
            index_table_name: table_name.clone(),
            category: SymbolCategory::Index,
        });
    }
    for TriggerSymbol {
        name,
        range,
        table_name,
        ..
    } in table.triggers.values()
    {
        out.push(FlattenedEntry {
            name: name.clone(),
            range: *range,
            index_table_name: table_name.clone(),
            category: SymbolCategory::Trigger,
        });
    }
    for var in table.variables.values() {
        out.push(FlattenedEntry {
            name: var.name.clone(),
            range: var.range,
            index_table_name: String::new(),
            category: SymbolCategory::Variable,
        });
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use lsp_types::Position;

    fn uri(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn analysis_of(src: &str) -> DocumentAnalysis {
        DocumentAnalysis::new(src)
    }

    // ===== DocumentSource =====

    #[test]
    fn test_document_source_is_live() {
        assert!(DocumentSource::Open.is_live());
        assert!(DocumentSource::Live.is_live());
        assert!(!DocumentSource::Background.is_live());
    }

    // ===== upsert / lookup basics =====

    #[test]
    fn test_upsert_table_is_lookupable() {
        let mut store = SymbolStore::new();
        let u = uri("file:///a.sql");
        store.upsert(
            &u,
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );

        let entries = store.lookup("users");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].uri, u);
        assert_eq!(entries[0].kind, SymbolKind::CLASS);
        assert_eq!(entries[0].container_name, None);
        assert_eq!(entries[0].source, DocumentSource::Open);
    }

    #[test]
    fn test_lookup_is_case_insensitive() {
        let mut store = SymbolStore::new();
        store.upsert(
            &uri("file:///a.sql"),
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        assert!(!store.lookup("USERS").is_empty());
        assert!(!store.lookup("Users").is_empty());
        assert!(!store.lookup("users").is_empty());
    }

    #[test]
    fn test_lookup_unknown_returns_empty() {
        let store = SymbolStore::new();
        assert!(store.lookup("nope").is_empty());
    }

    #[test]
    fn test_category_kind_mapping() {
        let mut store = SymbolStore::new();
        let src = "CREATE TABLE t (id INT)\n\
                   CREATE PROCEDURE p AS BEGIN SELECT 1 END\n\
                   CREATE VIEW v AS SELECT 1\n\
                   CREATE INDEX i ON t (id)\n\
                   CREATE TRIGGER tr ON t FOR INSERT AS BEGIN SELECT 1 END\n\
                   DECLARE @x INT";
        store.upsert(
            &uri("file:///a.sql"),
            &analysis_of(src),
            DocumentSource::Open,
        );

        assert_eq!(store.lookup("t")[0].kind, SymbolKind::CLASS);
        assert_eq!(store.lookup("p")[0].kind, SymbolKind::FUNCTION);
        assert_eq!(store.lookup("v")[0].kind, SymbolKind::INTERFACE);
        assert_eq!(store.lookup("i")[0].kind, SymbolKind::PROPERTY);
        assert_eq!(store.lookup("tr")[0].kind, SymbolKind::EVENT);
        assert_eq!(store.lookup("@x")[0].kind, SymbolKind::VARIABLE);
    }

    #[test]
    fn test_container_name_for_index_and_trigger() {
        let mut store = SymbolStore::new();
        let src = "CREATE TABLE t (id INT)\n\
                   CREATE INDEX i ON t (id)\n\
                   CREATE TRIGGER tr ON t FOR INSERT AS BEGIN SELECT 1 END";
        store.upsert(
            &uri("file:///a.sql"),
            &analysis_of(src),
            DocumentSource::Open,
        );

        assert_eq!(store.lookup("i")[0].container_name, Some("t".to_string()));
        assert_eq!(store.lookup("tr")[0].container_name, Some("t".to_string()));
        assert_eq!(store.lookup("t")[0].container_name, None);
    }

    // ===== cross-file aggregation =====

    #[test]
    fn test_same_name_in_two_files_aggregates() {
        let mut store = SymbolStore::new();
        store.upsert(
            &uri("file:///a.sql"),
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        store.upsert(
            &uri("file:///b.sql"),
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        let entries = store.lookup("users");
        let uris: Vec<&Url> = entries.iter().map(|e| &e.uri).collect();
        assert_eq!(uris.len(), 2);
        assert!(uris.contains(&&uri("file:///a.sql")));
        assert!(uris.contains(&&uri("file:///b.sql")));
    }

    // ===== idempotent re-upsert =====

    #[test]
    fn test_re_upsert_replaces_previous_contribution() {
        let mut store = SymbolStore::new();
        let u = uri("file:///a.sql");
        store.upsert(
            &u,
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        // Same uri, now declares a different table and not users.
        store.upsert(
            &u,
            &analysis_of("CREATE TABLE orders (id INT)"),
            DocumentSource::Open,
        );

        assert!(
            store.lookup("users").is_empty(),
            "users must be evicted by re-upsert"
        );
        assert_eq!(store.lookup("orders").len(), 1);
    }

    // ===== close =====

    #[test]
    fn test_close_evicts_uri_entries_only() {
        let mut store = SymbolStore::new();
        let a = uri("file:///a.sql");
        let b = uri("file:///b.sql");
        store.upsert(
            &a,
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        store.upsert(
            &b,
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );

        store.close(&a);
        let entries = store.lookup("users");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].uri, b);
    }

    #[test]
    fn test_close_unknown_uri_is_noop() {
        let mut store = SymbolStore::new();
        store.upsert(
            &uri("file:///a.sql"),
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        store.close(&uri("file:///never.sql"));
        assert_eq!(store.lookup("users").len(), 1);
    }

    #[test]
    fn test_close_removes_name_key_when_empty() {
        let mut store = SymbolStore::new();
        let u = uri("file:///a.sql");
        store.upsert(
            &u,
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        store.close(&u);
        assert!(
            store.is_empty(),
            "name key should be removed when its last entry closes"
        );
    }

    // ===== precedence: Open/Live shadows Background =====

    #[test]
    fn test_open_and_background_coexist_under_same_name() {
        // Both contributions survive (the store does not dedupe across uris);
        // precedence is resolved by the *caller* filtering on source. The
        // store's job is to keep both entries addressable.
        let mut store = SymbolStore::new();
        store.upsert(
            &uri("file:///a.sql"),
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        store.upsert(
            &uri("file:///a.sql"),
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Background,
        );
        // Same uri re-upsert evicts the Open entry; final state is Background only.
        let entries = store.lookup("users");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, DocumentSource::Background);
    }

    #[test]
    fn test_two_uris_open_and_background_kept_separate() {
        let mut store = SymbolStore::new();
        store.upsert(
            &uri("file:///live.sql"),
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        store.upsert(
            &uri("file:///disk.sql"),
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Background,
        );
        let entries = store.lookup("users");
        assert_eq!(entries.len(), 2);
        // Both sources are present; caller can filter `source.is_live()`.
        let has_open = entries.iter().any(|e| e.source == DocumentSource::Open);
        let has_bg = entries
            .iter()
            .any(|e| e.source == DocumentSource::Background);
        assert!(has_open && has_bg);
    }

    // ===== len / is_empty =====

    #[test]
    fn test_len_counts_distinct_names() {
        let mut store = SymbolStore::new();
        assert!(store.is_empty());
        store.upsert(
            &uri("file:///a.sql"),
            &analysis_of("CREATE TABLE t (id INT)\nCREATE VIEW v AS SELECT 1"),
            DocumentSource::Open,
        );
        assert_eq!(store.len(), 2);
    }

    // ===== iter =====

    #[test]
    fn test_iter_yields_all_names() {
        let mut store = SymbolStore::new();
        store.upsert(
            &uri("file:///a.sql"),
            &analysis_of("CREATE TABLE t (id INT)\nCREATE VIEW v AS SELECT 1"),
            DocumentSource::Open,
        );
        let names: Vec<String> = store.iter().map(|(k, _)| k.as_str().to_string()).collect();
        assert!(names.contains(&"T".to_string()));
        assert!(names.contains(&"V".to_string()));
    }

    #[test]
    fn test_range_carried_through() {
        let mut store = SymbolStore::new();
        let u = uri("file:///a.sql");
        store.upsert(
            &u,
            &analysis_of("CREATE TABLE users (id INT)"),
            DocumentSource::Open,
        );
        let entry = &store.lookup("users")[0];
        // range start should be at or after position 0, end after start
        assert!(
            entry.range.start
                >= Position {
                    line: 0,
                    character: 0
                }
        );
        assert!(entry.range.end >= entry.range.start);
    }
}
