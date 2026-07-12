//! Code Lens (#117).
//!
//! Renders actionable annotations above symbol definitions. Currently emits a
//! "N references" lens above each `CREATE TABLE` / `CREATE PROCEDURE` /
//! `CREATE VIEW` definition name. Lenses are returned **unresolved** by
//! [`code_lenses`] (cheap — just the definition ranges) and resolved — the
//! reference count computed — by [`resolve_lens`], implementing the LSP
//! two-stage `codeLens` / `codeLens/resolve` pattern.
//!
//! ## Scope / limitations
//!
//! The reference count is **document-local** (via
//! [`reference_ranges_with_analysis`][crate::references::reference_ranges_with_analysis]).
//! Cross-file counting through the [`SymbolStore`][crate::symbol_store::SymbolStore]
//! is a future enhancement: it would require the resolve handler to take the
//! workspace document snapshot, breaking the pure-function testability of
//! [`resolve_lens`]. The document-local count is still meaningful for the
//! common case where a definition and its usages live in the same file.
//!
//! `"Run"` / `"Debug"` lenses (issue #117 use cases) need a client-side
//! command binding and are out of scope for this MVP.

use crate::analysis::DocumentAnalysis;
use crate::references::reference_ranges_with_analysis;
use lsp_types::{CodeLens, Command, Position, Range, Url};
use serde::{Deserialize, Serialize};

/// Payload stored in [`CodeLens::data`] so [`resolve_lens`] can re-identify the
/// target symbol. The `codeLens/resolve` request carries only the lens (no
/// `textDocument`), so the owning document URI must be embedded here.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LensData {
    /// URI of the document owning this lens (to fetch the analysis at resolve time).
    uri: String,
    /// Line of the definition name (the symbol whose references to count).
    line: u32,
    /// Character of the definition name start.
    character: u32,
}

/// Build unresolved code lenses for every `CREATE TABLE` / `CREATE PROCEDURE` /
/// `CREATE VIEW` definition in the document (#117).
///
/// Each lens spans the definition-name [`Range`] (drawn from the
/// [`SymbolTable`][crate::symbol_table::SymbolTable]) and carries [`LensData`]
/// pointing at `range.start` for deferred resolution by [`resolve_lens`].
/// The lenses have `command: None` (unresolved); the client will call
/// `codeLens/resolve` to fill in the "N references" title.
#[must_use]
pub fn code_lenses(analysis: &DocumentAnalysis, uri: &Url) -> Vec<CodeLens> {
    let st = &analysis.symbol_table;
    let mut lenses = Vec::new();
    for table in st.tables.values() {
        lenses.push(make_lens(table.range, uri));
    }
    for proc in st.procedures.values() {
        lenses.push(make_lens(proc.range, uri));
    }
    for view in st.views.values() {
        lenses.push(make_lens(view.range, uri));
    }
    lenses
}

/// Extract the owning document URI embedded in a lens's `data` field.
///
/// Used by the server's `codeLens/resolve` handler to fetch the analysis (the
/// resolve request itself carries no `textDocument`). Returns `None` when the
/// lens has no data or the payload is malformed.
#[must_use]
pub fn lens_uri(lens: &CodeLens) -> Option<String> {
    let data = lens.data.as_ref()?;
    serde_json::from_value::<LensData>(data.clone())
        .ok()
        .map(|d| d.uri)
}

/// Construct an unresolved lens at `range` whose [`LensData`] points at
/// `range.start` in `uri`.
fn make_lens(range: Range, uri: &Url) -> CodeLens {
    let data = LensData {
        uri: uri.to_string(),
        line: range.start.line,
        character: range.start.character,
    };
    CodeLens {
        range,
        command: None,
        data: Some(serde_json::to_value(data).unwrap_or_default()),
    }
}

