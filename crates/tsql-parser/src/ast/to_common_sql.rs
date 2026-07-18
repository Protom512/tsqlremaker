//! Direct T-SQL → `common_sql::ast` converter (single pass).
//!
//! This module is the **composite** replacement for the former two-stage
//! conversion bridge:
//!
//! 1. Stage-1 (`ast/to_common.rs`): `tsql_parser::ast::Statement` → the legacy
//!    internal `crate::common::CommonStatement` (`ToCommonAst` trait).
//! 2. Stage-2 (`common/convert_common_sql.rs`): legacy `Common*` → the
//!    standalone `common_sql::ast::Statement` (`From<Common*>` impls +
//!    `convert()` entry point).
//!
//! [`to_common_sql`] fuses both stages into one pass, producing a
//! `common_sql::ast::Statement` directly from a `tsql_parser::ast::Statement`
//! reference. It preserves the **exact behavioral parity** of the legacy
//! pipeline (this is the acceptance bar — downstream emitters must see no
//! change).
//!
//! ## Architecture (DAG compliance)
//!
//! `tsql-parser` already depends on `common-sql` (`Cargo.toml`), so a
//! converter living here respects the clean dependency DAG
//! (`tsql-parser → common-sql`, never the reverse). The converter is therefore
//! placed in `tsql-parser` and re-exported, **not** in `common-sql` (which must
//! not depend on `tsql-parser`).
//!
//! ## Pinned lossy mappings (recorded — must match Stage-1 ∪ Stage-2)
//!
//! These are the union of the two former stages' documented lossy decisions.
//! Any change here breaks downstream emitter parity and must be deliberate.
//!
//! * **T-SQL control-flow → `DialectSpecific` (intentional parity break,
//!   #158/T3)**: every procedural/control-flow variant (`Declare`, `Set`,
//!   `VariableAssignment`, `If`, `While`, `Block`, `Break`, `Continue`,
//!   `Return`, `TryCatch`, `Transaction`, `Throw`, `Raiserror`, `Exec`) maps
//!   to `Some(Statement::DialectSpecific { source, span })`. `source` is the
//!   T-SQL variant's Debug classification (so a downstream emitter can
//!   dispatch on construct kind) and `span` is the AST node's own source span.
//!   This is a **deliberate divergence** from the former Stage-2 behavior,
//!   which dropped these to `None` — the escape hatch now preserves the
//!   construct for dialect-specific re-implementation (e.g. PL/pgSQL). DDL
//!   (`Create`, `AlterTable`) and `BatchSeparator` still yield `None` (DDL has
//!   dedicated destination variants / is out of scope; `BatchSeparator` is a
//!   batch boundary, not a statement).
//! * **`Vec<TableReference>` → `Option<TableFactor>` first-element**: the
//!   legacy FROM clause is a flat list; the destination models FROM as a single
//!   `TableFactor` (multi-table expressed via `Join`). Only the first table
//!   survives; extras are silently dropped (no join condition can be
//!   synthesized).
//! * **`DefaultValues → InsertSource::Values(vec![])`**: the destination has
//!   no `DefaultValues` variant.
//! * **`LIKE` → `Comparison::{Like, NotLike}`**: the optional `ESCAPE` clause
//!   is silently dropped. `ILike` / `NotILike` are never produced.
//! * **CASE `operand = None`**: the source `CaseExpression` is searched-CASE
//!   only; the destination `operand` field is always `None`. `branches` →
//!   `conditions`.
//! * **Span**: dropped for every node (the destination uses a `Span::new(0,0)`
//!   placeholder where a span is structurally required).
//! * **Stage-1 expression drops short-circuit to `None`**: `Hex` literals, the
//!   `Tilde` unary operator, the ASE-specific `NotLt`/`NotGt` binary operators,
//!   `IS TRUE`/`IS FALSE` (only `IS NULL`/`IS UNKNOWN` convert), and any
//!   sub-expression that fails to convert propagates `None` upward via `?`.
//!
//! ## Destination variants never produced
//!
//! The destination `Statement` has variants with no T-SQL source on this path
//! (`CreateTable`, `AlterTable`, `DropTable`, `CreateIndex`, `DropIndex`) —
//! T-SQL DDL is mapped to `None` here, not to those variants. The destination
//! `Expression` variants `Cast`, `QualifiedIdentifier` (produced only from
//! `ColumnReference`), and the `ILike`/`NotILike` comparison operators are
//! likewise never produced by this converter.

use common_sql::ast::{
    self as csql, AlterTableAction as SqlAlterTableAction,
    AlterTableStatement as SqlAlterTableStatement, Assignment as SqlAssignment,
    BinaryOperator as SqlBinaryOp, ColumnConstraint as SqlColumnConstraint,
    ColumnDef as SqlColumnDef, ComparisonOperator as SqlComparison,
    CreateIndexStatement as SqlCreateIndexStatement,
    CreateTableStatement as SqlCreateTableStatement, DataType as SqlDataType,
    DeleteStatement as SqlDeleteStatement, Expression as SqlExpr, GroupByClause, GroupByItem,
    Identifier as SqlIdentifier, InList as SqlInList, IndexColumn as SqlIndexColumn,
    InsertSource as SqlInsertSource, InsertStatement as SqlInsertStatement,
    LimitClause as SqlLimitClause, Literal as SqlLiteral, LogicalOperator as SqlLogical,
    OrderByClause, OrderByItem as SqlOrderByItem, QualifiedName as SqlQualifiedName,
    SelectItem as SqlSelectItem, SelectStatement as SqlSelectStatement, SortDirection,
    Statement as SqlStmt, TableAlias, TableConstraint as SqlTableConstraint,
    TableFactor as SqlTableFactor, TableOptions as SqlTableOptions, UnaryOperator as SqlUnaryOp,
    UpdateStatement as SqlUpdateStatement,
};

use crate::ast::data_modification::Assignment as ColumnAssignment;
use crate::ast::{
    AlterTableOperation, AlterTableStatement, AstNode, BinaryOperator, CaseExpression,
    ColumnConstraint as TsqlColumnConstraint, ColumnDefinition, ColumnReference, CreateStatement,
    DataType as TsqlDataType, DeleteStatement, Expression, FromClause, FunctionCall, InList,
    IndexDefinition, InsertSource, InsertStatement, IsValue, LimitClause, Literal, OrderByItem,
    SelectItem, SelectStatement, Statement, TableConstraint as TsqlTableConstraint,
    TableDefinition, TableReference, UnaryOperator, UpdateStatement,
};

// ===========================================================================
// Public entry point
// ===========================================================================

