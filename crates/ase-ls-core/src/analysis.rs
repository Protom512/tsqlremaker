//! Document Analysis — pre-computed derived data for a source document.
//!
//! Constructs all derived data (LineIndex, tokens, AST, symbol table) once
//! per source change, so LSP handlers can share the results without re-parsing.

use crate::line_index::LineIndex;
use crate::symbol_table::{SymbolTable, SymbolTableBuilder};
use tsql_token::{Span, TokenKind};

/// Owned copy of a lexer token, without lifetime dependency on source.
#[derive(Debug, Clone)]
pub struct OwnedToken {
    /// Token kind (keyword, identifier, operator, etc.)
    pub kind: TokenKind,
    /// Token text (cloned from source).
    pub text: String,
    /// Byte span in source.
    pub span: Span,
}

impl Clone for DocumentAnalysis {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            line_index: LineIndex::new(&self.source),
            tokens: self.tokens.clone(),
            statements: self.statements.clone(),
            parse_errors: self.parse_errors.clone(),
            symbol_table: self.symbol_table.clone(),
        }
    }
}

/// Pre-computed analysis of a source document.
///
/// Built once per `did_open`/`did_change`, shared by all LSP handlers.
pub struct DocumentAnalysis {
    /// Original source text (needed for position_to_offset and formatting).
    pub source: String,
    /// Pre-computed line offset index for O(log n) position conversion.
    pub line_index: LineIndex,
    /// All tokens from lexer (owned copies).
    pub tokens: Vec<OwnedToken>,
    /// Parsed AST statements.
    pub statements: Vec<tsql_parser::ast::Statement>,
    /// Parse errors (if any).
    pub parse_errors: Vec<tsql_parser::ParseError>,
    /// Extracted symbol table.
    pub symbol_table: SymbolTable,
}

impl DocumentAnalysis {
    /// Build a full analysis from source text.
    pub fn new(source: &str) -> Self {
        let owned_source = source.to_string();
        let line_index = LineIndex::new(source);

        let tokens: Vec<OwnedToken> = tsql_lexer::Lexer::new(source)
            .filter_map(|r| r.ok())
            .map(|t| OwnedToken {
                kind: t.kind,
                text: t.text.to_string(),
                span: t.span,
            })
            .collect();

        let (statements, parse_errors) = match tsql_parser::Parser::new(source).parse_with_errors()
        {
            Ok((stmts, errs)) => (stmts, errs),
            Err(errs) => (Vec::new(), errs.errors),
        };

        let symbol_table = SymbolTableBuilder::build_tolerant(source);
        let symbol_table = if symbol_table.tables.is_empty()
            && source.to_ascii_uppercase().contains("CREATE TABLE")
        {
            // Fallback: parse progressively shorter substrings to extract DDL definitions
            // from partially valid sources (e.g., incomplete INSERT after CREATE TABLE)
            let lines: Vec<&str> = source.lines().collect();
            let mut best = symbol_table;
            for cut in (1..lines.len()).rev() {
                let partial: String = lines[..cut].join("\n");
                let partial_table = SymbolTableBuilder::build_tolerant(&partial);
                if !partial_table.tables.is_empty() {
                    best = partial_table;
                    break;
                }
            }
            best
        } else {
            symbol_table
        };

        Self {
            source: owned_source,
            line_index,
            tokens,
            statements,
            parse_errors,
            symbol_table,
        }
    }

    /// Find the token at a given byte offset using binary search. O(log n).
    pub fn find_token_at(&self, offset: usize) -> Option<(&OwnedToken, usize)> {
        let idx = self
            .tokens
            .partition_point(|t| t.span.start as usize <= offset);
        if idx == 0 {
            return None;
        }
        let token = &self.tokens[idx - 1];
        let end = token.span.end as usize;
        if offset < end {
            Some((token, idx - 1))
        } else {
            None
        }
    }

