//! Bridge from the legacy internal `Common*` AST to the standalone
//! `common_sql::ast` crate.
//!
//! This is the data-type slice of the `tsql_parser -> common_sql` conversion
//! bridge — the T0 prerequisite for the mysql-emitter migration (#147). The
//! `common-sql` crate was extracted from this module, so the two enums are
//! near-identical. The single impedance is `CommonDataType::Float`, which has
//! no exact `common_sql` counterpart (the crate models `REAL` and
//! `DOUBLE PRECISION` only); it maps to `DoublePrecision` with its precision
//! discarded, which is range-safe for typical values.

use common_sql::ast::{
    Assignment as SqlAssignment, BinaryOperator as SqlBinaryOperator,
    ComparisonOperator as SqlComparisonOperator, DataType as SqlDataType,
    DeleteStatement as SqlDeleteStatement, Expression as SqlExpression,
    Identifier as SqlIdentifier, InList as SqlInList, InsertSource as SqlInsertSource,
    InsertStatement as SqlInsertStatement, LimitClause as SqlLimitClause, Literal as SqlLiteral,
    LogicalOperator as SqlLogicalOperator, OrderByItem as SqlOrderByItem,
    QualifiedName as SqlQualifiedName, SelectItem as SqlSelectItem,
    SelectStatement as SqlSelectStatement, TableFactor as SqlTableFactor,
    UnaryOperator as SqlUnaryOperator, UpdateStatement as SqlUpdateStatement,
};

use crate::common::{
    CommonAssignment, CommonBinaryOperator, CommonCaseExpression, CommonColumnReference,
    CommonDataType, CommonDeleteStatement, CommonExpression, CommonFunctionCall, CommonIdentifier,
    CommonInList, CommonInsertSource, CommonInsertStatement, CommonLimitClause, CommonLiteral,
    CommonOrderByItem, CommonSelectItem, CommonSelectStatement, CommonStatement,
    CommonTableReference, CommonUnaryOperator, CommonUpdateStatement,
};

/// Convert a legacy [`CommonDataType`] into the standalone
/// [`common_sql::ast::DataType`].
///
/// `Float` is the only lossy case (see the module docs).
impl From<CommonDataType> for SqlDataType {
    fn from(dt: CommonDataType) -> Self {
        match dt {
            CommonDataType::TinyInt => SqlDataType::TinyInt,
            CommonDataType::SmallInt => SqlDataType::SmallInt,
            CommonDataType::Int => SqlDataType::Int,
            CommonDataType::BigInt => SqlDataType::BigInt,
            CommonDataType::Decimal { precision, scale } => {
                SqlDataType::Decimal { precision, scale }
            }
            CommonDataType::Numeric { precision, scale } => {
                SqlDataType::Numeric { precision, scale }
            }
            CommonDataType::Real => SqlDataType::Real,
            CommonDataType::DoublePrecision => SqlDataType::DoublePrecision,
            // common-sql has no FLOAT variant; collapse to DOUBLE PRECISION.
            CommonDataType::Float { .. } => SqlDataType::DoublePrecision,
            CommonDataType::Char { length } => SqlDataType::Char { length },
            CommonDataType::VarChar { length } => SqlDataType::VarChar { length },
            CommonDataType::Text => SqlDataType::Text,
            CommonDataType::NChar { length } => SqlDataType::NChar { length },
            CommonDataType::NVarChar { length } => SqlDataType::NVarChar { length },
            CommonDataType::Date => SqlDataType::Date,
            CommonDataType::Time { precision } => SqlDataType::Time { precision },
            CommonDataType::DateTime { precision } => SqlDataType::DateTime { precision },
            CommonDataType::Timestamp { precision } => SqlDataType::Timestamp { precision },
            CommonDataType::Binary { length } => SqlDataType::Binary { length },
            CommonDataType::VarBinary { length } => SqlDataType::VarBinary { length },
            CommonDataType::Blob => SqlDataType::Blob,
            CommonDataType::Boolean => SqlDataType::Boolean,
            CommonDataType::Uuid => SqlDataType::Uuid,
            CommonDataType::Json => SqlDataType::Json,
        }
    }
}

/// Convert a legacy [`CommonLiteral`] into the standalone
/// [`common_sql::ast::Literal`].
///
/// `Float(f64)` is rendered to a string via `Display`, matching common-sql's
/// precision-preserving `Float(String)` shape. Precision beyond `f64` is
/// already lost in `CommonLiteral`, so this is the best the bridge can do.
impl From<CommonLiteral> for SqlLiteral {
    fn from(lit: CommonLiteral) -> Self {
        match lit {
            CommonLiteral::String(s) => SqlLiteral::String(s),
            CommonLiteral::Integer(i) => SqlLiteral::Integer(i),
            CommonLiteral::Float(f) => SqlLiteral::Float(f.to_string()),
            CommonLiteral::Null => SqlLiteral::Null,
            CommonLiteral::Boolean(b) => SqlLiteral::Boolean(b),
        }
    }
}

