//! Semantic Tokens 生成
//!
//! Lexer のトークンストリームから LSP Semantic Tokens を生成する。

use crate::analysis::DocumentAnalysis;
use lsp_types::{
    Range, SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
    SemanticTokensRangeResult, SemanticTokensResult,
};
use tsql_parser::ast::{
    DeleteStatement, Expression, FunctionArg, FunctionCall, InList, InsertSource, InsertStatement,
    Join, OrderByItem, SelectItem, SelectStatement, Statement, TableReference, UpdateStatement,
};
use tsql_token::{Span, TokenKind};

/// CLASS semantic-token index (tables / views).
const TYPE_CLASS: u32 = 9;
/// FUNCTION semantic-token index (procedure / function calls).
const TYPE_FUNCTION: u32 = 2;

/// カスタムセマンティックトークンタイプの定義
#[must_use]
pub fn semantic_tokens_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,     // 0
            SemanticTokenType::TYPE,        // 1 - データ型
            SemanticTokenType::FUNCTION,    // 2
            SemanticTokenType::STRING,      // 3
            SemanticTokenType::NUMBER,      // 4
            SemanticTokenType::COMMENT,     // 5
            SemanticTokenType::VARIABLE,    // 6 - @変数
            SemanticTokenType::OPERATOR,    // 7
            SemanticTokenType::PARAMETER,   // 8 - プロシージャパラメータ
            SemanticTokenType::CLASS,       // 9 - テーブル名
            SemanticTokenType::ENUM_MEMBER, // 10 - ブール値リテラル
        ],
        token_modifiers: vec![],
    }
}

/// TokenKind → セマンティックトークンタイプインデックスのマッピング
const fn token_kind_to_type_index(kind: TokenKind) -> Option<u32> {
    match kind {
        // キーワード (0)
        _ if kind.is_keyword() => Some(0),
        // データ型 (1)
        TokenKind::Int
        | TokenKind::Integer
        | TokenKind::Smallint
        | TokenKind::Tinyint
        | TokenKind::Bigint
        | TokenKind::Real
        | TokenKind::Double
        | TokenKind::Decimal
        | TokenKind::Numeric
        | TokenKind::Money
        | TokenKind::Smallmoney
        | TokenKind::Char
        | TokenKind::Varchar
        | TokenKind::Text
        | TokenKind::Nchar
        | TokenKind::Nvarchar
        | TokenKind::Ntext
        | TokenKind::Unichar
        | TokenKind::Univarchar
        | TokenKind::Unitext
        | TokenKind::Binary
        | TokenKind::Varbinary
        | TokenKind::Image
        | TokenKind::Date
        | TokenKind::Time
        | TokenKind::Datetime
        | TokenKind::Smalldatetime
        | TokenKind::Timestamp
        | TokenKind::Bigdatetime
        | TokenKind::Bit
        | TokenKind::Uniqueidentifier => Some(1),
        // 文字列 (3)
        TokenKind::String
        | TokenKind::NString
        | TokenKind::UnicodeString
        | TokenKind::HexString => Some(3),
        // 数値 (4)
        TokenKind::Number | TokenKind::FloatLiteral => Some(4),
        // コメント (5)
        TokenKind::LineComment | TokenKind::BlockComment => Some(5),
        // 変数 (6)
        TokenKind::LocalVar | TokenKind::GlobalVar => Some(6),
        // 一時テーブル (9 = CLASS)
        TokenKind::TempTable | TokenKind::GlobalTempTable => Some(9),
        // 演算子 (7)
        TokenKind::Eq
        | TokenKind::Ne
        | TokenKind::NeAlt
        | TokenKind::Lt
        | TokenKind::Gt
        | TokenKind::Le
        | TokenKind::Ge
        | TokenKind::NotLt
        | TokenKind::NotGt
        | TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Percent
        | TokenKind::Ampersand
        | TokenKind::Pipe
        | TokenKind::Caret
        | TokenKind::Tilde
        | TokenKind::Assign
        | TokenKind::PlusAssign
        | TokenKind::MinusAssign
        | TokenKind::StarAssign
        | TokenKind::SlashAssign
        | TokenKind::Concat => Some(7),
        _ => None,
    }
}

