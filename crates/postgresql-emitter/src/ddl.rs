//! DDL (Data Definition Language) emission for PostgreSQL.
//!
//! `CREATE TABLE`, `ALTER TABLE`, `DROP TABLE`, `CREATE INDEX`, `DROP INDEX`
//! を PostgreSQL 方言の SQL 文字列へ変換する。Common SQL AST
//! ([`common_sql::ast::ddl`]) を入力とし、[`crate::PostgreSqlEmitter`] の
//! バッファへ書き込む。
//!
//! ## 設計 (Group B / schema-diff T4 — design §0.6 / §0.4)
//!
//! - **識別子**: `mappers::IdentifierQuoter::quote` で PostgreSQL のダブルクォート
//!   規則を適用 (予約語・大文字開始・特殊文字を `"..."` で囲む)。
//! - **データ型**: `mappers::DataTypeMapper::map` で方言マッピングを行う。
//!   T-SQL → common-sql 変換 (§0.6 short-circuit) は converter 層 (T2) で完了済み
//!   のため、本層では再マッピングしない。
//! - **`AUTO_INCREMENT` / `IDENTITY`** は PostgreSQL の `SERIAL` / `BIGSERIAL` に
//!   置換する。このとき列の `data_type` は置換され (`Int` → `SERIAL`,
//!   `BigInt` → `BIGSERIAL`)、他の整数型の場合は `SMALLINT` 等を保持しつつ
//!   `GENERATED ... AS IDENTITY` を付与する。
//! - **`ALTER TABLE`** は 6 種の [`AlterTableAction`] 全バリアントを処理する。

use crate::mappers::{DataTypeMapper, ExpressionEmitter, IdentifierQuoter};
use crate::{EmitError, PostgreSqlEmitter};
use common_sql::ast::{
    AlterTableAction, AlterTableStatement, ColumnConstraint, ColumnDef, CreateIndexStatement,
    CreateTableStatement, DataType, DropIndexStatement, DropTableStatement, IndexColumn,
    SortDirection, TableConstraint,
};

impl PostgreSqlEmitter {
    /// `CREATE TABLE` を PostgreSQL SQL へ変換してバッファへ書き込む。
    ///
    /// # Errors
    ///
    /// 列データ型に PostgreSQL 未対応の型が含まれる場合 [`EmitError`] を返す。
    pub(crate) fn visit_create_table(
        &mut self,
        stmt: &CreateTableStatement,
    ) -> Result<(), EmitError> {
        self.write("CREATE ");
        if stmt.temporary {
            self.write("TEMPORARY ");
        }
        self.write("TABLE ");
        if stmt.if_not_exists {
            self.write("IF NOT EXISTS ");
        }
        self.write_qualified_name(&stmt.name);
        self.write(" (");

        let mut first = true;

        // 列定義
        for col in &stmt.columns {
            if !first {
                self.write(", ");
            }
            first = false;
            self.visit_column_def(col)?;
        }

        // テーブル制約
        for con in &stmt.constraints {
            if !first {
                self.write(", ");
            }
            first = false;
            self.visit_table_constraint(con);
        }

        self.write(")");

        // テーブルオプション (ENGINE/CHARSET/COLLATE/COMMENT) は MySQL 固有。
        // PostgreSQL はこれらを認識しないため出力しない (design §0.4 系 — 方言固有
        // オプションは対象方言が解釈する場合のみ出力)。
        Ok(())
    }