/// Convert a legacy [`CommonIdentifier`] into the standalone
/// [`common_sql::ast::Identifier`] (unquoted).
impl From<CommonIdentifier> for SqlIdentifier {
    fn from(id: CommonIdentifier) -> Self {
        SqlIdentifier::new(id.name)
    }
}

/// Convert a legacy [`CommonUnaryOperator`] into the standalone
/// [`common_sql::ast::UnaryOperator`].
impl From<CommonUnaryOperator> for SqlUnaryOperator {
    fn from(op: CommonUnaryOperator) -> Self {
        match op {
            CommonUnaryOperator::Plus => SqlUnaryOperator::Plus,
            CommonUnaryOperator::Minus => SqlUnaryOperator::Minus,
            CommonUnaryOperator::Not => SqlUnaryOperator::Not,
        }
    }
}

// ---------------------------------------------------------------------------
// Expression bridge (T3): recursive 13-variant conversion.
// ---------------------------------------------------------------------------

/// Dispatch a legacy binary operator + its two operands into the correct
/// `common_sql` `Expression` variant.
///
/// The legacy side models *all* binary operators with a single
/// 14-variant [`CommonBinaryOperator`] enum. The destination splits these
/// across three enums living in three different `Expression` variants
/// (`BinaryOp` / `Comparison` / `LogicalOp`). A plain `From` on the operator
/// alone cannot select the wrapping `Expression` variant, so this helper
/// inspects the operator and emits the correct node shape.
///
/// The match is exhaustive over `CommonBinaryOperator`: adding a new legacy
/// variant forces this bridge to be updated in lockstep.
///
/// Spans are silently dropped (the destination `Expression` carries none).
#[must_use]
fn convert_binary_op(
    left: CommonExpression,
    op: CommonBinaryOperator,
    right: CommonExpression,
) -> SqlExpression {
    let left = Box::new(SqlExpression::from(left));
    let right = Box::new(SqlExpression::from(right));
    match op {
        // Arithmetic -> Expression::BinaryOp
        CommonBinaryOperator::Plus => SqlExpression::BinaryOp {
            left,
            op: SqlBinaryOperator::Add,
            right,
        },
        CommonBinaryOperator::Minus => SqlExpression::BinaryOp {
            left,
            op: SqlBinaryOperator::Sub,
            right,
        },
        CommonBinaryOperator::Multiply => SqlExpression::BinaryOp {
            left,
            op: SqlBinaryOperator::Mul,
            right,
        },
        CommonBinaryOperator::Divide => SqlExpression::BinaryOp {
            left,
            op: SqlBinaryOperator::Div,
            right,
        },
        CommonBinaryOperator::Modulo => SqlExpression::BinaryOp {
            left,
            op: SqlBinaryOperator::Mod,
            right,
        },
        CommonBinaryOperator::Concat => SqlExpression::BinaryOp {
            left,
            op: SqlBinaryOperator::Concat,
            right,
        },
        // Comparison -> Expression::Comparison
        CommonBinaryOperator::Eq => SqlExpression::Comparison {
            left,
            op: SqlComparisonOperator::Eq,
            right,
        },
        CommonBinaryOperator::Ne => SqlExpression::Comparison {
            left,
            op: SqlComparisonOperator::Ne,
            right,
        },
        CommonBinaryOperator::Lt => SqlExpression::Comparison {
            left,
            op: SqlComparisonOperator::Lt,
            right,
        },
        CommonBinaryOperator::Le => SqlExpression::Comparison {
            left,
            op: SqlComparisonOperator::Le,
            right,
        },
        CommonBinaryOperator::Gt => SqlExpression::Comparison {
            left,
            op: SqlComparisonOperator::Gt,
            right,
        },
        CommonBinaryOperator::Ge => SqlExpression::Comparison {
            left,
            op: SqlComparisonOperator::Ge,
            right,
        },
        // Logical -> Expression::LogicalOp
        CommonBinaryOperator::And => SqlExpression::LogicalOp {
            left,
            op: SqlLogicalOperator::And,
            right,
        },
        CommonBinaryOperator::Or => SqlExpression::LogicalOp {
            left,
            op: SqlLogicalOperator::Or,
            right,
        },
    }
}

/// Convert a legacy [`CommonColumnReference`] into the destination expression
/// shape: a qualifier produces `QualifiedIdentifier`, otherwise a bare
/// `Identifier`.
impl From<CommonColumnReference> for SqlExpression {
    fn from(col: CommonColumnReference) -> Self {
        match col.table {
            Some(table) => SqlExpression::QualifiedIdentifier {
                table: SqlIdentifier::new(table),
                column: SqlIdentifier::new(col.column),
            },
            None => SqlExpression::Identifier(SqlIdentifier::new(col.column)),
        }
    }
}

/// Convert a legacy [`CommonFunctionCall`] into a destination `Function` node.
///
/// The legacy `name: String` becomes a `common_sql` `Identifier`; `args`
/// recurse through [`From<CommonExpression>`]; `distinct` passes through.
impl From<CommonFunctionCall> for SqlExpression {
    fn from(call: CommonFunctionCall) -> Self {
        SqlExpression::Function {
            name: SqlIdentifier::new(call.name),
            args: call.args.into_iter().map(SqlExpression::from).collect(),
            distinct: call.distinct,
        }
    }
}

