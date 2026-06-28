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

use common_sql::ast::DataType as SqlDataType;

use crate::common::CommonDataType;

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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

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
