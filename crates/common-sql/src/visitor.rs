//! Visitor pattern for the dialect-independent SQL AST.
//!
//! Provides a [`Visitor`] trait with default implementations for every AST
//! node kind (Statement, Expression, DataType), and a [`Visitable`] trait
//! implemented by the three root node types. Emitters (MySQL, PostgreSQL,
//! SQLite) implement `Visitor` and override only the methods they care about;
//! adding a new AST variant does not require touching existing visitors
//! (open-closed principle).
//!
//! The result type is intentionally generic (`type Output`) so each visitor
//! chooses its own return shape (e.g. `String`, `Result<String, E>`). When the
//! first downstream emitter lands, revisit whether `Output` should be pinned
//! to `Result<String, E>` — documented decision point, not a blocker now.

use crate::ast::datatype::DataType;
use crate::ast::ddl::{
    AlterTableStatement, CreateIndexStatement, CreateTableStatement, DropIndexStatement,
    DropTableStatement,
};
use crate::ast::expression::{
    BinaryOperator, ComparisonOperator, InList, LogicalOperator, UnaryOperator,
};
use crate::ast::identifier::Identifier;
use crate::ast::literal::Literal;
use crate::ast::statement::{
    DeleteStatement, InsertStatement, SelectStatement, Statement, UpdateStatement,
};
use crate::ast::Expression;

/// AST visitor.
///
/// Every method has a default implementation that returns
/// [`default_output`](Self::default_output), so a visitor only overrides the
/// node kinds it needs. The associated [`Output`](Self::Output) type is chosen
/// by the implementor.
///
/// `Visitor` is implemented for `&mut T`-style consumers: the trait is generic
/// over `Sized` self and methods take `&mut self` so visitors can accumulate
/// state (e.g. a `String` buffer) across nodes.
///
/// **Default methods are dispatch-only**: each `visit_*` hook routes a node to
/// its matching override but does NOT descend into child nodes — e.g.
/// [`visit_create_table_statement`](Self::visit_create_table_statement) does
/// not walk `columns`/`constraints`, and
/// [`visit_comparison`](Self::visit_comparison) does not recurse into
/// `left`/`right`. An emitter that needs traversal must override the hook and
/// call [`visit_expression`](Self::visit_expression) /
/// [`visit_data_type`](Self::visit_data_type) itself. Centralized `walk_*`
/// helpers may be added when the first downstream emitter lands.
pub trait Visitor: Sized {
    /// The value produced by visiting a node.
    type Output;

    /// The default value returned when a visitor does not override a node's
    /// `visit_*` method.
    fn default_output(&self) -> Self::Output;

    /// Dispatch a [`Statement`] to the matching `visit_*_statement` hook.
    fn visit_statement(&mut self, stmt: &Statement) -> Self::Output {
        match stmt {
            Statement::Select(s) => self.visit_select_statement(s),
            Statement::Insert(s) => self.visit_insert_statement(s),
            Statement::Update(s) => self.visit_update_statement(s),
            Statement::Delete(s) => self.visit_delete_statement(s),
            Statement::CreateTable(s) => self.visit_create_table_statement(s),
            Statement::AlterTable(s) => self.visit_alter_table_statement(s),
            Statement::DropTable(s) => self.visit_drop_table_statement(s),
            Statement::CreateIndex(s) => self.visit_create_index_statement(s),
            Statement::DropIndex(s) => self.visit_drop_index_statement(s),
        }
    }

