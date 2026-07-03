//! Statement-level MySQL emitter (Task 3.1 / 3.2 / 4.1-4.3).
//!
//! [`MySqlEmitter`] は [`common_sql::ast::Statement`] を MySQL 方言の SQL 文字列へ
//! 変換する。式の描画は [`MySqlExpressionEmitter`] に委譲し、文の構造化生成
//! (SELECT / INSERT / UPDATE / DELETE) を担う。DDL 系 (CREATE/ALTER/DROP TABLE,
//! CREATE/DROP INDEX) は現在 [`EmitError::UnsupportedStatement`] を返す
//! (design Task 4.4 / DD-3: ブリッジが DDL を共通 AST へ出力しないため E2E 未対応)。

use common_sql::ast::clause::{GroupByItem, SortDirection};
use common_sql::ast::identifier::QualifiedName;
use common_sql::ast::join::{Join, JoinType};
use common_sql::ast::{
    DeleteStatement, Expression, InsertSource, InsertStatement, SelectItem, SelectStatement,
    Statement, TableFactor, UpdateStatement,
};

use crate::emitter::MySqlExpressionEmitter;
use crate::{EmitError, EmitterConfig};

/// Statement-level MySQL emitter.
///
/// 共通 AST ([`Statement`]) を受け取り MySQL SQL 文字列を生成する。
/// 式の再帰的な描画は内部の [`MySqlExpressionEmitter`] に委譲する。
#[derive(Debug)]
pub struct MySqlEmitter {
    expr_emitter: MySqlExpressionEmitter,
}

impl MySqlEmitter {
    /// 指定コンフィグで Emitter を構築する。
    #[must_use]
    pub fn new(config: EmitterConfig) -> Self {
        Self {
            expr_emitter: MySqlExpressionEmitter::new(config),
        }
    }

    /// デフォルトコンフィグで Emitter を構築する。
    #[must_use]
    pub fn default_config() -> Self {
        Self::new(EmitterConfig::default())
    }

    /// コンフィグへの参照を返す。
    #[must_use]
    pub fn config(&self) -> &EmitterConfig {
        self.expr_emitter.config()
    }

    /// 単一の [`Expression`] を MySQL SQL 文字列へ描画する (式 emitter へ委譲)。
    ///
    /// [`SyntaxConverter`] / [`crate::converters::FunctionConverter`] 等の外部
    /// コンポーネントが式を描画するための公開 API。
    ///
    /// # Errors
    ///
    /// サポート対象外の式ノードに遭遇した場合 [`EmitError`] を返す。
    pub fn emit_expression(&mut self, e: &Expression) -> Result<String, EmitError> {
        self.expr_emitter.emit_expression(e)
    }

    /// 単一の [`Statement`] を MySQL SQL へ変換する。
    ///
    /// # Errors
    ///
    /// サポート対象外の文 (DDL 系) や、式内のサポート対象外ノードに遭遇した場合
    /// [`EmitError`] を返す。
    pub fn emit(&mut self, stmt: &Statement) -> Result<String, EmitError> {
        match stmt {
            Statement::Select(s) => self.emit_select(s),
            Statement::Insert(s) => self.emit_insert(s),
            Statement::Update(s) => self.emit_update(s),
            Statement::Delete(s) => self.emit_delete(s),
            Statement::CreateTable(_)
            | Statement::AlterTable(_)
            | Statement::DropTable(_)
            | Statement::CreateIndex(_)
            | Statement::DropIndex(_)
            // DialectSpecific (T-SQL control-flow etc.) is out of MySQL emitter
            // scope — tracked by #158 (postgresql-emitter PL/pgSQL restoration).
            | Statement::DialectSpecific { .. } => Err(EmitError::UnsupportedStatement {
                statement_type: statement_kind_name(stmt),
            }),
        }
    }