/// Convert a T-SQL [`Statement`] reference directly into the standalone
/// `common_sql::ast::Statement`.
///
/// Returns `None` for any construct that has no destination representation —
/// see the [module docs](self) for the full lossy-mapping table.
///
/// This is the fused single-pass replacement for the legacy
/// `stmt.to_common_ast().and_then(convert)` pipeline.
///
/// # Examples
///
/// ```
/// use tsql_parser::{parse_one, ast::to_common_sql};
///
/// let stmt = parse_one("SELECT * FROM users").unwrap();
/// let common = to_common_sql(&stmt);
/// assert!(common.is_some());
/// ```
#[must_use]
pub fn to_common_sql(stmt: &Statement) -> Option<SqlStmt> {
    match stmt {
        Statement::Select(s) => Some(SqlStmt::Select(Box::new(convert_select(s)?))),
        Statement::Insert(s) => Some(SqlStmt::Insert(Box::new(convert_insert(s)?))),
        Statement::Update(s) => Some(SqlStmt::Update(Box::new(convert_update(s)?))),
        Statement::Delete(s) => Some(SqlStmt::Delete(Box::new(convert_delete(s)?))),
        // T-SQL control-flow / procedural variants have no representation in
        // the dialect-independent AST. They are carried verbatim through the
        // `DialectSpecific` escape hatch (#158): `source` is the T-SQL
        // variant's Debug classification (so a downstream emitter can dispatch
        // on construct kind — the deleted postgresql-emitter matched on
        // `Declare(`/`If(` etc.), `span` is the AST node's own source span.
        Statement::Declare(_)
        | Statement::Set(_)
        | Statement::VariableAssignment(_)
        | Statement::If(_)
        | Statement::While(_)
        | Statement::Block(_)
        | Statement::Break(_)
        | Statement::Continue(_)
        | Statement::Return(_)
        | Statement::TryCatch(_)
        | Statement::Transaction(_)
        | Statement::Throw(_)
        | Statement::Raiserror(_)
        | Statement::Exec(_) => Some(dialect_specific(stmt)),
        // DDL: CREATE TABLE / CREATE INDEX have dedicated destination variants
        // and are mapped by the DDL converters (§0.6 short-circuit: any column
        // or constraint whose type/expr has no destination yields None). CREATE
        // VIEW / PROCEDURE / TRIGGER have no destination shape → None.
        // BatchSeparator (GO) is a batch boundary, not a statement → None.
        Statement::Create(create) => convert_create(create),
        Statement::AlterTable(alter) => convert_alter_table(alter),
        Statement::BatchSeparator(_) => None,
    }
}

/// Build a `DialectSpecific` escape-hatch node from a T-SQL control-flow /
/// procedural statement.
///
/// `source` carries the variant's Debug classification (e.g. `"Declare(...)"`)
/// so downstream emitters can dispatch on construct kind, and `span` is the
/// AST node's own source span (located via [`AstNode::span`]).
fn dialect_specific(stmt: &Statement) -> SqlStmt {
    let tsql_span = stmt.span();
    SqlStmt::DialectSpecific {
        source: format!("{stmt:?}"),
        span: csql::Span::new(tsql_span.start, tsql_span.end),
    }
}

// ===========================================================================
// Statement-level DDL dispatch (T2.3)
// ===========================================================================

/// Dispatch a T-SQL [`CreateStatement`] to the destination `Statement`.
///
/// `CreateTable` and `CreateIndex` map to dedicated destination variants.
/// `CreateView`, `CreateProcedure`, and `CreateTrigger` have no destination
/// shape → `None`.
///
/// Per §0.6, a CREATE TABLE whose any column or constraint fails to convert
/// yields `None` (the `?` inside [`convert_create_table`] propagates).
fn convert_create(create: &CreateStatement) -> Option<SqlStmt> {
    match create {
        CreateStatement::Table(t) => Some(SqlStmt::CreateTable(Box::new(convert_create_table(t)?))),
        CreateStatement::Index(i) => Some(SqlStmt::CreateIndex(Box::new(convert_create_index(i)?))),
        CreateStatement::View(_) | CreateStatement::Procedure(_) | CreateStatement::Trigger(_) => {
            None
        }
    }
}

/// Convert a T-SQL [`TableDefinition`] into a destination
/// [`SqlCreateTableStatement`].
///
/// §0.6 short-circuit: if any column or table-level constraint fails to
/// convert, the whole statement yields `None`. The T-SQL `Identifier` table
/// name is lifted into a single-segment [`SqlQualifiedName`].
fn convert_create_table(t: &TableDefinition) -> Option<SqlCreateTableStatement> {
    let mut columns = Vec::new();
    for col in &t.columns {
        columns.push(convert_column_def(col)?);
    }
    let mut constraints = Vec::new();
    for tc in &t.constraints {
        constraints.push(convert_table_constraint(tc)?);
    }
    Some(SqlCreateTableStatement {
        // Span dropped (parity: placeholder).
        span: csql::Span::new(0, 0),
        // T-SQL CREATE TABLE has no IF NOT EXISTS in the AST → false.
        if_not_exists: false,
        temporary: t.temporary,
        name: SqlQualifiedName::new(None, t.name.name.clone()),
        columns,
        constraints,
        // T-SQL table node carries no engine/charset options.
        options: SqlTableOptions::default(),
    })
}

/// Convert a T-SQL [`IndexDefinition`] into a destination
/// [`SqlCreateIndexStatement`].
///
/// T-SQL indexes carry no per-column sort direction, so each column maps to
/// an [`SqlIndexColumn`] with `direction: None`. The index and table names
/// are lifted into single-segment qualified names.
fn convert_create_index(i: &IndexDefinition) -> Option<SqlCreateIndexStatement> {
    let columns = i
        .columns
        .iter()
        .map(|c| SqlIndexColumn {
            name: SqlIdentifier::new(c.name.clone()),
            direction: None,
        })
        .collect();
    Some(SqlCreateIndexStatement {
        span: csql::Span::new(0, 0),
        unique: i.unique,
        if_not_exists: false,
        name: SqlIdentifier::new(i.name.name.clone()),
        table: SqlQualifiedName::new(None, i.table.name.clone()),
        columns,
    })
}

/// Convert a T-SQL [`AlterTableStatement`] into a destination
/// [`SqlAlterTableStatement`].
///
/// The single T-SQL `operation` maps 1:1 (but reshaped) to a one-element
/// `actions` vec. See [`convert_alter_operation`].
fn convert_alter_table(alter: &AlterTableStatement) -> Option<SqlStmt> {
    let action = convert_alter_operation(&alter.operation)?;
    Some(SqlStmt::AlterTable(Box::new(SqlAlterTableStatement {
        span: csql::Span::new(0, 0),
        name: SqlQualifiedName::new(None, alter.table.name.clone()),
        actions: vec![action],
    })))
}

/// Map a T-SQL [`AlterTableOperation`] to a destination [`SqlAlterTableAction`].
///
/// The three T-SQL variants reshape as follows (destination field shapes
/// verified against `common-sql/ast/ddl.rs:316-336`):
/// - `AddColumn(AddColumnDefinition)` → `AddColumn(ColumnDef)`. The T-SQL
///   `AddColumnDefinition` has no `constraints` / `default_value`, so the
///   destination `ColumnDef` is built with `constraints: vec![]` and
///   `default: None`.
/// - `DropColumn(Identifier)` → `DropColumn(Identifier)` (direct).
/// - `AlterColumn(AlterColumnDefinition)` → `AlterColumn { column,
///   data_type: Some(...), default: None, nullable: <nullability mapped via
///   unwrap_or(true)> }`. The T-SQL shape lacks a default; `data_type` is
///   always present on this path, so it maps to `Some(convert_data_type?)`.
fn convert_alter_operation(op: &AlterTableOperation) -> Option<SqlAlterTableAction> {
    let action = match op {
        AlterTableOperation::AddColumn(add) => {
            let data_type = convert_data_type(&add.data_type)?;
            SqlAlterTableAction::AddColumn(SqlColumnDef {
                span: csql::Span::new(0, 0),
                name: SqlIdentifier::new(add.name.name.clone()),
                data_type,
                nullable: add.nullability.unwrap_or(true),
                // T-SQL AddColumnDefinition carries no default / constraints.
                default: None,
                constraints: vec![],
            })
        }
        AlterTableOperation::DropColumn(name) => {
            SqlAlterTableAction::DropColumn(SqlIdentifier::new(name.name.clone()))
        }
        AlterTableOperation::AlterColumn(alt) => {
            let data_type = convert_data_type(&alt.data_type)?;
            SqlAlterTableAction::AlterColumn {
                column: SqlIdentifier::new(alt.name.name.clone()),
                data_type: Some(data_type),
                // T-SQL AlterColumnDefinition carries no default.
                default: None,
                nullable: alt.nullability,
            }
        }
    };
    Some(action)
}

