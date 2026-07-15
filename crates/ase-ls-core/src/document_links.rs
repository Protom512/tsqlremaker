//! Document Links (#119) — SQLCMD `:r` file-include directives.
//!
//! Surfaces clickable links over SQLCMD `:r <path>` include directives so the
//! user can `Ctrl+click` straight into the included script. Implements the LSP
//! two-stage `textDocument/documentLink` + `documentLink/resolve` pattern.
//!
//! ## Detection strategy (token-scan)
//!
//! The T-SQL lexer has **no SQLCMD / line-start awareness**: it emits a plain
//! [`TokenKind::Colon`] for every `:` (verified empirically at `lexer.rs:953`).
//! The parser has no `:r` support at all. Statement-walking therefore produces
//! nothing, so a token-scan is the only viable approach.
//!
//! A directive is recognised when a `Colon` token is immediately followed by an
//! `Ident` whose text equals `r`/`R` (case-insensitive) **and** the colon sits
//! at the start of its line (only ASCII whitespace before it on that line). The
//! line-start guard matches real `sqlcmd` semantics (where `:r` is only
//! meaningful as the first non-blank token of a line) and prevents a stray `:r`
//! mid-statement from being treated as an include. The lexer cannot tell us the
//! line position, so the scanner consults the source bytes directly.
//!
//! ## Path extraction
//!
//! The path is the trimmed remainder of the source line after the `r` token. A
//! quoted argument (`:r "path"` / `:r 'path'`) is unquoted. Windows backslashes
//! (`C:\dir\sub.sql`) are normalised to forward slashes before [`Url::join`].
//!
//! ## Base-path resolution (document-relative, `..` overflow drops the link)
//!
//! `:r` paths are resolved **document-relative**: the path joins against the
//! owning document URI's directory. Windows backslashes are normalised to
//! forward slashes before the join. A `..` segment that would escape above the
//! document's own directory **drops the link** — sqlcmd does not escape the
//! script's directory, and a stray `..` is far more likely a typo than an
//! intent, so no link is emitted rather than a surprising ancestor target.
//!
//! ## Two-stage resolve
//!
//! [`document_links`] emits links with `target` already populated whenever the
//! path resolves cleanly, and stashes [`LinkData`] (owning URI + raw path) in
//! `link.data` regardless. The `documentLink/resolve` request carries only the
//! link (no `textDocument`), so the owning document URI must be embedded in
//! `data`; [`resolve_document_link`] re-establishes the target from that
//! payload. `resolve_provider: Some(true)` is advertised because the resolve
//! request omits the base URI — the server cannot recompute the target without
//! the stashed payload.
//!
//! ## Scope / limitations
//!
//! Only `:r` is supported (not `:setvar` / other SQLCMD commands). No
//! workspace-relative fallback: a path that cannot be resolved against the
//! document directory is dropped (true workspace-root-relative resolution would
//! require the workspace folder set, breaking the pure-function contract).

use crate::analysis::{DocumentAnalysis, OwnedToken};
use crate::config::DocumentLinkConfig;
use crate::line_index::LineIndex;
use lsp_types::{DocumentLink, Range, Url};
use serde::{Deserialize, Serialize};
use tsql_token::TokenKind;

/// Payload stashed in [`DocumentLink::data`] so [`resolve_document_link`] can
/// re-establish the target URI. The `documentLink/resolve` request carries only
/// the link (no `textDocument`), so the owning document URI (base for
/// relative-path resolution) and the raw path text must be embedded here.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LinkData {
    /// URI of the document owning this link (base for relative-path resolution).
    #[allow(dead_code)]
    uri: String,
    /// The raw path as written in the directive (quotes stripped), e.g.
    /// `scripts/init.sql` or `sub\file.sql`.
    raw_path: String,
}

