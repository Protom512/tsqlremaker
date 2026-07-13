//! Inlay Hints (#118).
//!
//! Renders inline annotations — variable type annotations and procedure
//! parameter names — directly inside the editor text, so a developer
//! scanning a long stored procedure can see the type of each `DECLARE`d
//! variable and the parameter name each positional `EXEC` argument binds to
//! without jumping to the declaration.
//!
//! ## Design
//!
//! [`inlay_hints`] is a **pure function** over [`DocumentAnalysis`] (mirroring
//! [`code_lenses`][crate::code_lens::code_lenses]): it walks the parsed AST
//! statements, emits one [`InlayHint`] per `DECLARE` variable / positional
//! `EXEC` argument, and applies two filters:
//!
//! 1. **Config gate** — [`InlayConfig`] (`enable_variable_types` /
//!    `enable_parameter_names`) toggles each hint family independently. Both
//!    default to `true`, preserving pre-#118 behaviour (the server simply
//!    advertised no hints because the capability was absent).
//! 2. **Range filter** — when the client supplies a viewport [`Range`], only
//!    hints whose `position` falls inside it are returned.
//!
//! The MVP returns hints **eagerly resolved** (label/kind/padding all
//! populated). [`InlayHintData`] is stashed in each hint's `data` field now so
//! a future `inlayHint/resolve` provider can re-identify the hint category
//! without a breaking wire-format change.
//!
//! ## Scope / limitations
//!
//! Parameter-name resolution for positional `EXEC` arguments is
//! **document-local**: the procedure signature is looked up in the same
//! document's [`SymbolTable`][crate::symbol_table::SymbolTable]. Cross-file
//! procedure resolution through the
//! [`SymbolStore`][crate::symbol_store::SymbolStore] is a future L–XL
//! enhancement; it would require the handler to take a workspace snapshot,
//! breaking the pure-function contract. Named arguments (`@p = value`) are
//! skipped because the parameter name is already visible in source.
//!
//! Result-set column type hints (originally UC-3 of issue #118) are
//! out of scope: they need a separate column-type inference foundation.

use crate::analysis::DocumentAnalysis;
use crate::config::InlayConfig;
use crate::symbol_table::CaseInsensitiveKey;
use lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, Position, Range};
use serde::{Deserialize, Serialize};
use tsql_parser::ast::{AstNode, ExecArgument, Statement};

/// Internal payload stashed in [`InlayHint::data`] for future
/// `inlayHint/resolve` expansion (#118).
///
/// The MVP returns hints eagerly-resolved (label / kind / padding all
/// populated), so this payload is not consumed by the server today. It is
/// serialised into the `data` field now so a later resolve-provider can
/// re-identify the hint category without a breaking wire-format change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InlayHintData {
    /// A variable-type hint (`: INT`). Carries the rendered type label.
    VariableType {
        /// The `Display`-rendered data type, e.g. `"VARCHAR(100)"`.
        type_label: String,
    },
    /// A parameter-name hint (`@p:`). Carries the resolved parameter name.
    ParameterName {
        /// The procedure parameter name (with `@` prefix), e.g. `"@id"`.
        name: String,
    },
}

