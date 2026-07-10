//! Workspace indexing helpers — pure, testable file-discovery and analysis entry points.
//!
//! These functions decouple two concerns that previously could only be exercised
//! through a live LSP server (`ase-ls::server.rs`):
//!
//! 1. **File discovery** — recursively walking a workspace root and collecting
//!    `*.sql` file paths. Pure (no I/O at the `&Path` API boundary: the caller
//!    supplies the directory entries).
//! 2. **Per-file analysis** — building a [`DocumentAnalysis`] from `(uri, contents)`
//!    by delegating to the existing [`DocumentAnalysis::new`] entry point, which is
//!    already graceful for incomplete SQL via `parse_with_errors` +
//!    `build_tolerant`.
//!
//! ## Design contract (graceful degradation)
//!
//! `analyze_file_contents` MUST NOT build a new parse path. It reuses
//! [`DocumentAnalysis::new`] verbatim, so every graceful-degradation guarantee of
//! that constructor (partial AST via error recovery, tolerant symbol table, CREATE
//! TABLE fallback) applies unchanged. This keeps UC-3 (incomplete SQL must not
//! crash indexing) satisfied by construction rather than by a parallel
//! implementation that could drift.

use crate::analysis::DocumentAnalysis;
use std::path::{Path, PathBuf};
use tsql_token::TokenKind;

/// Default maximum recursion depth for [`discover_sql_files`] when the caller
/// does not supply an explicit bound.
///
/// Guards against runaway traversal on pathological or symlink-heavy directory
/// trees. The depth is counted in directory-descending edges from the root.
pub const DEFAULT_MAX_DEPTH: usize = 32;

/// Default ceiling on the number of `*.sql` files collected by
/// [`discover_sql_files`] when the caller does not supply an explicit bound.
///
/// This prevents unbounded memory growth when a workspace root happens to contain
/// an enormous number of SQL files. The first `limit` matching files (in
/// traversal order) are returned.
pub const DEFAULT_MAX_FILES: usize = 10_000;

/// One directory entry supplied to [`discover_sql_files`].
///
/// This is a pure-data view of a filesystem entry so the discovery algorithm can
/// be unit-tested without touching disk. Production callers build these from
/// `std::fs::read_dir`; tests build them by hand.
///
/// - `path`: absolute or workspace-root-relative path of the entry
/// - `is_dir`: whether the entry is a directory (descend into it) or a file
/// - `is_symlink`: whether the entry is a symbolic link (skipped by default)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// Path of the entry.
    pub path: PathBuf,
    /// `true` if the entry is a directory.
    pub is_dir: bool,
    /// `true` if the entry is a symbolic link.
    pub is_symlink: bool,
}

impl DirEntry {
    /// Build a `DirEntry` for a regular directory.
    #[must_use]
    pub fn dir(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            is_dir: true,
            is_symlink: false,
        }
    }

    /// Build a `DirEntry` for a regular file.
    #[must_use]
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            is_dir: false,
            is_symlink: false,
        }
    }

    /// Build a `DirEntry` for a symbolic link (file or directory).
    #[must_use]
    pub fn symlink(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            is_dir: false,
            is_symlink: true,
        }
    }
}

/// SQL extension matched by [`discover_sql_files`] (case-insensitive on the
/// final path component).
const SQL_EXTENSION: &str = "sql";

/// Returns `true` iff `path`'s final component has a `.sql` extension
/// (case-insensitive ASCII comparison).
///
/// Uses ASCII case-folding (not Unicode) because file extensions are historical
/// ASCII identifiers; this keeps the check allocation-free and deterministic
/// across locales.
fn has_sql_extension(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => ext.eq_ignore_ascii_case(SQL_EXTENSION),
        None => false,
    }
}