/// Resolve a code lens: count the document-local references to the symbol and
/// attach a "N references" command title (#117).
///
/// Returns `None` if the lens carries no `data` or the payload is malformed
/// (the server then returns the lens unresolved rather than dropping it).
/// A symbol with zero usages resolves to `"0 references"`.
#[must_use]
pub fn resolve_lens(lens: CodeLens, analysis: &DocumentAnalysis) -> Option<CodeLens> {
    let data = lens.data.as_ref()?;
    let data: LensData = serde_json::from_value(data.clone()).ok()?;
    let position = Position {
        line: data.line,
        character: data.character,
    };
    // include_declaration = false → count usages only (exclude the CREATE line).
    let count = reference_ranges_with_analysis(analysis, position, false).len();
    let title = if count == 1 {
        "1 reference".to_string()
    } else {
        format!("{count} references")
    };
    Some(CodeLens {
        range: lens.range,
        command: Some(Command {
            title,
            // No client-side command binding yet; the lens is informational
            // (the title text is what the editor displays above the symbol).
            command: String::new(),
            arguments: None,
        }),
        data: lens.data,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::analysis::DocumentAnalysis;
    use lsp_types::{Position, Range};

    fn uri() -> Url {
        Url::parse("file:///test.sql").unwrap()
    }

    #[test]
    fn code_lenses_one_per_table_proc_view_definition() {
        let src = "CREATE TABLE t1 (id INT)\n\
                   CREATE VIEW v1 AS SELECT * FROM t1\n\
                   CREATE PROC p1 AS SELECT * FROM t1";
        let analysis = DocumentAnalysis::new(src);
        let lenses = code_lenses(&analysis, &uri());
        // 1 table + 1 view + 1 procedure = 3 lenses.
        assert_eq!(lenses.len(), 3, "one lens per definition: {lenses:?}");
        // All lenses start unresolved (no command yet).
        assert!(lenses.iter().all(|l| l.command.is_none()));
        // All lenses carry data for resolution.
        assert!(lenses.iter().all(|l| l.data.is_some()));
    }

    #[test]
    fn code_lenses_empty_when_no_definitions() {
        let analysis = DocumentAnalysis::new("SELECT 1");
        let lenses = code_lenses(&analysis, &uri());
        assert!(lenses.is_empty(), "no definitions → no lenses");
    }

    #[test]
    fn resolve_lens_counts_document_local_usages() {
        // t is defined once and used in SELECT + INSERT (2 usages; the
        // CREATE TABLE line is excluded because include_declaration = false).
        let src = "CREATE TABLE t (id INT)\n\
                   SELECT * FROM t\n\
                   INSERT INTO t VALUES (1)";
        let analysis = DocumentAnalysis::new(src);
        let lens = code_lenses(&analysis, &uri())
            .into_iter()
            .next()
            .expect("one table lens");
        let resolved = resolve_lens(lens, &analysis).expect("resolves");
        let command = resolved.command.expect("command set after resolve");
        assert!(
            command.title.contains("2 references"),
            "expected 2 references, got: {}",
            command.title
        );
    }

    #[test]
    fn resolve_lens_singular_for_one_usage() {
        let src = "CREATE TABLE t (id INT)\nSELECT * FROM t";
        let analysis = DocumentAnalysis::new(src);
        let lens = code_lenses(&analysis, &uri())
            .into_iter()
            .next()
            .expect("one table lens");
        let resolved = resolve_lens(lens, &analysis).expect("resolves");
        assert_eq!(
            resolved.command.as_ref().unwrap().title,
            "1 reference",
            "singular form for a single usage"
        );
    }

    #[test]
    fn resolve_lens_zero_usages_shows_zero() {
        // Table defined but never used → "0 references".
        let src = "CREATE TABLE lonely (id INT)";
        let analysis = DocumentAnalysis::new(src);
        let lens = code_lenses(&analysis, &uri())
            .into_iter()
            .next()
            .expect("one table lens");
        let resolved = resolve_lens(lens, &analysis).expect("resolves");
        assert_eq!(resolved.command.as_ref().unwrap().title, "0 references");
    }

    #[test]
    fn resolve_lens_returns_none_without_data() {
        let analysis = DocumentAnalysis::new("CREATE TABLE t (id INT)");
        let lens = CodeLens {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 1,
                },
            },
            command: None,
            data: None,
        };
        assert!(
            resolve_lens(lens, &analysis).is_none(),
            "a lens with no data cannot be resolved"
        );
    }

    #[test]
    fn lens_uri_extracts_embedded_uri() {
        let analysis = DocumentAnalysis::new("CREATE TABLE t (id INT)");
        let lens = code_lenses(&analysis, &uri()).into_iter().next().unwrap();
        assert_eq!(lens_uri(&lens).as_deref(), Some("file:///test.sql"));
    }

    #[test]
    fn lens_uri_none_for_missing_data() {
        let lens = CodeLens {
            range: Range::default(),
            command: None,
            data: None,
        };
        assert!(lens_uri(&lens).is_none());
    }
}
