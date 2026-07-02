//! PostgreSQL Emitter 統合テスト
//!
//! Common SQL AST → PostgreSQL SQL 変換のテスト。
//!
//! 設計決定 DD-3 に基づき、共通 AST は本テスト内で直接構築する
//! (postgresql-emitter は common-sql のみに依存し tsql-* への直接依存を持たないため、
//! またブリッジの DDL/方言ギャップをテスト前提にしたくないため)。
//! E2E (T-SQL parse → common-sql → PostgreSQL) は別途ブリッジ網羅時に追加する
//! (mysql-emitter と同一方針)。
//!
//! なお、旧テストのうち DialectSpecific 系 (DECLARE / SET 変数宣言・代入の
//! 警告コメント出力) は common-sql AST に対応バリアントが存在せず、ブリッジを
//! 介さなければ構築不能なため本ファイルでは扱わない。これらの T-SQL→PL/pgSQL
//! コメント生成機能は別 Issue で追跡する。

#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]
#![allow(clippy::expect_used)]

use common_sql::ast::clause::{LimitClause, OrderByClause, OrderByItem, SortDirection};
use common_sql::ast::identifier::{Identifier, QualifiedName, TableAlias};
use common_sql::ast::literal::Literal;
use common_sql::ast::{
    Assignment, ComparisonOperator, DeleteStatement, Expression, InList, InsertSource,
    InsertStatement, SelectItem, SelectStatement, Statement, TableFactor, UpdateStatement,
};
use postgresql_emitter::{EmissionConfig, PostgreSqlEmitter};

// ---------------------------------------------------------------------------
// AST 構築ヘルパ (DD-3: テスト内で common_sql::ast を直接構築)
// ---------------------------------------------------------------------------

fn emitter(config: EmissionConfig) -> PostgreSqlEmitter {
    PostgreSqlEmitter::new(config)
}

fn ident(name: &str) -> Identifier {
    Identifier::new(name.to_string())
}

fn id_expr(name: &str) -> Expression {
    Expression::Identifier(ident(name))
}

/// `table.column` 形式の修飾識別子式
fn qualified_expr(table: &str, column: &str) -> Expression {
    Expression::QualifiedIdentifier {
        table: ident(table),
        column: ident(column),
    }
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

/// エイリアス付き派生テーブル (FROM 句サブクエリ)
fn derived_table(subquery: SelectStatement, alias: &str) -> TableFactor {
    TableFactor::Derived {
        subquery: Box::new(subquery),
        alias: Some(TableAlias::new(alias.to_string(), vec![])),
    }
}

/// 式 + エイリアスの SELECT 項目
fn aliased_expr(expr: Expression, alias: &str) -> SelectItem {
    SelectItem::Expression {
        expr,
        alias: Some(ident(alias)),
    }
}

fn span() -> common_sql::ast::Span {
    common_sql::ast::Span::new(0, 0)
}

/// `expr = value` の比較式
fn eq_cmp(left: Expression, right: Expression) -> Expression {
    Expression::Comparison {
        left: Box::new(left),
        op: ComparisonOperator::Eq,
        right: Box::new(right),
    }
}

// ---------------------------------------------------------------------------
// SELECT 系テスト
// ---------------------------------------------------------------------------

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
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert_eq!(pg_sql, "SELECT * FROM users");
}

/// WHERE 句付き SELECT の発行テスト (比較式は括弧で括られる)
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
        where_clause: Some(eq_cmp(id_expr("id"), int_expr(1))),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    // Binary operations are wrapped in parentheses for proper precedence
    assert!(pg_sql.contains("SELECT id"));
    assert!(pg_sql.contains("FROM users"));
    assert!(pg_sql.contains("WHERE (id = 1)"));
}

/// ORDER BY 句付き SELECT の発行テスト ("name" は予約語なのでクォートされる)
#[test]
fn emit_select_with_order_by() {
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("users")),
        where_clause: None,
        group_by: None,
        having: None,
        order_by: Some(OrderByClause {
            span: span(),
            items: vec![OrderByItem {
                expr: id_expr("name"),
                direction: Some(SortDirection::Asc),
                nulls: None,
            }],
        }),
        limit: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert_eq!(pg_sql, "SELECT * FROM users ORDER BY \"name\" ASC");
}

/// LIMIT 句付き SELECT の発行テスト (T-SQL TOP n は上流で LIMIT に変換済み)
#[test]
fn emit_select_with_limit() {
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("users")),
        where_clause: None,
        group_by: None,
        having: None,
        order_by: None,
        limit: Some(LimitClause {
            span: span(),
            limit: int_expr(10),
            offset: None,
        }),
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    // TOP 10 becomes LIMIT 10 in PostgreSQL
    assert!(pg_sql.contains("SELECT"));
    assert!(pg_sql.contains("FROM users"));
    assert!(pg_sql.contains("LIMIT 10"));
}