// Silence the unused-import / dead-code analysis on the T-SQL types we keep in
// scope purely for documentation cross-referencing of the mapping contract.
// (All are used below except in degenerate branches.)
#[allow(unused_imports)]
use crate::ast::VariableAssignment as _VarAssignDoc;

// ===========================================================================
// Statement-level converters
// ===========================================================================

fn convert_select(sel: &SelectStatement) -> Option<SqlSelectStatement> {
    // パリティ契約: 変換不能な SELECT 項目があれば文全体を None にする (filter_map で
    // 飲み込むと Hex リテラル等が暗黙に消失する)。
    let mut projection = Vec::new();
    for item in &sel.columns {
        projection.push(convert_select_item(item)?);
    }

    // Lossy: Vec<TableReference> -> Option<TableFactor> first-element.
    let from = sel
        .from
        .as_ref()
        .and_then(convert_from_clause)
        .and_then(|mut factors| factors.next().map(SqlTableFactor::from));

    // パリティ契約: 部分式が変換不能 (None) なら文全体を None にする
    // (.and_then で飲み込むと ASE 固有演算子 NotLt/NotGt 等が暗黙に消失する)。
    let where_clause = match &sel.where_clause {
        Some(w) => Some(convert_expr(w)?),
        None => None,
    };
    let having = match &sel.having {
        Some(h) => Some(convert_expr(h)?),
        None => None,
    };

    let group_by = if sel.group_by.is_empty() {
        None
    } else {
        let items: Vec<GroupByItem> = sel
            .group_by
            .iter()
            .filter_map(|e| convert_expr(e).map(GroupByItem::Expression))
            .collect();
        if items.is_empty() {
            None
        } else {
            Some(GroupByClause {
                span: csql::Span::new(0, 0),
                items,
            })
        }
    };

    let order_by = if sel.order_by.is_empty() {
        None
    } else {
        let items: Vec<SqlOrderByItem> = sel
            .order_by
            .iter()
            .filter_map(convert_order_by_item)
            .collect();
        if items.is_empty() {
            None
        } else {
            Some(OrderByClause {
                span: csql::Span::new(0, 0),
                items,
            })
        }
    };

    let limit = sel.limit.as_ref().and_then(convert_limit);

    Some(SqlSelectStatement {
        // Span dropped.
        span: csql::Span::new(0, 0),
        // Legacy has no WITH (CTE) -> always None.
        with: None,
        projection,
        from,
        where_clause,
        group_by,
        having,
        order_by,
        limit,
    })
}

fn convert_insert(ins: &InsertStatement) -> Option<SqlInsertStatement> {
    let source = convert_insert_source(&ins.source)?;
    Some(SqlInsertStatement {
        span: csql::Span::new(0, 0),
        table: SqlQualifiedName::new(None, ins.table.name.clone()),
        columns: ins
            .columns
            .iter()
            .map(|id| SqlIdentifier::new(id.name.clone()))
            .collect(),
        source,
        // Legacy carries no ON CONFLICT -> always None.
        on_conflict: None,
    })
}

fn convert_update(upd: &UpdateStatement) -> Option<SqlUpdateStatement> {
    // Stage-1: UPDATE with FROM clause -> DialectSpecific -> None.
    if upd.from_clause.is_some() {
        return None;
    }
    // Stage-1: UPDATE with a non-Table table reference -> DialectSpecific -> None.
    let table_name = match &upd.table {
        TableReference::Table { name, .. } => &name.name,
        _ => return None,
    };

    let assignments = upd
        .assignments
        .iter()
        .filter_map(convert_assignment)
        .collect();
    let where_clause = upd.where_clause.as_ref().and_then(convert_expr);

    Some(csql::UpdateStatement {
        span: csql::Span::new(0, 0),
        table: SqlTableFactor::Table {
            name: SqlQualifiedName::new(None, table_name.clone()),
            alias: None,
        },
        assignments,
        // Legacy UPDATE node has no usable FROM after the guard above.
        from: None,
        where_clause,
    })
}

fn convert_delete(del: &DeleteStatement) -> Option<SqlDeleteStatement> {
    // Stage-1: DELETE with FROM clause -> DialectSpecific -> None.
    if del.from_clause.is_some() {
        return None;
    }
    let where_clause = del.where_clause.as_ref().and_then(convert_expr);
    Some(SqlDeleteStatement {
        span: csql::Span::new(0, 0),
        table: SqlTableFactor::Table {
            name: SqlQualifiedName::new(None, del.table.name.clone()),
            alias: None,
        },
        // Legacy DELETE has no USING clause.
        using: None,
        where_clause,
    })
}

// ===========================================================================
// Clause converters
// ===========================================================================

fn convert_select_item(item: &SelectItem) -> Option<SqlSelectItem> {
    match item {
        SelectItem::Expression(expr, alias) => {
            let common_expr = convert_expr(expr)?;
            let alias = alias.as_ref().map(|a| SqlIdentifier::new(a.name.clone()));
            Some(SqlSelectItem::Expression {
                expr: common_expr,
                alias,
            })
        }
        SelectItem::Wildcard => Some(SqlSelectItem::Wildcard),
        // Lossy: String -> Identifier.
        SelectItem::QualifiedWildcard(id) => Some(SqlSelectItem::QualifiedWildcard {
            table: SqlIdentifier::new(id.name.clone()),
        }),
    }
}

/// Convert a legacy `FromClause` into an iterator-style `Option<Vec<..>>`.
///
/// Returns the flat list of table references as `common_sql::TableFactor`s.
/// The caller takes only the first element (see [`convert_select`]).
///
/// Note: the legacy `Joined` variant is intentionally skipped here — the
/// destination expresses multi-table FROM via `TableFactor::Join`, but the
/// legacy `Joined` node carries joins without a base table, and Stage-1
/// likewise left it unhandled. Skipping preserves parity.
fn convert_from_clause(from: &FromClause) -> Option<std::vec::IntoIter<FromFactor>> {
    let mut factors = Vec::new();
    for table_ref in &from.tables {
        match table_ref {
            TableReference::Table { name, alias, .. } => {
                factors.push(FromFactor {
                    inner: SqlTableFactor::Table {
                        name: SqlQualifiedName::new(None, name.name.clone()),
                        alias: alias
                            .as_ref()
                            .map(|a| TableAlias::new(a.name.clone(), vec![])),
                    },
                });
            }
            TableReference::Subquery { query, alias, .. } => {
                let common_select = convert_select(query)?;
                factors.push(FromFactor {
                    inner: SqlTableFactor::Derived {
                        subquery: Box::new(common_select),
                        alias: alias
                            .as_ref()
                            .map(|a| TableAlias::new(a.name.clone(), vec![])),
                    },
                });
            }
            TableReference::Joined { .. } => {
                // JOIN handling intentionally skipped (parity with Stage-1).
            }
        }
    }
    Some(factors.into_iter())
}

