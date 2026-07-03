//! # SQLite Emitter
//!
//! SQLite 方言の SQL を生成する Emitter ライブラリ。
//!
//! ## 概要
//!
//! このライブラリは、Common SQL AST ([`common_sql::ast`]) を入力として受け取り、
//! SQLite 方言の SQL 文字列を出力します。
//!
//! ## 設計 (P1 結合負債是正 — architecture §1.2)
//!
//! このクレートは [`common_sql`] のみに依存し、`tsql-parser` / `tsql-token` への
//! 直接依存を持ちません。旧 `tsql_parser::common::*` への依存は削除され、
//! 全て [`common_sql::ast`] の型を消費します。
//! PR #152 (mysql-emitter) / PR #159 (postgresql-emitter) と同一パターンです。
//!
//! ## 使用例
//!
//! ```rust,ignore
//! use sqlite_emitter::{SqliteEmitter, EmitterConfig};
//! use common_sql::ast::{Statement, SelectStatement, SelectItem};
//!
//! let config = EmitterConfig::default();
//! let mut emitter = SqliteEmitter::new(config);
//!
//! let stmt = Statement::Select(Box::new(SelectStatement::simple(vec![SelectItem::Wildcard])));
//! let sql = emitter.emit(&stmt).unwrap();
//! assert_eq!(sql, "SELECT *");
//! ```
//!
//! ## 機能
//!
//! - SELECT / INSERT / UPDATE / DELETE の SQLite SQL 生成
//! - データ型の変換 (CAST 式)
//! - T-SQL 関数の SQLite 関数への変換 (LEN→length, ISNULL→ifnull 等)
//! - 日付関数 DATEADD / DATEDIFF の SQLite 修飾子形式への変換
//!
//! ## 非スコープ
//!
//! `common_sql::ast::Statement::DialectSpecific` (T-SQL 制御構文等の方言固有文)
//! は #158 で AST に再追加されたが、本 emitter は SQLite 向け native 変換を
//! 行わないため [`EmitError::Unsupported`] を返します (旧実装の T-SQL→SQLite
//! ヒント生成の復元は #158 で追跡)。DDL 系 (CREATE/ALTER/DROP TABLE/INDEX) も
//! 同様に [`EmitError::Unsupported`] を返します。

#![warn(missing_docs)]
// workspace.lints から clippy 設定を継承
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

mod config;
mod error;
mod function_mapper;

pub use config::EmitterConfig;
pub use error::EmitError;

use common_sql::ast::clause::{GroupByItem, OrderByClause, SortDirection};
use common_sql::ast::identifier::QualifiedName;
use common_sql::ast::join::{Join, JoinType};
use common_sql::ast::{
    BinaryOperator, ComparisonOperator, DataType, DeleteStatement, Expression, Identifier, InList,
    InsertSource, InsertStatement, Literal, LogicalOperator, SelectItem, SelectStatement,
    Statement, TableFactor, UnaryOperator, UpdateStatement,
};

/// SQLite Emitter
///
/// Common SQL AST を SQLite SQL に変換します。
#[derive(Debug)]
pub struct SqliteEmitter {
    /// 出力バッファ
    buffer: String,
    /// インデントレベル
    indent_level: usize,
    /// コンフィグ
    config: EmitterConfig,
}

impl SqliteEmitter {
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

    /// Common SQL AST を SQLite SQL に変換（単一ステートメント）
    ///
    /// # Arguments
    ///
    /// * `stmt` - Common SQL ステートメント
    ///
    /// # Returns
    ///
    /// SQLite SQL 文字列、またはエラー
    ///
    /// # Errors
    ///
    /// サポート対象外のステートメント (DDL 系) の場合 [`EmitError::Unsupported`] を返す。
    pub fn emit(&mut self, stmt: &Statement) -> Result<String, EmitError> {
        self.reset();
        self.visit_statement(stmt)?;
        Ok(std::mem::take(&mut self.buffer))
    }