// ---------------------------------------------------------------------------
// DML 系テスト (INSERT / UPDATE / DELETE)
// ---------------------------------------------------------------------------

/// INSERT 文の発行テスト ("name" は PostgreSQL 予約語なのでクォートされる)
#[test]
fn emit_insert_values() {
    let stmt = InsertStatement {
        span: span(),
        table: QualifiedName::new(None, "users".to_string()),
        columns: vec![ident("id"), ident("name")],
        source: InsertSource::Values(vec![vec![int_expr(1), str_expr("test")]]),
        on_conflict: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Insert(Box::new(stmt)))
        .unwrap();
    assert_eq!(
        pg_sql,
        "INSERT INTO users (id, \"name\") VALUES (1, 'test')"
    );
}

/// UPDATE 文の発行テスト
#[test]
fn emit_update() {
    let stmt = UpdateStatement {
        span: span(),
        table: table("users"),
        assignments: vec![Assignment {
            column: ident("name"),
            value: str_expr("updated"),
        }],
        from: None,
        where_clause: Some(eq_cmp(id_expr("id"), int_expr(1))),
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Update(Box::new(stmt)))
        .unwrap();
    // "name" is a PostgreSQL reserved keyword, so it's quoted;
    // comparison expressions are parenthesized.
    assert_eq!(
        pg_sql,
        "UPDATE users SET \"name\" = 'updated' WHERE (id = 1)"
    );
}

/// DELETE 文の発行テスト
#[test]
fn emit_delete() {
    let stmt = DeleteStatement {
        span: span(),
        table: table("users"),
        using: None,
        where_clause: Some(eq_cmp(id_expr("id"), int_expr(1))),
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Delete(Box::new(stmt)))
        .unwrap();
    // Binary operations have parentheses
    assert_eq!(pg_sql, "DELETE FROM users WHERE (id = 1)");
}

// ---------------------------------------------------------------------------
// バッチ発行テスト
// ---------------------------------------------------------------------------

/// emit_batch で複数ステートメントをセミコロン区切りで発行
#[test]
fn emit_batch() {
    let s1 = Statement::Select(Box::new(SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("users")),
        where_clause: None,
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    }));
    let s2 = Statement::Select(Box::new(SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("orders")),
        where_clause: None,
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    }));
    let pg_sql = emitter(EmissionConfig::default())
        .emit_batch(&[s1, s2])
        .unwrap();
    assert!(pg_sql.contains("SELECT * FROM users"));
    assert!(pg_sql.contains("SELECT * FROM orders"));
    assert!(pg_sql.contains(";\n"));
}

// ---------------------------------------------------------------------------
// 識別子クォーティングテスト
// ---------------------------------------------------------------------------

/// 識別子クォート有効時、先頭大文字の "Users" はクォートされる
#[test]
fn emit_with_quoted_identifiers() {
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("Users")),
        where_clause: None,
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let config = EmissionConfig {
        quote_identifiers: true,
        uppercase_keywords: false,
        indent_size: 4,
        warn_unsupported: true,
    };
    let pg_sql = emitter(config)
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    // "Users" should be quoted as it starts with uppercase
    assert!(pg_sql.contains("\"Users\""));
}

/// 識別子クォート無効時、テーブル名はクォートされずそのまま出力
#[test]
fn emit_without_quoted_identifiers() {
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
    let config = EmissionConfig {
        quote_identifiers: false,
        uppercase_keywords: false,
        indent_size: 4,
        warn_unsupported: true,
    };
    let pg_sql = emitter(config)
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert_eq!(pg_sql, "SELECT * FROM users");
}

// ---------------------------------------------------------------------------
// サブクエリ系テスト (IN / NOT IN / EXISTS / NOT EXISTS / スカラー / 派生テーブル / 入れ子)
// ---------------------------------------------------------------------------

/// IN サブクエリ: WHERE customer_id IN (SELECT id FROM customers WHERE active = 1)
#[test]
fn emit_in_subquery() {
    let inner = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Expression {
            expr: id_expr("id"),
            alias: None,
        }],
        from: Some(table("customers")),
        where_clause: Some(eq_cmp(id_expr("active"), int_expr(1))),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("orders")),
        where_clause: Some(Expression::In {
            expr: Box::new(id_expr("customer_id")),
            list: InList::Subquery(Box::new(inner)),
            negated: false,
        }),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert!(pg_sql.contains("SELECT"));
    assert!(pg_sql.contains("FROM orders"));
    assert!(pg_sql.contains("customer_id IN (SELECT id"));
    assert!(pg_sql.contains("FROM customers"));
    assert!(pg_sql.contains("active"));
}