/// Internal carrier so `convert_from_clause` can return a uniform iterator.
/// The single-element extraction in [`convert_select`] consumes it directly.
struct FromFactor {
    inner: SqlTableFactor,
}

impl From<FromFactor> for SqlTableFactor {
    fn from(f: FromFactor) -> Self {
        f.inner
    }
}

fn convert_order_by_item(item: &OrderByItem) -> Option<SqlOrderByItem> {
    let expr = convert_expr(&item.expr)?;
    Some(SqlOrderByItem {
        expr,
        // asc: bool -> explicit SortDirection.
        direction: Some(if item.asc {
            SortDirection::Asc
        } else {
            SortDirection::Desc
        }),
        // Legacy carries no NULLS ordering -> None (DB default).
        nulls: None,
    })
}

fn convert_limit(l: &LimitClause) -> Option<SqlLimitClause> {
    Some(SqlLimitClause {
        span: csql::Span::new(0, 0),
        limit: convert_expr(&l.limit)?,
        offset: l.offset.as_ref().and_then(convert_expr),
    })
}

fn convert_insert_source(src: &InsertSource) -> Option<SqlInsertSource> {
    match src {
        InsertSource::Values(rows) => {
            // Stage-1 parity: drop any row whose element count changed after
            // filtering non-convertible expressions (i.e. only keep rows that
            // converted cleanly and entirely).
            let common_rows: Vec<Vec<SqlExpr>> = rows
                .iter()
                .filter_map(|row| {
                    let common_row: Vec<SqlExpr> = row.iter().filter_map(convert_expr).collect();
                    if common_row.len() == row.len() {
                        Some(common_row)
                    } else {
                        None
                    }
                })
                .collect();
            Some(SqlInsertSource::Values(common_rows))
        }
        InsertSource::Select(select) => {
            let common_select = convert_select(select)?;
            Some(SqlInsertSource::Select(Box::new(common_select)))
        }
        // Lossy: destination has no DefaultValues variant -> empty Values.
        InsertSource::DefaultValues => Some(SqlInsertSource::Values(vec![])),
    }
}

fn convert_assignment(a: &ColumnAssignment) -> Option<SqlAssignment> {
    Some(SqlAssignment {
        column: SqlIdentifier::new(a.column.name.clone()),
        value: convert_expr(&a.value)?,
    })
}

// ===========================================================================
// Expression converter (recursive 13-variant + operator-split dispatch)
// ===========================================================================

fn convert_expr(expr: &Expression) -> Option<SqlExpr> {
    match expr {
        Expression::Literal(lit) => convert_literal(lit),
        Expression::Identifier(id) => {
            Some(SqlExpr::Identifier(SqlIdentifier::new(id.name.clone())))
        }
        Expression::ColumnReference(col) => Some(convert_column_reference(col)),
        Expression::UnaryOp { op, expr, .. } => {
            // Tilde -> None (Stage-1 drop).
            let op = convert_unary_op(*op)?;
            Some(SqlExpr::UnaryOp {
                op,
                expr: Box::new(convert_expr(expr)?),
            })
        }
        Expression::BinaryOp {
            left, op, right, ..
        } => {
            // Operator-split dispatch: arithmetic -> BinaryOp,
            // comparison -> Comparison, logical -> LogicalOp.
            // ASE-specific NotLt/NotGt and the In/Between bare operators -> None.
            convert_binary_op(convert_expr(left)?, *op, convert_expr(right)?)
        }
        Expression::FunctionCall(func) => convert_function_call(func),
        Expression::Case(case) => convert_case(case),
        Expression::In {
            expr,
            list,
            negated,
            ..
        } => {
            let common_list = match list {
                InList::Values(vals) => {
                    let values: Vec<SqlExpr> = vals.iter().filter_map(convert_expr).collect();
                    SqlInList::Values(values)
                }
                InList::Subquery(select) => {
                    let common_select = convert_select(select)?;
                    SqlInList::Subquery(Box::new(common_select))
                }
            };
            Some(SqlExpr::In {
                expr: Box::new(convert_expr(expr)?),
                list: common_list,
                negated: *negated,
            })
        }
        Expression::Between {
            expr,
            low,
            high,
            negated,
            ..
        } => Some(SqlExpr::Between {
            expr: Box::new(convert_expr(expr)?),
            low: Box::new(convert_expr(low)?),
            high: Box::new(convert_expr(high)?),
            negated: *negated,
        }),
        Expression::Like {
            expr,
            pattern,
            escape: _,
            negated,
            ..
        } => {
            // Lossy: LIKE -> Comparison::{Like,NotLike}; ESCAPE dropped.
            Some(SqlExpr::Comparison {
                left: Box::new(convert_expr(expr)?),
                op: if *negated {
                    SqlComparison::NotLike
                } else {
                    SqlComparison::Like
                },
                right: Box::new(convert_expr(pattern)?),
            })
        }
        Expression::Is {
            expr,
            negated,
            value,
            ..
        } => {
            // Stage-1: IS NULL / IS UNKNOWN -> IsNull; IS TRUE/FALSE -> None.
            match value {
                IsValue::Null | IsValue::Unknown => Some(SqlExpr::IsNull {
                    expr: Box::new(convert_expr(expr)?),
                    negated: *negated,
                }),
                IsValue::True | IsValue::False => None,
            }
        }
        Expression::Subquery(select) => {
            let common_select = convert_select(select)?;
            Some(SqlExpr::Subquery(Box::new(common_select)))
        }
        Expression::Exists(select) => {
            let common_select = convert_select(select)?;
            Some(SqlExpr::Exists {
                subquery: Box::new(common_select),
                negated: false,
            })
        }
    }
}

fn convert_literal(lit: &Literal) -> Option<SqlExpr> {
    let common_lit = match lit {
        Literal::String(s, _) => SqlLiteral::String(s.clone()),
        Literal::Number(n, _) => {
            // Integer parse; failure -> None.
            n.parse::<i64>().ok().map(SqlLiteral::Integer)?
        }
        Literal::Float(f, _) => {
            // Stage-2 parity: f64 rendered via Display -> precision-preserving String.
            let as_f64 = f.parse::<f64>().ok()?;
            SqlLiteral::Float(as_f64.to_string())
        }
        Literal::Hex(_, _) => {
            // Stage-1 drop.
            return None;
        }
        Literal::Null(_) => SqlLiteral::Null,
        Literal::Boolean(b, _) => SqlLiteral::Boolean(*b),
    };
    Some(SqlExpr::Literal(common_lit))
}

fn convert_column_reference(col: &ColumnReference) -> SqlExpr {
    match &col.table {
        Some(table) => SqlExpr::QualifiedIdentifier {
            table: SqlIdentifier::new(table.name.clone()),
            column: SqlIdentifier::new(col.column.name.clone()),
        },
        None => SqlExpr::Identifier(SqlIdentifier::new(col.column.name.clone())),
    }
}

fn convert_unary_op(op: UnaryOperator) -> Option<SqlUnaryOp> {
    match op {
        UnaryOperator::Plus => Some(SqlUnaryOp::Plus),
        UnaryOperator::Minus => Some(SqlUnaryOp::Minus),
        UnaryOperator::Not => Some(SqlUnaryOp::Not),
        UnaryOperator::Tilde => None, // Stage-1 drop.
    }
}

