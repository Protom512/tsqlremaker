//! # Span Resolution Helpers
//!
//! パーサーが複数行プロシージャ等で出力する不正な span を補うための span 終端解決ヘルパ群。
//!
//! パーサーの span には2種類の不正さがある:
//! 1. **broken span** — `span = { start: 0, end: 0 }`（多文プロシージャ等で頻発）
//! 2. **overshooting span** — `span.end` が次の文にまで食い込む
//!    （例: `IF ... END\nSELECT 99` で IF の span.end が `SELECT 99` まで伸びる）
//!
//! 両方を堅牢に扱うため、`resolve_stmt_end` は次の3段戦略を採用する:
//!
//! 1. **AST span** — `span.end > span.start` なら採用。ただし depth-aware トークンスキャンで
//!    より手前に対応する `END` が見つかれば、そちらで **clamp**（食い込み防止）する。
//! 2. **子文 tail span** — Block/If/While/TryCatch/Procedure/Trigger の子構造の末尾 span。
//!    同様に clamp する。
//! 3. **depth-aware token scan** — `resolve_span_end_fallback` で `start_offset` 以降の最初の
//!    `BEGIN` を探し、対応する `END` の終端オフセットを返す。
//!
//! これは `architecture-coupling-balance.md` の「Functional coupling: same context」行に
//! 基づき、同一クレート内の `pub(crate)` 関数として提供されるコントラクト層である。

#![allow(clippy::module_inception)]

use crate::analysis::DocumentAnalysis;
use tsql_parser::ast::{Block, CreateStatement, Statement};
use tsql_parser::AstNode;

/// トークンスキャンの走査上限（`start_offset` からの相対バイト数）。
///
/// これを超えると走査を打ち切り `None` を返す。無限/全走査を避け、単一構文要素の
/// 範囲解決に十分な広さを確保する。
const SCAN_LIMIT: usize = 5000;

/// Resolve potentially broken span.end using depth-aware forward token scan.
///
/// `start_offset` 以降のトークンを走査し、**最初の `BEGIN`** を見つけて depth=1 とし、
/// その後の `BEGIN`/`END` のネストを追跡して対応する `END` の終端オフセットを返す。
/// `start_offset` が `BEGIN` トークン上にある場合はそれを開始点とする。
/// 対応する `END` が見つからない場合は `None` を返す。
///
/// Returns `Some(end_offset)` when the matching END token is found, `None` otherwise.
pub(crate) fn resolve_span_end_fallback(
    start_offset: usize,
    analysis: &DocumentAnalysis,
) -> Option<usize> {
    let mut depth = 0u32;
    let mut found_begin = false;
    for t in &analysis.tokens {
        let ts = t.span.start as usize;
        if ts < start_offset {
            continue;
        }
        if ts > start_offset.saturating_add(SCAN_LIMIT) {
            break;
        }
        let te = t.span.end as usize;
        if !found_begin {
            // start_offset を含むトークン、または start_offset 以降の最初の BEGIN トークン。
            let covers_start = ts <= start_offset && te > start_offset;
            let is_begin_after = ts >= start_offset && t.text.eq_ignore_ascii_case("BEGIN");
            if covers_start {
                // start_offset 上のトークン。BEGIN ならそこから、そうでなければ
                // 以降の最初の BEGIN を待つ（depth はまだ 0）。
                if t.text.eq_ignore_ascii_case("BEGIN") {
                    found_begin = true;
                    depth = 1;
                }
                continue;
            } else if is_begin_after {
                found_begin = true;
                depth = 1;
                continue;
            }
            continue;
        }
        if t.text.eq_ignore_ascii_case("BEGIN") {
            depth = depth.saturating_add(1);
        } else if t.text.eq_ignore_ascii_case("END") {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(te);
            }
        }
    }
    None
}