    /// Visit a `SELECT` statement (default: [`default_output`](Self::default_output)).
    fn visit_select_statement(&mut self, _stmt: &SelectStatement) -> Self::Output {
        self.default_output()
    }
    /// Visit an `INSERT` statement (default: [`default_output`](Self::default_output)).
    fn visit_insert_statement(&mut self, _stmt: &InsertStatement) -> Self::Output {
        self.default_output()
    }
    /// Visit an `UPDATE` statement (default: [`default_output`](Self::default_output)).
    fn visit_update_statement(&mut self, _stmt: &UpdateStatement) -> Self::Output {
        self.default_output()
    }
    /// Visit a `DELETE` statement (default: [`default_output`](Self::default_output)).
    fn visit_delete_statement(&mut self, _stmt: &DeleteStatement) -> Self::Output {
        self.default_output()
    }
    /// Visit a `CREATE TABLE` statement (default: [`default_output`](Self::default_output)).
    fn visit_create_table_statement(&mut self, _stmt: &CreateTableStatement) -> Self::Output {
        self.default_output()
    }
    /// Visit an `ALTER TABLE` statement (default: [`default_output`](Self::default_output)).
    fn visit_alter_table_statement(&mut self, _stmt: &AlterTableStatement) -> Self::Output {
        self.default_output()
    }
    /// Visit a `DROP TABLE` statement (default: [`default_output`](Self::default_output)).
    fn visit_drop_table_statement(&mut self, _stmt: &DropTableStatement) -> Self::Output {
        self.default_output()
    }
    /// Visit a `CREATE INDEX` statement (default: [`default_output`](Self::default_output)).
    fn visit_create_index_statement(&mut self, _stmt: &CreateIndexStatement) -> Self::Output {
        self.default_output()
    }
    /// Visit a `DROP INDEX` statement (default: [`default_output`](Self::default_output)).
    fn visit_drop_index_statement(&mut self, _stmt: &DropIndexStatement) -> Self::Output {
        self.default_output()
    }

    /// Dispatch an [`Expression`] to the matching `visit_*` hook.
    fn visit_expression(&mut self, expr: &Expression) -> Self::Output {
        match expr {
            Expression::Literal(l) => self.visit_literal(l),
            Expression::Identifier(i) => self.visit_identifier(i),
            Expression::QualifiedIdentifier { table, column } => {
                self.visit_qualified_identifier(table, column)
            }
            Expression::BinaryOp { left, op, right } => self.visit_binary_op(left, *op, right),
            Expression::UnaryOp { op, expr } => self.visit_unary_op(*op, expr),
            Expression::LogicalOp { left, op, right } => self.visit_logical_op(left, *op, right),
            Expression::Comparison { left, op, right } => self.visit_comparison(left, *op, right),
            Expression::Function {
                name,
                args,
                distinct,
            } => self.visit_function(name, args, *distinct),
            Expression::Case {
                operand,
                conditions,
                else_result,
            } => self.visit_case(operand, conditions, else_result),
            Expression::Subquery(sq) => self.visit_subquery(sq),
            Expression::Exists { subquery, negated } => self.visit_exists(subquery, *negated),
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
            Expression::Cast { expr, data_type } => self.visit_cast(expr, data_type),
            Expression::IsNull { expr, negated } => self.visit_is_null(expr, *negated),
        }
    }

