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
        /// ソースコード抜粋（最大80文字）
        source_excerpt: Option<String>,
    },

    /// 終了していないブロックコメント
    UnterminatedBlockComment {
        /// 開始位置
        start: Position,
        /// ネストの深さ
        depth: usize,
        /// ソースコード抜粋（最大80文字）
        source_excerpt: Option<String>,
    },

    /// 終了していない引用符付き識別子
    UnterminatedIdentifier {
        /// 開始位置
        start: Position,
        /// 括弧の種類
        bracket_type: BracketType,
        /// ソースコード抜粋（最大80文字）
        source_excerpt: Option<String>,
    },

    /// 不正な文字
    InvalidCharacter {
        /// 不正な文字
        ch: char,
        /// 位置
        position: Position,
        /// ソースコード抜粋（最大80文字）
        source_excerpt: Option<String>,
    },

    /// 予期しない EOF
    UnexpectedEof {
        /// 位置
        position: Position,
        /// 期待していたもの
        expected: String,
        /// ソースコード抜粋（最大80文字）
        source_excerpt: Option<String>,
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

/// ソースコードからエラー行の抜粋を取得する
///
/// # Arguments
///
/// * `source` - ソースコード全体
/// * `position` - エラー位置
///
/// # Returns
///
/// エラー行の抜粋（最大80文字）
#[must_use]
pub fn extract_source_line(source: &str, position: Position) -> Option<String> {
    let line_start = source
        .lines()
        .nth(position.line.saturating_sub(1) as usize)
        .map(|s| s.to_string())?;

    // 最大80文字に制限
    if line_start.len() <= 80 {
        Some(line_start)
    } else {
        // 80文字を超える場合は末尾を省略
        Some(format!("{}...", &line_start[..77]))
    }
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

    /// ソースコード抜粋を取得する
    #[must_use]
    pub fn source_excerpt(&self) -> Option<&str> {
        match self {
            Self::UnterminatedString { source_excerpt, .. } => source_excerpt.as_deref(),
            Self::UnterminatedBlockComment { source_excerpt, .. } => source_excerpt.as_deref(),
            Self::UnterminatedIdentifier { source_excerpt, .. } => source_excerpt.as_deref(),
            Self::InvalidCharacter { source_excerpt, .. } => source_excerpt.as_deref(),
            Self::UnexpectedEof { source_excerpt, .. } => source_excerpt.as_deref(),
        }
    }

    /// ソースコード抜粋を設定する
    #[must_use]
    pub fn with_excerpt(mut self, source: &str) -> Self {
        let excerpt = extract_source_line(source, self.position());
        match &mut self {
            Self::UnterminatedString { source_excerpt, .. } => *source_excerpt = excerpt,
            Self::UnterminatedBlockComment { source_excerpt, .. } => *source_excerpt = excerpt,
            Self::UnterminatedIdentifier { source_excerpt, .. } => *source_excerpt = excerpt,
            Self::InvalidCharacter { source_excerpt, .. } => *source_excerpt = excerpt,
            Self::UnexpectedEof { source_excerpt, .. } => *source_excerpt = excerpt,
        }
        self
    }

    /// ソースコード抜粋付きの詳細なエラーメッセージを生成する
    #[must_use]
    pub fn detailed_message(&self) -> String {
        let mut msg = format!(
            "{} at {}:{}",
            self.message(),
            self.position().line,
            self.position().column
        );
        if let Some(excerpt) = self.source_excerpt() {
            msg.push_str(&format!("\n  | {}", excerpt));
        }
        msg
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
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_unterminated_string_error() {
        let pos = Position::new(1, 1, 0);
        let error = LexError::UnterminatedString {
            start: pos,
            quote_char: '\'',
            source_excerpt: None,
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
            source_excerpt: None,
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
            source_excerpt: None,
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
            source_excerpt: None,
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
            source_excerpt: None,
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
            source_excerpt: None,
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

    #[test]
    fn test_extract_source_line() {
        let source = "SELECT * FROM users\nWHERE id = 1";
        let pos = Position::new(1, 1, 0);
        let excerpt = extract_source_line(source, pos);
        assert_eq!(excerpt, Some("SELECT * FROM users".to_string()));

        let pos2 = Position::new(2, 1, 21);
        let excerpt2 = extract_source_line(source, pos2);
        assert_eq!(excerpt2, Some("WHERE id = 1".to_string()));
    }

    #[test]
    fn test_extract_source_line_truncation() {
        let long_line = "a".repeat(100);
        let source = format!("{}\nnext line", long_line);
        let pos = Position::new(1, 1, 0);
        let excerpt = extract_source_line(&source, pos);
        // 80文字を超える場合は省略される
        let excerpt = excerpt.unwrap();
        assert!(excerpt.len() <= 80);
        assert!(excerpt.ends_with("..."));
    }

    #[test]
    fn test_error_with_excerpt() {
        let source = "SELECT 'unterminated FROM users";
        let pos = Position::new(1, 1, 0);
        let error = LexError::UnterminatedString {
            start: pos,
            quote_char: '\'',
            source_excerpt: None,
        }
        .with_excerpt(source);

        assert_eq!(
            error.source_excerpt(),
            Some("SELECT 'unterminated FROM users")
        );
    }

    #[test]
    fn test_detailed_message() {
        let source = "SELECT © FROM users";
        let pos = Position::new(1, 9, 8);
        let error = LexError::InvalidCharacter {
            ch: '©',
            position: pos,
            source_excerpt: None,
        }
        .with_excerpt(source);

        let msg = error.detailed_message();
        assert!(msg.contains("Invalid character"));
        assert!(msg.contains("1:9"));
        assert!(msg.contains("SELECT © FROM users"));
    }
}