/// Build document links for every SQLCMD `:r` directive in the document (#119).
///
/// Token-scans [`DocumentAnalysis::tokens`] for line-start `Colon` +
/// `Ident(r/R)` + path, resolves each path document-relative against `base_uri`,
/// and returns one [`DocumentLink`] per directive. Links whose path escapes the
/// document directory via `..` are dropped. The whole family is gated by
/// [`DocumentLinkConfig::enable`] (returns an empty vec when disabled).
///
/// Each emitted link spans the directive range (`:r` colon through the end of
/// the path argument) and carries [`LinkData`] in `data` for deferred
/// resolution by [`resolve_document_link`].
///
/// # Panics
///
/// Never. Broken spans, missing paths, and unresolvable URIs are silently
/// skipped.
#[must_use]
pub fn document_links(
    analysis: &DocumentAnalysis,
    base_uri: &Url,
    config: &DocumentLinkConfig,
) -> Vec<DocumentLink> {
    if !config.enable {
        return Vec::new();
    }
    let source = &analysis.source;
    let line_index = &analysis.line_index;
    let tokens = &analysis.tokens;
    let mut links = Vec::new();

    let mut i = 0;
    while i < tokens.len() {
        if !is_directive_head(tokens, i) {
            i += 1;
            continue;
        }
        let colon_tok = &tokens[i];
        let r_tok = &tokens[i + 1];
        // Broken-span guard: the colon and the r-ident must each have a valid
        // span (start < end), matching the inlay_hints.rs:105 guard pattern.
        // The parser/lexer sometimes leaves multi-line tokens with span.end = 0.
        if colon_tok.span.start >= colon_tok.span.end || r_tok.span.start >= r_tok.span.end {
            i += 1;
            continue;
        }
        // Line-start guard: only ASCII whitespace before the colon on its line.
        if !is_at_line_start(source, colon_tok.span.start as usize) {
            i += 1;
            continue;
        }
        let colon_start = colon_tok.span.start;
        if let Some((range, raw_path)) = extract_path(source, r_tok, colon_start, line_index) {
            if let Some(target) = resolve_target(base_uri, &raw_path) {
                let data = LinkData {
                    uri: base_uri.to_string(),
                    raw_path,
                };
                links.push(DocumentLink {
                    range,
                    target: Some(target),
                    tooltip: None,
                    data: serde_json::to_value(data).ok(),
                });
            }
        }
        // Advance past the r-ident regardless of whether a link was emitted.
        i += 2;
    }

    links
}

/// Resolve a document link: recover / re-establish the target URI from the
/// stashed [`LinkData`] payload (#119).
///
/// `document_links` already populates `target` when the path resolves cleanly;
/// this function guarantees the target is present by recomputing it from the
/// stashed `uri` + `raw_path`. Returns `None` when the link carries no `data`,
/// the payload is malformed, or the path no longer resolves (e.g. `..`
/// overflow) — the server then drops the link rather than emitting a dangling
/// target.
#[must_use]
pub fn resolve_document_link(
    link: &DocumentLink,
    _analysis: &DocumentAnalysis,
) -> Option<DocumentLink> {
    let data = link.data.as_ref()?;
    let payload: LinkData = serde_json::from_value(data.clone()).ok()?;
    let target = resolve_path(&payload.uri, &payload.raw_path)?;
    Some(DocumentLink {
        range: link.range,
        target: Some(target),
        tooltip: link.tooltip.clone(),
        data: link.data.clone(),
    })
}

/// Extract the owning document URI embedded in a link's `data` field.
///
/// Used by the server's `documentLink/resolve` handler to fetch the analysis
/// (the resolve request itself carries no `textDocument`). Returns `None` when
/// the link has no data or the payload is malformed — mirrors
/// [`code_lens::lens_uri`][crate::code_lens::lens_uri].
#[must_use]
pub fn link_uri(link: &DocumentLink) -> Option<String> {
    let data = link.data.as_ref()?;
    serde_json::from_value::<LinkData>(data.clone())
        .ok()
        .map(|d| d.uri)
}

///
/// Normalises backslashes to forward slashes, joins the path against the
/// document's directory, and rejects results that escape above the document
/// directory via `..`. Returns `None` on overflow or a malformed URI/path.
fn resolve_path(base_uri_str: &str, raw_path: &str) -> Option<Url> {
    let base = Url::parse(base_uri_str).ok()?;
    resolve_target(&base, raw_path)
}