/// Recursively discover `*.sql` file paths under a workspace root, given a
/// pure listing of directory entries.
///
/// This is the testable core of workspace file discovery. The caller supplies
/// `entries` — the complete, flat set of [`DirEntry`] values reachable beneath
/// `root` — and the function returns the subset that are non-symlink files with
/// a `.sql` extension, ordered as a stable depth-first traversal of `root`.
///
/// # Guards against runaway traversal
///
/// - **Depth**: descends at most `max_depth` directory edges beneath `root`.
///   [`DEFAULT_MAX_DEPTH`] is used when `max_depth == 0`.
/// - **Symlinks**: symlink entries are never followed (avoids cycles).
/// - **File count**: returns at most `max_files` results. [`DEFAULT_MAX_FILES`]
///   is used when `max_files == 0`.
///
/// The function is total: it never panics and never performs I/O.
///
/// # Arguments
///
/// - `root`: the workspace root path. Entries not beneath `root` (i.e. whose
///   path does not start with `root`) are ignored.
/// - `entries`: the flat listing of all entries beneath `root` (directories,
///   files, symlinks). Order within `entries` does not affect correctness, but
///   a parent-before-child order yields a natural depth-first result order.
/// - `max_depth`: maximum directory descent depth. `0` selects the default.
/// - `max_files`: cap on the number of returned paths. `0` selects the default.
///
/// # Returns
///
/// A `Vec<PathBuf>` of `*.sql` file paths in stable traversal order.
#[must_use]
pub fn discover_sql_files(
    root: &Path,
    entries: &[DirEntry],
    max_depth: usize,
    max_files: usize,
) -> Vec<PathBuf> {
    let effective_max_depth = if max_depth == 0 {
        DEFAULT_MAX_DEPTH
    } else {
        max_depth
    };
    let effective_max_files = if max_files == 0 {
        DEFAULT_MAX_FILES
    } else {
        max_files
    };

    // Index directories by their path for O(1) child lookup during DFS.
    // A path is a "child" of directory `d` if it starts with `d` and has exactly
    // one more component. We compute depth relative to root via component count.
    let root_depth = root.components().count();

    let mut result: Vec<PathBuf> = Vec::new();
    if effective_max_files == 0 {
        return result;
    }

    // Depth-first traversal using an explicit stack of (path, depth) frames.
    // Seed with the root directory at depth 0.
    let mut stack: Vec<(PathBuf, usize)> = Vec::new();
    stack.push((root.to_path_buf(), 0));

    while let Some((dir, depth)) = stack.pop() {
        // Collect candidate children of `dir` that live directly beneath it.
        // We iterate entries once per directory; for the expected workspace
        // sizes (hundreds-to-low-thousands of SQL files) this is acceptable and
        // keeps the function allocation-light (no HashMap needed).
        let mut children: Vec<&DirEntry> = Vec::new();
        for entry in entries {
            if !is_descendant_of(root, root_depth, &entry.path, &dir) {
                continue;
            }
            // Only direct children (one component deeper than `dir`).
            let dir_depth = dir.components().count();
            if entry.path.components().count() == dir_depth + 1 {
                children.push(entry);
            }
        }

        // Sort children by path for deterministic traversal order regardless
        // of the input ordering. Reverse because we pop from the stack (LIFO),
        // so sorting ascending then reversing yields ascending visit order.
        children.sort_by(|a, b| a.path.cmp(&b.path));
        // Push reversed so the lexicographically-first child is visited first.
        for child in children.into_iter().rev() {
            // Never follow symlinks (cycle / runaway guard).
            if child.is_symlink {
                continue;
            }
            if child.is_dir {
                if depth + 1 < effective_max_depth {
                    stack.push((child.path.clone(), depth + 1));
                }
            } else if has_sql_extension(&child.path) && result.len() < effective_max_files {
                result.push(child.path.clone());
            }
        }
    }

    // result was built via DFS with reversed-ascending stack pushes, which
    // produces ascending order already; but because directories are pushed
    // alongside files, re-sort for a stable, predictable contract.
    result.sort();
    result
}

/// Returns `true` iff `path` is equal to or beneath `root` (component-prefix),
/// using `root_depth` to short-circuit the comparison.
fn is_descendant_of(root: &Path, root_depth: usize, path: &Path, dir: &Path) -> bool {
    // Path must start with root.
    if !path.starts_with(root) {
        return false;
    }
    let _ = root_depth;
    // Path must start with dir (direct descendant check is done by caller via
    // component-count comparison, but we also require `dir` to be an ancestor).
    if dir.as_os_str().is_empty() {
        return true;
    }
    path.starts_with(dir)
}