    /// Visit a literal (default: [`default_output`](Self::default_output)).
    fn visit_literal(&mut self, _literal: &Literal) -> Self::Output {
        self.default_output()
    }
    /// Visit a simple identifier (default: [`default_output`](Self::default_output)).
    fn visit_identifier(&mut self, _ident: &Identifier) -> Self::Output {
        self.default_output()
    }
    /// Visit a `table.column` qualified identifier (default: [`default_output`](Self::default_output)).
    fn visit_qualified_identifier(
        &mut self,
        _table: &Identifier,
        _column: &Identifier,
    ) -> Self::Output {
        self.default_output()
    }
    /// Visit a binary operation (default: [`default_output`](Self::default_output)).
    fn visit_binary_op(
        &mut self,
        _left: &Expression,
        _op: BinaryOperator,
        _right: &Expression,
    ) -> Self::Output {
        self.default_output()
    }
    /// Visit a unary operation (default: [`default_output`](Self::default_output)).
    fn visit_unary_op(&mut self, _op: UnaryOperator, _expr: &Expression) -> Self::Output {
        self.default_output()
    }
    /// Visit a logical connective (default: [`default_output`](Self::default_output)).
    fn visit_logical_op(
        &mut self,
        _left: &Expression,
        _op: LogicalOperator,
        _right: &Expression,
    ) -> Self::Output {
        self.default_output()
    }
    /// Visit a comparison (default: [`default_output`](Self::default_output)).
    fn visit_comparison(
        &mut self,
        _left: &Expression,
        _op: ComparisonOperator,
        _right: &Expression,
    ) -> Self::Output {
        self.default_output()
    }
    /// Visit a function call (default: [`default_output`](Self::default_output)).
    fn visit_function(
        &mut self,
        _name: &Identifier,
        _args: &[Expression],
        _distinct: bool,
    ) -> Self::Output {
        self.default_output()
    }
    /// Visit a `CASE` expression (default: [`default_output`](Self::default_output)).
    fn visit_case(
        &mut self,
        _operand: &Option<Box<Expression>>,
        _conditions: &[(Expression, Expression)],
        _else_result: &Option<Box<Expression>>,
    ) -> Self::Output {
        self.default_output()
    }
    /// Visit a scalar subquery (default: [`default_output`](Self::default_output)).
    fn visit_subquery(&mut self, _subquery: &SelectStatement) -> Self::Output {
        self.default_output()
    }
    /// Visit an `EXISTS` expression (default: [`default_output`](Self::default_output)).
    fn visit_exists(&mut self, _subquery: &SelectStatement, _negated: bool) -> Self::Output {
        self.default_output()
    }
    /// Visit an `IN` expression (default: [`default_output`](Self::default_output)).
    fn visit_in(&mut self, _expr: &Expression, _list: &InList, _negated: bool) -> Self::Output {
        self.default_output()
    }
    /// Visit a `BETWEEN` expression (default: [`default_output`](Self::default_output)).
    fn visit_between(
        &mut self,
        _expr: &Expression,
        _low: &Expression,
        _high: &Expression,
        _negated: bool,
    ) -> Self::Output {
        self.default_output()
    }
    /// Visit a `CAST` expression (default: [`default_output`](Self::default_output)).
    fn visit_cast(&mut self, _expr: &Expression, _data_type: &DataType) -> Self::Output {
        self.default_output()
    }
    /// Visit an `IS NULL` expression (default: [`default_output`](Self::default_output)).
    fn visit_is_null(&mut self, _expr: &Expression, _negated: bool) -> Self::Output {
        self.default_output()
    }

    /// Visit a [`DataType`] (default: [`default_output`](Self::default_output)).
    fn visit_data_type(&mut self, _data_type: &DataType) -> Self::Output {
        self.default_output()
    }
}

/// A node that can accept a [`Visitor`].
///
/// Implemented by the three root AST node kinds ([`Statement`], [`Expression`],
/// [`DataType`]) so a caller can uniformly drive any of them through a visitor
/// via `node.accept(&mut visitor)`.
pub trait Visitable {
    /// Drive `visitor` over this node and return its result.
    fn accept<V: Visitor>(&self, visitor: &mut V) -> V::Output;
}

impl Visitable for Statement {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> V::Output {
        visitor.visit_statement(self)
    }
}

impl Visitable for Expression {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> V::Output {
        visitor.visit_expression(self)
    }
}

impl Visitable for DataType {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> V::Output {
        visitor.visit_data_type(self)
    }
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
    use crate::ast::ddl::{
        AlterTableAction, ColumnConstraint, ColumnDef, IndexColumn, TableOptions,
    };
    use crate::ast::expression::{
        BinaryOperator as BinOp, ComparisonOperator as CmpOp, InList, LogicalOperator as LogOp,
        UnaryOperator as UnOp,
    };
    use crate::ast::identifier::QualifiedName;
    use crate::ast::literal::Literal;
    use crate::ast::span::Span;
    use crate::ast::statement::{InsertSource, SelectItem};
    use std::cell::Cell;

    // A counting visitor that records how often each dispatch path fires.
    // The trait's `default_output` takes `&self` (per the design contract), so
    // the counters use `Cell` for interior mutability. We do NOT override the
    // `visit_statement` / `visit_expression` dispatchers — we let them fall
    // through to the per-variant hooks, which default to `default_output`.
    // Overriding only `visit_data_type` proves the DataType path is distinct.

    struct CountingVisitor {
        data_types: Cell<u32>,
        default_calls: Cell<u32>,
    }

    impl CountingVisitor {
        fn new() -> Self {
            Self {
                data_types: Cell::new(0),
                default_calls: Cell::new(0),
            }
        }

        fn data_types(&self) -> u32 {
            self.data_types.get()
        }

        fn default_calls(&self) -> u32 {
            self.default_calls.get()
        }
    }

