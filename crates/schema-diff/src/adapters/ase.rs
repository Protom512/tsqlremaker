//! # ASE catalog adapter (design §3.5 / §0.1 — `ase` feature gate)
//!
//! Live ASE catalog introspection backed by the `ase-driver` crate (the
//! library member of the Sou-Tokuda/ase-rs workspace; the workspace root has
//! no `ase-rs` library crate, so design.md §6.1's `dep:ase-rs` is corrected to
//! `dep:ase-driver` per CTO condition #1 / 2026-07-14 gate-blocked judgment).
//!
//! Compiled **only** under `--features ase` for everything that touches
//! `ase_driver` / `ase_types`. The pure, dialect-neutral ASE type-label →
//! [`common_sql::ast::DataType`] mapping ([`map_ase_type_name`]) is compiled
//! unconditionally so that the **default (publishable) build never resolves
//! the ase-driver git dependency** (design §0.1 / AC-5 / AC-6) while the unit
//! tests for the mapping still run in CI.
//!
//! # Async boundary (CTO condition #2)
//!
//! `ase_driver::Connection::query` is `async`, but
//! [`crate::catalog::CatalogProvider::load_schema`] is synchronous (frozen in
//! T6). `AseCatalogProvider::load_schema` is intended to bridge the two by
//! driving a private tokio runtime via `Runtime::block_on` once introspection
//! lands. This means `load_schema` MUST NOT be called from within an existing
//! tokio runtime (nested-runtime panic). If the caller becomes async in the
//! future, the trait itself must be revisited (`async-trait` +
//! JsonCatalogProvider impact) — out of scope for T9 (forward path documented
//! here, not taken).
//!
//! # Catalog introspection scope (CTO condition #3)
//!
//! design.md specifies NO concrete `sysobjects` / `syscolumns` / `sysindexes`
//! query. To avoid shipping unreviewed SQL inside an untested code path, the
//! full introspection (system-table reads, `syscolumns.type` status codes →
//! `AseDataType`, NULLability / index extraction) is deferred to a T9b
//! follow-up. `load_schema` surfaces this explicitly via
//! [`crate::catalog::CatalogError::NotImplemented`]. The CI-verifiable surface
//! in T9 is the pure ASE type-label → `DataType` mapping (default-build unit
//! tests) plus the `AseDataType`-based [`map_ase_type`] (feature-gated, local
//! `--features ase` only).

use common_sql::ast::DataType;

use crate::catalog::CatalogError;

// ---------------------------------------------------------------------------
// Pure mapping: ASE type-name label → common-sql DataType (default-build)
// ---------------------------------------------------------------------------
//
// ASE catalog introspection (ase-driver `Column::type_name`, or the
// `syscolumns.type` status-code decode) ultimately yields a type-label string.
// This pure function maps that label to a dialect-neutral `DataType` with no
// dependency on ase-driver, so it is unit-testable on the default build (the
// `ase` feature is offline-only and CI-skipped). It is the CI-verifiable
// surface for T9 and freezes the ASE-label→common-sql mapping that design §0.6
// left undefined (CTO 2026-07-20 note).
//
// Parameterized length / precision defaults to `None` here; the concrete
// values are carried through in T9b once syscolumns reads land.

