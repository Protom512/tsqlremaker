//! SELECT文レンダラー
//!
//! Common SQL AST の SELECT 文を PostgreSQL SQL 文字列にレンダリングする。
//!
//! このモジュールは `PostgreSqlEmitter` と `ExpressionEmitter` から再利用される。

use tsql_parser::common::{
    CommonLimitClause, CommonOrderByItem, CommonSelectItem, CommonSelectStatement,
    CommonTableReference,
};

use super::expression::ExpressionEmitter;
use super::identifier::IdentifierQuoter;

/// SELECT文レンダラー
pub struct SelectStatementRenderer;

impl SelectStatementRenderer {
    /// SELECT文をPostgreSQL SQL文字列にレンダリング
    ///
    /// # Arguments
    ///
    /// * `stmt` - レンダリングするSELECT文
    ///
    /// # Returns
    ///
    /// PostgreSQL SQLのSELECT文字列
    #[must_use]
    pub fn emit(stmt: &CommonSelectStatement) -> String {
        let mut parts = Vec::new();

        // SELECT
        parts.push("SELECT".to_string());
        if stmt.distinct {
            parts.push("DISTINCT".to_string());
        }

        // SELECTリスト
        let columns: Vec<String> = stmt.columns.iter().map(Self::emit_select_item).collect();
        parts.push(columns.join(", "));

        // FROM
        if !stmt.from.is_empty() {
            let from: Vec<String> = stmt.from.iter().map(Self::emit_table_reference).collect();
            parts.push(format!("FROM {}", from.join(", ")));
        }

        // WHERE
        if let Some(where_clause) = &stmt.where_clause {
            parts.push(format!("WHERE {}", ExpressionEmitter::emit(where_clause)));
        }

        // GROUP BY
        if !stmt.group_by.is_empty() {
            let group_by: Vec<String> = stmt.group_by.iter().map(ExpressionEmitter::emit).collect();
            parts.push(format!("GROUP BY {}", group_by.join(", ")));
        }

        // HAVING
        if let Some(having) = &stmt.having {
            parts.push(format!("HAVING {}", ExpressionEmitter::emit(having)));
        }

        // ORDER BY
        if !stmt.order_by.is_empty() {
            let order_by: Vec<String> =
                stmt.order_by.iter().map(Self::emit_order_by_item).collect();
            parts.push(format!("ORDER BY {}", order_by.join(", ")));
        }

        // LIMIT
        if let Some(limit) = &stmt.limit {
            parts.push(Self::emit_limit(limit));
        }

        parts.join(" ")
    }

    /// SELECTアイテムをレンダリング
    fn emit_select_item(item: &CommonSelectItem) -> String {
        match item {
            CommonSelectItem::Expression(expr, alias) => {
                let expr_str = ExpressionEmitter::emit(expr);
                if let Some(alias_name) = alias {
                    format!("{} AS {}", expr_str, IdentifierQuoter::quote(alias_name))
                } else {
                    expr_str
                }
            }
            CommonSelectItem::Wildcard => "*".to_string(),
            CommonSelectItem::QualifiedWildcard(table) => {
                format!("{}.*", IdentifierQuoter::quote(table))
            }
        }
    }

    /// テーブル参照をレンダリング
    fn emit_table_reference(table: &CommonTableReference) -> String {
        match table {
            CommonTableReference::Table { name, alias, .. } => {
                let name_str = IdentifierQuoter::quote(name);
                if let Some(alias_name) = alias {
                    format!("{} AS {}", name_str, IdentifierQuoter::quote(alias_name))
                } else {
                    name_str
                }
            }
            CommonTableReference::Derived {
                subquery, alias, ..
            } => {
                let subquery_str = Self::emit(subquery);
                if let Some(alias_name) = alias {
                    format!(
                        "({}) AS {}",
                        subquery_str,
                        IdentifierQuoter::quote(alias_name)
                    )
                } else {
                    format!("({})", subquery_str)
                }
            }
        }
    }

    /// ORDER BYアイテムをレンダリング
    fn emit_order_by_item(item: &CommonOrderByItem) -> String {
        let expr_str = ExpressionEmitter::emit(&item.expr);
        if item.asc {
            format!("{} ASC", expr_str)
        } else {
            format!("{} DESC", expr_str)
        }
    }

