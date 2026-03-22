//! パーサーエラー型の定義
//!
//! 構文解析中に発生するエラーを表現する。

use std::fmt;

use tsql_token::{Position, Span, TokenKind};

/// パース結果型エイリアス
pub type ParseResult<T> = Result<T, ParseError>;

/// パースエラー
///
/// 構文解析中に発生するエラーを表す。
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// 予期しないトークン
    UnexpectedToken {
        /// 期待されるトークン種別のリスト
        expected: Vec<TokenKind>,
        /// 見つかったトークン種別
        found: TokenKind,
        /// 位置情報
        position: Position,
    },
    /// 予期しないEOF
    UnexpectedEof {
        /// 期待されていた内容の説明
        expected: String,
        /// 位置情報
        position: Position,
    },
    /// 無効な構文
    InvalidSyntax {
        /// エラーメッセージ
        message: String,
        /// 位置情報
        position: Position,
    },
    /// 再帰深度制限超過
    RecursionLimitExceeded {
        /// 制限値
        limit: usize,
        /// 位置情報
        position: Position,
    },
    /// バッチエラー
    BatchError {
        /// バッチ番号
        batch_number: usize,
        /// 元のエラー
        error: Box<ParseError>,
    },
}

impl ParseError {
    /// 予期しないトークンエラーを作成
    #[must_use]
    pub fn unexpected_token(
        expected: Vec<TokenKind>,
        found: TokenKind,
        position: Position,
    ) -> Self {
        Self::UnexpectedToken {
            expected,
            found,
            position,
        }
    }

    /// 予期しないEOFエラーを作成
    #[must_use]
    pub fn unexpected_eof(expected: String, position: Position) -> Self {
        Self::UnexpectedEof { expected, position }
    }

    /// 無効な構文エラーを作成
    #[must_use]
    pub fn invalid_syntax(message: String, position: Position) -> Self {
        Self::InvalidSyntax { message, position }
    }

    /// 再帰制限超過エラーを作成
    #[must_use]
    pub fn recursion_limit(limit: usize, position: Position) -> Self {
        Self::RecursionLimitExceeded { limit, position }
    }

    /// エラーの位置情報をSpanとして返す
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        match self {
            Self::UnexpectedToken { position, .. }
            | Self::InvalidSyntax { position, .. }
            | Self::UnexpectedEof { position, .. }
            | Self::RecursionLimitExceeded { position, .. } => Some(Span {
                start: position.offset,
                end: position.offset,
            }),
            Self::BatchError { error, .. } => error.span(),
        }
    }

    /// エラーの開始位置を返す
    #[must_use]
    pub fn position(&self) -> Position {
        match self {
            Self::UnexpectedToken { position, .. }
            | Self::InvalidSyntax { position, .. }
            | Self::UnexpectedEof { position, .. }
            | Self::RecursionLimitExceeded { position, .. } => *position,
            Self::BatchError { error, .. } => error.position(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedToken {
                expected,
                found,
                position,
            } => {
                write!(
                    f,
                    "unexpected token at offset {}: expected ",
                    position.offset
                )?;
                for (i, kind) in expected.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", kind)?;
                }
                write!(f, ", found {:?}", found)
            }
            Self::UnexpectedEof { expected, position } => write!(
                f,
                "unexpected EOF at offset {}: expected {}",
                position.offset, expected
            ),
            Self::InvalidSyntax { message, position } => {
                write!(
                    f,
                    "invalid syntax at offset {}: {}",
                    position.offset, message
                )
            }
            Self::RecursionLimitExceeded { limit, position } => write!(
                f,
                "recursion limit exceeded at offset {}: maximum depth is {}",
                position.offset, limit
            ),
            Self::BatchError {
                batch_number,
                error,
            } => write!(f, "error in batch {}: {}", batch_number, error),
        }
    }
}

impl std::error::Error for ParseError {}

/// 複数エラーを含むパース結果型エイリアス
pub type ParseResultWithErrors<T> = Result<T, ParseErrors>;

/// 複数のパースエラーを表す型
///
/// エラー回復機能により、1回のパースで複数の構文エラーを検出できる場合に使用する。
#[derive(Debug, Clone, PartialEq)]
pub struct ParseErrors {
    /// 検出されたエラーのリスト
    pub errors: Vec<ParseError>,
}

impl ParseErrors {
    /// 新しいParseErrorsを作成
    #[must_use]
    pub fn new(errors: Vec<ParseError>) -> Self {
        Self { errors }
    }

    /// エラーが空かどうかを確認
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// エラーの数を返す
    #[must_use]
    pub fn len(&self) -> usize {
        self.errors.len()
    }

    /// 最初のエラーを返す
    #[must_use]
    pub fn first(&self) -> Option<&ParseError> {
        self.errors.first()
    }

    /// イテレータを返す
    pub fn iter(&self) -> impl Iterator<Item = &ParseError> {
        self.errors.iter()
    }
}

