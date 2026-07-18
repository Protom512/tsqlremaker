//! MySQL Emitter integration tests
//!
//! Common SQL AST → MySQL SQL 変換のテスト。
//!
//! 設計決定 DD-3 に基づき、共通 AST は本テスト内で直接構築する
//! (ブリッジの DDL ギャップと、mysql-emitter の tsql-* 直接依存ゼロ要件のため)。
//! E2E (T-SQL parse → common-sql → MySQL) は別途ブリッジ網羅時に追加する。

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use common_sql::ast::clause::{OrderByClause, OrderByItem, SortDirection};
use common_sql::ast::identifier::{Identifier, QualifiedName};
use common_sql::ast::literal::Literal;
use common_sql::ast::{
    Assignment, ComparisonOperator, DeleteStatement, Expression, InsertSource, InsertStatement,
    SelectItem, SelectStatement, Statement, TableFactor, UpdateStatement,
};
use mysql_emitter::{EmitterConfig, MySqlEmitter};

fn emitter() -> MySqlEmitter {
    MySqlEmitter::new(EmitterConfig::default())
}

fn ident(name: &str) -> Identifier {
    Identifier::new(name.to_string())
}

fn id_expr(name: &str) -> Expression {
    Expression::Identifier(ident(name))
}

fn int_expr(n: i64) -> Expression {
    Expression::Literal(Literal::Integer(n))
}

fn str_expr(s: &str) -> Expression {
    Expression::Literal(Literal::String(s.to_string()))
}

fn table(name: &str) -> TableFactor {
    TableFactor::Table {
        name: QualifiedName::new(None, name.to_string()),
        alias: None,
    }
}

fn span() -> common_sql::ast::Span {
    common_sql::ast::Span::new(0, 0)
}

/// SELECT * FROM <table> の基本的な発行テスト
#[test]
fn emit_select_star() {
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("users")),
        where_clause: None,
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let mysql_sql = emitter().emit(&Statement::Select(Box::new(stmt))).unwrap();
    assert_eq!(mysql_sql, "SELECT * FROM users");
}

/// WHERE 句付き SELECT の発行テスト
#[test]
fn emit_select_with_where() {
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![
            SelectItem::Expression {
                expr: id_expr("id"),
                alias: None,
            },
            SelectItem::Expression {
                expr: id_expr("name"),
                alias: None,
            },
        ],
        from: Some(table("users")),
        where_clause: Some(Expression::Comparison {
            left: Box::new(id_expr("id")),
            op: ComparisonOperator::Eq,
            right: Box::new(int_expr(1)),
        }),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let mysql_sql = emitter().emit(&Statement::Select(Box::new(stmt))).unwrap();
    assert!(mysql_sql.contains("SELECT"));
    assert!(mysql_sql.contains("FROM"));
    assert!(mysql_sql.contains("WHERE `id` = 1"));
}

/// UPDATE 文の発行テスト
#[test]
fn emit_update() {
    let stmt = UpdateStatement {
        span: span(),
        table: table("users"),
        assignments: vec![Assignment {
            column: ident("name"),
            value: str_expr("Bob"),
        }],
        from: None,
        where_clause: Some(Expression::Comparison {
            left: Box::new(id_expr("id")),
            op: ComparisonOperator::Eq,
            right: Box::new(int_expr(1)),
        }),
    };
    let mysql_sql = emitter().emit(&Statement::Update(Box::new(stmt))).unwrap();
    assert_eq!(mysql_sql, "UPDATE users SET `name` = 'Bob' WHERE `id` = 1");
}

/// INSERT 文の発行テスト
#[test]
fn emit_insert_values() {
    let stmt = InsertStatement {
        span: span(),
        table: QualifiedName::new(None, "users".to_string()),
        columns: vec![ident("id"), ident("name")],
        source: InsertSource::Values(vec![vec![int_expr(1), str_expr("Bob")]]),
        on_conflict: None,
    };
    let mysql_sql = emitter().emit(&Statement::Insert(Box::new(stmt))).unwrap();
    assert_eq!(
        mysql_sql,
        "INSERT INTO users (`id`, `name`) VALUES (1, 'Bob')"
    );
}

/// DELETE 文の発行テスト
#[test]
fn emit_delete() {
    let stmt = DeleteStatement {
        span: span(),
        table: table("users"),
        using: None,
        where_clause: Some(Expression::Comparison {
            left: Box::new(id_expr("id")),
            op: ComparisonOperator::Eq,
            right: Box::new(int_expr(1)),
        }),
    };
    let mysql_sql = emitter().emit(&Statement::Delete(Box::new(stmt))).unwrap();
    assert_eq!(mysql_sql, "DELETE FROM users WHERE `id` = 1");
}

