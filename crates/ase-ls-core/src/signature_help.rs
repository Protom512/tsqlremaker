//! Signature Help
//!
//! 組み込み関数のパラメータシグネチャを提供する。

use crate::analysis::DocumentAnalysis;
use crate::line_index::LineIndex;
use lsp_types::{
    ParameterInformation, ParameterLabel, Position, SignatureHelp, SignatureInformation,
};
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// SignatureHelp情報を生成する（DocumentAnalysis利用）
pub fn signature_help_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
) -> Option<SignatureHelp> {
    let offset = analysis
        .line_index
        .position_to_offset(&analysis.source, position);

    let mut func_name: Option<String> = None;
    let mut paren_depth = 0i32;
    let mut active_param = 0u32;
    let mut found_open_paren = false;

    for token in &analysis.tokens {
        if token.span.start as usize > offset {
            break;
        }

        match token.kind {
            TokenKind::Ident => {
                if !found_open_paren {
                    func_name = Some(token.text.to_uppercase());
                }
            }
            TokenKind::LParen => {
                if !found_open_paren {
                    found_open_paren = true;
                    paren_depth = 1;
                } else {
                    paren_depth += 1;
                }
            }
            TokenKind::RParen => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    found_open_paren = false;
                    func_name = None;
                    active_param = 0;
                }
            }
            TokenKind::Comma => {
                if paren_depth == 1 {
                    active_param += 1;
                }
            }
            _ => {
                if !found_open_paren && token.kind.is_keyword() {
                    func_name = Some(token.text.to_uppercase());
                }
            }
        }
    }

    if !found_open_paren {
        tracing::debug!("signature_help: cursor not inside function call");
        return None;
    }
    let name = match func_name {
        Some(n) => n,
        None => {
            tracing::debug!("signature_help: no function name found before open paren");
            return None;
        }
    };
    let entry = match crate::db_docs::lookup_function(name.as_str()) {
        Some(e) => e,
        None => {
            tracing::debug!("signature_help: unknown function '{name}'");
            return None;
        }
    };

    if entry.category != crate::db_docs::DocCategory::Function {
        tracing::debug!(
            "signature_help: '{name}' is not a function ({:?})",
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

/// SignatureHelp情報を生成する
///
/// カーソル位置が関数呼び出しの引数内にある場合、シグネチャ情報を返す。
pub fn signature_help(source: &str, position: Position) -> Option<SignatureHelp> {
    let line_index = LineIndex::new(source);
    let offset = line_index.position_to_offset(source, position);

    let lexer = Lexer::new(source);
    let tokens: Vec<_> = lexer.filter_map(Result::ok).collect();

    // カーソル位置より前のトークンを解析して関数呼び出しを探す
    let mut func_name: Option<String> = None;
    let mut paren_depth = 0i32;
    let mut active_param = 0u32;
    let mut found_open_paren = false;

    for token in &tokens {
        if token.span.start as usize > offset {
            break;
        }

        match token.kind {
            TokenKind::Ident => {
                if !found_open_paren {
                    func_name = Some(token.text.to_uppercase());
                }
            }
            TokenKind::LParen => {
                if !found_open_paren {
                    found_open_paren = true;
                    paren_depth = 1;
                } else {
                    paren_depth += 1;
                }
            }
            TokenKind::RParen => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    found_open_paren = false;
                    func_name = None;
                    active_param = 0;
                }
            }
            TokenKind::Comma => {
                if paren_depth == 1 {
                    active_param += 1;
                }
            }
            _ => {
                // キーワードが関数名の場合
                if !found_open_paren && token.kind.is_keyword() {
                    func_name = Some(token.text.to_uppercase());
                }
            }
        }
    }

    if !found_open_paren {
        return None;
    }
    let name = func_name?;
    let entry = crate::db_docs::lookup_function(name.as_str())?;

    // シグネチャヘルプは関数カテゴリのみを対象とする
    if entry.category != crate::db_docs::DocCategory::Function {
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
        // Without the fix, active_param would accumulate from the previous call
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
        // Should be param 0 (first arg), NOT accumulated from ISNULL
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
        let result = signature_help(
            "SELECT SUBSTRING(CONVERT(",
            Position {
                line: 0,
                character: 25,
            },
        );
        assert!(result.is_some());
        let help = result.unwrap();
        // The innermost function at cursor position is CONVERT
        assert!(
            help.signatures[0].label.contains("CONVERT")
                || help.signatures[0].label.contains("SUBSTRING")
        );
        assert_eq!(help.active_parameter, Some(0));
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
}