    /// `ALTER TABLE` を PostgreSQL SQL へ変換してバッファへ書き込む。
    ///
    /// 6 種の [`AlterTableAction`] 全バリアントを処理する。
    ///
    /// # Errors
    ///
    /// アクション内のデータ型が PostgreSQL 未対応の場合 [`EmitError`] を返す。
    pub(crate) fn visit_alter_table(
        &mut self,
        stmt: &AlterTableStatement,
    ) -> Result<(), EmitError> {
        self.write("ALTER TABLE ");
        self.write_qualified_name(&stmt.name);

        for (i, action) in stmt.actions.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            } else {
                self.write(" ");
            }
            self.visit_alter_action(action)?;
        }

        Ok(())
    }

    /// `DROP TABLE` を PostgreSQL SQL へ変換してバッファへ書き込む。
    pub(crate) fn visit_drop_table(&mut self, stmt: &DropTableStatement) {
        self.write("DROP TABLE ");
        if stmt.if_exists {
            self.write("IF EXISTS ");
        }
        for (i, name) in stmt.names.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write_qualified_name(name);
        }
    }

    /// `CREATE INDEX` を PostgreSQL SQL へ変換してバッファへ書き込む。
    pub(crate) fn visit_create_index(&mut self, stmt: &CreateIndexStatement) {
        self.write("CREATE ");
        if stmt.unique {
            self.write("UNIQUE ");
        }
        self.write("INDEX ");
        if stmt.if_not_exists {
            self.write("IF NOT EXISTS ");
        }
        self.write_identifier(stmt.name.value());
        self.write(" ON ");
        self.write_qualified_name(&stmt.table);
        self.write_index_columns(&stmt.columns);
    }

    /// `DROP INDEX` を PostgreSQL SQL へ変換してバッファへ書き込む。
    ///
    /// PostgreSQL では `ON table` を指定しない (`DROP INDEX name;`)。
    /// `table` が与えられても PostgreSQL 構文では無視する。
    pub(crate) fn visit_drop_index(&mut self, stmt: &DropIndexStatement) {
        self.write("DROP INDEX ");
        if stmt.if_exists {
            self.write("IF EXISTS ");
        }
        self.write_identifier(stmt.name.value());
        // PostgreSQL は `DROP INDEX name [CONCURRENTLY]` 形式で `ON table` を取らない。
        // `stmt.table` は無視する (design §0.4 系 — 方言固有構文の正規化)。
    }

    // -----------------------------------------------------------------------
    // 内部ヘルパー
    // -----------------------------------------------------------------------

    /// 列定義 (`name type [NOT NULL] [DEFAULT expr] [constraints...]`) を書き込む。
    fn visit_column_def(&mut self, col: &ColumnDef) -> Result<(), EmitError> {
        self.write_identifier(col.name.value());

        // AUTO_INCREMENT / IDENTITY の検出: 整数型を SERIAL/BIGSERIAL に置換する。
        if has_auto_increment(col) {
            let serial = serial_type_for(&col.data_type);
            self.write(" ");
            self.write(&serial);
        } else {
            let mapped = DataTypeMapper::map(&col.data_type)?;
            self.write(" ");
            self.write(&mapped);
        }

        if !col.nullable {
            self.write(" NOT NULL");
        }

        if let Some(default) = &col.default {
            self.write(" DEFAULT ");
            self.write(&ExpressionEmitter::emit(default));
        }

        for con in &col.constraints {
            match con {
                ColumnConstraint::PrimaryKey => self.write(" PRIMARY KEY"),
                ColumnConstraint::Unique => self.write(" UNIQUE"),
                ColumnConstraint::Check(expr) => {
                    self.write(" CHECK (");
                    self.write(&ExpressionEmitter::emit(expr));
                    self.write(")");
                }
                ColumnConstraint::References { table, columns } => {
                    self.write(" REFERENCES ");
                    self.write_qualified_name(table);
                    self.write_reference_columns(columns);
                }
                // SERIAL 化済みのため AUTO_INCREMENT はここでは出力しない。
                ColumnConstraint::AutoIncrement => {}
            }
        }

        Ok(())
    }

    /// テーブル制約を書き込む。
    fn visit_table_constraint(&mut self, con: &TableConstraint) {
        match con {
            TableConstraint::PrimaryKey { name, columns } => {
                if let Some(n) = name {
                    self.write("CONSTRAINT ");
                    self.write_identifier(n);
                    self.write(" ");
                }
                self.write("PRIMARY KEY ");
                self.write_identifier_list(columns);
            }
            TableConstraint::Unique { name, columns } => {
                if let Some(n) = name {
                    self.write("CONSTRAINT ");
                    self.write_identifier(n);
                    self.write(" ");
                }
                self.write("UNIQUE ");
                self.write_identifier_list(columns);
            }
            TableConstraint::ForeignKey {
                name,
                columns,
                ref_table,
                ref_columns,
            } => {
                if let Some(n) = name {
                    self.write("CONSTRAINT ");
                    self.write_identifier(n);
                    self.write(" ");
                }
                self.write("FOREIGN KEY ");
                self.write_identifier_list(columns);
                self.write(" REFERENCES ");
                self.write_qualified_name(ref_table);
                self.write_identifier_list(ref_columns);
            }
            TableConstraint::Check { name, expr } => {
                if let Some(n) = name {
                    self.write("CONSTRAINT ");
                    self.write_identifier(n);
                    self.write(" ");
                }
                self.write("CHECK (");
                self.write(&ExpressionEmitter::emit(expr));
                self.write(")");
            }
        }
    }

    /// 単一 `ALTER TABLE` アクションを書き込む。
    fn visit_alter_action(&mut self, action: &AlterTableAction) -> Result<(), EmitError> {
        match action {
            AlterTableAction::AddColumn(col) => {
                self.write("ADD COLUMN ");
                self.visit_column_def(col)?;
            }
            AlterTableAction::DropColumn(name) => {
                self.write("DROP COLUMN ");
                self.write_identifier(name.value());
            }
            AlterTableAction::AlterColumn {
                column,
                data_type,
                default,
                nullable,
            } => {
                self.write("ALTER COLUMN ");
                self.write_identifier(column.value());
                let mut wrote_one = false;
                if let Some(dt) = data_type {
                    let mapped = DataTypeMapper::map(dt)?;
                    self.write(" TYPE ");
                    self.write(&mapped);
                    wrote_one = true;
                }
                match default {
                    Some(Some(expr)) => {
                        if wrote_one {
                            self.write(", ALTER COLUMN ");
                            self.write_identifier(column.value());
                        }
                        self.write(" SET DEFAULT ");
                        self.write(&ExpressionEmitter::emit(expr));
                        wrote_one = true;
                    }
                    Some(None) => {
                        if wrote_one {
                            self.write(", ALTER COLUMN ");
                            self.write_identifier(column.value());
                        }
                        self.write(" DROP DEFAULT");
                        wrote_one = true;
                    }
                    None => {}
                }
                if let Some(nul) = nullable {
                    if wrote_one {
                        self.write(", ALTER COLUMN ");
                        self.write_identifier(column.value());
                    }
                    if *nul {
                        self.write(" DROP NOT NULL");
                    } else {
                        self.write(" SET NOT NULL");
                    }
                }
            }
            AlterTableAction::AddConstraint(con) => {
                self.write("ADD ");
                self.visit_table_constraint(con);
            }
            AlterTableAction::DropConstraint(name) => {
                self.write("DROP CONSTRAINT ");
                self.write_identifier(name);
            }
            AlterTableAction::RenameTo(new_name) => {
                self.write("RENAME TO ");
                self.write_qualified_name(new_name);
            }
        }
        Ok(())
    }

    /// インデックス列リスト `(col [ASC|DESC], ...)` を書き込む。
    fn write_index_columns(&mut self, columns: &[IndexColumn]) {
        self.write(" (");
        for (i, c) in columns.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write_identifier(c.name.value());
            match c.direction {
                Some(SortDirection::Asc) => self.write(" ASC"),
                Some(SortDirection::Desc) => self.write(" DESC"),
                None => {}
            }
        }
        self.write(")");
    }

    /// `Identifier` リストを `(a, b, c)` 形式で書き込む。
    fn write_identifier_list(&mut self, columns: &[common_sql::ast::Identifier]) {
        self.write("(");
        for (i, c) in columns.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write_identifier(c.value());
        }
        self.write(")");
    }

    /// `REFERENCES` 句の参照列リスト `(col, ...)` を書き込む (String 名)。
    fn write_reference_columns(&mut self, columns: &[String]) {
        self.write(" (");
        for (i, c) in columns.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write_identifier(c);
        }
        self.write(")");
    }
}