/// emit_batch で複数ステートメントを発行
#[test]
fn emit_batch_semicolon_separated() {
    let s1 = Statement::Select(Box::new(SelectStatement::simple(vec![
        SelectItem::Wildcard,
    ])));
    let s2 = Statement::Select(Box::new(SelectStatement::simple(vec![
        SelectItem::Wildcard,
    ])));
    let mysql_sql = emitter().emit_batch(&[s1, s2]).unwrap();
    assert_eq!(mysql_sql, "SELECT *;\nSELECT *");
}

/// ORDER BY 付き SELECT の発行テスト
#[test]
fn emit_select_order_by() {
    let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
    stmt.from = Some(table("users"));
    stmt.order_by = Some(OrderByClause {
        span: span(),
        items: vec![OrderByItem {
            expr: id_expr("name"),
            direction: Some(SortDirection::Asc),
            nulls: None,
        }],
    });
    let mysql_sql = emitter().emit(&Statement::Select(Box::new(stmt))).unwrap();
    assert!(mysql_sql.contains("ORDER BY `name` ASC"));
}

/// LIMIT 付き SELECT の発行テスト (T-SQL TOP n は上流で LIMIT に変換済み)
#[test]
fn emit_select_limit() {
    use common_sql::ast::clause::LimitClause;
    let mut stmt = SelectStatement::simple(vec![SelectItem::Wildcard]);
    stmt.from = Some(table("users"));
    stmt.limit = Some(LimitClause {
        span: span(),
        limit: int_expr(10),
        offset: None,
    });
    let mysql_sql = emitter().emit(&Statement::Select(Box::new(stmt))).unwrap();
    assert!(mysql_sql.contains("LIMIT 10"));
}

// =============================================================================
// DDL emit tests (T3): CREATE / ALTER / DROP TABLE, CREATE / DROP INDEX
// =============================================================================

use common_sql::ast::ddl::{
    AlterTableAction, AlterTableStatement, ColumnConstraint, ColumnDef, CreateIndexStatement,
    CreateTableStatement, DropIndexStatement, DropTableStatement, IndexColumn, TableConstraint,
    TableOptions,
};
use common_sql::ast::DataType;

fn qualified(name: &str) -> QualifiedName {
    QualifiedName::new(None, name.to_string())
}

fn column(name: &str, dt: DataType) -> ColumnDef {
    ColumnDef {
        span: span(),
        name: ident(name),
        data_type: dt,
        nullable: true,
        default: None,
        constraints: vec![],
    }
}

/// CREATE TABLE — single column, no constraints, no options.
#[test]
fn emit_create_table_basic() {
    let stmt = CreateTableStatement {
        span: span(),
        if_not_exists: false,
        temporary: false,
        name: qualified("users"),
        columns: vec![ColumnDef {
            span: span(),
            name: ident("id"),
            data_type: DataType::BigInt,
            nullable: false,
            default: None,
            constraints: vec![ColumnConstraint::PrimaryKey],
        }],
        constraints: vec![],
        options: TableOptions::default(),
    };
    let sql = emitter()
        .emit(&Statement::CreateTable(Box::new(stmt)))
        .unwrap();
    assert!(sql.starts_with("CREATE TABLE `users`"), "got: {sql}");
    assert!(
        sql.contains("`id` BIGINT NOT NULL PRIMARY KEY"),
        "got: {sql}"
    );
}

