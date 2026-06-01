//! Signature Help
//!
//! 組み込み関数のパラメータシグネチャを提供する。
//!
//! ネストした関数呼び出し（例: `SUBSTRING(CONVERT(...))`）では、
//! カーソル位置の最も内側の関数シグネチャを返す。

use crate::analysis::DocumentAnalysis;
use crate::line_index::LineIndex;
use lsp_types::{
    ParameterInformation, ParameterLabel, Position, SignatureHelp, SignatureInformation,
};
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// スタックベースの関数呼び出しフレーム。
///
/// 各 `( ` でプッシュされ、対応する `)` でポップされる。
#[derive(Debug)]
struct CallFrame {
    /// 関数名（大文字）
    func_name: String,
    /// 現在のパラメータインデックス（0-based）
    active_param: u32,
    /// このフレームの `(` が開いた時の paren_depth
    open_depth: i32,
}

/// トークンストリームをスキャンしてカーソル位置の CallFrame を特定する。
///
/// ネストした呼び出し（例: `ISNULL(SUBSTRING(col, 1, 3), 'def')`）では、
/// カーソル位置の最も内側の関数フレームを返す。
fn scan_for_call_frame<'a>(
    tokens: impl Iterator<Item = (TokenKind, &'a str, usize)>,
    offset: usize,
) -> Option<CallFrame> {
    let mut stack: Vec<CallFrame> = Vec::new();
    let mut paren_depth = 0i32;
    // LParen の前に Ident/Keyword があった場合、その名前を保留する
    let mut pending_name: Option<String> = None;

    for (kind, text, span_start) in tokens {
        if span_start > offset {
            break;
        }

        match kind {
            TokenKind::Ident => {
                pending_name = Some(text.to_uppercase());
            }
            TokenKind::LParen => {
                paren_depth += 1;
                if let Some(name) = pending_name.take() {
                    stack.push(CallFrame {
                        func_name: name,
                        active_param: 0,
                        open_depth: paren_depth,
                    });
                } else {
                    // 関数名なしの '(' — 純粋なグループ化
                    pending_name = None;
                }
            }
            TokenKind::RParen => {
                paren_depth -= 1;
                // 閉じ括弧に対応するフレームをポップ
                while stack.last().is_some_and(|f| f.open_depth > paren_depth) {
                    stack.pop();
                }
                // ちょうどこの深さのフレームもポップ（呼び出し完了）
                if stack
                    .last()
                    .is_some_and(|f| f.open_depth == paren_depth + 1)
                {
                    stack.pop();
                }
                pending_name = None;
            }
            TokenKind::Comma => {
                // スタックのトップフレームの直下にあるカンマのみカウント
                // （トップフレームの open_depth + 1 の深さ = その関数の引数レベル）
                if let Some(top) = stack.last_mut() {
                    if paren_depth == top.open_depth {
                        top.active_param += 1;
                    }
                }
            }
            _ => {
                // キーワードトークンも関数名候補として扱う
                if kind.is_keyword() {
                    pending_name = Some(text.to_uppercase());
                }
            }
        }
    }

    // カーソル位置が関数呼び出しの中にある場合、スタックのトップを返す
    if paren_depth > 0 && !stack.is_empty() {
        stack.pop()
    } else {
        None
    }
}

/// SignatureHelp情報を生成する（DocumentAnalysis利用）
pub fn signature_help_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
) -> Option<SignatureHelp> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    let frame = scan_for_call_frame(
        analysis
            .tokens
            .iter()
            .map(|t| (t.kind, t.text.as_str(), t.span.start as usize)),
        offset,
    )?;

    build_signature_help(&frame.func_name, frame.active_param)
}

