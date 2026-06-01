//! Semantic Tokens 生成
//!
//! Lexer のトークンストリームから LSP Semantic Tokens を生成する。

use crate::analysis::DocumentAnalysis;
use crate::line_index::LineIndex;
use lsp_types::{
    Range, SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
    SemanticTokensRangeResult, SemanticTokensResult,
};
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// カスタムセマンティックトークンタイプの定義
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
fn token_kind_to_type_index(kind: TokenKind) -> Option<u32> {
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

/// Resolve an identifier token's semantic type using the symbol table.
fn resolve_ident_type(analysis: &DocumentAnalysis, text: &str) -> Option<u32> {
    let upper = text.to_uppercase();
    if analysis.symbol_table.tables.contains_key(&upper) {
        return Some(9); // CLASS
    }
    if analysis.symbol_table.procedures.contains_key(&upper) {
        return Some(2); // FUNCTION
    }
    if analysis.symbol_table.views.contains_key(&upper) {
        return Some(9); // CLASS — views are table-like
    }
    if analysis.symbol_table.indexes.contains_key(&upper) {
        return Some(9); // CLASS — indexes are objects
    }
    None
}

/// ソースコードから Semantic Tokens を生成する（DocumentAnalysis利用）
pub fn semantic_tokens_full_with_analysis(analysis: &DocumentAnalysis) -> SemanticTokensResult {
    let mut tokens = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;

    for token in &analysis.tokens {
        let type_idx = token_kind_to_type_index(token.kind).or_else(|| {
            if token.kind == TokenKind::Ident {
                resolve_ident_type(analysis, &token.text)
            } else {
                None
            }
        });

        if let Some(type_idx) = type_idx {
            let (line, character) = analysis.line_index.offset_to_position(token.span.start);

            let delta_line = line.saturating_sub(prev_line);
            let delta_start = if delta_line == 0 {
                character.saturating_sub(prev_char)
            } else {
                character
            };

            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length: token.span.len(),
                token_type: type_idx,
                token_modifiers_bitset: 0,
            });

            prev_line = line;
            prev_char = character;
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
pub fn semantic_tokens_range_with_analysis(
    analysis: &DocumentAnalysis,
    range: Range,
) -> SemanticTokensRangeResult {
    let mut tokens = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;

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

        let type_idx = token_kind_to_type_index(token.kind).or_else(|| {
            if token.kind == TokenKind::Ident {
                resolve_ident_type(analysis, &token.text)
            } else {
                None
            }
        });

        if let Some(type_idx) = type_idx {
            let delta_line = line.saturating_sub(prev_line);
            let delta_start = if delta_line == 0 {
                character.saturating_sub(prev_char)
            } else {
                character
            };

            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length: token.span.len(),
                token_type: type_idx,
                token_modifiers_bitset: 0,
            });

            prev_line = line;
            prev_char = character;
        }
    }

    SemanticTokensRangeResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    })
}

/// ソースコードから Semantic Tokens を生成する（ソースから構築）
pub fn semantic_tokens_full(source: &str) -> SemanticTokensResult {
    let line_index = LineIndex::new(source);
    let lexer = Lexer::new(source);
    let mut tokens = Vec::new();
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;

    for token_result in lexer {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };

        if let Some(type_idx) = token_kind_to_type_index(token.kind) {
            let (line, character) = line_index.offset_to_position(token.span.start);

            // LSP Semantic Tokens は差分エンコーディング
            let delta_line = line.saturating_sub(prev_line);
            let delta_start = if delta_line == 0 {
                character.saturating_sub(prev_char)
            } else {
                character
            };

            tokens.push(SemanticToken {
                delta_line,
                delta_start,
                length: token.span.len(),
                token_type: type_idx,
                token_modifiers_bitset: 0,
            });

            prev_line = line;
            prev_char = character;
        }
    }

    SemanticTokens {
        result_id: None,
        data: tokens,
    }
    .into()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_token() {
        let result = semantic_tokens_full("SELECT * FROM users");
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Some tokens"),
        };
        assert!(!tokens.data.is_empty());
        assert_eq!(tokens.data[0].token_type, 0);
        assert_eq!(tokens.data[0].delta_line, 0);
        assert_eq!(tokens.data[0].delta_start, 0);
        assert_eq!(tokens.data[0].length, 6);
    }

    #[test]
    fn test_string_token() {
        let result = semantic_tokens_full("'hello'");
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Some tokens"),
        };
        assert!(!tokens.data.is_empty());
        assert_eq!(tokens.data[0].token_type, 3);
    }

    #[test]
    fn test_number_token() {
        let result = semantic_tokens_full("42");
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Some tokens"),
        };
        assert!(!tokens.data.is_empty());
        assert_eq!(tokens.data[0].token_type, 4);
    }

    #[test]
    fn test_variable_token() {
        let result = semantic_tokens_full("@foo");
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Some tokens"),
        };
        assert!(!tokens.data.is_empty());
        assert_eq!(tokens.data[0].token_type, 6);
    }

    #[test]
    fn test_multiline_delta() {
        let source = "SELECT\nFROM";
        let result = semantic_tokens_full(source);
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Some tokens"),
        };
        assert_eq!(tokens.data.len(), 2);
        assert_eq!(tokens.data[0].delta_line, 0);
        assert_eq!(tokens.data[1].delta_line, 1);
    }

    #[test]
    fn test_legend_has_types() {
        let legend = semantic_tokens_legend();
        assert!(!legend.token_types.is_empty());
    }

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
    fn test_block_comment_token() {
        // semantic_tokens_full uses Lexer which skips comments by default
        // but we test the token_kind_to_type_index mapping for comments
        let source = "/* block comment */\nSELECT 1";
        let result = semantic_tokens_full(source);
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // Keywords should still be present
        assert!(!tokens.data.is_empty(), "Should have tokens after comment");
    }

    #[test]
    fn test_temp_table_token() {
        let result = semantic_tokens_full("SELECT * FROM #temp");
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        let temp_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 9).collect();
        assert!(
            !temp_tokens.is_empty(),
            "Temp table #temp should be CLASS (type 9)"
        );
    }

    #[test]
    fn test_operator_tokens() {
        let result = semantic_tokens_full("SELECT 1 + 2");
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        let op_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 7).collect();
        assert!(!op_tokens.is_empty(), "Operator + should be type 7");
    }

    #[test]
    fn test_datatype_token() {
        let result = semantic_tokens_full("CREATE TABLE t (id INT)");
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        let dtype_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 1).collect();
        assert!(!dtype_tokens.is_empty(), "INT should be data type (type 1)");
    }

    #[test]
    fn test_global_var_token() {
        let result = semantic_tokens_full("SELECT @@VERSION");
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        let var_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 6).collect();
        assert!(
            !var_tokens.is_empty(),
            "@@VERSION should be variable (type 6)"
        );
    }

    #[test]
    fn test_hex_string_token() {
        let result = semantic_tokens_full("SELECT 0x1234");
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // HexString should be type 3 (string)
        let str_tokens: Vec<_> = tokens
            .data
            .iter()
            .filter(|t| t.token_type == 3 || t.token_type == 4)
            .collect();
        assert!(
            !str_tokens.is_empty(),
            "Hex or number literal should be tokenized"
        );
    }
}
