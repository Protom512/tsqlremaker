//! # PostgreSQL Emitter
//!
//! PostgreSQL 方言の SQL を生成する Emitter ライブラリ。
//!
//! ## 概要
//!
//! このライブラリは、Common SQL AST を入力として受け取り、
//! PostgreSQL 方言の SQL 文字列を出力します。
//!
//! ## 使用例
//!
//! 現在は式、関数、識別子のマッパーが使用可能です：
//!
//! ```rust
//! use postgresql_emitter::{ExpressionEmitter, FunctionMapper};
//! use tsql_parser::common::{CommonExpression, CommonLiteral};
//!
//! // 式をPostgreSQL SQLに変換
//! let expr = CommonExpression::Literal(CommonLiteral::Integer(42));
//! let sql = ExpressionEmitter::emit(&expr);
//! assert_eq!(sql, "42");
//!
//! // 関数名のマッピング
//! let func_name = FunctionMapper::map_function_name("GETDATE");
//! assert_eq!(func_name, Some("CURRENT_TIMESTAMP".to_string()));
//! ```
//!
//! ## 機能
//!
//! - Common SQL AST からの PostgreSQL SQL 生成
//! - データ型の変換
//! - 関数の変換
//! - T-SQL 固有構文の変換

#![warn(missing_docs)]
// workspace.lints から clippy 設定を継承
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

mod config;
mod error;
pub mod mappers;

pub use config::EmissionConfig;
pub use error::EmitError;

// よく使うマッパーを再エクスポート
pub use mappers::ExpressionEmitter;
pub use mappers::FunctionMapper;
pub use mappers::IdentifierQuoter;

/// PostgreSQL Emitter
///
/// Common SQL AST を PostgreSQL SQL に変換します。
#[derive(Debug)]
pub struct PostgreSqlEmitter {
    /// 出力バッファ
    buffer: String,
    /// インデントレベル
    indent_level: usize,
    /// コンフィグ
    config: EmissionConfig,
}

impl PostgreSqlEmitter {
    /// 新しい Emitter を作成
    ///
    /// # Arguments
    ///
    /// * `config` - Emitter の設定
    #[must_use]
    pub const fn new(config: EmissionConfig) -> Self {
        Self {
            buffer: String::new(),
            indent_level: 0,
            config,
        }
    }

    /// コンフィグを取得
    #[must_use]
    pub const fn config(&self) -> &EmissionConfig {
        &self.config
    }

