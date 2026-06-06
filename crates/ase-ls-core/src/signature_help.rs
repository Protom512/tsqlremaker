//! Signature Help
//!
//! 組み込み関数のパラメータシグネチャを提供する。
//!
//! ネストした関数呼び出し（例: `SUBSTRING(CONVERT(...))`）では、
//! カーソル位置の最も内側の関数シグネチャを返す。

use crate::analysis::DocumentAnalysis;
use lsp_types::{
    ParameterInformation, ParameterLabel, Position, SignatureHelp, SignatureInformation,
};
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
#[must_use]
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
    fn test_analysis_signature_help_convert() {
        let source = "SELECT CONVERT(";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = signature_help_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 15,
            },
        );
        assert!(result.is_some(), "CONVERT should have signature help");
        let sig = result.unwrap();
        assert!(sig.signatures.len() == 1);
        let info = &sig.signatures[0];
        assert!(
            info.label.contains("CONVERT"),
            "Label should contain CONVERT, got: {}",
            info.label
        );
    }

    #[test]
    fn test_analysis_signature_help_getdate_no_params() {
        let source = "SELECT GETDATE(";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = signature_help_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 15,
            },
        );
        assert!(result.is_some(), "GETDATE should have signature help");
        let sig = result.unwrap();
        assert!(sig.signatures.len() == 1);
    }

    #[test]
    fn test_analysis_signature_help_substring_multiple_params() {
        let source = "SELECT SUBSTRING('hello', 2,";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        let result = signature_help_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 28,
            },
        );
        assert!(
            result.is_some(),
            "SUBSTRING should have signature help at 3rd param"
        );
        let sig = result.unwrap();
        assert_eq!(sig.signatures[0].active_parameter, Some(2));
    }
}