    /// 複数の [`Statement`] を順に変換し、`;` で区切って結合する。
    ///
    /// # Errors
    ///
    /// いずれかの文でエラーが発生した場合、即座にそのエラーを返す。
    pub fn emit_batch(&mut self, stmts: &[Statement]) -> Result<String, EmitError> {
        let mut out = String::new();
        for (i, s) in stmts.iter().enumerate() {
            if i > 0 {
                out.push_str(";\n");
            }
            out.push_str(&self.emit(s)?);
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // DML generation
    // -----------------------------------------------------------------------

    /// 式を MySQL SQL 文字列へ描画する (式 emitter へ委譲)。
    fn expr(&mut self, e: &Expression) -> Result<String, EmitError> {
        self.expr_emitter.emit_expression(e)
    }

    /// SELECT 文を生成する。
    fn emit_select(&mut self, s: &SelectStatement) -> Result<String, EmitError> {
        let mut sql = String::from("SELECT ");

        // projection
        if s.projection.is_empty() {
            sql.push('*');
        } else {
            let mut parts = Vec::with_capacity(s.projection.len());
            for item in &s.projection {
                parts.push(self.render_select_item(item)?);
            }
            sql.push_str(&parts.join(", "));
        }

        // FROM
        if let Some(from) = &s.from {
            sql.push_str(" FROM ");
            sql.push_str(&self.render_table_factor(from)?);
        }

        // WHERE
        if let Some(w) = &s.where_clause {
            sql.push_str(" WHERE ");
            sql.push_str(&self.expr(w)?);
        }

        // GROUP BY
        if let Some(g) = &s.group_by {
            let mut parts = Vec::with_capacity(g.items.len());
            for item in &g.items {
                parts.push(self.render_group_by_item(item)?);
            }
            sql.push_str(" GROUP BY ");
            sql.push_str(&parts.join(", "));
        }

        // HAVING
        if let Some(h) = &s.having {
            sql.push_str(" HAVING ");
            sql.push_str(&self.expr(h)?);
        }

        // ORDER BY
        if let Some(o) = &s.order_by {
            let mut parts = Vec::with_capacity(o.items.len());
            for item in &o.items {
                let mut p = self.expr(&item.expr)?;
                if let Some(dir) = &item.direction {
                    p.push(' ');
                    p.push_str(match dir {
                        SortDirection::Asc => "ASC",
                        SortDirection::Desc => "DESC",
                    });
                }
                parts.push(p);
            }
            sql.push_str(" ORDER BY ");
            sql.push_str(&parts.join(", "));
        }

        // LIMIT / OFFSET
        if let Some(l) = &s.limit {
            sql.push_str(" LIMIT ");
            sql.push_str(&self.expr(&l.limit)?);
            if let Some(off) = &l.offset {
                sql.push_str(" OFFSET ");
                sql.push_str(&self.expr(off)?);
            }
        }

        Ok(sql)
    }

    /// INSERT 文を生成する。
    fn emit_insert(&mut self, s: &InsertStatement) -> Result<String, EmitError> {
        let mut sql = String::from("INSERT INTO ");
        sql.push_str(&render_qualified_name(&s.table));

        if !s.columns.is_empty() {
            sql.push_str(" (");
            let cols: Vec<String> = s
                .columns
                .iter()
                .map(|c| format!("`{}`", c.value()))
                .collect();
            sql.push_str(&cols.join(", "));
            sql.push(')');
        }

        match &s.source {
            InsertSource::Values(rows) => {
                sql.push_str(" VALUES ");
                let rendered: Vec<String> = rows
                    .iter()
                    .map(|row| {
                        let vals: Vec<String> = row
                            .iter()
                            .map(|v| self.expr(v).unwrap_or_default())
                            .collect();
                        format!("({})", vals.join(", "))
                    })
                    .collect();
                sql.push_str(&rendered.join(", "));
            }
            InsertSource::Select(sel) => {
                sql.push(' ');
                sql.push_str(&self.emit_select(sel)?);
            }
        }

        // ON CONFLICT は PostgreSQL 専用 → MySQL では出力しない (design 非スコープ)

        Ok(sql)
    }

    /// UPDATE 文を生成する。
    fn emit_update(&mut self, s: &UpdateStatement) -> Result<String, EmitError> {
        let mut sql = String::from("UPDATE ");
        sql.push_str(&self.render_table_factor(&s.table)?);

        sql.push_str(" SET ");
        if s.assignments.is_empty() {
            sql.push_str("-- no assignments");
        } else {
            let parts: Vec<String> = s
                .assignments
                .iter()
                .map(|a| {
                    let val = self.expr(&a.value).unwrap_or_default();
                    format!("`{}` = {}", a.column.value(), val)
                })
                .collect();
            sql.push_str(&parts.join(", "));
        }

        if let Some(w) = &s.where_clause {
            sql.push_str(" WHERE ");
            sql.push_str(&self.expr(w)?);
        }

        Ok(sql)
    }

    /// DELETE 文を生成する。
    fn emit_delete(&mut self, s: &DeleteStatement) -> Result<String, EmitError> {
        let mut sql = String::from("DELETE FROM ");
        sql.push_str(&self.render_table_factor(&s.table)?);

        if let Some(w) = &s.where_clause {
            sql.push_str(" WHERE ");
            sql.push_str(&self.expr(w)?);
        }

        Ok(sql)
    }

    // -----------------------------------------------------------------------
    // Rendering helpers
    // -----------------------------------------------------------------------

    /// SELECT リスト項目を描画する。
    fn render_select_item(&mut self, item: &SelectItem) -> Result<String, EmitError> {
        match item {
            SelectItem::Wildcard => Ok("*".to_string()),
            SelectItem::QualifiedWildcard { table } => Ok(format!("`{}`.*", table.value())),
            SelectItem::Expression { expr, alias } => {
                let mut s = self.expr(expr)?;
                if let Some(a) = alias {
                    s.push_str(" AS `");
                    s.push_str(a.value());
                    s.push('`');
                }
                Ok(s)
            }
        }
    }

    /// GROUP BY 項目を描画する。
    fn render_group_by_item(&mut self, item: &GroupByItem) -> Result<String, EmitError> {
        match item {
            GroupByItem::Expression(e) => self.expr(e),
            // Rollup/Cube/GroupingSets は MySQL 8 でも WITH ROLLUP 等の別構文が必要。
            // v1 では集約キー式のみ描画し、複合演算子はプレースホルダー扱い。
            other => Ok(format!("/* unsupported GROUP BY item: {other:?} */")),
        }
    }

    /// FROM 句のテーブル要素を描画する。
    fn render_table_factor(&mut self, factor: &TableFactor) -> Result<String, EmitError> {
        match factor {
            TableFactor::Table { name, alias } => {
                let mut s = render_qualified_name(name);
                if let Some(a) = alias {
                    s.push(' ');
                    s.push_str(a.name());
                }
                Ok(s)
            }
            TableFactor::Derived { subquery, alias } => {
                let inner = self.emit_select(subquery)?;
                let mut s = format!("({inner})");
                if let Some(a) = alias {
                    s.push(' ');
                    s.push_str(a.name());
                }
                Ok(s)
            }
            TableFactor::Join(j) => self.render_join(j),
        }
    }

    /// JOIN を描画する (左項は呼び出し文脈、ここでは右項と結合条件)。
    fn render_join(&mut self, join: &Join) -> Result<String, EmitError> {
        let kw = match join.join_type {
            JoinType::Inner => "INNER JOIN",
            JoinType::Left => "LEFT JOIN",
            JoinType::Right => "RIGHT JOIN",
            JoinType::Full => "FULL JOIN",
            JoinType::Cross => "CROSS JOIN",
        };
        let table = self.render_table_factor(&join.table)?;
        Ok(format!("{kw} {table}"))
    }
}

// -----------------------------------------------------------------------
// Free helpers
// -----------------------------------------------------------------------

/// 修飾テーブル名 (`schema.table` or `table`) を描画する。
fn render_qualified_name(name: &QualifiedName) -> String {
    match name.schema() {
        Some(schema) => format!("{schema}.{}", name.name()),
        None => name.name().to_string(),
    }
}

/// DDL 系の文種別名を返す (エラーメッセージ用)。
fn statement_kind_name(stmt: &Statement) -> String {
    match stmt {
        Statement::CreateTable(_) => "CREATE TABLE".to_string(),
        Statement::AlterTable(_) => "ALTER TABLE".to_string(),
        Statement::DropTable(_) => "DROP TABLE".to_string(),
        Statement::CreateIndex(_) => "CREATE INDEX".to_string(),
        Statement::DropIndex(_) => "DROP INDEX".to_string(),
        // DML は呼び出し側で処理済みのため、ここでは到達しない。
        _ => "UNKNOWN".to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::clause::{LimitClause, OrderByClause, OrderByItem};
    use common_sql::ast::identifier::{Identifier, QualifiedName};
    use common_sql::ast::literal::Literal;

    fn ident(name: &str) -> Identifier {
        Identifier::new(name.to_string())
    }

    fn id_expr(name: &str) -> Expression {
        Expression::Identifier(ident(name))
    }

    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn table(name: &str) -> TableFactor {
        TableFactor::Table {
            name: QualifiedName::new(None, name.to_string()),
            alias: None,
        }
    }

    #[test]
    fn select_wildcard_from_table() {
        let stmt = SelectStatement {
            span: common_sql::ast::Span::new(0, 0),
            with: None,
            projection: vec![SelectItem::Wildcard],
            from: Some(table("users")),
            where_clause: None,
            group_by: None,
            having: None,
            order_by: None,
            limit: None,
        };
        let sql = MySqlEmitter::default_config()
            .emit(&Statement::Select(Box::new(stmt)))
            .unwrap();
        assert_eq!(sql, "SELECT * FROM users");
    }

    #[test]
    fn select_with_where_and_limit() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Expression {
            expr: id_expr("name"),
            alias: None,
        }]);
        stmt.from = Some(table("users"));
        stmt.where_clause = Some(Expression::Comparison {
            left: Box::new(id_expr("id")),
            op: common_sql::ast::ComparisonOperator::Gt,
            right: Box::new(int_expr(100)),
        });
        stmt.limit = Some(LimitClause {
            span: common_sql::ast::Span::new(0, 0),
            limit: int_expr(10),
            offset: None,
        });
        let sql = MySqlEmitter::default_config()
            .emit(&Statement::Select(Box::new(stmt)))
            .unwrap();
        assert!(sql.contains("SELECT `name` FROM users"));
        assert!(sql.contains("WHERE `id` > 100"));
        assert!(sql.contains("LIMIT 10"));
    }

