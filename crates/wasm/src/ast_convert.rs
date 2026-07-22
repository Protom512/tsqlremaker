//! Non-WASM-gated AST-to-JS conversion helpers (Issue #61).
//!
//! このモジュールは `ast_js.rs` の `TryFrom<Statement> for JsStatement` から
//! 変換ロジックを切り出した純粋関数群である。**wasm feature の背後に隠さない**ことが
//! このモジュールの存在理由であり、`cargo nextest run` (default features) で
//! 直接テストできる (プロジェクトルール: nextest が source of truth)。
//!
//! T1 では以下の 4 つの public API を定義する (T2 で ast_js.rs の TryFrom が delegate):
//! - [`select_to_js`]: `(&SelectStatement) -> (Vec<String>, Option<String>)`
//! - [`table_ref_to_name`]: `(&TableReference) -> Option<String>`
//! - [`expr_to_column_string`]: `(&Expression) -> String`
//! - [`create_to_js`]: `(&CreateStatement) -> (Option<String>, Option<String>)`
//!
//! # 簡略化ポリシー (simplified representation)
//!
//! 式 (`Expression`) の完全 fidelity 印字は **非スコープ** である。emitter 相当の
//! コード生成は L 規模タスクで別途扱う。本モジュールでは以下のみを印字する:
//! - [`Expression::Identifier`]
//! - [`Expression::ColumnReference`]
//! - [`Expression::Literal`]
//! - [`Expression::FunctionCall`]
//!
//! 上記以外 (`BinaryOp`, `Case`, `Subquery`, `In`, `Between`, `Like`, `Is`,
//! `Exists`, `UnaryOp`) は一律 [`EXPR_PLACEHOLDER`] (`"<expr>"`) にフォールバックする。
//! JS 側コンシューマはこのプレースホルダ文字列で決定論的にパターンマッチできる。

// T1 では純粋関数を定義するのみ。T2 で ast_js.rs の TryFrom がこれらを delegate するまで
// クレート外 (および非テストの lib ビルド) から呼ばれないため、一時的に dead_code を
// 許容する。テストからは全関数を実行しており、実質的なカバレッジ欠損はない。
#![allow(dead_code)]

use tsql_parser::ast::{
    CreateStatement, Expression, FunctionArg, Literal, SelectItem, SelectStatement, TableReference,
};

/// 式のフォールバック時に使うプレースホルダ文字列。
///
/// JS 側コンシューマはこの値で「複雑式 (簡略化対象外)」を判定できる。
pub const EXPR_PLACEHOLDER: &str = "<expr>";

/// [`SelectStatement`] を (columns, from_table_name) の簡略表現へ変換する。
///
/// - `columns`: 各 SELECT リストアイテムの簡略文字列。`*` → `"*"`、`t.*` → `"t.*"`、
///   式は [`expr_to_column_string`] で印字 (alias 指定時は alias 名を採用)。
/// - `from_table_name`: FROM 句先頭テーブル名。JOIN / サブクエリ / FROM 句なしの場合は
///   `None` (簡略表現の非スコープ — 完全 FROM/JWASM emitter は別途)。
#[must_use]
pub fn select_to_js(stmt: &SelectStatement) -> (Vec<String>, Option<String>) {
    let columns = stmt
        .columns
        .iter()
        .map(select_item_to_column)
        .collect::<Vec<_>>();
    let from = stmt
        .from
        .as_ref()
        .and_then(|from_clause| from_clause.tables.first())
        .and_then(table_ref_to_name);
    (columns, from)
}

/// [`SelectItem`] を JS 向けカラム文字列に変換する。
fn select_item_to_column(item: &SelectItem) -> String {
    match item {
        SelectItem::Wildcard => "*".to_string(),
        SelectItem::QualifiedWildcard(ident) => format!("{}.*", ident.name),
        SelectItem::Expression(expr, alias) => {
            if let Some(a) = alias {
                a.name.clone()
            } else {
                expr_to_column_string(expr)
            }
        }
    }
}

/// [`TableReference`] からテーブル名を抽出する。
///
/// `Table` variant のみ `Some(name)` を返す。`Subquery` / `Joined` は簡略表現の
/// 対象外 (単一テーブル名で表せない) ため `None` を返す (graceful fallback)。
#[must_use]
pub fn table_ref_to_name(table_ref: &TableReference) -> Option<String> {
    match table_ref {
        TableReference::Table { name, .. } => Some(name.name.clone()),
        TableReference::Subquery { .. } | TableReference::Joined { .. } => None,
    }
}

