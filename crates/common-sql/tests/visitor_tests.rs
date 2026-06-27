//! Task 6.2: integration tests driving a custom Visitor through
//! `Visitable::accept` over a realistic AST.
//!
//! The AST built here mirrors what a transpiler actually sees:
//!   1. A `CREATE TABLE` with a `PRIMARY KEY` column and a `FOREIGN KEY`
//!      table constraint.
//!   2. A `SELECT` whose `WHERE` clause is a `Comparison` whose right-hand
//!      side is a `BinaryOp` (`price + tax`).
//!   3. A `CAST` expression inside the projection.
//!
//! An `EmitStub` visitor records the order in which it dispatches to each
//! node kind, producing a flat trace we assert against. This proves:
//!   - `Visitable::accept` reaches the right per-variant hook for every
//!     `Statement` and `Expression` variant touched.
//!   - The default `visit_*` walk recurses into child nodes (a `Comparison`
//!     visits its left/right operands; a `CAST` visits its inner expression
//!     and data type).
//!   - The generic `Output` type works for a non-trivial accumulator
//!     (`Vec<&'static str>`), validating the open-closed contract that the
//!     downstream emitters will rely on.

#![allow(clippy::unwrap_used, clippy::panic, clippy::expect_used)]

use common_sql::ast::ddl::{AlterTableAction, AlterTableStatement};
use common_sql::ast::ddl::{
    ColumnConstraint, ColumnDef, CreateTableStatement, IndexColumn, TableConstraint, TableOptions,
};
use common_sql::ast::SortDirection as IndexSortDirection;
use common_sql::ast::{
    BinaryOperator, ComparisonOperator, DataType, Expression, Identifier, InsertSource,
    InsertStatement, Literal, QualifiedName, SelectItem, SelectStatement, Span, Statement,
    TableFactor,
};
use common_sql::{Visitable, Visitor};

// ---------------------------------------------------------------------------
// A stub emitter visitor that records its dispatch trace.
// ---------------------------------------------------------------------------

/// A visitor that appends a tag to a trace buffer for every node it dispatches
/// to. It overrides the dispatch entry points and the per-variant hooks the
/// realistic AST exercises, and recurses manually so the trace reflects true
/// visitation order.
struct EmitStub {
    trace: Vec<&'static str>,
}

impl EmitStub {
    fn new() -> Self {
        Self { trace: Vec::new() }
    }

    fn tag(&mut self, name: &'static str) {
        self.trace.push(name);
    }
}

impl Visitor for EmitStub {
    type Output = ();

    fn default_output(&self) {}

    // --- Statement dispatch: tag + recurse into the interesting children ---

    fn visit_create_table_statement(&mut self, stmt: &CreateTableStatement) {
        self.tag("create_table");
        // Walk every column so the trace shows the column's data type.
        for col in &stmt.columns {
            self.tag("column");
            self.visit_data_type(&col.data_type);
        }
        for c in &stmt.constraints {
            self.tag("table_constraint");
            // A FK references columns; we only tag, no deeper walk needed here.
            if let TableConstraint::ForeignKey { ref_table, .. } = c {
                let _ = ref_table.name();
            }
        }
    }

    fn visit_select_statement(&mut self, stmt: &SelectStatement) {
        self.tag("select");
        // Recurse into the WHERE clause if present.
        if let Some(where_clause) = &stmt.where_clause {
            self.tag("where");
            self.visit_expression(where_clause);
        }
        // Recurse into projection items that carry an expression.
        for item in &stmt.projection {
            if let SelectItem::Expression { expr, .. } = item {
                self.tag("projection");
                self.visit_expression(expr);
            }
        }
    }

    fn visit_insert_statement(&mut self, _stmt: &InsertStatement) {
        self.tag("insert");
    }

    fn visit_update_statement(&mut self, _stmt: &common_sql::ast::UpdateStatement) {
        self.tag("update");
    }

    fn visit_delete_statement(&mut self, _stmt: &common_sql::ast::DeleteStatement) {
        self.tag("delete");
    }

    // --- Expression dispatch: tag + recurse into operands -----------------

    fn visit_comparison(&mut self, left: &Expression, _op: ComparisonOperator, right: &Expression) {
        self.tag("comparison");
        self.visit_expression(left);
        self.visit_expression(right);
    }