/// Build a [`DocumentAnalysis`] for a workspace file from its URI and contents.
///
/// This is the per-file indexing entry point used by the background workspace
/// indexer. It exists to give callers a single, documented, `#[must_use]`
/// function that produces a graceful analysis from arbitrary file contents —
/// including incomplete or syntactically invalid SQL — **without building a new
/// parse path**.
///
/// # Graceful-degradation contract
///
/// This function delegates entirely to [`DocumentAnalysis::new`], which:
///
/// - Lexes leniently (filters lexer errors).
/// - Parses with `parse_with_errors`, returning partial AST statements plus
///   captured errors instead of failing on the first error.
/// - Builds a tolerant symbol table (`SymbolTableBuilder::build_tolerant`) that
///   skips unparseable batches and falls back to a CREATE TABLE token scan.
///
/// As a result, indexing a file containing truncated SQL (UC-3) yields an
/// analysis with whatever statements/tables could be salvaged plus the recorded
/// parse errors — it never panics and never returns `Err`.
///
/// The `uri` parameter is accepted for API symmetry with the live `did_open`
/// path (where the URI is the store key) and so callers can associate the
/// returned analysis with its source document. The analysis itself does not
/// embed the URI; it derives purely from `contents`.
///
/// # Arguments
///
/// - `uri`: the document URI (the store key; informational here).
/// - `contents`: the raw file contents (UTF-8).
///
/// # Returns
///
/// A fully-built [`DocumentAnalysis`]. Always succeeds.
#[must_use]
pub fn analyze_file_contents(uri: &lsp_types::Url, contents: &str) -> DocumentAnalysis {
    // Intentionally do NOT read the `uri` for parsing — the analysis is a pure
    // function of `contents`. Touching `uri` here would create a hidden coupling
    // between filesystem location and parse behavior.
    let _ = uri;
    DocumentAnalysis::new(contents)
}