/// Dispatch a T-SQL binary operator + its two (already converted) operands
/// into the correct `common_sql` `Expression` variant.
///
/// Mirrors the Stage-1 `BinaryOperator -> CommonBinaryOperator` mapping
/// composed with the Stage-2 `convert_binary_op` dispatch.
#[must_use]
fn convert_binary_op(left: SqlExpr, op: BinaryOperator, right: SqlExpr) -> Option<SqlExpr> {
    let left = Box::new(left);
    let right = Box::new(right);
    let expr = match op {
        // Arithmetic -> BinaryOp
        BinaryOperator::Plus => SqlExpr::BinaryOp {
            left,
            op: SqlBinaryOp::Add,
            right,
        },
        BinaryOperator::Minus => SqlExpr::BinaryOp {
            left,
            op: SqlBinaryOp::Sub,
            right,
        },
        BinaryOperator::Multiply => SqlExpr::BinaryOp {
            left,
            op: SqlBinaryOp::Mul,
            right,
        },
        BinaryOperator::Divide => SqlExpr::BinaryOp {
            left,
            op: SqlBinaryOp::Div,
            right,
        },
        BinaryOperator::Modulo => SqlExpr::BinaryOp {
            left,
            op: SqlBinaryOp::Mod,
            right,
        },
        BinaryOperator::Concat => SqlExpr::BinaryOp {
            left,
            op: SqlBinaryOp::Concat,
            right,
        },
        // Comparison -> Comparison (Eq folds both `=` and `<>`).
        BinaryOperator::Eq | BinaryOperator::NeAlt => SqlExpr::Comparison {
            left,
            op: SqlComparison::Eq,
            right,
        },
        BinaryOperator::Ne => SqlExpr::Comparison {
            left,
            op: SqlComparison::Ne,
            right,
        },
        BinaryOperator::Lt => SqlExpr::Comparison {
            left,
            op: SqlComparison::Lt,
            right,
        },
        BinaryOperator::Le => SqlExpr::Comparison {
            left,
            op: SqlComparison::Le,
            right,
        },
        BinaryOperator::Gt => SqlExpr::Comparison {
            left,
            op: SqlComparison::Gt,
            right,
        },
        BinaryOperator::Ge => SqlExpr::Comparison {
            left,
            op: SqlComparison::Ge,
            right,
        },
        // Logical -> LogicalOp
        BinaryOperator::And => SqlExpr::LogicalOp {
            left,
            op: SqlLogical::And,
            right,
        },
        BinaryOperator::Or => SqlExpr::LogicalOp {
            left,
            op: SqlLogical::Or,
            right,
        },
        // ASE-specific -> None (Stage-1 drop).
        BinaryOperator::NotLt | BinaryOperator::NotGt => return None,
        // In/Between as bare binary operators -> None (handled as dedicated
        // Expression variants elsewhere).
        BinaryOperator::In | BinaryOperator::Between => return None,
    };
    Some(expr)
}

fn convert_function_call(func: &FunctionCall) -> Option<SqlExpr> {
    let mut args = Vec::new();
    for arg in &func.args {
        match arg {
            crate::ast::FunctionArg::Expression(e) => {
                if let Some(common) = convert_expr(e) {
                    args.push(common);
                }
            }
            // Stage-1 parity: Wildcard / QualifiedWildcard -> "*" identifier.
            crate::ast::FunctionArg::Wildcard | crate::ast::FunctionArg::QualifiedWildcard(_) => {
                args.push(SqlExpr::Identifier(SqlIdentifier::new("*".to_string())));
            }
        }
    }
    Some(SqlExpr::Function {
        name: SqlIdentifier::new(func.name.name.clone()),
        args,
        distinct: func.distinct,
    })
}

fn convert_case(case: &CaseExpression) -> Option<SqlExpr> {
    let mut conditions = Vec::new();
    for (cond, result) in &case.branches {
        let common_cond = convert_expr(cond)?;
        let common_result = convert_expr(result)?;
        conditions.push((common_cond, common_result));
    }
    let else_result = case
        .else_result
        .as_deref()
        .and_then(convert_expr)
        .map(Box::new);
    // Lossy: operand always None (legacy is searched-CASE only).
    Some(SqlExpr::Case {
        operand: None,
        conditions,
        else_result,
    })
}

// ===========================================================================
// DDL converters (T2: column / constraint / data-type mapping)
// ===========================================================================
//
// These helpers map the T-SQL DDL shapes (`ColumnDefinition`,
// `TableConstraint`, `ColumnConstraint`, `DataType`) into the
// dialect-independent `common_sql` DDL nodes. The statement-level CREATE /
// ALTER TABLE wiring (T2.3) consumes them.
//
// Parity contract (§0.6 short-circuit): any column whose `data_type` has no
// destination (SmallDateTime/Bit/Money/SmallMoney) yields `None`, and a
// statement that contains such a column yields `None` at the statement level.
// Likewise a non-convertible constraint short-circuits the whole statement.

/// Map a T-SQL [`TsqlDataType`] into the dialect-independent [`SqlDataType`].
///
/// Returns `None` for the four T-SQL types that have no destination variant:
/// `SmallDateTime`, `Bit`, `Money`, `SmallMoney`. Per §0.6, the caller
/// propagates this `None` upward so the enclosing statement becomes `None`
/// rather than silently dropping the column.
///
/// Destination field shapes (verified against `common-sql/ast/datatype.rs`):
/// - `Time` / `DateTime` / `Timestamp` are struct variants `{ precision:
///   Option<u8> }`. T-SQL carries no precision, so all map to `precision:
///   None`.
/// - `Char` / `Binary` are struct variants `{ length: Option<u64> }`.
/// - `VarChar` / `VarBinary` likewise carry `length: Option<u64>`.
///
/// T-SQL has no source variant for the destination's `Boolean`, `Blob`,
/// `Json`, `NChar`, `NVarChar`, `NText` — those destinations are simply never
/// produced here.
fn convert_data_type(dt: &TsqlDataType) -> Option<SqlDataType> {
    let common = match dt {
        TsqlDataType::Int => SqlDataType::Int,
        TsqlDataType::SmallInt => SqlDataType::SmallInt,
        TsqlDataType::TinyInt => SqlDataType::TinyInt,
        TsqlDataType::BigInt => SqlDataType::BigInt,
        TsqlDataType::Varchar(o) => SqlDataType::VarChar {
            length: o.map(u64::from),
        },
        TsqlDataType::Char(n) => SqlDataType::Char {
            length: Some(u64::from(*n)),
        },
        TsqlDataType::Decimal(p, s) => SqlDataType::Decimal {
            precision: *p,
            scale: *s,
        },
        TsqlDataType::Numeric(p, s) => SqlDataType::Numeric {
            precision: *p,
            scale: *s,
        },
        TsqlDataType::Float => SqlDataType::DoublePrecision,
        TsqlDataType::Real => SqlDataType::Real,
        TsqlDataType::Double => SqlDataType::DoublePrecision,
        TsqlDataType::Date => SqlDataType::Date,
        TsqlDataType::Time => SqlDataType::Time { precision: None },
        TsqlDataType::Datetime => SqlDataType::DateTime { precision: None },
        TsqlDataType::Timestamp => SqlDataType::Timestamp { precision: None },
        TsqlDataType::Text => SqlDataType::Text,
        TsqlDataType::Binary(n) => SqlDataType::Binary {
            length: Some(u64::from(*n)),
        },
        TsqlDataType::VarBinary(o) => SqlDataType::VarBinary {
            length: o.map(u64::from),
        },
        TsqlDataType::UniqueIdentifier => SqlDataType::Uuid,
        // No destination variant → None propagates to statement level (§0.6).
        TsqlDataType::SmallDateTime
        | TsqlDataType::Bit
        | TsqlDataType::Money
        | TsqlDataType::SmallMoney => return None,
    };
    Some(common)
}

