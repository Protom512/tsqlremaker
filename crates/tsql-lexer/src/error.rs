//! 字句解析エラー型
//!
//! 字句解析中に発生するエラーを表現する。

use tsql_token::Position;

/// 字句解析エラー
///
/// 字句解析中に発生する可能性のあるすべてのエラー型を表す。
#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    /// 終了していない文字列リテラル
    UnterminatedString {
        /// 開始位置
        start: Position,
        /// 引用符文字
        quote_char: char,
    },

    /// 終了していないブロックコメント
    UnterminatedBlockComment {
        /// 開始位置
        start: Position,
        /// ネストの深さ
        depth: usize,
    },

    /// 終了していない引用符付き識別子
    UnterminatedIdentifier {
        /// 開始位置
        start: Position,
        /// 括弧の種類
        bracket_type: BracketType,
    },

    /// 不正な文字
    InvalidCharacter {
        /// 不正な文字
        ch: char,
        /// 位置
        position: Position,
    },

    /// 予期しない EOF
    UnexpectedEof {
        /// 位置
        position: Position,
        /// 期待していたもの
        expected: String,
    },
}

/// 括弧の種類
///
/// 引用符付き識別子で使用される括弧の種類を表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracketType {
    /// 角括弧 [...]
    Square,
    /// 二重引用符 "..."
    DoubleQuote,
}

impl LexError {
    /// エラーメッセージを生成する
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnterminatedString { .. } => "Unterminated string literal".to_string(),
            Self::UnterminatedBlockComment { .. } => "Unterminated block comment".to_string(),
            Self::UnterminatedIdentifier { .. } => "Unterminated quoted identifier".to_string(),
            Self::InvalidCharacter { ch, .. } => {
                format!("Invalid character in SQL: '{}'", ch)
            }
            Self::UnexpectedEof { .. } => "Unexpected end of file".to_string(),
        }
    }

    /// エラー位置を取得する
    #[must_use]
    pub fn position(&self) -> Position {
        match self {
            Self::UnterminatedString { start, .. } => *start,
            Self::UnterminatedBlockComment { start, .. } => *start,
            Self::UnterminatedIdentifier { start, .. } => *start,
            Self::InvalidCharacter { position, .. } => *position,
            Self::UnexpectedEof { position, .. } => *position,
        }
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}:{}",
            self.message(),
            self.position().line,
            self.position().column
        )
    }
}

impl std::error::Error for LexError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unterminated_string_error() {
        let pos = Position::new(1, 1, 0);
        let error = LexError::UnterminatedString {
            start: pos,
            quote_char: '\'',
        };
        assert_eq!(error.message(), "Unterminated string literal");
        assert_eq!(error.position(), pos);
    }

    #[test]
    fn test_unterminated_block_comment_error() {
        let pos = Position::new(2, 5, 10);
        let error = LexError::UnterminatedBlockComment {
            start: pos,
            depth: 2,
        };
        assert_eq!(error.message(), "Unterminated block comment");
        assert_eq!(error.position(), pos);
    }

    #[test]
    fn test_unterminated_identifier_error() {
        let pos = Position::new(3, 10, 20);
        let error = LexError::UnterminatedIdentifier {
            start: pos,
            bracket_type: BracketType::Square,
        };
        assert_eq!(error.message(), "Unterminated quoted identifier");
        assert_eq!(error.position(), pos);
    }

    #[test]
    fn test_invalid_character_error() {
        let pos = Position::new(4, 15, 30);
        let error = LexError::InvalidCharacter {
            ch: '@',
            position: pos,
        };
        assert_eq!(error.message(), "Invalid character in SQL: '@'");
        assert_eq!(error.position(), pos);
    }

    #[test]
    fn test_unexpected_eof_error() {
        let pos = Position::new(5, 20, 40);
        let error = LexError::UnexpectedEof {
            position: pos,
            expected: "identifier".to_string(),
        };
        assert_eq!(error.message(), "Unexpected end of file");
        assert_eq!(error.position(), pos);
    }

    #[test]
    fn test_error_display() {
        let pos = Position::new(1, 5, 4);
        let error = LexError::UnterminatedString {
            start: pos,
            quote_char: '\'',
        };
        let display = format!("{}", error);
        assert!(display.contains("Unterminated string literal"));
        assert!(display.contains("1:5"));
    }

    #[test]
    fn test_bracket_type_equality() {
        assert_eq!(BracketType::Square, BracketType::Square);
        assert_eq!(BracketType::DoubleQuote, BracketType::DoubleQuote);
        assert_ne!(BracketType::Square, BracketType::DoubleQuote);
    }
}