    /// Get the text of a specific line. O(1) line lookup via LineIndex.
    pub fn get_line(&self, line: u32) -> &str {
        let line_count = self.line_index.line_count();
        let line = line as usize;
        if line >= line_count {
            return "";
        }
        let start = self.line_index.line_offset(line);
        let end = if line + 1 < line_count {
            self.line_index.line_offset(line + 1)
        } else {
            self.source.len()
        };
        let line_text = &self.source[start..end];
        line_text.trim_end_matches('\n').trim_end_matches('\r')
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_builds_line_index() {
        let analysis = DocumentAnalysis::new("SELECT *\nFROM users");
        assert_eq!(analysis.line_index.offset_to_position(0), (0, 0));
        assert_eq!(analysis.line_index.offset_to_position(9), (1, 0)); // after \n
    }

    #[test]
    fn test_analysis_collects_tokens() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        assert!(!analysis.tokens.is_empty());
        // First token should be SELECT
        assert_eq!(analysis.tokens[0].kind, TokenKind::Select);
        assert_eq!(analysis.tokens[0].text, "SELECT");
    }

    #[test]
    fn test_analysis_parses_statements() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        assert_eq!(analysis.statements.len(), 1);
    }

    #[test]
    fn test_analysis_collects_parse_errors() {
        let analysis = DocumentAnalysis::new("SELCT * FROM");
        assert!(!analysis.parse_errors.is_empty());
    }

    #[test]
    fn test_analysis_builds_symbol_table() {
        let source = "CREATE TABLE users (id INT)";
        let analysis = DocumentAnalysis::new(source);
        assert!(analysis.symbol_table.tables.contains_key("USERS"));
    }

    #[test]
    fn test_analysis_empty_source() {
        let analysis = DocumentAnalysis::new("");
        assert!(analysis.tokens.is_empty());
        assert!(analysis.statements.is_empty());
        assert!(analysis.parse_errors.is_empty());
        assert!(analysis.symbol_table.tables.is_empty());
    }

    #[test]
    fn test_analysis_invalid_sql_partial_results() {
        // Invalid SQL should still produce tokens and capture errors
        let analysis = DocumentAnalysis::new("SELCT FRO users");
        assert!(!analysis.tokens.is_empty());
        assert!(!analysis.parse_errors.is_empty());
    }

    #[test]
    fn test_find_token_at_start() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        let (token, _) = analysis.find_token_at(0).unwrap();
        assert_eq!(token.kind, TokenKind::Select);
        assert_eq!(token.text, "SELECT");
    }

    #[test]
    fn test_find_token_at_mid() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        let (token, _) = analysis.find_token_at(3).unwrap();
        assert_eq!(token.text, "SELECT");
    }

    #[test]
    fn test_find_token_at_whitespace() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        // offset 6 is space after SELECT
        assert!(analysis.find_token_at(6).is_none());
    }

    #[test]
    fn test_find_token_at_past_end() {
        let analysis = DocumentAnalysis::new("SELECT");
        assert!(analysis.find_token_at(100).is_none());
    }

    #[test]
    fn test_find_token_at_variable() {
        let analysis = DocumentAnalysis::new("DECLARE @count INT");
        let (token, _) = analysis.find_token_at(8).unwrap();
        assert_eq!(token.kind, TokenKind::LocalVar);
        assert_eq!(token.text, "@count");
    }

    // --- get_line() tests (Phase 2-C: O(1) line lookup) ---

    #[test]
    fn test_get_line_single_line() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        assert_eq!(analysis.get_line(0), "SELECT * FROM users");
    }

    #[test]
    fn test_get_line_multi_line() {
        let analysis = DocumentAnalysis::new("SELECT *\nFROM users\nWHERE id = 1");
        assert_eq!(analysis.get_line(0), "SELECT *");
        assert_eq!(analysis.get_line(1), "FROM users");
        assert_eq!(analysis.get_line(2), "WHERE id = 1");
    }

    #[test]
    fn test_get_line_out_of_range_returns_empty() {
        let analysis = DocumentAnalysis::new("SELECT *");
        assert_eq!(analysis.get_line(5), "");
    }

    #[test]
    fn test_get_line_empty_source() {
        let analysis = DocumentAnalysis::new("");
        assert_eq!(analysis.get_line(0), "");
    }

    #[test]
    fn test_get_line_trailing_newline() {
        let analysis = DocumentAnalysis::new("line1\nline2\n");
        assert_eq!(analysis.get_line(0), "line1");
        assert_eq!(analysis.get_line(1), "line2");
        // trailing newline creates an empty line 2
        assert_eq!(analysis.get_line(2), "");
    }

    #[test]
    fn test_get_line_crlf() {
        let analysis = DocumentAnalysis::new("line1\r\nline2");
        assert_eq!(analysis.get_line(0), "line1");
        assert_eq!(analysis.get_line(1), "line2");
    }
}