/// Resolve a `:r` path argument against the document URI.
///
/// Backslashes are normalised to forward slashes before the join. Absolute
/// paths (leading `/`) are rejected — sqlcmd treats `:r` paths as relative.
/// A `..` segment that escapes above the document's own directory drops the
/// link (returns `None`). Returns the resolved [`Url`] on success.
fn resolve_target(base: &Url, raw_path: &str) -> Option<Url> {
    let normalised: String = raw_path.replace('\\', "/");
    if normalised.starts_with('/') {
        return None;
    }
    let dir = base.join(".").ok()?;
    let resolved = dir.join(&normalised).ok()?;
    if !is_within_dir(&resolved, &dir) {
        return None;
    }
    Some(resolved)
}

/// Return `true` when `resolved` is `dir` itself or a descendant of `dir`.
///
/// Guards against `..` overflow: [`Url::join`] happily resolves
/// `../../etc/passwd` to an ancestor, so we compare the resolved path against
/// the directory path.
fn is_within_dir(resolved: &Url, dir: &Url) -> bool {
    let resolved_path = resolved.path();
    let dir_path = dir.path();
    if resolved_path == dir_path {
        return true;
    }
    // dir_path from `join(".")` ends in '/', but normalise defensively.
    let dir_prefix = if dir_path.ends_with('/') {
        dir_path.to_string()
    } else {
        format!("{dir_path}/")
    };
    resolved_path.starts_with(&dir_prefix)
}

/// Returns `true` iff `tokens[i]` is a `Colon` immediately followed by an
/// `Ident` whose text equals `r`/`R` (case-insensitive).
fn is_directive_head(tokens: &[OwnedToken], i: usize) -> bool {
    if tokens[i].kind != TokenKind::Colon {
        return false;
    }
    let Some(r_tok) = tokens.get(i + 1) else {
        return false;
    };
    r_tok.kind == TokenKind::Ident && r_tok.text.eq_ignore_ascii_case("r")
}

/// Returns `true` iff only ASCII whitespace precedes `colon_offset` on its line.
fn is_at_line_start(source: &str, colon_offset: usize) -> bool {
    let bytes = source.as_bytes();
    let mut j = colon_offset;
    while j > 0 {
        j -= 1;
        match bytes[j] {
            b' ' | b'\t' => continue,
            b'\n' | b'\r' => break,
            _ => return false,
        }
    }
    true
}

/// Extract the path argument and the full directive range from the source.
///
/// The path is the trimmed remainder of the line after the `r` token; a quoted
/// argument (single or double quotes) is unquoted. Returns `None` when the
/// remainder is empty (a bare `:r` with nothing after it).
fn extract_path(
    source: &str,
    r_tok: &OwnedToken,
    colon_start: u32,
    line_index: &LineIndex,
) -> Option<(Range, String)> {
    let bytes = source.as_bytes();
    let after_r = r_tok.span.end as usize;
    let line_end = line_end_from(bytes, after_r);
    let raw = source.get(after_r..line_end)?;
    let trimmed = raw.trim_matches(|c: char| c == ' ' || c == '\t' || c == '\r');
    if trimmed.is_empty() {
        return None;
    }
    // Offset of `trimmed` within the full source = after_r + its offset within
    // `raw` (both slices share the same backing buffer, so pointer distance is
    // the byte offset).
    let trim_offset_in_raw = trimmed.as_ptr() as usize - raw.as_ptr() as usize;
    let path_end = after_r + trim_offset_in_raw + trimmed.len();
    let range = line_index.offset_to_range(colon_start, path_end as u32);
    let path = strip_surrounding_quotes(trimmed).to_string();
    Some((range, path))
}