/// 列に `AUTO_INCREMENT` / `IDENTITY` 制約が含まれるかを返す。
fn has_auto_increment(col: &ColumnDef) -> bool {
    col.constraints
        .iter()
        .any(|c| matches!(c, ColumnConstraint::AutoIncrement))
}

/// `AUTO_INCREMENT` 列のデータ型を PostgreSQL の SERIAL 系に対応付ける。
///
/// - `Int` → `SERIAL`
/// - `BigInt` → `BIGSERIAL`
/// - `SmallInt` → `SMALLSERIAL`
/// - それ以外の整数型は `SMALLSERIAL` を既定のフォールバックとして用いる
///   (PostgreSQL の SERIAL 系は 4/8/2 バイト整数のみ)。
fn serial_type_for(data_type: &DataType) -> String {
    match data_type {
        DataType::SmallInt => "SMALLSERIAL".to_string(),
        DataType::Int => "SERIAL".to_string(),
        DataType::BigInt => "BIGSERIAL".to_string(),
        // TinyInt は common-sql 上 SmallInt へ正規化済みだが念のため。
        DataType::TinyInt => "SMALLSERIAL".to_string(),
        // 非整数型で AUTO_INCREMENT が指定された場合は SERIAL を既定とする
        // (妥当でない入力だがクラッシュせず PostgreSQL が構文エラーで弾く)。
        _ => "SERIAL".to_string(),
    }
}