/// Build inlay hints for a document region (#118).
///
/// Walks `analysis.statements`, emitting:
/// - **TYPE** hints (`: INT`) at the end of each `DECLARE` variable name, with
///   `padding_left = true`, and
/// - **PARAMETER** hints (`param_name:`) before each positional `EXEC`
///   argument whose procedure signature is defined in the same document, with
///   `padding_right = true`.
///
/// Both families are independently gated by [`InlayConfig`]. When `range` is
/// `Some`, only hints whose position lies inside the viewport are kept. Every
/// emitted hint is guarded by a `span.start < span.end` check so the parser's
/// multi-line broken-span issue (`span.end = 0`) degrades gracefully instead
/// of placing hints at bogus offsets.
///
/// # Panics
///
/// Never. Invalid/broken spans and missing procedure signatures are silently
/// skipped.
#[must_use]
pub fn inlay_hints(
    analysis: &DocumentAnalysis,
    range: Option<Range>,
    config: &InlayConfig,
) -> Vec<InlayHint> {
    let mut hints = Vec::new();
    for stmt in &analysis.statements {
        match stmt {
            Statement::Declare(declare) => {
                if !config.enable_variable_types {
                    continue;
                }
                for var in &declare.variables {
                    let span = var.name.span;
                    // Broken-span guard: multi-line DECLARE may yield span.end = 0.
                    if span.start >= span.end {
                        continue;
                    }
                    let position = offset_to_position(analysis, span.end);
                    if let Some(hint) = make_type_hint(position, &var.data_type) {
                        if in_range(position, range) {
                            hints.push(hint);
                        }
                    }
                }
            }
            Statement::Exec(exec) => {
                if !config.enable_parameter_names {
                    continue;
                }
                let proc_key = CaseInsensitiveKey::new(&exec.procedure.name);
                let Some(proc_sym) = analysis.symbol_table.procedures.get(&proc_key) else {
                    // No in-document CREATE PROC signature → cannot resolve
                    // positional arg names. Skip rather than emit a wrong name.
                    continue;
                };
                let params = &proc_sym.parameters;
                for (idx, arg) in exec.arguments.iter().enumerate() {
                    let ExecArgument::Positional(expr) = arg else {
                        // Named arg (@p = value): the name is already in source,
                        // so emitting it again would duplicate. Skip.
                        continue;
                    };
                    let Some(param) = params.get(idx) else {
                        // More positional args than declared parameters.
                        continue;
                    };
                    let span = expr.span();
                    // Broken-span guard (positional EXEC args are also subject
                    // to the multi-line broken-span issue).
                    if span.start >= span.end {
                        continue;
                    }
                    let position = offset_to_position(analysis, span.start);
                    let hint = make_param_hint(position, &param.name);
                    if in_range(position, range) {
                        hints.push(hint);
                    }
                }
            }
            _ => {}
        }
    }
    hints
}

/// Convert a byte offset to an LSP [`Position`] via the document's
/// [`LineIndex`][crate::line_index::LineIndex].
fn offset_to_position(analysis: &DocumentAnalysis, offset: u32) -> Position {
    let (line, character) = analysis.line_index.offset_to_position(offset);
    Position { line, character }
}

/// Construct a TYPE inlay hint with a `: <type>` label.
///
/// `padding_left = true` inserts a space between the variable name and the
/// type label (per #118 spec). [`InlayHintData::VariableType`] is stored in
/// `data` for future resolve-provider use. Returns `None` only if the data
/// type renders to an empty string (defensive — `DataType`'s `Display` impl
/// never does, but the guard keeps the contract honest for future variants).
fn make_type_hint(position: Position, data_type: &tsql_parser::ast::DataType) -> Option<InlayHint> {
    let type_label = format!("{}", data_type);
    if type_label.is_empty() {
        return None;
    }
    let data = InlayHintData::VariableType {
        type_label: type_label.clone(),
    };
    Some(InlayHint {
        position,
        label: InlayHintLabel::String(format!(": {type_label}")),
        kind: Some(InlayHintKind::TYPE),
        text_edits: None,
        tooltip: None,
        padding_left: Some(true),
        padding_right: None,
        data: serde_json::to_value(data).ok(),
    })
}

/// Construct a PARAMETER inlay hint with a `<name>:` label.
///
/// `padding_right = true` inserts a space between the parameter name and the
/// argument expression (per #118 spec). [`InlayHintData::ParameterName`] is
/// stored in `data` for future resolve-provider use.
fn make_param_hint(position: Position, param_name: &str) -> InlayHint {
    let data = InlayHintData::ParameterName {
        name: param_name.to_string(),
    };
    InlayHint {
        position,
        label: InlayHintLabel::String(format!("{param_name}:")),
        kind: Some(InlayHintKind::PARAMETER),
        text_edits: None,
        tooltip: None,
        padding_left: None,
        padding_right: Some(true),
        data: serde_json::to_value(data).ok(),
    }
}

