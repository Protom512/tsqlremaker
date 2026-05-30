//! Folding Ranges 生成
//!
//! SQL コードの折りたたみ範囲を検出する。

use crate::line_index::LineIndex;
use lsp_types::{FoldingRange, FoldingRangeKind};
use tsql_lexer::Lexer;
use tsql_parser::ast::Statement;
use tsql_token::TokenKind;

/// ソースコードから Folding Ranges を生成する
pub fn folding_ranges(source: &str) -> Vec<FoldingRange> {
    let line_index = LineIndex::new(source);
    let mut ranges = Vec::new();

    // 1. ブロックコメントの折りたたみ
    ranges.extend(fold_comments(&line_index, source));

    // 2. BEGIN...END ブロックの折りたたみ
    ranges.extend(fold_begin_end(&line_index, source));

    ranges
}

/// Folding Ranges を DocumentAnalysis から生成する（AST対応版）
pub fn folding_ranges_with_analysis(
    analysis: &crate::analysis::DocumentAnalysis,
) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();

    // 1. Block comments (token-level)
    ranges.extend(fold_comments(&analysis.line_index, &analysis.source));

    // 2. AST-based folding: walk statements for foldable structures
    for stmt in &analysis.statements {
        collect_ast_folds(stmt, analysis, &mut ranges);
    }

    ranges
}

/// Recursively walk statements to find foldable regions.
fn collect_ast_folds(
    stmt: &Statement,
    analysis: &crate::analysis::DocumentAnalysis,
    ranges: &mut Vec<FoldingRange>,
) {
    match stmt {
        Statement::Block(block) => {
            let start = block.span.start as usize;
            let end = block.span.end as usize;
            add_fold_if_multiline(start, end, analysis, ranges);
            for child in &block.statements {
                collect_ast_folds(child, analysis, ranges);
            }
        }
        Statement::If(if_stmt) => {
            // Fold the entire IF...ELSE
            let start = if_stmt.span.start as usize;
            let end = if_stmt.span.end as usize;
            add_fold_if_multiline(start, end, analysis, ranges);
            // Also recurse into branches for nested folds
            collect_ast_folds(&if_stmt.then_branch, analysis, ranges);
            if let Some(else_branch) = &if_stmt.else_branch {
                collect_ast_folds(else_branch, analysis, ranges);
            }
        }
        Statement::While(while_stmt) => {
            let start = while_stmt.span.start as usize;
            let end = while_stmt.span.end as usize;
            add_fold_if_multiline(start, end, analysis, ranges);
            collect_ast_folds(&while_stmt.body, analysis, ranges);
        }
        Statement::TryCatch(try_catch) => {
            // Fold TRY block
            let try_start = try_catch.try_block.span.start as usize;
            let try_end = try_catch.try_block.span.end as usize;
            add_fold_if_multiline(try_start, try_end, analysis, ranges);
            for child in &try_catch.try_block.statements {
                collect_ast_folds(child, analysis, ranges);
            }
            // Fold CATCH block
            let catch_start = try_catch.catch_block.span.start as usize;
            let catch_end = try_catch.catch_block.span.end as usize;
            add_fold_if_multiline(catch_start, catch_end, analysis, ranges);
            for child in &try_catch.catch_block.statements {
                collect_ast_folds(child, analysis, ranges);
            }
        }
        Statement::Create(create) => {
            if let tsql_parser::ast::CreateStatement::Procedure(proc) = create.as_ref() {
                // Fold procedure body if multi-line
                let start = proc.span.start as usize;
                let end = proc.span.end as usize;
                add_fold_if_multiline(start, end, analysis, ranges);
            }
        }
        // For other statements with nested bodies, recurse into children
        Statement::Select(_)
        | Statement::Insert(_)
        | Statement::Update(_)
        | Statement::Delete(_)
        | Statement::Declare(_)
        | Statement::Set(_)
        | Statement::VariableAssignment(_)
        | Statement::Break(_)
        | Statement::Continue(_)
        | Statement::Return(_)
        | Statement::Transaction(_)
        | Statement::Throw(_)
        | Statement::Raiserror(_)
        | Statement::AlterTable(_)
        | Statement::Exec(_)
        | Statement::BatchSeparator(_) => {}
    }
}