impl fmt::Display for ParseErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "found {} parse error(s):", self.errors.len())?;
        for (i, error) in self.errors.iter().enumerate() {
            writeln!(f, "  {}: {}", i + 1, error)?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseErrors {}

impl From<ParseError> for ParseErrors {
    fn from(error: ParseError) -> Self {
        Self::new(vec![error])
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_unexpected_token_error() {
        let position = Position::new(1, 11, 10);
        let error = ParseError::unexpected_token(
            vec![TokenKind::Select, TokenKind::From],
            TokenKind::Ident,
            position,
        );

        assert_eq!(
            error,
            ParseError::UnexpectedToken {
                expected: vec![TokenKind::Select, TokenKind::From],
                found: TokenKind::Ident,
                position
            }
        );
    }

    #[test]
    fn test_unexpected_eof_error() {
        let pos = Position::new(5, 10, 100);
        let error = ParseError::unexpected_eof("SELECT statement".to_string(), pos);

        assert_eq!(
            error,
            ParseError::UnexpectedEof {
                expected: "SELECT statement".to_string(),
                position: pos
            }
        );
    }

    #[test]
    fn test_invalid_syntax_error() {
        let position = Position::new(1, 1, 0);
        let error = ParseError::invalid_syntax("missing FROM clause".to_string(), position);

        assert_eq!(
            error,
            ParseError::InvalidSyntax {
                message: "missing FROM clause".to_string(),
                position
            }
        );
    }

    #[test]
    fn test_recursion_limit_error() {
        let pos = Position::new(100, 1, 5000);
        let error = ParseError::recursion_limit(1000, pos);

        assert_eq!(
            error,
            ParseError::RecursionLimitExceeded {
                limit: 1000,
                position: pos
            }
        );
    }

    #[test]
    fn test_error_display() {
        let position = Position::new(1, 11, 10);
        let error =
            ParseError::unexpected_token(vec![TokenKind::Semicolon], TokenKind::Ident, position);

        let display = format!("{}", error);
        assert!(display.contains("unexpected token"));
        assert!(display.contains("expected"));
        assert!(display.contains("Semicolon"));
    }

    #[test]
    fn test_error_span() {
        let position = Position::new(1, 11, 10);
        let error =
            ParseError::unexpected_token(vec![TokenKind::Select], TokenKind::Ident, position);

        assert_eq!(error.span(), Some(Span { start: 10, end: 10 }));
    }

    #[test]
    fn test_error_position() {
        let position = Position::new(1, 101, 100);
        let error =
            ParseError::unexpected_token(vec![TokenKind::Select], TokenKind::Ident, position);

        let pos = error.position();
        assert_eq!(pos.offset, 100);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 101);
    }

    #[test]
    fn test_error_span_for_eof() {
        let pos = Position::new(5, 10, 100);
        let error = ParseError::unexpected_eof("statement".to_string(), pos);

        let span = error.span();
        assert_eq!(
            span,
            Some(Span {
                start: 100,
                end: 100
            })
        );
    }

    #[test]
    fn test_error_position_for_eof() {
        let pos = Position::new(5, 10, 100);
        let error = ParseError::unexpected_eof("statement".to_string(), pos);

        let error_pos = error.position();
        assert_eq!(error_pos, pos);
    }

    #[test]
    fn test_error_span_for_recursion_limit() {
        let pos = Position::new(100, 1, 5000);
        let error = ParseError::recursion_limit(1000, pos);

        let span = error.span();
        assert_eq!(
            span,
            Some(Span {
                start: 5000,
                end: 5000
            })
        );
    }

    #[test]
    fn test_error_position_for_recursion_limit() {
        let pos = Position::new(100, 1, 5000);
        let error = ParseError::recursion_limit(1000, pos);

        let error_pos = error.position();
        assert_eq!(error_pos, pos);
    }

    #[test]
    fn test_batch_error_span() {
        let position = Position::new(1, 11, 10);
        let inner =
            ParseError::unexpected_token(vec![TokenKind::Select], TokenKind::Ident, position);
        let error = ParseError::BatchError {
            batch_number: 1,
            error: Box::new(inner),
        };

        assert_eq!(error.span(), Some(Span { start: 10, end: 10 }));
    }

    #[test]
    fn test_batch_error_position() {
        let position = Position::new(1, 11, 10);
        let inner =
            ParseError::unexpected_token(vec![TokenKind::Select], TokenKind::Ident, position);
        let error = ParseError::BatchError {
            batch_number: 1,
            error: Box::new(inner),
        };

        let pos = error.position();
        assert_eq!(pos.offset, 10);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 11);
    }

    #[test]
    fn test_display_unexpected_eof() {
        let pos = Position::new(5, 10, 100);
        let error = ParseError::unexpected_eof("SELECT statement".to_string(), pos);

        let display = format!("{}", error);
        assert!(display.contains("unexpected EOF"));
        assert!(display.contains("SELECT statement"));
    }

    #[test]
    fn test_display_invalid_syntax() {
        let position = Position::new(1, 1, 0);
        let error = ParseError::invalid_syntax("missing FROM clause".to_string(), position);

        let display = format!("{}", error);
        assert!(display.contains("invalid syntax"));
        assert!(display.contains("missing FROM clause"));
    }

    #[test]
    fn test_display_recursion_limit() {
        let pos = Position::new(100, 1, 5000);
        let error = ParseError::recursion_limit(1000, pos);

        let display = format!("{}", error);
        assert!(display.contains("recursion limit exceeded"));
        assert!(display.contains("1000"));
    }

    #[test]
    fn test_display_batch_error() {
        let position = Position::new(1, 11, 10);
        let inner =
            ParseError::unexpected_token(vec![TokenKind::Select], TokenKind::Ident, position);
        let error = ParseError::BatchError {
            batch_number: 2,
            error: Box::new(inner),
        };

        let display = format!("{}", error);
        assert!(display.contains("batch 2"));
    }
}
