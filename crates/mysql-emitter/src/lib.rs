//! # MySQL Emitter
//!
//! MySQL 方言の SQL を生成する Emitter ライブラリ。
//!
//! ## 概要
//!
//! このライブラリは、Common SQL AST を入力として受け取り、
//! MySQL 方言の SQL 文字列を出力します。
//!
//! ## 使用例
//!
//! ```rust,ignore
//! use mysql_emitter::{MySqlEmitter, EmitterConfig};
//! use tsql_parser::common::{CommonStatement, CommonExpression};
//!
//! let config = EmitterConfig::default();
//! let mut emitter = MySqlEmitter::new(config);
//!
//! // AST を MySQL SQL に変換
//! let sql = emitter.emit(&statement).unwrap();
//! ```
//!
//! ## 機能
//!
//! - Common SQL AST からの MySQL SQL 生成
//! - データ型の変換
//! - 関数の変換
//! - T-SQL 固有構文の変換

#![warn(missing_docs)]
// workspace.lints から clippy 設定を継承
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

mod config;
mod converters;
mod error;

pub use config::EmitterConfig;
pub use error::EmitError;

use converters::FunctionConverter;
use tsql_parser::common::{
    CommonBinaryOperator, CommonCaseExpression, CommonExpression, CommonFunctionCall,
    CommonIdentifier, CommonInList, CommonLiteral, CommonStatement, CommonUnaryOperator,
};

/// MySQL Emitter
///
/// Common SQL AST を MySQL SQL に変換します。
#[derive(Debug)]
pub struct MySqlEmitter {
    /// 出力バッファ
    buffer: String,
    /// インデントレベル
    indent_level: usize,
    /// コンフィグ
    config: EmitterConfig,
}

impl MySqlEmitter {
    /// 新しい Emitter を作成
    ///
    /// # Arguments
    ///
    /// * `config` - Emitter の設定
    #[must_use]
    pub const fn new(config: EmitterConfig) -> Self {
        Self {
            buffer: String::new(),
            indent_level: 0,
            config,
        }
    }

    /// コンフィグを取得
    #[must_use]
    pub const fn config(&self) -> &EmitterConfig {
        &self.config
    }

    /// Common SQL AST を MySQL SQL に変換（単一ステートメント）
    ///
    /// # Arguments
    ///
    /// * `stmt` - Common SQL ステートメント
    ///
    /// # Returns
    ///
    /// MySQL SQL 文字列、またはエラー
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mysql_emitter::{MySqlEmitter, EmitterConfig};
    /// use tsql_parser::common::{CommonStatement, CommonSelectStatement, CommonSelectItem};
    /// use tsql_token::Span;
    ///
    /// let config = EmitterConfig::default();
    /// let mut emitter = MySqlEmitter::new(config);
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
    pub fn emit(&mut self, stmt: &CommonStatement) -> Result<String, EmitError> {
        self.reset();
        self.visit_statement(stmt)?;
        Ok(self.buffer.clone())
    }