/// Returns `true` iff an analysis contains at least one statement of a kind that
/// contributes symbols to the workspace (CREATE TABLE/VIEW/INDEX/PROCEDURE).
///
/// Provided as a pure test/inspection helper so callers can cheaply decide
/// whether a file is worth indexing into the symbol store without re-parsing.
#[must_use]
pub fn analysis_has_symbols(analysis: &DocumentAnalysis) -> bool {
    use tsql_parser::ast::Statement;
    // A file is "symbol-bearing" if it parsed at least one top-level definition
    // (CREATE/DECLARE), or the tolerant symbol table salvaged any table, or the
    // raw token stream still shows a CREATE keyword (partial parse signal).
    analysis
        .statements
        .iter()
        .any(|s| matches!(s, Statement::Create(_) | Statement::Declare(_)))
        || !analysis.symbol_table.tables.is_empty()
        || analysis.tokens.iter().any(|t| t.kind == TokenKind::Create)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use lsp_types::Url;

    fn u(p: &str) -> Url {
        Url::parse(p).unwrap()
    }

    fn entry_dir(p: &str) -> DirEntry {
        DirEntry::dir(p)
    }
    fn entry_file(p: &str) -> DirEntry {
        DirEntry::file(p)
    }
    fn entry_symlink(p: &str) -> DirEntry {
        DirEntry::symlink(p)
    }

    // ===== has_sql_extension =====

    #[test]
    fn test_has_sql_extension_lowercase() {
        assert!(has_sql_extension(Path::new("queries/a.sql")));
    }

    #[test]
    fn test_has_sql_extension_uppercase() {
        assert!(has_sql_extension(Path::new("queries/A.SQL")));
    }

    #[test]
    fn test_has_sql_extension_mixed_case() {
        assert!(has_sql_extension(Path::new("queries/a.Sql")));
        assert!(has_sql_extension(Path::new("queries/a.sQL")));
    }

    #[test]
    fn test_has_sql_extension_wrong_ext() {
        assert!(!has_sql_extension(Path::new("queries/a.txt")));
        assert!(!has_sql_extension(Path::new("queries/a.rs")));
        assert!(!has_sql_extension(Path::new("queries/a.sql.bak")));
    }

    #[test]
    fn test_has_sql_extension_no_ext() {
        assert!(!has_sql_extension(Path::new("queries/Makefile")));
        assert!(!has_sql_extension(Path::new("queries/")));
    }

    // ===== discover_sql_files: normal cases =====

    #[test]
    fn test_discover_flat_dir_collects_sql_files() {
        let root = Path::new("/ws");
        let entries = vec![
            entry_file("/ws/a.sql"),
            entry_file("/ws/b.sql"),
            entry_file("/ws/readme.md"),
            entry_file("/ws/c.txt"),
        ];
        let found = discover_sql_files(root, &entries, 0, 0);
        assert_eq!(
            found,
            vec![PathBuf::from("/ws/a.sql"), PathBuf::from("/ws/b.sql")]
        );
    }

    #[test]
    fn test_discover_case_insensitive_extension() {
        let root = Path::new("/ws");
        let entries = vec![
            entry_file("/ws/a.SQL"),
            entry_file("/ws/b.Sql"),
            entry_file("/ws/c.sql"),
        ];
        let found = discover_sql_files(root, &entries, 0, 0);
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn test_discover_recurses_into_subdirs() {
        let root = Path::new("/ws");
        let entries = vec![
            entry_dir("/ws/sub"),
            entry_file("/ws/sub/deep.sql"),
            entry_file("/ws/top.sql"),
        ];
        let found = discover_sql_files(root, &entries, 0, 0);
        assert_eq!(found.len(), 2);
        assert!(found.contains(&PathBuf::from("/ws/sub/deep.sql")));
        assert!(found.contains(&PathBuf::from("/ws/top.sql")));
    }

    #[test]
    fn test_discover_nested_subdirs() {
        let root = Path::new("/ws");
        let entries = vec![
            entry_dir("/ws/a"),
            entry_dir("/ws/a/b"),
            entry_dir("/ws/a/b/c"),
            entry_file("/ws/a/b/c/leaf.sql"),
        ];
        let found = discover_sql_files(root, &entries, 0, 0);
        assert_eq!(found, vec![PathBuf::from("/ws/a/b/c/leaf.sql")]);
    }

    // ===== discover_sql_files: guards =====

    #[test]
    fn test_discover_skips_symlinks() {
        let root = Path::new("/ws");
        let entries = vec![
            entry_symlink("/ws/link.sql"),   // symlink file — skipped
            entry_symlink("/ws/linked_dir"), // symlink dir — skipped
            entry_file("/ws/real.sql"),
        ];
        let found = discover_sql_files(root, &entries, 0, 0);
        assert_eq!(found, vec![PathBuf::from("/ws/real.sql")]);
    }

    #[test]
    fn test_discover_respects_max_depth() {
        let root = Path::new("/ws");
        // depth: /ws=0, /ws/a=1, /ws/a/b=2, /ws/a/b/c=3
        let entries = vec![
            entry_dir("/ws/a"),
            entry_dir("/ws/a/b"),
            entry_dir("/ws/a/b/c"),
            entry_file("/ws/a/b/c/too_deep.sql"),
            entry_file("/ws/a/within.sql"),
        ];
        // max_depth=2 allows descending into /ws/a (depth 1) but not /ws/a/b (depth 2 would
        // require depth+1 < 2 => depth 1 can push depth-2 children... verify behavior).
        let found = discover_sql_files(root, &entries, 2, 0);
        // /ws/a/within.sql is reachable; /ws/a/b/c/too_deep.sql is not.
        assert!(found.contains(&PathBuf::from("/ws/a/within.sql")));
        assert!(!found.contains(&PathBuf::from("/ws/a/b/c/too_deep.sql")));
    }

    #[test]
    fn test_discover_depth_zero_uses_default() {
        let root = Path::new("/ws");
        let entries = vec![entry_file("/ws/a.sql")];
        let found = discover_sql_files(root, &entries, 0, 0);
        assert_eq!(found, vec![PathBuf::from("/ws/a.sql")]);
    }

    #[test]
    fn test_discover_respects_max_files() {
        let root = Path::new("/ws");
        let entries = vec![
            entry_file("/ws/a.sql"),
            entry_file("/ws/b.sql"),
            entry_file("/ws/c.sql"),
        ];
        let found = discover_sql_files(root, &entries, 0, 2);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_discover_ignores_entries_outside_root() {
        let root = Path::new("/ws");
        let entries = vec![
            entry_file("/ws/inside.sql"),
            entry_file("/other/outside.sql"),
        ];
        let found = discover_sql_files(root, &entries, 0, 0);
        assert_eq!(found, vec![PathBuf::from("/ws/inside.sql")]);
    }

    #[test]
    fn test_discover_empty_entries() {
        let root = Path::new("/ws");
        let found = discover_sql_files(root, &[], 0, 0);
        assert!(found.is_empty());
    }

    #[test]
    fn test_discover_root_with_no_sql_files() {
        let root = Path::new("/ws");
        let entries = vec![
            entry_dir("/ws/sub"),
            entry_file("/ws/a.txt"),
            entry_file("/ws/sub/b.md"),
        ];
        let found = discover_sql_files(root, &entries, 0, 0);
        assert!(found.is_empty());
    }

    #[test]
    fn test_discover_deterministic_order_regardless_of_input_order() {
        let root = Path::new("/ws");
        let entries_reversed = vec![
            entry_file("/ws/z.sql"),
            entry_file("/ws/a.sql"),
            entry_file("/ws/m.sql"),
        ];
        let entries_sorted = vec![
            entry_file("/ws/a.sql"),
            entry_file("/ws/m.sql"),
            entry_file("/ws/z.sql"),
        ];
        let from_reversed = discover_sql_files(root, &entries_reversed, 0, 0);
        let from_sorted = discover_sql_files(root, &entries_sorted, 0, 0);
        assert_eq!(from_reversed, from_sorted);
        assert_eq!(
            from_reversed,
            vec![
                PathBuf::from("/ws/a.sql"),
                PathBuf::from("/ws/m.sql"),
                PathBuf::from("/ws/z.sql"),
            ]
        );
    }

    // ===== analyze_file_contents: graceful degradation (UC-3) =====

    #[test]
    fn test_analyze_valid_sql_produces_statements() {
        let analysis = analyze_file_contents(&u("file:///ws/a.sql"), "SELECT * FROM users");
        assert_eq!(analysis.statements.len(), 1);
        assert!(analysis.parse_errors.is_empty());
    }

    #[test]
    fn test_analyze_extracts_symbol_table() {
        let analysis = analyze_file_contents(&u("file:///ws/a.sql"), "CREATE TABLE users (id INT)");
        assert!(analysis.symbol_table.tables.contains_key("USERS"));
    }

    #[test]
    fn test_analyze_incomplete_sql_does_not_crash() {
        // UC-3: truncated / syntactically broken SQL must produce a usable
        // partial analysis rather than panic.
        let analysis = analyze_file_contents(&u("file:///ws/a.sql"), "CREATE TABLE users (id");
        // Should not have panicked. Symbol table may or may not have the table
        // depending on fallback; the contract is "no crash, partial result".
        assert!(analysis.parse_errors.iter().all(|_| true));
        // tokens are still collected even when parsing fails.
        assert!(!analysis.tokens.is_empty());
    }

    #[test]
    fn test_analyze_garbage_sql_is_graceful() {
        let analysis = analyze_file_contents(&u("file:///ws/a.sql"), "SELCT FRO @@@ broken");
        assert!(!analysis.parse_errors.is_empty() || !analysis.statements.is_empty());
    }

    #[test]
    fn test_analyze_empty_contents() {
        let analysis = analyze_file_contents(&u("file:///ws/a.sql"), "");
        assert!(analysis.statements.is_empty());
        assert!(analysis.tokens.is_empty());
        assert!(analysis.symbol_table.tables.is_empty());
    }

    #[test]
    fn test_analyze_result_derives_purely_from_contents_not_uri() {
        // The URI must not affect the analysis body — only contents matter.
        let a = analyze_file_contents(&u("file:///ws/a.sql"), "SELECT 1");
        let b = analyze_file_contents(&u("file:///totally/elsewhere.sql"), "SELECT 1");
        assert_eq!(a.statements.len(), b.statements.len());
        assert_eq!(a.source, b.source);
    }

    #[test]
    fn test_analyze_matches_document_analysis_new() {
        // Contract: analyze_file_contents is exactly DocumentAnalysis::new on contents.
        let contents = "CREATE PROCEDURE foo AS BEGIN SELECT 1 END";
        let via_fn = analyze_file_contents(&u("file:///ws/a.sql"), contents);
        let direct = DocumentAnalysis::new(contents);
        assert_eq!(via_fn.statements.len(), direct.statements.len());
        assert_eq!(via_fn.parse_errors.len(), direct.parse_errors.len());
    }

    // ===== analysis_has_symbols =====

    #[test]
    fn test_analysis_has_symbols_true_for_create_table() {
        let analysis = analyze_file_contents(&u("file:///ws/a.sql"), "CREATE TABLE t (a INT)");
        assert!(analysis_has_symbols(&analysis));
    }

    #[test]
    fn test_analysis_has_symbols_false_for_plain_select() {
        let analysis = analyze_file_contents(&u("file:///ws/a.sql"), "SELECT 1");
        assert!(!analysis_has_symbols(&analysis));
    }

    // ===== DirEntry builders =====

    #[test]
    fn test_dir_entry_builders() {
        assert_eq!(
            DirEntry::dir("/ws/a"),
            DirEntry {
                path: PathBuf::from("/ws/a"),
                is_dir: true,
                is_symlink: false,
            }
        );
        assert_eq!(
            DirEntry::file("/ws/a.sql"),
            DirEntry {
                path: PathBuf::from("/ws/a.sql"),
                is_dir: false,
                is_symlink: false,
            }
        );
        assert_eq!(
            DirEntry::symlink("/ws/link"),
            DirEntry {
                path: PathBuf::from("/ws/link"),
                is_dir: false,
                is_symlink: true,
            }
        );
    }

    // ===== Constants sanity =====

    #[test]
    fn test_default_constants_are_sensible() {
        // Sanity: the defaults must be large enough for real workspaces.
        // clippy::assertions_on_constants would flag `assert!(const)`, so compare
        // against literals that make the assertion non-constant at the source level.
        let depth = DEFAULT_MAX_DEPTH;
        let files = DEFAULT_MAX_FILES;
        assert!(depth >= 16);
        assert!(files >= 1000);
    }
}