/// Convert a legacy [`CommonCaseExpression`] into a destination `Case` node.
///
/// Lossy design decision (recorded): the legacy CASE is searched-CASE only —
/// there is no simple-CASE operand on the source side, so the destination
/// `operand` field is always `None`. The source `branches` field is renamed to
/// `conditions`. `else_result` recurses.
impl From<CommonCaseExpression> for SqlExpression {
    fn from(case: CommonCaseExpression) -> Self {
        SqlExpression::Case {
            operand: None,
            conditions: case
                .branches
                .into_iter()
                .map(|(when, then)| (SqlExpression::from(when), SqlExpression::from(then)))
                .collect(),
            else_result: case.else_result.map(|e| Box::new(SqlExpression::from(*e))),
        }
    }
}

/// Convert a legacy [`CommonInList`] into the destination [`SqlInList`].
///
/// `Values` recurses element-wise; `Subquery` converts the inner select.
impl From<CommonInList> for SqlInList {
    fn from(list: CommonInList) -> Self {
        match list {
            CommonInList::Values(exprs) => {
                SqlInList::Values(exprs.into_iter().map(SqlExpression::from).collect())
            }
            CommonInList::Subquery(query) => {
                SqlInList::Subquery(Box::new(SqlSelectStatement::from(*query)))
            }
        }
    }
}

/// Convert a legacy [`CommonExpression`] into the standalone
/// `common_sql::ast::Expression`.
///
/// # Lossy mappings (recorded)
///
/// * **Span** is dropped on *every* variant — the destination `Expression`
///   has no span field. Any downstream assertion on offset/line info will not
///   round-trip.
/// * **`Like`** is folded into `Expression::Comparison` with operator
///   `ComparisonOperator::Like` / `NotLike`. The optional `ESCAPE` clause is
///   silently dropped (there is no place for it on the destination). The
///   destination's `ILike` / `NotILike` variants are **never** produced by
///   this bridge — the legacy side has no case-insensitive LIKE.
/// * **`Case`** always sets `operand = None` (see
///   [`From<CommonCaseExpression>`]).
/// * **`Subquery` / `Exists`** rename the source `query` field to the
///   destination `subquery` field.
///
/// # Destination variants never produced
///
/// The bridge is one-directional (legacy -> common_sql). The destination
/// `Expression` has variants with no legacy source and which are therefore
/// never emitted here: `Cast` (legacy has no CAST), and the `ILike` /
/// `NotILike` comparison operators. The exhaustiveness guard relies on
/// `match` compile-fail, which catches *new* legacy variants only — not
/// unmapped destination variants.
impl From<CommonExpression> for SqlExpression {
    fn from(expr: CommonExpression) -> Self {
        match expr {
            // Leaves (existing From impls).
            CommonExpression::Literal(lit) => SqlExpression::Literal(SqlLiteral::from(lit)),
            CommonExpression::Identifier(id) => SqlExpression::Identifier(SqlIdentifier::from(id)),
            // ColumnReference -> QualifiedIdentifier | Identifier.
            CommonExpression::ColumnReference(col) => SqlExpression::from(col),
            // UnaryOp reuses the leaf operator From.
            CommonExpression::UnaryOp { op, expr, .. } => SqlExpression::UnaryOp {
                op: SqlUnaryOperator::from(op),
                expr: Box::new(SqlExpression::from(*expr)),
            },
            // BinaryOp dispatches across the three destination variants.
            CommonExpression::BinaryOp {
                left, op, right, ..
            } => convert_binary_op(*left, op, *right),
            // FunctionCall (name String -> Identifier).
            CommonExpression::FunctionCall(call) => SqlExpression::from(call),
            // CASE: operand=None always.
            CommonExpression::Case(case) => SqlExpression::from(case),
            // IN: negated passes through, list recurses.
            CommonExpression::In {
                expr,
                list,
                negated,
                ..
            } => SqlExpression::In {
                expr: Box::new(SqlExpression::from(*expr)),
                list: SqlInList::from(list),
                negated,
            },
            // BETWEEN: bounds recurse, negated passes through.
            CommonExpression::Between {
                expr,
                low,
                high,
                negated,
                ..
            } => SqlExpression::Between {
                expr: Box::new(SqlExpression::from(*expr)),
                low: Box::new(SqlExpression::from(*low)),
                high: Box::new(SqlExpression::from(*high)),
                negated,
            },
            // LIKE -> Comparison::{Like,NotLike}; ESCAPE dropped.
            CommonExpression::Like {
                expr,
                pattern,
                escape: _,
                negated,
                ..
            } => SqlExpression::Comparison {
                left: Box::new(SqlExpression::from(*expr)),
                op: if negated {
                    SqlComparisonOperator::NotLike
                } else {
                    SqlComparisonOperator::Like
                },
                right: Box::new(SqlExpression::from(*pattern)),
            },
            // IS NULL: negated passes through.
            CommonExpression::IsNull { expr, negated, .. } => SqlExpression::IsNull {
                expr: Box::new(SqlExpression::from(*expr)),
                negated,
            },
            // Subquery: query field -> subquery field rename.
            CommonExpression::Subquery { query, .. } => {
                SqlExpression::Subquery(Box::new(SqlSelectStatement::from(*query)))
            }
            // Exists: query field -> subquery field rename, negated passes through.
            CommonExpression::Exists { query, negated, .. } => SqlExpression::Exists {
                subquery: Box::new(SqlSelectStatement::from(*query)),
                negated,
            },
        }
    }
}

