//! DDL (CREATE / ALTER / DROP TABLE + CREATE / DROP INDEX) emit for SQLite.
//!
//! Design §0.4 (SQLite ALTER TABLE 限制):
//! - `ADD COLUMN` — supported (SQLite >= 3.35 era constraint-compatible).
//! - `DROP COLUMN` — supported (SQLite >= 3.35.0).
//! - `ALTER COLUMN` (type change) — returns [`EmitError::Unsupported`] because
//!   SQLite has no native `ALTER COLUMN` (requires the 12-step table-rebuild
//!   procedure; out of scope for the emitter).
//! - `DROP CONSTRAINT` — returns [`EmitError::Unsupported`] for the same
//!   reason (SQLite constraints live inside the table definition).
//! - `ADD CONSTRAINT` / `RENAME TO` — also `Unsupported` here; they map to
//!   table-rebuild / `ALTER TABLE ... RENAME TO` respectively, the latter is a
//!   straightforward future extension.
//!
//! Design §0.6: callers receive only converted common-sql `DataType` values
//! (the T-SQL → common-sql short-circuit happens in the converter, T2). This
//! module reuses [`SqliteEmitter::emit_data_type`] and never re-implements
//! T-SQL type mapping.

use common_sql::ast::{
    AlterTableAction, AlterTableStatement, ColumnConstraint, ColumnDef, CreateIndexStatement,
    CreateTableStatement, DropIndexStatement, DropTableStatement, IndexColumn, TableConstraint,
};

use crate::error::EmitError;
use crate::SqliteEmitter;