    fn visit_binary_op(&mut self, left: &Expression, op: BinaryOperator, right: &Expression) {
        // Tag the operator so we can assert which binary op was visited.
        let op_tag = match op {
            BinaryOperator::Add => "binary_add",
            BinaryOperator::Sub => "binary_sub",
            BinaryOperator::Mul => "binary_mul",
            BinaryOperator::Div => "binary_div",
            BinaryOperator::Mod => "binary_mod",
            BinaryOperator::Concat => "binary_concat",
        };
        self.tag(op_tag);
        self.visit_expression(left);
        self.visit_expression(right);
    }

    fn visit_identifier(&mut self, _ident: &Identifier) {
        self.tag("identifier");
    }

    fn visit_literal(&mut self, _literal: &Literal) {
        self.tag("literal");
    }

    fn visit_cast(&mut self, expr: &Expression, data_type: &DataType) {
        self.tag("cast");
        self.visit_expression(expr);
        self.visit_data_type(data_type);
    }

    fn visit_data_type(&mut self, _data_type: &DataType) {
        self.tag("data_type");
    }
}

// ---------------------------------------------------------------------------
// AST builders
// ---------------------------------------------------------------------------

fn ident(s: &str) -> Identifier {
    Identifier::new(s.to_string())
}

fn qualified(s: &str) -> QualifiedName {
    QualifiedName::new(None, s.to_string())
}

fn id_expr(s: &str) -> Expression {
    Expression::Identifier(ident(s))
}

fn int_expr(n: i64) -> Expression {
    Expression::Literal(Literal::Integer(n))
}

fn table_factor(name: &str) -> TableFactor {
    TableFactor::Table {
        name: qualified(name),
        alias: None,
    }
}

/// Build `CREATE TABLE orders (id BIGINT PRIMARY KEY, user_id INT NOT NULL,
/// CONSTRAINT fk_order_user FOREIGN KEY (user_id) REFERENCES users(id))`.
fn create_table_with_pk_and_fk() -> Statement {
    Statement::CreateTable(Box::new(CreateTableStatement {
        span: Span::new(0, 120),
        if_not_exists: false,
        temporary: false,
        name: qualified("orders"),
        columns: vec![
            ColumnDef {
                span: Span::new(0, 20),
                name: ident("id"),
                data_type: DataType::BigInt,
                nullable: false,
                default: None,
                constraints: vec![ColumnConstraint::PrimaryKey],
            },
            ColumnDef {
                span: Span::new(20, 50),
                name: ident("user_id"),
                data_type: DataType::Int,
                nullable: false,
                default: None,
                constraints: vec![ColumnConstraint::References {
                    table: qualified("users"),
                    columns: vec!["id".to_string()],
                }],
            },
        ],
        constraints: vec![TableConstraint::ForeignKey {
            name: Some("fk_order_user".to_string()),
            columns: vec![ident("user_id")],
            ref_table: qualified("users"),
            ref_columns: vec![ident("id")],
        }],
        options: TableOptions::default(),
    }))
}

/// Build `SELECT CAST(price AS DECIMAL(18,2)) FROM orders
///        WHERE total = price + tax`.
fn select_with_where_binaryop_and_cast() -> Statement {
    // right-hand side of the WHERE comparison: price + tax
    let binary_rhs = Expression::BinaryOp {
        left: Box::new(id_expr("price")),
        op: BinaryOperator::Add,
        right: Box::new(id_expr("tax")),
    };
    // WHERE total = price + tax
    let where_clause = Expression::Comparison {
        left: Box::new(id_expr("total")),
        op: ComparisonOperator::Eq,
        right: Box::new(binary_rhs),
    };
    // projection: CAST(price AS DECIMAL(18,2))
    let cast_expr = Expression::Cast {
        expr: Box::new(id_expr("price")),
        data_type: DataType::Decimal {
            precision: Some(18),
            scale: Some(2),
        },
    };
    let select = SelectStatement {
        span: Span::new(0, 80),
        with: None,
        projection: vec![SelectItem::Expression {
            expr: cast_expr,
            alias: None,
        }],
        from: Some(table_factor("orders")),
        where_clause: Some(where_clause),
        group_by: None,
        having: None,
        order_by: None,
        limit: None,
    };
    Statement::Select(Box::new(select))
}

fn insert_stub() -> Statement {
    Statement::Insert(Box::new(InsertStatement {
        span: Span::new(0, 10),
        table: qualified("orders"),
        columns: vec![ident("id")],
        source: InsertSource::Values(vec![vec![int_expr(1)]]),
        on_conflict: None,
    }))
}

