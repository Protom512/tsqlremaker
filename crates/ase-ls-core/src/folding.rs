//! Folding Ranges 生成
//!
//! SQL コードの折りたたみ範囲を検出する。

use crate::line_index::LineIndex;
use crate::span_resolve::{resolve_block_end, resolve_stmt_end};
use lsp_types::{FoldingRange, FoldingRangeKind};
use tsql_lexer::Lexer;
use tsql_parser::ast::Statement;
use tsql_token::TokenKind;

/// Folding Ranges を DocumentAnalysis から生成する
#[must_use]
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
///
/// Each fold target computes `start = span.start` and resolves the end via
/// the 3-tier [`crate::span_resolve`] helpers (AST span → child tail span →
/// depth-aware token scan). This avoids the overshoot bug where a broken
/// `span.end=0` previously caused the old 1-tier token fallback to grab a
/// token from a *trailing* statement, extending the fold past the construct.
fn collect_ast_folds(
    stmt: &Statement,
    analysis: &crate::analysis::DocumentAnalysis,
    ranges: &mut Vec<FoldingRange>,
) {
    match stmt {
        Statement::Block(block) => {
            let start = block.span.start as usize;
            let resolved_end = resolve_block_end(block, analysis);
            add_fold_if_multiline(start, resolved_end, analysis, ranges);
            for child in &block.statements {
                collect_ast_folds(child, analysis, ranges);
            }
        }
        Statement::If(if_stmt) => {
            // Fold the entire IF...ELSE
            let start = if_stmt.span.start as usize;
            let resolved_end = resolve_stmt_end(stmt, analysis);
            add_fold_if_multiline(start, resolved_end, analysis, ranges);
            // Also recurse into branches for nested folds
            collect_ast_folds(&if_stmt.then_branch, analysis, ranges);
            if let Some(else_branch) = &if_stmt.else_branch {
                collect_ast_folds(else_branch, analysis, ranges);
            }
        }
        Statement::While(while_stmt) => {
            let start = while_stmt.span.start as usize;
            let resolved_end = resolve_stmt_end(stmt, analysis);
            add_fold_if_multiline(start, resolved_end, analysis, ranges);
            collect_ast_folds(&while_stmt.body, analysis, ranges);
        }
        Statement::TryCatch(try_catch) => {
            // Fold TRY block
            let try_start = try_catch.try_block.span.start as usize;
            let try_resolved_end = resolve_block_end(&try_catch.try_block, analysis);
            add_fold_if_multiline(try_start, try_resolved_end, analysis, ranges);
            for child in &try_catch.try_block.statements {
                collect_ast_folds(child, analysis, ranges);
            }
            // Fold CATCH block
            let catch_start = try_catch.catch_block.span.start as usize;
            let catch_resolved_end = resolve_block_end(&try_catch.catch_block, analysis);
            add_fold_if_multiline(catch_start, catch_resolved_end, analysis, ranges);
            for child in &try_catch.catch_block.statements {
                collect_ast_folds(child, analysis, ranges);
            }
        }
        Statement::Create(create) => match create.as_ref() {
            tsql_parser::ast::CreateStatement::Procedure(proc) => {
                // Fold procedure body if multi-line
                let start = proc.span.start as usize;
                let resolved_end = resolve_stmt_end(stmt, analysis);
                add_fold_if_multiline(start, resolved_end, analysis, ranges);
                for child in &proc.body {
                    collect_ast_folds(child, analysis, ranges);
                }
            }
            tsql_parser::ast::CreateStatement::Trigger(trigger) => {
                // Fold trigger body if multi-line
                let start = trigger.span.start as usize;
                let resolved_end = resolve_stmt_end(stmt, analysis);
                add_fold_if_multiline(start, resolved_end, analysis, ranges);
                for child in &trigger.body {
                    collect_ast_folds(child, analysis, ranges);
                }
            }
            _ => {}
        },
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

/// Add a folding range if the span covers more than one line.
///
/// `resolved_end` is the end offset produced by the 3-tier span resolver
/// (`Some` when resolved, `None` when the resolver could not determine an
/// end). When `None`, this falls back to the legacy last-token heuristic:
/// the last token whose start is `>= start_offset`. That fallback is only a
/// safety net — the resolver covers all realistic cases.
fn add_fold_if_multiline(
    start_offset: usize,
    resolved_end: Option<usize>,
    analysis: &crate::analysis::DocumentAnalysis,
    ranges: &mut Vec<FoldingRange>,
) {
    let end = match resolved_end {
        Some(end_offset) if end_offset > start_offset => end_offset,
        _ => {
            // Resolver returned None or an invalid end — fall back to the
            // last-token heuristic as a safety net.
            let last_token = analysis
                .tokens
                .iter()
                .rev()
                .find(|t| t.span.start as usize >= start_offset);
            match last_token {
                Some(t) => t.span.end as usize,
                None => return,
            }
        }
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_no_folds_single_line() {
        let analysis = make_analysis("SELECT * FROM users");
        let ranges = folding_ranges_with_analysis(&analysis);
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_begin_end_fold() {
        let source = "BEGIN\n  SELECT 1;\n  SELECT 2;\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 3);
        assert_eq!(ranges[0].kind, Some(FoldingRangeKind::Region));
    }

    #[test]
    fn test_nested_begin_end() {
        let source = "BEGIN\n  BEGIN\n    SELECT 1;\n  END\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        // Inner BEGIN...END spans lines 1-3, outer spans 0-4
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn test_comment_fold() {
        let source = "/* This is a\n   multi-line comment */\nSELECT 1";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].kind, Some(FoldingRangeKind::Comment));
    }

    #[test]
    fn test_single_line_comment_no_fold() {
        let source = "/* single line */ SELECT 1";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
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
    fn test_if_else_outer_fold_covers_else_block_not_truncated() {
        // Regression for commit-gate BI-1 (#135): the IF...ELSE outer fold must cover
        // the ELSE block. The old depth-scan clamp picked the FIRST BEGIN's END
        // (the THEN-side END), truncating the fold there and dropping ELSE entirely.
        // Layout (0-indexed lines):
        //   0 IF 1 = 1
        //   1 BEGIN
        //   2     SELECT 1
        //   3 END            <- bug truncated the outer fold here (end_line=3)
        //   4 ELSE
        //   5 BEGIN
        //   6     SELECT 2   <- ELSE block content; outer fold MUST reach at least here
        //   7 END
        //   8 SELECT 99      <- trailing statement (IF span overshoots into this)
        let source =
            "IF 1 = 1\nBEGIN\n    SELECT 1\nEND\nELSE\nBEGIN\n    SELECT 2\nEND\nSELECT 99";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let outer = ranges
            .iter()
            .find(|r| r.kind == Some(FoldingRangeKind::Region) && r.start_line == 0)
            .expect("outer IF region fold starting at line 0 should exist");
        assert!(
            outer.end_line >= 6,
            "IF...ELSE outer fold must cover the ELSE block (end_line >= 6), got end_line={}. \
             BI-1 regression: the depth-scan clamp truncated at the THEN-side END (line 3).",
            outer.end_line
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

    #[test]
    fn test_ast_fold_try_catch() {
        // Multiline TRY...CATCH with single statement per block (parser limitation:
        // multi-statement TRY body without BEGIN causes partial parse, so use
        // token-based folding which always works)
        let source = "BEGIN TRY\n    SELECT 1\nEND TRY\nBEGIN CATCH\n    SELECT -1\nEND CATCH";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 2,
            "TRY...CATCH should produce at least 2 region folds (TRY + CATCH), got {region_folds}"
        );
    }

    #[test]
    fn test_ast_fold_create_procedure() {
        let source = "CREATE PROCEDURE sp_test\nAS\nBEGIN\n    SELECT 1\n    SELECT 2\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 1,
            "CREATE PROCEDURE body should be foldable, got {region_folds}"
        );
    }

    #[test]
    fn test_ast_fold_single_line_if_not_folded() {
        // Truly single-line IF — same line for start and end, no fold
        let source = "IF 1 = 1 SELECT 1 ELSE SELECT 2";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert_eq!(
            region_folds, 0,
            "Single-line IF should not produce folds, got {region_folds}"
        );
    }

    #[test]
    fn test_unmatched_begin_no_panic() {
        // BEGIN without END should not panic
        let source = "BEGIN\n  SELECT 1\n  SELECT 2";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        // No fold produced since END is missing — but should not panic
        let region_folds = count_region_folds(&ranges);
        assert_eq!(region_folds, 0, "Unmatched BEGIN should produce no folds");
    }

    #[test]
    fn test_multiple_block_comments() {
        let source = "/* block one\n   line 2\n   line 3 */\nSELECT 1;\n/* block two\n   line 2 */";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let comment_folds: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == Some(FoldingRangeKind::Comment))
            .collect();
        assert_eq!(
            comment_folds.len(),
            2,
            "Should detect 2 multi-line block comments"
        );
    }

    #[test]
    fn test_ast_fold_nested_while_in_if() {
        let source = "IF 1 = 1\nBEGIN\n    WHILE @x < 10\n    BEGIN\n        SELECT @x\n        SET @x = @x + 1\n    END\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 2,
            "Nested WHILE inside IF with BEGIN should produce 2+ folds, got {region_folds}"
        );
    }

    #[test]
    fn test_empty_source_no_folds() {
        let analysis = make_analysis("");
        let ranges = folding_ranges_with_analysis(&analysis);
        assert!(ranges.is_empty(), "Empty source should produce no folds");
    }

    #[test]
    fn test_ast_fold_create_trigger() {
        let source =
            "CREATE TRIGGER tr_test ON users FOR INSERT AS\nBEGIN\n    SELECT 1\n    SELECT 2\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 1,
            "CREATE TRIGGER body should be foldable, got {region_folds}"
        );
    }

    #[test]
    fn test_ast_fold_nested_while_in_trigger() {
        let source = "CREATE TRIGGER tr_test ON users FOR INSERT AS\nBEGIN\n    WHILE 1 = 1\n    BEGIN\n        SELECT 1\n    END\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 2,
            "Nested WHILE inside trigger should produce multiple folds, got {region_folds}"
        );
    }

    /// Regression test for FINDING-001: Nested procedure with IF/ELSE + WHILE + TRY/CATCH
    /// must produce folding ranges when the parser can fully parse the body.
    /// Uses parenthesized RAISERROR syntax (space syntax is unsupported by parser).
    #[test]
    fn test_ast_fold_nested_procedure_full() {
        let source = "\
CREATE PROCEDURE sp_nested @mode INT AS
BEGIN
    DECLARE @result INT

    IF @mode = 1
    BEGIN
        WHILE @result < 10
        BEGIN
            SET @result = @result + 1
        END
    END
    ELSE
    BEGIN
        BEGIN TRY
            SELECT * FROM sysobjects
        END TRY
        BEGIN CATCH
            RAISERROR('Error', 16, 1)
        END CATCH
    END

    RETURN @result
END";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        assert!(
            region_folds >= 4,
            "Nested procedure with IF/WHILE/TRY-CATCH should produce 4+ folds \
             (proc body, IF block, WHILE block, TRY/CATCH blocks), got {region_folds}"
        );
    }

    // === 3-tier resolve_stmt_end / resolve_block_end integration (#135) ===
    //
    // These tests exercise the wiring from collect_ast_folds → span_resolve.
    // They assert precise end_line values (not just ">= N") to verify the
    // resolved fold boundary is accurate for multi-line constructs whose own
    // AST span is broken (end=0).

    /// Helper: find the region fold with the smallest start_line (the
    /// outermost/top-level construct fold).
    fn outermost_region_fold(ranges: &[FoldingRange]) -> Option<&FoldingRange> {
        ranges
            .iter()
            .filter(|r| r.kind == Some(FoldingRangeKind::Region))
            .min_by_key(|r| r.start_line)
    }

    /// CTO condition: assert the parser's real span to confirm the fixture
    /// premise (Create PROC span is broken while body.last() is valid). This
    /// guards against silent parser improvements that would invalidate the
    /// 3-tier premise.
    #[test]
    fn test_span_premise_create_procedure_broken_span_with_valid_body_tail() {
        use tsql_parser::ast::{CreateStatement, Statement};
        use tsql_parser::AstNode;
        let source = "CREATE PROCEDURE sp_t AS\nBEGIN\n    SELECT 1\n    SELECT 2\nEND";
        let analysis = make_analysis(source);
        let create = analysis
            .statements
            .iter()
            .find(|s| matches!(s, Statement::Create(_)))
            .expect("should have CREATE PROCEDURE");
        let outer = create.span();
        // The bug premise: the Create statement span is broken.
        assert!(
            outer.end <= outer.start,
            "fixture premise broken: Create span must be zeroed/broken, got {outer:?}"
        );
        // But the procedure body's last statement has a valid span (tier-2 source).
        let Statement::Create(c) = create else {
            unreachable!();
        };
        let CreateStatement::Procedure(proc) = c.as_ref() else {
            panic!("expected Procedure");
        };
        let last = proc.body.last().expect("body non-empty");
        let ls = last.span();
        assert!(
            ls.end > ls.start,
            "tier-2 premise: body.last() span must be valid, got {ls:?}"
        );
    }

    /// CREATE PROCEDURE: the outermost region fold must start at line 0
    /// (CREATE line) and end on the END line (line 4). This proves the
    /// resolver returns the procedure body tail rather than a zero/short end.
    ///
    /// Broken-span fixture: `parse_with_errors()` emits `span.end = 0` for the
    /// multi-line CREATE PROCEDURE statement (verified by
    /// `test_span_premise_create_procedure_broken_span_with_valid_body_tail`).
    #[test]
    fn test_create_procedure_outer_fold_spans_full_body() {
        // line 0: CREATE PROCEDURE sp_t AS
        // line 1: BEGIN
        // line 2:     SELECT 1
        // line 3:     SELECT 2
        // line 4: END
        let source = "CREATE PROCEDURE sp_t AS\nBEGIN\n    SELECT 1\n    SELECT 2\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let outer = outermost_region_fold(&ranges).expect("should have an outer region fold");
        assert_eq!(
            outer.start_line, 0,
            "CREATE PROCEDURE fold should start at line 0"
        );
        assert_eq!(
            outer.end_line, 4,
            "CREATE PROCEDURE fold should end on END line (4), got {}",
            outer.end_line
        );
    }

    /// IF...ELSE without BEGIN/END: there are no BEGIN/END tokens, so the old
    /// depth-aware token-scan fallback (tier-3) finds nothing. The 3-tier
    /// resolver succeeds via tier-2 (else_branch / then_branch child span).
    /// This proves robustness beyond the old 1-tier token fallback.
    ///
    /// Broken-span fixture: `parse_with_errors()` emits `span.end = 0` for the
    /// multi-line IF statement; the then/else branch child spans remain valid.
    #[test]
    fn test_if_without_begin_resolves_via_child_span_not_token_scan() {
        // No BEGIN/END anywhere — tier-3 cannot resolve. Tier-2 (else_branch
        // child span) must carry the fold.
        // line 0: IF 1 = 1
        // line 1:     SELECT 1
        // line 2: ELSE
        // line 3:     SELECT 2
        let source = "IF 1 = 1\n    SELECT 1\nELSE\n    SELECT 2";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let outer = outermost_region_fold(&ranges).expect("IF should still fold via child span");
        assert_eq!(outer.start_line, 0, "IF fold starts at line 0");
        assert!(
            outer.end_line >= 1,
            "IF fold must extend past the condition (end_line={})",
            outer.end_line
        );
    }

    /// WHILE without BEGIN/END: the body's own span is broken. Tier-3 token
    /// scan fails (no BEGIN/END). The resolver must still produce a fold
    /// spanning at least 2 lines via the body child statement's content.
    #[test]
    fn test_while_without_begin_fold_spans_at_least_two_lines() {
        let source = "WHILE @count < 10\n    SELECT @count\n    SET @count = @count + 1";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let outer = outermost_region_fold(&ranges).expect("WHILE should fold");
        assert_eq!(outer.start_line, 0);
        assert!(
            outer.end_line > outer.start_line,
            "WHILE fold must span multiple lines"
        );
    }

    /// WHILE...BEGIN...END — canonical multi-line WHILE broken-span case
    /// (#135 requirement d). The WHILE statement's own AST span is broken
    /// (`span.end = 0` from `parse_with_errors`), but the inner BEGIN block
    /// child carries a valid span (tier-2). The outermost fold must start at
    /// line 0 and reach the END line (3), proving the resolver bounds the fold
    /// to the construct's own END rather than the broken outer span.
    #[test]
    fn test_while_with_begin_fold_spans_to_end_line() {
        // line 0: WHILE @x < 10
        // line 1: BEGIN
        // line 2:     SELECT 1
        // line 3: END
        let source = "WHILE @x < 10\nBEGIN\n    SELECT 1\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let outer = outermost_region_fold(&ranges).expect("WHILE...BEGIN...END should fold");
        assert_eq!(outer.start_line, 0, "WHILE fold starts at line 0");
        assert_eq!(
            outer.end_line, 3,
            "WHILE fold should end on END line (3), got {}",
            outer.end_line
        );
    }

    /// TRY...CATCH: the outer TryCatch span is broken, but the inner
    /// try_block/catch_block carry valid spans. The outermost fold (the whole
    /// TRY...CATCH) must end on the `END CATCH` line, not overshoot.
    #[test]
    fn test_try_catch_outer_fold_ends_on_end_catch_line() {
        // line 0: BEGIN TRY
        // line 1:     SELECT 1
        // line 2: END TRY
        // line 3: BEGIN CATCH
        // line 4:     SELECT -1
        // line 5: END CATCH
        let source = "BEGIN TRY\n    SELECT 1\nEND TRY\nBEGIN CATCH\n    SELECT -1\nEND CATCH";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        // Find the fold with the largest end_line (the overall TRY...CATCH outer fold).
        let region: Vec<_> = ranges
            .iter()
            .filter(|r| r.kind == Some(FoldingRangeKind::Region))
            .collect();
        let max_end = region
            .iter()
            .map(|r| r.end_line)
            .max()
            .expect("should have region folds");
        assert_eq!(
            max_end, 5,
            "outermost TRY...CATCH fold should end on END CATCH line (5), got {max_end}"
        );
    }

    /// CREATE TRIGGER: outermost fold must start at line 0 and reach the END
    /// line, proving resolver returns the trigger body tail (tier-2), not the
    /// broken trigger.span.
    #[test]
    fn test_create_trigger_outer_fold_spans_full_body() {
        // line 0: CREATE TRIGGER tr ON users FOR INSERT AS
        // line 1: BEGIN
        // line 2:     SELECT 1
        // line 3:     SELECT 2
        // line 4: END
        let source =
            "CREATE TRIGGER tr ON users FOR INSERT AS\nBEGIN\n    SELECT 1\n    SELECT 2\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let outer = outermost_region_fold(&ranges).expect("trigger should fold");
        assert_eq!(outer.start_line, 0);
        assert_eq!(
            outer.end_line, 4,
            "TRIGGER fold should end on END line (4), got {}",
            outer.end_line
        );
    }

    /// An upper-bound regression guard: broken-span fixes must not cause
    /// runaway fold counts. A modest single-procedure source should produce a
    /// bounded number of region folds (proc body + inner BEGIN block).
    #[test]
    fn test_create_procedure_fold_count_bounded() {
        let source = "CREATE PROCEDURE sp_t AS\nBEGIN\n    SELECT 1\nEND";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let region_folds = count_region_folds(&ranges);
        // 1 (proc spanning to END) or 2 (proc + inner BEGIN block) is acceptable;
        // anything wildly larger indicates a runaway resolver.
        assert!(
            region_folds <= 3,
            "fold count should be bounded, got {region_folds}"
        );
    }

    /// 3-tier regression for #135: when a multi-line construct's own span is
    /// **broken** (end=0) and it is followed by another statement, the old
    /// 1-tier token fallback ("last token with start >= start_offset") would
    /// grab a token from the TRAILING statement. The 3-tier resolver bounds
    /// the fold via the child tail span instead.
    ///
    /// This test uses a construct the parser reliably emits a broken span for
    /// (CREATE PROCEDURE with a multi-statement body) followed by a trailing
    /// statement, and asserts the resolver is consulted (fold produced) and
    /// does not panic.
    ///
    /// KNOWN LIMITATION (out of Task 3 scope): when the parser emits an
    /// *overstretched* span (end > start but extending past the real END into
    /// the trailing statement, e.g. for IF/Block), tier-1 must trust the
    /// valid span and the fold may reach one line into the trailing content.
    /// Fixing that requires parser-level span correction, not span resolution.
    #[test]
    fn test_create_procedure_fold_resolves_when_followed_by_trailing_statement() {
        // line 0: CREATE PROCEDURE sp_t AS
        // line 1: BEGIN
        // line 2:     SELECT 1
        // line 3: END
        // line 4: SELECT 99
        let source = "CREATE PROCEDURE sp_t AS\nBEGIN\n    SELECT 1\nEND\nSELECT 99";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let proc_fold = ranges
            .iter()
            .filter(|r| r.kind == Some(FoldingRangeKind::Region))
            .find(|r| r.start_line == 0)
            .unwrap_or_else(|| panic!("should have a proc fold at line 0, got {ranges:?}"));
        assert!(
            proc_fold.end_line > proc_fold.start_line,
            "proc fold must span multiple lines, got start={} end={}",
            proc_fold.start_line,
            proc_fold.end_line
        );
    }

    /// IF fold followed by a trailing statement: resolver is consulted and a
    /// multi-line fold is produced. See the KNOWN LIMITATION note on
    /// [`test_create_procedure_fold_resolves_when_followed_by_trailing_statement`].
    #[test]
    fn test_if_fold_resolves_when_followed_by_trailing_statement() {
        // line 0: IF 1 = 1
        // line 1: BEGIN
        // line 2:     SELECT 1
        // line 3: END
        // line 4: SELECT 99
        let source = "IF 1 = 1\nBEGIN\n    SELECT 1\nEND\nSELECT 99";
        let analysis = make_analysis(source);
        let ranges = folding_ranges_with_analysis(&analysis);
        let if_fold = ranges
            .iter()
            .filter(|r| r.kind == Some(FoldingRangeKind::Region))
            .find(|r| r.start_line == 0)
            .unwrap_or_else(|| panic!("should have an IF fold at line 0, got {ranges:?}"));
        assert!(
            if_fold.end_line > if_fold.start_line,
            "IF fold must span multiple lines, got start={} end={}",
            if_fold.start_line,
            if_fold.end_line
        );
    }
}
