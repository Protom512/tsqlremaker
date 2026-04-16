//! Semantic Tokens 生成
//!
//! Lexer のトークンストリームから LSP Semantic Tokens を生成する。

use crate::offset_to_position;
use lsp_types::{
    SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend, SemanticTokensResult,
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

/// ソースコードから Semantic Tokens を生成する
pub fn semantic_tokens_full(source: &str) -> SemanticTokensResult {
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
            let (line, character) = offset_to_position(source, token.span.start);

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
}
