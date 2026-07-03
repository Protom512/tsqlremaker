//! # PostgreSQL Emitter
//!
//! PostgreSQL 方言の SQL を生成する Emitter ライブラリ。
//!
//! ## 概要
//!
//! このライブラリは、Common SQL AST ([`common_sql::ast`]) を入力として受け取り、
//! PostgreSQL 方言の SQL 文字列を出力します。
//!
//! ## 設計 (P1 結合負債是正 — architecture §1.2)
//!
//! このクレートは [`common_sql`] のみに依存し、`tsql-parser` / `tsql-token` への
//! 直接依存を持ちません。旧 `tsql_parser::common::*` への依存は削除され、
//! 全て [`common_sql::ast`] の型を消費します。
//!
//! ## 使用例
//!
//! ```rust,ignore
//! use postgresql_emitter::{PostgreSqlEmitter, EmissionConfig};
//! use common_sql::ast::{Statement, SelectStatement, SelectItem};
//!
//! let config = EmissionConfig::default();
//! let mut emitter = PostgreSqlEmitter::new(config);
//!
//! let stmt = Statement::Select(Box::new(SelectStatement::simple(vec![SelectItem::Wildcard])));
//! let sql = emitter.emit(&stmt).unwrap();
//! assert_eq!(sql, "SELECT *");
//! ```
//!
//! ## 機能
//!
//! - SELECT / INSERT / UPDATE / DELETE の生成
//! - 式・データ型・関数の PostgreSQL 方言への変換
//!
//! ## 非スコープ (Issue #157 後続)
//!
//! 旧 `tsql_parser::common::CommonStatement::DialectSpecific` が保持していた
//! T-SQL→PL/pgSQL 変換ヒントコメント生成 (~150行) は、`common_sql::ast::Statement`
//! に等価なバリアントが存在しないため削除された。この振る舞いの復元は別 Issue で
//! 扱う (PR 本文に明記)。

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