/// Clamp a candidate end offset to the matching END found by depth-aware scan.
///
/// AST span や子文 span が次の文に食い込む（overshooting）場合、`start_offset` から
/// トークンスキャンして対応する `END` を探し、より手前にあればそちらを採用する。
/// スキャンが `None`（BEGIN/END なし、または非BEGIN構文）なら `candidate` をそのまま返す。
fn clamp_end_to_scan(start_offset: usize, candidate: usize, analysis: &DocumentAnalysis) -> usize {
    match resolve_span_end_fallback(start_offset, analysis) {
        Some(scan_end) if scan_end < candidate => scan_end,
        _ => candidate,
    }
}

/// Resolve the end offset for a [`Block`], using span.end when valid,
/// falling back to child-statement spans, and finally to depth-aware token scan.
///
/// Block 専用の3段フォールバック:
/// 1. Block 自身の `span.end > span.start` なら採用（overshoot は clamp）
/// 2. 子文リストの末尾 `Statement::span()` が有効なら採用（overshoot は clamp）
/// 3. `resolve_span_end_fallback` へフォールバック
pub(crate) fn resolve_block_end(block: &Block, analysis: &DocumentAnalysis) -> Option<usize> {
    let span = block.span;
    let start = span.start as usize;
    if span.end > span.start {
        return Some(clamp_end_to_scan(start, span.end as usize, analysis));
    }
    if let Some(last) = block.statements.last() {
        let s = last.span();
        if s.end > s.start {
            return Some(clamp_end_to_scan(start, s.end as usize, analysis));
        }
    }
    resolve_span_end_fallback(start, analysis)
}