/// CREATE TABLE with TEMPORARY, IF NOT EXISTS, table-level constraint, and options.
#[test]
fn emit_create_table_with_constraint_and_options() {
    let stmt = CreateTableStatement {
        span: span(),
        if_not_exists: true,
        temporary: true,
        name: QualifiedName::new(Some("dbo".to_string()), "orders".to_string()),
        columns: vec![
            column("id", DataType::BigInt),
            column("user_id", DataType::BigInt),
            column(
                "total",
                DataType::Decimal {
                    precision: Some(18),
                    scale: Some(4),
                },
            ),
        ],
        constraints: vec![
            TableConstraint::PrimaryKey {
                name: Some("pk_orders".to_string()),
                columns: vec![ident("id")],
            },
            TableConstraint::ForeignKey {
                name: None,
                columns: vec![ident("user_id")],
                ref_table: qualified("users"),
                ref_columns: vec![ident("id")],
            },
        ],
        options: TableOptions {
            engine: Some("InnoDB".to_string()),
            charset: Some("utf8mb4".to_string()),
            collation: None,
            comment: Some("order table".to_string()),
        },
    };
    let sql = emitter()
        .emit(&Statement::CreateTable(Box::new(stmt)))
        .unwrap();
    assert!(
        sql.starts_with("CREATE TEMPORARY TABLE IF NOT EXISTS `dbo`.`orders`"),
        "got: {sql}"
    );
    assert!(
        sql.contains("CONSTRAINT `pk_orders` PRIMARY KEY (`id`)"),
        "got: {sql}"
    );
    assert!(
        sql.contains("FOREIGN KEY (`user_id`) REFERENCES `users` (`id`)"),
        "got: {sql}"
    );
    assert!(sql.contains("ENGINE=InnoDB"), "got: {sql}");
    assert!(sql.contains("DEFAULT CHARSET=utf8mb4"), "got: {sql}");
    assert!(sql.contains("COMMENT='order table'"), "got: {sql}");
}

/// ALTER TABLE — all 6 AlterTableAction variants in one statement.
#[test]
fn emit_alter_table_all_actions() {
    let stmt = AlterTableStatement {
        span: span(),
        name: qualified("users"),
        actions: vec![
            AlterTableAction::AddColumn(ColumnDef {
                span: span(),
                name: ident("email"),
                data_type: DataType::VarChar { length: Some(255) },
                nullable: true,
                default: None,
                constraints: vec![],
            }),
            AlterTableAction::DropColumn(ident("legacy")),
            AlterTableAction::AlterColumn {
                column: ident("name"),
                data_type: Some(DataType::VarChar { length: Some(200) }),
                default: None,
                nullable: Some(false),
            },
            AlterTableAction::AddConstraint(TableConstraint::Unique {
                name: Some("uk_email".to_string()),
                columns: vec![ident("email")],
            }),
            AlterTableAction::DropConstraint("uk_email".to_string()),
            AlterTableAction::RenameTo(qualified("accounts")),
        ],
    };
    let sql = emitter()
        .emit(&Statement::AlterTable(Box::new(stmt)))
        .unwrap();
    assert!(sql.starts_with("ALTER TABLE `users`"), "got: {sql}");
    assert!(
        sql.contains("ADD COLUMN `email` VARCHAR(255)"),
        "got: {sql}"
    );
    assert!(sql.contains("DROP COLUMN `legacy`"), "got: {sql}");
    assert!(
        sql.contains("MODIFY COLUMN `name` VARCHAR(200) NOT NULL"),
        "got: {sql}"
    );
    assert!(
        sql.contains("ADD CONSTRAINT `uk_email` UNIQUE (`email`)"),
        "got: {sql}"
    );
    assert!(sql.contains("DROP CONSTRAINT `uk_email`"), "got: {sql}");
    assert!(sql.contains("RENAME TO `accounts`"), "got: {sql}");
}

/// DROP TABLE — multiple names with IF EXISTS.
#[test]
fn emit_drop_table() {
    let stmt = DropTableStatement {
        span: span(),
        if_exists: true,
        names: vec![qualified("a"), qualified("b")],
    };
    let sql = emitter()
        .emit(&Statement::DropTable(Box::new(stmt)))
        .unwrap();
    assert_eq!(sql, "DROP TABLE IF EXISTS `a`, `b`");
}

/// CREATE [UNIQUE] INDEX with directional columns.
#[test]
fn emit_create_index() {
    let stmt = CreateIndexStatement {
        span: span(),
        unique: true,
        if_not_exists: false,
        name: ident("uk_email"),
        table: qualified("users"),
        columns: vec![IndexColumn {
            name: ident("email"),
            direction: Some(SortDirection::Desc),
        }],
    };
    let sql = emitter()
        .emit(&Statement::CreateIndex(Box::new(stmt)))
        .unwrap();
    assert_eq!(
        sql,
        "CREATE UNIQUE INDEX `uk_email` ON `users` (`email` DESC)"
    );
}

/// DROP INDEX with ON table clause.
#[test]
fn emit_drop_index() {
    let stmt = DropIndexStatement {
        span: span(),
        if_exists: false,
        name: ident("uk_email"),
        table: Some(qualified("users")),
    };
    let sql = emitter()
        .emit(&Statement::DropIndex(Box::new(stmt)))
        .unwrap();
    assert_eq!(sql, "DROP INDEX `uk_email` ON `users`");
}
