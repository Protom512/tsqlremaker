//! SELECT文レンダラー
//!
//! Common SQL AST (`common_sql::ast`) の SELECT 文を PostgreSQL SQL 文字列に
//! レンダリングする。
//!
//! このモジュールは `PostgreSqlEmitter` (lib.rs) と `ExpressionEmitter` から
//! 再利用される。旧 `tsql_parser::common::*` への依存は P1 結合負債是正
//! (architecture §1.2) により削除され、`common_sql::ast` のみを消費する。

use common_sql::ast::clause::{GroupByItem, LimitClause, OrderByClause, SortDirection};
use common_sql::ast::identifier::QualifiedName;
use common_sql::ast::join::{Join, JoinType};
use common_sql::ast::{SelectItem, SelectStatement, TableFactor};

use super::expression::ExpressionEmitter;
use super::identifier::IdentifierQuoter;

/// SELECT文レンダラー
///
/// [`SelectStatement`] を PostgreSQL SQL 文字列へ変換する。式の描画は
/// [`ExpressionEmitter`] へ委譲する。
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
    pub fn emit(stmt: &SelectStatement) -> String {
        let mut parts = Vec::new();

        // SELECT
        parts.push("SELECT".to_string());

        // SELECTリスト (projection)
        // 旧 columns (Vec) → projection (Vec<SelectItem>)。
        // projection が空の場合は '*' を出力する (旧 columns が空のときの挙動と互換)。
        if stmt.projection.is_empty() {
            parts.push("*".to_string());
        } else {
            let columns: Vec<String> = stmt.projection.iter().map(Self::emit_select_item).collect();
            parts.push(columns.join(", "));
        }

        // FROM: 旧 Vec<CommonTableReference> → Option<TableFactor>
        // 空 Vec 判定は None 判定へ吸収済み (bridge 側で collapse)。
        if let Some(from) = &stmt.from {
            parts.push(format!("FROM {}", Self::emit_table_factor(from)));
        }

        // WHERE
        if let Some(where_clause) = &stmt.where_clause {
            parts.push(format!("WHERE {}", ExpressionEmitter::emit(where_clause)));
        }

        // GROUP BY: Vec → Option<GroupByClause>
        if let Some(group_by) = &stmt.group_by {
            let items: Vec<String> = group_by
                .items
                .iter()
                .map(Self::emit_group_by_item)
                .collect();
            parts.push(format!("GROUP BY {}", items.join(", ")));
        }

        // HAVING
        if let Some(having) = &stmt.having {
            parts.push(format!("HAVING {}", ExpressionEmitter::emit(having)));
        }

        // ORDER BY: Vec<CommonOrderByItem> → Option<OrderByClause>
        if let Some(order_by) = &stmt.order_by {
            parts.push(format!("ORDER BY {}", Self::emit_order_by_clause(order_by)));
        }

        // LIMIT
        if let Some(limit) = &stmt.limit {
            parts.push(Self::emit_limit(limit));
        }

        parts.join(" ")
    }

    /// SELECTアイテムをレンダリング
    fn emit_select_item(item: &SelectItem) -> String {
        match item {
            SelectItem::Expression { expr, alias } => {
                let expr_str = ExpressionEmitter::emit(expr);
                if let Some(alias_name) = alias {
                    format!(
                        "{} AS {}",
                        expr_str,
                        IdentifierQuoter::quote(alias_name.value())
                    )
                } else {
                    expr_str
                }
            }
            SelectItem::Wildcard => "*".to_string(),
            SelectItem::QualifiedWildcard { table } => {
                format!("{}.*", IdentifierQuoter::quote(table.value()))
            }
        }
    }

    /// GROUP BYアイテムをレンダリング
    /// Rollup/Cube/GroupingSets は PostgreSQL も別構文だが v1 では式のみ描画。
    fn emit_group_by_item(item: &GroupByItem) -> String {
        match item {
            GroupByItem::Expression(expr) => ExpressionEmitter::emit(expr),
            // 複合演算子はプレースホルダー (mysql-emitter の DD-3 パターンと同等)。
            other => format!("/* unsupported GROUP BY item: {other:?} */"),
        }
    }

    /// テーブル要素 (TableFactor) をレンダリング
    /// 旧 CommonTableReference{Table,Derived} → TableFactor{Table,Derived,Join}
    /// 第3バリアント (Join) 追加対応。
    fn emit_table_factor(factor: &TableFactor) -> String {
        match factor {
            TableFactor::Table { name, alias } => {
                let name_str = Self::emit_qualified_name(name);
                if let Some(alias_name) = alias {
                    format!(
                        "{} AS {}",
                        name_str,
                        IdentifierQuoter::quote(alias_name.name())
                    )
                } else {
                    name_str
                }
            }
            TableFactor::Derived { subquery, alias } => {
                let subquery_str = Self::emit(subquery);
                if let Some(alias_name) = alias {
                    format!(
                        "({}) AS {}",
                        subquery_str,
                        IdentifierQuoter::quote(alias_name.name())
                    )
                } else {
                    format!("({})", subquery_str)
                }
            }
            TableFactor::Join(join) => Self::emit_join(join),
        }
    }

    /// JOIN をレンダリング (左項は呼び出し文脈、ここでは右項と結合条件)。
    fn emit_join(join: &Join) -> String {
        let kw = match join.join_type {
            JoinType::Inner => "INNER JOIN",
            JoinType::Left => "LEFT JOIN",
            JoinType::Right => "RIGHT JOIN",
            JoinType::Full => "FULL JOIN",
            JoinType::Cross => "CROSS JOIN",
        };
        let table = Self::emit_table_factor(&join.table);
        format!("{kw} {table}")
    }

    /// 修飾テーブル名 (schema.table or table) をレンダリング
    fn emit_qualified_name(name: &QualifiedName) -> String {
        match name.schema() {
            Some(schema) => format!(
                "{}.{}",
                IdentifierQuoter::quote(schema),
                IdentifierQuoter::quote(name.name())
            ),
            None => IdentifierQuoter::quote(name.name()),
        }
    }

    /// ORDER BY句をレンダリング
    /// 旧 CommonOrderByItem{expr, asc: bool} → OrderByItem{expr, direction: Option<SortDirection>}
    fn emit_order_by_clause(order_by: &OrderByClause) -> String {
        let items: Vec<String> = order_by
            .items
            .iter()
            .map(|item| {
                let expr_str = ExpressionEmitter::emit(&item.expr);
                // direction が None のときは ASC/DESCを出さない (DB default)。
                match item.direction {
                    Some(SortDirection::Asc) => format!("{expr_str} ASC"),
                    Some(SortDirection::Desc) => format!("{expr_str} DESC"),
                    None => expr_str,
                }
            })
            .collect();
        items.join(", ")
    }

    /// LIMIT句をレンダリング
    fn emit_limit(limit: &LimitClause) -> String {
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
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::clause::{
        GroupByClause, GroupByItem, LimitClause, OrderByClause, OrderByItem, SortDirection,
    };
    use common_sql::ast::identifier::{Identifier, QualifiedName, TableAlias};
    use common_sql::ast::join::{Join, JoinCondition, JoinType, TableFactor};
    use common_sql::ast::{Expression, Literal, SelectItem};

    // ---- 構築ヘルパー ----

    fn ident_expr(name: &str) -> Expression {
        Expression::Identifier(Identifier::new(name.to_string()))
    }

    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn table_factor(name: &str) -> TableFactor {
        TableFactor::Table {
            name: QualifiedName::new(None, name.to_string()),
            alias: None,
        }
    }

    // ===== 正常系 =====

    #[test]
    fn test_emit_simple_select_wildcard() {
        // SELECT * (projection なし → '*' フォールバック)
        let stmt = SelectStatement::simple(vec![]);

        let result = SelectStatementRenderer::emit(&stmt);
        assert_eq!(result, "SELECT *");
    }

    #[test]
    fn test_emit_select_with_explicit_wildcard() {
        let stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);

        let result = SelectStatementRenderer::emit(&stmt);
        assert_eq!(result, "SELECT *");
    }

    #[test]
    fn test_emit_select_with_columns_and_alias() {
        // SELECT 1 AS id, name
        let stmt = SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: int_expr(1),
                alias: Some(Identifier::new("id".to_string())),
            },
            SelectItem::Expression {
                expr: ident_expr("name"),
                alias: None,
            },
        ]);

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("SELECT"));
        // 小文字識別子はクォート不要
        assert!(result.contains("AS id"));
        assert!(result.contains("name"));
    }

    #[test]
    fn test_emit_select_with_from_table() {
        // SELECT * FROM users
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(table_factor("users"));

        let result = SelectStatementRenderer::emit(&stmt);
        assert_eq!(result, "SELECT * FROM users");
    }

    #[test]
    fn test_emit_select_with_from_none_omits_from() {
        // from が None のとき FROM 句を出力しない (空 Vec 判定 → None 判定)
        let stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(!result.contains("FROM"));
    }

    #[test]
    fn test_emit_select_with_where() {
        // SELECT * FROM users WHERE 1
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(table_factor("users"));
        stmt.where_clause = Some(int_expr(1));

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("WHERE 1"));
    }

    #[test]
    fn test_emit_subquery_in_from_with_alias() {
        // SELECT * FROM (SELECT * FROM users) AS u
        let mut subquery = SelectStatement::simple(vec![SelectItem::Wildcard]);
        subquery.from = Some(table_factor("users"));

        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(TableFactor::Derived {
            subquery: Box::new(subquery),
            alias: Some(TableAlias::new("u".to_string(), vec![])),
        });

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("SELECT * FROM (SELECT * FROM users) AS u"));
    }

    #[test]
    fn test_emit_derived_table_without_alias() {
        // エイリアスなしの派生テーブル
        let mut subquery = SelectStatement::simple(vec![SelectItem::Wildcard]);
        subquery.from = Some(table_factor("users"));

        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(TableFactor::Derived {
            subquery: Box::new(subquery),
            alias: None,
        });

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("(SELECT * FROM users)"));
        assert!(!result.contains("AS"));
    }

    #[test]
    fn test_emit_group_by() {
        // SELECT * FROM users GROUP BY dept
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(table_factor("users"));
        stmt.group_by = Some(GroupByClause {
            span: common_sql::ast::Span::new(0, 10),
            items: vec![GroupByItem::Expression(ident_expr("dept"))],
        });

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("GROUP BY dept"));
    }

    #[test]
    fn test_emit_having() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.having = Some(ident_expr("count"));

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("HAVING count"));
    }

    #[test]
    fn test_emit_order_by_asc() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.order_by = Some(OrderByClause {
            span: common_sql::ast::Span::new(0, 10),
            items: vec![OrderByItem {
                expr: ident_expr("name"),
                direction: Some(SortDirection::Asc),
                nulls: None,
            }],
        });

        let result = SelectStatementRenderer::emit(&stmt);
        // `name` は PostgreSQL 予約語 (IdentifierQuoter::RESERVED_WORDS) のため "name" と
        // ダブルクォートされる。mysql-emitter (PR #152) の `` `name` `` バックティック期待と同一方針。
        assert!(result.contains("ORDER BY \"name\" ASC"));
    }

    #[test]
    fn test_emit_order_by_desc() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.order_by = Some(OrderByClause {
            span: common_sql::ast::Span::new(0, 10),
            items: vec![OrderByItem {
                expr: ident_expr("name"),
                direction: Some(SortDirection::Desc),
                nulls: None,
            }],
        });

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("ORDER BY \"name\" DESC"));
    }

    #[test]
    fn test_emit_order_by_implicit_asc_no_direction() {
        // direction が None のとき ASC/DESCを出力しない (DB default)
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.order_by = Some(OrderByClause {
            span: common_sql::ast::Span::new(0, 10),
            items: vec![OrderByItem {
                expr: ident_expr("name"),
                direction: None,
                nulls: None,
            }],
        });

        let result = SelectStatementRenderer::emit(&stmt);
        // `name` は予約語のため "name" とクォートされる (direction なしでもクォート自体は維持)
        assert!(result.contains("ORDER BY \"name\""));
        assert!(!result.contains("ASC"));
        assert!(!result.contains("DESC"));
    }

    #[test]
    fn test_emit_limit_without_offset() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.limit = Some(LimitClause {
            span: common_sql::ast::Span::new(0, 5),
            limit: int_expr(10),
            offset: None,
        });

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("LIMIT 10"));
        assert!(!result.contains("OFFSET"));
    }

    #[test]
    fn test_emit_limit_with_offset() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.limit = Some(LimitClause {
            span: common_sql::ast::Span::new(0, 10),
            limit: int_expr(20),
            offset: Some(int_expr(5)),
        });

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("LIMIT 20 OFFSET 5"));
    }

    #[test]
    fn test_emit_qualified_table_name() {
        // SELECT * FROM dbo.users (schema.table)
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(TableFactor::Table {
            name: QualifiedName::new(Some("dbo".to_string()), "users".to_string()),
            alias: None,
        });

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("FROM dbo.users"));
    }

    #[test]
    fn test_emit_table_with_alias() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(TableFactor::Table {
            name: QualifiedName::new(None, "users".to_string()),
            alias: Some(TableAlias::new("u".to_string(), vec![])),
        });

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("FROM users AS u"));
    }

    #[test]
    fn test_emit_qualified_wildcard() {
        // SELECT u.* FROM ...
        let stmt = SelectStatement::simple(vec![SelectItem::QualifiedWildcard {
            table: Identifier::new("u".to_string()),
        }]);

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("u.*"));
    }

    // ===== 第3バリアント (Join) の対応 =====

    #[test]
    fn test_emit_select_with_inner_join() {
        // SELECT * FROM users INNER JOIN orders
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(TableFactor::Join(Box::new(Join {
            span: common_sql::ast::Span::new(0, 30),
            join_type: JoinType::Inner,
            table: table_factor("orders"),
            condition: JoinCondition::On(ident_expr("users.id")),
            lateral: false,
        })));

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("INNER JOIN orders"));
    }

    #[test]
    fn test_emit_select_with_left_join() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(TableFactor::Join(Box::new(Join {
            span: common_sql::ast::Span::new(0, 30),
            join_type: JoinType::Left,
            table: table_factor("profiles"),
            condition: JoinCondition::Natural,
            lateral: false,
        })));

        let result = SelectStatementRenderer::emit(&stmt);
        assert!(result.contains("LEFT JOIN profiles"));
    }

    // ===== エッジケース =====

    #[test]
    fn test_emit_empty_projection_yields_wildcard() {
        let stmt = SelectStatement::simple(vec![]);
        let result = SelectStatementRenderer::emit(&stmt);
        assert_eq!(result, "SELECT *");
    }

    #[test]
    fn test_emit_distinct_not_in_select_statement() {
        // common_sql::ast::SelectStatement には distinct フィールドがない
        // (旧 CommonSelectStatement.distinct: bool は削除された)。
        // このテストは distinct が存在しないことを文書化するガード。
        let stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let result = SelectStatementRenderer::emit(&stmt);
        // DISTINCT が出力されないことを確認
        assert!(!result.contains("DISTINCT"));
    }
}