    #[test]
    fn select_order_by_desc() {
        let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
        stmt.from = Some(table("t"));
        stmt.order_by = Some(OrderByClause {
            span: common_sql::ast::Span::new(0, 0),
            items: vec![OrderByItem {
                expr: id_expr("name"),
                direction: Some(SortDirection::Desc),
                nulls: None,
            }],
        });
        let sql = MySqlEmitter::default_config()
            .emit(&Statement::Select(Box::new(stmt)))
            .unwrap();
        assert!(sql.contains("ORDER BY `name` DESC"));
    }

    #[test]
    fn insert_values() {
        let stmt = InsertStatement {
            span: common_sql::ast::Span::new(0, 0),
            table: QualifiedName::new(None, "users".to_string()),
            columns: vec![ident("id"), ident("name")],
            source: InsertSource::Values(vec![vec![int_expr(1), id_expr("a")]]),
            on_conflict: None,
        };
        let sql = MySqlEmitter::default_config()
            .emit(&Statement::Insert(Box::new(stmt)))
            .unwrap();
        assert_eq!(sql, "INSERT INTO users (`id`, `name`) VALUES (1, `a`)");
    }

    #[test]
    fn update_set_where() {
        let stmt = UpdateStatement {
            span: common_sql::ast::Span::new(0, 0),
            table: table("users"),
            assignments: vec![common_sql::ast::Assignment {
                column: ident("name"),
                value: id_expr("x"),
            }],
            from: None,
            where_clause: Some(Expression::Comparison {
                left: Box::new(id_expr("id")),
                op: common_sql::ast::ComparisonOperator::Eq,
                right: Box::new(int_expr(5)),
            }),
        };
        let sql = MySqlEmitter::default_config()
            .emit(&Statement::Update(Box::new(stmt)))
            .unwrap();
        assert_eq!(sql, "UPDATE users SET `name` = `x` WHERE `id` = 5");
    }