/// Map an ASE catalog type-name label to a dialect-neutral [`DataType`].
///
/// `label` is matched case-insensitively with internal whitespace collapsed
/// (so `"Double Precision"` and `"double  precision"` are equivalent).
///
/// # Errors
///
/// Returns [`CatalogError::UnsupportedCatalogShape`] for ASE types that have
/// no faithful common-sql representation — `money`/`smallmoney`, `bit`,
/// `image`, the high-precision `bigdatetime`/`bigtime` (need a T9b precision
/// policy), and any unrecognized label.
///
/// # ASE → common-sql mapping (frozen here)
///
/// | ASE label            | common-sql `DataType`        | notes                          |
/// |----------------------|------------------------------|--------------------------------|
/// | `tinyint`            | `TinyInt`                    | 1-byte                         |
/// | `smallint`           | `SmallInt`                   | 2-byte                         |
/// | `int` / `integer`    | `Int`                        | 4-byte                         |
/// | `bigint`             | `BigInt`                     | 8-byte                         |
/// | `unsigned smallint`  | `SmallInt`                   | lossy; ASE-only unsigned       |
/// | `unsigned int`       | `Int`                        | lossy; ASE-only unsigned       |
/// | `unsigned bigint`    | `BigInt`                     | lossy; ASE-only unsigned       |
/// | `real`               | `Real`                       | 4-byte float                   |
/// | `double precision` / `float` | `DoublePrecision`    | 8-byte float                   |
/// | `decimal`            | `Decimal`                    | precision/scale T9b            |
/// | `numeric`            | `Numeric`                    | precision/scale T9b            |
/// | `char`               | `Char`                       | length T9b                     |
/// | `varchar`            | `VarChar`                    | length T9b                     |
/// | `nchar`              | `NChar`                      | length T9b                     |
/// | `nvarchar`           | `NVarChar`                   | length T9b                     |
/// | `text`               | `Text`                       | LOB                            |
/// | `unitext`            | `NText`                      | national LOB                   |
/// | `date`               | `Date`                       |                                |
/// | `time`               | `Time`                       | precision T9b                  |
/// | `datetime` / `smalldatetime` | `DateTime`           | precision T9b                  |
/// | `binary`             | `Binary`                     | length T9b                     |
/// | `varbinary`          | `VarBinary`                  | length T9b                     |
/// | `timestamp`          | `VarBinary`                  | ASE row-version binary(8)      |
/// | `money` / `smallmoney` | —                          | `UnsupportedCatalogShape`      |
/// | `bit`                | —                            | `UnsupportedCatalogShape`      |
/// | `image`              | —                            | `UnsupportedCatalogShape`      |
/// | `bigdatetime` / `bigtime` | —                       | `UnsupportedCatalogShape` (T9b)|
//
// NOTE: no `#[must_use]` — `Result<_, CatalogError>` is already `must_use` by
// virtue of `CatalogError: std::error::Error`, and adding the attribute
// trips `clippy::double_must_use`.
pub fn map_ase_type_name(label: &str) -> Result<DataType, CatalogError> {
    let key = normalize_label(label);
    match key.as_str() {
        // ---- integers (ASE unsigned variants map lossily to signed) ----
        "tinyint" => Ok(DataType::TinyInt),
        "smallint" | "unsigned smallint" => Ok(DataType::SmallInt),
        "int" | "integer" | "unsigned int" => Ok(DataType::Int),
        "bigint" | "unsigned bigint" => Ok(DataType::BigInt),
        // ---- decimals ----
        "decimal" => Ok(DataType::Decimal {
            precision: None,
            scale: None,
        }),
        "numeric" => Ok(DataType::Numeric {
            precision: None,
            scale: None,
        }),
        // ---- floats ----
        "real" => Ok(DataType::Real),
        "double precision" | "float" => Ok(DataType::DoublePrecision),
        // ---- character ----
        "char" | "character" => Ok(DataType::Char { length: None }),
        "varchar" | "char varying" | "character varying" => Ok(DataType::VarChar { length: None }),
        "nchar" | "national char" | "national character" => Ok(DataType::NChar { length: None }),
        "nvarchar" | "national char varying" | "national character varying" | "nchar varying" => {
            Ok(DataType::NVarChar { length: None })
        }
        "text" => Ok(DataType::Text),
        "unitext" => Ok(DataType::NText),
        // ---- date/time ----
        "date" => Ok(DataType::Date),
        "time" => Ok(DataType::Time { precision: None }),
        "datetime" | "smalldatetime" => Ok(DataType::DateTime { precision: None }),
        // ---- binary ----
        "binary" => Ok(DataType::Binary { length: None }),
        "varbinary" | "binary varying" => Ok(DataType::VarBinary { length: None }),
        // ASE `timestamp` is a row-version binary(8) surrogate (NOT SQL-standard
        // TIMESTAMP; ASE has no such type). Model as binary so it round-trips.
        "timestamp" => Ok(DataType::VarBinary { length: None }),
        // ---- unsupported (CTO condition #3: defer / surface explicitly) ----
        "money" | "smallmoney" => Err(unsupported("ASE money types have no common-sql shape")),
        "bit" => Err(unsupported("ASE bit type has no common-sql shape")),
        "image" => Err(unsupported("ASE image LOB has no common-sql shape")),
        "bigdatetime" | "bigtime" => Err(unsupported(
            "ASE bigdatetime/bigtime need a T9b precision policy",
        )),
        // ---- unknown ----
        other => Err(unsupported(&format!(
            "unrecognized ASE type label '{other}'"
        ))),
    }
}

