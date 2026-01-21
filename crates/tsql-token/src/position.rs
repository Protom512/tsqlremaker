//! ソースコード上の位置情報の表現
//!
//! トークンの開始位置と終了位置を追跡するための構造体を定義する。

/// ソースコード上のバイト単位の範囲
///
/// トークンまたはノードの開始位置と終了位置をバイトオフセットで表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// 開始位置のバイトオフセット（0-indexed）
    pub start: u32,
    /// 終了位置のバイトオフセット（0-indexed、排他的）
    pub end: u32,
}

impl Span {
    /// 新しい Span を作成する
    ///
    /// # Arguments
    ///
    /// * `start` - 開始位置のバイトオフセット
    /// * `end` - 終了位置のバイトオフセット
    ///
    /// # Panics
    ///
    /// `end` が `start` より小さい場合にパニックする。
    #[must_use]
    pub const fn new(start: u32, end: u32) -> Self {
        assert!(end >= start, "Span end must be >= start");
        Self { start, end }
    }

    /// Span の長さ（バイト数）を返す
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    /// 空の Span かどうかを判定する
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// 2つの Span を結合する
    ///
    /// # Arguments
    ///
    /// * `other` - 結合するもう一方の Span
    ///
    /// # Returns
    ///
    /// 両方の Span を含む最小の Span
    #[must_use]
    pub const fn merge(self, other: Span) -> Span {
        Span {
            start: if self.start < other.start {
                self.start
            } else {
                other.start
            },
            end: if self.end > other.end {
                self.end
            } else {
                other.end
            },
        }
    }
}

/// ソースコード上の人間可読な位置
///
/// 行番号、列番号、バイトオフセットを含む。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// 行番号（1-indexed）
    pub line: u32,
    /// 列番号（1-indexed、タブ幅を8として計算）
    pub column: u32,
    /// バイトオフセット（0-indexed）
    pub offset: u32,
}

impl Position {
    /// 新しい Position を作成する
    ///
    /// # Arguments
    ///
    /// * `line` - 行番号（1-indexed）
    /// * `column` - 列番号（1-indexed）
    /// * `offset` - バイトオフセット（0-indexed）
    #[must_use]
    pub const fn new(line: u32, column: u32, offset: u32) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }

    /// 先頭の位置（1行目、1列目、オフセット0）を作成する
    #[must_use]
    pub const fn start() -> Self {
        Self {
            line: 1,
            column: 1,
            offset: 0,
        }
    }

    /// 次の文字の位置を計算する
    ///
    /// # Arguments
    ///
    /// * `ch` - 現在の文字
    /// * `tab_width` - タブ幅（デフォルトは8）
    ///
    /// # Returns
    ///
    /// 次の文字の位置
    #[must_use]
    pub const fn next_char(self, ch: char, tab_width: u32) -> Self {
        match ch {
            '\r' => Self {
                line: self.line + 1,
                column: 1,
                offset: self.offset + 1,
            },
            '\n' => Self {
                line: self.line + 1,
                column: 1,
                offset: self.offset + 1,
            },
            '\t' => {
                // タブ幅を考慮して列位置を計算
                let next_tab_stop = ((self.column - 1 + tab_width) / tab_width) * tab_width + 1;
                Self {
                    line: self.line,
                    column: next_tab_stop,
                    offset: self.offset + 1,
                }
            }
            _ => Self {
                line: self.line,
                column: self.column + 1,
                offset: self.offset + ch.len_utf8() as u32,
            },
        }
    }

    /// 指定したバイト数だけ位置を進める
    ///
    /// # Arguments
    ///
    /// * `bytes` - 進めるバイト数
    ///
    /// # Returns
    ///
    /// 進めた後の位置（行と列は変更せず、オフセットのみ更新）
    #[must_use]
    pub const fn advance_bytes(self, bytes: u32) -> Self {
        Self {
            line: self.line,
            column: self.column,
            offset: self.offset + bytes,
        }
    }

    /// Span を作成する
    ///
    /// # Arguments
    ///
    /// * `length` - Span の長さ（バイト数）
    ///
    /// # Returns
    ///
    /// この位置から始まる Span
    #[must_use]
    pub const fn to_span(self, length: u32) -> Span {
        Span {
            start: self.offset,
            end: self.offset + length,
        }
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::start()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_new() {
        let span = Span::new(10, 20);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 20);
        assert_eq!(span.len(), 10);
        assert!(!span.is_empty());
    }

    #[test]
    fn test_span_empty() {
        let span = Span::new(10, 10);
        assert_eq!(span.len(), 0);
        assert!(span.is_empty());
    }

    #[test]
    fn test_span_merge() {
        let span1 = Span::new(10, 20);
        let span2 = Span::new(15, 30);
        let merged = span1.merge(span2);
        assert_eq!(merged.start, 10);
        assert_eq!(merged.end, 30);
    }

    #[test]
    fn test_position_start() {
        let pos = Position::start();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 1);
        assert_eq!(pos.offset, 0);
    }

    #[test]
    fn test_position_new() {
        let pos = Position::new(5, 10, 100);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.column, 10);
        assert_eq!(pos.offset, 100);
    }

    #[test]
    fn test_position_next_char_regular() {
        let pos = Position::start();
        let next = pos.next_char('a', 8);
        assert_eq!(next.line, 1);
        assert_eq!(next.column, 2);
        assert_eq!(next.offset, 1);
    }

    #[test]
    fn test_position_next_char_newline() {
        let pos = Position::new(1, 5, 4);
        let next = pos.next_char('\n', 8);
        assert_eq!(next.line, 2);
        assert_eq!(next.column, 1);
        assert_eq!(next.offset, 5);
    }

    #[test]
    fn test_position_next_char_tab() {
        let pos = Position::new(1, 1, 0);
        let next = pos.next_char('\t', 8);
        assert_eq!(next.line, 1);
        assert_eq!(next.column, 9); // 次のタブ位置
        assert_eq!(next.offset, 1);
    }

    #[test]
    fn test_position_next_char_tab_mid_line() {
        let pos = Position::new(1, 5, 4);
        let next = pos.next_char('\t', 8);
        assert_eq!(next.line, 1);
        assert_eq!(next.column, 9); // 次のタブ位置
        assert_eq!(next.offset, 5);
    }

    #[test]
    fn test_position_next_char_multibyte() {
        let pos = Position::start();
        // マルチバイト文字のテスト（日本語「あ」 - 3バイト）
        let next = pos.next_char('あ', 8);
        assert_eq!(next.line, 1);
        assert_eq!(next.column, 2);
        assert_eq!(next.offset, 3);
    }

    #[test]
    fn test_position_to_span() {
        let pos = Position::new(1, 1, 10);
        let span = pos.to_span(20);
        assert_eq!(span.start, 10);
        assert_eq!(span.end, 30);
    }

    #[test]
    fn test_position_default() {
        let pos = Position::default();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 1);
        assert_eq!(pos.offset, 0);
    }
}