    /// LIMIT句をレンダリング
    fn emit_limit(limit: &CommonLimitClause) -> String {
        let limit_str = ExpressionEmitter::emit(&limit.limit);
        if let Some(offset) = &limit.offset {
            let offset_str = ExpressionEmitter::emit(offset);
            format!("LIMIT {} OFFSET {}", limit_str, offset_str)
        } else {
            format!("LIMIT {}", limit_str)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsql_parser::common::{
        CommonExpression, CommonIdentifier, CommonLiteral, CommonSelectStatement,
        CommonTableReference,
    };
    use tsql_parser::Span;

    #[test]
    fn test_emit_simple_select() {
        let stmt = CommonSelectStatement {
            span: Span { start: 0, end: 10 },
            distinct: false,
            columns: vec![CommonSelectItem::Wildcard],
            from: vec![],
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let result = SelectStatementRenderer::emit(&stmt);
        assert_eq!(result, "SELECT *");
    }

    #[test]
    fn test_emit_select_with_columns() {
        let stmt = CommonSelectStatement {
            span: Span { start: 0, end: 20 },
            distinct: false,
            columns: vec![
                CommonSelectItem::Expression(
                    CommonExpression::Literal(CommonLiteral::Integer(1)),
                    Some("id".to_string()),
                ),
                CommonSelectItem::Expression(
                    CommonExpression::Identifier(CommonIdentifier {
                        name: "name".to_string(),
                    }),
                    None,
                ),
            ],
            from: vec![],
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("SELECT"));
        // 識別子は小文字なのでクォート不要
        assert!(result.contains("AS id"));
    }

    #[test]
    fn test_emit_select_with_from() {
        let stmt = CommonSelectStatement {
            span: Span { start: 0, end: 20 },
            distinct: false,
            columns: vec![CommonSelectItem::Wildcard],
            from: vec![CommonTableReference::Table {
                name: "users".to_string(),
                alias: None,
                span: Span { start: 7, end: 11 },
            }],
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let result = SelectStatementRenderer::emit(&stmt);
        assert_eq!(result, "SELECT * FROM users");
    }

    #[test]
    fn test_emit_select_with_where() {
        let stmt = CommonSelectStatement {
            span: Span { start: 0, end: 30 },
            distinct: false,
            columns: vec![CommonSelectItem::Wildcard],
            from: vec![CommonTableReference::Table {
                name: "users".to_string(),
                alias: None,
                span: Span { start: 7, end: 11 },
            }],
            where_clause: Some(CommonExpression::Literal(CommonLiteral::Integer(1))),
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("WHERE 1"));
    }

    #[test]
    fn test_emit_subquery_in_from() {
        // サブクエリを含む FROM 句
        let subquery = CommonSelectStatement {
            span: Span { start: 7, end: 35 },
            distinct: false,
            columns: vec![CommonSelectItem::Wildcard],
            from: vec![CommonTableReference::Table {
                name: "users".to_string(),
                alias: None,
                span: Span { start: 20, end: 24 },
            }],
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let stmt = CommonSelectStatement {
            span: Span { start: 0, end: 50 },
            distinct: false,
            columns: vec![CommonSelectItem::Wildcard],
            from: vec![CommonTableReference::Derived {
                subquery: Box::new(subquery),
                alias: Some("u".to_string()),
                span: Span { start: 7, end: 40 },
            }],
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("SELECT * FROM (SELECT * FROM users) AS u"));
    }

    #[test]
    fn test_emit_select_with_derived_table_no_alias() {
        // エイリアスなしの派生テーブル
        let subquery = CommonSelectStatement {
            span: Span { start: 7, end: 35 },
            distinct: false,
            columns: vec![CommonSelectItem::Wildcard],
            from: vec![CommonTableReference::Table {
                name: "users".to_string(),
                alias: None,
                span: Span { start: 20, end: 24 },
            }],
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let stmt = CommonSelectStatement {
            span: Span { start: 0, end: 40 },
            distinct: false,
            columns: vec![CommonSelectItem::Wildcard],
            from: vec![CommonTableReference::Derived {
                subquery: Box::new(subquery),
                alias: None,
                span: Span { start: 7, end: 35 },
            }],
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("(SELECT * FROM users)"));
    }
}