/// Return `true` when `position` lies inside `range` (or `range` is `None`).
///
/// Inclusive of `range.start`, exclusive of `range.end` — matching how editors
/// define a selection viewport.
fn in_range(position: Position, range: Option<Range>) -> bool {
    match range {
        None => true,
        Some(r) => position >= r.start && position < r.end,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::analysis::DocumentAnalysis;
    use crate::config::InlayConfig;
    use lsp_types::{InlayHintKind, InlayHintLabel, Position, Range};

    /// Extract the hint label as a plain `&str` (panics on label-parts form,
    /// which this module never emits).
    fn label_str(hint: &InlayHint) -> &str {
        match &hint.label {
            InlayHintLabel::String(s) => s,
            InlayHintLabel::LabelParts(_) => panic!("unexpected label-parts form"),
        }
    }

    fn default_config() -> InlayConfig {
        InlayConfig::default()
    }

    // --- Variable-type hints (DECLARE) ---

    // (1) DECLARE @count INT → one TYPE hint labeled ": INT" at variable end.
    #[test]
    fn declare_single_variable_emits_one_type_hint_at_name_end() {
        let src = "DECLARE @count INT";
        let analysis = DocumentAnalysis::new(src);
        let hints = inlay_hints(&analysis, None, &default_config());
        assert_eq!(hints.len(), 1, "exactly one hint: {hints:?}");
        let hint = &hints[0];
        assert_eq!(label_str(hint), ": INT");
        assert_eq!(hint.kind, Some(InlayHintKind::TYPE));
        // padding_left=true separates the name from the type label.
        assert_eq!(hint.padding_left, Some(true));
        // TYPE hints do not set padding_right.
        assert_eq!(hint.padding_right, None);
        // @count ends at byte 14 ("DECLARE @count" = 14 chars); line 0.
        assert_eq!(
            hint.position,
            Position {
                line: 0,
                character: 14,
            }
        );
    }

    // (2) DECLARE @x VARCHAR(50), @d DECIMAL(10,2) → 2 hints with correct
    //     DataType Display formatting (VARCHAR(50) / DECIMAL(10,2)).
    #[test]
    fn declare_multiple_variables_emits_display_formatted_type_labels() {
        let src = "DECLARE @x VARCHAR(50), @d DECIMAL(10,2)";
        let analysis = DocumentAnalysis::new(src);
        let hints = inlay_hints(&analysis, None, &default_config());
        assert_eq!(hints.len(), 2, "one hint per variable: {hints:?}");

        let labels: Vec<&str> = hints.iter().map(label_str).collect();
        assert!(labels.contains(&": VARCHAR(50)"), "labels: {labels:?}");
        // DECIMAL(10,2) — no space after comma (matches DataType Display).
        assert!(labels.contains(&": DECIMAL(10,2)"), "labels: {labels:?}");
        assert!(hints.iter().all(|h| h.kind == Some(InlayHintKind::TYPE)));
    }

    // (3) EXEC with Positional args + matching in-document CREATE PROC
    //     signature → PARAMETER hints.
    #[test]
    fn exec_positional_args_with_in_document_proc_signature_emits_param_hints() {
        let src = "CREATE PROC myproc @a INT, @b INT AS\nSELECT 1\nEXEC myproc 10, 20";
        let analysis = DocumentAnalysis::new(src);
        let hints = inlay_hints(&analysis, None, &default_config());
        // Two positional args → two PARAMETER hints (DECLARE family absent).
        let param_hints: Vec<&InlayHint> = hints
            .iter()
            .filter(|h| h.kind == Some(InlayHintKind::PARAMETER))
            .collect();
        assert_eq!(param_hints.len(), 2, "param hints: {param_hints:?}");
        assert_eq!(label_str(param_hints[0]), "@a:");
        assert_eq!(label_str(param_hints[1]), "@b:");
        // padding_right=true separates the param name from the argument.
        assert_eq!(param_hints[0].padding_right, Some(true));
        // PARAMETER hints do not set padding_left.
        assert_eq!(param_hints[0].padding_left, None);
    }

    // (4) EXEC Named args → no duplicate hints (name already in source).
    #[test]
    fn exec_named_args_emit_no_param_hints() {
        let src = "CREATE PROC myproc @a INT, @b INT AS\nSELECT 1\nEXEC myproc @a = 10, @b = 20";
        let analysis = DocumentAnalysis::new(src);
        let hints = inlay_hints(&analysis, None, &default_config());
        let param_hints: Vec<&InlayHint> = hints
            .iter()
            .filter(|h| h.kind == Some(InlayHintKind::PARAMETER))
            .collect();
        assert!(
            param_hints.is_empty(),
            "named args must not duplicate the in-source name: {param_hints:?}"
        );
    }

    // (5) InlayConfig{enable_variable_types:false} → no type hints.
    #[test]
    fn config_disable_variable_types_suppresses_type_hints() {
        let src = "DECLARE @count INT";
        let analysis = DocumentAnalysis::new(src);
        let cfg = InlayConfig {
            enable_variable_types: false,
            enable_parameter_names: true,
        };
        let hints = inlay_hints(&analysis, None, &cfg);
        assert!(
            hints.iter().all(|h| h.kind != Some(InlayHintKind::TYPE)),
            "no TYPE hints when disabled: {hints:?}"
        );
        assert!(hints.is_empty(), "no hints at all: {hints:?}");
    }

    // (6) InlayConfig{enable_parameter_names:false} → no param hints.
    #[test]
    fn config_disable_parameter_names_suppresses_param_hints() {
        let src = "CREATE PROC myproc @a INT AS\nSELECT 1\nEXEC myproc 10";
        let analysis = DocumentAnalysis::new(src);
        let cfg = InlayConfig {
            enable_variable_types: true,
            enable_parameter_names: false,
        };
        let hints = inlay_hints(&analysis, None, &cfg);
        assert!(
            hints
                .iter()
                .all(|h| h.kind != Some(InlayHintKind::PARAMETER)),
            "no PARAMETER hints when disabled: {hints:?}"
        );
    }

    // (7) empty / no-DECLARE document → empty vec.
    #[test]
    fn empty_or_no_declare_document_emits_nothing() {
        let empty = DocumentAnalysis::new("");
        assert!(inlay_hints(&empty, None, &default_config()).is_empty());

        let no_declare = DocumentAnalysis::new("SELECT 1\nFROM t");
        assert!(inlay_hints(&no_declare, None, &default_config()).is_empty());

        // EXEC without an in-document CREATE PROC → no resolution, no hints.
        let exec_only = DocumentAnalysis::new("EXEC unknown_proc 1, 2");
        assert!(inlay_hints(&exec_only, None, &default_config()).is_empty());
    }

    // (8) range filtering — hints outside the requested Range are excluded.
    #[test]
    fn range_filter_excludes_hints_outside_viewport() {
        // Two DECLAREs on separate lines.
        let src = "DECLARE @a INT\ndeclare @b INT";
        let analysis = DocumentAnalysis::new(src);
        // Full range → both hints.
        let full = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 10,
                character: 0,
            },
        };
        let full_hints = inlay_hints(&analysis, Some(full), &default_config());
        assert_eq!(
            full_hints.len(),
            2,
            "both within full range: {full_hints:?}"
        );

        // Range covering only line 0 → exactly one hint.
        let line0_only = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 1,
                character: 0,
            },
        };
        let filtered = inlay_hints(&analysis, Some(line0_only), &default_config());
        assert_eq!(filtered.len(), 1, "only line-0 hint survives: {filtered:?}");
        assert_eq!(filtered[0].position.line, 0);
    }

    // (9) broken-span guard (span.end=0) skips emission gracefully.
    #[test]
    fn broken_span_guard_skips_emission() {
        // Direct unit test of the guard: synthesize an analysis whose only
        // statement carries a zero-length / inverted span, mirroring the
        // parser's multi-line broken-span issue (span.end = 0). We do this
        // by constructing a DeclareStatement by hand so we don't depend on
        // the parser happening to produce a broken span.
        use tsql_parser::ast::{DataType, DeclareStatement, Identifier, VariableDeclaration};
        use tsql_token::Span;

        let broken_name = Identifier {
            name: "@broken".to_string(),
            span: Span { start: 8, end: 0 }, // end < start → broken
        };
        let var = VariableDeclaration {
            name: broken_name,
            data_type: DataType::Int,
            default_value: None,
        };
        let stmt = Statement::Declare(Box::new(DeclareStatement {
            span: Span { start: 0, end: 0 },
            variables: vec![var],
        }));

        // Build a minimal analysis around the synthetic statement.
        let src = "DECLARE @broken INT";
        let analysis = DocumentAnalysis::new(src);
        let patched = DocumentAnalysis {
            source: analysis.source.clone(),
            line_index: analysis.line_index.clone(),
            tokens: analysis.tokens.clone(),
            statements: vec![stmt],
            parse_errors: analysis.parse_errors.clone(),
            symbol_table: analysis.symbol_table.clone(),
        };

        let hints = inlay_hints(&patched, None, &default_config());
        assert!(
            hints.is_empty(),
            "broken span (end < start) must not emit a hint: {hints:?}"
        );
    }

    // (10) more positional args than declared params → excess args skipped.
    #[test]
    fn exec_more_positional_args_than_params_skips_excess() {
        let src = "CREATE PROC myproc @a INT AS\nSELECT 1\nEXEC myproc 10, 20";
        let analysis = DocumentAnalysis::new(src);
        let hints = inlay_hints(&analysis, None, &default_config());
        let param_hints: Vec<&InlayHint> = hints
            .iter()
            .filter(|h| h.kind == Some(InlayHintKind::PARAMETER))
            .collect();
        assert_eq!(param_hints.len(), 1, "excess arg skipped: {param_hints:?}");
        assert_eq!(label_str(param_hints[0]), "@a:");
    }

    // (11) case-insensitive procedure lookup.
    #[test]
    fn exec_case_insensitive_procedure_lookup() {
        // CREATE PROC lowercase, EXEC uppercase — must still resolve.
        let src = "CREATE PROC myproc @x INT AS\nSELECT 1\nEXEC MYPROC 5";
        let analysis = DocumentAnalysis::new(src);
        let hints = inlay_hints(&analysis, None, &default_config());
        let param_hints: Vec<&InlayHint> = hints
            .iter()
            .filter(|h| h.kind == Some(InlayHintKind::PARAMETER))
            .collect();
        assert_eq!(param_hints.len(), 1, "case-insensitive lookup: {hints:?}");
    }

    // --- InlayHintData payload round-trips through serde ---

    #[test]
    fn type_hint_carries_variable_type_data() {
        let src = "DECLARE @count INT";
        let analysis = DocumentAnalysis::new(src);
        let hints = inlay_hints(&analysis, None, &default_config());
        let data_value = hints[0].data.as_ref().expect("data present");
        let payload: InlayHintData = serde_json::from_value(data_value.clone()).unwrap();
        match payload {
            InlayHintData::VariableType { type_label } => assert_eq!(type_label, "INT"),
            InlayHintData::ParameterName { .. } => panic!("expected VariableType"),
        }
    }

    #[test]
    fn parameter_hint_carries_parameter_name_data() {
        let src = "CREATE PROC p @p1 INT AS\nSELECT 1\nEXEC p 1";
        let analysis = DocumentAnalysis::new(src);
        let hints = inlay_hints(&analysis, None, &default_config());
        let param_hint = hints
            .iter()
            .find(|h| h.kind == Some(InlayHintKind::PARAMETER))
            .expect("parameter hint present");
        let data_value = param_hint.data.as_ref().expect("data present");
        let payload: InlayHintData = serde_json::from_value(data_value.clone()).unwrap();
        match payload {
            InlayHintData::ParameterName { name } => assert_eq!(name, "@p1"),
            InlayHintData::VariableType { .. } => panic!("expected ParameterName"),
        }
    }

    /// Defaults reproduce pre-#118 expectation: both hint families enabled.
    #[test]
    fn inlay_config_defaults_enable_both_families() {
        let cfg = InlayConfig::default();
        assert!(cfg.enable_variable_types);
        assert!(cfg.enable_parameter_names);
    }
}
