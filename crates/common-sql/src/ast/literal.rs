//! SQL literal values.
//!
//! Represents literal values that appear in SQL expressions:
//! integers, floats, strings, booleans, and NULL.

// TDD: Tests written FIRST, implementation follows.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // ---- Construction tests ----

    #[test]
    fn test_integer_literal() {
        let lit = Literal::Integer(42_i64);
        assert!(matches!(lit, Literal::Integer(n) if n == 42));
    }

    #[test]
    fn test_integer_literal_negative() {
        let lit = Literal::Integer(-1_i64);
        assert!(matches!(lit, Literal::Integer(n) if n == -1));
    }

    #[test]
    fn test_integer_literal_max() {
        let lit = Literal::Integer(i64::MAX);
        assert!(matches!(lit, Literal::Integer(n) if n == i64::MAX));
    }

    #[test]
    fn test_integer_literal_min() {
        let lit = Literal::Integer(i64::MIN);
        assert!(matches!(lit, Literal::Integer(n) if n == i64::MIN));
    }

    #[test]
    fn test_float_literal() {
        let lit = Literal::Float("3.14".to_string());
        assert!(matches!(lit, Literal::Float(s) if s == "3.14"));
    }

    #[test]
    fn test_float_literal_preserves_precision() {
        // DECIMAL(18,4) value must not lose precision
        let lit = Literal::Float("123456789012.3456".to_string());
        assert!(matches!(lit, Literal::Float(s) if s == "123456789012.3456"));
    }

    #[test]
    fn test_float_literal_scientific_notation() {
        let lit = Literal::Float("1.5e-10".to_string());
        assert!(matches!(lit, Literal::Float(s) if s == "1.5e-10"));
    }

    #[test]
    fn test_float_literal_zero() {
        let lit = Literal::Float("0.0".to_string());
        assert!(matches!(lit, Literal::Float(s) if s == "0.0"));
    }

    #[test]
    fn test_string_literal() {
        let lit = Literal::String("hello world".to_string());
        assert!(matches!(lit, Literal::String(s) if s == "hello world"));
    }

    #[test]
    fn test_string_literal_empty() {
        let lit = Literal::String(String::new());
        assert!(matches!(lit, Literal::String(s) if s.is_empty()));
    }

    #[test]
    fn test_string_literal_with_special_chars() {
        let lit = Literal::String("O'Brien".to_string());
        assert!(matches!(lit, Literal::String(s) if s == "O'Brien"));
    }

    #[test]
    fn test_string_literal_unicode() {
        let lit = Literal::String("日本語テスト".to_string());
        assert!(matches!(lit, Literal::String(s) if s == "日本語テスト"));
    }

    #[test]
    fn test_boolean_literal_true() {
        let lit = Literal::Boolean(true);
        assert!(matches!(lit, Literal::Boolean(b) if b));
    }

    #[test]
    fn test_boolean_literal_false() {
        let lit = Literal::Boolean(false);
        assert!(matches!(lit, Literal::Boolean(b) if !b));
    }

    #[test]
    fn test_null_literal() {
        let lit = Literal::Null;
        assert!(matches!(lit, Literal::Null));
    }

    // ---- Debug derive tests ----

    #[test]
    fn test_debug_integer() {
        let lit = Literal::Integer(42);
        let debug_str = format!("{lit:?}");
        assert!(debug_str.contains("42"));
    }

    #[test]
    fn test_debug_float() {
        let lit = Literal::Float("3.14".to_string());
        let debug_str = format!("{lit:?}");
        assert!(debug_str.contains("3.14"));
    }

    #[test]
    fn test_debug_string() {
        let lit = Literal::String("test".to_string());
        let debug_str = format!("{lit:?}");
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_debug_boolean() {
        let lit = Literal::Boolean(true);
        let debug_str = format!("{lit:?}");
        assert!(debug_str.contains("true"));
    }

    #[test]
    fn test_debug_null() {
        let lit = Literal::Null;
        let debug_str = format!("{lit:?}");
        assert!(debug_str.contains("Null"));
    }

    // ---- Clone derive tests ----

    #[test]
    fn test_clone_integer() {
        let lit = Literal::Integer(42);
        let cloned = lit.clone();
        assert_eq!(lit, cloned);
    }

    #[test]
    fn test_clone_float() {
        let lit = Literal::Float("3.14".to_string());
        let cloned = lit.clone();
        assert_eq!(lit, cloned);
    }

    #[test]
    fn test_clone_string() {
        let lit = Literal::String("hello".to_string());
        let cloned = lit.clone();
        assert_eq!(lit, cloned);
    }

    #[test]
    fn test_clone_boolean() {
        let lit = Literal::Boolean(true);
        let cloned = lit.clone();
        assert_eq!(lit, cloned);
    }

    #[test]
    fn test_clone_null() {
        let lit = Literal::Null;
        let cloned = lit.clone();
        assert_eq!(lit, cloned);
    }

    // ---- PartialEq derive tests ----

    #[test]
    fn test_eq_same_integer() {
        assert_eq!(Literal::Integer(42), Literal::Integer(42));
    }

    #[test]
    fn test_eq_different_integer() {
        assert_ne!(Literal::Integer(42), Literal::Integer(43));
    }

    #[test]
    fn test_eq_same_float() {
        assert_eq!(
            Literal::Float("3.14".to_string()),
            Literal::Float("3.14".to_string())
        );
    }

    #[test]
    fn test_eq_different_float() {
        assert_ne!(
            Literal::Float("3.14".to_string()),
            Literal::Float("3.14159".to_string())
        );
    }

    #[test]
    fn test_eq_same_string() {
        assert_eq!(
            Literal::String("hello".to_string()),
            Literal::String("hello".to_string())
        );
    }

    #[test]
    fn test_eq_different_string() {
        assert_ne!(
            Literal::String("hello".to_string()),
            Literal::String("world".to_string())
        );
    }

    #[test]
    fn test_eq_same_boolean() {
        assert_eq!(Literal::Boolean(true), Literal::Boolean(true));
        assert_eq!(Literal::Boolean(false), Literal::Boolean(false));
    }

    #[test]
    fn test_eq_different_boolean() {
        assert_ne!(Literal::Boolean(true), Literal::Boolean(false));
    }

    #[test]
    fn test_eq_null() {
        assert_eq!(Literal::Null, Literal::Null);
    }

    #[test]
    fn test_ne_cross_types() {
        // Different variants are never equal
        assert_ne!(Literal::Integer(1), Literal::Float("1".to_string()));
        assert_ne!(Literal::Integer(0), Literal::Boolean(false));
        assert_ne!(Literal::Integer(0), Literal::Null);
        assert_ne!(Literal::String("true".to_string()), Literal::Boolean(true));
        assert_ne!(Literal::Float("0".to_string()), Literal::Null);
    }

    // ---- Eq derive tests (Hash / HashMap usage) ----

    #[test]
    fn test_eq_hash_consistency() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Literal::Integer(42));
        set.insert(Literal::Integer(42)); // duplicate
        set.insert(Literal::Integer(43));
        set.insert(Literal::Null);
        assert_eq!(set.len(), 3);
    }

    // ---- Edge case tests ----

    #[test]
    fn test_integer_zero() {
        let lit = Literal::Integer(0);
        assert!(matches!(lit, Literal::Integer(0)));
    }

    #[test]
    fn test_float_empty_string_is_valid() {
        // Edge: empty string for float — semantically unusual but type-valid
        let lit = Literal::Float(String::new());
        assert!(matches!(lit, Literal::Float(s) if s.is_empty()));
    }

    #[test]
    fn test_string_literal_with_newlines() {
        let lit = Literal::String("line1\nline2".to_string());
        assert!(matches!(lit, Literal::String(s) if s.contains('\n')));
    }
}

/// SQL literal value.
///
/// Uses `Float(String)` instead of `Float(f64)` to preserve decimal precision
/// across dialect transpilation (e.g., `DECIMAL(18,4)` values must not lose
/// precision when converting from ASE T-SQL to MySQL).
///
/// # Examples
///
/// ```
/// use common_sql::ast::Literal;
///
/// let int_lit = Literal::Integer(42);
/// let float_lit = Literal::Float("3.14159".to_string());
/// let str_lit = Literal::String("hello".to_string());
/// let bool_lit = Literal::Boolean(true);
/// let null_lit = Literal::Null;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Literal {
    /// Integer literal (e.g., `42`, `-1`, `0`)
    Integer(i64),
    /// Float literal stored as string to preserve precision (e.g., `"3.14"`, `"1.5e-10"`)
    Float(String),
    /// String literal (e.g., `'hello'`, `'O''Brien'`)
    String(String),
    /// Boolean literal (`TRUE` / `FALSE`)
    Boolean(bool),
    /// NULL literal
    Null,
}