    impl Visitor for CountingVisitor {
        type Output = ();

        fn default_output(&self) -> Self::Output {
            self.default_calls.set(self.default_calls.get() + 1);
        }

        // Override ONLY visit_data_type to prove it is a distinct dispatch
        // path; every other node kind falls back to default_output.
        fn visit_data_type(&mut self, _data_type: &DataType) -> Self::Output {
            self.data_types.set(self.data_types.get() + 1);
        }
    }

    fn ident(s: &str) -> Identifier {
        Identifier::new(s.to_string())
    }

    fn qualified(s: &str) -> QualifiedName {
        QualifiedName::new(None, s.to_string())
    }

    fn minimal_select() -> Statement {
        Statement::Select(Box::new(SelectStatement::simple(vec![
            SelectItem::Wildcard,
        ])))
    }

    fn minimal_create_table() -> Statement {
        Statement::CreateTable(Box::new(crate::ast::ddl::CreateTableStatement {
            span: Span::new(0, 1),
            if_not_exists: false,
            temporary: false,
            name: qualified("t"),
            columns: vec![ColumnDef {
                span: Span::new(0, 1),
                name: ident("id"),
                data_type: DataType::BigInt,
                nullable: false,
                default: None,
                constraints: vec![ColumnConstraint::PrimaryKey],
            }],
            constraints: vec![],
            options: TableOptions::default(),
        }))
    }

    fn minimal_alter_table() -> Statement {
        Statement::AlterTable(Box::new(crate::ast::ddl::AlterTableStatement {
            span: Span::new(0, 1),
            name: qualified("t"),
            actions: vec![AlterTableAction::DropColumn(ident("c"))],
        }))
    }

    fn minimal_drop_table() -> Statement {
        Statement::DropTable(Box::new(crate::ast::ddl::DropTableStatement {
            span: Span::new(0, 1),
            if_exists: false,
            names: vec![qualified("t")],
        }))
    }

    fn minimal_create_index() -> Statement {
        Statement::CreateIndex(Box::new(crate::ast::ddl::CreateIndexStatement {
            span: Span::new(0, 1),
            unique: false,
            if_not_exists: false,
            name: ident("idx"),
            table: qualified("t"),
            columns: vec![IndexColumn {
                name: ident("c"),
                direction: None,
            }],
        }))
    }

    fn minimal_drop_index() -> Statement {
        Statement::DropIndex(Box::new(crate::ast::ddl::DropIndexStatement {
            span: Span::new(0, 1),
            if_exists: false,
            name: ident("idx"),
            table: None,
        }))
    }

    fn minimal_insert() -> Statement {
        Statement::Insert(Box::new(InsertStatement {
            span: Span::new(0, 1),
            table: qualified("t"),
            columns: vec![],
            source: InsertSource::Values(vec![]),
            on_conflict: None,
        }))
    }

    fn minimal_update() -> Statement {
        Statement::Update(Box::new(crate::ast::statement::UpdateStatement {
            span: Span::new(0, 1),
            table: crate::ast::join::TableFactor::Table {
                name: qualified("t"),
                alias: None,
            },
            assignments: vec![],
            from: None,
            where_clause: None,
        }))
    }

    fn minimal_delete() -> Statement {
        Statement::Delete(Box::new(crate::ast::statement::DeleteStatement {
            span: Span::new(0, 1),
            table: crate::ast::join::TableFactor::Table {
                name: qualified("t"),
                alias: None,
            },
            using: None,
            where_clause: None,
        }))
    }

    // -- Visitable.accept dispatches each Statement variant -----------------

    #[test]
    fn statement_visitable_accept_dispatches_select() {
        let mut v = CountingVisitor::new();
        minimal_select().accept(&mut v);
        assert_eq!(v.default_calls(), 1);
        assert_eq!(v.data_types(), 0);
    }

    #[test]
    fn statement_visitable_accept_dispatches_insert() {
        let mut v = CountingVisitor::new();
        minimal_insert().accept(&mut v);
        assert_eq!(v.default_calls(), 1);
    }

    #[test]
    fn statement_visitable_accept_dispatches_update() {
        let mut v = CountingVisitor::new();
        minimal_update().accept(&mut v);
        assert_eq!(v.default_calls(), 1);
    }