impl SqliteEmitter {
    /// `CREATE TABLE` 文を emit する。
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
        // カラム定義 → テーブル制約の順で出力。両者が混在する場合は
        // 要素間を ", " で区切る。
        let mut first = true;
        for col in &stmt.columns {
            if !first {
                self.write(", ");
            }
            first = false;
            self.visit_column_def(col)?;
        }
        for c in &stmt.constraints {
            if !first {
                self.write(", ");
            }
            first = false;
            self.visit_table_constraint(c)?;
        }
        self.write(")");
        // SQLite は MySQL 系の ENGINE/CHARSET/COLLATE/COMMENT を無視するため、
        // TableOptions は出力しない (design §0.4: silent skip, not error)。
        Ok(())
    }

    /// `ALTER TABLE` 文を emit する (design §0.4)。
    ///
    /// `ADD COLUMN` / `DROP COLUMN` のみサポート。`ALTER COLUMN` (型変更) と
    /// `DROP CONSTRAINT` は [`EmitError::Unsupported`] を返す。
    pub(crate) fn visit_alter_table(
        &mut self,
        stmt: &AlterTableStatement,
    ) -> Result<(), EmitError> {
        self.write("ALTER TABLE ");
        self.write_qualified_name(&stmt.name);
        // ALTER TABLE は複数アクションを許容する AST だが、SQLite は1文につき
        // 単一アクションのみを許す。AST が単一アクションを運ぶ場合は直接、
        // 2件以上の場合は先頭アクションで Unsupported を返して呼び出し側に
        // バッチ分割を促す (design §0.4)。
        if stmt.actions.len() != 1 {
            return Err(EmitError::Unsupported(format!(
                "ALTER TABLE with {} actions (SQLite requires one action per statement)",
                stmt.actions.len()
            )));
        }
        match &stmt.actions[0] {
            AlterTableAction::AddColumn(col) => {
                self.write(" ADD COLUMN ");
                self.visit_column_def(col)?;
                Ok(())
            }
            AlterTableAction::DropColumn(name) => {
                self.write(" DROP COLUMN ");
                self.write_identifier(name.value());
                Ok(())
            }
            AlterTableAction::AlterColumn { column, .. } => {
                // SQLite は ALTER COLUMN をネイティブサポートしない (design §0.4)。
                Err(EmitError::Unsupported(format!(
                    "ALTER COLUMN \"{column}\" type/default change (SQLite requires table-rebuild)",
                    column = column.value()
                )))
            }
            AlterTableAction::AddConstraint(c) => {
                // ADD CONSTRAINT も同様に table-rebuild が必要だが、将来的な
                // RENAME 拡張の余地を残す。現状は Unsupported。
                let _ = c;
                Err(EmitError::Unsupported(
                    "ALTER TABLE ... ADD CONSTRAINT (SQLite requires table-rebuild)".to_string(),
                ))
            }
            AlterTableAction::DropConstraint(name) => {
                // DROP CONSTRAINT は design §0.4 で明示 Unsupported。
                Err(EmitError::Unsupported(format!(
                    "DROP CONSTRAINT {name} (SQLite requires table-rebuild)"
                )))
            }
            AlterTableAction::RenameTo(new_name) => {
                // RENAME TO は SQLite ネイティブサポート。
                self.write(" RENAME TO ");
                self.write_qualified_name(new_name);
                Ok(())
            }
        }
    }

    /// `DROP TABLE` 文を emit する。
    pub(crate) fn visit_drop_table(&mut self, stmt: &DropTableStatement) -> Result<(), EmitError> {
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
        Ok(())
    }

    /// `CREATE INDEX` 文を emit する。
    pub(crate) fn visit_create_index(
        &mut self,
        stmt: &CreateIndexStatement,
    ) -> Result<(), EmitError> {
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
        self.write(" (");
        for (i, col) in stmt.columns.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.visit_index_column(col);
        }
        self.write(")");
        Ok(())
    }

    /// `DROP INDEX` 文を emit する。
    pub(crate) fn visit_drop_index(&mut self, stmt: &DropIndexStatement) -> Result<(), EmitError> {
        self.write("DROP INDEX ");
        if stmt.if_exists {
            self.write("IF EXISTS ");
        }
        self.write_identifier(stmt.name.value());
        // SQLite の DROP INDEX は ON table を許容しないが、情報として付与可能。
        // ここでは AST の table が存在しても省略する (SQLite 構文準拠)。
        let _ = &stmt.table;
        Ok(())
    }

    // ---- 共有ヘルパー (CREATE TABLE / ALTER TABLE ADD COLUMN) ----

    /// カラム定義を出力する。
    fn visit_column_def(&mut self, col: &ColumnDef) -> Result<(), EmitError> {
        self.write_identifier(col.name.value());
        self.write(" ");
        self.write(&Self::emit_data_type(&col.data_type));
        if !col.nullable {
            self.write(" NOT NULL");
        }
        if let Some(default_expr) = &col.default {
            self.write(" DEFAULT ");
            self.visit_expression(default_expr)?;
        }
        for c in &col.constraints {
            self.visit_column_constraint(c)?;
        }
        Ok(())
    }

    /// カラムレベル制約を出力する。
    fn visit_column_constraint(&mut self, c: &ColumnConstraint) -> Result<(), EmitError> {
        match c {
            ColumnConstraint::PrimaryKey => self.write(" PRIMARY KEY"),
            ColumnConstraint::Unique => self.write(" UNIQUE"),
            ColumnConstraint::Check(expr) => {
                self.write(" CHECK (");
                self.visit_expression(expr)?;
                self.write(")");
            }
            ColumnConstraint::References { table, columns } => {
                self.write(" REFERENCES ");
                self.write_qualified_name(table);
                self.write(" (");
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write_identifier(col);
                }
                self.write(")");
            }
            ColumnConstraint::AutoIncrement => self.write(" AUTOINCREMENT"),
        }
        Ok(())
    }

    /// テーブルレベル制約を出力する。
    fn visit_table_constraint(&mut self, c: &TableConstraint) -> Result<(), EmitError> {
        match c {
            TableConstraint::PrimaryKey { name, columns } => {
                if let Some(n) = name {
                    self.write("CONSTRAINT ");
                    self.write_identifier(n);
                    self.write(" ");
                }
                self.write("PRIMARY KEY (");
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write_identifier(col.value());
                }
                self.write(")");
            }
            TableConstraint::Unique { name, columns } => {
                if let Some(n) = name {
                    self.write("CONSTRAINT ");
                    self.write_identifier(n);
                    self.write(" ");
                }
                self.write("UNIQUE (");
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write_identifier(col.value());
                }
                self.write(")");
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
                self.write("FOREIGN KEY (");
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write_identifier(col.value());
                }
                self.write(") REFERENCES ");
                self.write_qualified_name(ref_table);
                self.write(" (");
                for (i, col) in ref_columns.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write_identifier(col.value());
                }
                self.write(")");
            }
            TableConstraint::Check { name, expr } => {
                if let Some(n) = name {
                    self.write("CONSTRAINT ");
                    self.write_identifier(n);
                    self.write(" ");
                }
                self.write("CHECK (");
                self.visit_expression(expr)?;
                self.write(")");
            }
        }
        Ok(())
    }

    /// INDEX カラム (名前 + ソート方向) を出力する。
    fn visit_index_column(&mut self, col: &IndexColumn) {
        self.write_identifier(col.name.value());
        match col.direction {
            None => {}
            Some(common_sql::ast::clause::SortDirection::Asc) => self.write(" ASC"),
            Some(common_sql::ast::clause::SortDirection::Desc) => self.write(" DESC"),
        }
    }
}

