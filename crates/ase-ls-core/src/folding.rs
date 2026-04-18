//! Folding Ranges 生成
//!
//! SQL コードの折りたたみ範囲を検出する。

use crate::offset_to_position;
use lsp_types::{FoldingRange, FoldingRangeKind};
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// ソースコードから Folding Ranges を生成する
pub fn folding_ranges(source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();

    // 1. ブロックコメントの折りたたみ
    ranges.extend(fold_comments(source));

    // 2. BEGIN...END ブロックの折りたたみ
    ranges.extend(fold_begin_end(source));

    ranges
}

/// ブロックコメントの折りたたみ範囲を検出
fn fold_comments(source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let lexer = Lexer::new(source).with_comments(true);

    for token_result in lexer {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };

        if token.kind == TokenKind::BlockComment {
            let (start_line, _) = offset_to_position(source, token.span.start);
            let (end_line, _) = offset_to_position(source, token.span.end.saturating_sub(1));
            if start_line < end_line {
                ranges.push(FoldingRange {
                    start_line,
                    start_character: None,
                    end_line,
                    end_character: None,
                    kind: Some(FoldingRangeKind::Comment),
                    collapsed_text: None,
                });
            }
        }
    }

    ranges
}

/// BEGIN...END ブロックの折りたたみ範囲を検出
fn fold_begin_end(source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let lexer = Lexer::new(source);
    let tokens: Vec<_> = lexer.filter_map(Result::ok).collect();

    let mut begin_stack: Vec<(u32, u32)> = Vec::new(); // (line, offset)

    for token in &tokens {
        match token.kind {
            TokenKind::Begin => {
                let (line, _) = offset_to_position(source, token.span.start);
                begin_stack.push((line, token.span.start));
            }
            TokenKind::End => {
                if let Some((start_line, _)) = begin_stack.pop() {
                    let (end_line, _) = offset_to_position(source, token.span.end);
                    if start_line < end_line {
                        ranges.push(FoldingRange {
                            start_line,
                            start_character: None,
                            end_line,
                            end_character: None,
                            kind: Some(FoldingRangeKind::Region),
                            collapsed_text: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    ranges
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_no_folds_single_line() {
        let ranges = folding_ranges("SELECT * FROM users");
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_begin_end_fold() {
        let source = "BEGIN\n  SELECT 1;\n  SELECT 2;\nEND";
        let ranges = folding_ranges(source);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 3);
        assert_eq!(ranges[0].kind, Some(FoldingRangeKind::Region));
    }

    #[test]
    fn test_nested_begin_end() {
        let source = "BEGIN\n  BEGIN\n    SELECT 1;\n  END\nEND";
        let ranges = folding_ranges(source);
        // Inner BEGIN...END spans lines 1-3, outer spans 0-4
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn test_comment_fold() {
        let source = "/* This is a\n   multi-line comment */\nSELECT 1";
        let ranges = folding_ranges(source);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].kind, Some(FoldingRangeKind::Comment));
    }

    #[test]
    fn test_single_line_comment_no_fold() {
        let source = "/* single line */ SELECT 1";
        let ranges = folding_ranges(source);
        // Single line comment should not produce a fold
        let comment_folds: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == Some(FoldingRangeKind::Comment))
            .collect();
        assert!(comment_folds.is_empty());
    }
}