/// Resolve a token's semantic type index, handling both direct kinds and symbol-table identifiers.
///
/// Resolution order (first match wins):
/// 1. `token_kind_to_type_index` — lexer-level kinds (keywords, types, vars, operators).
/// 2. AST-derived span index (authoritative for references): table refs / DML
///    targets → CLASS, function-call names → FUNCTION. Looked up by exact
///    byte-offset span equality (token.span == identifier.span).
/// 3. `symbol_table.resolve_semantic_type` — definition-site fallback for
///    CREATE TABLE/VIEW/PROCEDURE names already registered in the symbol table.
#[inline]
fn resolve_token_type(
    analysis: &DocumentAnalysis,
    span_index: &SemanticSpanIndex,
    kind: TokenKind,
    text: &str,
    span: Span,
) -> Option<u32> {
    token_kind_to_type_index(kind)
        .or_else(|| span_index.lookup(span))
        .or_else(|| {
            if kind == TokenKind::Ident {
                analysis.symbol_table.resolve_semantic_type(text)
            } else {
                None
            }
        })
}

/// Index of AST-derived `(span → semantic type)` entries for identifier
/// classification that the lexer/symbol-table layers cannot provide.
///
/// Covers:
/// - `TableReference` table names (SELECT FROM / JOIN / subquery-derived) → CLASS(9)
/// - DML target tables (`INSERT`/`DELETE` Identifier, `UPDATE` TableReference) → CLASS(9)
/// - `Expression::FunctionCall` names → FUNCTION(2)
///
/// Entries are sorted by `span.start` for O(log n) exact-span lookup. Broken
/// spans (`start >= end`, see MEMORY.md parser broken-span issue) are rejected
/// at insertion time so they can never shadow a real token.
struct SemanticSpanIndex {
    /// Entries sorted ascending by `span.start`.
    entries: Vec<(Span, u32)>,
}

impl SemanticSpanIndex {
    /// Build the index by walking `analysis.statements` once.
    fn build(analysis: &DocumentAnalysis) -> Self {
        let mut entries: Vec<(Span, u32)> = Vec::new();
        for stmt in &analysis.statements {
            walk_statement(stmt, &mut entries);
        }
        // Stable sort by start offset preserves determinism; duplicate spans
        // (e.g. the same identifier reached via two paths) are deduped on lookup
        // by returning the first match, which is fine since all duplicates of a
        // given span carry an identical identifier.
        entries.sort_by_key(|(span, _)| span.start);
        Self { entries }
    }

    /// O(log n) exact-span lookup. Returns the semantic type for a token whose
    /// span exactly equals an indexed identifier span, or `None`.
    #[inline]
    fn lookup(&self, span: Span) -> Option<u32> {
        // binary search by start offset
        let idx = self.entries.partition_point(|(s, _)| s.start < span.start);
        self.entries.get(idx).and_then(|(candidate, ty)| {
            // Exact byte-offset equality: both spans derive from the same OwnedToken
            // / AST source, so equality is reliable and avoids nested-construct
            // misclassification (per approval: no overlap heuristics).
            if *candidate == span {
                Some(*ty)
            } else {
                None
            }
        })
    }
}

/// Push an entry, guarding against the known parser broken-span issue
/// (`span.end == 0` or inverted spans). See MEMORY.md.
#[inline]
fn push_entry(entries: &mut Vec<(Span, u32)>, span: Span, ty: u32) {
    if span.start < span.end {
        entries.push((span, ty));
    }
}

// === AST walkers ===
//
// All walkers use only `pub use tsql_parser::ast::*` types (re-exported at the
// `ast` module root). Private submodules are never referenced, per
// project-ast-types.md.

