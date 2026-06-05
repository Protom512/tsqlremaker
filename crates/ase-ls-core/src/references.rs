//! Find References provider
//!
//! カーソル位置のシンボルの全参照箇所を検索する。
//! - 変数: DECLARE + 全使用箇所
//! - テーブル: CREATE TABLE + SELECT/INSERT/UPDATE/DELETE内の参照
//! - プロシージャ: CREATE PROCEDURE + EXEC呼び出し
//! - ビュー: CREATE VIEW + SELECT内の参照

use crate::analysis::DocumentAnalysis;
use crate::token_matches_symbol;
use lsp_types::{Position, Range};
use tsql_token::TokenKind;

/// カーソル位置のシンボルの全参照箇所を検索する（DocumentAnalysis利用）
pub fn reference_ranges_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
    include_declaration: bool,
) -> Vec<Range> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    let (target_kind, target_text) = match analysis.find_token_at(offset) {
        Some((t, _)) => (t.kind, t.text.clone()),
        None => return Vec::new(),
    };

    let is_var = target_kind == TokenKind::LocalVar;

    let mut refs = Vec::new();

    for token in &analysis.tokens {
        if token_matches_symbol(token.kind, &token.text, &target_text, is_var) {
            let range = analysis
                .line_index
                .offset_to_range(token.span.start, token.span.end);

            let is_declaration = !include_declaration
                && is_definition_token(&analysis.source, token.span.start as usize, is_var);

            if include_declaration || !is_declaration {
                refs.push(range);
            }
        }
    }

    refs.dedup_by(|a, b| a.start == b.start && a.end == b.end);
    refs
}

/// Check if `haystack` ends with `suffix`, comparing ASCII characters case-insensitively.
#[inline]
fn ends_with_ignore_ascii_case(haystack: &str, suffix: &str) -> bool {
    if suffix.len() > haystack.len() {
        return false;
    }
    let haystack_bytes = haystack.as_bytes();
    let suffix_bytes = suffix.as_bytes();
    haystack_bytes[haystack.len() - suffix.len()..]
        .iter()
        .zip(suffix_bytes)
        .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// トークンが定義箇所かどうかを判定する
fn is_definition_token(source: &str, span_start: usize, is_var: bool) -> bool {
    let before = &source[..span_start];
    let trimmed = before.trim_end();

    if is_var {
        // 変数定義: DECLARE @var
        if ends_with_ignore_ascii_case(trimmed, "DECLARE") || trimmed.ends_with(',') {
            return true;
        }
    } else {
        // テーブル/プロシージャ/ビュー/インデックス/トリガー定義: CREATE [OBJECT] name
        if ends_with_ignore_ascii_case(trimmed, "CREATE TABLE")
            || ends_with_ignore_ascii_case(trimmed, "CREATE PROCEDURE")
            || ends_with_ignore_ascii_case(trimmed, "CREATE VIEW")
            || ends_with_ignore_ascii_case(trimmed, "CREATE INDEX")
            || ends_with_ignore_ascii_case(trimmed, "CREATE UNIQUE INDEX")
            || ends_with_ignore_ascii_case(trimmed, "CREATE TRIGGER")
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // --- reference_ranges_with_analysis tests ---

    #[test]
    fn test_references_with_analysis_variable() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "DECLARE @count INT\nSET @count = 1\nSELECT @count",
        );
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 5,
            },
            true,
        );
        assert_eq!(ranges.len(), 3);
    }

    #[test]
    fn test_references_with_analysis_table() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE users (id INT)\nSELECT * FROM users",
        );
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            },
            true,
        );
        assert!(ranges.len() >= 2);
    }

    #[test]
    fn test_references_with_analysis_empty_source() {
        let analysis = crate::analysis::DocumentAnalysis::new("");
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 0,
            },
            true,
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_references_with_analysis_no_token() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT  FROM t");
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 7,
            },
            true,
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_references_with_analysis_exclude_declaration() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE users (id INT)\nSELECT * FROM users",
        );
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            },
            false,
        );
        assert!(!ranges.is_empty());
        for range in &ranges {
            assert_ne!(range.start.line, 0, "Definition should be excluded");
        }
    }

    #[test]
    fn test_is_definition_unique_index() {
        // CREATE UNIQUE INDEX idx ON t(c) — idx should be recognized as definition
        assert!(is_definition_token(
            "CREATE TABLE t (c INT)\nCREATE UNIQUE INDEX idx ON t (c)",
            "CREATE TABLE t (c INT)\nCREATE UNIQUE INDEX ".len(),
            false,
        ));
    }

    #[test]
    fn test_is_definition_trigger() {
        // CREATE TRIGGER trg ... — trg should be recognized as definition
        assert!(is_definition_token(
            "CREATE TABLE t (c INT)\nCREATE TRIGGER trg ON t FOR INSERT AS BEGIN END",
            "CREATE TABLE t (c INT)\nCREATE TRIGGER ".len(),
            false,
        ));
    }

    #[test]
    fn test_is_definition_regular_index() {
        // CREATE INDEX idx — still recognized
        assert!(is_definition_token(
            "CREATE TABLE t (c INT)\nCREATE INDEX idx ON t (c)",
            "CREATE TABLE t (c INT)\nCREATE INDEX ".len(),
            false,
        ));
    }

    #[test]
    fn test_is_not_definition_select_reference() {
        // SELECT FROM users — users is NOT a definition
        assert!(!is_definition_token(
            "CREATE TABLE users (id INT)\nSELECT * FROM ",
            "CREATE TABLE users (id INT)\nSELECT * FROM ".len(),
            false,
        ));
    }

    #[test]
    fn test_is_definition_variable_in_declare() {
        // DECLARE @count — @count IS a definition
        assert!(is_definition_token(
            "DECLARE @count INT",
            "DECLARE ".len(),
            true
        ));
    }

    #[test]
    fn test_ends_with_ignore_ascii_case() {
        assert!(ends_with_ignore_ascii_case("CREATE TABLE", "TABLE"));
        assert!(ends_with_ignore_ascii_case("create table", "TABLE"));
        assert!(ends_with_ignore_ascii_case("CREATE table", "CREATE TABLE"));
        assert!(!ends_with_ignore_ascii_case("CREATE", "CREATE TABLE"));
        assert!(ends_with_ignore_ascii_case("DECLARE", "DECLARE"));
        assert!(!ends_with_ignore_ascii_case("DECLARE @x", "DECLARE"));
    }
}