/// Strip one layer of matching surrounding quotes (`'...'` or `"..."`) from
/// `s`. Returns `s` unchanged when it is not quoted or the quotes are
/// unbalanced.
fn strip_surrounding_quotes(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
            // Safe: the quote chars are ASCII (1 byte each).
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Find the offset of the line terminator at or after `from` (or end of source).
fn line_end_from(bytes: &[u8], from: usize) -> usize {
    let mut j = from;
    while j < bytes.len() {
        match bytes[j] {
            b'\n' | b'\r' => return j,
            _ => j += 1,
        }
    }
    bytes.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::analysis::DocumentAnalysis;
    use crate::config::DocumentLinkConfig;
    use lsp_types::{Position, Range, Url};

    fn base_uri() -> Url {
        Url::parse("file:///home/user/scripts/main.sql").unwrap()
    }

    fn enabled() -> DocumentLinkConfig {
        DocumentLinkConfig { enable: true }
    }

    fn disabled() -> DocumentLinkConfig {
        DocumentLinkConfig { enable: false }
    }

    /// Pull the target URI out of a link as a plain string (panics if absent).
    fn target_str(link: &DocumentLink) -> String {
        link.target.as_ref().unwrap().to_string()
    }

    // (1) Normal unquoted `:r scripts/init.sql` → one link, target joined to
    //     the document directory, data stashed.
    #[test]
    fn normal_unquoted_r_directive_emits_resolved_link() {
        let src = ":r init.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert_eq!(links.len(), 1, "one :r directive → one link: {links:?}");
        assert_eq!(target_str(&links[0]), "file:///home/user/scripts/init.sql");
        assert!(links[0].data.is_some(), "data stashed for resolve");
    }

    // (2) Subpath joined document-relatively.
    #[test]
    fn subpath_joined_document_relatively() {
        let src = ":r sub/deep/seed.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert_eq!(links.len(), 1);
        assert_eq!(
            target_str(&links[0]),
            "file:///home/user/scripts/sub/deep/seed.sql"
        );
    }

    // (3) Double-quoted path — quotes stripped before resolution.
    #[test]
    fn double_quoted_path_strips_quotes() {
        let src = ":r \"other.sql\"";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert_eq!(links.len(), 1, "double-quoted link: {links:?}");
        assert_eq!(target_str(&links[0]), "file:///home/user/scripts/other.sql");
    }

    // (4) Single-quoted path — quotes stripped.
    #[test]
    fn single_quoted_path_strips_quotes() {
        let src = ":r 'other.sql'";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert_eq!(links.len(), 1, "single-quoted link: {links:?}");
        assert_eq!(target_str(&links[0]), "file:///home/user/scripts/other.sql");
    }

    // (5) Quoted backslash path → normalised to forward slash.
    #[test]
    fn quoted_backslash_path_normalised_to_forward_slash() {
        let src = ":r 'sub\\file.sql'";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert_eq!(links.len(), 1, "backslash link: {links:?}");
        assert_eq!(
            target_str(&links[0]),
            "file:///home/user/scripts/sub/file.sql"
        );
    }

    // (6) Disabled-by-config → no links even when directives are present.
    #[test]
    fn disabled_config_emits_no_links() {
        let src = ":r init.sql\n:r other.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &disabled());
        assert!(links.is_empty(), "config gate must suppress all links");
    }

    // (7) Empty path (`:r` with nothing after it) → no link.
    #[test]
    fn empty_path_emits_no_link() {
        let src = ":r";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert!(links.is_empty(), "bare `:r` → no link: {links:?}");

        // `:r` followed immediately by a newline is also empty.
        let src2 = ":r\nSELECT 1";
        let analysis2 = DocumentAnalysis::new(src2);
        assert!(document_links(&analysis2, &base_uri(), &enabled()).is_empty());
    }

    // (8) `..` overflow escapes the document directory → link dropped.
    #[test]
    fn dotdot_overflow_drops_link() {
        // Document dir is /home/user/scripts; ../../etc/x escapes above it.
        let src = ":r ../../etc/passwd";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert!(
            links.is_empty(),
            "`..` overflow must drop the link: {links:?}"
        );
    }

    // (9) A single `..` still escapes the document's own directory → dropped.
    #[test]
    fn single_dotdot_above_doc_dir_drops_link() {
        let src = ":r ../sibling.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert!(
            links.is_empty(),
            "`..` must not escape the document directory: {links:?}"
        );
    }

    // (10) `:r` not at line start (preceded by non-whitespace) → not a
    //      directive, no link.
    #[test]
    fn r_not_at_line_start_is_not_a_directive() {
        let src = "SELECT x :r nope.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert!(links.is_empty(), "mid-line `:r` is not sqlcmd: {links:?}");
    }

    // (11) Leading whitespace before `:r` is allowed (still line start).
    #[test]
    fn indented_r_is_still_line_start() {
        let src = "  :r init.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert_eq!(links.len(), 1, "indented `:r` is still a directive");
        assert_eq!(target_str(&links[0]), "file:///home/user/scripts/init.sql");
    }

    // (12) Uppercase `:R` is recognised case-insensitively.
    #[test]
    fn uppercase_r_directive_recognised() {
        let src = ":R init.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert_eq!(links.len(), 1, "`:R` (uppercase) recognised: {links:?}");
    }

    // (13) Multiple directives across lines → multiple links.
    #[test]
    fn multiple_directives_emit_multiple_links() {
        let src = ":r a.sql\n:r b.sql\n:r c.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert_eq!(links.len(), 3, "one link per directive: {links:?}");
        assert!(links
            .iter()
            .all(|l| target_str(l).starts_with("file:///home/user/scripts/")));
    }

    // (14) Resolve round-trip: target survives a resolve pass.
    #[test]
    fn resolve_round_trip_recovers_target() {
        let src = ":r init.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        let link = &links[0];
        let resolved = resolve_document_link(link, &analysis).expect("resolves");
        assert_eq!(target_str(&resolved), "file:///home/user/scripts/init.sql");
    }

    // (15) URI recovery: a link whose target was stripped by a round-trip still
    //      resolves from the stashed payload.
    #[test]
    fn resolve_recovers_target_from_data_when_target_absent() {
        let src = ":r init.sql";
        let analysis = DocumentAnalysis::new(src);
        let mut links = document_links(&analysis, &base_uri(), &enabled());
        let mut link = links.pop().unwrap();
        // Simulate a client/server round-trip that drops the target.
        link.target = None;
        let resolved = resolve_document_link(&link, &analysis).expect("resolves from data");
        assert_eq!(target_str(&resolved), "file:///home/user/scripts/init.sql");
    }

    // (16) resolve returns None when data is missing.
    #[test]
    fn resolve_returns_none_without_data() {
        let analysis = DocumentAnalysis::new(":r init.sql");
        let link = DocumentLink {
            range: Range::default(),
            target: None,
            tooltip: None,
            data: None,
        };
        assert!(resolve_document_link(&link, &analysis).is_none());
    }

    // (17) No directives in a normal SQL document → empty.
    #[test]
    fn normal_sql_document_has_no_links() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users WHERE id = 1");
        assert!(document_links(&analysis, &base_uri(), &enabled()).is_empty());
    }

    // (18) `:setvar` is not matched (only `:r` is supported).
    #[test]
    fn setvar_directive_is_not_matched() {
        let src = ":setvar X 1";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        assert!(links.is_empty(), ":setvar must not emit a link");
    }

    // (19) Link range covers the colon through the end of the path argument.
    #[test]
    fn link_range_covers_colon_through_end_of_path() {
        let src = ":r init.sql";
        let analysis = DocumentAnalysis::new(src);
        let links = document_links(&analysis, &base_uri(), &enabled());
        let link = &links[0];
        assert_eq!(
            link.range.start,
            Position {
                line: 0,
                character: 0
            }
        );
        // ":r init.sql" is 11 chars → end character 11.
        assert_eq!(
            link.range.end,
            Position {
                line: 0,
                character: 11
            }
        );
    }

    // (20) strip_surrounding_quotes helper — straight + no-quotes cases.
    #[test]
    fn strip_surrounding_quotes_handles_all_forms() {
        assert_eq!(strip_surrounding_quotes("'abc'"), "abc");
        assert_eq!(strip_surrounding_quotes("\"abc\""), "abc");
        assert_eq!(strip_surrounding_quotes("abc"), "abc");
        assert_eq!(strip_surrounding_quotes("'a"), "'a"); // unbalanced → unchanged
        assert_eq!(strip_surrounding_quotes(""), "");
    }
}
