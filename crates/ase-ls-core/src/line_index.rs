//! Line offset index for O(log n) byte offset ↔ position conversion.

use lsp_types::Position;

/// Pre-computed line offset index for O(log n) position conversion.
///
/// Builds a table of byte offsets for each line start, enabling
/// binary search instead of linear scan for offset→position and position→offset.
#[derive(Clone)]
pub struct LineIndex {
    /// Byte offset of each line start. line_offsets[i] = byte offset of line i.
    /// Always has at least one entry (offset 0 for line 0).
    line_offsets: Vec<u32>,
}

impl LineIndex {
    /// Build the line index from source text. O(n) construction.
    pub fn new(source: &str) -> Self {
        let mut offsets = vec![0u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                offsets.push((i + 1) as u32);
            }
        }
        Self {
            line_offsets: offsets,
        }
    }

    /// Convert a byte offset to (line, character), both 0-indexed. O(log n).
    pub fn offset_to_position(&self, offset: u32) -> (u32, u32) {
        let line = self.line_number(offset);
        let line_start = self.line_offsets[line as usize];
        let character = offset.saturating_sub(line_start);
        (line, character)
    }

    /// Convert an LSP Position to a byte offset. O(log n) + O(line_length).
    pub fn position_to_offset(&self, source: &str, position: Position) -> usize {
        let line = position.line as usize;
        if line >= self.line_offsets.len() {
            return source.len();
        }
        let char_offset = self.line_offsets[line] as usize;
        let chars_to_target = position.character as usize;
        let mut counted = 0;
        for c in source[char_offset..].chars() {
            if counted >= chars_to_target {
                return char_offset + counted;
            }
            counted += c.len_utf8();
        }
        char_offset + counted
    }

    /// Get the line number for a byte offset using binary search. O(log n).
    fn line_number(&self, offset: u32) -> u32 {
        let idx = self.line_offsets.partition_point(|&off| off <= offset);
        (idx as u32).saturating_sub(1)
    }

    /// Get the number of lines.
    pub fn line_count(&self) -> usize {
        self.line_offsets.len()
    }

    /// Get the byte offset of the start of a line. O(1).
    pub fn line_offset(&self, line: usize) -> usize {
        self.line_offsets.get(line).copied().unwrap_or(0) as usize
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_source() {
        let idx = LineIndex::new("");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.offset_to_position(0), (0, 0));
    }

    #[test]
    fn test_single_line() {
        let idx = LineIndex::new("SELECT * FROM users");
        assert_eq!(idx.offset_to_position(0), (0, 0));
        assert_eq!(idx.offset_to_position(6), (0, 6)); // after "SELECT"
        assert_eq!(idx.offset_to_position(18), (0, 18)); // end
    }

    #[test]
    fn test_two_lines() {
        let idx = LineIndex::new("SELECT *\nFROM users");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.offset_to_position(0), (0, 0));
        assert_eq!(idx.offset_to_position(8), (0, 8)); // before \n
        assert_eq!(idx.offset_to_position(9), (1, 0)); // start of line 2
        assert_eq!(idx.offset_to_position(14), (1, 5)); // "FROM " (5 chars)
    }

    #[test]
    fn test_many_lines() {
        let source = "line1\nline2\nline3\nline4\n";
        let idx = LineIndex::new(source);
        assert_eq!(idx.line_count(), 5); // 4 content lines + trailing newline creates 5th
        assert_eq!(idx.offset_to_position(0), (0, 0)); // "l"
        assert_eq!(idx.offset_to_position(6), (1, 0)); // start of "line2"
        assert_eq!(idx.offset_to_position(12), (2, 0)); // start of "line3"
        assert_eq!(idx.offset_to_position(18), (3, 0)); // start of "line4"
    }

    #[test]
    fn test_offset_beyond_end() {
        let idx = LineIndex::new("abc\n");
        // Offset beyond source length should still return valid position
        assert_eq!(idx.offset_to_position(100), (1, 96));
    }

    #[test]
    fn test_consecutive_newlines() {
        let idx = LineIndex::new("a\n\nb");
        assert_eq!(idx.line_count(), 3);
        assert_eq!(idx.offset_to_position(0), (0, 0)); // "a"
        assert_eq!(idx.offset_to_position(2), (1, 0)); // empty line
        assert_eq!(idx.offset_to_position(3), (2, 0)); // "b"
    }

    #[test]
    fn test_crlf_treated_as_two_lines() {
        // \r\n: \r is char, \n triggers new line
        let idx = LineIndex::new("a\r\nb");
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.offset_to_position(0), (0, 0)); // "a"
        assert_eq!(idx.offset_to_position(1), (0, 1)); // "\r"
        assert_eq!(idx.offset_to_position(2), (0, 2)); // "\n" byte
        assert_eq!(idx.offset_to_position(3), (1, 0)); // "b"
    }

    #[test]
    fn test_position_to_offset_single_line() {
        let source = "SELECT * FROM users";
        let idx = LineIndex::new(source);
        assert_eq!(idx.position_to_offset(source, Position::new(0, 0)), 0);
        assert_eq!(idx.position_to_offset(source, Position::new(0, 7)), 7);
        assert_eq!(idx.position_to_offset(source, Position::new(0, 19)), 19);
    }

    #[test]
    fn test_position_to_offset_multiline() {
        let source = "SELECT *\nFROM users";
        let idx = LineIndex::new(source);
        assert_eq!(idx.position_to_offset(source, Position::new(0, 0)), 0);
        assert_eq!(idx.position_to_offset(source, Position::new(1, 0)), 9);
        assert_eq!(idx.position_to_offset(source, Position::new(1, 5)), 14);
    }

    #[test]
    fn test_position_to_offset_beyond_end() {
        let source = "abc";
        let idx = LineIndex::new(source);
        // Line beyond end → source.len()
        assert_eq!(idx.position_to_offset(source, Position::new(5, 0)), 3);
        // Character beyond line → end of line
        assert_eq!(idx.position_to_offset(source, Position::new(0, 100)), 3);
    }

    #[test]
    fn test_consistency_with_old_implementation() {
        // Verify LineIndex produces same results as old offset_to_position
        let source = "CREATE TABLE users (\n  id INT,\n  name VARCHAR(100)\n)";
        let idx = LineIndex::new(source);

        for offset in 0..=source.len() {
            let (line, character) = idx.offset_to_position(offset as u32);
            let (old_line, old_char) = old_offset_to_position(source, offset as u32);
            assert_eq!(
                (line, character),
                (old_line, old_char),
                "Mismatch at offset {offset}"
            );
        }
    }

    #[test]
    fn test_consistency_position_to_offset() {
        let source = "CREATE TABLE users (\n  id INT,\n  name VARCHAR(100)\n)";
        let idx = LineIndex::new(source);

        for line in 0..4 {
            for col in 0..25 {
                let offset = idx.position_to_offset(source, Position::new(line, col));
                let old_offset = old_position_to_offset(source, Position::new(line, col));
                assert_eq!(offset, old_offset, "Mismatch at line {line}, col {col}");
            }
        }
    }

    /// Old implementation for comparison testing
    fn old_offset_to_position(source: &str, offset: u32) -> (u32, u32) {
        let mut line = 0u32;
        let mut last_newline = 0u32;
        let bytes = source.as_bytes();
        let end = (offset as usize).min(bytes.len());
        for (i, &b) in bytes.iter().enumerate().take(end) {
            if b == b'\n' {
                line += 1;
                last_newline = (i + 1) as u32;
            }
        }
        let character = offset.saturating_sub(last_newline);
        (line, character)
    }

    /// Old implementation for comparison testing
    fn old_position_to_offset(source: &str, position: Position) -> usize {
        let mut offset = 0;
        let mut current_line = 0u32;
        for ch in source.chars() {
            if current_line == position.line {
                let char_offset = offset;
                let chars_to_target = position.character as usize;
                let mut counted = 0;
                for c in source[char_offset..].chars() {
                    if counted >= chars_to_target {
                        return char_offset + counted;
                    }
                    counted += c.len_utf8();
                }
                return char_offset + counted;
            }
            offset += ch.len_utf8();
            if ch == '\n' {
                current_line += 1;
            }
        }
        offset
    }

    #[test]
    fn test_line_offset_method() {
        let source = "abc\ndef\nghi";
        let idx = LineIndex::new(source);
        assert_eq!(idx.line_offset(0), 0);
        assert_eq!(idx.line_offset(1), 4); // after "abc\n"
        assert_eq!(idx.line_offset(2), 8); // after "def\n"
        assert_eq!(idx.line_offset(99), 0); // out of range → 0
    }

    #[test]
    fn test_multibyte_position_to_offset() {
        // Japanese characters (3 bytes each in UTF-8)
        let source = "SELECT あ";
        let idx = LineIndex::new(source);
        // "SELECT " = 7 bytes, "あ" starts at byte 7
        assert_eq!(idx.position_to_offset(source, Position::new(0, 7)), 7);
        assert_eq!(idx.position_to_offset(source, Position::new(0, 8)), 10); // past "あ" (3 bytes)
    }

    #[test]
    fn test_position_to_offset_crlf_source() {
        let source = "abc\r\ndef";
        let idx = LineIndex::new(source);
        // Line 0: "abc\r\n" (5 bytes), Line 1: "def" starts at byte 5
        assert_eq!(idx.position_to_offset(source, Position::new(1, 0)), 5);
        assert_eq!(idx.position_to_offset(source, Position::new(1, 3)), 8);
    }
}