    /// Common SQL AST を SQLite SQL に変換（複数ステートメント）
    ///
    /// # Arguments
    ///
    /// * `stmts` - Common SQL ステートメントのスライス
    ///
    /// # Returns
    ///
    /// SQLite SQL 文字列（セミコロン区切り）、またはエラー
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
            // 旧 CommonStatement::DialectSpecific に相当する逃げ道は
            // common_sql::ast::Statement に存在しなかったが、#158 で
            // DialectSpecific バリアントが再追加された。SQLite は未対応のため
            // Unsupported を返す (native 再実装は #158 で追跡)。
            Statement::CreateTable(_)
            | Statement::AlterTable(_)
            | Statement::DropTable(_)
            | Statement::CreateIndex(_)
            | Statement::DropIndex(_)
            | Statement::DialectSpecific { .. } => {
                Err(EmitError::Unsupported(statement_kind_name(stmt)))
            }
        }
    }

    /// SELECT文を訪問
    fn visit_select_statement(&mut self, stmt: &SelectStatement) -> Result<(), EmitError> {
        self.write("SELECT ");

        // DISTINCT
        // 旧 CommonSelectStatement.distinct: bool は SelectStatement にはフィールドがなく、
        // bridge 経由で projection に畳み込まれる想定だが、common-sql 上位の
        // SelectStatement には distinct フラグが存在しないため出力しない。
        // (旧実装の distinct 出力は bridge 側で SelectItem::Wildcard 等へ反映済み)

        // SELECTリスト (projection): 旧 columns (Vec) → projection (Vec<SelectItem>)。
        // projection が空の場合は '*' を出力する (旧 columns が空のときの挙動と互換)。
        if stmt.projection.is_empty() {
            self.write("*");
        } else {
            for (i, item) in stmt.projection.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.visit_select_item(item)?;
            }
        }

        // FROM: 旧 Vec<CommonTableReference> → Option<TableFactor>
        if let Some(from) = &stmt.from {
            self.write(" FROM ");
            self.visit_table_factor(from)?;
        }

        // WHERE
        if let Some(where_clause) = &stmt.where_clause {
            self.write(" WHERE ");
            self.visit_expression(where_clause)?;
        }

        // GROUP BY: Vec → Option<GroupByClause>
        if let Some(group_by) = &stmt.group_by {
            self.write(" GROUP BY ");
            for (i, item) in group_by.items.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.visit_group_by_item(item)?;
            }
        }

        // HAVING
        if let Some(having) = &stmt.having {
            self.write(" HAVING ");
            self.visit_expression(having)?;
        }

        // ORDER BY: Vec<CommonOrderByItem> → Option<OrderByClause>
        if let Some(order_by) = &stmt.order_by {
            self.write(" ORDER BY ");
            self.visit_order_by_clause(order_by)?;
        }

        // LIMIT (SQLite は LIMIT/OFFSET をサポート)
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
    fn visit_select_item(&mut self, item: &SelectItem) -> Result<(), EmitError> {
        match item {
            SelectItem::Expression { expr, alias } => {
                self.visit_expression(expr)?;
                if let Some(alias_name) = alias {
                    self.write(" AS ");
                    self.write_identifier(alias_name.value());
                }
            }
            SelectItem::Wildcard => {
                self.write("*");
            }
            // 旧 CommonSelectItem::QualifiedWildcard(String) →
            // SelectItem::QualifiedWildcard { table: Identifier }
            SelectItem::QualifiedWildcard { table } => {
                self.write_identifier(table.value());
                self.write(".*");
            }
        }
        Ok(())
    }

    /// GROUP BYアイテムを訪問
    fn visit_group_by_item(&mut self, item: &GroupByItem) -> Result<(), EmitError> {
        match item {
            GroupByItem::Expression(expr) => self.visit_expression(expr),
            // Rollup/Cube/GroupingSets は SQLite が直接サポートしない。
            // mysql-emitter/postgresql-emitter の DD-3 パターンと同等に式のみ描画せず、
            // プレースホルダーコメントを出力する。
            other => {
                self.write(&format!("/* unsupported GROUP BY item: {other:?} */"));
                Ok(())
            }
        }
    }

    /// ORDER BY句を訪問
    fn visit_order_by_clause(&mut self, order_by: &OrderByClause) -> Result<(), EmitError> {
        for (i, item) in order_by.items.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.visit_expression(&item.expr)?;
            // direction が None のときは ASC/DESCを出さない (DB default)。
            match item.direction {
                Some(SortDirection::Asc) => self.write(" ASC"),
                Some(SortDirection::Desc) => self.write(" DESC"),
                None => {}
            }
        }
        Ok(())
    }

    /// テーブル要素 (TableFactor) を訪問
    /// 旧 CommonTableReference{Table,Derived} → TableFactor{Table,Derived,Join}
    fn visit_table_factor(&mut self, factor: &TableFactor) -> Result<(), EmitError> {
        match factor {
            TableFactor::Table { name, alias } => {
                self.write_qualified_name(name);
                if let Some(alias_name) = alias {
                    self.write(" AS ");
                    self.write_identifier(alias_name.name());
                }
            }
            TableFactor::Derived { subquery, alias } => {
                self.write("(");
                self.visit_select_statement(subquery)?;
                self.write(")");
                if let Some(alias_name) = alias {
                    self.write(" AS ");
                    self.write_identifier(alias_name.name());
                }
            }
            TableFactor::Join(join) => self.visit_join(join)?,
        }
        Ok(())
    }

    /// JOIN を訪問 (左項は呼び出し文脈、ここでは右項と結合条件)。
    fn visit_join(&mut self, join: &Join) -> Result<(), EmitError> {
        let kw = match join.join_type {
            JoinType::Inner => "INNER JOIN ",
            JoinType::Left => "LEFT JOIN ",
            JoinType::Right => "RIGHT JOIN ",
            JoinType::Full => "FULL JOIN ",
            JoinType::Cross => "CROSS JOIN ",
        };
        self.write(kw);
        self.visit_table_factor(&join.table)
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
        // 旧 CommonInsertSource::DefaultValues は common_sql::InsertSource に存在しない。
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
                self.visit_select_statement(select)?;
            }
        }

        Ok(())
    }

    /// UPDATE文を訪問
    fn visit_update_statement(&mut self, stmt: &UpdateStatement) -> Result<(), EmitError> {
        self.write("UPDATE ");
        self.visit_table_factor(&stmt.table)?;
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
        self.visit_table_factor(&stmt.table)?;

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
        let indent = self.current_indent();
        self.buffer.push_str(&indent);
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

    /// 式を訪問してバッファに SQLite SQL 文字列を生成
    ///
    /// common_sql::ast::Expression は旧 CommonExpression から形状変更されている:
    /// - BinaryOp/Comparison/LogicalOp が 3 分岐に分割 (注意点 a)
    /// - ColumnReference → QualifiedIdentifier (注意点 b)
    /// - Case が {operand, conditions, else_result} (注意点 c)
    /// - In の list が InList enum (注意点 d)
    /// - Function が {name: Identifier, args, distinct} (注意点 e)
    /// - CAST が追加 (common-sql に存在、旧 CommonExpression にはなかった)
    /// - Like が比較演算子 ComparisonOperator::{Like,NotLike} に統合 (注意点 f)
    fn visit_expression(&mut self, expr: &Expression) -> Result<(), EmitError> {
        match expr {
            Expression::Literal(lit) => self.visit_literal(lit),
            Expression::Identifier(ident) => {
                self.write_identifier(ident.value());
                Ok(())
            }
            // schema.table.column / table.column 形式の修飾識別子。
            // 旧 CommonColumnReference に相当 (注意点 b)。
            Expression::QualifiedIdentifier { table, column } => {
                self.write_identifier(table.value());
                self.write(".");
                self.write_identifier(column.value());
                Ok(())
            }
            // 注意点 a: BinaryOp 分岐 (算術/文字列)
            Expression::BinaryOp { left, op, right } => self.visit_binary_op(left, *op, right),
            // 注意点 a: Comparison 分岐
            // 注意点 f: Like/NotLike もここに統合されている。
            Expression::Comparison { left, op, right } => self.visit_comparison(left, *op, right),
            // 注意点 a: LogicalOp 分岐 (AND/OR)
            Expression::LogicalOp { left, op, right } => self.visit_logical_op(left, *op, right),
            Expression::UnaryOp { op, expr } => self.visit_unary_op(*op, expr),
            // 注意点 e: Function{name, args, distinct}
            Expression::Function {
                name,
                args,
                distinct,
            } => self.visit_function(name, args, *distinct),
            // 注意点 c: Case{operand, conditions, else_result}
            Expression::Case {
                operand,
                conditions,
                else_result,
            } => self.visit_case(operand, conditions, else_result.as_deref()),
            // 注意点 d: In{expr, list:InList, negated}
            Expression::In {
                expr,
                list,
                negated,
            } => self.visit_in(expr, list, *negated),
            Expression::Between {
                expr,
                low,
                high,
                negated,
            } => self.visit_between(expr, low, high, *negated),
            // common-sql に存在するが旧 CommonExpression にはなかったバリアント。
            Expression::Cast { expr, data_type } => self.visit_cast(expr, data_type),
            Expression::IsNull { expr, negated } => self.visit_is_null(expr, *negated),
            Expression::Subquery(query) => {
                self.write("(");
                self.visit_select_statement(query)?;
                self.write(")");
                Ok(())
            }
            Expression::Exists { subquery, negated } => {
                if *negated {
                    self.write("NOT ");
                }
                self.write("EXISTS (");
                self.visit_select_statement(subquery)?;
                self.write(")");
                Ok(())
            }
        }
    }

    /// リテラルを訪問
    fn visit_literal(&mut self, lit: &Literal) -> Result<(), EmitError> {
        match lit {
            Literal::String(s) => {
                // 文字列をシングルクォートで囲む
                self.write(&format!("'{}'", s.replace('\'', "''")));
            }
            Literal::Integer(n) => {
                self.write(&n.to_string());
            }
            // common-sql の Float は精度保持のため String (旧 CommonLiteral::Float(f64) とは異なる)
            Literal::Float(s) => {
                self.write(s);
            }
            Literal::Null => {
                self.write("NULL");
            }
            Literal::Boolean(b) => {
                // SQLite は Boolean を INTEGER (0/1) として扱う
                self.write(if *b { "1" } else { "0" });
            }
        }
        Ok(())
    }

    /// 単項演算子を訪問
    fn visit_unary_op(&mut self, op: UnaryOperator, expr: &Expression) -> Result<(), EmitError> {
        let op_str = match op {
            UnaryOperator::Plus => "+",
            UnaryOperator::Minus => "-",
            UnaryOperator::Not => "NOT ",
        };
        self.write(op_str);
        self.visit_expression(expr)
    }

    /// 二項算術/文字列演算子を訪問 (注意点 a: BinaryOp 分岐)
    /// SQLite は旧実装と同様に演算子を括弧で囲まない。
    fn visit_binary_op(
        &mut self,
        left: &Expression,
        op: BinaryOperator,
        right: &Expression,
    ) -> Result<(), EmitError> {
        self.visit_expression(left)?;
        self.write(" ");
        self.write(match op {
            BinaryOperator::Add => "+",
            BinaryOperator::Sub => "-",
            BinaryOperator::Mul => "*",
            BinaryOperator::Div => "/",
            BinaryOperator::Mod => "%",
            BinaryOperator::Concat => "||",
        });
        self.write(" ");
        self.visit_expression(right)
    }

    /// 比較演算子を訪問 (注意点 a: Comparison 分岐)
    /// 注意点 f: common-sql では LIKE も ComparisonOperator に統合されている。
    fn visit_comparison(
        &mut self,
        left: &Expression,
        op: ComparisonOperator,
        right: &Expression,
    ) -> Result<(), EmitError> {
        self.visit_expression(left)?;
        self.write(match op {
            ComparisonOperator::Eq => " = ",
            ComparisonOperator::Ne => " != ",
            ComparisonOperator::Lt => " < ",
            ComparisonOperator::Le => " <= ",
            ComparisonOperator::Gt => " > ",
            ComparisonOperator::Ge => " >= ",
            ComparisonOperator::Like => " LIKE ",
            ComparisonOperator::NotLike => " NOT LIKE ",
            // ILIKE は SQLite がネイティブでサポートしない (case-insensitive LIKE が必要)。
            // LIKE にフォールバックするのが妥当だが、ここでは原文を保持して拡張点とする。
            ComparisonOperator::ILike => " LIKE ",
            ComparisonOperator::NotILike => " NOT LIKE ",
        });
        self.visit_expression(right)
    }

    /// 論理演算子を訪問 (注意点 a: LogicalOp 分岐)
    fn visit_logical_op(
        &mut self,
        left: &Expression,
        op: LogicalOperator,
        right: &Expression,
    ) -> Result<(), EmitError> {
        self.visit_expression(left)?;
        self.write(match op {
            LogicalOperator::And => " AND ",
            LogicalOperator::Or => " OR ",
        });
        self.visit_expression(right)
    }

    /// 関数呼び出しを訪問 (注意点 e: name が Identifier)
    fn visit_function(
        &mut self,
        name: &Identifier,
        args: &[Expression],
        distinct: bool,
    ) -> Result<(), EmitError> {
        let upper_name = name.value().to_uppercase();

        // DATEADD関数の特殊処理
        if upper_name == "DATEADD" {
            return self.emit_dateadd(name, args);
        }

        // DATEDIFF関数の特殊処理
        if upper_name == "DATEDIFF" {
            return self.emit_datediff(name, args);
        }

        // 関数名を変換（T-SQL → SQLite）
        let sqlite_name = function_mapper::map_function_name(&upper_name)
            .map(str::to_string)
            .unwrap_or_else(|| name.value().to_string());
        self.write(&sqlite_name);
        self.write("(");

        if distinct {
            self.write("DISTINCT ");
        }

        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.visit_expression(arg)?;
        }

        self.write(")");
        Ok(())
    }

    /// DATEADD関数をSQLite形式に変換
    ///
    /// T-SQL: DATEADD(datepart, number, date)
    /// SQLite: date(date_expression, '+N days') または datetime(date_expression, '+N hours')
    fn emit_dateadd(&mut self, name: &Identifier, args: &[Expression]) -> Result<(), EmitError> {
        if args.len() != 3 {
            return Err(EmitError::UnsupportedFunction(format!(
                "DATEADD: expected 3 arguments, got {}",
                args.len()
            )));
        }

        // 第1引数: datepart (文字列リテラルまたは識別子)
        let datepart = match &args[0] {
            Expression::Literal(Literal::String(s)) => s.to_lowercase(),
            Expression::Identifier(ident) => ident.value().to_lowercase(),
            _ => {
                return Err(EmitError::UnsupportedFunction(
                    "DATEADD: first argument must be a string literal or identifier".to_string(),
                ));
            }
        };

        // 第2引数: number (整数リテラル)
        let number = match &args[1] {
            Expression::Literal(Literal::Integer(n)) => *n,
            _ => {
                return Err(EmitError::UnsupportedFunction(
                    "DATEADD: second argument must be an integer literal".to_string(),
                ));
            }
        };

        // datepartをSQLite修飾子に変換
        let (modifier_unit, multiplier) = function_mapper::map_datepart_to_modifier(&datepart)
            .ok_or_else(|| {
                if datepart == "millisecond" || datepart == "ms" {
                    EmitError::UnsupportedFunction(
                        "DATEADD: SQLite does not support millisecond precision".to_string(),
                    )
                } else {
                    EmitError::UnsupportedFunction(format!(
                        "DATEADD: unsupported datepart: {datepart}"
                    ))
                }
            })?;

        // multiplier で数値を調整 (例: quarter → *3, week → *7)
        let adjusted_number = number * multiplier;

        // SQLiteの修飾子を生成
        let modifier = if adjusted_number >= 0 {
            format!("+{adjusted_number} {modifier_unit}")
        } else {
            format!("{adjusted_number} {modifier_unit}")
        };

        // 第3引数: date (式)
        // 時刻を含む関数の場合はdatetime、それ以外はdateを使用
        let use_datetime = function_mapper::is_time_datepart(&datepart);

        // 第3引数がGETDATE/GETUTCDATEの場合は特別処理
        // GETDATE/GETUTCDATEは常にdatetime('now')を使い、修飾子を直接適用
        let is_getdate = matches!(&args[2], Expression::Function { name: fname, .. }
        if {
            let u = fname.value().to_uppercase();
            u == "GETDATE" || u == "GETUTCDATE"
        });

        if is_getdate {
            // GETDATE/GETUTCDATE: datetime('now', modifier) または date('now', modifier)
            if use_datetime {
                self.write(&format!("datetime('now', '{modifier}')"));
            } else {
                self.write(&format!("date('now', '{modifier}')"));
            }
        } else {
            // その他の式: date(base_expr, modifier) または datetime(base_expr, modifier)
            let base_expr = self.extract_date_expression(&args[2])?;

            // SQLite形式の式を生成
            if use_datetime {
                self.write(&format!("datetime({base_expr}, '{modifier}')"));
            } else {
                self.write(&format!("date({base_expr}, '{modifier}')"));
            }
        }

        // 引数名の未使用警告を抑制 (関数名は検証済みだが、シグネチャ上一致させる)
        let _ = name;
        Ok(())
    }

    /// DATEDIFF関数をSQLite形式に変換
    ///
    /// T-SQL: DATEDIFF(datepart, startdate, enddate)
    /// SQLite: julianday(enddate) - julianday(startdate)  (日数差分)
    fn emit_datediff(&mut self, name: &Identifier, args: &[Expression]) -> Result<(), EmitError> {
        if args.len() != 3 {
            return Err(EmitError::UnsupportedFunction(format!(
                "DATEDIFF: expected 3 arguments, got {}",
                args.len()
            )));
        }

        // 第1引数: datepart (文字列リテラルまたは識別子)
        let datepart = match &args[0] {
            Expression::Literal(Literal::String(s)) => s.to_lowercase(),
            Expression::Identifier(ident) => ident.value().to_lowercase(),
            _ => {
                return Err(EmitError::UnsupportedFunction(
                    "DATEDIFF: first argument must be a string literal or identifier".to_string(),
                ));
            }
        };

        // SQLiteのjuliandayは日数を返すので、日付関連のみサポート
        if !function_mapper::is_date_datepart(&datepart) {
            return Err(EmitError::UnsupportedFunction(format!(
                "DATEDIFF: unsupported datepart: {datepart} (only date-based dateparts are supported)"
            )));
        }

        // 第2引数: startdate
        let start_date = self.extract_date_expression(&args[1])?;

        // 第3引数: enddate
        let end_date = self.extract_date_expression(&args[2])?;

        // SQLite形式の式を生成
        self.write(&format!(
            "(julianday({end_date}) - julianday({start_date}))"
        ));

        let _ = name;
        Ok(())
    }

    /// 日付式を文字列表現として抽出
    fn extract_date_expression(&self, expr: &Expression) -> Result<String, EmitError> {
        match expr {
            Expression::Literal(Literal::String(s)) => Ok(format!("'{s}'")),
            Expression::Literal(Literal::Integer(n)) => Ok(n.to_string()),
            Expression::Identifier(ident) => Ok(ident.value().to_string()),
            Expression::QualifiedIdentifier { table, column } => {
                Ok(format!("{}.{}", table.value(), column.value()))
            }
            Expression::Function { name: fname, .. }
                if {
                    let u = fname.value().to_uppercase();
                    u == "GETDATE" || u == "GETUTCDATE"
                } =>
            {
                Ok("datetime('now')".to_string())
            }
            _ => Err(EmitError::UnsupportedFunction(
                "DATEADD/DATEDIFF: complex date expressions are not yet supported".to_string(),
            )),
        }
    }

    /// CASE式を訪問 (注意点 c: operand, conditions, else_result)
    /// common-sql の Case は simple CASE の被験式 (operand) を保持する。
    fn visit_case(
        &mut self,
        operand: &Option<Box<Expression>>,
        conditions: &[(Expression, Expression)],
        else_result: Option<&Expression>,
    ) -> Result<(), EmitError> {
        self.write("CASE");
        // simple CASE (operand あり) の場合は "CASE <operand>"
        if let Some(operand_expr) = operand {
            self.write(" ");
            self.visit_expression(operand_expr)?;
        }

        for (when_expr, then_expr) in conditions {
            self.write(" WHEN ");
            self.visit_expression(when_expr)?;
            self.write(" THEN ");
            self.visit_expression(then_expr)?;
        }

        if let Some(else_expr) = else_result {
            self.write(" ELSE ");
            self.visit_expression(else_expr)?;
        }

        self.write(" END");
        Ok(())
    }

    /// IN式を訪問 (注意点 d: list が InList enum)
    fn visit_in(
        &mut self,
        expr: &Expression,
        list: &InList,
        negated: bool,
    ) -> Result<(), EmitError> {
        self.visit_expression(expr)?;
        self.write(if negated { " NOT IN (" } else { " IN (" });

        match list {
            InList::Values(values) => {
                for (i, item) in values.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.visit_expression(item)?;
                }
            }
            InList::Subquery(subquery) => {
                self.visit_select_statement(subquery)?;
            }
        }

        self.write(")");
        Ok(())
    }

    /// BETWEEN式を訪問
    fn visit_between(
        &mut self,
        expr: &Expression,
        low: &Expression,
        high: &Expression,
        negated: bool,
    ) -> Result<(), EmitError> {
        self.visit_expression(expr)?;
        self.write(if negated {
            " NOT BETWEEN "
        } else {
            " BETWEEN "
        });
        self.visit_expression(low)?;
        self.write(" AND ");
        self.visit_expression(high)
    }

    /// CAST式を訪問 (common-sql に存在するが旧 CommonExpression にはなかったバリアント)
    fn visit_cast(&mut self, expr: &Expression, data_type: &DataType) -> Result<(), EmitError> {
        self.write("CAST(");
        self.visit_expression(expr)?;
        self.write(" AS ");
        self.write(&Self::emit_data_type(data_type));
        self.write(")");
        Ok(())
    }

    /// SQLite のデータ型名を発行 (CAST 式で使用)。
    fn emit_data_type(data_type: &DataType) -> String {
        match data_type {
            // SQLite は親和性ベースの型システム: INTEGER/REAL/TEXT/BLOB/NUMERIC
            DataType::TinyInt | DataType::SmallInt | DataType::Int | DataType::BigInt => {
                "INTEGER".to_string()
            }
            DataType::Boolean => "INTEGER".to_string(),
            DataType::VarChar { length } => match length {
                Some(len) => format!("VARCHAR({len})"),
                None => "VARCHAR".to_string(),
            },
            DataType::Char { length } => match length {
                Some(len) => format!("CHAR({len})"),
                None => "CHAR".to_string(),
            },
            DataType::Text => "TEXT".to_string(),
            DataType::Real | DataType::DoublePrecision => "REAL".to_string(),
            DataType::Decimal { precision, scale } => match (precision, scale) {
                (Some(p), Some(s)) => format!("DECIMAL({p},{s})"),
                (Some(p), None) => format!("DECIMAL({p})"),
                (None, _) => "NUMERIC".to_string(),
            },
            DataType::Numeric { precision, scale } => match (precision, scale) {
                (Some(p), Some(s)) => format!("NUMERIC({p},{s})"),
                (Some(p), None) => format!("NUMERIC({p})"),
                (None, _) => "NUMERIC".to_string(),
            },
            DataType::Date => "TEXT".to_string(),
            DataType::Timestamp { .. } => "TEXT".to_string(),
            // その他の型は TEXT 親和性にフォールバック
            _ => "TEXT".to_string(),
        }
    }

    /// IS NULL式を訪問
    fn visit_is_null(&mut self, expr: &Expression, negated: bool) -> Result<(), EmitError> {
        self.visit_expression(expr)?;
        self.write(if negated { " IS NOT NULL" } else { " IS NULL" });
        Ok(())
    }

    /// 修飾テーブル名 (schema.table or table) を書き込む
    fn write_qualified_name(&mut self, name: &QualifiedName) {
        match name.schema() {
            Some(schema) => {
                self.write_identifier(schema);
                self.write(".");
                self.write_identifier(name.name());
            }
            None => self.write_identifier(name.name()),
        }
    }

    /// 識別子を書き込む（適切にクォート）
    fn write_identifier(&mut self, name: &str) {
        if self.config.quote_identifiers {
            // SQLite はダブルクォートまたはバッククォートで識別子をエスケープ
            // ここではダブルクォートを使用（標準的）
            let quoted = format!("\"{}\"", name.replace('"', "\"\""));
            self.write(&quoted);
        } else {
            self.write(name);
        }
    }
}

