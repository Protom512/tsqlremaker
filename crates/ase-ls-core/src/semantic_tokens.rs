//! Semantic Tokens 生成
//!
//! Lexer のトークンストリームから LSP Semantic Tokens を生成する。

use crate::analysis::DocumentAnalysis;
use lsp_types::{
    Range, SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
    SemanticTokensRangeResult, SemanticTokensResult,
};
use tsql_token::TokenKind;

/// カスタムセマンティックトークンタイプの定義
#[must_use]
pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,     // 0
            SemanticTokenType::TYPE,        // 1 - データ型
            SemanticTokenType::FUNCTION,    // 2
            SemanticTokenType::STRING,      // 3
            SemanticTokenType::NUMBER,      // 4
            SemanticTokenType::COMMENT,     // 5
            SemanticTokenType::VARIABLE,    // 6 - @変数
            SemanticTokenType::OPERATOR,    // 7
            SemanticTokenType::PARAMETER,   // 8 - プロシージャパラメータ
            SemanticTokenType::CLASS,       // 9 - テーブル名
            SemanticTokenType::ENUM_MEMBER, // 10 - ブール値リテラル
        ],
        token_modifiers: vec![],
    }
}

/// TokenKind → セマンティックトークンタイプインデックスのマッピング
const fn token_kind_to_type_index(kind: TokenKind) -> Option<u32> {
    match kind {
        // キーワード (0)
        _ if kind.is_keyword() => Some(0),
        // データ型 (1)
        TokenKind::Int
        | TokenKind::Integer
        | TokenKind::Smallint
        | TokenKind::Tinyint
        | TokenKind::Bigint
        | TokenKind::Real
        | TokenKind::Double
        | TokenKind::Decimal
        | TokenKind::Numeric
        | TokenKind::Money
        | TokenKind::Smallmoney
        | TokenKind::Char
        | TokenKind::Varchar
        | TokenKind::Text
        | TokenKind::Nchar
        | TokenKind::Nvarchar
        | TokenKind::Ntext
        | TokenKind::Unichar
        | TokenKind::Univarchar
        | TokenKind::Unitext
        | TokenKind::Binary
        | TokenKind::Varbinary
        | TokenKind::Image
        | TokenKind::Date
        | TokenKind::Time
        | TokenKind::Datetime
        | TokenKind::Smalldatetime
        | TokenKind::Timestamp
        | TokenKind::Bigdatetime
        | TokenKind::Bit
        | TokenKind::Uniqueidentifier => Some(1),
        // 文字列 (3)
        TokenKind::String
        | TokenKind::NString
        | TokenKind::UnicodeString
        | TokenKind::HexString => Some(3),
        // 数値 (4)
        TokenKind::Number | TokenKind::FloatLiteral => Some(4),
        // コメント (5)
        TokenKind::LineComment | TokenKind::BlockComment => Some(5),
        // 変数 (6)
        TokenKind::LocalVar | TokenKind::GlobalVar => Some(6),
        // 一時テーブル (9 = CLASS)
        TokenKind::TempTable | TokenKind::GlobalTempTable => Some(9),
        // 演算子 (7)
        TokenKind::Eq
        | TokenKind::Ne
        | TokenKind::NeAlt
        | TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::Le
        | TokenKind::Ge
        | TokenKind::NotLt
        | TokenKind::NotGt
        | TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent
        | TokenKind::Ampersand
        | TokenKind::Pipe
        | TokenKind::Caret
        | TokenKind::Tilde
        | TokenKind::Assign
        | TokenKind::PlusAssign
        | TokenKind::MinusAssign
        | TokenKind::StarAssign
        | TokenKind::SlashAssign
        | TokenKind::Concat => Some(7),
        _ => None,
    }
}

/// Resolve a token's semantic type index, handling both direct kinds and symbol-table identifiers.
#[inline]
fn resolve_token_type(analysis: &DocumentAnalysis, kind: TokenKind, text: &str) -> Option<u32> {
    token_kind_to_type_index(kind).or_else(|| {
        if kind == TokenKind::Ident {
            analysis.symbol_table.resolve_semantic_type(text)
        } else {
            None
        }
    })
}

/// Accumulator for LSP semantic token delta encoding.
struct DeltaEncoder {
    prev_line: u32,
    prev_char: u32,
}

impl DeltaEncoder {
    const fn new() -> Self {
        Self {
            prev_line: 0,
            prev_char: 0,
        }
    }

    /// Push a delta-encoded semantic token.
    fn push(
        &mut self,
        tokens: &mut Vec<SemanticToken>,
        line: u32,
        character: u32,
        length: u32,
        token_type: u32,
    ) {
        let delta_line = line.saturating_sub(self.prev_line);
        let delta_start = if delta_line == 0 {
            character.saturating_sub(self.prev_char)
        } else {
            character
        };

        tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        });

        self.prev_line = line;
        self.prev_char = character;
    }
}

/// ソースコードから Semantic Tokens を生成する（DocumentAnalysis利用）
#[must_use]
pub fn semantic_tokens_full_with_analysis(analysis: &DocumentAnalysis) -> SemanticTokensResult {
    let mut tokens = Vec::new();
    let mut encoder = DeltaEncoder::new();

    for token in &analysis.tokens {
        if let Some(type_idx) = resolve_token_type(analysis, token.kind, &token.text) {
            let (line, character) = analysis.line_index.offset_to_position(token.span.start);
            encoder.push(&mut tokens, line, character, token.span.len(), type_idx);
        }
    }

    SemanticTokens {
        result_id: None,
        data: tokens,
    }
    .into()
}