/// Lowercase and collapse internal ASCII whitespace so `"Double Precision"`
/// and `"double  precision"` both normalize to `"double precision"`.
fn normalize_label(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    let mut prev_space = false;
    for ch in label.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(lower);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn unsupported(detail: &str) -> CatalogError {
    CatalogError::UnsupportedCatalogShape {
        detail: detail.to_string(),
    }
}

// ---------------------------------------------------------------------------
// AseDataType → DataType (feature-gated; consumes ase-types)
// ---------------------------------------------------------------------------

/// Map an upstream `ase_types::AseDataType` to a dialect-neutral [`DataType`].
///
/// Forwarding wrapper over [`map_ase_type_name`]: converts the upstream enum
/// to its canonical ASE label string and delegates. ASE types that common-sql
/// cannot represent (`Money`/`Money4`, `Bit`, `Image`, `Null`, and the
/// high-precision `BigDateTime`/`BigTime`) return
/// [`CatalogError::UnsupportedCatalogShape`].
///
/// Only compiled under `--features ase`.
///
/// # Errors
///
/// See [`map_ase_type_name`].
#[cfg(feature = "ase")]
pub fn map_ase_type(ty: &ase_types::AseDataType) -> Result<DataType, CatalogError> {
    use ase_types::AseDataType;
    let label = match ty {
        AseDataType::Int1 => "tinyint",
        AseDataType::Int2 => "smallint",
        AseDataType::Int4 => "int",
        AseDataType::Int8 => "bigint",
        AseDataType::UInt2 => "unsigned smallint",
        AseDataType::UInt4 => "unsigned int",
        AseDataType::UInt8 => "unsigned bigint",
        AseDataType::Flt4 => "real",
        AseDataType::Flt8 => "double precision",
        AseDataType::Decimal => "decimal",
        AseDataType::Money => "money",
        AseDataType::Money4 => "smallmoney",
        AseDataType::Char => "char",
        AseDataType::VarChar => "varchar",
        AseDataType::Text => "text",
        AseDataType::Binary => "binary",
        AseDataType::VarBinary => "varbinary",
        AseDataType::Image => "image",
        AseDataType::Date => "date",
        AseDataType::Time => "time",
        AseDataType::DateTime => "datetime",
        AseDataType::BigDateTime => "bigdatetime",
        AseDataType::BigTime => "bigtime",
        AseDataType::Bit => "bit",
        AseDataType::Null => return Err(unsupported("ASE Null type is not a column type")),
    };
    map_ase_type_name(label)
}

// ---------------------------------------------------------------------------
// AseCatalogProvider (feature-gated)
// ---------------------------------------------------------------------------

/// Catalog provider backed by a live ASE connection (`ase_driver`).
///
/// Constructed from a DSN connection string via [`AseCatalogProvider::new`].
/// [`crate::catalog::CatalogProvider::load_schema`] is the eventual site of
/// the async-bridge (DSN → `ase_dsn::DsnParser::parse_uri` →
/// `ase_tds::connection::TdsConnection::connect` → `ase_driver::Connection`,
/// driven to completion on a private tokio runtime via `Runtime::block_on`).
/// Per CTO condition #3 / estimate scope-narrowing, the connection path +
/// introspection SQL land in **T9b**, so `load_schema` currently surfaces
/// [`crate::catalog::CatalogError::NotImplemented`] and stores only the DSN.
///
/// See the module-level async-boundary note: callers MUST NOT invoke
/// `load_schema` from within an existing tokio runtime (nested-runtime panic)
/// once T9b wires `Runtime::block_on`.
///
/// Compiled only under `--features ase`.
#[cfg(feature = "ase")]
#[derive(Debug)]
pub struct AseCatalogProvider {
    /// Connection string as supplied by the caller (env-injected in tests).
    dsn: String,
}

#[cfg(feature = "ase")]
impl AseCatalogProvider {
    /// Construct a provider for the ASE instance reachable at `dsn`.
    ///
    /// Connection is deferred to
    /// [`crate::catalog::CatalogProvider::load_schema`]; `new` only validates
    /// that `dsn` is non-empty so an obviously-bad caller fails fast at
    /// construction rather than at the (network-gated) load site.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::AccessFailed`] if `dsn` is empty/whitespace-only.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(dsn: impl Into<String>) -> Result<Self, CatalogError> {
        let dsn = dsn.into();
        if dsn.trim().is_empty() {
            return Err(CatalogError::AccessFailed {
                message: "ASE DSN is empty".to_string(),
            });
        }
        Ok(Self { dsn })
    }

    /// The DSN this provider was constructed with (test/diagnostic access).
    #[must_use]
    pub fn dsn(&self) -> &str {
        &self.dsn
    }

    /// Build a single-threaded tokio runtime for driving the async
    /// `ase-driver` query to completion from the sync trait method.
    ///
    /// A fresh runtime per `load_schema` call — this is a network-bound
    /// operation (live ASE connect + introspection) invoked rarely (the
    /// `#[ignore]` integration test today, T9b introspection later), so the
    /// construction cost is negligible and avoids the unstable
    /// `OnceLock::get_or_try_init` plus the lifetime/guard complications of
    /// caching a `Runtime` in a `Mutex`. `current_thread` + `enable_all` is
    /// sufficient — schema introspection is not CPU-parallel, and `ase-tds`
    /// needs the `net`/`io`/`time` drivers `enable_all` turns on.
    fn runtime() -> Result<tokio::runtime::Runtime, CatalogError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| CatalogError::AccessFailed {
                message: format!("failed to create tokio runtime: {e}"),
            })
    }

    /// Open the ASE connection (async) and introspect the catalog.
    ///
    /// `Runtime::block_on` panics if called from inside an existing tokio
    /// context (see module "Async boundary"). All current call sites are
    /// synchronous (the T11 CLI).
    async fn load_schema_async(&self) -> Result<crate::catalog::CatalogSchema, CatalogError> {
        // 1. DSN string → ConnectionConfig (ase-dsn). `parse_uri` accepts the
        //    full `ase://user:pass@host:port[/db]` form.
        let config =
            ase_dsn::DsnParser::parse_uri(&self.dsn).map_err(|e| CatalogError::AccessFailed {
                message: format!("ASE DSN parse failed: {e}"),
            })?;

        // 2. Establish the TDS connection, then wrap in the high-level driver.
        //    This is the step the live `#[ignore]` integration test exercises
        //    end-to-end against a real ASE endpoint.
        let tds = ase_tds::connection::TdsConnection::connect(config)
            .await
            .map_err(|e| CatalogError::AccessFailed {
                message: format!("ASE connect failed: {e}"),
            })?;
        let _conn: ase_driver::Connection = ase_driver::Connection::new(tds);

        // 3. Catalog introspection (sysobjects/syscolumns/sysindexes) is NOT
        //    specified in design.md (CTO 2026-07-14 condition #3) — deferred
        //    to T9b. Until then we surface NotImplemented explicitly rather
        //    than ship unreviewed SQL inside an untested code path.
        Err(CatalogError::NotImplemented {
            what: "AseCatalogProvider catalog introspection (T9b)".to_string(),
        })
    }
}