fn walk_statement(stmt: &Statement, entries: &mut Vec<(Span, u32)>) {
    // Most variants wrap the inner node in `Box<T>`, so we bind the box and
    // dereference rather than destructuring the inner struct inline.
    match stmt {
        Statement::Select(s) => walk_select(s, entries),
        Statement::Insert(i) => walk_insert(i, entries),
        Statement::Update(u) => walk_update(u, entries),
        Statement::Delete(d) => walk_delete(d, entries),
        Statement::Set(s) => walk_expression(&s.value, entries),
        Statement::VariableAssignment(va) => {
            for a in &va.assignments {
                walk_expression(&a.value, entries);
            }
        }
        Statement::If(i) => {
            walk_expression(&i.condition, entries);
            walk_statement(&i.then_branch, entries);
            if let Some(els) = &i.else_branch {
                walk_statement(els, entries);
            }
        }
        Statement::While(w) => {
            walk_expression(&w.condition, entries);
            walk_statement(&w.body, entries);
        }
        Statement::Block(b) => {
            for s in &b.statements {
                walk_statement(s, entries);
            }
        }
        Statement::TryCatch(tc) => {
            for s in &tc.try_block.statements {
                walk_statement(s, entries);
            }
            for s in &tc.catch_block.statements {
                walk_statement(s, entries);
            }
        }
        Statement::Return(r) => {
            if let Some(expr) = &r.expression {
                walk_expression(expr, entries);
            }
        }
        Statement::Throw(t) => {
            if let Some(e) = &t.error_number {
                walk_expression(e, entries);
            }
            if let Some(e) = &t.message {
                walk_expression(e, entries);
            }
            if let Some(e) = &t.state {
                walk_expression(e, entries);
            }
        }
        Statement::Raiserror(r) => {
            walk_expression(&r.message, entries);
            if let Some(e) = &r.severity {
                walk_expression(e, entries);
            }
            if let Some(e) = &r.state {
                walk_expression(e, entries);
            }
        }
        // CREATE/ALTER/DECLARE/EXEC/etc. either register names via the symbol
        // table (already handled by resolve_semantic_type) or carry no
        // reference-side identifiers this index targets.
        _ => {}
    }
}

fn walk_select(sel: &SelectStatement, entries: &mut Vec<(Span, u32)>) {
    if let Some(top) = &sel.top {
        walk_expression(top, entries);
    }
    for item in &sel.columns {
        walk_select_item(item, entries);
    }
    if let Some(from) = &sel.from {
        for tr in &from.tables {
            walk_table_reference(tr, entries);
        }
        for join in &from.joins {
            walk_join(join, entries);
        }
    }
    if let Some(expr) = &sel.where_clause {
        walk_expression(expr, entries);
    }
    for expr in &sel.group_by {
        walk_expression(expr, entries);
    }
    if let Some(expr) = &sel.having {
        walk_expression(expr, entries);
    }
    for OrderByItem { expr, .. } in &sel.order_by {
        walk_expression(expr, entries);
    }
    if let Some(limit) = &sel.limit {
        walk_expression(&limit.limit, entries);
        if let Some(off) = &limit.offset {
            walk_expression(off, entries);
        }
    }
}

fn walk_select_item(item: &SelectItem, entries: &mut Vec<(Span, u32)>) {
    match item {
        SelectItem::Expression(expr, _alias) => walk_expression(expr, entries),
        SelectItem::QualifiedWildcard(_ident) => {}
        SelectItem::Wildcard => {}
    }
}

fn walk_table_reference(tr: &TableReference, entries: &mut Vec<(Span, u32)>) {
    match tr {
        TableReference::Table { name, .. } => {
            push_entry(entries, name.span, TYPE_CLASS);
        }
        TableReference::Subquery { query, .. } => walk_select(query, entries),
        TableReference::Joined { joins, .. } => {
            for join in joins {
                walk_join(join, entries);
            }
        }
    }
}

