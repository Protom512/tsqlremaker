//! Document Analysis — pre-computed derived data for a source document.
//!
//! Constructs all derived data (LineIndex, tokens, AST, symbol table) once
//! per source change, so LSP handlers can share the results without re-parsing.

use crate::line_index::LineIndex;
use crate::symbol_table::{SymbolTable, SymbolTableBuilder};
use std::sync::Arc;
use tsql_token::{Span, TokenKind};

/// Owned copy of a lexer token, without lifetime dependency on source.
#[derive(Debug, Clone)]
pub struct OwnedToken {
    /// Token kind (keyword, identifier, operator, etc.)
    pub kind: TokenKind,
    /// Token text (Arc<str> for O(1) clone on hot-path handler access).
    pub text: Arc<str>,
    /// Byte span in source.
    pub span: Span,
}

/// Pre-computed analysis of a source document.
///
/// Built once per `did_open`/`did_change`, shared by all LSP handlers.
#[derive(Debug, Clone)]
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
        let source = source.to_string();
        let line_index = LineIndex::new(&source);

        let tokens: Vec<OwnedToken> = tsql_lexer::Lexer::new(&source)
            .filter_map(|r| r.ok())
            .map(|t| OwnedToken {
                kind: t.kind,
                text: Arc::from(t.text),
                span: t.span,
            })
            .collect();

        let (statements, parse_errors) = tsql_parser::Parser::new(&source).parse_with_errors();

        let symbol_table = SymbolTableBuilder::build_tolerant(&source);
        let symbol_table = if symbol_table.tables.is_empty()
            && source
                .as_bytes()
                .windows(b"CREATE TABLE".len())
                .any(|w| w.eq_ignore_ascii_case(b"CREATE TABLE"))
        {
            // Fallback: scan tokens to find the CREATE TABLE definition boundary,
            // then parse just that portion. O(n) token scan + single parse attempt.
            let table_end = find_create_table_end(&tokens);
            if let Some(end) = table_end {
                if let Some(partial) = source.get(..end) {
                    let partial_table = SymbolTableBuilder::build_tolerant(partial);
                    if !partial_table.tables.is_empty() {
                        partial_table
                    } else {
                        symbol_table
                    }
                } else {
                    symbol_table
                }
            } else {
                symbol_table
            }
        } else {
            symbol_table
        };

        Self {
            source,
            line_index,
            tokens,
            statements,
            parse_errors,
            symbol_table,
        }
    }

    /// Find the token at a given byte offset using binary search. O(log n).
    #[must_use]
    #[inline]
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

    /// Convert an LSP Position to a byte offset, then find the token at that offset.
    ///
    /// Convenience method that combines `LineIndex::position_to_offset` and
    /// `find_token_at`, eliminating a repeated pattern across LSP handlers.
    #[must_use]
    #[inline]
    pub fn find_token_at_position(
        &self,
        position: lsp_types::Position,
    ) -> Option<(&OwnedToken, usize)> {
        let offset = self.line_index.position_to_offset(&self.source, position);
        self.find_token_at(offset)
    }

    /// Get the text of a specific line. O(1) line lookup via LineIndex.
    #[must_use]
    #[inline]
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

/// Find the byte offset of the closing `)` that ends the first CREATE TABLE definition.
///
/// Scans the token stream for `CREATE TABLE ident (` ... `)` and returns the
/// end offset of the matching `)`. Returns `None` if no such pattern is found
/// or if the parentheses are unbalanced.
fn find_create_table_end(tokens: &[OwnedToken]) -> Option<usize> {
    let mut i = 0;
    while i + 2 < tokens.len() {
        if tokens[i].kind == TokenKind::Create
            && tokens[i + 1].kind == TokenKind::Table
            && tokens[i + 2].kind == TokenKind::Ident
        {
            // Skip past CREATE TABLE <ident> to find the opening (
            let mut j = i + 3;
            while j < tokens.len() && tokens[j].kind != TokenKind::LParen {
                j += 1;
            }
            if j >= tokens.len() {
                return None;
            }
            // Track paren depth to find the matching )
            let mut depth = 0i32;
            for tok in &tokens[j..] {
                match tok.kind {
                    TokenKind::LParen => depth += 1,
                    TokenKind::RParen => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(tok.span.end as usize);
                        }
                    }
                    _ => {}
                }
            }
            return None; // Unbalanced parens
        }
        i += 1;
    }
    None
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
        assert_eq!(&*analysis.tokens[0].text, "SELECT");
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
        assert_eq!(&*token.text, "SELECT");
    }

    #[test]
    fn test_find_token_at_mid() {
        let analysis = DocumentAnalysis::new("SELECT * FROM users");
        let (token, _) = analysis.find_token_at(3).unwrap();
        assert_eq!(&*token.text, "SELECT");
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
        assert_eq!(&*token.text, "@count");
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

    // --- Binary search fallback tests ---

    #[test]
    fn test_fallback_extracts_table_from_partial_source() {
        // Full source has garbage after CREATE TABLE that breaks the parser.
        // The fallback should find and extract the table definition.
        let source = "CREATE TABLE users (id INT, name VARCHAR(100))\nINSERT INTO users VALUES (";
        let analysis = DocumentAnalysis::new(source);
        assert!(
            analysis.symbol_table.tables.contains_key("USERS"),
            "Fallback should extract table from partial source"
        );
    }

    #[test]
    fn test_no_fallback_when_table_already_found() {
        let source = "CREATE TABLE users (id INT)";
        let analysis = DocumentAnalysis::new(source);
        assert!(analysis.symbol_table.tables.contains_key("USERS"));
    }

    #[test]
    fn test_no_fallback_without_create_table_keyword() {
        let source = "SELECT * FROM users\nWHERE id = 1";
        let analysis = DocumentAnalysis::new(source);
        assert!(analysis.symbol_table.tables.is_empty());
    }

    #[test]
    fn test_fallback_with_many_lines() {
        // Stress test: many lines before the garbage
        let mut lines = vec!["CREATE TABLE big_table (id INT".to_string()];
        for i in 1..50 {
            lines.push(format!("  , col_{i} VARCHAR(100)"));
        }
        lines.push(")".to_string());
        lines.push("INSERT INTO big_table VALUES (1,".to_string()); // incomplete
        let source = lines.join("\n");

        let analysis = DocumentAnalysis::new(&source);
        assert!(
            analysis.symbol_table.tables.contains_key("BIG_TABLE"),
            "Fallback should handle many-line sources"
        );
    }
}