    /// バッファをクリア
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.indent_level = 0;
    }

    /// 現在のインデントを取得
    fn current_indent(&self) -> String {
        " ".repeat(self.indent_level * self.config.indent_size)
    }

    /// バッファに文字列を追加
    fn write(&mut self, s: &str) {
        self.buffer.push_str(s);
    }

    /// 改行を追加
    fn writeln(&mut self) {
        self.buffer.push('\n');
    }

    /// インデントを追加
    fn write_indent(&mut self) {
        if self.config.quote_identifiers {
            let indent = self.current_indent();
            self.buffer.push_str(&indent);
        }
    }

    /// インデントを増やす
    fn inc_indent(&mut self) {
        self.indent_level += 1;
    }

    /// インデントを減らす
    fn dec_indent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    /// Common SQL AST を PostgreSQL SQL に変換（単一ステートメント）
    ///
    /// # Arguments
    ///
    /// * `stmt` - Common SQL ステートメント
    ///
    /// # Returns
    ///
    /// PostgreSQL SQL 文字列、またはエラー
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use postgresql_emitter::{PostgreSqlEmitter, EmissionConfig};
    /// use tsql_parser::common::{CommonStatement, CommonSelectStatement, CommonSelectItem};
    /// use tsql_token::Span;
    ///
    /// let config = EmissionConfig::default();
    /// let mut emitter = PostgreSqlEmitter::new(config);
    ///
    /// let stmt = CommonStatement::Select(CommonSelectStatement {
    ///     span: Span { start: 0, end: 10 },
    ///     distinct: false,
    ///     columns: vec![CommonSelectItem::Wildcard],
    ///     from: vec![],
    ///     where_clause: None,
    ///     group_by: vec![],
    ///     having: None,
    ///     order_by: vec![],
    ///     limit: None,
    /// });
    ///
    /// let sql = emitter.emit(&stmt).unwrap();
    /// assert_eq!(sql, "SELECT *");
    /// ```
    pub fn emit(&mut self, stmt: &tsql_parser::common::CommonStatement) -> Result<String, EmitError> {
        self.reset();
        self.visit_statement(stmt)?;
        Ok(self.buffer.clone())
    }

    /// Common SQL AST を PostgreSQL SQL に変換（複数ステートメント）
    ///
    /// # Arguments
    ///
    /// * `stmts` - Common SQL ステートメントのスライス
    ///
    /// # Returns
    ///
    /// PostgreSQL SQL 文字列（セミコロン区切り）、またはエラー
    pub fn emit_batch(
        &mut self,
        stmts: &[tsql_parser::common::CommonStatement],
    ) -> Result<String, EmitError> {
        self.reset();
        for (i, stmt) in stmts.iter().enumerate() {
            self.visit_statement(stmt)?;
            if i < stmts.len() - 1 {
                self.write(";\n");
            }
        }
        Ok(self.buffer.clone())
    }

    /// ステートメントを訪問
    fn visit_statement(
        &mut self,
        stmt: &tsql_parser::common::CommonStatement,
    ) -> Result<(), EmitError> {
        match stmt {
            tsql_parser::common::CommonStatement::Select(select) => {
                self.visit_select_statement(select)
            }
            tsql_parser::common::CommonStatement::Insert(insert) => {
                self.visit_insert_statement(insert)
            }
            tsql_parser::common::CommonStatement::Update(update) => {
                self.visit_update_statement(update)
            }
            tsql_parser::common::CommonStatement::Delete(delete) => {
                self.visit_delete_statement(delete)
            }
            tsql_parser::common::CommonStatement::DialectSpecific { description, .. } => {
                // 方言固有構文はエラーとする
                Err(EmitError::Unsupported(description.clone()))
            }
        }
    }

    /// SELECT文を訪問
    fn visit_select_statement(
        &mut self,
        stmt: &tsql_parser::common::CommonSelectStatement,
    ) -> Result<(), EmitError> {
        self.write("SELECT ");

        if stmt.distinct {
            self.write("DISTINCT ");
        }

        // SELECTリスト
        for (i, item) in stmt.columns.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.visit_select_item(item)?;
        }

        // FROM
        if !stmt.from.is_empty() {
            self.write(" FROM ");
            for (i, table) in stmt.from.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.visit_table_reference(table)?;
            }
        }

        // WHERE
        if let Some(where_clause) = &stmt.where_clause {
            self.write(" WHERE ");
            self.visit_expression(where_clause)?;
        }

        // GROUP BY
        if !stmt.group_by.is_empty() {
            self.write(" GROUP BY ");
            for (i, expr) in stmt.group_by.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.visit_expression(expr)?;
            }
        }

        // HAVING
        if let Some(having) = &stmt.having {
            self.write(" HAVING ");
            self.visit_expression(having)?;
        }

        // ORDER BY
        if !stmt.order_by.is_empty() {
            self.write(" ORDER BY ");
            for (i, item) in stmt.order_by.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.visit_expression(&item.expr)?;
                if item.asc {
                    self.write(" ASC");
                } else {
                    self.write(" DESC");
                }
            }
        }

        // LIMIT
        if let Some(limit) = &stmt.limit {
            self.write(" LIMIT ");
            self.visit_expression(&limit.limit)?;
            if let Some(offset) = &limit.offset {
                self.write(" OFFSET ");
                self.visit_expression(offset)?;
            }
        }

        Ok(())
    }

    /// SELECTアイテムを訪問
    fn visit_select_item(
        &mut self,
        item: &tsql_parser::common::CommonSelectItem,
    ) -> Result<(), EmitError> {
        match item {
            tsql_parser::common::CommonSelectItem::Expression(expr, alias) => {
                self.visit_expression(expr)?;
                if let Some(alias_name) = alias {
                    self.write(" AS ");
                    self.write_identifier(alias_name);
                }
            }
            tsql_parser::common::CommonSelectItem::Wildcard => {
                self.write("*");
            }
            tsql_parser::common::CommonSelectItem::QualifiedWildcard(table) => {
                self.write_identifier(table);
                self.write(".*");
            }
        }
        Ok(())
    }

    /// テーブル参照を訪問
    fn visit_table_reference(
        &mut self,
        table: &tsql_parser::common::CommonTableReference,
    ) -> Result<(), EmitError> {
        match table {
            tsql_parser::common::CommonTableReference::Table { name, alias, .. } => {
                self.write_identifier(name);
                if let Some(alias_name) = alias {
                    self.write(" AS ");
                    self.write_identifier(alias_name);
                }
            }
            tsql_parser::common::CommonTableReference::Derived { subquery, alias, .. } => {
                self.write("(");
                self.visit_select_statement(subquery)?;
                self.write(")");
                if let Some(alias_name) = alias {
                    self.write(" AS ");
                    self.write_identifier(alias_name);
                }
            }
        }
        Ok(())
    }

    /// INSERT文を訪問
    fn visit_insert_statement(
        &mut self,
        stmt: &tsql_parser::common::CommonInsertStatement,
    ) -> Result<(), EmitError> {
        self.write("INSERT INTO ");
        self.write_identifier(&stmt.table);

        // カラムリスト
        if !stmt.columns.is_empty() {
            self.write(" (");
            for (i, col) in stmt.columns.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write_identifier(col);
            }
            self.write(")");
        }

        // VALUES
        match &stmt.source {
            tsql_parser::common::CommonInsertSource::Values(rows) => {
                self.write(" VALUES ");
                for (i, row) in rows.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write("(");
                    for (j, expr) in row.iter().enumerate() {
                        if j > 0 {
                            self.write(", ");
                        }
                        self.visit_expression(expr)?;
                    }
                    self.write(")");
                }
            }
            tsql_parser::common::CommonInsertSource::Select(select) => {
                self.writeln();
                self.visit_select_statement(select)?;
            }
            tsql_parser::common::CommonInsertSource::DefaultValues => {
                self.write(" DEFAULT VALUES");
            }
        }

        Ok(())
    }

    /// UPDATE文を訪問
    fn visit_update_statement(
        &mut self,
        stmt: &tsql_parser::common::CommonUpdateStatement,
    ) -> Result<(), EmitError> {
        self.write("UPDATE ");
        self.write_identifier(&stmt.table);
        self.write(" SET ");

        // 代入リスト
        for (i, assignment) in stmt.assignments.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write_identifier(&assignment.column);
            self.write(" = ");
            self.visit_expression(&assignment.value)?;
        }

        // WHERE
        if let Some(where_clause) = &stmt.where_clause {
            self.write(" WHERE ");
            self.visit_expression(where_clause)?;
        }

        Ok(())
    }

    /// DELETE文を訪問
    fn visit_delete_statement(
        &mut self,
        stmt: &tsql_parser::common::CommonDeleteStatement,
    ) -> Result<(), EmitError> {
        self.write("DELETE FROM ");
        self.write_identifier(&stmt.table);

        // WHERE
        if let Some(where_clause) = &stmt.where_clause {
            self.write(" WHERE ");
            self.visit_expression(where_clause)?;
        }

        Ok(())
    }

    /// 式を訪問
    fn visit_expression(
        &mut self,
        expr: &tsql_parser::common::CommonExpression,
    ) -> Result<(), EmitError> {
        self.write(&mappers::ExpressionEmitter::emit(expr));
        Ok(())
    }

    /// 識別子を書き込む（適切にクォート）
    fn write_identifier(&mut self, name: &str) {
        if self.config.quote_identifiers {
            let quoted = mappers::IdentifierQuoter::quote(name);
            self.write(&quoted);
        } else {
            self.write(name);
        }
    }
}