fn walk_join(join: &Join, entries: &mut Vec<(Span, u32)>) {
    walk_table_reference(&join.table, entries);
    if let Some(cond) = &join.on_condition {
        walk_expression(cond, entries);
    }
}

fn walk_insert(ins: &InsertStatement, entries: &mut Vec<(Span, u32)>) {
    push_entry(entries, ins.table.span, TYPE_CLASS);
    match &ins.source {
        InsertSource::Values(rows) => {
            for row in rows {
                for expr in row {
                    walk_expression(expr, entries);
                }
            }
        }
        InsertSource::Select(sel) => walk_select(sel, entries),
        InsertSource::DefaultValues => {}
    }
}

fn walk_update(upd: &UpdateStatement, entries: &mut Vec<(Span, u32)>) {
    walk_table_reference(&upd.table, entries);
    for a in &upd.assignments {
        walk_expression(&a.value, entries);
    }
    if let Some(from) = &upd.from_clause {
        for tr in &from.tables {
            walk_table_reference(tr, entries);
        }
        for join in &from.joins {
            walk_join(join, entries);
        }
    }
    if let Some(expr) = &upd.where_clause {
        walk_expression(expr, entries);
    }
}

fn walk_delete(del: &DeleteStatement, entries: &mut Vec<(Span, u32)>) {
    push_entry(entries, del.table.span, TYPE_CLASS);
    if let Some(from) = &del.from_clause {
        for tr in &from.tables {
            walk_table_reference(tr, entries);
        }
        for join in &from.joins {
            walk_join(join, entries);
        }
    }
    if let Some(expr) = &del.where_clause {
        walk_expression(expr, entries);
    }
}

fn walk_expression(expr: &Expression, entries: &mut Vec<(Span, u32)>) {
    match expr {
        Expression::FunctionCall(call) => {
            walk_function_call(call, entries);
        }
        Expression::UnaryOp { expr, .. } => walk_expression(expr, entries),
        Expression::BinaryOp { left, right, .. } => {
            walk_expression(left, entries);
            walk_expression(right, entries);
        }
        Expression::Case(case) => {
            for (when, then) in &case.branches {
                walk_expression(when, entries);
                walk_expression(then, entries);
            }
            if let Some(els) = &case.else_result {
                walk_expression(els, entries);
            }
        }
        Expression::Subquery(sel) | Expression::Exists(sel) => walk_select(sel, entries),
        Expression::In { expr, list, .. } => {
            walk_expression(expr, entries);
            match list {
                InList::Values(values) => {
                    for v in values {
                        walk_expression(v, entries);
                    }
                }
                InList::Subquery(sel) => walk_select(sel, entries),
            }
        }
        Expression::Between {
            expr, low, high, ..
        } => {
            walk_expression(expr, entries);
            walk_expression(low, entries);
            walk_expression(high, entries);
        }
        Expression::Like {
            expr,
            pattern,
            escape,
            ..
        } => {
            walk_expression(expr, entries);
            walk_expression(pattern, entries);
            if let Some(esc) = escape {
                walk_expression(esc, entries);
            }
        }
        Expression::Is { expr, .. } => walk_expression(expr, entries),
        // Literals / Identifier / ColumnReference carry no function-call or
        // table-reference information this index targets.
        Expression::Literal(_) | Expression::Identifier(_) | Expression::ColumnReference(_) => {}
    }
}