/// SignatureHelp情報を生成する
///
/// カーソル位置が関数呼び出しの引数内にある場合、シグネチャ情報を返す。
pub fn signature_help(source: &str, position: Position) -> Option<SignatureHelp> {
    let line_index = LineIndex::new(source);
    let offset = line_index.position_to_offset(source, position);

    let tokens: Vec<_> = Lexer::new(source).filter_map(Result::ok).collect();

    let frame = scan_for_call_frame(
        tokens
            .iter()
            .map(|t| (t.kind, t.text, t.span.start as usize)),
        offset,
    )?;

    build_signature_help(&frame.func_name, frame.active_param)
}

/// 関数名とパラメータインデックスから SignatureHelp を構築する。
fn build_signature_help(func_name: &str, active_param: u32) -> Option<SignatureHelp> {
    let entry = crate::db_docs::lookup_function(func_name)?;

    if entry.category != crate::db_docs::DocCategory::Function {
        tracing::debug!(
            "signature_help: '{func_name}' is not a function ({:?})",
            entry.category
        );
        return None;
    }

    let parameters: Vec<ParameterInformation> = entry
        .params
        .iter()
        .map(|p| ParameterInformation {
            label: ParameterLabel::Simple(p.to_string()),
            documentation: None,
        })
        .collect();

    let active_parameter = if active_param < parameters.len() as u32 {
        Some(active_param)
    } else {
        Some((parameters.len() as u32).saturating_sub(1))
    };

    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label: entry.syntax.to_string(),
            documentation: Some(lsp_types::Documentation::String(
                entry.description.to_string(),
            )),
            parameters: Some(parameters),
            active_parameter,
        }],
        active_signature: Some(0),
        active_parameter,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_help_substring() {
        let result = signature_help(
            "SELECT SUBSTRING(",
            Position {
                line: 0,
                character: 18,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert_eq!(help.signatures.len(), 1);
        assert!(help.signatures[0].label.contains("SUBSTRING"));
        assert_eq!(help.active_parameter, Some(0));
    }

    #[test]
    fn test_signature_help_second_param() {
        let result = signature_help(
            "SELECT SUBSTRING(col, ",
            Position {
                line: 0,
                character: 23,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert_eq!(help.active_parameter, Some(1));
    }

    #[test]
    fn test_signature_help_convert() {
        let result = signature_help(
            "SELECT CONVERT(",
            Position {
                line: 0,
                character: 15,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(help.signatures[0].label.contains("CONVERT"));
    }

    #[test]
    fn test_signature_help_unknown_function() {
        let result = signature_help(
            "SELECT MYFUNC(",
            Position {
                line: 0,
                character: 14,
            },
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_signature_help_no_paren() {
        let result = signature_help(
            "SELECT SUBSTRING",
            Position {
                line: 0,
                character: 10,
            },
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_signature_help_count() {
        let result = signature_help(
            "SELECT COUNT(",
            Position {
                line: 0,
                character: 13,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(help.signatures[0].label.contains("COUNT"));
    }

    #[test]
    fn test_signature_help_dateadd() {
        let result = signature_help(
            "SELECT DATEADD(day, 1, ",
            Position {
                line: 0,
                character: 23,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert_eq!(help.active_parameter, Some(2));
    }

    #[test]
    fn test_signature_help_isnull() {
        let result = signature_help(
            "SELECT ISNULL(col, ",
            Position {
                line: 0,
                character: 20,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(help.signatures[0].label.contains("ISNULL"));
        assert_eq!(help.active_parameter, Some(1));
    }

    #[test]
    fn test_signature_help_sibling_calls_resets_param() {
        // After ISNULL(a, b), the next call SUBSTRING(x should be at param 0
        let source = "SELECT ISNULL(a, b), SUBSTRING(x";
        let result = signature_help(
            source,
            Position {
                line: 0,
                character: source.len() as u32,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(help.signatures[0].label.contains("SUBSTRING"));
        assert_eq!(help.active_parameter, Some(0));
    }

    #[test]
    fn test_signature_help_third_param() {
        let result = signature_help(
            "SELECT SUBSTRING(col, 1, ",
            Position {
                line: 0,
                character: 26,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert_eq!(help.active_parameter, Some(2));
    }

    #[test]
    fn test_signature_help_nested_parens() {
        // SUBSTRING(CONVERT( — cursor inside CONVERT, should show CONVERT
        let result = signature_help(
            "SELECT SUBSTRING(CONVERT(",
            Position {
                line: 0,
                character: 25,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(
            help.signatures[0].label.contains("CONVERT"),
            "Should show innermost function CONVERT, got: {}",
            help.signatures[0].label
        );
        assert_eq!(help.active_parameter, Some(0));
    }

    #[test]
    fn test_signature_help_nested_with_comma_in_outer() {
        // SUBSTRING(x, CONVERT( — cursor inside CONVERT at param 0
        // SUBSTRING is at param 1 (after comma)
        let result = signature_help(
            "SELECT SUBSTRING(x, CONVERT(",
            Position {
                line: 0,
                character: 28,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(
            help.signatures[0].label.contains("CONVERT"),
            "Should show innermost function CONVERT, got: {}",
            help.signatures[0].label
        );
        assert_eq!(
            help.active_parameter,
            Some(0),
            "CONVERT should be at param 0"
        );
    }

    #[test]
    fn test_signature_help_after_closing_nested() {
        // SUBSTRING(CONVERT(a, b), ← cursor here, back to SUBSTRING at param 1
        let result = signature_help(
            "SELECT SUBSTRING(CONVERT(a, b), ",
            Position {
                line: 0,
                character: 31,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(
            help.signatures[0].label.contains("SUBSTRING"),
            "Should show outer function SUBSTRING, got: {}",
            help.signatures[0].label
        );
        assert_eq!(
            help.active_parameter,
            Some(1),
            "SUBSTRING should be at param 1 (after comma)"
        );
    }

    #[test]
    fn test_signature_help_parameter_count_matches() {
        let result = signature_help(
            "SELECT DATEADD(",
            Position {
                line: 0,
                character: 15,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        let params = help.signatures[0].parameters.as_ref().unwrap();
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_signature_help_deeply_nested() {
        // ISNULL(SUBSTRING(CONVERT(a, — cursor inside CONVERT
        let result = signature_help(
            "SELECT ISNULL(SUBSTRING(CONVERT(a, ",
            Position {
                line: 0,
                character: 33,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(
            help.signatures[0].label.contains("CONVERT"),
            "Should show innermost function CONVERT, got: {}",
            help.signatures[0].label
        );
        assert_eq!(help.active_parameter, Some(1), "CONVERT at param 1");
    }

    // === signature_help_with_analysis tests ===

    #[test]
    fn test_analysis_signature_help_substring() {
        let source = "SELECT SUBSTRING(col, 1, 3) FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = signature_help_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 18,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(help.signatures[0].label.contains("SUBSTRING"));
    }

    #[test]
    fn test_analysis_signature_help_nested() {
        let source = "SELECT SUBSTRING(CONVERT(a, b), 1, 3) FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = signature_help_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 25,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        assert!(
            help.signatures[0].label.contains("CONVERT"),
            "Should show innermost CONVERT"
        );
    }

    #[test]
    fn test_analysis_signature_help_no_function() {
        let source = "SELECT col FROM t";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = signature_help_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 10,
            },
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_analysis_signature_help_unknown_function() {
        let source = "SELECT MYFUNC(";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = signature_help_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            },
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_signature_help_active_param_clamped() {
        // SUBSTRING with too many params — active_param should be clamped to last
        let result = signature_help(
            "SELECT SUBSTRING(a, b, c, d, e, ",
            Position {
                line: 0,
                character: 31,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        // SUBSTRING has 3 params, active_param clamped to 2
        assert_eq!(
            help.active_parameter,
            Some(2),
            "active_parameter should be clamped to last param index"
        );
    }

    #[test]
    fn test_signature_help_grouping_parens() {
        // SELECT (1 + 2) — no function, just grouping
        let result = signature_help(
            "SELECT (1 + 2)",
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(
            result.is_none(),
            "Grouping parens should not trigger signature help"
        );
    }
}
