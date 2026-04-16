//! Signature Help
//!
//! 組み込み関数のパラメータシグネチャを提供する。

use crate::position_to_offset;
use lsp_types::{
    ParameterInformation, ParameterLabel, Position, SignatureHelp, SignatureInformation,
};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use tsql_lexer::Lexer;
use tsql_token::TokenKind;

/// 関数シグネチャ定義
struct FunctionSignature {
    label: &'static str,
    documentation: &'static str,
    params: Vec<&'static str>,
}

/// 関数シグネチャデータベース
static SIGNATURES: Lazy<HashMap<&str, FunctionSignature>> = Lazy::new(|| {
    let sigs: Vec<FunctionSignature> = vec![
        FunctionSignature {
            label: "SUBSTRING(expression, start, length)",
            documentation: "Extracts a substring from a string expression",
            params: vec!["expression", "start", "length"],
        },
        FunctionSignature {
            label: "CHAR_LENGTH(expression)",
            documentation: "Returns the length of a string in characters",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "UPPER(expression)",
            documentation: "Converts a string to uppercase",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "LOWER(expression)",
            documentation: "Converts a string to lowercase",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "LTRIM(expression)",
            documentation: "Removes leading spaces",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "RTRIM(expression)",
            documentation: "Removes trailing spaces",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "CONVERT(type, expression[, style])",
            documentation: "Converts an expression to the specified data type",
            params: vec!["type", "expression", "style"],
        },
        FunctionSignature {
            label: "CAST(expression AS type)",
            documentation: "Converts an expression to the specified data type",
            params: vec!["expression", "type"],
        },
        FunctionSignature {
            label: "DATEADD(unit, number, date)",
            documentation: "Adds an interval to a date",
            params: vec!["unit", "number", "date"],
        },
        FunctionSignature {
            label: "DATEDIFF(unit, date1, date2)",
            documentation: "Returns the difference between two dates",
            params: vec!["unit", "date1", "date2"],
        },
        FunctionSignature {
            label: "DATEPART(unit, date)",
            documentation: "Extracts a part of a date as an integer",
            params: vec!["unit", "date"],
        },
        FunctionSignature {
            label: "ISNULL(expression, replacement)",
            documentation: "Replaces NULL with the specified replacement value",
            params: vec!["expression", "replacement"],
        },
        FunctionSignature {
            label: "COALESCE(expr1, expr2, ...)",
            documentation: "Returns the first non-NULL expression",
            params: vec!["expr1", "expr2", "..."],
        },
        FunctionSignature {
            label: "COUNT([DISTINCT] expression | *)",
            documentation: "Returns the number of rows",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "SUM([DISTINCT] expression)",
            documentation: "Returns the sum of values",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "AVG([DISTINCT] expression)",
            documentation: "Returns the average of values",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "MIN(expression)",
            documentation: "Returns the minimum value",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "MAX(expression)",
            documentation: "Returns the maximum value",
            params: vec!["expression"],
        },
        FunctionSignature {
            label: "STR_REPLACE(source, pattern, replacement)",
            documentation: "Replaces all occurrences of a pattern in a string",
            params: vec!["source", "pattern", "replacement"],
        },
        FunctionSignature {
            label: "STUFF(source, start, length, insert)",
            documentation: "Deletes and inserts characters at a specified position",
            params: vec!["source", "start", "length", "insert"],
        },
        FunctionSignature {
            label: "ROUND(expression, n)",
            documentation: "Rounds a numeric value to n decimal places",
            params: vec!["expression", "n"],
        },
    ];

    sigs.into_iter()
        .map(|s| {
            let name = s.label.split('(').next().unwrap_or("");
            (name, s)
        })
        .collect()
});

/// SignatureHelp情報を生成する
///
/// カーソル位置が関数呼び出しの引数内にある場合、シグネチャ情報を返す。
pub fn signature_help(source: &str, position: Position) -> Option<SignatureHelp> {
    let offset = position_to_offset(source, position);

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
    let sig = SIGNATURES.get(name.as_str())?;

    let parameters: Vec<ParameterInformation> = sig
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
            label: sig.label.to_string(),
            documentation: Some(lsp_types::Documentation::String(
                sig.documentation.to_string(),
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
}