#[cfg(feature = "ase")]
impl crate::catalog::CatalogProvider for AseCatalogProvider {
    fn load_schema(&self) -> Result<crate::catalog::CatalogSchema, CatalogError> {
        // The connection path (DSN → TdsConnection::connect →
        // ase_driver::Connection) IS wired so the live `#[ignore]` integration
        // test can assert the connection succeeds end-to-end. The
        // introspection SQL itself (sysobjects/syscolumns/sysindexes) is NOT
        // specified in design.md (CTO 2026-07-14 condition #3) and lands in
        // T9b — `load_schema_async` returns NotImplemented after a successful
        // connect.
        //
        // Drive the async driver on a private runtime. See the module
        // "Async boundary" note: `block_on` panics if the caller is already
        // inside a tokio context — schema-diff's sync CLI is the only caller.
        let rt = Self::runtime()?;
        rt.block_on(self.load_schema_async())
    }
}

// ===========================================================================
// Tests — pure mapping (default build, CI-verifiable)
// ===========================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::DataType;

    // ===== map_ase_type_name: normal cases (>= 3 happy paths) =====

    #[test]
    fn uc1_int_maps_to_common_int() {
        assert_eq!(map_ase_type_name("int").unwrap(), DataType::Int);
    }

    #[test]
    fn uc1_varchar_maps_unparameterized() {
        assert_eq!(
            map_ase_type_name("varchar").unwrap(),
            DataType::VarChar { length: None }
        );
    }

    #[test]
    fn uc1_datetime_maps_to_common_datetime() {
        assert_eq!(
            map_ase_type_name("datetime").unwrap(),
            DataType::DateTime { precision: None }
        );
    }

    // ===== additional normal-case coverage =====

    #[test]
    fn bigint_and_integer_aliases_resolve() {
        assert_eq!(map_ase_type_name("bigint").unwrap(), DataType::BigInt);
        assert_eq!(map_ase_type_name("integer").unwrap(), DataType::Int);
    }

    #[test]
    fn unsigned_int_maps_lossily_to_signed() {
        // ASE unsigned int -> common-sql Int (lossy; documented in mapping table).
        assert_eq!(map_ase_type_name("unsigned int").unwrap(), DataType::Int);
        assert_eq!(
            map_ase_type_name("unsigned bigint").unwrap(),
            DataType::BigInt
        );
    }

    #[test]
    fn decimal_and_numeric_distinct() {
        assert_eq!(
            map_ase_type_name("decimal").unwrap(),
            DataType::Decimal {
                precision: None,
                scale: None
            }
        );
        assert_eq!(
            map_ase_type_name("numeric").unwrap(),
            DataType::Numeric {
                precision: None,
                scale: None
            }
        );
        assert_ne!(
            map_ase_type_name("decimal").unwrap(),
            map_ase_type_name("numeric").unwrap()
        );
    }

    #[test]
    fn float_alias_maps_to_double_precision() {
        // ASE `float` is an 8-byte double.
        assert_eq!(
            map_ase_type_name("float").unwrap(),
            DataType::DoublePrecision
        );
        assert_eq!(map_ase_type_name("real").unwrap(), DataType::Real);
    }

    #[test]
    fn national_char_variants_map() {
        assert_eq!(
            map_ase_type_name("nvarchar").unwrap(),
            DataType::NVarChar { length: None }
        );
        assert_eq!(
            map_ase_type_name("national char varying").unwrap(),
            DataType::NVarChar { length: None }
        );
        assert_eq!(map_ase_type_name("unitext").unwrap(), DataType::NText);
    }

    #[test]
    fn binary_family_maps() {
        assert_eq!(
            map_ase_type_name("binary").unwrap(),
            DataType::Binary { length: None }
        );
        assert_eq!(
            map_ase_type_name("varbinary").unwrap(),
            DataType::VarBinary { length: None }
        );
        // ASE `timestamp` is a row-version binary surrogate, not SQL TIMESTAMP.
        assert_eq!(
            map_ase_type_name("timestamp").unwrap(),
            DataType::VarBinary { length: None }
        );
    }

    // ===== edge cases: UnsupportedCatalogShape =====

    #[test]
    fn uc2_money_is_unsupported_catalog_shape() {
        let err = map_ase_type_name("money").unwrap_err();
        assert!(
            matches!(err, CatalogError::UnsupportedCatalogShape { .. }),
            "expected UnsupportedCatalogShape, got {err:?}"
        );
    }

    #[test]
    fn bit_and_image_are_unsupported() {
        assert!(matches!(
            map_ase_type_name("bit").unwrap_err(),
            CatalogError::UnsupportedCatalogShape { .. }
        ));
        assert!(matches!(
            map_ase_type_name("image").unwrap_err(),
            CatalogError::UnsupportedCatalogShape { .. }
        ));
    }

    #[test]
    fn bigdatetime_bigtime_deferred_to_t9b() {
        for label in ["bigdatetime", "bigtime"] {
            let err = map_ase_type_name(label).unwrap_err();
            assert!(
                matches!(err, CatalogError::UnsupportedCatalogShape { .. }),
                "{label} should be UnsupportedCatalogShape (T9b), got {err:?}"
            );
        }
    }

    #[test]
    fn unrecognized_label_is_unsupported_shape() {
        let err = map_ase_type_name("definitely_not_a_type").unwrap_err();
        assert!(matches!(err, CatalogError::UnsupportedCatalogShape { .. }));
    }

    // ===== normalization (case + whitespace) =====

    #[test]
    fn case_insensitive_and_whitespace_collapsed() {
        assert_eq!(
            map_ase_type_name("  Double  Precision  ").unwrap(),
            DataType::DoublePrecision
        );
        assert_eq!(map_ase_type_name("INT").unwrap(), DataType::Int);
        assert_eq!(
            map_ase_type_name("VarChar").unwrap(),
            DataType::VarChar { length: None }
        );
    }

    #[test]
    fn empty_label_is_unrecognized() {
        let err = map_ase_type_name("").unwrap_err();
        assert!(matches!(err, CatalogError::UnsupportedCatalogShape { .. }));
    }

    // ===== exhaustive: supported resolves, rejected errors =====

    #[test]
    fn all_supported_labels_resolve_without_error() {
        let supported = [
            "tinyint",
            "smallint",
            "int",
            "integer",
            "bigint",
            "unsigned smallint",
            "unsigned int",
            "unsigned bigint",
            "real",
            "double precision",
            "float",
            "decimal",
            "numeric",
            "char",
            "character",
            "varchar",
            "char varying",
            "character varying",
            "nchar",
            "national char",
            "national character",
            "nvarchar",
            "national char varying",
            "national character varying",
            "nchar varying",
            "text",
            "unitext",
            "date",
            "time",
            "datetime",
            "smalldatetime",
            "binary",
            "varbinary",
            "binary varying",
            "timestamp",
        ];
        for label in supported {
            assert!(
                map_ase_type_name(label).is_ok(),
                "supported label '{label}' should resolve"
            );
        }
    }

    #[test]
    fn all_rejected_labels_return_unsupported_shape() {
        let rejected = [
            "money",
            "smallmoney",
            "bit",
            "image",
            "bigdatetime",
            "bigtime",
        ];
        for label in rejected {
            let err = map_ase_type_name(label).unwrap_err();
            assert!(
                matches!(err, CatalogError::UnsupportedCatalogShape { .. }),
                "rejected label '{label}' should be UnsupportedCatalogShape, got {err:?}"
            );
        }
    }

    // ===== dyn-compatibility parity with json.rs (T9.3 forward note) =====

    #[test]
    fn map_ase_type_name_is_pure_no_io() {
        // Calling twice with the same input yields the same result (pure fn).
        let a = map_ase_type_name("int");
        let b = map_ase_type_name("int");
        assert_eq!(a, b);
    }
}

