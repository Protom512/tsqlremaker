//! DDL (Data Definition Language) emit for the MySQL dialect (Task 3.3 / T3).
//!
//! [`MySqlEmitter`] から呼ばれる DDL 描画ヘルパ群。共通 AST
//! ([`common_sql::ast::ddl`]) の `CREATE TABLE` / `ALTER TABLE` / `DROP TABLE` /
//! `CREATE INDEX` / `DROP INDEX` を MySQL 方言の SQL 文字列へ変換する。
//!
//! ## 設計契約
//!
//! - 識別子はバッククォートで囲む (`converters/syntax.rs` および
//!   `statement.rs` の INSERT 列リスト `format!("`{}`", ...)` と同一規約)。
//! - [`DataType`] は [`DataTypeConverter`] に委譲し、エミッタ内で T-SQL 型
//!   マッピングを再実装しない (design §0.6: DataType short-circuit)。
//! - [`TableOptions`] はカラムリスト末尾に `ENGINE=...` / `DEFAULT CHARSET=...` /
//!   `COLLATE=...` / `COMMENT='...'` として付与する (MySQL 固有)。
//! - [`AlterTableAction`] は全 6 バリアントをサポートする。`ALTER COLUMN` は
//!   MySQL の `MODIFY COLUMN` 構文へ変換する。
//! - 式の描画 (DEFAULT 式 / CHECK 述語) は呼び出し側の [`MySqlEmitter`] が
//!   持つ式 emitter を経由するため、本モジュールの関数は `&mut MySqlEmitter`
//!   を取る。

use common_sql::ast::clause::SortDirection;
use common_sql::ast::ddl::{
    AlterTableAction, AlterTableStatement, ColumnConstraint, ColumnDef, CreateIndexStatement,
    CreateTableStatement, DropIndexStatement, DropTableStatement, IndexColumn, TableConstraint,
    TableOptions,
};
use common_sql::ast::identifier::{Identifier, QualifiedName};

use crate::converters::DataTypeConverter;
use crate::{EmitError, MySqlEmitter};

// ---------------------------------------------------------------------------
// Free rendering helpers (shared with statement.rs convention)
// ---------------------------------------------------------------------------

/// 識別子を MySQL のバッククォート付きで描画する。
fn quote_ident(id: &Identifier) -> String {
    format!("`{}`", id.value())
}

/// 修飾名 (`schema.table` / `table`) をバッククォート付きで描画する。
/// スキーマ部と名前部はそれぞれ個別にバッククォートで囲む。
fn quote_qualified(name: &QualifiedName) -> String {
    match name.schema() {
        Some(schema) => format!("`{schema}`.`{}`", name.name()),
        None => format!("`{}`", name.name()),
    }
}

// ---------------------------------------------------------------------------
// CREATE TABLE
// ---------------------------------------------------------------------------

/// `CREATE TABLE` 文を MySQL SQL へ描画する。
///
/// `if_not_exists` / `temporary` フラグ、カラム定義、テーブル制約、
/// および末尾の [`TableOptions`] をすべて反映する。
///
/// # Errors
///
/// カラムの `DEFAULT` 式や `CHECK` 述語内にサポート対象外ノードがある場合
/// [`EmitError`] を返す。
pub(crate) fn emit_create_table(
    emitter: &mut MySqlEmitter,
    s: &CreateTableStatement,
) -> Result<String, EmitError> {
    let mut sql = String::from("CREATE ");
    if s.temporary {
        sql.push_str("TEMPORARY ");
    }
    sql.push_str("TABLE ");
    if s.if_not_exists {
        sql.push_str("IF NOT EXISTS ");
    }
    sql.push_str(&quote_qualified(&s.name));
    sql.push_str(" (");

    let mut parts: Vec<String> = Vec::new();

    for col in &s.columns {
        parts.push(render_column_def(emitter, col)?);
    }

    for tc in &s.constraints {
        parts.push(render_table_constraint(emitter, tc)?);
    }

    sql.push_str(&parts.join(", "));
    sql.push(')');

    push_table_options(&mut sql, &s.options);

    Ok(sql)
}

// ---------------------------------------------------------------------------
// ALTER TABLE
// ---------------------------------------------------------------------------