// =============================================================================
// Tests (TDD)
// =============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use common_sql::ast::clause::SortDirection;
    use common_sql::ast::{
        ColumnConstraint, ColumnDef, CreateIndexStatement, CreateTableStatement, DataType,
        DropIndexStatement, DropTableStatement, Identifier, Literal, QualifiedName, Span,
        Statement, TableConstraint, TableOptions,
    };

    use crate::{EmitError, EmitterConfig, SqliteEmitter};

    fn emitter() -> SqliteEmitter {
        SqliteEmitter::default()
    }

    fn unquoted_emitter() -> SqliteEmitter {
        SqliteEmitter::new(EmitterConfig {
            uppercase_keywords: false,
            quote_identifiers: false,
            indent_size: 4,
        })
    }

    fn q(name: &str) -> QualifiedName {
        QualifiedName::new(None, name.to_string())
    }

    fn ident(name: &str) -> Identifier {
        Identifier::new(name.to_string())
    }

    fn column(name: &str, dt: DataType) -> ColumnDef {
        ColumnDef {
            span: Span::new(0, 10),
            name: ident(name),
            data_type: dt,
            nullable: true,
            default: None,
            constraints: vec![],
        }
    }

    /// `visit_*` DDL メソッドを呼び出し、バッファの内容を String として取り出す。
    /// visit_* は `Result<(), EmitError>` を返し、出力は emitter のバッファに書き込まれる
    /// ため、テストからはバッファを読み出して検証する。
    fn run_ddl<F>(mut e: SqliteEmitter, f: F) -> Result<String, EmitError>
    where
        F: FnOnce(&mut SqliteEmitter) -> Result<(), EmitError>,
    {
        f(&mut e)?;
        Ok(std::mem::take(&mut e.buffer))
    }

    // ============================================================
    // UC-1: CREATE TABLE
    // ============================================================

    #[test]
    fn test_visit_create_table_basic() {
        // CREATE TABLE users (id INTEGER NOT NULL, name TEXT)
        let stmt = CreateTableStatement {
            span: Span::new(0, 50),
            if_not_exists: false,
            temporary: false,
            name: q("users"),
            columns: vec![
                ColumnDef {
                    nullable: false,
                    constraints: vec![ColumnConstraint::PrimaryKey],
                    ..column("id", DataType::BigInt)
                },
                column("name", DataType::Text),
            ],
            constraints: vec![],
            options: TableOptions::default(),
        };
        let sql = run_ddl(emitter(), |e| e.visit_create_table(&stmt));
        assert!(sql.is_ok());
        assert_eq!(
            sql.unwrap(),
            "CREATE TABLE \"users\" (\"id\" INTEGER NOT NULL PRIMARY KEY, \"name\" TEXT)"
        );
    }

    #[test]
    fn test_visit_create_table_if_not_exists_and_options_silently_skipped() {
        // MySQL 系 TableOptions は SQLite で無視される (design §0.4)。
        let stmt = CreateTableStatement {
            span: Span::new(0, 50),
            if_not_exists: true,
            temporary: false,
            name: q("t"),
            columns: vec![column("c", DataType::Int)],
            constraints: vec![],
            options: TableOptions {
                engine: Some("InnoDB".to_string()),
                charset: Some("utf8mb4".to_string()),
                collation: None,
                comment: Some("ignored".to_string()),
            },
        };
        let sql = run_ddl(emitter(), |e| e.visit_create_table(&stmt)).unwrap();
        assert_eq!(sql, "CREATE TABLE IF NOT EXISTS \"t\" (\"c\" INTEGER)");
    }

    #[test]
    fn test_visit_create_table_table_constraint() {
        // テーブルレベル PRIMARY KEY 制約。
        let stmt = CreateTableStatement {
            span: Span::new(0, 60),
            if_not_exists: false,
            temporary: false,
            name: q("t"),
            columns: vec![column("a", DataType::Int), column("b", DataType::Int)],
            constraints: vec![TableConstraint::PrimaryKey {
                name: Some("pk_t".to_string()),
                columns: vec![ident("a"), ident("b")],
            }],
            options: TableOptions::default(),
        };
        let sql = run_ddl(emitter(), |e| e.visit_create_table(&stmt)).unwrap();
        assert_eq!(
            sql,
            "CREATE TABLE \"t\" (\"a\" INTEGER, \"b\" INTEGER, CONSTRAINT \"pk_t\" PRIMARY KEY (\"a\", \"b\"))"
        );
    }

    // ============================================================
    // UC-1b: DROP TABLE
    // ============================================================

    #[test]
    fn test_visit_drop_table_basic() {
        let stmt = DropTableStatement {
            span: Span::new(0, 20),
            if_exists: false,
            names: vec![q("users")],
        };
        let sql = run_ddl(emitter(), |e| e.visit_drop_table(&stmt));
        assert!(sql.is_ok());
        assert_eq!(sql.unwrap(), "DROP TABLE \"users\"");
    }

    #[test]
    fn test_visit_drop_table_if_exists_multiple() {
        let stmt = DropTableStatement {
            span: Span::new(0, 40),
            if_exists: true,
            names: vec![q("a"), q("b")],
        };
        let sql = run_ddl(emitter(), |e| e.visit_drop_table(&stmt)).unwrap();
        assert_eq!(sql, "DROP TABLE IF EXISTS \"a\", \"b\"");
    }

    // ============================================================
    // UC-1c: CREATE INDEX
    // ============================================================

    #[test]
    fn test_visit_create_index_basic() {
        let stmt = CreateIndexStatement {
            span: Span::new(0, 40),
            unique: false,
            if_not_exists: false,
            name: ident("idx_name"),
            table: q("users"),
            columns: vec![common_sql::ast::IndexColumn {
                name: ident("name"),
                direction: None,
            }],
        };
        let sql = run_ddl(emitter(), |e| e.visit_create_index(&stmt));
        assert!(sql.is_ok());
        assert_eq!(
            sql.unwrap(),
            "CREATE INDEX \"idx_name\" ON \"users\" (\"name\")"
        );
    }

    #[test]
    fn test_visit_create_index_unique_desc_multi() {
        let stmt = CreateIndexStatement {
            span: Span::new(0, 50),
            unique: true,
            if_not_exists: true,
            name: ident("uk_email"),
            table: q("users"),
            columns: vec![
                common_sql::ast::IndexColumn {
                    name: ident("email"),
                    direction: Some(SortDirection::Desc),
                },
                common_sql::ast::IndexColumn {
                    name: ident("domain"),
                    direction: Some(SortDirection::Asc),
                },
            ],
        };
        let sql = run_ddl(emitter(), |e| e.visit_create_index(&stmt)).unwrap();
        assert_eq!(
            sql,
            "CREATE UNIQUE INDEX IF NOT EXISTS \"uk_email\" ON \"users\" (\"email\" DESC, \"domain\" ASC)"
        );
    }

    // ============================================================
    // UC-1d: DROP INDEX
    // ============================================================

    #[test]
    fn test_visit_drop_index_basic() {
        // SQLite の DROP INDEX は ON table を許容しないため省略する。
        let stmt = DropIndexStatement {
            span: Span::new(0, 20),
            if_exists: false,
            name: ident("idx_name"),
            table: Some(q("users")),
        };
        let sql = run_ddl(emitter(), |e| e.visit_drop_index(&stmt));
        assert!(sql.is_ok());
        assert_eq!(sql.unwrap(), "DROP INDEX \"idx_name\"");
    }

    #[test]
    fn test_visit_drop_index_if_exists() {
        let stmt = DropIndexStatement {
            span: Span::new(0, 15),
            if_exists: true,
            name: ident("idx"),
            table: None,
        };
        let sql = run_ddl(emitter(), |e| e.visit_drop_index(&stmt)).unwrap();
        assert_eq!(sql, "DROP INDEX IF EXISTS \"idx\"");
    }

    // ============================================================
    // UC-2: ALTER TABLE (ADD COLUMN / DROP COLUMN — SQLite 3.35+)
    // ============================================================

    #[test]
    fn test_visit_alter_table_add_column() {
        use common_sql::ast::{AlterTableAction, AlterTableStatement};
        let stmt = AlterTableStatement {
            span: Span::new(0, 30),
            name: q("users"),
            actions: vec![AlterTableAction::AddColumn(ColumnDef {
                span: Span::new(10, 30),
                name: ident("email"),
                data_type: DataType::VarChar { length: Some(255) },
                nullable: true,
                default: None,
                constraints: vec![],
            })],
        };
        let sql = run_ddl(emitter(), |e| e.visit_alter_table(&stmt));
        assert!(sql.is_ok());
        assert_eq!(
            sql.unwrap(),
            "ALTER TABLE \"users\" ADD COLUMN \"email\" VARCHAR(255)"
        );
    }

    #[test]
    fn test_visit_alter_table_drop_column() {
        use common_sql::ast::{AlterTableAction, AlterTableStatement};
        let stmt = AlterTableStatement {
            span: Span::new(0, 30),
            name: q("users"),
            actions: vec![AlterTableAction::DropColumn(ident("email"))],
        };
        let sql = run_ddl(emitter(), |e| e.visit_alter_table(&stmt)).unwrap();
        assert_eq!(sql, "ALTER TABLE \"users\" DROP COLUMN \"email\"");
    }

    #[test]
    fn test_visit_alter_table_rename_to() {
        use common_sql::ast::{AlterTableAction, AlterTableStatement};
        let stmt = AlterTableStatement {
            span: Span::new(0, 30),
            name: q("old"),
            actions: vec![AlterTableAction::RenameTo(q("new"))],
        };
        let sql = run_ddl(emitter(), |e| e.visit_alter_table(&stmt)).unwrap();
        assert_eq!(sql, "ALTER TABLE \"old\" RENAME TO \"new\"");
    }

    // ============================================================
    // UC-3 (edge): ALTER COLUMN type-change = Unsupported (design §0.4)
    // ============================================================

    #[test]
    fn test_visit_alter_table_alter_column_type_change_is_unsupported() {
        use common_sql::ast::{AlterTableAction, AlterTableStatement};
        let stmt = AlterTableStatement {
            span: Span::new(0, 40),
            name: q("users"),
            actions: vec![AlterTableAction::AlterColumn {
                column: ident("name"),
                data_type: Some(DataType::VarChar { length: Some(200) }),
                default: None,
                nullable: None,
            }],
        };
        let result = emitter().visit_alter_table(&stmt);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmitError::Unsupported(msg) => {
                assert!(
                    msg.contains("ALTER COLUMN") && msg.contains("name"),
                    "expected ALTER COLUMN name in: {msg}"
                );
            }
            other => panic!("expected Unsupported for ALTER COLUMN, got {other:?}"),
        }
    }

    #[test]
    fn test_visit_alter_table_drop_constraint_is_unsupported() {
        use common_sql::ast::{AlterTableAction, AlterTableStatement};
        let stmt = AlterTableStatement {
            span: Span::new(0, 40),
            name: q("users"),
            actions: vec![AlterTableAction::DropConstraint("uk_email".to_string())],
        };
        let result = emitter().visit_alter_table(&stmt);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmitError::Unsupported(msg) => {
                assert!(
                    msg.contains("DROP CONSTRAINT") && msg.contains("uk_email"),
                    "expected DROP CONSTRAINT uk_email in: {msg}"
                );
            }
            other => panic!("expected Unsupported for DROP CONSTRAINT, got {other:?}"),
        }
    }

    #[test]
    fn test_visit_alter_table_multiple_actions_is_unsupported() {
        // SQLite は1文につき単一アクションしか許容しない。
        use common_sql::ast::{AlterTableAction, AlterTableStatement};
        let stmt = AlterTableStatement {
            span: Span::new(0, 60),
            name: q("users"),
            actions: vec![
                AlterTableAction::AddColumn(column("a", DataType::Int)),
                AlterTableAction::DropColumn(ident("b")),
            ],
        };
        let result = emitter().visit_alter_table(&stmt);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmitError::Unsupported(msg) => {
                assert!(msg.contains("2 actions"), "unexpected: {msg}");
            }
            other => panic!("expected Unsupported for multi-action, got {other:?}"),
        }
    }

    // ============================================================
    // UC-2b: ADD COLUMN with DEFAULT and constraints
    // ============================================================

    #[test]
    fn test_visit_column_def_default_and_unique() {
        let col = ColumnDef {
            span: Span::new(0, 30),
            name: ident("status"),
            data_type: DataType::Int,
            nullable: false,
            default: Some(common_sql::ast::Expression::Literal(Literal::Integer(0))),
            constraints: vec![ColumnConstraint::Unique],
        };
        // 個別ヘルパー経由ではなく、ALTER TABLE 経由で確認。
        use common_sql::ast::{AlterTableAction, AlterTableStatement};
        let stmt = AlterTableStatement {
            span: Span::new(0, 40),
            name: q("t"),
            actions: vec![AlterTableAction::AddColumn(col)],
        };
        let sql = run_ddl(emitter(), |e| e.visit_alter_table(&stmt)).unwrap();
        assert_eq!(
            sql,
            "ALTER TABLE \"t\" ADD COLUMN \"status\" INTEGER NOT NULL DEFAULT 0 UNIQUE"
        );
    }

    // ============================================================
    // UC-3b: unquoted identifier config path
    // ============================================================

    #[test]
    fn test_create_table_unquoted_when_configured() {
        let stmt = CreateTableStatement {
            span: Span::new(0, 30),
            if_not_exists: false,
            temporary: false,
            name: q("t"),
            columns: vec![column("c", DataType::Int)],
            constraints: vec![],
            options: TableOptions::default(),
        };
        let sql = run_ddl(unquoted_emitter(), |e| e.visit_create_table(&stmt)).unwrap();
        assert_eq!(sql, "CREATE TABLE t (c INTEGER)");
    }

    // ============================================================
    // dispatch integration (via emit) — DialectSpecific stays Unsupported
    // ============================================================

    #[test]
    fn test_emit_dispatches_create_table() {
        let ddl = Statement::CreateTable(Box::new(CreateTableStatement {
            span: Span::new(0, 30),
            if_not_exists: false,
            temporary: false,
            name: q("t"),
            columns: vec![column("c", DataType::Int)],
            constraints: vec![],
            options: TableOptions::default(),
        }));
        let sql = emitter().emit(&ddl).unwrap();
        assert_eq!(sql, "CREATE TABLE \"t\" (\"c\" INTEGER)");
    }

    #[test]
    fn test_emit_dispatches_drop_index() {
        let ddl = Statement::DropIndex(Box::new(DropIndexStatement {
            span: Span::new(0, 20),
            if_exists: true,
            name: ident("idx"),
            table: None,
        }));
        let sql = emitter().emit(&ddl).unwrap();
        assert_eq!(sql, "DROP INDEX IF EXISTS \"idx\"");
    }

    #[test]
    fn test_emit_alter_column_unsupported_via_dispatch() {
        use common_sql::ast::{AlterTableAction, AlterTableStatement};
        let ddl = Statement::AlterTable(Box::new(AlterTableStatement {
            span: Span::new(0, 40),
            name: q("users"),
            actions: vec![AlterTableAction::AlterColumn {
                column: ident("name"),
                data_type: Some(DataType::Text),
                default: None,
                nullable: None,
            }],
        }));
        let result = emitter().emit(&ddl);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EmitError::Unsupported(_)));
    }

    #[test]
    fn test_emit_dialect_specific_still_unsupported() {
        // design: DialectSpecific は Unsupported を維持 (native 変換しない)。
        let stmt = Statement::DialectSpecific {
            source: "DECLARE @v INT".to_string(),
            span: Span::new(0, 15),
        };
        let result = emitter().emit(&stmt);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EmitError::Unsupported(_)));
    }
}