/// Generate Semantic Tokens for a specific range using DocumentAnalysis.
/// Only tokens whose start position falls within [range.start, range.end] are included.
#[must_use]
pub fn semantic_tokens_range_with_analysis(
    analysis: &DocumentAnalysis,
    range: Range,
) -> SemanticTokensRangeResult {
    let mut tokens = Vec::new();
    let mut encoder = DeltaEncoder::new();

    for token in &analysis.tokens {
        let (line, character) = analysis.line_index.offset_to_position(token.span.start);

        // Skip tokens before range
        if line < range.start.line
            || (line == range.start.line && character < range.start.character)
        {
            continue;
        }
        // Stop past range
        if line > range.end.line || (line == range.end.line && character > range.end.character) {
            break;
        }

        if let Some(type_idx) = resolve_token_type(analysis, token.kind, &token.text) {
            encoder.push(&mut tokens, line, character, token.span.len(), type_idx);
        }
    }

    SemanticTokensRangeResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    // --- Semantic token enhancement tests (Phase #83) ---

    #[test]
    fn test_table_name_gets_class_token() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected tokens"),
        };
        // Find token at "users" on line 1 (the FROM clause)
        // CLASS = index 9
        let class_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 9).collect();
        assert!(
            !class_tokens.is_empty(),
            "Table name 'users' should be highlighted as CLASS (type 9), got tokens: {:?}",
            tokens
                .data
                .iter()
                .map(|t| (t.token_type, t.delta_line, t.delta_start, t.length))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_procedure_name_gets_function_token() {
        let source = "CREATE PROCEDURE my_proc AS BEGIN RETURN 1 END";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected tokens"),
        };
        // FUNCTION = index 2
        let func_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 2).collect();
        assert!(
            !func_tokens.is_empty(),
            "Procedure name 'my_proc' should be highlighted as FUNCTION (type 2)"
        );
    }

    #[test]
    fn test_identifier_not_in_symbol_table_not_highlighted() {
        let source = "SELECT * FROM unknown_table";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected tokens"),
        };
        // 'unknown_table' is not in any symbol table → should NOT get CLASS token
        // But it will still be there as an Ident (not highlighted)
        // Check that no CLASS (9) tokens exist since no tables are defined
        let class_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 9).collect();
        assert!(
            class_tokens.is_empty(),
            "Unknown identifiers should not get CLASS token"
        );
    }

    // === Coverage gap tests ===

    #[test]
    fn test_range_tokens_basic() {
        use lsp_types::{Position, Range as LspRange};
        let source = "CREATE TABLE t (id INT)\nSELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = LspRange {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 20,
            },
        };
        let result = semantic_tokens_range_with_analysis(&analysis, range);
        let tokens = match result {
            lsp_types::SemanticTokensRangeResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // Should have tokens for SELECT, *, FROM at minimum
        assert!(
            !tokens.data.is_empty(),
            "Range tokens should not be empty for SELECT line"
        );
    }

    #[test]
    fn test_range_tokens_empty_range() {
        use lsp_types::{Position, Range as LspRange};
        let source = "SELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Range outside any tokens
        let range = LspRange {
            start: Position {
                line: 5,
                character: 0,
            },
            end: Position {
                line: 5,
                character: 10,
            },
        };
        let result = semantic_tokens_range_with_analysis(&analysis, range);
        let tokens = match result {
            lsp_types::SemanticTokensRangeResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        assert!(tokens.data.is_empty());
    }

    #[test]
    fn test_view_name_gets_class_token() {
        let source = "CREATE VIEW my_view AS SELECT 1";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // "my_view" should get CLASS token (index 9)
        assert!(
            tokens.data.iter().any(|t| t.token_type == 9),
            "View name should get CLASS semantic token"
        );
    }

    #[test]
    fn test_keyword_tokens_present() {
        let source = "SELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // SELECT and FROM should be keyword tokens (type 0)
        assert!(
            tokens.data.iter().any(|t| t.token_type == 0),
            "Keywords should get KEYWORD semantic token"
        );
    }

    #[test]
    fn test_empty_source_no_tokens() {
        let source = "";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        assert!(tokens.data.is_empty());
    }

    #[test]
    fn test_variable_gets_variable_token() {
        let source = "DECLARE @count INT\nSET @count = 1";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // VARIABLE = index 6 (see semantic_tokens_legend)
        assert!(
            tokens.data.iter().any(|t| t.token_type == 6),
            "Local variable @count should get VARIABLE semantic token (type 6)"
        );
    }

    #[test]
    fn test_datatype_gets_type_token() {
        let source = "DECLARE @x INT";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // TYPE = index 1 (see semantic_tokens_legend)
        assert!(
            tokens.data.iter().any(|t| t.token_type == 1),
            "INT data type should get TYPE semantic token (type 1)"
        );
    }

    #[test]
    fn test_range_tokens_intersecting_boundary() {
        use lsp_types::{Position, Range as LspRange};
        let source = "SELECT * FROM t WHERE id = 1";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Range covering FROM and t (char 9-15)
        let range = LspRange {
            start: Position {
                line: 0,
                character: 9,
            },
            end: Position {
                line: 0,
                character: 16,
            },
        };
        let result = semantic_tokens_range_with_analysis(&analysis, range);
        let tokens = match result {
            SemanticTokensRangeResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // Should include FROM keyword token and identifier t
        assert!(!tokens.data.is_empty(), "FROM and t should be in the range");
        // At least one keyword (FROM) and optionally one identifier
        assert!(
            tokens.data.iter().any(|t| t.token_type == 0),
            "FROM keyword should get KEYWORD token in range"
        );
    }
}