/// `ALTER TABLE` 文を MySQL SQL へ描画する。
///
/// 全 6 種の [`AlterTableAction`] を順序どおりカンマ区切りで連結する。
/// `AlterColumn` は MySQL の `MODIFY COLUMN` 構文へマップされる。
///
/// # Errors
///
/// ADD COLUMN のカラム定義や ADD CONSTRAINT の CHECK 述語内にサポート対象外
/// 式ノードがある場合 [`EmitError`] を返す。
pub(crate) fn emit_alter_table(
    emitter: &mut MySqlEmitter,
    s: &AlterTableStatement,
) -> Result<String, EmitError> {
    let mut sql = String::from("ALTER TABLE ");
    sql.push_str(&quote_qualified(&s.name));

    let mut action_strs: Vec<String> = Vec::with_capacity(s.actions.len());
    for action in &s.actions {
        action_strs.push(render_alter_action(emitter, action)?);
    }

    if action_strs.is_empty() {
        // ALTER TABLE with no actions is invalid SQL, but we emit the prefix
        // rather than panic — the caller decides whether to reject upstream.
        return Ok(sql);
    }

    sql.push(' ');
    sql.push_str(&action_strs.join(", "));
    Ok(sql)
}

// ---------------------------------------------------------------------------
// DROP TABLE
// ---------------------------------------------------------------------------

/// `DROP TABLE` 文を MySQL SQL へ描画する。
pub(crate) fn emit_drop_table(s: &DropTableStatement) -> Result<String, EmitError> {
    let mut sql = String::from("DROP TABLE ");
    if s.if_exists {
        sql.push_str("IF EXISTS ");
    }
    let names: Vec<String> = s.names.iter().map(quote_qualified).collect();
    sql.push_str(&names.join(", "));
    Ok(sql)
}

// ---------------------------------------------------------------------------
// CREATE INDEX
// ---------------------------------------------------------------------------

/// `CREATE [UNIQUE] INDEX` 文を MySQL SQL へ描画する。
pub(crate) fn emit_create_index(s: &CreateIndexStatement) -> Result<String, EmitError> {
    let mut sql = String::from("CREATE ");
    if s.unique {
        sql.push_str("UNIQUE ");
    }
    sql.push_str("INDEX ");
    if s.if_not_exists {
        sql.push_str("IF NOT EXISTS ");
    }
    sql.push_str(&quote_ident(&s.name));
    sql.push_str(" ON ");
    sql.push_str(&quote_qualified(&s.table));
    sql.push_str(" (");
    let cols: Vec<String> = s.columns.iter().map(render_index_column).collect();
    sql.push_str(&cols.join(", "));
    sql.push(')');
    Ok(sql)
}

/// 1つのインデックスカラムを描画する (方向付き)。
fn render_index_column(c: &IndexColumn) -> String {
    let mut s = quote_ident(&c.name);
    if let Some(dir) = &c.direction {
        s.push(' ');
        s.push_str(match dir {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        });
    }
    s
}

// ---------------------------------------------------------------------------
// DROP INDEX
// ---------------------------------------------------------------------------

/// `DROP INDEX` 文を MySQL SQL へ描画する。
///
/// MySQL は `DROP INDEX name ON table` 構文を要求するため、`table` が
/// [`None`] の場合はテーブル修飾なしで出力する (一部ワークベンチ互換)。
pub(crate) fn emit_drop_index(s: &DropIndexStatement) -> Result<String, EmitError> {
    let mut sql = String::from("DROP INDEX ");
    if s.if_exists {
        sql.push_str("IF EXISTS ");
    }
    sql.push_str(&quote_ident(&s.name));
    if let Some(table) = &s.table {
        sql.push_str(" ON ");
        sql.push_str(&quote_qualified(table));
    }
    Ok(sql)
}

// ---------------------------------------------------------------------------
// Column / constraint rendering (shared by CREATE TABLE and ALTER TABLE ADD)
// ---------------------------------------------------------------------------

/// カラム定義を `` `name` TYPE [NOT NULL] [DEFAULT expr] [constraints] `` 形式で描画する。
fn render_column_def(emitter: &mut MySqlEmitter, col: &ColumnDef) -> Result<String, EmitError> {
    let mut s = quote_ident(&col.name);
    s.push(' ');
    s.push_str(&DataTypeConverter::convert(&col.data_type));

    if !col.nullable {
        s.push_str(" NOT NULL");
    }

    if let Some(default) = &col.default {
        s.push_str(" DEFAULT ");
        s.push_str(&emitter.emit_expression(default)?);
    }

    for c in &col.constraints {
        s.push(' ');
        s.push_str(&render_column_constraint(emitter, c)?);
    }

    Ok(s)
}