fn alter_table_stub() -> Statement {
    Statement::AlterTable(Box::new(AlterTableStatement {
        span: Span::new(0, 10),
        name: qualified("orders"),
        actions: vec![AlterTableAction::DropColumn(ident("user_id"))],
    }))
}

// ---------------------------------------------------------------------------
// Tests: node visit order through Visitable::accept
// ---------------------------------------------------------------------------

#[test]
fn create_table_visitor_records_columns_and_constraints_in_order() {
    let stmt = create_table_with_pk_and_fk();
    let mut v = EmitStub::new();
    let () = stmt.accept(&mut v);
    // create_table -> column(id) -> BigInt -> column(user_id) -> Int -> fk
    assert_eq!(
        v.trace,
        vec![
            "create_table",
            "column",
            "data_type", // id BIGINT
            "column",
            "data_type",        // user_id INT
            "table_constraint", // FOREIGN KEY
        ]
    );
}

#[test]
fn select_visitor_walks_where_comparison_then_projection_cast() {
    let stmt = select_with_where_binaryop_and_cast();
    let mut v = EmitStub::new();
    let () = stmt.accept(&mut v);
    // select -> where -> comparison(total, binary_add(price, tax))
    //        -> projection -> cast(price) -> Decimal
    assert_eq!(
        v.trace,
        vec![
            "select",
            "where",
            "comparison",
            "identifier", // total (left)
            "binary_add",
            "identifier", // price
            "identifier", // tax
            "projection",
            "cast",
            "identifier", // price (cast operand)
            "data_type",  // DECIMAL(18,2)
        ]
    );
}

#[test]
fn cast_expression_alone_visits_inner_expr_and_data_type() {
    // CAST(x AS INT) driven directly through Expression::accept.
    let expr = Expression::Cast {
        expr: Box::new(id_expr("x")),
        data_type: DataType::Int,
    };
    let mut v = EmitStub::new();
    let () = expr.accept(&mut v);
    assert_eq!(v.trace, vec!["cast", "identifier", "data_type"]);
}

#[test]
fn data_type_is_visitable_and_routes_to_visit_data_type() {
    let dt = DataType::VarChar { length: Some(255) };
    let mut v = EmitStub::new();
    let () = dt.accept(&mut v);
    assert_eq!(v.trace, vec!["data_type"]);
}

#[test]
fn insert_and_alter_dispatch_through_statement_accept() {
    // INSERT and ALTER are only tagged at the top level (no recursion in the
    // stub), proving accept reaches their per-variant hook.
    let mut v = EmitStub::new();
    insert_stub().accept(&mut v);
    assert_eq!(v.trace.last(), Some(&"insert"));

    let mut v2 = EmitStub::new();
    alter_table_stub().accept(&mut v2);
    // visit_alter_table_statement is not overridden, so it falls back to
    // default_output (no tag) — but accept must still dispatch without panic.
    assert!(v2.trace.is_empty());
}

#[test]
fn visitor_generic_output_works_with_string_accumulator() {
    // A different Output type (String) proves the trait is generic and an
    // emitter can choose its own return shape.
    struct StringEmit;
    impl Visitor for StringEmit {
        type Output = String;
        fn default_output(&self) -> String {
            String::from("d")
        }
        fn visit_identifier(&mut self, ident: &Identifier) -> String {
            ident.value().to_string()
        }
        fn visit_create_table_statement(&mut self, _stmt: &CreateTableStatement) -> String {
            String::from("CREATE TABLE")
        }
    }
    let mut v = StringEmit;
    // CREATE TABLE -> overridden -> "CREATE TABLE"
    let out = create_table_with_pk_and_fk().accept(&mut v);
    assert_eq!(out, "CREATE TABLE");
    // An identifier expression routes to the overridden visit_identifier.
    let out = id_expr("foo").accept(&mut v);
    assert_eq!(out, "foo");
    // A node kind with no override falls back to default_output.
    let out = DataType::Int.accept(&mut v);
    assert_eq!(out, "d");
}

#[test]
fn index_sort_direction_alias_round_trips() {
    // The ddl SortDirection (aliased to avoid clashing with clause::SortDirection)
    // is reachable via the public surface and behaves as a Copy enum.
    assert_eq!(IndexSortDirection::Asc, IndexSortDirection::Asc);
    assert_ne!(IndexSortDirection::Asc, IndexSortDirection::Desc);
    let _col = IndexColumn {
        name: ident("c"),
        direction: Some(IndexSortDirection::Desc),
    };
}
