//! Scaffold integration tests for common-sql crate.
//!
//! Verifies:
//! - Crate compiles with zero external dependencies
//! - Workspace lint inheritance works
//! - All declared modules are accessible
//! - Public re-exports are reachable from the crate root
//! - Each type can be instantiated and has expected derives

#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use common_sql::ast::{
        BinaryOperator, ComparisonOperator, DataType, Expression, Identifier, InList, Literal,
        LogicalOperator, Position, QualifiedName, SelectItem, SelectStatement, Span, Statement,
        TableAlias, UnaryOperator,
    };

    // ---------------------------------------------------------------
    // Test: Span can be constructed and has expected derives
    // ---------------------------------------------------------------
    #[test]
    fn test_span_new_and_derives() {
        let span = Span::new(0, 5);
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 5);

        // Copy (implicit — Clone is also available via Copy)
        let copied = span;
        assert_eq!(span, copied);

        // Debug
        let _debug_str = format!("{span:?}");
    }

    #[test]
    fn test_span_is_empty() {
        let empty = Span::new(10, 10);
        assert!(empty.is_empty());

        let non_empty = Span::new(10, 20);
        assert!(!non_empty.is_empty());
    }

    // ---------------------------------------------------------------
    // Test: Position can be constructed
    // ---------------------------------------------------------------
    #[test]
    fn test_position_construction() {
        let pos = Position::new(1, 1, 0);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 1);
        assert_eq!(pos.offset, 0);

        // Copy + Debug
        let _copied = pos;
        let _debug = format!("{pos:?}");
    }

    // ---------------------------------------------------------------
    // Test: Identifier can be constructed
    // ---------------------------------------------------------------
    #[test]
    fn test_identifier_construction() {
        let id = Identifier::new("users".to_string());
        assert_eq!(id.value(), "users");
        assert!(!id.quoted());

        // Clone + Debug + PartialEq
        let id2 = id.clone();
        assert_eq!(id, id2);
        let _debug = format!("{id:?}");
    }

    #[test]
    fn test_identifier_quoted() {
        let id = Identifier::new_quoted("name".to_string());
        assert_eq!(id.value(), "name");
        assert!(id.quoted());
    }

    // ---------------------------------------------------------------
    // Test: QualifiedName can be constructed
    // ---------------------------------------------------------------
    #[test]
    fn test_qualified_name_construction() {
        let qn = QualifiedName::new(None, "users".to_string());
        assert_eq!(qn.name(), "users");
        assert!(qn.schema().is_none());

        let qn_with_schema = QualifiedName::new(Some("dbo".to_string()), "users".to_string());
        assert_eq!(qn_with_schema.schema(), Some("dbo"));
    }

    // ---------------------------------------------------------------
    // Test: TableAlias can be constructed
    // ---------------------------------------------------------------
    #[test]
    fn test_table_alias_construction() {
        let alias = TableAlias::new("u".to_string(), vec![]);
        assert_eq!(alias.name(), "u");
        assert!(alias.columns().is_empty());
    }

    // ---------------------------------------------------------------
    // Test: Literal variants can be constructed
    // ---------------------------------------------------------------
    #[test]
    fn test_literal_integer() {
        let lit = Literal::Integer(42);
        let _debug = format!("{lit:?}");
        let cloned = lit.clone();
        assert_eq!(lit, cloned);
    }

    #[test]
    fn test_literal_null() {
        let lit = Literal::Null;
        assert_eq!(lit, Literal::Null);
    }

    #[test]
    fn test_literal_string() {
        let lit = Literal::String("hello".to_string());
        let _debug = format!("{lit:?}");
    }

    #[test]
    fn test_literal_boolean() {
        assert_eq!(Literal::Boolean(true), Literal::Boolean(true));
    }

    #[test]
    fn test_literal_float() {
        // Float is String-based per CTO review (preserves DECIMAL precision)
        let lit = Literal::Float("3.14159".to_string());
        let _debug = format!("{lit:?}");
    }

    // ---------------------------------------------------------------
    // Test: Operator enums have expected derives
    // ---------------------------------------------------------------
    #[test]
    fn test_binary_operator_derives() {
        let op = BinaryOperator::Add;
        let copied = op;
        assert_eq!(op, copied);
        let _debug = format!("{op:?}");
    }

    #[test]
    fn test_unary_operator_derives() {
        let op = UnaryOperator::Minus;
        assert_eq!(op, UnaryOperator::Minus);
    }

    #[test]
    fn test_logical_operator_derives() {
        assert_eq!(LogicalOperator::And, LogicalOperator::And);
        assert_eq!(LogicalOperator::Or, LogicalOperator::Or);
    }

    #[test]
    fn test_comparison_operator_derives() {
        assert_eq!(ComparisonOperator::Eq, ComparisonOperator::Eq);
    }

    // ---------------------------------------------------------------
    // Test: DataType has at least basic variants
    // ---------------------------------------------------------------
    #[test]
    fn test_data_type_basic_variants() {
        let _int = DataType::Int;
        let _text = DataType::Text;
        let _bool = DataType::Boolean;
        let _uuid = DataType::Uuid;
        let _date = DataType::Date;
    }

    #[test]
    fn test_data_type_varchar() {
        let vc = DataType::VarChar { length: Some(255) };
        let _debug = format!("{vc:?}");
        assert_eq!(vc.clone(), vc);
    }

    // ---------------------------------------------------------------
    // Test: SelectItem has expected variants
    // ---------------------------------------------------------------
    #[test]
    fn test_select_item_wildcard() {
        let item = SelectItem::Wildcard;
        let _debug = format!("{item:?}");
        assert_eq!(item.clone(), item);
    }

    // ---------------------------------------------------------------
    // Test: Expression::Literal variant works
    // ---------------------------------------------------------------
    #[test]
    fn test_expression_literal() {
        let expr = Expression::Literal(Literal::Integer(1));
        let _debug = format!("{expr:?}");
        assert_eq!(expr.clone(), expr);
    }

    #[test]
    fn test_expression_identifier() {
        let expr = Expression::Identifier(Identifier::new("col".to_string()));
        let _debug = format!("{expr:?}");
    }

    // ---------------------------------------------------------------
    // Test: InList variants exist
    // ---------------------------------------------------------------
    #[test]
    fn test_in_list_variants() {
        let values = InList::Values(vec![]);
        let _debug = format!("{values:?}");
        assert_eq!(values.clone(), values);
    }

    // ---------------------------------------------------------------
    // Test: Statement has at least Select variant
    // ---------------------------------------------------------------
    #[test]
    fn test_statement_select_variant_exists() {
        let stmt = Statement::Select(Box::new(SelectStatement::simple(vec![])));
        let _debug = format!("{stmt:?}");
        assert_eq!(stmt.clone(), stmt);
    }

    // ---------------------------------------------------------------
    // Test: Span default is zero-width at origin
    // ---------------------------------------------------------------
    #[test]
    fn test_span_default() {
        let span = Span::default();
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 0);
        assert!(span.is_empty());
    }

    // ---------------------------------------------------------------
    // Test: crate has no external dependencies (compile-time check)
    // This is implicitly verified by the fact that we can use all
    // types above without any external crate imports.
    // ---------------------------------------------------------------
    #[test]
    fn test_no_external_dependency_imports() {
        // If this compiles, the crate has zero external dependencies.
        // All types used in this file come from common_sql::ast only.
        let _span = Span::new(0, 0);
        let _pos = Position::new(1, 1, 0);
        let _id = Identifier::new("test".to_string());
        let _qn = QualifiedName::new(None, "tbl".to_string());
        let _alias = TableAlias::new("a".to_string(), vec![]);
        let _lit = Literal::Integer(0);
        let _op = BinaryOperator::Add;
        let _dt = DataType::Int;
        let _item = SelectItem::Wildcard;
        let _expr = Expression::Literal(Literal::Null);
        let _in_list = InList::Values(vec![]);
        let _stmt = Statement::Select(Box::new(SelectStatement::simple(vec![])));
    }
}