    /// Common SQL AST を MySQL SQL に変換（複数ステートメント）
    ///
    /// # Arguments
    ///
    /// * `stmts` - Common SQL ステートメントのスライス
    ///
    /// # Returns
    ///
    /// MySQL SQL 文字列（セミコロン区切り）、またはエラー
    pub fn emit_batch(&mut self, stmts: &[CommonStatement]) -> Result<String, EmitError> {
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
    fn visit_statement(&mut self, stmt: &CommonStatement) -> Result<(), EmitError> {
        match stmt {
            CommonStatement::Select(select) => self.visit_select_statement(select),
            CommonStatement::Insert(insert) => self.visit_insert_statement(insert),
            CommonStatement::Update(update) => self.visit_update_statement(update),
            CommonStatement::Delete(delete) => self.visit_delete_statement(delete),
            CommonStatement::DialectSpecific { description, .. } => {
                // 方言固有構文はエラーとする
                Err(EmitError::UnsupportedStatement {
                    statement_type: description.clone(),
                })
            }
        }
    }

    /// INSERT文を訪問
    fn visit_insert_statement(
        &mut self,
        stmt: &tsql_parser::common::CommonInsertStatement,
    ) -> Result<(), EmitError> {
        self.write("INSERT INTO `");
        self.write(&stmt.table);
        self.write("`");

        // カラムリスト
        if !stmt.columns.is_empty() {
            self.write(" (");
            for (i, col) in stmt.columns.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&format!("`{}`", col));
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
        self.write("UPDATE `");
        self.write(&stmt.table);
        self.write("` SET ");

        // 代入リスト
        for (i, assignment) in stmt.assignments.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&format!("`{}` = ", assignment.column));
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
        self.write("DELETE FROM `");
        self.write(&stmt.table);
        self.write("`");

        // WHERE
        if let Some(where_clause) = &stmt.where_clause {
            self.write(" WHERE ");
            self.visit_expression(where_clause)?;
        }

        Ok(())
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
        if self.config.format {
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

    /// 式を訪問してMySQL SQL文字列を生成
    ///
    /// # Arguments
    ///
    /// * `expr` - Common SQL 式
    ///
    /// # Returns
    ///
    /// MySQL SQL文字列
    pub fn visit_expression(&mut self, expr: &CommonExpression) -> Result<String, EmitError> {
        let old_buffer = self.buffer.clone();
        self.buffer.clear();

        match expr {
            CommonExpression::Literal(lit) => self.visit_literal(lit),
            CommonExpression::Identifier(ident) => self.visit_identifier(ident),
            CommonExpression::ColumnReference(col) => self.visit_column_reference(col),
            CommonExpression::UnaryOp { op, expr, .. } => self.visit_unary_op(*op, expr),
            CommonExpression::BinaryOp {
                left, op, right, ..
            } => self.visit_binary_op(left, *op, right),
            CommonExpression::FunctionCall(func) => self.visit_function(func),
            CommonExpression::Case(case) => self.visit_case(case),
            CommonExpression::In {
                expr,
                list,
                negated,
                ..
            } => self.visit_in(expr, list, negated),
            CommonExpression::Between {
                expr,
                low,
                high,
                negated,
                ..
            } => self.visit_between(expr, low, high, negated),
            CommonExpression::Like {
                expr,
                pattern,
                escape,
                negated,
                ..
            } => self.visit_like(expr, pattern, escape, negated),
            CommonExpression::IsNull { expr, negated, .. } => self.visit_is_null(expr, negated),
            CommonExpression::Subquery { query, .. } => {
                self.write("(");
                self.visit_select_statement(query)?;
                self.write(")");
                Ok(())
            }
            CommonExpression::Exists { query, .. } => {
                self.write("EXISTS (");
                self.visit_select_statement(query)?;
                self.write(")");
                Ok(())
            }
        }?;

        let result = self.buffer.clone();
        self.buffer = old_buffer;
        Ok(result)
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

        // カラムリスト
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
                self.write(if item.asc { " ASC" } else { " DESC" });
            }
        }

        // LIMIT
        if let Some(limit) = &stmt.limit {
            self.write(" LIMIT ");
            self.visit_expression(&limit.limit)?;
            if let Some(offset) = &limit.offset {
                self.write(" OFFSET ");
                self.visit_expression(offset)?;
                self.write(" ROWS");
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
            tsql_parser::common::CommonSelectItem::Wildcard => {
                self.write("*");
            }
            tsql_parser::common::CommonSelectItem::QualifiedWildcard(table) => {
                self.write(&format!("`{}`.*", table));
            }
            tsql_parser::common::CommonSelectItem::Expression(expr, alias) => {
                self.visit_expression(expr)?;
                if let Some(a) = alias {
                    self.write(&format!(" AS `{}`", a));
                }
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
                self.write(&format!("`{}`", name));
                if let Some(a) = alias {
                    self.write(&format!(" AS `{}`", a));
                }
            }
            tsql_parser::common::CommonTableReference::Derived {
                subquery, alias, ..
            } => {
                self.write("(");
                self.visit_select_statement(subquery)?;
                self.write(")");
                if let Some(a) = alias {
                    self.write(&format!(" AS `{}`", a));
                }
            }
        }
        Ok(())
    }

    /// リテラルを訪問
    fn visit_literal(&mut self, lit: &CommonLiteral) -> Result<(), EmitError> {
        match lit {
            CommonLiteral::String(s) => {
                // 文字列をクォートで囲む
                self.write(&format!("'{}'", s.replace("'", "''")));
            }
            CommonLiteral::Integer(n) => {
                self.write(&n.to_string());
            }
            CommonLiteral::Float(f) => {
                self.write(&f.to_string());
            }
            CommonLiteral::Null => {
                self.write("NULL");
            }
            CommonLiteral::Boolean(b) => {
                self.write(if *b { "TRUE" } else { "FALSE" });
            }
        }
        Ok(())
    }

    /// 識別子を訪問
    fn visit_identifier(&mut self, ident: &CommonIdentifier) -> Result<(), EmitError> {
        // 識別子をバッククォートで囲む（MySQLの識別子エスケープ）
        self.write(&format!("`{}`", ident.name));
        Ok(())
    }

    /// カラム参照を訪問
    fn visit_column_reference(
        &mut self,
        col: &tsql_parser::common::CommonColumnReference,
    ) -> Result<(), EmitError> {
        if let Some(table) = &col.table {
            self.write(&format!("`{}`.`{}`", table, col.column));
        } else {
            self.write(&format!("`{}`", col.column));
        }
        Ok(())
    }

    /// 単項演算子を訪問
    fn visit_unary_op(
        &mut self,
        op: CommonUnaryOperator,
        expr: &CommonExpression,
    ) -> Result<(), EmitError> {
        let op_str = match op {
            CommonUnaryOperator::Plus => "+",
            CommonUnaryOperator::Minus => "-",
            CommonUnaryOperator::Not => "NOT ",
        };
        self.write(op_str);
        let expr_str = self.visit_expression(expr)?;
        self.write(&expr_str);
        Ok(())
    }

    /// 二項演算子を訪問
    fn visit_binary_op(
        &mut self,
        left: &CommonExpression,
        op: CommonBinaryOperator,
        right: &CommonExpression,
    ) -> Result<(), EmitError> {
        let left_str = self.visit_expression(left)?;
        self.write(&left_str);
        self.write(" ");
        self.write(match op {
            CommonBinaryOperator::Plus => "+",
            CommonBinaryOperator::Minus => "-",
            CommonBinaryOperator::Multiply => "*",
            CommonBinaryOperator::Divide => "/",
            CommonBinaryOperator::Modulo => "%",
            CommonBinaryOperator::Eq => "=",
            CommonBinaryOperator::Ne => "!=",
            CommonBinaryOperator::Lt => "<",
            CommonBinaryOperator::Le => "<=",
            CommonBinaryOperator::Gt => ">",
            CommonBinaryOperator::Ge => ">=",
            CommonBinaryOperator::And => "AND",
            CommonBinaryOperator::Or => "OR",
            CommonBinaryOperator::Concat => "||",
        });
        self.write(" ");
        let right_str = self.visit_expression(right)?;
        self.write(&right_str);
        Ok(())
    }

    /// 関数呼び出しを訪問
    fn visit_function(&mut self, func: &CommonFunctionCall) -> Result<(), EmitError> {
        let result = FunctionConverter::convert_function(
            &CommonIdentifier {
                name: func.name.clone(),
            },
            &func.args,
            func.distinct,
            self,
        )?;
        self.write(&result);
        Ok(())
    }

    /// CASE式を訪問
    fn visit_case(&mut self, case: &CommonCaseExpression) -> Result<(), EmitError> {
        self.write("CASE");
        if self.config.format {
            self.writeln();
            self.inc_indent();
        }

        for (condition, result) in &case.branches {
            if self.config.format {
                self.write_indent();
            }
            self.write("WHEN ");
            self.visit_expression(condition)?;
            self.write(" THEN ");
            self.visit_expression(result)?;
            if self.config.format {
                self.writeln();
            }
        }

        if let Some(else_result) = &case.else_result {
            if self.config.format {
                self.write_indent();
            }
            self.write("ELSE ");
            self.visit_expression(else_result)?;
            if self.config.format {
                self.writeln();
            }
        }

        if self.config.format {
            self.dec_indent();
            self.write_indent();
        }
        self.write("END");
        Ok(())
    }

    /// IN式を訪問
    fn visit_in(
        &mut self,
        expr: &CommonExpression,
        list: &CommonInList,
        negated: &bool,
    ) -> Result<(), EmitError> {
        let expr_str = self.visit_expression(expr)?;
        self.write(&expr_str);
        self.write(if *negated { " NOT IN (" } else { " IN (" });

        match list {
            CommonInList::Values(values) => {
                for (i, item) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    let item_str = self.visit_expression(item)?;
                    self.write(&item_str);
                }
            }
            CommonInList::Subquery(subquery) => {
                self.visit_select_statement(subquery)?;
            }
        }

        self.write(")");
        Ok(())
    }