/// Resolve the end offset of a statement using a child-first 3-tier strategy.
///
/// 文の種類に応じて子構造の tail span を再帰的に参照する:
/// - `Block`: 子文末尾 span
/// - `If`: `max(then_branch, else_branch)` の tail span
/// - `While`: body の tail span
/// - `TryCatch`: `max(try_block, catch_block)` の tail span（`resolve_block_end` 経由）
/// - `Create(Procedure|Trigger)`: `body.last().span()`（※ `proc.span()` は broken のため使用しない）
///
/// 戦略（順序）:
/// 1. **子構造の tail span** — 複合文の子を再帰解決した結果。**depth-scan で clamp しない**。
///    scan は `IF...THEN..END` と `IF...ELSE..END` を区別できず、then 側の END で打ち切って
///    ELSE ブロックを切り捨ててしまうため（Issue #135 ゲート BI-1）。子構造 span は精密。
/// 2. **AST span** — 単純文・子構造未解決時。有効なら採用（overshoot は clamp）。
/// 3. **depth-aware token scan** — `resolve_span_end_fallback`。
///
/// いずれの tier でも解決できない場合は `None` を返す。
pub(crate) fn resolve_stmt_end(stmt: &Statement, analysis: &DocumentAnalysis) -> Option<usize> {
    let span = stmt.span();
    let start = span.start as usize;

    // Tier-1: 子構造の tail span（複合文）。再帰結果は clamp しない（BI-1）。
    let tier_children: Option<usize> = match stmt {
        Statement::Block(block) => block
            .statements
            .last()
            .map(|s| s.span())
            .filter(|s| s.end > s.start)
            .map(|s| s.end as usize),
        Statement::If(if_stmt) => {
            // then_branch と else_branch の tail（再帰）の大きい方。ELSE があればそれが終端。
            let then_end = resolve_stmt_end(&if_stmt.then_branch, analysis);
            let else_end = if_stmt
                .else_branch
                .as_ref()
                .and_then(|e| resolve_stmt_end(e, analysis));
            match (then_end, else_end) {
                (Some(t), Some(e)) => Some(t.max(e)),
                (Some(t), None) | (None, Some(t)) => Some(t),
                (None, None) => None,
            }
        }
        Statement::While(while_stmt) => resolve_stmt_end(&while_stmt.body, analysis),
        Statement::TryCatch(try_catch) => {
            // try_block と catch_block の tail の大きい方（resolve_block_end は Block 開始=BEGIN
            // を基準にするため IF のようなキーワード開始の clamp 問題は起きない）。
            let try_end = resolve_block_end(&try_catch.try_block, analysis);
            let catch_end = resolve_block_end(&try_catch.catch_block, analysis);
            match (try_end, catch_end) {
                (Some(t), Some(c)) => Some(t.max(c)),
                (Some(t), None) | (None, Some(t)) => Some(t),
                (None, None) => None,
            }
        }
        Statement::Create(create) => {
            let body: &[Statement] = match create.as_ref() {
                CreateStatement::Procedure(proc) => &proc.body,
                CreateStatement::Trigger(trigger) => &trigger.body,
                _ => return resolve_span_end_fallback(start, analysis),
            };
            body.last()
                .map(AstNode::span)
                .filter(|s| s.end > s.start)
                .map(|s| s.end as usize)
        }
        _ => None,
    };
    if let Some(end) = tier_children {
        return Some(end);
    }

    // Tier-2: AST span が有効なら採用（overshoot は clamp）。単純文・子構造未解決時。
    if span.end > span.start {
        return Some(clamp_end_to_scan(start, span.end as usize, analysis));
    }

    // Tier-3: depth-aware token scan
    resolve_span_end_fallback(start, analysis)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::analysis::DocumentAnalysis;
    use tsql_parser::AstNode;

    /// Build a DocumentAnalysis from source SQL, capturing the parser's real
    /// (possibly broken/overshooting) span output. This mirrors production behavior.
    fn analyze(source: &str) -> DocumentAnalysis {
        DocumentAnalysis::new(source)
    }

    // ===== resolve_span_end_fallback (depth-aware token scan) =====

    #[test]
    fn resolve_span_end_fallback_finds_matching_end_for_simple_block() {
        let analysis = analyze("BEGIN SELECT 1 END");
        let end = resolve_span_end_fallback(0, &analysis);
        assert!(end.is_some(), "should find END token");
        let end_off = end.unwrap();
        assert!(end_off <= analysis.source.len(), "end offset in bounds");
    }

    #[test]
    fn resolve_span_end_fallback_handles_nested_begin_end() {
        let analysis = analyze("BEGIN BEGIN SELECT 1 END END");
        let end = resolve_span_end_fallback(0, &analysis).expect("should resolve nested");
        assert_eq!(end, analysis.source.len());
    }

    #[test]
    fn resolve_span_end_fallback_returns_none_when_no_end() {
        let analysis = analyze("BEGIN SELECT 1");
        let end = resolve_span_end_fallback(0, &analysis);
        assert!(end.is_none(), "unterminated BEGIN yields None");
    }

    #[test]
    fn resolve_span_end_fallback_finds_first_begin_after_start_offset() {
        // start_offset is the CREATE keyword (offset 0), not a BEGIN.
        // The scan must locate the BEGIN that follows and match its END.
        let analysis = analyze("CREATE PROCEDURE p AS BEGIN SELECT 1 END");
        let end = resolve_span_end_fallback(0, &analysis).expect("should find BEGIN after CREATE");
        assert!(
            end <= analysis.source.len(),
            "resolved end within source bounds"
        );
    }

    #[test]
    fn resolve_span_end_fallback_returns_none_when_no_begin_at_all() {
        let analysis = analyze("SELECT 1 FROM t");
        let end = resolve_span_end_fallback(0, &analysis);
        assert!(end.is_none(), "no BEGIN/END yields None");
    }

    // ===== resolve_block_end (Block-specific 3-tier) =====

    #[test]
    fn resolve_block_end_uses_ast_span_when_valid() {
        let analysis = analyze("DECLARE @i INT\nBEGIN\n    SET @i = 1\nEND\n");
        let block_stmt = analysis.statements.iter().find_map(|s| match s {
            Statement::Block(b) => Some(b),
            _ => None,
        });
        let block = block_stmt.expect("test fixture should contain a Block");
        let end = resolve_block_end(block, &analysis).expect("should resolve block end");
        assert!(end > block.span.start as usize);
    }

    #[test]
    fn resolve_block_end_falls_back_to_depth_scan_when_span_broken() {
        let analysis = analyze("BEGIN SELECT 1 END");
        let broken_block = Block {
            span: tsql_token::Span { start: 0, end: 0 },
            statements: vec![],
        };
        let end = resolve_block_end(&broken_block, &analysis);
        assert!(end.is_some(), "fallback should find END via token scan");
    }

    // ===== resolve_stmt_end (generic 3-tier with overshoot clamp) =====

    #[test]
    fn resolve_stmt_end_returns_ast_span_when_valid() {
        let analysis = analyze("BEGIN SELECT 1 END");
        let valid = Statement::Break(Box::new(tsql_parser::ast::BreakStatement {
            span: tsql_token::Span { start: 5, end: 20 },
        }));
        let end = resolve_stmt_end(&valid, &analysis).expect("valid span must resolve via tier-1");
        // Break has no BEGIN/END near offset 5, so clamp is a no-op → returns span.end.
        assert_eq!(
            end, 20,
            "tier-1 returns span.end unchanged when no clamp applies"
        );
    }

    #[test]
    fn resolve_stmt_end_resolves_create_procedure_via_body_tail() {
        let sql = "CREATE PROCEDURE my_proc AS\nBEGIN\n    SELECT 1\n    SELECT 2\nEND\n";
        let analysis = analyze(sql);
        let create_stmt = analysis
            .statements
            .iter()
            .find(|s| matches!(s, Statement::Create(_)))
            .expect("should parse CREATE PROCEDURE");

        let outer_span = create_stmt.span();
        let end = resolve_stmt_end(create_stmt, &analysis).expect("should resolve via body tail");
        assert!(
            end > outer_span.start as usize,
            "resolved end ({}) must exceed broken span start ({})",
            end,
            outer_span.start
        );
        assert!(end <= sql.len(), "resolved end within source bounds");
    }

    #[test]
    fn resolve_stmt_end_resolves_if_statement_via_branch_tail() {
        let sql = "IF 1 = 1\nBEGIN\n    SELECT 1\nEND\n";
        let analysis = analyze(sql);
        let if_stmt = analysis
            .statements
            .iter()
            .find(|s| matches!(s, Statement::If(_)))
            .expect("should parse IF");
        let end = resolve_stmt_end(if_stmt, &analysis).expect("should resolve If via branch tail");
        assert!(end > if_stmt.span().start as usize);
        assert!(end <= sql.len());
    }

    #[test]
    fn resolve_stmt_end_resolves_if_with_else_via_else_branch_tail() {
        let sql = "IF 1 = 1\nBEGIN\n    SELECT 1\nEND\nELSE\nBEGIN\n    SELECT 2\nEND\n";
        let analysis = analyze(sql);
        let if_stmt = analysis
            .statements
            .iter()
            .find(|s| matches!(s, Statement::If(_)))
            .expect("should parse IF...ELSE");
        let end = resolve_stmt_end(if_stmt, &analysis).expect("should resolve If/Else");
        assert!(end > if_stmt.span().start as usize);
        assert!(end <= sql.len());
    }

    #[test]
    fn resolve_stmt_end_resolves_while_via_body_tail() {
        let sql = "WHILE 1 = 1\nBEGIN\n    SELECT 1\nEND\n";
        let analysis = analyze(sql);
        let while_stmt = analysis
            .statements
            .iter()
            .find(|s| matches!(s, Statement::While(_)))
            .expect("should parse WHILE");
        let end =
            resolve_stmt_end(while_stmt, &analysis).expect("should resolve While via body tail");
        assert!(end > while_stmt.span().start as usize);
        assert!(end <= sql.len());
    }

    #[test]
    fn resolve_stmt_end_resolves_trycatch_via_catch_tail() {
        let sql = "BEGIN TRY\n    SELECT 1\nEND TRY\nBEGIN CATCH\n    SELECT 2\nEND CATCH\n";
        let analysis = analyze(sql);
        let tc_stmt = analysis
            .statements
            .iter()
            .find(|s| matches!(s, Statement::TryCatch(_)))
            .expect("should parse TRY...CATCH");
        let end = resolve_stmt_end(tc_stmt, &analysis).expect("should resolve TryCatch");
        assert!(end > tc_stmt.span().start as usize);
        assert!(end <= sql.len());
    }

    #[test]
    fn resolve_stmt_end_resolves_create_trigger_via_body_tail() {
        let sql = "CREATE TRIGGER my_trig ON my_table FOR INSERT\nAS\nBEGIN\n    SELECT 1\nEND\n";
        let analysis = analyze(sql);
        let create_stmt = analysis
            .statements
            .iter()
            .find(|s| matches!(s, Statement::Create(_)))
            .expect("should parse CREATE TRIGGER");
        let end = resolve_stmt_end(create_stmt, &analysis);
        if let Some(end_off) = end {
            assert!(end_off > 0);
            assert!(end_off <= sql.len());
        }
    }

    /// The decisive overshoot regression: an IF whose AST span.end extends into a
    /// trailing statement must be clamped to the IF's own END.
    #[test]
    fn resolve_stmt_end_clamps_if_overshooting_span_to_own_end() {
        // IF span from parser = {start:0, end:38} but the END is at offset ~26-29.
        // "SELECT 99" follows at offset ~30.
        let sql = "IF 1 = 1\nBEGIN\n    SELECT 1\nEND\nSELECT 99";
        let analysis = analyze(sql);
        let if_stmt = analysis
            .statements
            .iter()
            .find(|s| matches!(s, Statement::If(_)))
            .expect("should parse IF");
        let raw_end = if_stmt.span().end as usize;
        let end = resolve_stmt_end(if_stmt, &analysis).expect("should resolve");
        // Clamp must reduce the overshooting span end.
        assert!(
            end <= raw_end,
            "clamp must not increase the span end (raw={raw_end}, got={end})"
        );
        // The resolved end must fall before the trailing SELECT 99.
        let trailing_offset = sql.find("SELECT 99").unwrap();
        assert!(
            end <= trailing_offset,
            "resolved end ({end}) must not overshoot into trailing SELECT 99 (offset {trailing_offset})"
        );
    }

    /// Same overshoot clamp for CREATE PROCEDURE followed by trailing content.
    #[test]
    fn resolve_stmt_end_clamps_procedure_overshooting_span_to_own_end() {
        let sql = "CREATE PROCEDURE sp_t AS\nBEGIN\n    SELECT 1\nEND\nSELECT 99";
        let analysis = analyze(sql);
        let create = analysis
            .statements
            .iter()
            .find(|s| matches!(s, Statement::Create(_)))
            .expect("should parse CREATE PROCEDURE");
        let end = resolve_stmt_end(create, &analysis).expect("should resolve");
        let trailing_offset = sql.find("SELECT 99").unwrap();
        assert!(
            end <= trailing_offset,
            "resolved end ({end}) must not overshoot into trailing SELECT 99 (offset {trailing_offset})"
        );
    }

    #[test]
    fn resolve_stmt_end_returns_none_for_unresovable_broken_statement() {
        let analysis = analyze("BEGIN SELECT 1 END");
        let broken = Statement::Break(Box::new(tsql_parser::ast::BreakStatement {
            span: tsql_token::Span {
                start: 9999,
                end: 0,
            },
        }));
        let end = resolve_stmt_end(&broken, &analysis);
        assert!(end.is_none(), "unresolvable broken statement yields None");
    }
}