/// NOT IN サブクエリ: WHERE customer_id NOT IN (SELECT id FROM blocked_customers)
#[test]
fn emit_not_in_subquery() {
    let inner = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Expression {
            expr: id_expr("id"),
            alias: None,
        }],
        from: Some(table("blocked_customers")),
        where_clause: None,
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("orders")),
        where_clause: Some(Expression::In {
            expr: Box::new(id_expr("customer_id")),
            list: InList::Subquery(Box::new(inner)),
            negated: true,
        }),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert!(pg_sql.contains("customer_id NOT IN (SELECT id"));
    assert!(pg_sql.contains("FROM blocked_customers"));
}

/// EXISTS サブクエリ: WHERE EXISTS (SELECT 1 FROM orders WHERE ...)
#[test]
fn emit_exists_subquery() {
    let inner = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Expression {
            expr: int_expr(1),
            alias: None,
        }],
        from: Some(table("orders")),
        where_clause: Some(eq_cmp(
            qualified_expr("orders", "customer_id"),
            qualified_expr("customers", "id"),
        )),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("customers")),
        where_clause: Some(Expression::Exists {
            subquery: Box::new(inner),
            negated: false,
        }),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert!(pg_sql.contains("EXISTS (SELECT 1"));
    assert!(pg_sql.contains("FROM orders"));
    assert!(pg_sql.contains("orders.customer_id = customers.id"));
}

/// NOT EXISTS サブクエリ: WHERE NOT EXISTS (SELECT 1 FROM orders WHERE ...)
#[test]
fn emit_not_exists_subquery() {
    let inner = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Expression {
            expr: int_expr(1),
            alias: None,
        }],
        from: Some(table("orders")),
        where_clause: Some(eq_cmp(
            qualified_expr("orders", "customer_id"),
            qualified_expr("customers", "id"),
        )),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("customers")),
        where_clause: Some(Expression::Exists {
            subquery: Box::new(inner),
            negated: true,
        }),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert!(pg_sql.contains("NOT EXISTS (SELECT 1"));
}

/// スカラーサブクエリ (SELECT リスト内): (SELECT COUNT(*) ...) AS order_count
#[test]
fn emit_scalar_subquery() {
    let count_sub = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Expression {
            expr: Expression::Function {
                name: ident("COUNT"),
                // common_sql::ast に Expression::Wildcard は存在しないため、
                // COUNT(*) の * は識別子 "*" で表現する (mapper が "*" を特別扱いする)。
                args: vec![id_expr("*")],
                distinct: false,
            },
            alias: None,
        }],
        from: Some(table("orders")),
        where_clause: Some(eq_cmp(
            qualified_expr("orders", "customer_id"),
            qualified_expr("customers", "id"),
        )),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![
            SelectItem::Expression {
                expr: id_expr("id"),
                alias: None,
            },
            aliased_expr(Expression::Subquery(Box::new(count_sub)), "order_count"),
        ],
        from: Some(table("customers")),
        where_clause: None,
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert!(pg_sql.contains("SELECT id"));
    assert!(pg_sql.contains("SELECT COUNT(*)"));
    assert!(pg_sql.contains("AS order_count"));
}

/// FROM 句の派生テーブル (サブクエリ): FROM (SELECT ...) AS active_users
#[test]
fn emit_derived_table() {
    let inner = SelectStatement {
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
        where_clause: Some(eq_cmp(id_expr("active"), int_expr(1))),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(derived_table(inner, "active_users")),
        where_clause: None,
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert!(pg_sql.contains("SELECT * FROM (SELECT id"));
    assert!(pg_sql.contains("AS active_users"));
}

/// 入れ子のサブクエリ: IN (SELECT ... WHERE region_id IN (SELECT ...))
#[test]
fn emit_nested_subquery() {
    let innermost = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Expression {
            expr: id_expr("id"),
            alias: None,
        }],
        from: Some(table("regions")),
        where_clause: Some(eq_cmp(id_expr("country"), str_expr("USA"))),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let middle = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Expression {
            expr: id_expr("id"),
            alias: None,
        }],
        from: Some(table("customers")),
        where_clause: Some(Expression::In {
            expr: Box::new(id_expr("region_id")),
            list: InList::Subquery(Box::new(innermost)),
            negated: false,
        }),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let stmt = SelectStatement {
        span: span(),
        with: None,
        projection: vec![SelectItem::Wildcard],
        from: Some(table("orders")),
        where_clause: Some(Expression::In {
            expr: Box::new(id_expr("customer_id")),
            list: InList::Subquery(Box::new(middle)),
            negated: false,
        }),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    let pg_sql = emitter(EmissionConfig::default())
        .emit(&Statement::Select(Box::new(stmt)))
        .unwrap();
    assert!(pg_sql.contains("customer_id IN (SELECT id FROM customers"));
    assert!(pg_sql.contains("region_id IN (SELECT id FROM regions"));
}