    /// BETWEEN式を訪問
    fn visit_between(
        &mut self,
        expr: &CommonExpression,
        low: &CommonExpression,
        high: &CommonExpression,
        negated: &bool,
    ) -> Result<(), EmitError> {
        let expr_str = self.visit_expression(expr)?;
        self.write(&expr_str);
        self.write(if *negated {
            " NOT BETWEEN "
        } else {
            " BETWEEN "
        });
        let low_str = self.visit_expression(low)?;
        self.write(&low_str);
        self.write(" AND ");
        let high_str = self.visit_expression(high)?;
        self.write(&high_str);
        Ok(())
    }

    /// LIKE式を訪問
    fn visit_like(
        &mut self,
        expr: &CommonExpression,
        pattern: &CommonExpression,
        escape: &Option<Box<CommonExpression>>,
        negated: &bool,
    ) -> Result<(), EmitError> {
        let expr_str = self.visit_expression(expr)?;
        self.write(&expr_str);
        self.write(if *negated { " NOT LIKE " } else { " LIKE " });
        let pattern_str = self.visit_expression(pattern)?;
        self.write(&pattern_str);

        // ESCAPE句を出力
        if let Some(esc) = escape {
            let escape_str = self.visit_expression(esc)?;
            self.write(&format!(" ESCAPE {}", escape_str));
        }

        Ok(())
    }