/// Convert a legacy [`CommonTableReference`] into a destination
/// [`SqlTableFactor`].
///
/// * `Table { name, alias }` — the bare `String` name becomes a
///   [`SqlQualifiedName`] with no schema; the optional `String` alias becomes
///   a [`common_sql::ast::TableAlias`] with no column aliases.
/// * `Derived { subquery, alias }` — the subquery recurses through
///   [`From<CommonSelectStatement>`]; alias mapping is the same as `Table`.
///
/// Span is dropped (the destination `TableFactor` carries none).
impl From<CommonTableReference> for SqlTableFactor {
    fn from(ref_: CommonTableReference) -> Self {
        match ref_ {
            CommonTableReference::Table { name, alias, .. } => SqlTableFactor::Table {
                name: qualified_name_from_string(name),
                alias: alias.map(|a| common_sql::ast::TableAlias::new(a, vec![])),
            },
            CommonTableReference::Derived {
                subquery, alias, ..
            } => SqlTableFactor::Derived {
                subquery: Box::new(SqlSelectStatement::from(*subquery)),
                alias: alias.map(|a| common_sql::ast::TableAlias::new(a, vec![])),
            },
        }
    }
}

/// Collapse a legacy `Vec<CommonTableReference>` into the destination's
/// `Option<TableFactor>`.
///
/// # Lossy mapping (recorded)
///
/// The legacy FROM clause is a `Vec<CommonTableReference>` (comma-separated
/// tables in T-SQL), but the destination models FROM as a single
/// `Option<TableFactor>` (multi-table FROM is expressed via `Join` nodes). We
/// cannot safely flatten N>1 table references into one `TableFactor`:
///
/// * A `CROSS JOIN` would change semantics and is not what the legacy comma
///   list necessarily means, and
/// * no join condition can be synthesized from the legacy node (it carries
///   none).
///
/// Therefore the **first** element is taken and any additional elements are
/// **silently dropped**. This is lossy for multi-table FROM; the accepted
/// trade-off is documented here. An empty `Vec` yields `None`.
fn collapse_from(from: Vec<CommonTableReference>) -> Option<SqlTableFactor> {
    let mut iter = from.into_iter();
    iter.next().map(SqlTableFactor::from)
}

/// Convert a legacy [`CommonSelectStatement`] into the destination
/// [`SqlSelectStatement`].
///
/// This is the full T4 conversion (projection / FROM / WHERE / GROUP BY /
/// HAVING / ORDER BY / LIMIT). `WITH` (CTE) is always `None` — the legacy
/// node carries no CTE clause. Span is dropped.
///
/// # Lossy mapping (recorded)
///
/// `from: Vec<CommonTableReference>` is collapsed via [`collapse_from`] —
/// only the first table reference survives. See that function's docs.
/// Convert a legacy [`CommonSelectItem`] into the destination
/// [`SqlSelectItem`].
///
/// Span is not carried (destination variants hold none).
impl From<CommonSelectItem> for SqlSelectItem {
    fn from(item: CommonSelectItem) -> Self {
        match item {
            CommonSelectItem::Wildcard => SqlSelectItem::Wildcard,
            CommonSelectItem::QualifiedWildcard(table) => SqlSelectItem::QualifiedWildcard {
                table: SqlIdentifier::new(table),
            },
            CommonSelectItem::Expression(expr, alias) => SqlSelectItem::Expression {
                expr: SqlExpression::from(expr),
                alias: alias.map(SqlIdentifier::new),
            },
        }
    }
}

/// Convert a legacy [`CommonOrderByItem`] into the destination
/// [`SqlOrderByItem`].
///
/// `asc: bool` maps to an explicit
/// [`SortDirection`](common_sql::ast::SortDirection); the legacy node carries
/// no NULLS ordering, so `nulls` is `None`.
impl From<CommonOrderByItem> for SqlOrderByItem {
    fn from(item: CommonOrderByItem) -> Self {
        SqlOrderByItem {
            expr: SqlExpression::from(item.expr),
            direction: Some(if item.asc {
                common_sql::ast::SortDirection::Asc
            } else {
                common_sql::ast::SortDirection::Desc
            }),
            // Legacy carries no NULLS ordering -> None (DB default).
            nulls: None,
        }
    }
}

/// Convert a legacy [`CommonLimitClause`] into the destination
/// [`SqlLimitClause`].
///
/// Span is dropped (destination uses a zero placeholder).
impl From<CommonLimitClause> for SqlLimitClause {
    fn from(l: CommonLimitClause) -> Self {
        SqlLimitClause {
            span: common_sql::ast::Span::new(0, 0),
            limit: SqlExpression::from(l.limit),
            offset: l.offset.map(SqlExpression::from),
        }
    }
}