/// カラムレベル制約を描画する。
fn render_column_constraint(
    emitter: &mut MySqlEmitter,
    c: &ColumnConstraint,
) -> Result<String, EmitError> {
    Ok(match c {
        ColumnConstraint::PrimaryKey => "PRIMARY KEY".to_string(),
        ColumnConstraint::Unique => "UNIQUE".to_string(),
        ColumnConstraint::AutoIncrement => "AUTO_INCREMENT".to_string(),
        ColumnConstraint::Check(expr) => {
            format!("CHECK ({})", emitter.emit_expression(expr)?)
        }
        ColumnConstraint::References { table, columns } => {
            let cols: Vec<String> = columns.iter().map(|c| format!("`{c}`")).collect();
            format!(
                "REFERENCES {} ({})",
                quote_qualified(table),
                cols.join(", ")
            )
        }
    })
}

/// テーブルレベル制約を描画する。
fn render_table_constraint(
    emitter: &mut MySqlEmitter,
    tc: &TableConstraint,
) -> Result<String, EmitError> {
    Ok(match tc {
        TableConstraint::PrimaryKey { name, columns } => {
            let cols: Vec<String> = columns.iter().map(quote_ident).collect();
            let mut s = String::new();
            if let Some(n) = name {
                s.push_str("CONSTRAINT `");
                s.push_str(n);
                s.push_str("` ");
            }
            s.push_str("PRIMARY KEY (");
            s.push_str(&cols.join(", "));
            s.push(')');
            s
        }
        TableConstraint::Unique { name, columns } => {
            let cols: Vec<String> = columns.iter().map(quote_ident).collect();
            let mut s = String::new();
            if let Some(n) = name {
                s.push_str("CONSTRAINT `");
                s.push_str(n);
                s.push_str("` ");
            }
            s.push_str("UNIQUE (");
            s.push_str(&cols.join(", "));
            s.push(')');
            s
        }
        TableConstraint::ForeignKey {
            name,
            columns,
            ref_table,
            ref_columns,
        } => {
            let cols: Vec<String> = columns.iter().map(quote_ident).collect();
            let refs: Vec<String> = ref_columns.iter().map(quote_ident).collect();
            let mut s = String::new();
            if let Some(n) = name {
                s.push_str("CONSTRAINT `");
                s.push_str(n);
                s.push_str("` ");
            }
            s.push_str("FOREIGN KEY (");
            s.push_str(&cols.join(", "));
            s.push_str(") REFERENCES ");
            s.push_str(&quote_qualified(ref_table));
            s.push_str(" (");
            s.push_str(&refs.join(", "));
            s.push(')');
            s
        }
        TableConstraint::Check { name, expr } => {
            let mut s = String::new();
            if let Some(n) = name {
                s.push_str("CONSTRAINT `");
                s.push_str(n);
                s.push_str("` ");
            }
            s.push_str("CHECK (");
            s.push_str(&emitter.emit_expression(expr)?);
            s.push(')');
            s
        }
    })
}

/// 1つの [`AlterTableAction`] を描画する。
fn render_alter_action(
    emitter: &mut MySqlEmitter,
    action: &AlterTableAction,
) -> Result<String, EmitError> {
    Ok(match action {
        AlterTableAction::AddColumn(col) => {
            format!("ADD COLUMN {}", render_column_def(emitter, col)?)
        }
        AlterTableAction::DropColumn(col) => {
            format!("DROP COLUMN {}", quote_ident(col))
        }
        AlterTableAction::AlterColumn {
            column,
            data_type,
            default,
            nullable,
        } => {
            // MySQL は ALTER COLUMN ... TYPE をサポートせず MODIFY COLUMN を使う。
            // MODIFY は完全なカラム定義を要求するため、data_type が None の場合
            // は空の型描画となり呼び出し元の責任となる (graceful)。
            let mut s = format!("MODIFY COLUMN {}", quote_ident(column));
            if let Some(dt) = data_type {
                s.push(' ');
                s.push_str(&DataTypeConverter::convert(dt));
            }
            match nullable {
                Some(false) => s.push_str(" NOT NULL"),
                Some(true) => s.push_str(" NULL"),
                None => {}
            }
            if let Some(default_opt) = default {
                s.push_str(" DEFAULT ");
                match default_opt {
                    Some(expr) => s.push_str(&emitter.emit_expression(expr)?),
                    // Some(None) == DROP DEFAULT. MySQL MODIFY には直接対応が
                    // ないため NULL リテラルで表現する (best-effort)。
                    None => s.push_str("NULL"),
                }
            }
            s
        }
        AlterTableAction::AddConstraint(tc) => {
            format!("ADD {}", render_table_constraint(emitter, tc)?)
        }
        AlterTableAction::DropConstraint(name) => {
            format!("DROP CONSTRAINT `{name}`")
        }
        AlterTableAction::RenameTo(new_name) => {
            format!("RENAME TO {}", quote_qualified(new_name))
        }
    })
}