    /// IS NULL式を訪問
    fn visit_is_null(&mut self, expr: &CommonExpression, negated: &bool) -> Result<(), EmitError> {
        let expr_str = self.visit_expression(expr)?;
        self.write(&expr_str);
        self.write(if *negated { " IS NOT NULL" } else { " IS NULL" });
        Ok(())
    }
}

impl Default for MySqlEmitter {
    fn default() -> Self {
        Self::new(EmitterConfig::default())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use tsql_parser::common::{
        CommonBinaryOperator, CommonCaseExpression, CommonColumnReference, CommonExpression,
        CommonFunctionCall, CommonIdentifier, CommonInList, CommonLiteral, CommonUnaryOperator,
    };

    #[test]
    fn test_new_emitter() {
        let config = EmitterConfig::default();
        let emitter = MySqlEmitter::new(config);
        assert_eq!(emitter.indent_level, 0);
        assert!(emitter.buffer.is_empty());
    }

    #[test]
    fn test_default_emitter() {
        let emitter = MySqlEmitter::default();
        assert!(emitter.config().format);
        assert_eq!(emitter.config().indent_size, 4);
    }

    #[test]
    fn test_reset() {
        let mut emitter = MySqlEmitter::default();
        emitter.write("SELECT 1");
        emitter.writeln();
        assert!(!emitter.buffer.is_empty());

        emitter.reset();
        assert!(emitter.buffer.is_empty());
        assert_eq!(emitter.indent_level, 0);
    }

    #[test]
    fn test_current_indent() {
        let mut emitter = MySqlEmitter::new(EmitterConfig {
            format: true,
            indent_size: 2,
        });

        assert_eq!(emitter.current_indent(), "");

        emitter.indent_level = 1;
        assert_eq!(emitter.current_indent(), "  ");

        emitter.indent_level = 2;
        assert_eq!(emitter.current_indent(), "    ");
    }

    #[test]
    fn test_visit_literal_string() {
        let mut emitter = MySqlEmitter::default();
        let lit = CommonLiteral::String("hello".to_string());
        let result = emitter.visit_expression(&CommonExpression::Literal(lit));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "'hello'");
    }