/// Classify a function-call name as FUNCTION(2).
///
/// T3 decision (Issue #133): every `Ident(args)` call site — i.e. every
/// `Expression::FunctionCall` name — is classified as FUNCTION regardless of
/// whether the name is a known builtin (e.g. SUBSTRING), a user-defined
/// function, or an unknown identifier. For *highlighting* purposes the
/// distinction is irrelevant: call syntax is a function.
///
/// We deliberately do NOT consult the symbol table to gate this: even when the
/// name happens to match a known procedure (a `CREATE PROCEDURE` definition in
/// the same document), FUNCTION(2) is still the correct token for a
/// parenthesised call site. EXEC-style user-defined proc *calls* are out of
/// scope here (parser Exec handling produces `ExecStatement`, not a
/// `FunctionCall`).
///
/// Non-scope (future work): distinguishing UDFs from builtins for distinct
/// coloring would require a cross-check against `db_docs::lookup_function`.
/// That is intentionally deferred — see the feature's non-scope list.
fn walk_function_call(call: &FunctionCall, entries: &mut Vec<(Span, u32)>) {
    push_entry(entries, call.name.span, TYPE_FUNCTION);
    for arg in &call.args {
        walk_function_arg(arg, entries);
    }
}

fn walk_function_arg(arg: &FunctionArg, entries: &mut Vec<(Span, u32)>) {
    match arg {
        FunctionArg::Expression(expr) => walk_expression(expr, entries),
        FunctionArg::QualifiedWildcard(_) | FunctionArg::Wildcard => {}
    }
}

/// Accumulator for LSP semantic token delta encoding.
struct DeltaEncoder {
    prev_line: u32,
    prev_char: u32,
}

impl DeltaEncoder {
    const fn new() -> Self {
        Self {
            prev_line: 0,
            prev_char: 0,
        }
    }

    /// Push a delta-encoded semantic token.
    fn push(
        &mut self,
        tokens: &mut Vec<SemanticToken>,
        line: u32,
        character: u32,
        length: u32,
        token_type: u32,
    ) {
        let delta_line = line.saturating_sub(self.prev_line);
        let delta_start = if delta_line == 0 {
            character.saturating_sub(self.prev_char)
        } else {
            character
        };

        tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: 0,
        });

        self.prev_line = line;
        self.prev_char = character;
    }
}

/// ソースコードから Semantic Tokens を生成する（DocumentAnalysis利用）
#[must_use]
pub fn semantic_tokens_full_with_analysis(analysis: &DocumentAnalysis) -> SemanticTokensResult {
    let mut tokens = Vec::new();
    let mut encoder = DeltaEncoder::new();
    // Build the AST-derived span index ONCE before the token loop (not per token).
    let span_index = SemanticSpanIndex::build(analysis);

    for token in &analysis.tokens {
        if let Some(type_idx) =
            resolve_token_type(analysis, &span_index, token.kind, &token.text, token.span)
        {
            let (line, character) = analysis.line_index.offset_to_position(token.span.start);
            encoder.push(&mut tokens, line, character, token.span.len(), type_idx);
        }
    }

    SemanticTokens {
        result_id: None,
        data: tokens,
    }
    .into()
}