// ===========================================================================
// Feature-gated tests: map_ase_type + AseCatalogProvider (local --features ase)
// ===========================================================================
//
// These run only under `cargo nextest run -p schema-diff --features ase`, which
// resolves the private upstream and is therefore local-only (CI runs the
// default build per design §0.1 / AC-5).

#[cfg(all(test, feature = "ase"))]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod ase_feature_tests {
    use super::*;
    use crate::catalog::CatalogProvider;
    use ase_types::AseDataType;

    // ===== map_ase_type happy paths =====

    #[test]
    fn map_ase_type_int4_to_int() {
        assert_eq!(map_ase_type(&AseDataType::Int4).unwrap(), DataType::Int);
    }

    #[test]
    fn map_ase_type_varchar_unparameterized() {
        assert_eq!(
            map_ase_type(&AseDataType::VarChar).unwrap(),
            DataType::VarChar { length: None }
        );
    }

    #[test]
    fn map_ase_type_datetime_unparameterized() {
        assert_eq!(
            map_ase_type(&AseDataType::DateTime).unwrap(),
            DataType::DateTime { precision: None }
        );
    }

    // ===== map_ase_type edge (UnsupportedCatalogShape) =====

    #[test]
    fn map_ase_type_money_unsupported() {
        let err = map_ase_type(&AseDataType::Money).unwrap_err();
        assert!(matches!(err, CatalogError::UnsupportedCatalogShape { .. }));
    }

    #[test]
    fn map_ase_type_bit_image_null_unsupported() {
        for ty in [AseDataType::Bit, AseDataType::Image, AseDataType::Null] {
            let err = map_ase_type(&ty).unwrap_err();
            assert!(
                matches!(err, CatalogError::UnsupportedCatalogShape { .. }),
                "{ty:?} should be UnsupportedCatalogShape"
            );
        }
    }

    // ===== AseCatalogProvider::new (DSN validation, no network) =====

    #[test]
    fn new_stores_dsn_and_defers_connection() {
        let p = AseCatalogProvider::new("ase://user:pass@host:5000").unwrap();
        assert_eq!(p.dsn(), "ase://user:pass@host:5000");
    }

    #[test]
    fn new_rejects_empty_dsn() {
        assert!(AseCatalogProvider::new("").is_err());
        assert!(AseCatalogProvider::new("   ").is_err());
    }

    #[test]
    fn ase_provider_is_dyn_compatible_with_catalog_provider() {
        // CatalogProvider is object-safe (catalog.rs); the ase adapter must
        // satisfy the same `Box<dyn>` shape as JsonCatalogProvider. We assert
        // object-safety by coercion alone — deliberately NOT calling
        // `load_schema()` here, because that would attempt a real network
        // connect (flaky/slow in CI). The vtable-dispatch contract is covered
        // by the `#[ignore]` live test instead.
        let provider = AseCatalogProvider::new("ase://u:p@h:5000").unwrap();
        assert_eq!(provider.dsn(), "ase://u:p@h:5000");
        // Coercion to `Box<dyn CatalogProvider>` compiles iff the trait is
        // object-safe AND AseCatalogProvider implements it — that is the test.
        let _coerced: Box<dyn crate::catalog::CatalogProvider> = Box::new(provider);
    }

    // ===== load_schema stub contract (no network) =====
    //
    // design.md does not specify the introspection SQL (CTO 2026-07-14
    // condition #3); the stub short-circuits to NotImplemented BEFORE opening
    // a connection... EXCEPT the DSN parse step runs first. To keep this unit
    // test network-free, the DSN is malformed so the DSN parse fails with
    // AccessFailed rather than reaching the network-bound connect step.

    #[test]
    fn load_schema_returns_access_failed_on_malformed_dsn_without_network() {
        // A malformed DSN fails at ase_dsn::DsnParser::parse before any socket
        // is opened, so this needs no ASE endpoint. The exact error is
        // AccessFailed (the connect/parse family) — never NotImplemented,
        // because parse runs before the stubbed introspection branch.
        let p = AseCatalogProvider::new("not a valid dsn at all").unwrap();
        let err = p.load_schema().unwrap_err();
        assert!(
            matches!(err, CatalogError::AccessFailed { .. }),
            "malformed DSN should surface AccessFailed, got {err:?}"
        );
    }

    // ===== Live ASE integration smoke test (#[ignore] — CI skips) =====
    //
    // Run locally with:
    //   ASE_TEST_DSN="ase://user:pass@host:5000" \
    //     cargo nextest run -p schema-diff --features ase --run-ignored only
    //
    // T9.3 acceptance: `AseCatalogProvider::new(dsn).load_schema()` exercises
    // the full connection path (DSN parse → TdsConnection::connect →
    // ase_driver::Connection) against a real ASE endpoint. The introspection
    // body is deferred to T9b (returns NotImplemented today), so the verifiable
    // contract is: the CONNECTION PATH must not error with AccessFailed. Once
    // T9b implements the body, `load_schema` returns `Ok(schema)` and this
    // assertion upgrades naturally to a full `is_ok()` pass.

    /// Live ASE integration smoke test against `ASE_TEST_DSN`.
    ///
    /// Marked `#[ignore]` because it requires a reachable ASE instance and
    /// MUST be skipped in CI (AC: no network dependency in the default or
    /// `--features ase` CI build beyond git-dep resolution).
    #[test]
    #[ignore = "requires live ASE endpoint via ASE_TEST_DSN; run with --run-ignored only"]
    fn live_load_schema_returns_ok_against_real_ase() {
        let dsn = std::env::var("ASE_TEST_DSN").unwrap_or_else(|_| {
            panic!(
                "ASE_TEST_DSN must be set to a reachable ASE connection string \
                 (e.g. ase://user:pass@host:5000) to run the live integration test"
            )
        });

        let provider = AseCatalogProvider::new(dsn).expect("DSN must construct a provider");
        let result = provider.load_schema();

        // Today (pre-T9b) load_schema returns NotImplemented after a successful
        // connect. Accept either Ok (T9b+) or NotImplemented (connect succeeded,
        // introspection pending) as a PASS — both prove the connection path is
        // sound. A connect/parse AccessFailed is a hard FAIL.
        match &result {
            Ok(_) => {}
            Err(CatalogError::NotImplemented { .. }) => {}
            Err(other) => panic!(
                "live ASE connection path failed (expected Ok or NotImplemented \
                 after a successful connect, got {other:?})"
            ),
        }
    }
}