    #[test]
    fn test_visit_literal_string_with_quote() {
        let mut emitter = MySqlEmitter::default();
        let lit = CommonLiteral::String("it's".to_string());
        let result = emitter.visit_expression(&CommonExpression::Literal(lit));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "'it''s'");
    }

    #[test]
    fn test_visit_literal_integer() {
        let mut emitter = MySqlEmitter::default();
        let lit = CommonLiteral::Integer(42);
        let result = emitter.visit_expression(&CommonExpression::Literal(lit));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "42");
    }

    #[test]
    fn test_visit_literal_float() {
        let mut emitter = MySqlEmitter::default();
        let lit = CommonLiteral::Float(123.456);
        let result = emitter.visit_expression(&CommonExpression::Literal(lit));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "123.456");
    }

    #[test]
    fn test_visit_literal_null() {
        let mut emitter = MySqlEmitter::default();
        let lit = CommonLiteral::Null;
        let result = emitter.visit_expression(&CommonExpression::Literal(lit));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "NULL");
    }

    #[test]
    fn test_visit_literal_boolean_true() {
        let mut emitter = MySqlEmitter::default();
        let lit = CommonLiteral::Boolean(true);
        let result = emitter.visit_expression(&CommonExpression::Literal(lit));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "TRUE");
    }

    #[test]
    fn test_visit_literal_boolean_false() {
        let mut emitter = MySqlEmitter::default();
        let lit = CommonLiteral::Boolean(false);
        let result = emitter.visit_expression(&CommonExpression::Literal(lit));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "FALSE");
    }

    #[test]
    fn test_visit_identifier() {
        let mut emitter = MySqlEmitter::default();
        let ident = CommonIdentifier {
            name: "users".to_string(),
        };
        let result = emitter.visit_expression(&CommonExpression::Identifier(ident));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "`users`");
    }

    #[test]
    fn test_visit_column_reference() {
        let mut emitter = MySqlEmitter::default();
        let col = CommonColumnReference {
            table: None,
            column: "id".to_string(),
        };
        let result = emitter.visit_expression(&CommonExpression::ColumnReference(col));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "`id`");
    }

    #[test]
    fn test_visit_column_reference_with_table() {
        let mut emitter = MySqlEmitter::default();
        let col = CommonColumnReference {
            table: Some("users".to_string()),
            column: "id".to_string(),
        };
        let result = emitter.visit_expression(&CommonExpression::ColumnReference(col));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "`users`.`id`");
    }

    #[test]
    fn test_visit_binary_op() {
        let mut emitter = MySqlEmitter::default();
        let left = CommonExpression::Literal(CommonLiteral::Integer(10));
        let right = CommonExpression::Literal(CommonLiteral::Integer(5));
        let expr = CommonExpression::BinaryOp {
            left: Box::new(left),
            op: CommonBinaryOperator::Plus,
            right: Box::new(right),
            span: tsql_token::Span { start: 0, end: 10 },
        };
        let result = emitter.visit_expression(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "10 + 5");
    }

    #[test]
    fn test_visit_unary_op() {
        let mut emitter = MySqlEmitter::default();
        let expr = CommonExpression::UnaryOp {
            op: CommonUnaryOperator::Minus,
            expr: Box::new(CommonExpression::Literal(CommonLiteral::Integer(5))),
            span: tsql_token::Span { start: 0, end: 10 },
        };
        let result = emitter.visit_expression(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "-5");
    }

    #[test]
    fn test_visit_unary_op_not() {
        let mut emitter = MySqlEmitter::default();
        let expr = CommonExpression::UnaryOp {
            op: CommonUnaryOperator::Not,
            expr: Box::new(CommonExpression::Literal(CommonLiteral::Boolean(true))),
            span: tsql_token::Span { start: 0, end: 10 },
        };
        let result = emitter.visit_expression(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "NOT TRUE");
    }

    #[test]
    fn test_visit_in() {
        let mut emitter = MySqlEmitter::default();
        let expr = CommonExpression::Identifier(CommonIdentifier {
            name: "id".to_string(),
        });
        let list = CommonInList::Values(vec![
            CommonExpression::Literal(CommonLiteral::Integer(1)),
            CommonExpression::Literal(CommonLiteral::Integer(2)),
            CommonExpression::Literal(CommonLiteral::Integer(3)),
        ]);
        let in_expr = CommonExpression::In {
            expr: Box::new(expr),
            list,
            negated: false,
            span: tsql_token::Span { start: 0, end: 10 },
        };
        let result = emitter.visit_expression(&in_expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "`id` IN (1, 2, 3)");
    }

    #[test]
    fn test_visit_not_in() {
        let mut emitter = MySqlEmitter::default();
        let expr = CommonExpression::Identifier(CommonIdentifier {
            name: "id".to_string(),
        });
        let list = CommonInList::Values(vec![
            CommonExpression::Literal(CommonLiteral::Integer(1)),
            CommonExpression::Literal(CommonLiteral::Integer(2)),
        ]);
        let in_expr = CommonExpression::In {
            expr: Box::new(expr),
            list,
            negated: true,
            span: tsql_token::Span { start: 0, end: 10 },
        };
        let result = emitter.visit_expression(&in_expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "`id` NOT IN (1, 2)");
    }

    #[test]
    fn test_visit_between() {
        let mut emitter = MySqlEmitter::default();
        let expr = CommonExpression::Identifier(CommonIdentifier {
            name: "age".to_string(),
        });
        let low = CommonExpression::Literal(CommonLiteral::Integer(18));
        let high = CommonExpression::Literal(CommonLiteral::Integer(65));
        let between_expr = CommonExpression::Between {
            expr: Box::new(expr),
            low: Box::new(low),
            high: Box::new(high),
            negated: false,
            span: tsql_token::Span { start: 0, end: 10 },
        };
        let result = emitter.visit_expression(&between_expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "`age` BETWEEN 18 AND 65");
    }

    #[test]
    fn test_visit_like() {
        let mut emitter = MySqlEmitter::default();
        let expr = CommonExpression::Identifier(CommonIdentifier {
            name: "name".to_string(),
        });
        let pattern = CommonExpression::Literal(CommonLiteral::String("%John%".to_string()));
        let like_expr = CommonExpression::Like {
            expr: Box::new(expr),
            pattern: Box::new(pattern),
            escape: None,
            negated: false,
            span: tsql_token::Span { start: 0, end: 10 },
        };
        let result = emitter.visit_expression(&like_expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "`name` LIKE '%John%'");
    }

    #[test]
    fn test_visit_is_null() {
        let mut emitter = MySqlEmitter::default();
        let expr = CommonExpression::Identifier(CommonIdentifier {
            name: "email".to_string(),
        });
        let is_null_expr = CommonExpression::IsNull {
            expr: Box::new(expr),
            negated: false,
            span: tsql_token::Span { start: 0, end: 10 },
        };
        let result = emitter.visit_expression(&is_null_expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "`email` IS NULL");
    }

    #[test]
    fn test_visit_is_not_null() {
        let mut emitter = MySqlEmitter::default();
        let expr = CommonExpression::Identifier(CommonIdentifier {
            name: "email".to_string(),
        });
        let is_null_expr = CommonExpression::IsNull {
            expr: Box::new(expr),
            negated: true,
            span: tsql_token::Span { start: 0, end: 10 },
        };
        let result = emitter.visit_expression(&is_null_expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "`email` IS NOT NULL");
    }
}