/// Generate Semantic Tokens for a specific range using DocumentAnalysis.
/// Only tokens whose start position falls within [range.start, range.end] are included.
#[must_use]
pub fn semantic_tokens_range_with_analysis(
    analysis: &DocumentAnalysis,
    range: Range,
) -> SemanticTokensRangeResult {
    let mut tokens = Vec::new();
    let mut encoder = DeltaEncoder::new();
    // Build the AST-derived span index ONCE before the token loop. Range
    // filtering is independent of classification and remains unchanged.
    let span_index = SemanticSpanIndex::build(analysis);

    for token in &analysis.tokens {
        let (line, character) = analysis.line_index.offset_to_position(token.span.start);

        // Skip tokens before range
        if line < range.start.line
            || (line == range.start.line && character < range.start.character)
        {
            continue;
        }
        // Stop past range
        if line > range.end.line || (line == range.end.line && character > range.end.character) {
            break;
        }

        if let Some(type_idx) =
            resolve_token_type(analysis, &span_index, token.kind, &token.text, token.span)
        {
            encoder.push(&mut tokens, line, character, token.span.len(), type_idx);
        }
    }

    SemanticTokensRangeResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // --- Semantic token enhancement tests (Phase #83) ---

    #[test]
    fn test_table_name_gets_class_token() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected tokens"),
        };
        // Find token at "users" on line 1 (the FROM clause)
        // CLASS = index 9
        let class_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 9).collect();
        assert!(
            !class_tokens.is_empty(),
            "Table name 'users' should be highlighted as CLASS (type 9), got tokens: {:?}",
            tokens
                .data
                .iter()
                .map(|t| (t.token_type, t.delta_line, t.delta_start, t.length))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_procedure_name_gets_function_token() {
        let source = "CREATE PROCEDURE my_proc AS BEGIN RETURN 1 END";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected tokens"),
        };
        // FUNCTION = index 2
        let func_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 2).collect();
        assert!(
            !func_tokens.is_empty(),
            "Procedure name 'my_proc' should be highlighted as FUNCTION (type 2)"
        );
    }

    #[test]
    fn test_plain_select_list_identifier_not_highlighted_as_class() {
        // A bare column identifier in the SELECT list (NOT a table reference,
        // NOT a function call) must remain unclassified — it is neither a table
        // nor a function, so it should not receive a CLASS or FUNCTION token.
        // (The AST walk intentionally classifies FROM/JOIN/DML *table* targets
        // as CLASS; this guard ensures it does not over-classify columns.)
        let source = "SELECT my_column";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            lsp_types::SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected tokens"),
        };
        let class_tokens: Vec<_> = tokens.data.iter().filter(|t| t.token_type == 9).collect();
        assert!(
            class_tokens.is_empty(),
            "A bare SELECT-list column should NOT get CLASS token; got tokens: {:?}",
            tokens
                .data
                .iter()
                .map(|t| (t.token_type, t.delta_line, t.delta_start, t.length))
                .collect::<Vec<_>>()
        );
    }

    // === Coverage gap tests ===

    #[test]
    fn test_range_tokens_basic() {
        use lsp_types::{Position, Range as LspRange};
        let source = "CREATE TABLE t (id INT)\nSELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let range = LspRange {
            start: Position {
                line: 1,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 20,
            },
        };
        let result = semantic_tokens_range_with_analysis(&analysis, range);
        let tokens = match result {
            lsp_types::SemanticTokensRangeResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // Should have tokens for SELECT, *, FROM at minimum
        assert!(
            !tokens.data.is_empty(),
            "Range tokens should not be empty for SELECT line"
        );
    }

    #[test]
    fn test_range_tokens_empty_range() {
        use lsp_types::{Position, Range as LspRange};
        let source = "SELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Range outside any tokens
        let range = LspRange {
            start: Position {
                line: 5,
                character: 0,
            },
            end: Position {
                line: 5,
                character: 10,
            },
        };
        let result = semantic_tokens_range_with_analysis(&analysis, range);
        let tokens = match result {
            lsp_types::SemanticTokensRangeResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        assert!(tokens.data.is_empty());
    }

    #[test]
    fn test_view_name_gets_class_token() {
        let source = "CREATE VIEW my_view AS SELECT 1";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // "my_view" should get CLASS token (index 9)
        assert!(
            tokens.data.iter().any(|t| t.token_type == 9),
            "View name should get CLASS semantic token"
        );
    }

    #[test]
    fn test_keyword_tokens_present() {
        let source = "SELECT * FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // SELECT and FROM should be keyword tokens (type 0)
        assert!(
            tokens.data.iter().any(|t| t.token_type == 0),
            "Keywords should get KEYWORD semantic token"
        );
    }

    #[test]
    fn test_empty_source_no_tokens() {
        let source = "";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        assert!(tokens.data.is_empty());
    }

    #[test]
    fn test_variable_gets_variable_token() {
        let source = "DECLARE @count INT\nSET @count = 1";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // VARIABLE = index 6 (see semantic_tokens_legend)
        assert!(
            tokens.data.iter().any(|t| t.token_type == 6),
            "Local variable @count should get VARIABLE semantic token (type 6)"
        );
    }

    #[test]
    fn test_datatype_gets_type_token() {
        let source = "DECLARE @x INT";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // TYPE = index 1 (see semantic_tokens_legend)
        assert!(
            tokens.data.iter().any(|t| t.token_type == 1),
            "INT data type should get TYPE semantic token (type 1)"
        );
    }

    #[test]
    fn test_range_tokens_intersecting_boundary() {
        use lsp_types::{Position, Range as LspRange};
        let source = "SELECT * FROM t WHERE id = 1";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Range covering FROM and t (char 9-15)
        let range = LspRange {
            start: Position {
                line: 0,
                character: 9,
            },
            end: Position {
                line: 0,
                character: 16,
            },
        };
        let result = semantic_tokens_range_with_analysis(&analysis, range);
        let tokens = match result {
            SemanticTokensRangeResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // Should include FROM keyword token and identifier t
        assert!(!tokens.data.is_empty(), "FROM and t should be in the range");
        // At least one keyword (FROM) and optionally one identifier
        assert!(
            tokens.data.iter().any(|t| t.token_type == 0),
            "FROM keyword should get KEYWORD token in range"
        );
    }

    // === Table-driven semantic token classification tests (Issue #133) ===
    //
    // These tests assert the *presence* of an expected SemanticTokenType index
    // without over-specifying token ordering or ordinal position (per
    // tdd-coupling.md: test behavior, not implementation details).

    /// UC-1: ローカル変数 @count は VARIABLE (type 6) に分類される。
    /// DECLARE @count / SET @count の両方の参照位置で VARIABLE が出現する。
    #[test]
    fn test_local_variable_classified() {
        let source = "DECLARE @count INT\nSET @count = 1";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // VARIABLE = index 6 (see semantic_tokens_legend)
        // LocalVar tokens always map to 6 via token_kind_to_type_index, so both
        // the DECLARE-site and SET-site @count occurrences are covered.
        assert!(
            tokens.data.iter().any(|t| t.token_type == 6),
            "Local variable @count should be classified as VARIABLE (type 6)"
        );
    }

    /// UC-2: 同一ドキュメントで定義されたテーブル users は、FROM 句の参照位置でも
    /// CLASS (type 9) に分類される（シンボルテーブル経由のクロスリファレンス）。
    #[test]
    fn test_table_reference_in_from_clause_classified() {
        let source = "CREATE TABLE users (id INT)\nSELECT * FROM users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // CLASS = index 9 (see semantic_tokens_legend)
        // The CREATE TABLE definition registers 'users' in the symbol table,
        // and the FROM-clause reference resolves to the same CLASS type.
        assert!(
            tokens.data.iter().any(|t| t.token_type == 9),
            "Table reference 'users' in FROM clause should be classified as CLASS (type 9)"
        );
    }

    /// UC-2: 関数呼び出し SUBSTRING(name, 1, 10) は FUNCTION (type 2) に分類される。
    /// これは AST の FunctionCall ノードから導出される（T3 の実装対象）。
    #[test]
    fn test_function_call_classified() {
        let source = "SELECT SUBSTRING(name, 1, 10) FROM users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // FUNCTION = index 2 (see semantic_tokens_legend)
        // SUBSTRING is a function-call identifier; it should be classified via
        // the AST FunctionCall node rather than appearing as a plain Ident.
        assert!(
            tokens.data.iter().any(|t| t.token_type == 2),
            "Function call 'SUBSTRING' should be classified as FUNCTION (type 2)"
        );
    }

    /// T3: 関数呼び出し名が組み込みでもユーザー定義でも未知でも FUNCTION (type 2) になる。
    /// すべての Ident(args) 呼び出し構文はハイライト用途では関数（T3 決定）。
    #[test]
    fn test_unknown_function_call_still_function() {
        let source = "SELECT my_weird_func(col) FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        assert!(
            tokens.data.iter().any(|t| t.token_type == 2),
            "Unknown function-call name should still be FUNCTION (type 2)"
        );
    }

    /// T3: 括弧のない単なるカラム参照は FUNCTION に誤分類されない。
    /// AST の Identifier / ColumnReference ノードはインデックスに載らない。
    #[test]
    fn test_plain_column_not_misclassified_as_function() {
        let source = "SELECT name FROM users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        assert!(
            !tokens.data.iter().any(|t| t.token_type == 2),
            "Plain column 'name' must not be highlighted as FUNCTION"
        );
    }

    /// JOIN 内のテーブル参照 (t1, t2 with aliases a, b) の分類。
    /// 少なくとも FROM 側のテーブル参照が CLASS に分類されることを検証する。
    #[test]
    fn test_join_table_reference_classified() {
        let source = "SELECT a.x FROM t1 a JOIN t2 b ON a.x = b.x";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // t1/t2 are NOT defined via CREATE TABLE in this source, so they will
        // only be classified as CLASS if the AST-based reference classifier
        // (T2) maps FROM/JOIN table references. Assert presence of CLASS (9).
        assert!(
            tokens.data.iter().any(|t| t.token_type == 9),
            "JOIN table references t1/t2 should be classified as CLASS (type 9)"
        );
    }

    /// INSERT の対象テーブル (INSERT INTO users) は、同一ドキュメントで
    /// CREATE TABLE されていれば CLASS (type 9) に分類される。
    #[test]
    fn test_insert_target_table_classified() {
        let source = "CREATE TABLE users (id INT)\nINSERT INTO users VALUES (1)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // CLASS = index 9. 'users' is registered by CREATE TABLE and the
        // INSERT target reference resolves to CLASS via the symbol table.
        assert!(
            tokens.data.iter().any(|t| t.token_type == 9),
            "INSERT target table 'users' should be classified as CLASS (type 9)"
        );
    }

    /// UPDATE / DELETE の対象テーブル参照の分類。
    /// CREATE TABLE 済みテーブルに対する UPDATE/DELETE で CLASS (type 9) が出現する。
    #[test]
    fn test_update_delete_table_classified() {
        let source = "CREATE TABLE users (id INT)\nUPDATE users SET id = 1\nDELETE FROM users";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // CLASS = index 9. Both UPDATE and DELETE target the defined 'users'
        // table; at least one CLASS occurrence is expected.
        assert!(
            tokens.data.iter().any(|t| t.token_type == 9),
            "UPDATE/DELETE target table 'users' should be classified as CLASS (type 9)"
        );
    }

    /// UC-3: パースエラーを含む SQL でもトークンレベル分類にフォールバックし、
    /// キーワード / LocalVar 等の既存の強調表示を維持する（graceful degradation）。
    /// パニックせずにトークンを生成することが前提。
    #[test]
    fn test_parse_error_falls_back_to_token_level() {
        // 'CREATE UNIQUE INDEX' produces a parse error (parser-level recovery),
        // and the trailing '@count' is a LocalVar. The handler must not panic
        // and must still emit token-level classifications (keyword + variable).
        let source = "CREATE UNIQUE INDEX idx ON t (c)\nDECLARE @count INT";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = semantic_tokens_full_with_analysis(&analysis);
        let tokens = match result {
            SemanticTokensResult::Tokens(t) => t,
            _ => panic!("Expected Tokens"),
        };
        // Must not panic and must produce some tokens.
        assert!(
            !tokens.data.is_empty(),
            "Parse-error source should still produce token-level semantic tokens"
        );
        // Keyword classification (CREATE/DECLARE/ON etc.) survives the fallback.
        assert!(
            tokens.data.iter().any(|t| t.token_type == 0),
            "Keyword tokens should still be classified under parse-error fallback"
        );
        // LocalVar (@count) classification survives the fallback (VARIABLE = 6).
        assert!(
            tokens.data.iter().any(|t| t.token_type == 6),
            "LocalVar @count should still be classified as VARIABLE under parse-error fallback"
        );
    }
}