impl From<CommonSelectStatement> for SqlSelectStatement {
    fn from(sel: CommonSelectStatement) -> Self {
        use common_sql::ast::{GroupByClause, GroupByItem, OrderByClause};

        SqlSelectStatement {
            // Span dropped (destination field required but unused here).
            span: common_sql::ast::Span::new(0, 0),
            // Legacy has no WITH (CTE) -> always None.
            with: None,
            projection: sel.columns.into_iter().map(SqlSelectItem::from).collect(),
            from: collapse_from(sel.from),
            where_clause: sel.where_clause.map(SqlExpression::from),
            group_by: if sel.group_by.is_empty() {
                None
            } else {
                Some(GroupByClause {
                    span: common_sql::ast::Span::new(0, 0),
                    items: sel
                        .group_by
                        .into_iter()
                        .map(SqlExpression::from)
                        .map(GroupByItem::Expression)
                        .collect(),
                })
            },
            having: sel.having.map(SqlExpression::from),
            order_by: if sel.order_by.is_empty() {
                None
            } else {
                Some(OrderByClause {
                    span: common_sql::ast::Span::new(0, 0),
                    items: sel.order_by.into_iter().map(SqlOrderByItem::from).collect(),
                })
            },
            limit: sel.limit.map(SqlLimitClause::from),
        }
    }
}

// ---------------------------------------------------------------------------
// Statement bridge (T6): CommonStatement -> Option<common_sql::ast::Statement>.
// ---------------------------------------------------------------------------

/// Convert a legacy [`CommonStatement`] into the standalone
/// `common_sql::ast::Statement`.
///
/// This is the **entry point** of the `tsql_parser -> common_sql` statement
/// bridge. It is a free function returning `Option<Statement>` — **not** a
/// plain `From<CommonStatement>` — because
/// [`CommonStatement::DialectSpecific`] has no equivalent variant on the
/// destination [`Statement`] enum, and the bridge therefore drops it
/// (returning `None`). This mirrors the source-side
/// [`ToCommonAst::to_common_ast`](crate::common::ToCommonAst::to_common_ast)
/// -> `Option<CommonStatement>`, which itself returns `None` for constructs it
/// cannot represent.
///
/// `Select` / `Insert` / `Update` / `Delete` each map to the corresponding
/// boxed destination variant (`Statement::Select(Box<_>)`, etc.).
///
/// # Lossy mapping (recorded)
///
/// * **`DialectSpecific`** -> `None` (dropped; no destination variant).
/// * **Span** is dropped for every statement (destination spans are set to a
///   zero placeholder; the legacy span does not round-trip).
///
/// # Destination variants never produced
///
/// The bridge is one-directional (legacy -> common_sql). The destination
/// `Statement` has variants with no legacy source: `CreateTable`, `AlterTable`,
/// `DropTable`, `CreateIndex`, `DropIndex`. These are never emitted by this
/// function. The exhaustiveness guard relies on `match` compile-fail, which
/// catches *new* legacy variants only — not unmapped destination variants.
#[must_use]
pub fn convert(stmt: CommonStatement) -> Option<common_sql::ast::Statement> {
    use common_sql::ast::Statement as SqlStatement;
    match stmt {
        CommonStatement::Select(sel) => Some(SqlStatement::Select(Box::new(
            SqlSelectStatement::from(sel),
        ))),
        CommonStatement::Insert(ins) => Some(SqlStatement::Insert(Box::new(
            SqlInsertStatement::from(ins),
        ))),
        CommonStatement::Update(upd) => Some(SqlStatement::Update(Box::new(
            SqlUpdateStatement::from(upd),
        ))),
        CommonStatement::Delete(del) => Some(SqlStatement::Delete(Box::new(
            SqlDeleteStatement::from(del),
        ))),
        // No destination variant for dialect-specific statements -> drop.
        CommonStatement::DialectSpecific { .. } => None,
    }
}

// ---------------------------------------------------------------------------
// DML bridge (T5): INSERT / UPDATE / DELETE / Assignment.
// ---------------------------------------------------------------------------

/// Convert a legacy `String` table name into a destination [`SqlQualifiedName`]
/// with no schema qualifier.
fn qualified_name_from_string(name: String) -> SqlQualifiedName {
    SqlQualifiedName::new(None, name)
}

/// Convert a legacy `String` table name into a destination bare-table
/// [`SqlTableFactor`] (no alias).
fn table_factor_from_string(name: String) -> SqlTableFactor {
    SqlTableFactor::Table {
        name: qualified_name_from_string(name),
        alias: None,
    }
}

/// Convert a legacy [`CommonAssignment`] into a destination
/// [`SqlAssignment`].
///
/// The legacy `column: String` becomes a `common_sql` `Identifier`; the
/// `value` recurses through [`From<CommonExpression>`].
impl From<CommonAssignment> for SqlAssignment {
    fn from(a: CommonAssignment) -> Self {
        SqlAssignment {
            column: SqlIdentifier::new(a.column),
            value: SqlExpression::from(a.value),
        }
    }
}