    #[test]
    fn delete_from_where() {
        let stmt = DeleteStatement {
            span: common_sql::ast::Span::new(0, 0),
            table: table("users"),
            using: None,
            where_clause: Some(Expression::Comparison {
                left: Box::new(id_expr("id")),
                op: common_sql::ast::ComparisonOperator::Eq,
                right: Box::new(int_expr(5)),
            }),
        };
        let sql = MySqlEmitter::default_config()
            .emit(&Statement::Delete(Box::new(stmt)))
            .unwrap();
        assert_eq!(sql, "DELETE FROM users WHERE `id` = 5");
    }

    #[test]
    fn unsupported_ddl_returns_error() {
        // CreateTable はブリッジ未対応かつ emitter 未サポート → エラー。
        let empty = common_sql::ast::ddl::CreateTableStatement {
            span: common_sql::ast::Span::new(0, 0),
            if_not_exists: false,
            temporary: false,
            name: QualifiedName::new(None, "t".to_string()),
            columns: vec![],
            constraints: vec![],
            options: common_sql::ast::ddl::TableOptions::default(),
        };
        let result = MySqlEmitter::default_config().emit(&Statement::CreateTable(Box::new(empty)));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            EmitError::UnsupportedStatement { ref statement_type } if statement_type == "CREATE TABLE"
        ));
    }

    #[test]
    fn emit_batch_joins_with_semicolon() {
        let s1 = Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Wildcard,
        ])));
        let s2 = Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Wildcard,
        ])));
        let sql = MySqlEmitter::default_config()
            .emit_batch(&[s1, s2])
            .unwrap();
        assert_eq!(sql, "SELECT *;\nSELECT *");
    }
}
