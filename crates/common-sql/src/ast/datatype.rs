//! SQL data type definitions.

/// SQL data type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataType {
    // Integer types
    /// 1-byte integer (`TINYINT`).
    TinyInt,
    /// 2-byte integer (`SMALLINT`).
    SmallInt,
    /// 4-byte integer (`INT` / `INTEGER`).
    Int,
    /// 8-byte integer (`BIGINT`).
    BigInt,

    // Decimal types
    /// Fixed-point decimal (`DECIMAL(p, s)`).
    Decimal {
        /// Total digit count.
        precision: Option<u8>,
        /// Digits after the decimal point.
        scale: Option<u8>,
    },
    /// Fixed-point numeric (`NUMERIC(p, s)`).
    Numeric {
        /// Total digit count.
        precision: Option<u8>,
        /// Digits after the decimal point.
        scale: Option<u8>,
    },
    /// Single-precision floating point (`REAL`).
    Real,
    /// Double-precision floating point (`DOUBLE PRECISION`).
    DoublePrecision,

    // String types
    /// Fixed-length character string (`CHAR(n)`).
    Char {
        /// String length in characters.
        length: Option<u64>,
    },
    /// Variable-length character string (`VARCHAR(n)`).
    VarChar {
        /// Maximum string length in characters.
        length: Option<u64>,
    },
    /// Unlimited-length text (`TEXT`).
    Text,
    /// Fixed-length national character string (`NCHAR(n)`).
    NChar {
        /// String length in characters.
        length: Option<u64>,
    },
    /// Variable-length national character string (`NVARCHAR(n)`).
    NVarChar {
        /// Maximum string length in characters.
        length: Option<u64>,
    },
    /// Unlimited-length national text (`NTEXT`).
    NText,

    // Date/time types
    /// Calendar date (`DATE`).
    Date,
    /// Time of day (`TIME(p)`).
    Time {
        /// Fractional seconds precision.
        precision: Option<u8>,
    },
    /// Date and time (`DATETIME(p)`).
    DateTime {
        /// Fractional seconds precision.
        precision: Option<u8>,
    },
    /// Date and time stamp (`TIMESTAMP(p)`).
    Timestamp {
        /// Fractional seconds precision.
        precision: Option<u8>,
    },

    // Binary types
    /// Fixed-length binary (`BINARY(n)`).
    Binary {
        /// Byte length.
        length: Option<u64>,
    },
    /// Variable-length binary (`VARBINARY(n)`).
    VarBinary {
        /// Maximum byte length.
        length: Option<u64>,
    },
    /// Binary large object (`BLOB`).
    Blob,

    // Other
    /// Boolean (`BOOLEAN`).
    Boolean,
    /// Universally unique identifier (`UUID`).
    Uuid,
    /// JSON data (`JSON`).
    Json,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ── Integer types: construction & equality ───────────

    #[test]
    fn tinyint_constructs_and_equals() {
        let a = DataType::TinyInt;
        let b = DataType::TinyInt;
        assert_eq!(a, b);
    }

    #[test]
    fn smallint_constructs_and_equals() {
        assert_eq!(DataType::SmallInt, DataType::SmallInt);
    }

    #[test]
    fn int_constructs_and_equals() {
        assert_eq!(DataType::Int, DataType::Int);
    }

    #[test]
    fn bigint_constructs_and_equals() {
        assert_eq!(DataType::BigInt, DataType::BigInt);
    }

    #[test]
    fn integer_types_are_distinct() {
        let types = [
            DataType::TinyInt,
            DataType::SmallInt,
            DataType::Int,
            DataType::BigInt,
        ];
        for (i, a) in types.iter().enumerate() {
            for (j, b) in types.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "{:?} should differ from {:?}", a, b);
                }
            }
        }
    }

    // ── Decimal types: precision & scale ─────────────────

    #[test]
    fn decimal_with_both_precision_and_scale() {
        let dt = DataType::Decimal {
            precision: Some(18),
            scale: Some(4),
        };
        assert_eq!(
            dt,
            DataType::Decimal {
                precision: Some(18),
                scale: Some(4),
            }
        );
    }

    #[test]
    fn decimal_with_precision_only() {
        let dt = DataType::Decimal {
            precision: Some(10),
            scale: None,
        };
        assert_eq!(
            dt,
            DataType::Decimal {
                precision: Some(10),
                scale: None,
            }
        );
        assert_ne!(
            dt,
            DataType::Decimal {
                precision: Some(10),
                scale: Some(0),
            }
        );
    }

    #[test]
    fn decimal_with_no_params() {
        let dt = DataType::Decimal {
            precision: None,
            scale: None,
        };
        assert_eq!(
            dt,
            DataType::Decimal {
                precision: None,
                scale: None,
            }
        );
    }

    #[test]
    fn numeric_with_precision_and_scale() {
        let dt = DataType::Numeric {
            precision: Some(38),
            scale: Some(10),
        };
        assert_eq!(
            dt,
            DataType::Numeric {
                precision: Some(38),
                scale: Some(10),
            }
        );
    }

    #[test]
    fn decimal_and_numeric_are_distinct() {
        assert_ne!(
            DataType::Decimal {
                precision: Some(10),
                scale: Some(2),
            },
            DataType::Numeric {
                precision: Some(10),
                scale: Some(2),
            }
        );
    }

    #[test]
    fn real_constructs_and_equals() {
        assert_eq!(DataType::Real, DataType::Real);
    }

    #[test]
    fn double_precision_constructs_and_equals() {
        assert_eq!(DataType::DoublePrecision, DataType::DoublePrecision);
    }

    #[test]
    fn real_and_double_precision_are_distinct() {
        assert_ne!(DataType::Real, DataType::DoublePrecision);
    }

    // ── String types: length parameter ───────────────────

    #[test]
    fn char_with_length() {
        let dt = DataType::Char { length: Some(10) };
        assert_eq!(dt, DataType::Char { length: Some(10) });
    }

    #[test]
    fn char_without_length() {
        let dt = DataType::Char { length: None };
        assert_eq!(dt, DataType::Char { length: None });
    }

    #[test]
    fn varchar_with_length() {
        let dt = DataType::VarChar { length: Some(255) };
        assert_eq!(dt, DataType::VarChar { length: Some(255) });
    }

    #[test]
    fn varchar_without_length() {
        let dt = DataType::VarChar { length: None };
        assert_eq!(dt, DataType::VarChar { length: None });
    }

    #[test]
    fn text_constructs_and_equals() {
        assert_eq!(DataType::Text, DataType::Text);
    }

    #[test]
    fn nchar_with_length() {
        let dt = DataType::NChar { length: Some(50) };
        assert_eq!(dt, DataType::NChar { length: Some(50) });
    }

    #[test]
    fn nvarchar_with_length() {
        let dt = DataType::NVarChar { length: Some(100) };
        assert_eq!(dt, DataType::NVarChar { length: Some(100) });
    }

    #[test]
    fn ntext_constructs_and_equals() {
        assert_eq!(DataType::NText, DataType::NText);
    }

    #[test]
    fn char_and_varchar_are_distinct_even_with_same_length() {
        assert_ne!(
            DataType::Char { length: Some(10) },
            DataType::VarChar { length: Some(10) }
        );
    }

    #[test]
    fn char_and_nchar_are_distinct_even_with_same_length() {
        assert_ne!(
            DataType::Char { length: Some(10) },
            DataType::NChar { length: Some(10) }
        );
    }

    // ── Date/time types ──────────────────────────────────

    #[test]
    fn date_constructs_and_equals() {
        assert_eq!(DataType::Date, DataType::Date);
    }

    #[test]
    fn time_with_precision() {
        let dt = DataType::Time { precision: Some(6) };
        assert_eq!(dt, DataType::Time { precision: Some(6) });
    }

    #[test]
    fn time_without_precision() {
        let dt = DataType::Time { precision: None };
        assert_eq!(dt, DataType::Time { precision: None });
    }

    #[test]
    fn datetime_with_precision() {
        let dt = DataType::DateTime { precision: Some(3) };
        assert_eq!(dt, DataType::DateTime { precision: Some(3) });
    }

    #[test]
    fn timestamp_with_precision() {
        let dt = DataType::Timestamp { precision: Some(6) };
        assert_eq!(dt, DataType::Timestamp { precision: Some(6) });
    }

    #[test]
    fn date_and_datetime_are_distinct() {
        assert_ne!(DataType::Date, DataType::DateTime { precision: None });
    }

    // ── Binary types: length parameter ──────────────────

    #[test]
    fn binary_with_length() {
        let dt = DataType::Binary { length: Some(16) };
        assert_eq!(dt, DataType::Binary { length: Some(16) });
    }

    #[test]
    fn binary_without_length() {
        let dt = DataType::Binary { length: None };
        assert_eq!(dt, DataType::Binary { length: None });
    }

    #[test]
    fn varbinary_with_length() {
        let dt = DataType::VarBinary { length: Some(1024) };
        assert_eq!(dt, DataType::VarBinary { length: Some(1024) });
    }

    #[test]
    fn blob_constructs_and_equals() {
        assert_eq!(DataType::Blob, DataType::Blob);
    }

    #[test]
    fn binary_and_varbinary_are_distinct() {
        assert_ne!(
            DataType::Binary { length: Some(16) },
            DataType::VarBinary { length: Some(16) }
        );
    }

    // ── Other types ──────────────────────────────────────

    #[test]
    fn boolean_constructs_and_equals() {
        assert_eq!(DataType::Boolean, DataType::Boolean);
    }

    #[test]
    fn uuid_constructs_and_equals() {
        assert_eq!(DataType::Uuid, DataType::Uuid);
    }

    #[test]
    fn json_constructs_and_equals() {
        assert_eq!(DataType::Json, DataType::Json);
    }

    #[test]
    fn other_types_are_mutually_distinct() {
        let types = [DataType::Boolean, DataType::Uuid, DataType::Json];
        for (i, a) in types.iter().enumerate() {
            for (j, b) in types.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "{:?} should differ from {:?}", a, b);
                }
            }
        }
    }

    // ── Clone ────────────────────────────────────────────

    #[test]
    fn clone_simple_type() {
        let dt = DataType::Int;
        let cloned = dt.clone();
        assert_eq!(dt, cloned);
    }

    #[test]
    fn clone_parametrized_type() {
        let dt = DataType::Decimal {
            precision: Some(18),
            scale: Some(4),
        };
        let cloned = dt.clone();
        assert_eq!(dt, cloned);
    }

    #[test]
    fn clone_varchar_with_length() {
        let dt = DataType::VarChar { length: Some(100) };
        let cloned = dt.clone();
        assert_eq!(dt, cloned);
    }

    // ── Debug output ─────────────────────────────────────

    #[test]
    fn debug_output_simple_type() {
        let debug = format!("{:?}", DataType::Int);
        assert!(
            debug.contains("Int"),
            "Debug output should contain 'Int': {debug}"
        );
    }

    #[test]
    fn debug_output_decimal() {
        let debug = format!(
            "{:?}",
            DataType::Decimal {
                precision: Some(18),
                scale: Some(4),
            }
        );
        assert!(
            debug.contains("Decimal"),
            "Debug output should contain 'Decimal': {debug}"
        );
    }

    #[test]
    fn debug_output_varchar() {
        let debug = format!("{:?}", DataType::VarChar { length: Some(255) });
        assert!(
            debug.contains("VarChar"),
            "Debug output should contain 'VarChar': {debug}"
        );
    }

    // ── Hash / Eq (HashSet usage) ───────────────────────

    #[test]
    fn eq_allows_hashset_dedup() {
        let mut set = HashSet::new();
        set.insert(DataType::Int);
        set.insert(DataType::Int);
        set.insert(DataType::BigInt);
        assert_eq!(set.len(), 2, "HashSet should deduplicate equal types");
    }

    #[test]
    fn eq_parametrized_types_with_same_params_are_equal() {
        assert_eq!(
            DataType::Decimal {
                precision: Some(10),
                scale: Some(2),
            },
            DataType::Decimal {
                precision: Some(10),
                scale: Some(2),
            }
        );
    }

    #[test]
    fn eq_parametrized_types_with_different_params_are_not_equal() {
        assert_ne!(
            DataType::Decimal {
                precision: Some(10),
                scale: Some(2),
            },
            DataType::Decimal {
                precision: Some(18),
                scale: Some(2),
            }
        );
        assert_ne!(
            DataType::VarChar { length: Some(50) },
            DataType::VarChar { length: Some(100) }
        );
    }

    // ── Edge cases ───────────────────────────────────────

    #[test]
    fn precision_at_maximum_u8() {
        let dt = DataType::Decimal {
            precision: Some(u8::MAX),
            scale: Some(u8::MAX),
        };
        assert_eq!(
            dt,
            DataType::Decimal {
                precision: Some(255),
                scale: Some(255),
            }
        );
    }

    #[test]
    fn length_at_large_u64_value() {
        let dt = DataType::VarChar {
            length: Some(u64::MAX),
        };
        assert_eq!(
            dt,
            DataType::VarChar {
                length: Some(u64::MAX),
            }
        );
    }

    #[test]
    fn zero_length_is_valid() {
        let dt = DataType::Char { length: Some(0) };
        assert_eq!(dt, DataType::Char { length: Some(0) });
    }

    #[test]
    fn zero_precision_and_scale_is_valid() {
        let dt = DataType::Decimal {
            precision: Some(0),
            scale: Some(0),
        };
        assert_eq!(
            dt,
            DataType::Decimal {
                precision: Some(0),
                scale: Some(0),
            }
        );
    }

    // ── Cross-category distinctness ─────────────────────

    #[test]
    fn int_is_distinct_from_boolean() {
        assert_ne!(DataType::Int, DataType::Boolean);
    }

    #[test]
    fn date_is_distinct_from_timestamp() {
        assert_ne!(DataType::Date, DataType::Timestamp { precision: None });
    }

    #[test]
    fn text_is_distinct_from_blob() {
        assert_ne!(DataType::Text, DataType::Blob);
    }

    #[test]
    fn real_is_distinct_from_int() {
        assert_ne!(DataType::Real, DataType::Int);
    }
}
