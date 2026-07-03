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
//! * **`DialectSpecific → None`**: every statement variant that Stage-1 mapped
//!   to `CommonStatement::DialectSpecific` (control flow, CREATE, ALTER TABLE,
//!   EXEC, variable assignment) yields `None`, because the destination
//!   `Statement` has no escape hatch. `BatchSeparator` also yields `None`
//!   (Stage-1 returned `None` directly).
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
    self as csql, Assignment as SqlAssignment, BinaryOperator as SqlBinaryOp,
    ComparisonOperator as SqlComparison, DeleteStatement as SqlDeleteStatement,
    Expression as SqlExpr, GroupByClause, GroupByItem, Identifier as SqlIdentifier,
    InList as SqlInList, InsertSource as SqlInsertSource, InsertStatement as SqlInsertStatement,
    LimitClause as SqlLimitClause, Literal as SqlLiteral, LogicalOperator as SqlLogical,
    OrderByClause, OrderByItem as SqlOrderByItem, QualifiedName as SqlQualifiedName,
    SelectItem as SqlSelectItem, SelectStatement as SqlSelectStatement, SortDirection,
    Statement as SqlStmt, TableAlias, TableFactor as SqlTableFactor, UnaryOperator as SqlUnaryOp,
    UpdateStatement as SqlUpdateStatement,
};

use crate::ast::data_modification::Assignment as ColumnAssignment;
use crate::ast::{
    BinaryOperator, CaseExpression, ColumnReference, DeleteStatement, Expression, FromClause,
    FunctionCall, InList, InsertSource, InsertStatement, IsValue, LimitClause, Literal,
    OrderByItem, SelectItem, SelectStatement, Statement, TableReference, UnaryOperator,
    UpdateStatement,
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
        // Stage-1 mapped all of these to DialectSpecific (or None for
        // BatchSeparator); Stage-2 dropped DialectSpecific -> None.
        // Therefore the direct converter returns None uniformly here.
        Statement::Create(_)
        | Statement::AlterTable(_)
        | Statement::Declare(_)
        | Statement::Set(_)
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
        | Statement::Exec(_)
        | Statement::VariableAssignment(_)
        | Statement::BatchSeparator(_) => None,
    }
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