/// Convert a legacy [`CommonInsertSource`] into a destination
/// [`SqlInsertSource`].
///
/// # Lossy mapping (recorded)
///
/// `CommonInsertSource::DefaultValues` has no counterpart on the destination
/// `InsertSource` (there is no `DefaultValues` variant). It is mapped to
/// `InsertSource::Values(vec![])` — an empty `VALUES` list. This is lossy: the
/// destination can no longer distinguish `INSERT ... DEFAULT VALUES` from
/// `INSERT ... VALUES ()`. `Values` and `Select` recurse normally.
impl From<CommonInsertSource> for SqlInsertSource {
    fn from(src: CommonInsertSource) -> Self {
        match src {
            CommonInsertSource::Values(rows) => SqlInsertSource::Values(
                rows.into_iter()
                    .map(|row| row.into_iter().map(SqlExpression::from).collect())
                    .collect(),
            ),
            CommonInsertSource::Select(query) => {
                SqlInsertSource::Select(Box::new(SqlSelectStatement::from(*query)))
            }
            // Lossy: destination has no DefaultValues variant -> empty Values.
            CommonInsertSource::DefaultValues => SqlInsertSource::Values(vec![]),
        }
    }
}

/// Convert a legacy [`CommonInsertStatement`] into a destination
/// [`SqlInsertStatement`].
///
/// Impedance resolved:
/// * `table: String` -> `QualifiedName::new(None, name)` (no schema).
/// * `columns: Vec<String>` -> `Vec<Identifier>`.
/// * `source` recurses through [`From<CommonInsertSource>`].
/// * `on_conflict` is always `None` (the legacy node carries none).
/// * Span is dropped.
impl From<CommonInsertStatement> for SqlInsertStatement {
    fn from(ins: CommonInsertStatement) -> Self {
        SqlInsertStatement {
            // Span dropped (destination field required but unused here).
            span: common_sql::ast::Span::new(0, 0),
            table: qualified_name_from_string(ins.table),
            columns: ins.columns.into_iter().map(SqlIdentifier::new).collect(),
            source: SqlInsertSource::from(ins.source),
            on_conflict: None,
        }
    }
}

/// Convert a legacy [`CommonUpdateStatement`] into a destination
/// [`SqlUpdateStatement`].
///
/// Impedance resolved:
/// * `table: String` -> `TableFactor::Table { name, alias: None }` (the
///   destination `table` field expects a `TableFactor`).
/// * `assignments: Vec<CommonAssignment>` -> `Vec<Assignment>`.
/// * `from` -> `None` (the legacy node has no FROM clause).
/// * `where_clause` recurses through [`From<CommonExpression>`].
/// * Span is dropped.
impl From<CommonUpdateStatement> for SqlUpdateStatement {
    fn from(upd: CommonUpdateStatement) -> Self {
        SqlUpdateStatement {
            span: common_sql::ast::Span::new(0, 0),
            table: table_factor_from_string(upd.table),
            assignments: upd
                .assignments
                .into_iter()
                .map(SqlAssignment::from)
                .collect(),
            from: None,
            where_clause: upd.where_clause.map(SqlExpression::from),
        }
    }
}