    #[test]
    fn statement_visitable_accept_dispatches_delete() {
        let mut v = CountingVisitor::new();
        minimal_delete().accept(&mut v);
        assert_eq!(v.default_calls(), 1);
    }

    #[test]
    fn statement_visitable_accept_dispatches_create_table() {
        let mut v = CountingVisitor::new();
        minimal_create_table().accept(&mut v);
        assert_eq!(v.default_calls(), 1);
    }

    #[test]
    fn statement_visitable_accept_dispatches_alter_table() {
        let mut v = CountingVisitor::new();
        minimal_alter_table().accept(&mut v);
        assert_eq!(v.default_calls(), 1);
    }

    #[test]
    fn statement_visitable_accept_dispatches_drop_table() {
        let mut v = CountingVisitor::new();
        minimal_drop_table().accept(&mut v);
        assert_eq!(v.default_calls(), 1);
    }

    #[test]
    fn statement_visitable_accept_dispatches_create_index() {
        let mut v = CountingVisitor::new();
        minimal_create_index().accept(&mut v);
        assert_eq!(v.default_calls(), 1);
    }

    #[test]
    fn statement_visitable_accept_dispatches_drop_index() {
        let mut v = CountingVisitor::new();
        minimal_drop_index().accept(&mut v);
        assert_eq!(v.default_calls(), 1);
    }

    // -- visit_statement dispatches all 9 variants without panic -----------
    #[test]
    fn visit_statement_covers_all_nine_variants() {
        let stmts = vec![
            minimal_select(),
            minimal_insert(),
            minimal_update(),
            minimal_delete(),
            minimal_create_table(),
            minimal_alter_table(),
            minimal_drop_table(),
            minimal_create_index(),
            minimal_drop_index(),
        ];
        assert_eq!(stmts.len(), 9, "all 9 Statement variants must be exercised");
        for stmt in &stmts {
            let mut v = CountingVisitor::new();
            stmt.accept(&mut v);
            assert_eq!(v.default_calls(), 1, "variant {stmt:?} did not dispatch");
        }
    }

    // -- Expression Visitable + visit_expression coverage -------------------

    #[test]
    fn expression_visitable_accept_dispatches_literal() {
        let mut v = CountingVisitor::new();
        let expr = Expression::Literal(Literal::Integer(7));
        expr.accept(&mut v);
        assert_eq!(v.default_calls(), 1);
    }

    #[test]
    fn visit_expression_covers_identifier_and_qualified() {
        let mut v = CountingVisitor::new();
        Expression::Identifier(ident("a")).accept(&mut v);
        Expression::QualifiedIdentifier {
            table: ident("t"),
            column: ident("c"),
        }
        .accept(&mut v);
        assert_eq!(v.default_calls(), 2);
    }

    #[test]
    fn visit_expression_covers_operator_variants() {
        let mut v = CountingVisitor::new();
        let leaf = Expression::Literal(Literal::Integer(1));
        Expression::BinaryOp {
            left: Box::new(leaf.clone()),
            op: BinOp::Add,
            right: Box::new(leaf.clone()),
        }
        .accept(&mut v);
        Expression::UnaryOp {
            op: UnOp::Minus,
            expr: Box::new(leaf.clone()),
        }
        .accept(&mut v);
        Expression::LogicalOp {
            left: Box::new(leaf.clone()),
            op: LogOp::And,
            right: Box::new(leaf.clone()),
        }
        .accept(&mut v);
        Expression::Comparison {
            left: Box::new(leaf.clone()),
            op: CmpOp::Eq,
            right: Box::new(leaf),
        }
        .accept(&mut v);
        assert_eq!(v.default_calls(), 4);
    }

    #[test]
    fn visit_expression_covers_advanced_variants() {
        let mut v = CountingVisitor::new();
        let leaf = Expression::Literal(Literal::Integer(1));
        Expression::Function {
            name: ident("count"),
            args: vec![leaf.clone()],
            distinct: true,
        }
        .accept(&mut v);
        Expression::Case {
            operand: None,
            conditions: vec![(leaf.clone(), leaf.clone())],
            else_result: None,
        }
        .accept(&mut v);
        Expression::Cast {
            expr: Box::new(leaf.clone()),
            data_type: DataType::Int,
        }
        .accept(&mut v);
        // Cast routes to visit_cast (default), not visit_data_type, so the
        // DataType inside it must NOT inflate the data_types counter.
        assert_eq!(v.data_types(), 0);
        assert_eq!(v.default_calls(), 3);
    }