impl Default for PostgreSqlEmitter {
    fn default() -> Self {
        Self::new(EmissionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_emitter() {
        let config = EmissionConfig::default();
        let emitter = PostgreSqlEmitter::new(config);
        assert_eq!(emitter.indent_level, 0);
        assert!(emitter.buffer.is_empty());
    }

    #[test]
    fn test_default_emitter() {
        let emitter = PostgreSqlEmitter::default();
        assert_eq!(emitter.config().quote_identifiers, true);
        assert_eq!(emitter.config().indent_size, 4);
    }

    #[test]
    fn test_reset() {
        let mut emitter = PostgreSqlEmitter::default();
        emitter.write("SELECT 1");
        emitter.writeln();
        assert!(!emitter.buffer.is_empty());

        emitter.reset();
        assert!(emitter.buffer.is_empty());
        assert_eq!(emitter.indent_level, 0);
    }

    #[test]
    fn test_current_indent() {
        let mut emitter = PostgreSqlEmitter::new(EmissionConfig {
            quote_identifiers: true,
            uppercase_keywords: false,
            indent_size: 2,
        });

        assert_eq!(emitter.current_indent(), "");

        emitter.indent_level = 1;
        assert_eq!(emitter.current_indent(), "  ");

        emitter.indent_level = 2;
        assert_eq!(emitter.current_indent(), "    ");
    }
}