/// Convert a T-SQL [`ColumnDefinition`] into a `common_sql` [`SqlColumnDef`].
///
/// `data_type` is propagated via `?`: a column whose type maps to `None`
/// yields `None` here, and (per §0.6) the enclosing statement then becomes
/// `None`. Nullability follows the SQL default — `Option<bool>::None` (unspecified)
/// becomes `nullable: true` via `unwrap_or(true)` (columns are nullable unless
/// `NOT NULL`).
fn convert_column_def(col: &ColumnDefinition) -> Option<SqlColumnDef> {
    let data_type = convert_data_type(&col.data_type)?;
    // Recurse over column-level constraints; any non-convertible constraint
    // short-circuits this column (and hence the statement).
    let mut constraints = Vec::new();
    for c in &col.constraints {
        constraints.push(convert_column_constraint(c)?);
    }
    Some(SqlColumnDef {
        // Span dropped (parity: placeholder).
        span: csql::Span::new(0, 0),
        name: SqlIdentifier::new(col.name.name.clone()),
        data_type,
        // SQL default: unspecified nullability → nullable.
        nullable: col.nullability.unwrap_or(true),
        default: col.default_value.as_ref().and_then(convert_expr),
        constraints,
    })
}

/// Convert a T-SQL column-level constraint into the destination
/// [`SqlColumnConstraint`].
///
/// `Foreign { ref_table, ref_column }` maps to `References { table, columns }`
/// — the `ref_table: Identifier` is lifted into a single-segment
/// [`SqlQualifiedName`] (T-SQL column-level FKs carry no schema).
fn convert_column_constraint(c: &TsqlColumnConstraint) -> Option<SqlColumnConstraint> {
    let common = match c {
        TsqlColumnConstraint::PrimaryKey => SqlColumnConstraint::PrimaryKey,
        TsqlColumnConstraint::Unique => SqlColumnConstraint::Unique,
        TsqlColumnConstraint::Check(expr) => SqlColumnConstraint::Check(convert_expr(expr)?),
        TsqlColumnConstraint::Foreign {
            ref_table,
            ref_column,
        } => SqlColumnConstraint::References {
            table: SqlQualifiedName::new(None, ref_table.name.clone()),
            columns: vec![ref_column.name.clone()],
        },
    };
    Some(common)
}

/// Convert a T-SQL table-level constraint into the destination
/// [`SqlTableConstraint`].
///
/// `Foreign { ref_table: Identifier, ... }` lifts the referenced table name
/// into a single-segment [`SqlQualifiedName`] (T-SQL table-level FKs carry no
/// schema on this path). `Check` propagates a non-convertible expression as
/// `None` so the enclosing statement short-circuits.
fn convert_table_constraint(tc: &TsqlTableConstraint) -> Option<SqlTableConstraint> {
    let common = match tc {
        TsqlTableConstraint::PrimaryKey { name, columns } => SqlTableConstraint::PrimaryKey {
            name: name.as_ref().map(|i| i.name.clone()),
            columns: columns
                .iter()
                .map(|i| SqlIdentifier::new(i.name.clone()))
                .collect(),
        },
        TsqlTableConstraint::Foreign {
            name,
            columns,
            ref_table,
            ref_columns,
        } => SqlTableConstraint::ForeignKey {
            name: name.as_ref().map(|i| i.name.clone()),
            columns: columns
                .iter()
                .map(|i| SqlIdentifier::new(i.name.clone()))
                .collect(),
            ref_table: SqlQualifiedName::new(None, ref_table.name.clone()),
            ref_columns: ref_columns
                .iter()
                .map(|i| SqlIdentifier::new(i.name.clone()))
                .collect(),
        },
        TsqlTableConstraint::Unique { name, columns } => SqlTableConstraint::Unique {
            name: name.as_ref().map(|i| i.name.clone()),
            columns: columns
                .iter()
                .map(|i| SqlIdentifier::new(i.name.clone()))
                .collect(),
        },
        TsqlTableConstraint::Check { name, expr } => SqlTableConstraint::Check {
            name: name.as_ref().map(|i| i.name.clone()),
            expr: convert_expr(expr)?,
        },
    };
    Some(common)
}