    /// All 15 Expression variants are reachable through accept(). Building the
    /// list forces a compile error if `visit_expression` ever drops an arm.
    #[test]
    fn visit_expression_covers_all_fifteen_variants() {
        let sub = SelectStatement::simple(vec![SelectItem::Wildcard]);
        let exprs = vec![
            Expression::Literal(Literal::Integer(1)),
            Expression::Identifier(ident("c")),
            Expression::QualifiedIdentifier {
                table: ident("t"),
                column: ident("c"),
            },
            Expression::BinaryOp {
                left: Box::new(Expression::Literal(Literal::Integer(1))),
                op: BinOp::Add,
                right: Box::new(Expression::Literal(Literal::Integer(2))),
            },
            Expression::UnaryOp {
                op: UnOp::Plus,
                expr: Box::new(Expression::Literal(Literal::Integer(1))),
            },
            Expression::LogicalOp {
                left: Box::new(Expression::Literal(Literal::Boolean(true))),
                op: LogOp::And,
                right: Box::new(Expression::Literal(Literal::Boolean(false))),
            },
            Expression::Comparison {
                left: Box::new(Expression::Identifier(ident("x"))),
                op: CmpOp::Eq,
                right: Box::new(Expression::Literal(Literal::Integer(1))),
            },
            Expression::Function {
                name: ident("COUNT"),
                args: vec![],
                distinct: false,
            },
            Expression::Case {
                operand: None,
                conditions: vec![],
                else_result: None,
            },
            Expression::Subquery(Box::new(sub.clone())),
            Expression::Exists {
                subquery: Box::new(sub.clone()),
                negated: false,
            },
            Expression::In {
                expr: Box::new(Expression::Identifier(ident("x"))),
                list: InList::Values(vec![]),
                negated: false,
            },
            Expression::Between {
                expr: Box::new(Expression::Identifier(ident("x"))),
                low: Box::new(Expression::Literal(Literal::Integer(1))),
                high: Box::new(Expression::Literal(Literal::Integer(10))),
                negated: false,
            },
            Expression::Cast {
                expr: Box::new(Expression::Identifier(ident("x"))),
                data_type: DataType::Int,
            },
            Expression::IsNull {
                expr: Box::new(Expression::Identifier(ident("x"))),
                negated: false,
            },
        ];
        assert_eq!(
            exprs.len(),
            15,
            "all 15 Expression variants must be exercised"
        );
        for e in &exprs {
            let mut v = CountingVisitor::new();
            e.accept(&mut v);
            assert_eq!(v.default_calls(), 1, "variant not dispatched");
        }
    }

    // -- DataType Visitable routes to the overridden hook -------------------

    #[test]
    fn datatype_visitable_accept_routes_to_visit_data_type() {
        let mut v = CountingVisitor::new();
        DataType::BigInt.accept(&mut v);
        DataType::VarChar { length: Some(255) }.accept(&mut v);
        assert_eq!(v.data_types(), 2);
        assert_eq!(v.default_calls(), 0);
    }

    // -- Generic Output type: a String-returning visitor --------------------

    struct StringVisitor;

    impl Visitor for StringVisitor {
        type Output = String;
        fn default_output(&self) -> Self::Output {
            String::from("default")
        }
        fn visit_identifier(&mut self, ident: &Identifier) -> Self::Output {
            ident.value().to_string()
        }
        fn visit_data_type(&mut self, _data_type: &DataType) -> Self::Output {
            String::from("datatype")
        }
    }

    #[test]
    fn visitor_with_string_output_uses_overridden_method() {
        let mut v = StringVisitor;
        let out = Expression::Identifier(ident("foo")).accept(&mut v);
        assert_eq!(out, "foo");
    }

    #[test]
    fn visitor_with_string_output_falls_back_to_default_for_select() {
        let mut v = StringVisitor;
        let out = minimal_select().accept(&mut v);
        assert_eq!(out, "default");
    }
}