impl Default for SqliteEmitter {
    fn default() -> Self {
        Self::new(EmitterConfig::default())
    }
}

/// DDL 系・方言固有の文種別名を返す (エラーメッセージ用)。
fn statement_kind_name(stmt: &Statement) -> String {
    match stmt {
        Statement::CreateTable(_) => "CREATE TABLE".to_string(),
        Statement::AlterTable(_) => "ALTER TABLE".to_string(),
        Statement::DropTable(_) => "DROP TABLE".to_string(),
        Statement::CreateIndex(_) => "CREATE INDEX".to_string(),
        Statement::DropIndex(_) => "DROP INDEX".to_string(),
        // #158: DialectSpecific (T-SQL 制御構文等の方言固有文)。
        // SQLite は native 変換しないため Unsupported。復元は #158 で追跡。
        Statement::DialectSpecific { .. } => "DialectSpecific".to_string(),
        // DML/SELECT は呼び出し側で処理済みのため、ここでは到達しない。
        _ => "UNKNOWN".to_string(),
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
    use common_sql::ast::{Assignment, Literal, SelectItem, Span};

    // ---- 構築ヘルパー ----

    fn ident_expr(name: &str) -> Expression {
        Expression::Identifier(Identifier::new(name.to_string()))
    }

    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn str_expr(s: &str) -> Expression {
        Expression::Literal(Literal::String(s.to_string()))
    }

    fn id(name: &str) -> Identifier {
        Identifier::new(name.to_string())
    }

    fn table_factor(name: &str) -> TableFactor {
        TableFactor::Table {
            name: QualifiedName::new(None, name.to_string()),
            alias: None,
        }
    }

    fn qualified_table(name: &str) -> QualifiedName {
        QualifiedName::new(None, name.to_string())
    }

    fn select_star() -> Statement {
        Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Wildcard,
        ])))
    }

    // ============================================================
    // Emitter 構築
    // ============================================================

    #[test]
    fn test_new_emitter() {
        let config = EmitterConfig::default();
        let emitter = SqliteEmitter::new(config);
        assert_eq!(emitter.indent_level, 0);
        assert!(emitter.buffer.is_empty());
    }

    #[test]
    fn test_default_emitter() {
        let emitter = SqliteEmitter::default();
        assert!(emitter.config().quote_identifiers);
        assert_eq!(emitter.config().indent_size, 4);
    }

    #[test]
    fn test_reset() {
        let mut emitter = SqliteEmitter::default();
        emitter.write("SELECT 1");
        emitter.writeln();
        assert!(!emitter.buffer.is_empty());

        emitter.reset();
        assert!(emitter.buffer.is_empty());
        assert_eq!(emitter.indent_level, 0);
    }

    #[test]
    fn test_current_indent() {
        let mut emitter = SqliteEmitter::new(EmitterConfig {
            uppercase_keywords: false,
            quote_identifiers: true,
            indent_size: 2,
        });

        assert_eq!(emitter.current_indent(), "");

        emitter.indent_level = 1;
        assert_eq!(emitter.current_indent(), "  ");

        emitter.indent_level = 2;
        assert_eq!(emitter.current_indent(), "    ");
    }

    // ============================================================
    // リテラル (SQLite 固有: Boolean→0/1, Float は String)
    // ============================================================

    #[test]
    fn test_visit_literal_string() {
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: str_expr("hello"),
                alias: None,
            },
        ]))));
        assert!(sql.is_ok());
        assert_eq!(sql.unwrap(), "SELECT 'hello'");
    }

    #[test]
    fn test_visit_literal_string_with_quote() {
        let mut emitter = SqliteEmitter::default();
        // 'it''s' — embedded quote doubled
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: str_expr("it's"),
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT 'it''s'");
    }

    #[test]
    fn test_visit_literal_integer() {
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: int_expr(42),
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT 42");
    }

    #[test]
    fn test_visit_literal_float_preserves_string() {
        // common-sql の Float は精度保持のため String
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: Expression::Literal(Literal::Float("123.456".to_string())),
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT 123.456");
    }

    #[test]
    fn test_visit_literal_null() {
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: Expression::Literal(Literal::Null),
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT NULL");
    }

    #[test]
    fn test_visit_literal_boolean_true_is_one() {
        // SQLite は Boolean を INTEGER 1 として扱う
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: Expression::Literal(Literal::Boolean(true)),
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT 1");
    }

    #[test]
    fn test_visit_literal_boolean_false_is_zero() {
        // SQLite は Boolean を INTEGER 0 として扱う
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: Expression::Literal(Literal::Boolean(false)),
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT 0");
    }

    // ============================================================
    // 識別子 (SQLite: ダブルクォート, quote_identifiers config)
    // ============================================================

    #[test]
    fn test_visit_identifier_default_quoted() {
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: ident_expr("users"),
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT \"users\"");
    }

    #[test]
    fn test_visit_identifier_unquoted_when_configured() {
        let mut emitter = SqliteEmitter::new(EmitterConfig {
            uppercase_keywords: false,
            quote_identifiers: false,
            indent_size: 4,
        });
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: ident_expr("users"),
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT users");
    }

    #[test]
    fn test_qualified_identifier() {
        // 注意点 b: QualifiedIdentifier (table.column)
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: Expression::QualifiedIdentifier {
                    table: id("users"),
                    column: id("id"),
                },
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT \"users\".\"id\"");
    }

    // ============================================================
    // 単項演算子 (exhaustive: Plus / Minus / Not)
    // ============================================================

    #[test]
    fn test_visit_unary_op_minus() {
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: Expression::UnaryOp {
                    op: UnaryOperator::Minus,
                    expr: Box::new(int_expr(5)),
                },
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT -5");
    }

    #[test]
    fn test_visit_unary_op_not() {
        // NOT 1 (boolean true → 1)
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression {
                expr: Expression::UnaryOp {
                    op: UnaryOperator::Not,
                    expr: Box::new(Expression::Literal(Literal::Boolean(true))),
                },
                alias: None,
            },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT NOT 1");
    }

    // ============================================================
    // 二項算術/文字列演算子 (注意点 a: BinaryOp 分岐, exhaustive, 括弧なし)
    // ============================================================

    #[test]
    fn test_binary_op_add_no_parens() {
        // SQLite は旧実装通り括弧で囲まない: "10 + 5"
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::BinaryOp {
            left: Box::new(int_expr(10)),
            op: BinaryOperator::Add,
            right: Box::new(int_expr(5)),
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT 10 + 5");
    }

    #[test]
    fn test_binary_op_concat() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::BinaryOp {
            left: Box::new(ident_expr("a")),
            op: BinaryOperator::Concat,
            right: Box::new(ident_expr("b")),
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT \"a\" || \"b\"");
    }

    #[test]
    fn test_binary_op_all_variants() {
        let ops = [
            (BinaryOperator::Add, "+"),
            (BinaryOperator::Sub, "-"),
            (BinaryOperator::Mul, "*"),
            (BinaryOperator::Div, "/"),
            (BinaryOperator::Mod, "%"),
        ];
        for (op, sym) in ops {
            let mut emitter = SqliteEmitter::default();
            let expr = Expression::BinaryOp {
                left: Box::new(int_expr(1)),
                op,
                right: Box::new(int_expr(2)),
            };
            let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
                SelectItem::Expression { expr, alias: None },
            ]))));
            assert_eq!(sql.unwrap(), format!("SELECT 1 {sym} 2"));
        }
    }

    // ============================================================
    // 比較演算子 (注意点 a/f: ComparisonOperator, LIKE 統合)
    // ============================================================

    #[test]
    fn test_comparison_eq_in_where() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        sel.where_clause = Some(Expression::Comparison {
            left: Box::new(ident_expr("id")),
            op: ComparisonOperator::Eq,
            right: Box::new(int_expr(1)),
        });
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(sel))).unwrap();
        assert_eq!(sql, "SELECT * FROM \"users\" WHERE \"id\" = 1");
    }

    #[test]
    fn test_comparison_like_integrated() {
        // 注意点 f: LIKE は ComparisonOperator に統合
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        sel.where_clause = Some(Expression::Comparison {
            left: Box::new(ident_expr("name")),
            op: ComparisonOperator::Like,
            right: Box::new(str_expr("%John%")),
        });
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(sel))).unwrap();
        assert_eq!(sql, "SELECT * FROM \"users\" WHERE \"name\" LIKE '%John%'");
    }

    #[test]
    fn test_comparison_not_like_integrated() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        sel.where_clause = Some(Expression::Comparison {
            left: Box::new(ident_expr("name")),
            op: ComparisonOperator::NotLike,
            right: Box::new(str_expr("%admin%")),
        });
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(sel))).unwrap();
        assert_eq!(
            sql,
            "SELECT * FROM \"users\" WHERE \"name\" NOT LIKE '%admin%'"
        );
    }

    // ============================================================
    // 論理演算子 (注意点 a: LogicalOp 分岐)
    // ============================================================

    #[test]
    fn test_logical_and() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        sel.where_clause = Some(Expression::LogicalOp {
            left: Box::new(Expression::Comparison {
                left: Box::new(ident_expr("a")),
                op: ComparisonOperator::Eq,
                right: Box::new(int_expr(1)),
            }),
            op: LogicalOperator::And,
            right: Box::new(Expression::Comparison {
                left: Box::new(ident_expr("b")),
                op: ComparisonOperator::Eq,
                right: Box::new(int_expr(2)),
            }),
        });
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(sel))).unwrap();
        assert_eq!(sql, "SELECT * FROM \"users\" WHERE \"a\" = 1 AND \"b\" = 2");
    }

    // ============================================================
    // 関数 (注意点 e: name が Identifier)
    // ============================================================

    #[test]
    fn test_function_len_to_length() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Function {
            name: id("LEN"),
            args: vec![ident_expr("name")],
            distinct: false,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT length(\"name\")");
    }

    #[test]
    fn test_function_isnull_to_ifnull() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Function {
            name: id("ISNULL"),
            args: vec![ident_expr("a"), int_expr(0)],
            distinct: false,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT ifnull(\"a\", 0)");
    }

    #[test]
    fn test_function_distinct() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Function {
            name: id("COUNT"),
            args: vec![ident_expr("id")],
            distinct: true,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT count(DISTINCT \"id\")");
    }

    #[test]
    fn test_function_name_preserved_when_no_mapping() {
        // マッピングテーブルにない関数名はそのまま (小文字化しない)
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Function {
            name: id("MyCustomFunc"),
            args: vec![],
            distinct: false,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT MyCustomFunc()");
    }

    // ============================================================
    // DATEADD / DATEDIFF (SQLite 固有の特殊変換)
    // ============================================================

    fn func_call(name: &str, args: Vec<Expression>) -> Expression {
        Expression::Function {
            name: id(name),
            args,
            distinct: false,
        }
    }

    #[test]
    fn test_dateadd_day_with_getdate() {
        // DATEADD(day, 7, GETDATE()) → date('now', '+7 days')
        let mut emitter = SqliteEmitter::default();
        let expr = func_call(
            "DATEADD",
            vec![str_expr("day"), int_expr(7), func_call("GETDATE", vec![])],
        );
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT date('now', '+7 days')");
    }

    #[test]
    fn test_dateadd_month_with_string_date() {
        // DATEADD(month, 3, '2024-01-01') → date('2024-01-01', '+3 months')
        let mut emitter = SqliteEmitter::default();
        let expr = func_call(
            "DATEADD",
            vec![str_expr("month"), int_expr(3), str_expr("2024-01-01")],
        );
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT date('2024-01-01', '+3 months')");
    }

    #[test]
    fn test_dateadd_hour_negative_getdate() {
        // DATEADD(hour, -2, GETDATE()) → datetime('now', '-2 hours')
        let mut emitter = SqliteEmitter::default();
        let expr = func_call(
            "DATEADD",
            vec![str_expr("hour"), int_expr(-2), func_call("GETDATE", vec![])],
        );
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT datetime('now', '-2 hours')");
    }

    #[test]
    fn test_dateadd_quarter_multiplier() {
        // DATEADD(quarter, 1, GETDATE()) → date('now', '+3 months')  (quarter * 3)
        let mut emitter = SqliteEmitter::default();
        let expr = func_call(
            "DATEADD",
            vec![
                str_expr("quarter"),
                int_expr(1),
                func_call("GETDATE", vec![]),
            ],
        );
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT date('now', '+3 months')");
    }

    #[test]
    fn test_dateadd_week_multiplier() {
        // DATEADD(week, 2, GETDATE()) → date('now', '+14 days')  (week * 7)
        let mut emitter = SqliteEmitter::default();
        let expr = func_call(
            "DATEADD",
            vec![str_expr("week"), int_expr(2), func_call("GETDATE", vec![])],
        );
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT date('now', '+14 days')");
    }

    #[test]
    fn test_dateadd_with_identifier_datepart_and_date() {
        // DATEADD(day, 7, created_at) → date(created_at, '+7 days')
        let mut emitter = SqliteEmitter::default();
        let expr = func_call(
            "DATEADD",
            vec![ident_expr("day"), int_expr(7), ident_expr("created_at")],
        );
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT date(created_at, '+7 days')");
    }

    #[test]
    fn test_dateadd_error_invalid_args() {
        let mut emitter = SqliteEmitter::default();
        let expr = func_call("DATEADD", vec![str_expr("day"), int_expr(7)]);
        let result = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("expected 3 arguments"));
    }

    #[test]
    fn test_dateadd_error_unsupported_datepart_millisecond() {
        let mut emitter = SqliteEmitter::default();
        let expr = func_call(
            "DATEADD",
            vec![
                str_expr("millisecond"),
                int_expr(100),
                str_expr("2024-01-01"),
            ],
        );
        let result = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("millisecond"));
    }

    #[test]
    fn test_datediff_day() {
        // DATEDIFF(day, '2024-01-01', '2024-01-10')
        // → (julianday('2024-01-10') - julianday('2024-01-01'))
        let mut emitter = SqliteEmitter::default();
        let expr = func_call(
            "DATEDIFF",
            vec![
                str_expr("day"),
                str_expr("2024-01-01"),
                str_expr("2024-01-10"),
            ],
        );
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(
            sql.unwrap(),
            "SELECT (julianday('2024-01-10') - julianday('2024-01-01'))"
        );
    }

    // ============================================================
    // CASE式 (注意点 c: operand / conditions / else_result)
    // ============================================================

    #[test]
    fn test_case_searched() {
        // CASE WHEN x > 0 THEN 'pos' ELSE 'other' END
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Case {
            operand: None,
            conditions: vec![(
                Expression::Comparison {
                    left: Box::new(ident_expr("x")),
                    op: ComparisonOperator::Gt,
                    right: Box::new(int_expr(0)),
                },
                str_expr("pos"),
            )],
            else_result: Some(Box::new(str_expr("other"))),
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(
            sql.unwrap(),
            "SELECT CASE WHEN \"x\" > 0 THEN 'pos' ELSE 'other' END"
        );
    }

    #[test]
    fn test_case_simple_with_operand() {
        // simple CASE: CASE x WHEN 1 THEN 'one' END
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Case {
            operand: Some(Box::new(ident_expr("x"))),
            conditions: vec![(int_expr(1), str_expr("one"))],
            else_result: None,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT CASE \"x\" WHEN 1 THEN 'one' END");
    }

    // ============================================================
    // IN式 (注意点 d: InList enum)
    // ============================================================

    #[test]
    fn test_in_values() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::In {
            expr: Box::new(ident_expr("id")),
            list: InList::Values(vec![int_expr(1), int_expr(2), int_expr(3)]),
            negated: false,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT \"id\" IN (1, 2, 3)");
    }

    #[test]
    fn test_not_in_values() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::In {
            expr: Box::new(ident_expr("id")),
            list: InList::Values(vec![int_expr(1), int_expr(2)]),
            negated: true,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT \"id\" NOT IN (1, 2)");
    }

    #[test]
    fn test_in_subquery() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Expression {
            expr: ident_expr("id"),
            alias: None,
        }]);
        sel.from = Some(table_factor("src"));
        let expr = Expression::In {
            expr: Box::new(ident_expr("user_id")),
            list: InList::Subquery(Box::new(sel)),
            negated: false,
        };
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        let result = sql.unwrap();
        assert!(result.contains("\"user_id\" IN (SELECT \"id\" FROM \"src\")"));
    }

    // ============================================================
    // BETWEEN式
    // ============================================================

    #[test]
    fn test_between() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Between {
            expr: Box::new(ident_expr("age")),
            low: Box::new(int_expr(18)),
            high: Box::new(int_expr(65)),
            negated: false,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT \"age\" BETWEEN 18 AND 65");
    }

    #[test]
    fn test_not_between() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Between {
            expr: Box::new(ident_expr("age")),
            low: Box::new(int_expr(18)),
            high: Box::new(int_expr(65)),
            negated: true,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT \"age\" NOT BETWEEN 18 AND 65");
    }

    // ============================================================
    // CAST式 (common-sql に追加されたバリアント)
    // ============================================================

    #[test]
    fn test_cast_to_integer() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Cast {
            expr: Box::new(ident_expr("price")),
            data_type: DataType::Int,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT CAST(\"price\" AS INTEGER)");
    }

    #[test]
    fn test_cast_to_varchar_with_length() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::Cast {
            expr: Box::new(int_expr(123)),
            data_type: DataType::VarChar { length: Some(50) },
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT CAST(123 AS VARCHAR(50))");
    }

    // ============================================================
    // IS NULL式
    // ============================================================

    #[test]
    fn test_is_null() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::IsNull {
            expr: Box::new(ident_expr("email")),
            negated: false,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT \"email\" IS NULL");
    }

    #[test]
    fn test_is_not_null() {
        let mut emitter = SqliteEmitter::default();
        let expr = Expression::IsNull {
            expr: Box::new(ident_expr("email")),
            negated: true,
        };
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT \"email\" IS NOT NULL");
    }

    // ============================================================
    // Subquery / EXISTS
    // ============================================================

    #[test]
    fn test_scalar_subquery() {
        let inner = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let expr = Expression::Subquery(Box::new(inner));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT (SELECT *)");
    }

    #[test]
    fn test_exists() {
        let inner = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let expr = Expression::Exists {
            subquery: Box::new(inner),
            negated: false,
        };
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT EXISTS (SELECT *)");
    }

    #[test]
    fn test_not_exists() {
        let inner = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let expr = Expression::Exists {
            subquery: Box::new(inner),
            negated: true,
        };
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Expression { expr, alias: None },
        ]))));
        assert_eq!(sql.unwrap(), "SELECT NOT EXISTS (SELECT *)");
    }

    // ============================================================
    // SELECT 文全体 (FROM / WHERE / GROUP BY / ORDER BY / LIMIT)
    // ============================================================

    #[test]
    fn test_select_star_empty_projection() {
        // projection が空の場合は '*' を出力
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&select_star()).unwrap();
        assert_eq!(sql, "SELECT *");
    }

    #[test]
    fn test_select_with_from() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT * FROM \"users\"");
    }

    #[test]
    fn test_select_qualified_wildcard() {
        // SelectItem::QualifiedWildcard { table: Identifier }
        let mut sel =
            SelectStatement::simple(vec![SelectItem::QualifiedWildcard { table: id("users") }]);
        sel.from = Some(table_factor("users"));
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT \"users\".* FROM \"users\"");
    }

    #[test]
    fn test_select_with_alias() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Expression {
            expr: ident_expr("id"),
            alias: Some(id("user_id")),
        }]);
        sel.from = Some(table_factor("users"));
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT \"id\" AS \"user_id\" FROM \"users\"");
    }

    #[test]
    fn test_select_group_by() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        sel.group_by = Some(GroupByClause {
            span: Span::new(0, 10),
            items: vec![GroupByItem::Expression(ident_expr("dept"))],
        });
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT * FROM \"users\" GROUP BY \"dept\"");
    }

    #[test]
    fn test_select_order_by_asc() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        sel.order_by = Some(OrderByClause {
            span: Span::new(0, 10),
            items: vec![OrderByItem {
                expr: ident_expr("name"),
                direction: Some(SortDirection::Asc),
                nulls: None,
            }],
        });
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT * FROM \"users\" ORDER BY \"name\" ASC");
    }

    #[test]
    fn test_select_order_by_desc() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        sel.order_by = Some(OrderByClause {
            span: Span::new(0, 10),
            items: vec![OrderByItem {
                expr: ident_expr("name"),
                direction: Some(SortDirection::Desc),
                nulls: None,
            }],
        });
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT * FROM \"users\" ORDER BY \"name\" DESC");
    }

    #[test]
    fn test_select_limit() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        sel.limit = Some(LimitClause {
            span: Span::new(0, 10),
            limit: int_expr(10),
            offset: None,
        });
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT * FROM \"users\" LIMIT 10");
    }

    #[test]
    fn test_select_limit_offset() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(table_factor("users"));
        sel.limit = Some(LimitClause {
            span: Span::new(0, 10),
            limit: int_expr(10),
            offset: Some(int_expr(5)),
        });
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT * FROM \"users\" LIMIT 10 OFFSET 5");
    }

    // ============================================================
    // FROM句の TableFactor (Derived / Join)
    // ============================================================

    #[test]
    fn test_select_from_derived_subquery() {
        let inner = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(TableFactor::Derived {
            subquery: Box::new(inner),
            alias: Some(TableAlias::new("sub".to_string(), vec![])),
        });
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT * FROM (SELECT *) AS \"sub\"");
    }

    #[test]
    fn test_select_from_join() {
        // TableFactor::Join (第3バリアント)
        let join = Join {
            span: Span::new(0, 30),
            join_type: JoinType::Inner,
            table: table_factor("orders"),
            condition: JoinCondition::On(Expression::Comparison {
                left: Box::new(ident_expr("u.id")),
                op: ComparisonOperator::Eq,
                right: Box::new(ident_expr("o.user_id")),
            }),
            lateral: false,
        };
        let mut sel = SelectStatement::simple(vec![SelectItem::Wildcard]);
        sel.from = Some(TableFactor::Table {
            name: QualifiedName::new(None, "users".to_string()),
            alias: Some(TableAlias::new("u".to_string(), vec![])),
        });
        // join を from の後に付加するため、派生テーブルではなく直接 Join を使う
        sel.from = Some(TableFactor::Join(Box::new(join)));
        let stmt = Statement::Select(Box::new(sel));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "SELECT * FROM INNER JOIN \"orders\"");
    }

    // ============================================================
    // INSERT
    // ============================================================

    #[test]
    fn test_insert_values() {
        let stmt = Statement::Insert(Box::new(InsertStatement {
            span: Span::new(0, 30),
            table: qualified_table("users"),
            columns: vec![id("id"), id("name")],
            source: InsertSource::Values(vec![
                vec![int_expr(1), str_expr("a")],
                vec![int_expr(2), str_expr("b")],
            ]),
            on_conflict: None,
        }));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(
            sql,
            "INSERT INTO \"users\" (\"id\", \"name\") VALUES (1, 'a'), (2, 'b')"
        );
    }

    #[test]
    fn test_insert_no_columns() {
        let stmt = Statement::Insert(Box::new(InsertStatement {
            span: Span::new(0, 20),
            table: qualified_table("t"),
            columns: vec![],
            source: InsertSource::Values(vec![vec![int_expr(1)]]),
            on_conflict: None,
        }));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "INSERT INTO \"t\" VALUES (1)");
    }

    #[test]
    fn test_insert_empty_values_fallback() {
        // 旧 DefaultValues → bridge で Values(vec![]) にフォールバック
        let stmt = Statement::Insert(Box::new(InsertStatement {
            span: Span::new(0, 20),
            table: qualified_table("t"),
            columns: vec![],
            source: InsertSource::Values(vec![]),
            on_conflict: None,
        }));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "INSERT INTO \"t\" VALUES ");
    }

    #[test]
    fn test_insert_select() {
        let mut sel = SelectStatement::simple(vec![SelectItem::Expression {
            expr: ident_expr("id"),
            alias: None,
        }]);
        sel.from = Some(table_factor("source"));
        let stmt = Statement::Insert(Box::new(InsertStatement {
            span: Span::new(0, 50),
            table: qualified_table("archive"),
            columns: vec![id("id")],
            source: InsertSource::Select(Box::new(sel)),
            on_conflict: None,
        }));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert!(sql.contains("INSERT INTO \"archive\" (\"id\")"));
        assert!(sql.contains("SELECT \"id\" FROM \"source\""));
    }

    // ============================================================
    // UPDATE
    // ============================================================

    #[test]
    fn test_update() {
        let stmt = Statement::Update(Box::new(UpdateStatement {
            span: Span::new(0, 40),
            table: table_factor("users"),
            assignments: vec![Assignment {
                column: id("name"),
                value: str_expr("Bob"),
            }],
            from: None,
            where_clause: Some(Expression::Comparison {
                left: Box::new(ident_expr("id")),
                op: ComparisonOperator::Eq,
                right: Box::new(int_expr(1)),
            }),
        }));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(
            sql,
            "UPDATE \"users\" SET \"name\" = 'Bob' WHERE \"id\" = 1"
        );
    }

    #[test]
    fn test_update_multiple_assignments() {
        let stmt = Statement::Update(Box::new(UpdateStatement {
            span: Span::new(0, 40),
            table: table_factor("users"),
            assignments: vec![
                Assignment {
                    column: id("name"),
                    value: str_expr("x"),
                },
                Assignment {
                    column: id("count"),
                    value: int_expr(1),
                },
            ],
            from: None,
            where_clause: None,
        }));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "UPDATE \"users\" SET \"name\" = 'x', \"count\" = 1");
    }

    // ============================================================
    // DELETE
    // ============================================================

    #[test]
    fn test_delete() {
        let stmt = Statement::Delete(Box::new(DeleteStatement {
            span: Span::new(0, 30),
            table: table_factor("users"),
            using: None,
            where_clause: Some(ident_expr("id")),
        }));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "DELETE FROM \"users\" WHERE \"id\"");
    }

    #[test]
    fn test_delete_no_where() {
        let stmt = Statement::Delete(Box::new(DeleteStatement {
            span: Span::new(0, 30),
            table: table_factor("users"),
            using: None,
            where_clause: None,
        }));
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit(&stmt).unwrap();
        assert_eq!(sql, "DELETE FROM \"users\"");
    }

    // ============================================================
    // emit_batch
    // ============================================================

    #[test]
    fn test_emit_batch() {
        let stmts = vec![select_star(), select_star()];
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit_batch(&stmts).unwrap();
        assert_eq!(sql, "SELECT *;\nSELECT *");
    }

    #[test]
    fn test_emit_batch_single() {
        let stmts = vec![select_star()];
        let mut emitter = SqliteEmitter::default();
        let sql = emitter.emit_batch(&stmts).unwrap();
        assert_eq!(sql, "SELECT *");
    }

    // ============================================================
    // DDL は Unsupported (common_sql::ast::Statement に DialectSpecific なし)
    // ============================================================

    #[test]
    fn test_emit_ddl_returns_unsupported() {
        // DDL 系は本 emitter が未対応。
        // 最小の CreateTableStatement を構築して Unsupported を確認。
        use common_sql::ast::{CreateTableStatement, TableConstraint, TableOptions};
        let ddl = Statement::CreateTable(Box::new(CreateTableStatement {
            span: Span::new(0, 10),
            if_not_exists: false,
            temporary: false,
            name: qualified_table("t"),
            columns: vec![],
            constraints: Vec::<TableConstraint>::new(),
            options: TableOptions {
                engine: None,
                charset: None,
                collation: None,
                comment: None,
            },
        }));
        let mut emitter = SqliteEmitter::default();
        let result = emitter.emit(&ddl);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmitError::Unsupported(msg) => assert_eq!(msg, "CREATE TABLE"),
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    // T6 (#158): DialectSpecific バリアントのパリティ検証。
    // common_sql::ast::Statement に #158 で DialectSpecific が再追加されたため、
    // visit_statement は新 arm を要求する (exhaustiveness)。SQLite は native 変換を
    // 行わないため Unsupported を返すが、種別名として意味のある文字列を返すこと。
    // (旧実装の T-SQL→SQLite ヒント生成の復元は #158 で追跡。本 PR では arm 追加のみ。)
    #[test]
    fn test_emit_dialect_specific_returns_unsupported_with_descriptive_name() {
        let stmt = Statement::DialectSpecific {
            source: "DECLARE @v INT".to_string(),
            span: Span::new(0, 15),
        };
        let mut emitter = SqliteEmitter::default();
        let result = emitter.emit(&stmt);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmitError::Unsupported(msg) => {
                // "UNKNOWN" ではなく DialectSpecific 由来であることが分かる名称であること
                assert_ne!(
                    msg, "UNKNOWN",
                    "DialectSpecific must produce a descriptive kind name, not UNKNOWN"
                );
                assert!(
                    msg.contains("Dialect") || msg.contains("dialect"),
                    "expected dialect-specific kind name, got: {msg}"
                );
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    #[test]
    fn test_statement_kind_name_all_ddl() {
        // 全 DDL バリアントの種別名 (エラーメッセージ用)
        // 直接関数は private だが、エミット経由で各メッセージを検証
        use common_sql::ast::{
            CreateTableStatement, DropTableStatement, TableConstraint, TableOptions,
        };
        let mk = |kind: &str, stmt: Statement| {
            let mut emitter = SqliteEmitter::default();
            match emitter.emit(&stmt) {
                Err(EmitError::Unsupported(msg)) => assert_eq!(msg, kind),
                other => panic!("expected Unsupported({kind}), got {other:?}"),
            }
        };
        mk(
            "CREATE TABLE",
            Statement::CreateTable(Box::new(CreateTableStatement {
                span: Span::new(0, 10),
                if_not_exists: false,
                temporary: false,
                name: qualified_table("t"),
                columns: vec![],
                constraints: Vec::<TableConstraint>::new(),
                options: TableOptions {
                    engine: None,
                    charset: None,
                    collation: None,
                    comment: None,
                },
            })),
        );
        mk(
            "DROP TABLE",
            Statement::DropTable(Box::new(DropTableStatement {
                span: Span::new(0, 10),
                if_exists: false,
                names: vec![qualified_table("t")],
            })),
        );
    }

    // ============================================================
    // function_mapper (AST 非依存、互換性確認)
    // ============================================================

    #[test]
    fn test_function_name_mapping_table() {
        // function_mapper は AST に依存しないため変更なし。互換性を再確認。
        assert_eq!(function_mapper::map_function_name("LEN"), Some("length"));
        assert_eq!(
            function_mapper::map_function_name("GETDATE"),
            Some("datetime('now')")
        );
        assert_eq!(function_mapper::map_function_name("ISNULL"), Some("ifnull"));
        assert_eq!(function_mapper::map_function_name("COUNT"), Some("count"));
        assert_eq!(function_mapper::map_function_name("CEILING"), Some("ceil"));
        assert_eq!(function_mapper::map_function_name("SUM"), Some("sum"));
    }
}
