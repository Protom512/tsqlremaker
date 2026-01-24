//! ASTテスト

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::module_inception)]
mod tests {
    use tsql_token::Span;

    use crate::ast::base::AstNode;
    use crate::ast::{Identifier, Literal, SelectStatement};

    #[test]
    fn test_identifier_ast_node() {
        let ident = Identifier {
            name: "test".to_string(),
            span: Span { start: 0, end: 4 },
        };
        assert_eq!(ident.span(), Span { start: 0, end: 4 });
    }

    #[test]
    fn test_select_statement_span() {
        let select = SelectStatement {
            span: Span { start: 0, end: 100 },
            distinct: false,
            top: None,
            columns: vec![],
            from: None,
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
        };
        assert_eq!(select.span(), Span { start: 0, end: 100 });
    }

    #[test]
    fn test_literal_span() {
        let string_lit = Literal::String("test".to_string(), Span { start: 0, end: 6 });
        assert_eq!(string_lit.span().start, 0);
        assert_eq!(string_lit.span().end, 6); // including quotes

        let number_lit = Literal::Number("123".to_string(), Span { start: 0, end: 3 });
        assert_eq!(number_lit.span().start, 0);
        assert_eq!(number_lit.span().end, 3);

        let null_lit = Literal::Null(Span { start: 0, end: 4 });
        assert_eq!(null_lit.span().start, 0);
        assert_eq!(null_lit.span().end, 4);

        let bool_lit = Literal::Boolean(true, Span { start: 0, end: 4 });
        assert_eq!(bool_lit.span().start, 0);
        assert_eq!(bool_lit.span().end, 4); // TRUE
    }
}