/// [`TableOptions`] を `ENGINE=...` 等の末尾句として `sql` に追記する。
/// オプションが1つもない場合は何も追加しない。
fn push_table_options(sql: &mut String, opts: &TableOptions) {
    let mut clauses: Vec<String> = Vec::new();
    if let Some(engine) = &opts.engine {
        clauses.push(format!("ENGINE={engine}"));
    }
    if let Some(charset) = &opts.charset {
        clauses.push(format!("DEFAULT CHARSET={charset}"));
    }
    if let Some(collation) = &opts.collation {
        clauses.push(format!("COLLATE={collation}"));
    }
    if let Some(comment) = &opts.comment {
        // シングルクォート内のエスケープは最小限 ('→'') のみ。
        let escaped = comment.replace('\'', "''");
        clauses.push(format!("COMMENT='{escaped}'"));
    }
    if clauses.is_empty() {
        return;
    }
    sql.push(' ');
    sql.push_str(&clauses.join(" "));
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::EmitterConfig;
    use common_sql::ast::{Expression, Literal};

    fn emitter() -> MySqlEmitter {
        MySqlEmitter::new(EmitterConfig::default())
    }

    fn ident(name: &str) -> Identifier {
        Identifier::new(name.to_string())
    }

    fn qualified(name: &str) -> QualifiedName {
        QualifiedName::new(None, name.to_string())
    }

    #[test]
    fn quote_ident_wraps_with_backticks() {
        assert_eq!(quote_ident(&ident("col")), "`col`");
    }

    #[test]
    fn quote_qualified_unqualified_is_single_backtick_pair() {
        assert_eq!(quote_qualified(&qualified("t")), "`t`");
    }

    #[test]
    fn quote_qualified_with_schema_emits_two_backtick_pairs() {
        let q = QualifiedName::new(Some("dbo".to_string()), "t".to_string());
        assert_eq!(quote_qualified(&q), "`dbo`.`t`");
    }

    #[test]
    fn drop_table_single_no_if_exists() {
        let stmt = DropTableStatement {
            span: common_sql::ast::Span::new(0, 0),
            if_exists: false,
            names: vec![qualified("users")],
        };
        let sql = emit_drop_table(&stmt).unwrap();
        assert_eq!(sql, "DROP TABLE `users`");
    }

    #[test]
    fn create_index_single_column_no_direction() {
        let stmt = CreateIndexStatement {
            span: common_sql::ast::Span::new(0, 0),
            unique: false,
            if_not_exists: false,
            name: ident("idx"),
            table: qualified("t"),
            columns: vec![IndexColumn {
                name: ident("c"),
                direction: None,
            }],
        };
        let sql = emit_create_index(&stmt).unwrap();
        assert_eq!(sql, "CREATE INDEX `idx` ON `t` (`c`)");
    }

    #[test]
    fn render_column_def_with_default_and_not_null() {
        let col = ColumnDef {
            span: common_sql::ast::Span::new(0, 0),
            name: ident("status"),
            data_type: common_sql::ast::DataType::Int,
            nullable: false,
            default: Some(Expression::Literal(Literal::Integer(0))),
            constraints: vec![],
        };
        let rendered = render_column_def(&mut emitter(), &col).unwrap();
        assert_eq!(rendered, "`status` INT NOT NULL DEFAULT 0");
    }

    #[test]
    fn render_table_constraint_check_uses_expression() {
        let tc = TableConstraint::Check {
            name: Some("ck_pos".to_string()),
            expr: Expression::Comparison {
                left: Box::new(Expression::Identifier(ident("total"))),
                op: common_sql::ast::ComparisonOperator::Ge,
                right: Box::new(Expression::Literal(Literal::Integer(0))),
            },
        };
        let rendered = render_table_constraint(&mut emitter(), &tc).unwrap();
        assert_eq!(rendered, "CONSTRAINT `ck_pos` CHECK (`total` >= 0)");
    }

    #[test]
    fn alter_action_drop_default_renders_null() {
        let action = AlterTableAction::AlterColumn {
            column: ident("c"),
            data_type: Some(common_sql::ast::DataType::Int),
            default: Some(None),
            nullable: None,
        };
        let rendered = render_alter_action(&mut emitter(), &action).unwrap();
        assert_eq!(rendered, "MODIFY COLUMN `c` INT DEFAULT NULL");
    }
}