/// Convert a legacy [`CommonDeleteStatement`] into a destination
/// [`SqlDeleteStatement`].
///
/// Impedance resolved:
/// * `table: String` -> `TableFactor::Table { name, alias: None }`.
/// * `using` -> `None` (the legacy node has no USING clause).
/// * `where_clause` recurses through [`From<CommonExpression>`].
/// * Span is dropped.
impl From<CommonDeleteStatement> for SqlDeleteStatement {
    fn from(del: CommonDeleteStatement) -> Self {
        SqlDeleteStatement {
            span: common_sql::ast::Span::new(0, 0),
            table: table_factor_from_string(del.table),
            using: None,
            where_clause: del.where_clause.map(SqlExpression::from),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // -- leaf conversions: Literal / Identifier / UnaryOperator ------------

    #[test]
    fn literal_maps_identity_except_float() {
        assert_eq!(
            SqlLiteral::from(CommonLiteral::String("hi".to_string())),
            SqlLiteral::String("hi".to_string())
        );
        assert_eq!(
            SqlLiteral::from(CommonLiteral::Integer(7)),
            SqlLiteral::Integer(7)
        );
        assert_eq!(SqlLiteral::from(CommonLiteral::Null), SqlLiteral::Null);
        assert_eq!(
            SqlLiteral::from(CommonLiteral::Boolean(true)),
            SqlLiteral::Boolean(true)
        );
    }

    #[test]
    fn literal_float_renders_to_string() {
        let got: SqlLiteral = CommonLiteral::Float(1.5_f64).into();
        assert_eq!(got, SqlLiteral::Float("1.5".to_string()));
    }

    #[test]
    fn identifier_preserves_name() {
        let id = SqlIdentifier::from(CommonIdentifier {
            name: "count".to_string(),
        });
        assert_eq!(id.value(), "count");
        assert!(!id.quoted());
    }

    #[test]
    fn unary_operator_maps_identity() {
        assert_eq!(
            SqlUnaryOperator::from(CommonUnaryOperator::Plus),
            SqlUnaryOperator::Plus
        );
        assert_eq!(
            SqlUnaryOperator::from(CommonUnaryOperator::Minus),
            SqlUnaryOperator::Minus
        );
        assert_eq!(
            SqlUnaryOperator::from(CommonUnaryOperator::Not),
            SqlUnaryOperator::Not
        );
    }

    // -- operator-split dispatch (Task #36 / T2) ----------------------------
    //
    // The legacy `CommonBinaryOperator` is a single 14-variant enum that must
    // dispatch into THREE destination Expression variants depending on the
    // operator class:
    //   * arithmetic + concat  -> Expression::BinaryOp  (BinaryOperator)
    //   * comparison           -> Expression::Comparison (ComparisonOperator)
    //   * logical              -> Expression::LogicalOp  (LogicalOperator)
    //
    // A plain `From<CommonBinaryOperator>` cannot choose the wrapping variant,
    // so the bridge exposes `convert_binary_op(left, op, right) -> Expression`.

    /// Build a trivial leaf expression for test inputs.
    fn leaf(s: &str) -> CommonExpression {
        CommonExpression::Identifier(CommonIdentifier {
            name: s.to_string(),
        })
    }

    #[test]
    fn arithmetic_operators_dispatch_to_binary_op() {
        for (legacy_op, sql_op) in [
            (CommonBinaryOperator::Plus, SqlBinaryOperator::Add),
            (CommonBinaryOperator::Minus, SqlBinaryOperator::Sub),
            (CommonBinaryOperator::Multiply, SqlBinaryOperator::Mul),
            (CommonBinaryOperator::Divide, SqlBinaryOperator::Div),
            (CommonBinaryOperator::Modulo, SqlBinaryOperator::Mod),
            (CommonBinaryOperator::Concat, SqlBinaryOperator::Concat),
        ] {
            let got = convert_binary_op(leaf("a"), legacy_op, leaf("b"));
            match got {
                SqlExpression::BinaryOp { op, .. } => assert_eq!(op, sql_op),
                other => panic!("expected BinaryOp for {legacy_op:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn comparison_operators_dispatch_to_comparison() {
        for (legacy_op, sql_op) in [
            (CommonBinaryOperator::Eq, SqlComparisonOperator::Eq),
            (CommonBinaryOperator::Ne, SqlComparisonOperator::Ne),
            (CommonBinaryOperator::Lt, SqlComparisonOperator::Lt),
            (CommonBinaryOperator::Le, SqlComparisonOperator::Le),
            (CommonBinaryOperator::Gt, SqlComparisonOperator::Gt),
            (CommonBinaryOperator::Ge, SqlComparisonOperator::Ge),
        ] {
            let got = convert_binary_op(leaf("a"), legacy_op, leaf("b"));
            match got {
                SqlExpression::Comparison { op, .. } => assert_eq!(op, sql_op),
                other => panic!("expected Comparison for {legacy_op:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn logical_operators_dispatch_to_logical_op() {
        for (legacy_op, sql_op) in [
            (CommonBinaryOperator::And, SqlLogicalOperator::And),
            (CommonBinaryOperator::Or, SqlLogicalOperator::Or),
        ] {
            let got = convert_binary_op(leaf("a"), legacy_op, leaf("b"));
            match got {
                SqlExpression::LogicalOp { op, .. } => assert_eq!(op, sql_op),
                other => panic!("expected LogicalOp for {legacy_op:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn binary_dispatch_preserves_recursed_operands() {
        // (a + b) > c — the helper converts each operand via From, so a nested
        // legacy BinaryOp operand recurses into the correct destination shape.
        let inner = CommonExpression::BinaryOp {
            left: Box::new(leaf("a")),
            op: CommonBinaryOperator::Plus,
            right: Box::new(leaf("b")),
            span: tsql_token::Span::new(0, 0),
        };
        let outer_sql = convert_binary_op(inner, CommonBinaryOperator::Gt, leaf("c"));

        match outer_sql {
            SqlExpression::Comparison { op, left, right } => {
                assert_eq!(op, SqlComparisonOperator::Gt);
                // The left operand should have recursed into a BinaryOp.
                assert!(matches!(*left, SqlExpression::BinaryOp { .. }));
                assert!(matches!(*right, SqlExpression::Identifier(_)));
            }
            other => panic!("expected Comparison, got {other:?}"),
        }
    }

    #[test]
    fn all_fourteen_legacy_operators_are_covered_by_dispatch() {
        // Exhaustiveness guard: if a new variant is added to CommonBinaryOperator,
        // the dispatch `match` stops compiling. This test asserts the 14 known
        // variants each land in the right destination class.
        let all = [
            CommonBinaryOperator::Plus,
            CommonBinaryOperator::Minus,
            CommonBinaryOperator::Multiply,
            CommonBinaryOperator::Divide,
            CommonBinaryOperator::Modulo,
            CommonBinaryOperator::Concat,
            CommonBinaryOperator::Eq,
            CommonBinaryOperator::Ne,
            CommonBinaryOperator::Lt,
            CommonBinaryOperator::Le,
            CommonBinaryOperator::Gt,
            CommonBinaryOperator::Ge,
            CommonBinaryOperator::And,
            CommonBinaryOperator::Or,
        ];
        for op in all {
            let got = convert_binary_op(leaf("x"), op, leaf("y"));
            let class = match &got {
                SqlExpression::BinaryOp { .. } => "binary",
                SqlExpression::Comparison { .. } => "comparison",
                SqlExpression::LogicalOp { .. } => "logical",
                other => panic!("unexpected dispatch target {other:?} for {op:?}"),
            };
            assert!(!class.is_empty());
        }
    }

    // -- identity mappings --------------------------------------------------

    #[test]
    fn integer_types_map_identity() {
        assert_eq!(
            SqlDataType::from(CommonDataType::TinyInt),
            SqlDataType::TinyInt
        );
        assert_eq!(
            SqlDataType::from(CommonDataType::SmallInt),
            SqlDataType::SmallInt
        );
        assert_eq!(SqlDataType::from(CommonDataType::Int), SqlDataType::Int);
        assert_eq!(
            SqlDataType::from(CommonDataType::BigInt),
            SqlDataType::BigInt
        );
    }

    #[test]
    fn decimal_and_numeric_preserve_precision_and_scale() {
        assert_eq!(
            SqlDataType::from(CommonDataType::Decimal {
                precision: Some(10),
                scale: Some(2),
            }),
            SqlDataType::Decimal {
                precision: Some(10),
                scale: Some(2),
            }
        );
        assert_eq!(
            SqlDataType::from(CommonDataType::Numeric {
                precision: None,
                scale: None,
            }),
            SqlDataType::Numeric {
                precision: None,
                scale: None,
            }
        );
    }

    #[test]
    fn floating_types_preserve_real_and_double() {
        assert_eq!(SqlDataType::from(CommonDataType::Real), SqlDataType::Real);
        assert_eq!(
            SqlDataType::from(CommonDataType::DoublePrecision),
            SqlDataType::DoublePrecision
        );
    }

    #[test]
    fn float_collapses_to_double_precision_discarding_precision() {
        // common-sql has no FLOAT; the precision must be dropped.
        assert_eq!(
            SqlDataType::from(CommonDataType::Float {
                precision: Some(24)
            }),
            SqlDataType::DoublePrecision
        );
    }

    #[test]
    fn character_types_preserve_length() {
        assert_eq!(
            SqlDataType::from(CommonDataType::Char { length: Some(10) }),
            SqlDataType::Char { length: Some(10) }
        );
        assert_eq!(
            SqlDataType::from(CommonDataType::VarChar { length: None }),
            SqlDataType::VarChar { length: None }
        );
        assert_eq!(SqlDataType::from(CommonDataType::Text), SqlDataType::Text);
    }

    #[test]
    fn national_character_types_preserve_length() {
        assert_eq!(
            SqlDataType::from(CommonDataType::NChar { length: Some(5) }),
            SqlDataType::NChar { length: Some(5) }
        );
        assert_eq!(
            SqlDataType::from(CommonDataType::NVarChar { length: Some(50) }),
            SqlDataType::NVarChar { length: Some(50) }
        );
    }

    #[test]
    fn temporal_types_preserve_precision() {
        assert_eq!(SqlDataType::from(CommonDataType::Date), SqlDataType::Date);
        assert_eq!(
            SqlDataType::from(CommonDataType::Time { precision: Some(3) }),
            SqlDataType::Time { precision: Some(3) }
        );
        assert_eq!(
            SqlDataType::from(CommonDataType::DateTime { precision: None }),
            SqlDataType::DateTime { precision: None }
        );
        assert_eq!(
            SqlDataType::from(CommonDataType::Timestamp { precision: Some(6) }),
            SqlDataType::Timestamp { precision: Some(6) }
        );
    }

    #[test]
    fn binary_types_preserve_length() {
        assert_eq!(
            SqlDataType::from(CommonDataType::Binary { length: Some(16) }),
            SqlDataType::Binary { length: Some(16) }
        );
        assert_eq!(
            SqlDataType::from(CommonDataType::VarBinary { length: Some(255) }),
            SqlDataType::VarBinary { length: Some(255) }
        );
        assert_eq!(SqlDataType::from(CommonDataType::Blob), SqlDataType::Blob);
    }

    #[test]
    fn misc_types_map_identity() {
        assert_eq!(
            SqlDataType::from(CommonDataType::Boolean),
            SqlDataType::Boolean
        );
        assert_eq!(SqlDataType::from(CommonDataType::Uuid), SqlDataType::Uuid);
        assert_eq!(SqlDataType::from(CommonDataType::Json), SqlDataType::Json);
    }

    // -- exhaustiveness guard ----------------------------------------------
    // Every CommonDataType variant is exercised above; if a variant is added
    // to CommonDataType, the `match` in `From` stops compiling, forcing this
    // bridge to be updated in lockstep.

    #[test]
    fn into_works_via_from_impl() {
        let sql: SqlDataType = CommonDataType::Int.into();
        assert_eq!(sql, SqlDataType::Int);
    }
}