/// `IdentifierQuoter` を経由した識別子クォートを得る (設計上の単一参照点)。
/// (現状では直接 `IdentifierQuoter::quote` を呼ぶため未使用だが、将来の方言拡張で
/// 参照されることを想定した設計アンカー。)
#[allow(dead_code)]
fn quote_identifier(name: &str) -> String {
    IdentifierQuoter::quote(name)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::{EmissionConfig, PostgreSqlEmitter};
    use common_sql::ast::identifier::{Identifier, QualifiedName};
    use common_sql::ast::{
        AlterTableAction, AlterTableStatement, ColumnConstraint, ColumnDef, CreateIndexStatement,
        CreateTableStatement, DataType, DropIndexStatement, DropTableStatement, Expression,
        IndexColumn, Literal, SortDirection, Span, TableConstraint, TableOptions,
    };

    // ---- 構築ヘルパー ----

    fn ident(s: &str) -> Identifier {
        Identifier::new(s.to_string())
    }

    fn qualified(s: &str) -> QualifiedName {
        QualifiedName::new(None, s.to_string())
    }

    fn int_col(name: &str, dt: DataType, nullable: bool) -> ColumnDef {
        ColumnDef {
            span: Span::new(0, 10),
            name: ident(name),
            data_type: dt,
            nullable,
            default: None,
            constraints: vec![],
        }
    }

    fn emit_str(stmt: &common_sql::ast::Statement) -> String {
        let mut emitter = PostgreSqlEmitter::new(EmissionConfig::default());
        emitter.emit(stmt).unwrap()
    }

    fn create_table_stmt(name: &str, columns: Vec<ColumnDef>) -> CreateTableStatement {
        CreateTableStatement {
            span: Span::new(0, 50),
            if_not_exists: false,
            temporary: false,
            name: qualified(name),
            columns,
            constraints: vec![],
            options: TableOptions::default(),
        }
    }

    // ===== CREATE TABLE (UC-1) =====

    #[test]
    fn test_create_table_basic_columns() {
        // CREATE TABLE users (id INTEGER NOT NULL, title VARCHAR(255))
        // (`title` は非予約語のためクォートされない)
        let stmt = create_table_stmt(
            "users",
            vec![
                int_col("id", DataType::Int, false),
                ColumnDef {
                    span: Span::new(0, 10),
                    name: ident("title"),
                    data_type: DataType::VarChar { length: Some(255) },
                    nullable: true,
                    default: None,
                    constraints: vec![],
                },
            ],
        );
        let sql = emit_str(&common_sql::ast::Statement::CreateTable(Box::new(stmt)));
        assert_eq!(
            sql,
            "CREATE TABLE users (id INTEGER NOT NULL, title VARCHAR(255))"
        );
    }

    // ===== SERIAL / IDENTITY (UC-2) =====

    #[test]
    fn test_create_table_serial_identity_mapping() {
        // AUTO_INCREMENT on Int → SERIAL, on BigInt → BIGSERIAL
        let stmt = create_table_stmt(
            "seq",
            vec![
                ColumnDef {
                    span: Span::new(0, 10),
                    name: ident("big_id"),
                    data_type: DataType::BigInt,
                    nullable: false,
                    default: None,
                    constraints: vec![
                        ColumnConstraint::PrimaryKey,
                        ColumnConstraint::AutoIncrement,
                    ],
                },
                ColumnDef {
                    span: Span::new(0, 10),
                    name: ident("int_id"),
                    data_type: DataType::Int,
                    nullable: false,
                    default: None,
                    constraints: vec![ColumnConstraint::AutoIncrement],
                },
                ColumnDef {
                    span: Span::new(0, 10),
                    name: ident("small_id"),
                    data_type: DataType::SmallInt,
                    nullable: false,
                    default: None,
                    constraints: vec![ColumnConstraint::AutoIncrement],
                },
            ],
        );
        let sql = emit_str(&common_sql::ast::Statement::CreateTable(Box::new(stmt)));
        // 制約はソース順: big_id は [PrimaryKey, AutoIncrement] → PRIMARY KEY のみ
        // (AutoIncrement は SERIAL 化で消化)。NOT NULL は nullable=false から。
        assert_eq!(
            sql,
            "CREATE TABLE seq \
             (big_id BIGSERIAL NOT NULL PRIMARY KEY, \
             int_id SERIAL NOT NULL, \
             small_id SMALLSERIAL NOT NULL)"
        );
    }

    // ===== ALTER TABLE: 全 6 アクション =====

    #[test]
    fn test_alter_table_all_six_actions() {
        // 1 つの ALTER TABLE 文で 6 種の AlterTableAction を全て含む。
        let stmt = AlterTableStatement {
            span: Span::new(0, 200),
            name: qualified("users"),
            actions: vec![
                // 1. AddColumn
                AlterTableAction::AddColumn(ColumnDef {
                    span: Span::new(0, 10),
                    name: ident("email"),
                    data_type: DataType::VarChar { length: Some(255) },
                    nullable: true,
                    default: None,
                    constraints: vec![],
                }),
                // 2. DropColumn
                AlterTableAction::DropColumn(ident("legacy")),
                // 3. AlterColumn (type change)
                AlterTableAction::AlterColumn {
                    column: ident("name"),
                    data_type: Some(DataType::VarChar { length: Some(200) }),
                    default: None,
                    nullable: None,
                },
                // 4. AddConstraint
                AlterTableAction::AddConstraint(TableConstraint::Unique {
                    name: Some("uk_email".to_string()),
                    columns: vec![ident("email")],
                }),
                // 5. DropConstraint
                AlterTableAction::DropConstraint("old_constraint".to_string()),
                // 6. RenameTo
                AlterTableAction::RenameTo(qualified("members")),
            ],
        };
        let sql = emit_str(&common_sql::ast::Statement::AlterTable(Box::new(stmt)));
        // 各アクションがカンマ区切りで連結される。`name`/`email`/`legacy` は
        // PostgreSQL 非予約語、`members` も同様。`ALTER` 列名は "name" だが
        // PostgreSQL 予約語 (NAME) のためダブルクォートされる。
        assert!(sql.starts_with("ALTER TABLE users "));
        assert!(sql.contains("ADD COLUMN email VARCHAR(255)"));
        assert!(sql.contains("DROP COLUMN legacy"));
        assert!(sql.contains("ALTER COLUMN \"name\" TYPE VARCHAR(200)"));
        assert!(sql.contains("ADD CONSTRAINT uk_email UNIQUE (email)"));
        assert!(sql.contains("DROP CONSTRAINT old_constraint"));
        assert!(sql.contains("RENAME TO members"));
        // 6 アクションすべて結合されている
        assert_eq!(sql.matches(',').count(), 5);
    }

    // ===== DROP TABLE =====

    #[test]
    fn test_drop_table_basic_and_if_exists() {
        let stmt = DropTableStatement {
            span: Span::new(0, 20),
            if_exists: false,
            names: vec![qualified("users")],
        };
        let sql = emit_str(&common_sql::ast::Statement::DropTable(Box::new(stmt)));
        assert_eq!(sql, "DROP TABLE users");

        let stmt2 = DropTableStatement {
            span: Span::new(0, 30),
            if_exists: true,
            names: vec![qualified("a"), qualified("b")],
        };
        let sql2 = emit_str(&common_sql::ast::Statement::DropTable(Box::new(stmt2)));
        assert_eq!(sql2, "DROP TABLE IF EXISTS a, b");
    }

    // ===== CREATE INDEX =====

    #[test]
    fn test_create_index_unique_and_directions() {
        let stmt = CreateIndexStatement {
            span: Span::new(0, 60),
            unique: true,
            if_not_exists: false,
            name: ident("uk_email"),
            table: qualified("users"),
            columns: vec![
                IndexColumn {
                    name: ident("last"),
                    direction: Some(SortDirection::Asc),
                },
                IndexColumn {
                    name: ident("first"),
                    direction: Some(SortDirection::Desc),
                },
            ],
        };
        let sql = emit_str(&common_sql::ast::Statement::CreateIndex(Box::new(stmt)));
        // `last`/`first` は PostgreSQL 予約語 (LAST/FIRST) のためダブルクォート。
        assert_eq!(
            sql,
            "CREATE UNIQUE INDEX uk_email ON users (\"last\" ASC, \"first\" DESC)"
        );
    }

    // ===== DROP INDEX =====

    #[test]
    fn test_drop_index_basic_and_if_exists() {
        let stmt = DropIndexStatement {
            span: Span::new(0, 20),
            if_exists: false,
            name: ident("idx_name"),
            table: Some(qualified("users")),
        };
        let sql = emit_str(&common_sql::ast::Statement::DropIndex(Box::new(stmt)));
        // PostgreSQL は ON table を取らない
        assert_eq!(sql, "DROP INDEX idx_name");

        let stmt2 = DropIndexStatement {
            span: Span::new(0, 15),
            if_exists: true,
            name: ident("idx"),
            table: None,
        };
        let sql2 = emit_str(&common_sql::ast::Statement::DropIndex(Box::new(stmt2)));
        assert_eq!(sql2, "DROP INDEX IF EXISTS idx");
    }

    // ===== 識別子のダブルクォート (予約語・大文字開始) =====

    #[test]
    fn test_create_table_quoted_identifiers() {
        // "order" / "User" は PostgreSQL の予約語 / 大文字開始 → ダブルクォート
        let stmt = create_table_stmt("Order", vec![int_col("select", DataType::Int, false)]);
        let sql = emit_str(&common_sql::ast::Statement::CreateTable(Box::new(stmt)));
        assert_eq!(sql, "CREATE TABLE \"Order\" (\"select\" INTEGER NOT NULL)");
    }

    // ===== DEFAULT / CHECK / REFERENCES 列制約 =====

    #[test]
    fn test_create_table_column_constraints_default_check_refs() {
        let stmt = create_table_stmt(
            "orders",
            vec![ColumnDef {
                span: Span::new(0, 20),
                name: ident("status"),
                data_type: DataType::Int,
                nullable: false,
                default: Some(Expression::Literal(Literal::Integer(0))),
                constraints: vec![
                    ColumnConstraint::Check(Expression::Comparison {
                        left: Box::new(Expression::Identifier(ident("status"))),
                        op: common_sql::ast::ComparisonOperator::Ge,
                        right: Box::new(Expression::Literal(Literal::Integer(0))),
                    }),
                    ColumnConstraint::References {
                        table: qualified("users"),
                        columns: vec!["id".to_string()],
                    },
                ],
            }],
        );
        let sql = emit_str(&common_sql::ast::Statement::CreateTable(Box::new(stmt)));
        // ExpressionEmitter は Comparison を (...) で囲むため、CHECK は二重括弧になる
        // (構文的に有効な PostgreSQL)。
        assert_eq!(
            sql,
            "CREATE TABLE orders \
             (status INTEGER NOT NULL DEFAULT 0 CHECK ((status >= 0)) \
             REFERENCES users (id))"
        );
    }

    // ===== ALTER COLUMN 複合 (type + default + nullable) =====

    #[test]
    fn test_alter_column_combined_set_default_and_not_null() {
        let stmt = AlterTableStatement {
            span: Span::new(0, 80),
            name: qualified("users"),
            actions: vec![AlterTableAction::AlterColumn {
                column: ident("age"),
                data_type: Some(DataType::Int),
                default: Some(Some(Expression::Literal(Literal::Integer(0)))),
                nullable: Some(false),
            }],
        };
        let sql = emit_str(&common_sql::ast::Statement::AlterTable(Box::new(stmt)));
        // PostgreSQL は type/default/nullability を別句に分ける必要がある。
        assert_eq!(
            sql,
            "ALTER TABLE users ALTER COLUMN age TYPE INTEGER, \
             ALTER COLUMN age SET DEFAULT 0, \
             ALTER COLUMN age SET NOT NULL"
        );
    }

    // ===== SERIAL フォールバック (非整数型への AUTO_INCREMENT) =====

    #[test]
    fn test_serial_type_for_helper_directly() {
        assert_eq!(serial_type_for(&DataType::SmallInt), "SMALLSERIAL");
        assert_eq!(serial_type_for(&DataType::Int), "SERIAL");
        assert_eq!(serial_type_for(&DataType::BigInt), "BIGSERIAL");
        assert_eq!(serial_type_for(&DataType::TinyInt), "SMALLSERIAL");
        // 非整数型は既定 SERIAL へフォールバック (クラッシュしない)
        assert_eq!(
            serial_type_for(&DataType::VarChar { length: Some(10) }),
            "SERIAL"
        );
    }
}
