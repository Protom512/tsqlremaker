//! 文字カーソル
//!
//! 入力文字列を文字単位で走査するカーソルを提供する。

use tsql_token::Position;

/// 文字カーソル
///
/// 入力文字列の現在位置を追跡し、文字レベルの操作を提供する。
pub struct Cursor<'src> {
    input: &'src str,
    chars: std::str::CharIndices<'src>,
    current: Option<(usize, char)>,
    position: Position,
    tab_width: u32,
}

impl<'src> Cursor<'src> {
    /// 新しい Cursor を作成する
    ///
    /// # Arguments
    ///
    /// * `input` - 走査する入力文字列
    #[must_use]
    pub fn new(input: &'src str) -> Self {
        let mut chars = input.char_indices();
        let current = chars.next();
        Self {
            input,
            chars,
            current,
            position: Position::start(),
            tab_width: 8,
        }
    }

    /// 現在の文字を取得する
    #[must_use]
    pub const fn current(&self) -> Option<char> {
        match self.current {
            Some((_, ch)) => Some(ch),
            None => None,
        }
    }

    /// 次の文字を先読みする（消費しない）
    #[must_use]
    pub fn peek(&self) -> Option<char> {
        self.chars.clone().next().map(|(_, ch)| ch)
    }

    /// 2文字先を先読みする
    #[must_use]
    pub fn peek2(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.next().map(|(_, ch)| ch)
    }

    /// 次の文字に進む
    ///
    /// 現在の文字を返し、内部位置を更新する。
    /// 改行文字の場合は行番号と列番号を適切に更新する。
    pub fn bump(&mut self) -> Option<char> {
        let (idx, ch) = self.current?;
        self.current = self.chars.next();

        // 位置情報の更新
        if ch == '\r' {
            // CRLF のチェック
            if self.current.map(|(_, c)| c) == Some('\n') {
                let _ = self.current.take();
                self.current = self.chars.next();
            }
            self.position.line += 1;
            self.position.column = 1;
        } else if ch == '\n' {
            self.position.line += 1;
            self.position.column = 1;
        } else if ch == '\t' {
            // タブ幅を8スペースとして計算
            let next_tab = ((self.position.column - 1 + self.tab_width) / self.tab_width)
                * self.tab_width + 1;
            self.position.column = next_tab;
        } else {
            self.position.column += 1;
        }

        self.position.offset = idx as u32 + ch.len_utf8() as u32;

        Some(ch)
    }

    /// 現在の位置を取得する
    #[must_use]
    pub const fn position(&self) -> Position {
        self.position
    }

    /// EOF かどうかを判定する
    #[must_use]
    pub fn is_eof(&self) -> bool {
        self.current.is_none()
    }

    /// 残りの入力文字列を取得する
    #[must_use]
    pub fn rest(&self) -> &'src str {
        match self.current {
            Some((idx, _)) => &self.input[idx..],
            None => "",
        }
    }

    /// タブ幅を設定する
    ///
    /// # Arguments
    ///
    /// * `width` - タブ幅（デフォルトは8）
    pub fn set_tab_width(&mut self, width: u32) {
        self.tab_width = width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_new() {
        let cursor = Cursor::new("hello");
        assert_eq!(cursor.current(), Some('h'));
        assert_eq!(cursor.position().line, 1);
        assert_eq!(cursor.position().column, 1);
    }

    #[test]
    fn test_cursor_bump() {
        let mut cursor = Cursor::new("hello");
        assert_eq!(cursor.bump(), Some('h'));
        assert_eq!(cursor.current(), Some('e'));
        assert_eq!(cursor.position().column, 2);
    }

    #[test]
    fn test_cursor_bump_all() {
        let mut cursor = Cursor::new("hi");
        assert_eq!(cursor.bump(), Some('h'));
        assert_eq!(cursor.bump(), Some('i'));
        assert_eq!(cursor.bump(), None);
        assert!(cursor.is_eof());
    }

    #[test]
    fn test_cursor_peek() {
        let cursor = Cursor::new("hello");
        assert_eq!(cursor.peek(), Some('e'));
        assert_eq!(cursor.current(), Some('h')); // Unchanged
    }

    #[test]
    fn test_cursor_peek2() {
        let cursor = Cursor::new("hello");
        assert_eq!(cursor.peek2(), Some('l'));
        assert_eq!(cursor.current(), Some('h')); // Unchanged
    }

    #[test]
    fn test_cursor_newline() {
        let mut cursor = Cursor::new("h\ni");
        assert_eq!(cursor.bump(), Some('h'));
        assert_eq!(cursor.position().line, 1);
        assert_eq!(cursor.bump(), Some('\n'));
        assert_eq!(cursor.position().line, 2);
        assert_eq!(cursor.position().column, 1);
    }

    #[test]
    fn test_cursor_crlf() {
        let mut cursor = Cursor::new("h\r\ni");
        assert_eq!(cursor.bump(), Some('h'));
        assert_eq!(cursor.bump(), Some('\r')); // Should consume both \r\n
        assert_eq!(cursor.position().line, 2);
        assert_eq!(cursor.position().column, 1);
        assert_eq!(cursor.current(), Some('i'));
    }

    #[test]
    fn test_cursor_tab() {
        let mut cursor = Cursor::new("\tx");
        assert_eq!(cursor.bump(), Some('\t'));
        assert_eq!(cursor.position().column, 9); // Tab stop at 9
    }

    #[test]
    fn test_cursor_rest() {
        let cursor = Cursor::new("hello");
        assert_eq!(cursor.rest(), "hello");
    }

    #[test]
    fn test_cursor_rest_after_bump() {
        let mut cursor = Cursor::new("hello");
        cursor.bump();
        assert_eq!(cursor.rest(), "ello");
    }

    #[test]
    fn test_cursor_set_tab_width() {
        let mut cursor = Cursor::new("\tx");
        cursor.set_tab_width(4);
        assert_eq!(cursor.bump(), Some('\t'));
        assert_eq!(cursor.position().column, 5); // Tab stop at 5
    }
}
