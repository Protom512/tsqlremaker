//! Span and Position types for source location tracking.
//!
//! These types mirror `tsql_token::Span` and `tsql_token::Position` layout
//! for zero-cost conversion in the tsql-parser conversion layer.

/// Source location span (byte offsets).
///
/// Same layout as `tsql_token::Span { start: u32, end: u32 }` —
/// conversion via `From<tsql_token::Span>` will be implemented in tsql-parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
}

impl Span {
    /// Creates a new span from start and end byte offsets.
    #[must_use]
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    /// Returns `true` if this span covers zero bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Human-readable source position (line, column, offset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    /// Line number (1-indexed).
    pub line: u32,
    /// Column number (1-indexed).
    pub column: u32,
    /// Byte offset from start of source.
    pub offset: u32,
}

impl Position {
    /// Creates a new position.
    #[must_use]
    pub fn new(line: u32, column: u32, offset: u32) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }
}

impl Default for Position {
    fn default() -> Self {
        Self {
            line: 1,
            column: 1,
            offset: 0,
        }
    }
}