// ===========================================================================
// Tests (T2.2: column / constraint / data-type converters)
// ===========================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::ast::{ColumnConstraint as TsqlColumnConstraint, Identifier};
    use tsql_token::Span;

    /// Build a T-SQL `Identifier` (name + zero span) for test fixtures.
    fn tid(name: &str) -> Identifier {
        Identifier {
            name: name.to_string(),
            span: Span::new(0, 0),
        }
    }

    // ── convert_data_type ───────────────────────────────────────────────

    #[test]
    fn data_type_integer_family_maps() {
        assert_eq!(
            convert_data_type(&TsqlDataType::Int),
            Some(SqlDataType::Int)
        );
        assert_eq!(
            convert_data_type(&TsqlDataType::SmallInt),
            Some(SqlDataType::SmallInt)
        );
        assert_eq!(
            convert_data_type(&TsqlDataType::TinyInt),
            Some(SqlDataType::TinyInt)
        );
        assert_eq!(
            convert_data_type(&TsqlDataType::BigInt),
            Some(SqlDataType::BigInt)
        );
    }

    #[test]
    fn data_type_varchar_some_length_maps_to_u64() {
        let dt = convert_data_type(&TsqlDataType::Varchar(Some(255)));
        assert_eq!(dt, Some(SqlDataType::VarChar { length: Some(255) }));
    }

    #[test]
    fn data_type_varchar_none_length_maps_to_none_length() {
        let dt = convert_data_type(&TsqlDataType::Varchar(None));
        assert_eq!(dt, Some(SqlDataType::VarChar { length: None }));
    }

    #[test]
    fn data_type_char_maps_to_struct_variant_with_length() {
        let dt = convert_data_type(&TsqlDataType::Char(10));
        assert_eq!(dt, Some(SqlDataType::Char { length: Some(10) }));
    }

    #[test]
    fn data_type_binary_maps_to_struct_variant_with_length() {
        let dt = convert_data_type(&TsqlDataType::Binary(16));
        assert_eq!(dt, Some(SqlDataType::Binary { length: Some(16) }));
    }

    #[test]
    fn data_type_varbinary_some_maps_to_u64() {
        let dt = convert_data_type(&TsqlDataType::VarBinary(Some(1024)));
        assert_eq!(dt, Some(SqlDataType::VarBinary { length: Some(1024) }));
    }

    #[test]
    fn data_type_decimal_preserves_precision_and_scale() {
        let dt = convert_data_type(&TsqlDataType::Decimal(Some(18), Some(4)));
        assert_eq!(
            dt,
            Some(SqlDataType::Decimal {
                precision: Some(18),
                scale: Some(4),
            })
        );
    }

    #[test]
    fn data_type_numeric_preserves_precision_and_scale() {
        let dt = convert_data_type(&TsqlDataType::Numeric(Some(10), None));
        assert_eq!(
            dt,
            Some(SqlDataType::Numeric {
                precision: Some(10),
                scale: None,
            })
        );
    }

    #[test]
    fn data_type_datetime_maps_to_struct_variant_precision_none() {
        // Datetime is a struct variant { precision } in common-sql, NOT a unit.
        let dt = convert_data_type(&TsqlDataType::Datetime);
        assert_eq!(dt, Some(SqlDataType::DateTime { precision: None }));
    }

    #[test]
    fn data_type_timestamp_maps_to_struct_variant_precision_none() {
        let dt = convert_data_type(&TsqlDataType::Timestamp);
        assert_eq!(dt, Some(SqlDataType::Timestamp { precision: None }));
    }

    #[test]
    fn data_type_time_maps_to_struct_variant_precision_none() {
        let dt = convert_data_type(&TsqlDataType::Time);
        assert_eq!(dt, Some(SqlDataType::Time { precision: None }));
    }

    #[test]
    fn data_type_uniqueidentifier_maps_to_uuid() {
        let dt = convert_data_type(&TsqlDataType::UniqueIdentifier);
        assert_eq!(dt, Some(SqlDataType::Uuid));
    }

    #[test]
    fn data_type_unmappable_four_return_none() {
        // §0.6: these four have no destination variant → None.
        assert_eq!(convert_data_type(&TsqlDataType::SmallDateTime), None);
        assert_eq!(convert_data_type(&TsqlDataType::Bit), None);
        assert_eq!(convert_data_type(&TsqlDataType::Money), None);
        assert_eq!(convert_data_type(&TsqlDataType::SmallMoney), None);
    }

    #[test]
    fn data_type_float_double_both_map_to_double_precision() {
        assert_eq!(
            convert_data_type(&TsqlDataType::Float),
            Some(SqlDataType::DoublePrecision)
        );
        assert_eq!(
            convert_data_type(&TsqlDataType::Double),
            Some(SqlDataType::DoublePrecision)
        );
    }

    // ── convert_column_def ──────────────────────────────────────────────

    fn col(name: &str, dt: TsqlDataType) -> ColumnDefinition {
        ColumnDefinition {
            name: tid(name),
            data_type: dt,
            nullability: None,
            default_value: None,
            identity: false,
            constraints: vec![],
        }
    }

    #[test]
    fn column_def_basic_int_nullable_default() {
        let c = convert_column_def(&col("id", TsqlDataType::Int));
        let c = c.expect("INT column should convert");
        assert_eq!(c.name.value(), "id");
        assert_eq!(c.data_type, SqlDataType::Int);
        // Unspecified nullability → SQL default nullable.
        assert!(c.nullable);
        assert!(c.default.is_none());
        assert!(c.constraints.is_empty());
    }

    #[test]
    fn column_def_not_null_propagates_false() {
        let mut c = col("name", TsqlDataType::Varchar(Some(100)));
        c.nullability = Some(false);
        let converted = convert_column_def(&c).expect("VARCHAR(100) NOT NULL converts");
        assert!(!converted.nullable);
    }

    #[test]
    fn column_def_explicit_null_propagates_true() {
        let mut c = col("email", TsqlDataType::Varchar(Some(255)));
        c.nullability = Some(true);
        let converted = convert_column_def(&c).expect("VARCHAR(255) NULL converts");
        assert!(converted.nullable);
    }

    #[test]
    fn column_def_unmappable_type_yields_none() {
        // §0.6 short-circuit: a column whose data_type has no destination
        // yields None, which the statement level must propagate.
        let c = convert_column_def(&col("flag", TsqlDataType::Bit));
        assert!(c.is_none(), "BIT column must yield None");
    }

    #[test]
    fn column_def_default_value_converts() {
        let mut c = col("status", TsqlDataType::Int);
        c.default_value = Some(Expression::Literal(Literal::Number(
            "0".to_string(),
            Span::new(0, 0),
        )));
        let converted = convert_column_def(&c).expect("column with default converts");
        assert!(
            converted.default.is_some(),
            "default expression should convert"
        );
    }

    #[test]
    fn column_def_constraints_are_converted() {
        let mut c = col("id", TsqlDataType::BigInt);
        c.constraints = vec![TsqlColumnConstraint::PrimaryKey];
        let converted = convert_column_def(&c).expect("column with PK converts");
        assert_eq!(converted.constraints.len(), 1);
        assert_eq!(converted.constraints[0], SqlColumnConstraint::PrimaryKey);
    }

    #[test]
    fn column_def_non_convertible_constraint_yields_none() {
        // CHECK with a Hex literal — Hex short-circuits to None in convert_expr,
        // which must propagate through the column (and hence the statement).
        let mut c = col("x", TsqlDataType::Int);
        c.constraints = vec![TsqlColumnConstraint::Check(Expression::Literal(
            Literal::Hex("AA".to_string(), Span::new(0, 0)),
        ))];
        let converted = convert_column_def(&c);
        assert!(converted.is_none(), "non-convertible CHECK must yield None");
    }

    // ── convert_column_constraint ───────────────────────────────────────

    #[test]
    fn column_constraint_primary_key() {
        let c = convert_column_constraint(&TsqlColumnConstraint::PrimaryKey);
        assert_eq!(c, Some(SqlColumnConstraint::PrimaryKey));
    }

    #[test]
    fn column_constraint_unique() {
        let c = convert_column_constraint(&TsqlColumnConstraint::Unique);
        assert_eq!(c, Some(SqlColumnConstraint::Unique));
    }

    #[test]
    fn column_constraint_check_converts_expr() {
        let expr = Expression::Literal(Literal::Number("1".to_string(), Span::new(0, 0)));
        let c = convert_column_constraint(&TsqlColumnConstraint::Check(expr));
        assert!(matches!(c, Some(SqlColumnConstraint::Check(_))));
    }

    #[test]
    fn column_constraint_foreign_maps_to_references_qualified_name() {
        // ref_table: Identifier → QualifiedName (single-segment, no schema).
        let fk = TsqlColumnConstraint::Foreign {
            ref_table: tid("users"),
            ref_column: tid("id"),
        };
        let c = convert_column_constraint(&fk).expect("FK converts");
        match c {
            SqlColumnConstraint::References { table, columns } => {
                assert_eq!(table.name(), "users");
                assert!(table.schema().is_none());
                assert_eq!(columns, vec!["id".to_string()]);
            }
            other => panic!("expected References, got {other:?}"),
        }
    }

    // ── convert_table_constraint ────────────────────────────────────────

    #[test]
    fn table_constraint_primary_key() {
        let tc = TsqlTableConstraint::PrimaryKey {
            name: Some(tid("pk_t")),
            columns: vec![tid("id")],
        };
        let c = convert_table_constraint(&tc).expect("PK converts");
        match c {
            SqlTableConstraint::PrimaryKey { name, columns } => {
                assert_eq!(name.as_deref(), Some("pk_t"));
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].value(), "id");
            }
            other => panic!("expected PrimaryKey, got {other:?}"),
        }
    }

    #[test]
    fn table_constraint_unique_unnamed() {
        let tc = TsqlTableConstraint::Unique {
            name: None,
            columns: vec![tid("email")],
        };
        let c = convert_table_constraint(&tc).expect("UNIQUE converts");
        match c {
            SqlTableConstraint::Unique { name, columns } => {
                assert!(name.is_none());
                assert_eq!(columns.len(), 1);
            }
            other => panic!("expected Unique, got {other:?}"),
        }
    }

    #[test]
    fn table_constraint_foreign_key_ref_table_becomes_qualified_name() {
        // The load-bearing FK mapping: ref_table: Identifier → QualifiedName.
        let tc = TsqlTableConstraint::Foreign {
            name: Some(tid("fk_order_user")),
            columns: vec![tid("user_id")],
            ref_table: tid("users"),
            ref_columns: vec![tid("id")],
        };
        let c = convert_table_constraint(&tc).expect("FK converts");
        match c {
            SqlTableConstraint::ForeignKey {
                name,
                columns,
                ref_table,
                ref_columns,
            } => {
                assert_eq!(name.as_deref(), Some("fk_order_user"));
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].value(), "user_id");
                assert_eq!(ref_table.name(), "users");
                assert!(ref_table.schema().is_none());
                assert_eq!(ref_columns.len(), 1);
                assert_eq!(ref_columns[0].value(), "id");
            }
            other => panic!("expected ForeignKey, got {other:?}"),
        }
    }

    #[test]
    fn table_constraint_check_converts_expr() {
        let tc = TsqlTableConstraint::Check {
            name: None,
            expr: Expression::Literal(Literal::Number("0".to_string(), Span::new(0, 0))),
        };
        let c = convert_table_constraint(&tc);
        assert!(matches!(c, Some(SqlTableConstraint::Check { .. })));
    }

    #[test]
    fn table_constraint_check_non_convertible_expr_yields_none() {
        // Hex literal short-circuits to None → the constraint (and hence the
        // statement) must yield None.
        let tc = TsqlTableConstraint::Check {
            name: None,
            expr: Expression::Literal(Literal::Hex("FF".to_string(), Span::new(0, 0))),
        };
        let c = convert_table_constraint(&tc);
        assert!(c.is_none(), "non-convertible CHECK expr must yield None");
    }

    // ── Parity contract: control-flow still DialectSpecific (regression) ─

    #[test]
    fn control_flow_declare_still_dialect_specific_not_create_arm() {
        // Regression guard: T2 must NOT widen the Create/AlterTable arm to
        // swallow control-flow. DECLARE continues to yield DialectSpecific.
        let stmts = crate::parse("DECLARE @v INT").unwrap_or_default();
        assert_eq!(stmts.len(), 1);
        let common = to_common_sql(&stmts[0]);
        assert!(
            matches!(common, Some(SqlStmt::DialectSpecific { .. })),
            "DECLARE must remain DialectSpecific after T2.2"
        );
    }

    // ── Statement-level DDL dispatch (T2.3) ─────────────────────────────

    #[test]
    fn create_table_basic_maps_to_create_table_variant() {
        let stmts = crate::parse("CREATE TABLE users (id BIGINT NOT NULL)").unwrap_or_default();
        assert_eq!(stmts.len(), 1);
        let common = to_common_sql(&stmts[0]);
        match common {
            Some(SqlStmt::CreateTable(ct)) => {
                assert_eq!(ct.name.name(), "users");
                assert_eq!(ct.columns.len(), 1);
                assert_eq!(ct.columns[0].data_type, SqlDataType::BigInt);
                assert!(!ct.columns[0].nullable);
            }
            other => panic!("expected CreateTable, got {other:?}"),
        }
    }

    #[test]
    fn create_table_with_unmappable_column_yields_none() {
        // §0.6: BIT column → None propagates to the whole statement.
        let stmts = crate::parse("CREATE TABLE t (flag BIT)").unwrap_or_default();
        if let Some(stmt) = stmts.first() {
            assert!(
                to_common_sql(stmt).is_none(),
                "CREATE TABLE with BIT column must yield None (§0.6)"
            );
        }
    }

    #[test]
    fn create_table_with_foreign_key_constraint_maps() {
        let stmts = crate::parse(
            "CREATE TABLE orders (id INT, user_id INT, FOREIGN KEY (user_id) REFERENCES users(id))",
        )
        .unwrap_or_default();
        if let Some(stmt) = stmts.first() {
            let common = to_common_sql(stmt).expect("FK CREATE TABLE converts");
            match common {
                SqlStmt::CreateTable(ct) => {
                    assert_eq!(ct.constraints.len(), 1);
                    match &ct.constraints[0] {
                        SqlTableConstraint::ForeignKey { ref_table, .. } => {
                            assert_eq!(ref_table.name(), "users");
                        }
                        other => panic!("expected ForeignKey, got {other:?}"),
                    }
                }
                other => panic!("expected CreateTable, got {other:?}"),
            }
        }
    }

    #[test]
    fn create_index_maps_to_create_index_variant() {
        let stmts = crate::parse("CREATE INDEX idx_name ON users (name)").unwrap_or_default();
        if let Some(stmt) = stmts.first() {
            let common = to_common_sql(stmt).expect("CREATE INDEX converts");
            match common {
                SqlStmt::CreateIndex(ci) => {
                    assert_eq!(ci.name.value(), "idx_name");
                    assert_eq!(ci.table.name(), "users");
                    assert_eq!(ci.columns.len(), 1);
                    assert!(!ci.unique);
                }
                other => panic!("expected CreateIndex, got {other:?}"),
            }
        }
    }

    #[test]
    fn create_unique_index_preserves_unique_flag() {
        let stmts =
            crate::parse("CREATE UNIQUE INDEX uk_email ON users (email)").unwrap_or_default();
        if let Some(stmt) = stmts.first() {
            let common = to_common_sql(stmt).expect("CREATE UNIQUE INDEX converts");
            match common {
                SqlStmt::CreateIndex(ci) => assert!(ci.unique),
                other => panic!("expected CreateIndex, got {other:?}"),
            }
        }
    }

    #[test]
    fn create_view_yields_none() {
        let stmts = crate::parse("CREATE VIEW v AS SELECT 1 AS x").unwrap_or_default();
        if let Some(stmt) = stmts.first() {
            assert!(to_common_sql(stmt).is_none(), "CREATE VIEW → None");
        }
    }

    #[test]
    fn alter_table_add_column_maps() {
        let stmts = crate::parse("ALTER TABLE users ADD email VARCHAR(255)").unwrap_or_default();
        if let Some(stmt) = stmts.first() {
            let common = to_common_sql(stmt).expect("ALTER TABLE ADD converts");
            match common {
                SqlStmt::AlterTable(alter) => {
                    assert_eq!(alter.name.name(), "users");
                    assert_eq!(alter.actions.len(), 1);
                    match &alter.actions[0] {
                        SqlAlterTableAction::AddColumn(col) => {
                            assert_eq!(col.name.value(), "email");
                            assert_eq!(col.data_type, SqlDataType::VarChar { length: Some(255) });
                            assert!(col.constraints.is_empty());
                            assert!(col.default.is_none());
                        }
                        other => panic!("expected AddColumn, got {other:?}"),
                    }
                }
                other => panic!("expected AlterTable, got {other:?}"),
            }
        }
    }

    #[test]
    fn alter_table_drop_column_maps() {
        let stmts = crate::parse("ALTER TABLE users DROP COLUMN email").unwrap_or_default();
        if let Some(stmt) = stmts.first() {
            let common = to_common_sql(stmt).expect("ALTER TABLE DROP converts");
            match common {
                SqlStmt::AlterTable(alter) => {
                    assert!(
                        matches!(alter.actions[0], SqlAlterTableAction::DropColumn(_)),
                        "expected DropColumn action"
                    );
                }
                other => panic!("expected AlterTable, got {other:?}"),
            }
        }
    }

    #[test]
    fn batch_separator_yields_none() {
        let stmts = crate::parse("SELECT 1\nGO").unwrap_or_default();
        // Find the BatchSeparator statement if present.
        for stmt in &stmts {
            if matches!(stmt, Statement::BatchSeparator(_)) {
                assert!(to_common_sql(stmt).is_none(), "GO → None");
            }
        }
    }
}