use common_sql::ast::{
    DeleteStatement, InsertSource, InsertStatement, SelectStatement, Statement, UpdateStatement,
};

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

    /// 現在のインデントを取得（将来のフォーマット機能用）
    #[allow(dead_code)]
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

    /// インデントを追加（将来のフォーマット機能用）
    #[allow(dead_code)]
    fn write_indent(&mut self) {
        if self.config.quote_identifiers {
            let indent = self.current_indent();
            self.buffer.push_str(&indent);
        }
    }

    /// インデントを増やす（将来のフォーマット機能用）
    #[allow(dead_code)]
    fn inc_indent(&mut self) {
        self.indent_level += 1;
    }

    /// インデントを減らす（将来のフォーマット機能用）
    #[allow(dead_code)]
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
    /// # Errors
    ///
    /// サポート対象外のステートメント (DDL 系) の場合 [`EmitError::Unsupported`] を返す。
    pub fn emit(&mut self, stmt: &Statement) -> Result<String, EmitError> {
        self.reset();
        self.visit_statement(stmt)?;
        Ok(std::mem::take(&mut self.buffer))
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
    ///
    /// # Errors
    ///
    /// いずれかのステートメントでエラーが発生した場合、即座にそのエラーを返す。
    pub fn emit_batch(&mut self, stmts: &[Statement]) -> Result<String, EmitError> {
        self.reset();
        for (i, stmt) in stmts.iter().enumerate() {
            self.visit_statement(stmt)?;
            if i < stmts.len() - 1 {
                self.write(";\n");
            }
        }
        Ok(std::mem::take(&mut self.buffer))
    }

    /// ステートメントを訪問
    fn visit_statement(&mut self, stmt: &Statement) -> Result<(), EmitError> {
        match stmt {
            Statement::Select(select) => self.visit_select_statement(select),
            Statement::Insert(insert) => self.visit_insert_statement(insert),
            Statement::Update(update) => self.visit_update_statement(update),
            Statement::Delete(delete) => self.visit_delete_statement(delete),
            // DDL 系は本 emitter が未対応のため Unsupported を返す。
            Statement::CreateTable(_)
            | Statement::AlterTable(_)
            | Statement::DropTable(_)
            | Statement::CreateIndex(_)
            | Statement::DropIndex(_) => Err(EmitError::Unsupported(statement_kind_name(stmt))),
            // #158: DialectSpecific は common AST で表現できない T-SQL 制御構文等の
            // エスケープハッチ (Option B: verbatim source text)。本 emitter は真の
            // PL/pgSQL 変換を行わず、元ソースを構文カテゴリ別ガイドコメント付きで
            // 出力する graceful fallback を返す (実質 no-op の有効 PostgreSQL)。
            Statement::DialectSpecific { source, .. } => self.visit_dialect_specific(source),
        }
    }

    /// DialectSpecific 構文の graceful fallback (#158)。
    ///
    /// 元の T-SQL ソース (`source`) を構文カテゴリ別の PL/pgSQL 変換ガイドコメントと
    /// 共にコメントアウトして出力する。出力は全行コメント化されるため、PostgreSQL で
    /// 実行しても no-op となる (構文エラーを起こさない)。
    fn visit_dialect_specific(&mut self, source: &str) -> Result<(), EmitError> {
        let trimmed = source.trim_start();
        let upper: String = trimmed
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .map(|c| c.to_ascii_uppercase())
            .collect();
        let category = match upper.as_str() {
            "DECLARE" => "変数宣言 (DECLARE @v → DECLARE v)",
            "SET" => "変数代入 (SET @v = expr → v := expr)",
            "IF" => "条件分岐 (IF ... ELSE → IF ... THEN ... END IF)",
            "WHILE" => "ループ (WHILE ... BEGIN END → WHILE ... LOOP END LOOP)",
            "BEGIN" => "複合ブロック (BEGIN ... END)",
            _ => "サポート対象外の T-SQL 構文",
        };
        self.write("-- [T-SQL → PostgreSQL] ");
        self.write(category);
        self.writeln();
        self.write("-- 元の T-SQL:\n");
        for line in source.lines() {
            self.write("-- ");
            self.write(line);
            self.writeln();
        }
        Ok(())
    }

    /// SELECT文を訪問
    fn visit_select_statement(&mut self, stmt: &SelectStatement) -> Result<(), EmitError> {
        // SelectStatementRenderer に委譲し、結果をバッファへ書き込む。
        let rendered = mappers::SelectStatementRenderer::emit(stmt);
        self.write(&rendered);
        Ok(())
    }

    /// INSERT文を訪問
    fn visit_insert_statement(&mut self, stmt: &InsertStatement) -> Result<(), EmitError> {
        self.write("INSERT INTO ");
        self.write_qualified_name(&stmt.table);

        // カラムリスト
        if !stmt.columns.is_empty() {
            self.write(" (");
            for (i, col) in stmt.columns.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write_identifier(col.value());
            }
            self.write(")");
        }

        // VALUES / SELECT
        // 旧 CommonInsertSource::DefaultValues は common_sql::InsertSource に存在しない
        // → bridge 側で InsertSource::Values(vec![]) にフォールバック済み。
        match &stmt.source {
            InsertSource::Values(rows) => {
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
            InsertSource::Select(select) => {
                self.writeln();
                let rendered = mappers::SelectStatementRenderer::emit(select);
                self.write(&rendered);
            }
        }

        Ok(())
    }

    /// UPDATE文を訪問
    fn visit_update_statement(&mut self, stmt: &UpdateStatement) -> Result<(), EmitError> {
        self.write("UPDATE ");
        self.write_table_factor(&stmt.table);
        self.write(" SET ");

        // 代入リスト
        for (i, assignment) in stmt.assignments.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write_identifier(assignment.column.value());
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
    fn visit_delete_statement(&mut self, stmt: &DeleteStatement) -> Result<(), EmitError> {
        self.write("DELETE FROM ");
        self.write_table_factor(&stmt.table);

        // WHERE
        if let Some(where_clause) = &stmt.where_clause {
            self.write(" WHERE ");
            self.visit_expression(where_clause)?;
        }

        Ok(())
    }

    /// 式を訪問
    fn visit_expression(&mut self, expr: &common_sql::ast::Expression) -> Result<(), EmitError> {
        self.write(&mappers::ExpressionEmitter::emit(expr));
        Ok(())
    }

    /// 修飾テーブル名 (schema.table or table) を書き込む
    fn write_qualified_name(&mut self, name: &common_sql::ast::QualifiedName) {
        match name.schema() {
            Some(schema) => {
                self.write_identifier(schema);
                self.write(".");
                self.write_identifier(name.name());
            }
            None => self.write_identifier(name.name()),
        }
    }

    /// テーブル要素 (TableFactor) を書き込む
    /// 旧 table: Identifier(String) → TableFactor への差替に対応。
    fn write_table_factor(&mut self, factor: &common_sql::ast::TableFactor) {
        match factor {
            common_sql::ast::TableFactor::Table { name, alias } => {
                self.write_qualified_name(name);
                if let Some(alias_name) = alias {
                    self.write(" AS ");
                    self.write_identifier(alias_name.name());
                }
            }
            common_sql::ast::TableFactor::Derived { subquery, alias } => {
                self.write("(");
                let rendered = mappers::SelectStatementRenderer::emit(subquery);
                self.write(&rendered);
                self.write(")");
                if let Some(alias_name) = alias {
                    self.write(" AS ");
                    self.write_identifier(alias_name.name());
                }
            }
            common_sql::ast::TableFactor::Join(join) => {
                let kw = match join.join_type {
                    common_sql::ast::JoinType::Inner => "INNER JOIN",
                    common_sql::ast::JoinType::Left => "LEFT JOIN",
                    common_sql::ast::JoinType::Right => "RIGHT JOIN",
                    common_sql::ast::JoinType::Full => "FULL JOIN",
                    common_sql::ast::JoinType::Cross => "CROSS JOIN",
                };
                self.write(kw);
                self.write(" ");
                self.write_table_factor(&join.table);
            }
        }
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

/// DDL 系の文種別名を返す (エラーメッセージ用)。
fn statement_kind_name(stmt: &Statement) -> String {
    match stmt {
        Statement::CreateTable(_) => "CREATE TABLE".to_string(),
        Statement::AlterTable(_) => "ALTER TABLE".to_string(),
        Statement::DropTable(_) => "DROP TABLE".to_string(),
        Statement::CreateIndex(_) => "CREATE INDEX".to_string(),
        Statement::DropIndex(_) => "DROP INDEX".to_string(),
        // DML/SELECT は呼び出し側で処理済みのため、ここでは到達しない。
        _ => "UNKNOWN".to_string(),
    }
}

impl Default for PostgreSqlEmitter {
    fn default() -> Self {
        Self::new(EmissionConfig::default())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::identifier::{Identifier, QualifiedName};
    use common_sql::ast::{
        Assignment, CreateTableStatement, Expression, InsertSource, Literal, SelectItem,
        SelectStatement, TableFactor, TableOptions,
    };

    // ---- 構築ヘルパー ----

    fn ident_expr(name: &str) -> Expression {
        Expression::Identifier(Identifier::new(name.to_string()))
    }

    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn select_stmt() -> Statement {
        Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Wildcard,
        ])))
    }

    fn table(name: &str) -> TableFactor {
        TableFactor::Table {
            name: QualifiedName::new(None, name.to_string()),
            alias: None,
        }
    }

    // ===== Emitter 構築 =====

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
        assert!(emitter.config().quote_identifiers);
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
            warn_unsupported: false,
        });

        assert_eq!(emitter.current_indent(), "");

        emitter.indent_level = 1;
        assert_eq!(emitter.current_indent(), "  ");

        emitter.indent_level = 2;
        assert_eq!(emitter.current_indent(), "    ");
    }

    // ===== SELECT =====

    #[test]
    fn test_emit_select_wildcard() {
        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&select_stmt()).unwrap();
        assert_eq!(sql, "SELECT *");
    }

    #[test]
    fn test_emit_select_with_from() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table("users"));
        let stmt = Statement::Select(Box::new(sel));

        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT * FROM users");
    }

    #[test]
    fn test_emit_select_with_where() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table("users"));
        sel.where_clause = Some(int_expr(1));
        let stmt = Statement::Select(Box::new(sel));

        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert!(sql.contains("WHERE 1"));
    }

    // ===== INSERT =====

    #[test]
    fn test_emit_insert_values() {
        // INSERT INTO users (id) VALUES (1)
        let stmt = Statement::Insert(Box::new(InsertStatement {
            span: common_sql::ast::Span::new(0, 30),
            table: QualifiedName::new(None, "users".to_string()),
            columns: vec![Identifier::new("id".to_string())],
            source: InsertSource::Values(vec![vec![int_expr(1)]]),
            on_conflict: None,
        }));

        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "INSERT INTO users (id) VALUES (1)");
    }

    #[test]
    fn test_emit_insert_multiple_rows() {
        // INSERT INTO t (id) VALUES (1), (2)
        let stmt = Statement::Insert(Box::new(InsertStatement {
            span: common_sql::ast::Span::new(0, 40),
            table: QualifiedName::new(None, "t".to_string()),
            columns: vec![Identifier::new("id".to_string())],
            source: InsertSource::Values(vec![vec![int_expr(1)], vec![int_expr(2)]]),
            on_conflict: None,
        }));

        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "INSERT INTO t (id) VALUES (1), (2)");
    }

    #[test]
    fn test_emit_insert_no_columns() {
        // INSERT INTO t VALUES (1)  (column list omitted)
        let stmt = Statement::Insert(Box::new(InsertStatement {
            span: common_sql::ast::Span::new(0, 20),
            table: QualifiedName::new(None, "t".to_string()),
            columns: vec![],
            source: InsertSource::Values(vec![vec![int_expr(1)]]),
            on_conflict: None,
        }));

        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "INSERT INTO t VALUES (1)");
    }

    #[test]
    fn test_emit_insert_select() {
        // INSERT INTO archive (id) SELECT id FROM source
        let mut sel = SelectStatement::simple(vec![SelectItem::Expression {
            expr: ident_expr("id"),
            alias: None,
        }]);
        sel.from = Some(table("source"));
        let stmt = Statement::Insert(Box::new(InsertStatement {
            span: common_sql::ast::Span::new(0, 50),
            table: QualifiedName::new(None, "archive".to_string()),
            columns: vec![Identifier::new("id".to_string())],
            source: InsertSource::Select(Box::new(sel)),
            on_conflict: None,
        }));

        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert!(sql.contains("INSERT INTO archive (id)"));
        assert!(sql.contains("SELECT id FROM source"));
    }

    #[test]
    fn test_emit_insert_default_values_falls_back_to_empty_values() {
        // 旧 CommonInsertSource::DefaultValues は bridge で InsertSource::Values(vec![])
        // にフォールバックされる。空 VALUES を出力する。
        let stmt = Statement::Insert(Box::new(InsertStatement {
            span: common_sql::ast::Span::new(0, 20),
            table: QualifiedName::new(None, "t".to_string()),
            columns: vec![],
            source: InsertSource::Values(vec![]),
            on_conflict: None,
        }));

        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        // 空 VALUES リスト: "VALUES " のみ (行がない)
        assert!(sql.contains("VALUES"));
    }

    // ===== UPDATE =====

    #[test]
    fn test_emit_update() {
        // UPDATE users SET name = 1 WHERE id
        let stmt = Statement::Update(Box::new(UpdateStatement {
            span: common_sql::ast::Span::new(0, 40),
            table: table("users"),
            assignments: vec![Assignment {
                column: Identifier::new("name".to_string()),
                value: int_expr(1),
            }],
            from: None,
            where_clause: Some(ident_expr("id")),
        }));

        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        // "name" is PostgreSQL reserved → quoted
        assert!(sql.contains("UPDATE users SET \"name\" = 1 WHERE id"));
    }

    // ===== DELETE =====

    #[test]
    fn test_emit_delete() {
        let stmt = Statement::Delete(Box::new(DeleteStatement {
            span: common_sql::ast::Span::new(0, 30),
            table: table("users"),
            using: None,
            where_clause: Some(ident_expr("id")),
        }));

        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "DELETE FROM users WHERE id");
    }

    // ===== バッチ =====

    #[test]
    fn test_emit_batch() {
        let stmts = vec![select_stmt(), select_stmt()];
        let mut emitter = PostgreSqlEmitter::default();
        let sql = emitter.emit_batch(&stmts).unwrap();
        assert!(sql.contains("SELECT *;\nSELECT *"));
    }

    // ===== DDL は Unsupported =====

    #[test]
    fn test_emit_ddl_returns_unsupported() {
        // common_sql Statement に DialectSpecific はない。
        // DDL 系は本 emitter が未対応のため Unsupported を返す。
        let ddl = Statement::CreateTable(Box::new(CreateTableStatement {
            span: common_sql::ast::Span::new(0, 10),
            if_not_exists: false,
            temporary: false,
            name: QualifiedName::new(None, "t".to_string()),
            columns: vec![],
            constraints: vec![],
            options: TableOptions {
                engine: None,
                charset: None,
                collation: None,
                comment: None,
            },
        }));
        let mut emitter = PostgreSqlEmitter::default();
        let result = emitter.emit(&ddl);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmitError::Unsupported(msg) => assert_eq!(msg, "CREATE TABLE"),
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }
}