/// [`Expression`] を JS 向けカラム文字列に変換する (simplified)。
///
/// 対応: Identifier / ColumnReference / Literal / FunctionCall。
/// それ以外 (BinaryOp / UnaryOp / Case / Subquery / In / Between / Like / Is / Exists) は
/// 一律 [`EXPR_PLACEHOLDER`] (`"<expr>"`) にフォールバックする (完全 emitter は非スコープ)。
#[must_use]
pub fn expr_to_column_string(expr: &Expression) -> String {
    match expr {
        Expression::Identifier(ident) => ident.name.clone(),
        Expression::ColumnReference(col_ref) => {
            if let Some(table) = &col_ref.table {
                format!("{}.{}", table.name, col_ref.column.name)
            } else {
                col_ref.column.name.clone()
            }
        }
        Expression::Literal(lit) => literal_to_string(lit),
        Expression::FunctionCall(call) => {
            let args = call
                .args
                .iter()
                .map(function_arg_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", call.name.name, args)
        }
        // BinaryOp / Case / Subquery / In / Between / Like / Is / Exists / UnaryOp
        // → 完全 emitter は非スコープ。決定論的プレースホルダで逃げる。
        Expression::UnaryOp { .. }
        | Expression::BinaryOp { .. }
        | Expression::Case(_)
        | Expression::Subquery(_)
        | Expression::Exists(_)
        | Expression::In { .. }
        | Expression::Between { .. }
        | Expression::Like { .. }
        | Expression::Is { .. } => EXPR_PLACEHOLDER.to_string(),
    }
}

/// [`Literal`] を文字列へ (simplified)。
fn literal_to_string(lit: &Literal) -> String {
    match lit {
        Literal::String(s, _) => s.clone(),
        Literal::Number(n, _) => n.clone(),
        Literal::Float(f, _) => f.clone(),
        Literal::Hex(h, _) => h.clone(),
        Literal::Null(_) => "NULL".to_string(),
        Literal::Boolean(b, _) => {
            if *b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
    }
}

/// [`FunctionArg`] を文字列へ (simplified)。
fn function_arg_to_string(arg: &FunctionArg) -> String {
    match arg {
        FunctionArg::Expression(e) => expr_to_column_string(e),
        FunctionArg::QualifiedWildcard(ident) => format!("{}.*", ident.name),
        FunctionArg::Wildcard => "*".to_string(),
    }
}

/// [`CreateStatement`] を (object_type, name) の簡略表現へ変換する。
///
/// 各バリアントの object_type と定義名を返す:
/// - `Table` → `("TABLE", name)`
/// - `Index` → `("INDEX", name)`
/// - `View` → `("VIEW", name)`
/// - `Procedure` → `("PROCEDURE", name)`
/// - `Trigger` → `(None, None)` (Trigger は `JsStatement::Trigger` にマップされるため、
///   本関数の呼び出し元で別経路として処理される。`create_to_js` は「Create ではない」
///   ことを `(None, None)` で signal する)
#[must_use]
pub fn create_to_js(stmt: &CreateStatement) -> (Option<String>, Option<String>) {
    match stmt {
        CreateStatement::Table(d) => (Some("TABLE".to_string()), Some(d.name.name.clone())),
        CreateStatement::Index(d) => (Some("INDEX".to_string()), Some(d.name.name.clone())),
        CreateStatement::View(d) => (Some("VIEW".to_string()), Some(d.name.name.clone())),
        CreateStatement::Procedure(d) => (Some("PROCEDURE".to_string()), Some(d.name.name.clone())),
        // Trigger は JsStatement::Trigger にマップされるため、create_to_js では
        // (None, None) を返して呼び出し元に別経路を促す。
        CreateStatement::Trigger(_) => (None, None),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use tsql_parser::ast::{FromClause, Identifier, Statement};
    use tsql_token::Span;

    /// SELECT a, b FROM t → columns=["a","b"], from=Some("t")
    #[test]
    fn test_select_column_list() {
        let stmts = tsql_parser::parse("SELECT a, b FROM t").unwrap();
        let select = expect_single_select(&stmts);
        let (columns, from) = select_to_js(select);
        assert_eq!(columns, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(from.as_deref(), Some("t"));
    }

    /// SELECT * FROM t → columns=["*"], from=Some("t")
    #[test]
    fn test_select_wildcard() {
        let stmts = tsql_parser::parse("SELECT * FROM t").unwrap();
        let select = expect_single_select(&stmts);
        let (columns, from) = select_to_js(select);
        assert_eq!(columns, vec!["*".to_string()]);
        assert_eq!(from.as_deref(), Some("t"));
    }

    /// SELECT t.* FROM t → columns contains "t.*"
    #[test]
    fn test_select_qualified_wildcard() {
        let stmts = tsql_parser::parse("SELECT t.* FROM t").unwrap();
        let select = expect_single_select(&stmts);
        let (columns, _from) = select_to_js(select);
        assert!(
            columns.iter().any(|c| c == "t.*"),
            "expected qualified wildcard t.* in columns, got {columns:?}"
        );
    }

    /// SELECT a AS x FROM t → alias 採用で columns contains "x"
    #[test]
    fn test_select_with_alias() {
        let stmts = tsql_parser::parse("SELECT a AS x FROM t").unwrap();
        let select = expect_single_select(&stmts);
        let (columns, _from) = select_to_js(select);
        assert!(
            columns.iter().any(|c| c == "x"),
            "expected alias 'x' in columns, got {columns:?}"
        );
    }

    /// SELECT * FROM (SELECT 1) sub → Subquery は graceful None
    #[test]
    fn test_select_from_subquery_graceful_none() {
        let stmts = tsql_parser::parse("SELECT * FROM (SELECT 1 AS x) sub").unwrap();
        let select = expect_single_select(&stmts);
        let (_columns, from) = select_to_js(select);
        assert!(
            from.is_none(),
            "subquery FROM must produce None, got {from:?}"
        );
    }

    /// SELECT * FROM a JOIN b ON 1=1 → 先頭テーブル a を返す (JOIN 相手 b は無視)
    ///
    /// Parser は `FROM a JOIN b` を `from.tables[0]=Table(a)` + `from.joins=[...]` として
    /// 表現する (Joined variant ではない)。そのため select_to_js は先頭テーブル名 a を
    /// 返す。Joined variant になるのはカッコ付きの結合表現のみで、それは別経路で None。
    #[test]
    fn test_select_from_join_returns_first_table() {
        let stmts = tsql_parser::parse("SELECT * FROM a JOIN b ON 1 = 1").unwrap();
        let select = expect_single_select(&stmts);
        let (_columns, from) = select_to_js(select);
        assert_eq!(
            from.as_deref(),
            Some("a"),
            "FROM a JOIN b must return first table 'a', got {from:?}"
        );
    }

    /// CREATE TABLE name + object_type 抽出
    #[test]
    fn test_create_to_js_table() {
        let stmts = tsql_parser::parse("CREATE TABLE t (id INT)").unwrap();
        let stmt = stmts.first().expect("at least one statement");
        let create = expect_create(stmt);
        let (object_type, name) = create_to_js(create);
        assert_eq!(object_type.as_deref(), Some("TABLE"));
        assert_eq!(name.as_deref(), Some("t"));
    }

    /// CREATE INDEX name + object_type 抽出
    #[test]
    fn test_create_to_js_index() {
        let stmts = tsql_parser::parse("CREATE INDEX idx ON t (c)").unwrap();
        let stmt = stmts.first().expect("at least one statement");
        let create = expect_create(stmt);
        let (object_type, name) = create_to_js(create);
        assert_eq!(object_type.as_deref(), Some("INDEX"));
        assert_eq!(name.as_deref(), Some("idx"));
    }

    /// CREATE VIEW name + object_type 抽出
    #[test]
    fn test_create_to_js_view() {
        let stmts = tsql_parser::parse("CREATE VIEW v AS SELECT 1 AS x").unwrap();
        let stmt = stmts.first().expect("at least one statement");
        let create = expect_create(stmt);
        let (object_type, name) = create_to_js(create);
        assert_eq!(object_type.as_deref(), Some("VIEW"));
        assert_eq!(name.as_deref(), Some("v"));
    }

    /// CREATE PROCEDURE name + object_type 抽出
    #[test]
    fn test_create_to_js_procedure() {
        let stmts = tsql_parser::parse("CREATE PROCEDURE p AS BEGIN RETURN END").unwrap();
        let stmt = stmts.first().expect("at least one statement");
        let create = expect_create(stmt);
        let (object_type, name) = create_to_js(create);
        assert_eq!(object_type.as_deref(), Some("PROCEDURE"));
        assert_eq!(name.as_deref(), Some("p"));
    }

    /// CREATE TRIGGER → (None, None) (JsStatement::Trigger への別経路 signal)
    #[test]
    fn test_create_to_js_trigger_returns_none_none() {
        let stmts =
            tsql_parser::parse("CREATE TRIGGER tr ON t FOR INSERT AS BEGIN RETURN END").unwrap();
        let stmt = stmts.first().expect("at least one statement");
        let create = expect_create(stmt);
        let (object_type, name) = create_to_js(create);
        assert!(
            object_type.is_none(),
            "Trigger must signal (None, None), got object_type={object_type:?}"
        );
        assert!(
            name.is_none(),
            "Trigger must signal (None, None), got name={name:?}"
        );
    }

    // ===================== expr_to_column_string (直接 AST 構築) =====================

    #[test]
    fn test_expr_identifier_returns_name() {
        let expr = Expression::Identifier(Identifier {
            name: "user_id".to_string(),
            span: Span::new(0, 0),
        });
        assert_eq!(expr_to_column_string(&expr), "user_id");
    }

    #[test]
    fn test_expr_literal_null_returns_null_keyword() {
        let expr = Expression::Literal(Literal::Null(Span::new(0, 0)));
        assert_eq!(expr_to_column_string(&expr), "NULL");
    }

    #[test]
    fn test_expr_function_call_count_star() {
        use tsql_parser::ast::{FunctionArg, FunctionCall};
        let call = FunctionCall {
            name: Identifier {
                name: "COUNT".to_string(),
                span: Span::new(0, 0),
            },
            args: vec![FunctionArg::Wildcard],
            distinct: false,
            span: Span::new(0, 0),
        };
        let expr = Expression::FunctionCall(call);
        assert_eq!(expr_to_column_string(&expr), "COUNT(*)");
    }

    /// EDGE CASE: BinaryOp を含む SELECT item → "<expr>" プレースホルダフォールバック
    #[test]
    fn test_edge_binary_op_expression_falls_back_to_placeholder() {
        // SELECT a + b FROM t → "a + b" は BinaryOp なので "<expr>" になる
        let stmts = tsql_parser::parse("SELECT a + b FROM t").unwrap();
        let select = expect_single_select(&stmts);
        let (columns, _from) = select_to_js(select);
        assert!(
            columns.iter().any(|c| c == EXPR_PLACEHOLDER),
            "BinaryOp must fall back to {EXPR_PLACEHOLDER:?}, got {columns:?}"
        );
    }

    /// EDGE CASE: カラムリストが空の SELECT → columns=[] (純粋関数契約テスト)
    #[test]
    fn test_edge_empty_columns_vec() {
        let select = build_empty_columns_select();
        let (columns, _from) = select_to_js(&select);
        assert!(
            columns.is_empty(),
            "empty columns Vec must remain empty, got {columns:?}"
        );
    }

    /// EDGE CASE: FROM 句なしの SELECT → from=None
    #[test]
    fn test_edge_no_from_clause_returns_none() {
        let stmts = tsql_parser::parse("SELECT 1").unwrap();
        let select = expect_single_select(&stmts);
        let (_columns, from) = select_to_js(select);
        assert!(
            from.is_none(),
            "SELECT without FROM must produce None, got {from:?}"
        );
    }

    /// EDGE CASE: table_ref_to_name に Table variant を直接適用
    #[test]
    fn test_table_ref_to_name_table_variant_direct() {
        let table_ref = TableReference::Table {
            name: Identifier {
                name: "orders".to_string(),
                span: Span::new(0, 0),
            },
            alias: None,
            span: Span::new(0, 0),
        };
        assert_eq!(table_ref_to_name(&table_ref), Some("orders".to_string()));
    }

    /// EDGE CASE: alias 付き Table variant は名前のみ返す (alias 無視)
    #[test]
    fn test_table_ref_to_name_table_with_alias_ignores_alias() {
        let table_ref = TableReference::Table {
            name: Identifier {
                name: "orders".to_string(),
                span: Span::new(0, 0),
            },
            alias: Some(Identifier {
                name: "o".to_string(),
                span: Span::new(0, 0),
            }),
            span: Span::new(0, 0),
        };
        assert_eq!(table_ref_to_name(&table_ref), Some("orders".to_string()));
    }

    // ---- helpers ----

    fn expect_single_select(stmts: &[Statement]) -> &SelectStatement {
        let stmt = stmts.first().expect("at least one statement");
        match stmt {
            Statement::Select(s) => s.as_ref(),
            other => panic!("expected Select, got {other:?}"),
        }
    }

    fn expect_create(stmt: &Statement) -> &CreateStatement {
        match stmt {
            Statement::Create(c) => c.as_ref(),
            other => panic!("expected Create, got {other:?}"),
        }
    }

    fn build_empty_columns_select() -> SelectStatement {
        let table = TableReference::Table {
            name: Identifier {
                name: "t".to_string(),
                span: Span::new(0, 1),
            },
            alias: None,
            span: Span::new(0, 1),
        };
        SelectStatement {
            span: Span::new(0, 1),
            distinct: false,
            top: None,
            columns: vec![],
            from: Some(FromClause {
                tables: vec![table],
                joins: vec![],
            }),
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        }
    }
}