fn add_fold_if_multiline(
    start_offset: usize,
    end_offset: usize,
    analysis: &crate::analysis::DocumentAnalysis,
    ranges: &mut Vec<FoldingRange>,
) {
    let end = if end_offset == 0 || end_offset <= start_offset {
        // Parser produced broken span — fall back to token-based end detection.
        // Find the last token whose start is >= start_offset.
        let last_token = analysis
            .tokens
            .iter()
            .rev()
            .find(|t| t.span.start as usize >= start_offset);
        match last_token {
            Some(t) => t.span.end as usize,
            None => return,
        }
    } else {
        end_offset
    };

    let (start_line, _) = analysis.line_index.offset_to_position(start_offset as u32);
    let (end_line, _) = analysis.line_index.offset_to_position(end as u32);

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

/// ブロックコメントの折りたたみ範囲を検出
fn fold_comments(line_index: &LineIndex, source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let lexer = Lexer::new(source).with_comments(true);

    for token_result in lexer {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };

        if token.kind == TokenKind::BlockComment {
            let (start_line, _) = line_index.offset_to_position(token.span.start);
            let (end_line, _) = line_index.offset_to_position(token.span.end.saturating_sub(1));
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
fn fold_begin_end(line_index: &LineIndex, source: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let lexer = Lexer::new(source);
    let tokens: Vec<_> = lexer.filter_map(Result::ok).collect();

    let mut begin_stack: Vec<(u32, u32)> = Vec::new(); // (line, offset)

    for token in &tokens {
        match token.kind {
            TokenKind::Begin => {
                let (line, _) = line_index.offset_to_position(token.span.start);
                begin_stack.push((line, token.span.start));
            }
            TokenKind::End => {
                if let Some((start_line, _)) = begin_stack.pop() {
                    let (end_line, _) = line_index.offset_to_position(token.span.end);
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

    // === AST folding tests (#76) ===

    fn make_analysis(source: &str) -> crate::analysis::DocumentAnalysis {
        crate::analysis::DocumentAnalysis::new(source)
    }

    fn count_region_folds(ranges: &[FoldingRange]) -> usize {
        ranges
            .iter()
            .filter(|r| r.kind == Some(FoldingRangeKind::Region))
            .count()
    }

    #[test]
    fn test_ast_fold_if_else_no_begin() {
        let source = "IF 1 = 1\n    SELECT 1\nELSE\n    SELECT 2";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 1,
            "IF/ELSE without BEGIN should produce at least 1 region fold via AST, got {region_folds}"
        );
    }

    #[test]
    fn test_ast_fold_while_no_begin() {
        let source = "WHILE @count < 10\n    SELECT @count";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 1,
            "WHILE without BEGIN should produce at least 1 region fold via AST, got {region_folds}"
        );
    }

    #[test]
    fn test_ast_fold_if_multiline() {
        let source = "IF @x > 0\n    INSERT INTO t VALUES (@x)\n    DELETE FROM t WHERE id = @x";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 1,
            "Multi-line IF body should be foldable via AST, got {region_folds}"
        );
    }

    #[test]
    fn test_ast_fold_if_else_with_begin_extra_fold() {
        let source = "IF 1 = 1\nBEGIN\n    SELECT 1\nEND\nELSE\nBEGIN\n    SELECT 2\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 3,
            "IF/ELSE with BEGIN should produce 3+ folds (IF + 2 BEGIN blocks), got {region_folds}"
        );
    }

    #[test]
    fn test_ast_fold_preserves_comment_folds() {
        let source = "/* multi-line\n   comment */\nSELECT 1";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let comment_folds: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == Some(FoldingRangeKind::Comment))
            .collect();
        assert_eq!(comment_folds.len(), 1, "comment folds should be preserved");
    }
}
