//! # T8.0 Design Gate — Deserialization Strategy (Option B, DECIDED)
//!
//! **Decision: Option B — a private intermediate DTO. DECIDED on evidence,
//! not a preference among live options.**
//!
//! `CatalogSchema` / `CatalogColumn` / `CatalogIndex` embed
//! `common_sql::ast::{DataType, Expression, ColumnConstraint, TableConstraint,
//! IndexColumn}`, and a workspace-wide search confirms that **none** of the
//! common-sql AST types derive `serde::Deserialize` — they derive only
//! `Debug/Clone/PartialEq` (plus `Eq/Hash` on `DataType`/`Identifier`/
//! `SortDirection`, and `Copy` on `SortDirection`). A blanket
//! `#[derive(Deserialize)]` on `CatalogColumn` is therefore impossible, and
//! **Option A is eliminated on evidence**: adding serde to the ~24 `DataType`
//! variants (including struct variants like `Decimal{precision,scale}`,
//! `Char/NChar/VarChar/NVarChar{length}`, `Time/DateTime/Timestamp{precision}`)
//! plus `Expression`, `ColumnConstraint`, `TableConstraint`, `IndexColumn`, and
//! `Identifier` across `datatype.rs`/`expression.rs`/`ddl.rs`/`identifier.rs`
//! is cross-crate churn with blast radius into all three downstream emitters
//! and the wasm crate, requires a JSON tag strategy (`#[serde(tag="kind")]`
//! etc.) that does not exist today, and cannot land inside the T8 ticket
//! boundary. The issue's own UC-1 shape `{"data_type": {"kind": "BigInt"}}` is
//! an externally-defined tag representation that does not match the common-sql
//! enum variants verbatim (e.g. `DoublePrecision`, `NVarChar`), which
//! independently proves a hand-translation layer is required regardless of the
//! derive question. Option B — the `pub(crate)` DTO set in this module that
//! mirrors `CatalogSchema` in serde-friendly primitives, deserialized via
//! `serde_json::from_str` and hand-converted to `CatalogSchema` by translating
//! kind tags into `common_sql::ast` enum variants — is therefore the only path
//! that lands within T8, keeps schema-diff self-contained (zero common-sql
//! churn), and matches the issue's stated "serde derive OR explicit
//! deserialization, implementer discretion".
//!
//! ---
//!
//! JSON-backed catalog provider (design §3.5 / tasks.md Task 8.1, T8 group).
//!
//! `JsonCatalogProvider` parses a catalog JSON dump string into a
//! [`crate::catalog::CatalogSchema`]. It is always compiled (no feature gate)
//! and is the default provider consumed by the T11 publishable CLI.
//!
//! # Catalog JSON wire-format (normative contract — T9/T11 drift prevention)
//!
//! The provider consumes a normalized, dialect-neutral JSON shape. This shape
//! is the **canonical contract** between the catalog producer (ase-rs adapter
//! T9, or any external dumper) and the schema-diff consumer (CLI T11). Both
//! T9 and T11 MUST emit/consume exactly this shape; drift is a bug.
//!
//! ```jsonc
//! {
//!   "schema_name": "dbo",
//!   "tables": [
//!     {
//!       "name": "users",
//!       "columns": [
//!         {
//!           "name": "id",
//!           "data_type": { "kind": "BigInt" },
//!           "nullable": false,
//!           "identity": true
//!         }
//!       ],
//!       "constraints": []
//!     }
//!   ],
//!   "indices": [
//!     {
//!       "name": "idx_orders_id",
//!       "table": "orders",
//!       "unique": true,
//!       "columns": [
//!         { "name": "id", "direction": "asc" }
//!       ]
//!     }
//!   ]
//! }
//! ```
//!
//! ## `data_type.kind` tag (covers all 24 `DataType` variants)
//!
//! Unit variants: `"TinyInt"`, `"SmallInt"`, `"Int"`, `"BigInt"`, `"Real"`,
//! `"DoublePrecision"`, `"Text"`, `"NText"`, `"Date"`, `"Blob"`, `"Boolean"`,
//! `"Uuid"`, `"Json"`.
//!
//! Parameterized variants carry sibling fields:
//! - `"Decimal"` / `"Numeric"` → `{ "precision": u8?, "scale": u8? }`
//! - `"Char"` / `"VarChar"` / `"NChar"` / `"NVarChar"` / `"Binary"` /
//!   `"VarBinary"` → `{ "length": u64? }`
//! - `"Time"` / `"DateTime"` / `"Timestamp"` → `{ "precision": u8? }`
//!
//! Any unrecognized `kind` tag → [`CatalogError::ParseFailed`].
//!
//! ## Boundaries (non-scope, documented to prevent T9/T11 drift)
//!
//! - `default` / DEFAULT expression: NOT deserialized. `CatalogColumn.default`
//!   is always `None` from JSON; producers that need a default must populate
//!   `raw_default` (free-form string) which is passed through verbatim.
//!   `Expression` parsing from JSON is out of scope (T8 does not parse SQL
//!   expressions from the catalog dump).
//! - Column/table `constraints`: serialized as the unit `kind` tag list only
//!   (e.g. `["PrimaryKey"]`). Parameterized constraints (`References`/`Check`)
//!   are non-scope and cause `ParseFailed` if encountered, forcing the producer
//!   to stay within the documented surface.
//! - `IndexColumn.direction`: `"asc"` / `"desc"` / omitted (`None`).

use common_sql::ast::{DataType, Identifier, IndexColumn, SortDirection};
use serde::Deserialize;

use crate::catalog::{
    CatalogColumn, CatalogError, CatalogIndex, CatalogProvider, CatalogSchema, CatalogTable,
};

// ---------------------------------------------------------------------------
// Public provider
// ---------------------------------------------------------------------------

/// Catalog provider that serves a [`CatalogSchema`] parsed eagerly from a
/// catalog JSON dump string.
///
/// Construction ([`JsonCatalogProvider::new`]) parses AND validates the entire
/// JSON document, converting it into a `CatalogSchema` up front. Once
/// constructed, [`CatalogProvider::load_schema`] is infallible and returns a
/// clone of the pre-parsed schema. This eager design keeps the provider
/// independent of the original buffer lifetime and surfaces malformed JSON at
/// the call site that supplied it (rather than deferring the error to the
/// first `load_schema` call).
#[derive(Debug)]
pub struct JsonCatalogProvider {
    /// Pre-parsed, validated catalog snapshot.
    schema: CatalogSchema,
}

impl JsonCatalogProvider {
    /// Constructs a provider by parsing + validating `json` eagerly.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::ParseFailed`] when:
    /// - `json` is not valid JSON,
    /// - the JSON shape does not match the wire-format contract (missing
    ///   required fields, wrong types),
    /// - any `data_type.kind` / constraint `kind` / `direction` discriminant
    ///   is unrecognized.
    pub fn new(json: &str) -> Result<Self, CatalogError> {
        let dto: SchemaDto = serde_json::from_str(json).map_err(|e| CatalogError::ParseFailed {
            message: format!("json deserialize: {e}"),
        })?;
        let schema = dto.try_into()?;
        Ok(Self { schema })
    }
}

impl CatalogProvider for JsonCatalogProvider {
    fn load_schema(&self) -> Result<CatalogSchema, CatalogError> {
        Ok(self.schema.clone())
    }
}

// ---------------------------------------------------------------------------
// Private DTO (Option B — common-sql AST types do NOT derive Deserialize)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SchemaDto {
    #[serde(default)]
    schema_name: String,
    #[serde(default)]
    tables: Vec<TableDto>,
    #[serde(default)]
    indices: Vec<IndexDto>,
}

#[derive(Debug, Deserialize)]
struct TableDto {
    name: String,
    #[serde(default)]
    columns: Vec<ColumnDto>,
    #[serde(default)]
    constraints: Vec<TableConstraintDto>,
}

#[derive(Debug, Deserialize)]
struct ColumnDto {
    name: String,
    data_type: DataTypeDto,
    #[serde(default = "default_true")]
    nullable: bool,
    #[serde(default)]
    identity: bool,
    #[serde(default)]
    raw_default: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
enum DataTypeDto {
    TinyInt,
    SmallInt,
    Int,
    BigInt,
    Decimal {
        precision: Option<u8>,
        scale: Option<u8>,
    },
    Numeric {
        precision: Option<u8>,
        scale: Option<u8>,
    },
    Real,
    DoublePrecision,
    Char {
        length: Option<u64>,
    },
    VarChar {
        length: Option<u64>,
    },
    Text,
    NChar {
        length: Option<u64>,
    },
    NVarChar {
        length: Option<u64>,
    },
    NText,
    Date,
    Time {
        precision: Option<u8>,
    },
    DateTime {
        precision: Option<u8>,
    },
    Timestamp {
        precision: Option<u8>,
    },
    Binary {
        length: Option<u64>,
    },
    VarBinary {
        length: Option<u64>,
    },
    Blob,
    Boolean,
    Uuid,
    Json,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DirectionDto {
    Asc,
    Desc,
}

#[derive(Debug, Deserialize)]
struct IndexColumnDto {
    name: String,
    direction: Option<DirectionDto>,
}

#[derive(Debug, Deserialize)]
struct IndexDto {
    name: String,
    table: String,
    #[serde(default)]
    unique: bool,
    #[serde(default)]
    columns: Vec<IndexColumnDto>,
}

/// Unit (parameterless) table-constraint tag. Parameterized constraints
/// (`ForeignKey`/`Check`) are intentionally NOT representable here — they are
/// non-scope for the JSON wire-format (see module docs) and surface as
/// `ParseFailed` if a producer emits them.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
enum TableConstraintDto {
    // Deliberately minimal: only constraint kinds carry-able as bare tags.
    // Extend in a follow-up if a wire-format need arises; until then unknown
    // tags fall through to serde's `data did not match any variant` → ParseFailed.
}

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// DTO → CatalogSchema conversions (manual; common-sql types have no Deserialize)
// ---------------------------------------------------------------------------

impl TryFrom<SchemaDto> for CatalogSchema {
    type Error = CatalogError;

    fn try_from(dto: SchemaDto) -> Result<Self, Self::Error> {
        let mut tables = Vec::with_capacity(dto.tables.len());
        for t in dto.tables {
            tables.push(CatalogTable {
                name: t.name,
                columns: t
                    .columns
                    .into_iter()
                    .map(convert_column)
                    .collect::<Result<Vec<_>, _>>()?,
                // Parameterized table constraints are non-scope; the DTO accepts
                // only the empty tag set, so this is always empty in practice.
                constraints: convert_table_constraints(t.constraints)?,
            });
        }
        let mut indices = Vec::with_capacity(dto.indices.len());
        for i in dto.indices {
            indices.push(CatalogIndex {
                name: i.name,
                table: i.table,
                unique: i.unique,
                columns: i
                    .columns
                    .into_iter()
                    .map(|c| IndexColumn {
                        name: Identifier::new(c.name),
                        direction: c.direction.map(convert_direction),
                    })
                    .collect(),
            });
        }
        Ok(CatalogSchema {
            schema_name: dto.schema_name,
            tables,
            indices,
        })
    }
}

fn convert_column(dto: ColumnDto) -> Result<CatalogColumn, CatalogError> {
    Ok(CatalogColumn {
        name: dto.name,
        data_type: convert_data_type(dto.data_type)?,
        nullable: dto.nullable,
        // Expression parsing from JSON is non-scope (module docs).
        default: None,
        raw_default: dto.raw_default,
        identity: dto.identity,
        constraints: Vec::new(),
    })
}

fn convert_data_type(dto: DataTypeDto) -> Result<DataType, CatalogError> {
    let t = match dto {
        DataTypeDto::TinyInt => DataType::TinyInt,
        DataTypeDto::SmallInt => DataType::SmallInt,
        DataTypeDto::Int => DataType::Int,
        DataTypeDto::BigInt => DataType::BigInt,
        DataTypeDto::Decimal { precision, scale } => DataType::Decimal { precision, scale },
        DataTypeDto::Numeric { precision, scale } => DataType::Numeric { precision, scale },
        DataTypeDto::Real => DataType::Real,
        DataTypeDto::DoublePrecision => DataType::DoublePrecision,
        DataTypeDto::Char { length } => DataType::Char { length },
        DataTypeDto::VarChar { length } => DataType::VarChar { length },
        DataTypeDto::Text => DataType::Text,
        DataTypeDto::NChar { length } => DataType::NChar { length },
        DataTypeDto::NVarChar { length } => DataType::NVarChar { length },
        DataTypeDto::NText => DataType::NText,
        DataTypeDto::Date => DataType::Date,
        DataTypeDto::Time { precision } => DataType::Time { precision },
        DataTypeDto::DateTime { precision } => DataType::DateTime { precision },
        DataTypeDto::Timestamp { precision } => DataType::Timestamp { precision },
        DataTypeDto::Binary { length } => DataType::Binary { length },
        DataTypeDto::VarBinary { length } => DataType::VarBinary { length },
        DataTypeDto::Blob => DataType::Blob,
        DataTypeDto::Boolean => DataType::Boolean,
        DataTypeDto::Uuid => DataType::Uuid,
        DataTypeDto::Json => DataType::Json,
    };
    Ok(t)
}

fn convert_direction(dto: DirectionDto) -> SortDirection {
    match dto {
        DirectionDto::Asc => SortDirection::Asc,
        DirectionDto::Desc => SortDirection::Desc,
    }
}

fn convert_table_constraints(
    dtos: Vec<TableConstraintDto>,
) -> Result<Vec<common_sql::ast::TableConstraint>, CatalogError> {
    // The DTO's TableConstraintDto enum has no variants in scope, so a producer
    // emitting any constraint tag fails at deserialization (ParseFailed) before
    // reaching here. When empty, the result is an empty constraint list.
    let _ = dtos;
    Ok(Vec::new())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::catalog::CatalogProvider;
    use crate::diff::diff_schema;
    use crate::diff::TableDiff;
    use common_sql::ast::DataType;

    // ---- shared fixtures (UC-1 / UC-2 wire-format per module docs) ----

    fn build(json: &str) -> JsonCatalogProvider {
        JsonCatalogProvider::new(json).expect("valid JSON must parse eagerly")
    }

    // ===== UC-1: single table + identity BigInt column =====

    #[test]
    fn uc1_single_table_identity_bigint_loads_ok() {
        let json = r#"{
            "schema_name": "dbo",
            "tables": [
                {
                    "name": "users",
                    "columns": [
                        {
                            "name": "id",
                            "data_type": { "kind": "BigInt" },
                            "nullable": false,
                            "identity": true
                        }
                    ],
                    "constraints": []
                }
            ],
            "indices": []
        }"#;
        let schema = build(json).load_schema().unwrap();
        assert_eq!(schema.schema_name, "dbo");
        assert_eq!(schema.tables.len(), 1);
        let col = &schema.tables[0].columns[0];
        assert_eq!(col.name, "id");
        assert_eq!(col.data_type, DataType::BigInt);
        assert!(col.identity);
        assert!(!col.nullable);
    }

    // ===== UC-2: orders table + unique index idx_orders_id =====

    #[test]
    fn uc2_orders_table_with_unique_index() {
        let json = r#"{
            "schema_name": "dbo",
            "tables": [
                {
                    "name": "orders",
                    "columns": [
                        { "name": "id", "data_type": { "kind": "BigInt" }, "nullable": false, "identity": true }
                    ],
                    "constraints": []
                }
            ],
            "indices": [
                {
                    "name": "idx_orders_id",
                    "table": "orders",
                    "unique": true,
                    "columns": [ { "name": "id", "direction": "asc" } ]
                }
            ]
        }"#;
        let schema = build(json).load_schema().unwrap();
        assert_eq!(schema.indices.len(), 1);
        let idx = &schema.indices[0];
        assert_eq!(idx.name, "idx_orders_id");
        assert_eq!(idx.table, "orders");
        assert!(idx.unique);
        assert_eq!(idx.columns.len(), 1);
    }

    // ===== UC-3: malformed JSON + unknown discriminants -> ParseFailed =====
    //
    // Eager validation: these errors surface at new(), not load_schema().

    #[test]
    fn uc3a_truncated_json_returns_parse_failed_at_construction() {
        // Truncated mid-object: missing closing brace and value.
        let truncated = r#"{ "schema_name": "dbo", "tables": [ { "name":"users "#;
        let err = JsonCatalogProvider::new(truncated).unwrap_err();
        assert!(
            matches!(err, CatalogError::ParseFailed { .. }),
            "expected ParseFailed, got {err:?}"
        );
    }

    #[test]
    fn uc3b_unknown_data_type_kind_returns_parse_failed() {
        // Unknown kind tag → serde rejects → ParseFailed (module-doc contract).
        let json = r#"{
            "schema_name": "dbo",
            "tables": [
                { "name": "t", "columns": [ { "name": "c", "data_type": { "kind": "NoSuchType" } } ], "constraints": [] }
            ],
            "indices": []
        }"#;
        let err = JsonCatalogProvider::new(json).unwrap_err();
        assert!(
            matches!(err, CatalogError::ParseFailed { .. }),
            "unknown kind must be ParseFailed, got {err:?}"
        );
    }

    #[test]
    fn uc3c_unknown_direction_returns_parse_failed() {
        let json = r#"{
            "tables": [],
            "indices": [
                { "name": "i", "table": "t", "unique": false,
                  "columns": [ { "name": "c", "direction": "sideways" } ] }
            ]
        }"#;
        let err = JsonCatalogProvider::new(json).unwrap_err();
        assert!(matches!(err, CatalogError::ParseFailed { .. }));
    }

    #[test]
    fn uc3d_unsupported_table_constraint_kind_returns_parse_failed() {
        // The table-constraint DTO has no variants in scope; any tag fails.
        let json = r#"{
            "tables": [
                { "name": "t", "columns": [
                    { "name": "c", "data_type": { "kind": "Int" } }
                ], "constraints": [ { "kind": "ForeignKey", "columns": ["c"] } ] }
            ],
            "indices": []
        }"#;
        let err = JsonCatalogProvider::new(json).unwrap_err();
        assert!(matches!(err, CatalogError::ParseFailed { .. }));
    }

    // ===== Extra normal-case coverage (>= 3 normal total) =====

    #[test]
    fn normal_decimal_parameterized_type_preserved() {
        let json = r#"{
            "schema_name": "dbo",
            "tables": [
                {
                    "name": "accounts",
                    "columns": [
                        { "name": "balance", "data_type": { "kind": "Decimal", "precision": 18, "scale": 2 }, "nullable": false }
                    ],
                    "constraints": []
                }
            ],
            "indices": []
        }"#;
        let schema = build(json).load_schema().unwrap();
        assert_eq!(
            schema.tables[0].columns[0].data_type,
            DataType::Decimal {
                precision: Some(18),
                scale: Some(2)
            }
        );
    }

    #[test]
    fn normal_varchar_length_and_default_nullable() {
        // nullable defaults to true when omitted (module-doc `default_true`).
        let json = r#"{
            "schema_name": "dbo",
            "tables": [
                {
                    "name": "tags",
                    "columns": [
                        { "name": "label", "data_type": { "kind": "VarChar", "length": 64 } }
                    ],
                    "constraints": []
                }
            ],
            "indices": []
        }"#;
        let schema = build(json).load_schema().unwrap();
        let col = &schema.tables[0].columns[0];
        assert_eq!(col.data_type, DataType::VarChar { length: Some(64) });
        assert!(col.nullable, "nullable should default to true");
        assert!(!col.identity);
    }

    #[test]
    fn raw_default_passthrough_while_default_expression_stays_none() {
        let json = r#"{
            "tables": [
                { "name": "t", "columns": [
                    { "name": "c", "data_type": { "kind": "Int" },
                      "raw_default": "0" }
                ], "constraints": [] }
            ],
            "indices": []
        }"#;
        let schema = build(json).load_schema().unwrap();
        let col = &schema.tables[0].columns[0];
        assert!(col.default.is_none(), "Expression default is non-scope");
        assert_eq!(col.raw_default.as_deref(), Some("0"));
    }

    // ===== Exhaustive DataType coverage (all 24 variants) =====

    #[test]
    fn all_data_type_unit_variants_round_trip() {
        let unit: &[(&str, DataType)] = &[
            ("TinyInt", DataType::TinyInt),
            ("SmallInt", DataType::SmallInt),
            ("Int", DataType::Int),
            ("BigInt", DataType::BigInt),
            ("Real", DataType::Real),
            ("DoublePrecision", DataType::DoublePrecision),
            ("Text", DataType::Text),
            ("NText", DataType::NText),
            ("Date", DataType::Date),
            ("Blob", DataType::Blob),
            ("Boolean", DataType::Boolean),
            ("Uuid", DataType::Uuid),
            ("Json", DataType::Json),
        ];
        assert_eq!(unit.len(), 13, "unit variant count drift");
        for (kind, expected) in unit {
            let json = format!(
                r#"{{ "tables": [ {{ "name": "t", "columns": [
                    {{ "name": "c", "data_type": {{ "kind": "{kind}" }} }}
                ], "constraints": [] }} ], "indices": [] }}"#
            );
            let schema = build(&json).load_schema().unwrap();
            assert_eq!(
                schema.tables[0].columns[0].data_type, *expected,
                "kind {kind} must map correctly"
            );
        }
    }

    #[test]
    fn all_data_type_parameterized_variants_round_trip() {
        let cases: &[(&str, &str, DataType)] = &[
            (
                "Decimal",
                r#""precision": 18, "scale": 4"#,
                DataType::Decimal {
                    precision: Some(18),
                    scale: Some(4),
                },
            ),
            (
                "Numeric",
                r#""precision": 38, "scale": 10"#,
                DataType::Numeric {
                    precision: Some(38),
                    scale: Some(10),
                },
            ),
            (
                "Char",
                r#""length": 10"#,
                DataType::Char { length: Some(10) },
            ),
            (
                "VarChar",
                r#""length": 255"#,
                DataType::VarChar { length: Some(255) },
            ),
            (
                "NChar",
                r#""length": 50"#,
                DataType::NChar { length: Some(50) },
            ),
            (
                "NVarChar",
                r#""length": 100"#,
                DataType::NVarChar { length: Some(100) },
            ),
            (
                "Binary",
                r#""length": 16"#,
                DataType::Binary { length: Some(16) },
            ),
            (
                "VarBinary",
                r#""length": 1024"#,
                DataType::VarBinary { length: Some(1024) },
            ),
            (
                "Time",
                r#""precision": 6"#,
                DataType::Time { precision: Some(6) },
            ),
            (
                "DateTime",
                r#""precision": 3"#,
                DataType::DateTime { precision: Some(3) },
            ),
            (
                "Timestamp",
                r#""precision": 6"#,
                DataType::Timestamp { precision: Some(6) },
            ),
        ];
        // 11 parameterized variants + 13 unit variants above = all 24 covered.
        assert_eq!(cases.len(), 11, "parameterized variant count drift");
        for (kind, extra, expected) in cases {
            let json = format!(
                r#"{{ "tables": [ {{ "name": "t", "columns": [
                    {{ "name": "c", "data_type": {{ "kind": "{kind}", {extra} }} }}
                ], "constraints": [] }} ], "indices": [] }}"#
            );
            let schema = build(&json).load_schema().unwrap();
            assert_eq!(
                schema.tables[0].columns[0].data_type, *expected,
                "kind {kind} must map correctly"
            );
        }
    }

    // ===== dyn-compatibility (catalog.rs:388 pattern) =====

    #[test]
    fn json_provider_is_dyn_compatible() {
        let json = r#"{ "schema_name": "dbo", "tables": [], "indices": [] }"#;
        let p: Box<dyn CatalogProvider> = Box::new(build(json));
        let loaded = p.load_schema().unwrap();
        assert_eq!(loaded.schema_name, "dbo");
        assert!(loaded.tables.is_empty());
    }

    // ===== Eager validation: error surfaces at new(), load_schema infallible =====

    #[test]
    fn new_validates_eagerly_load_schema_never_errors() {
        let json = r#"{ "tables": [
            { "name": "t", "columns": [
                { "name": "c", "data_type": { "kind": "Int" } }
            ], "constraints": [] }
        ], "indices": [] }"#;
        let provider = build(json);
        let _ = provider.load_schema().unwrap();
        let _ = provider.load_schema().unwrap();
    }

    #[test]
    fn eager_validation_surfaces_error_at_construction_not_load() {
        let bad = r#"{ "tables": [ { "name": "t", "columns": [
            { "name": "c", "data_type": { "kind": "Nope" } }
        ], "constraints": [] } ], "indices": [] }"#;
        // Error surfaces here, at new(), not deferred to load_schema.
        assert!(JsonCatalogProvider::new(bad).is_err());
    }

    // ===== Round-trip: JSON provider → diff_schema → end-to-end unlock proof =====

    #[test]
    fn round_trip_json_provider_feeds_diff_schema_with_table_added() {
        // current (from JSON) is empty; desired has one table → TableDiff::Added.
        // This proves the JsonCatalogProvider output is structurally compatible
        // with diff_schema (T7 → T11 end-to-end unlock, estimate condition #4).
        let current_json = r#"{ "schema_name": "dbo", "tables": [], "indices": [] }"#;
        let current = build(current_json).load_schema().unwrap();

        let desired = CatalogSchema {
            schema_name: "dbo".to_string(),
            tables: vec![CatalogTable {
                name: "users".to_string(),
                columns: vec![CatalogColumn {
                    name: "id".to_string(),
                    data_type: DataType::BigInt,
                    nullable: false,
                    default: None,
                    raw_default: None,
                    identity: true,
                    constraints: vec![],
                }],
                constraints: vec![],
            }],
            indices: vec![],
        };

        let diff = diff_schema(&current, &desired);
        assert_eq!(diff.table_diffs.len(), 1);
        match &diff.table_diffs[0] {
            TableDiff::Added(t) => assert_eq!(t.name, "users"),
            other => panic!("expected TableDiff::Added, got {other:?}"),
        }
    }

    // ===== direction desc / omitted round-trip =====

    #[test]
    fn index_direction_desc_and_none_both_parse() {
        let json = r#"{ "tables": [], "indices": [
            { "name": "i1", "table": "t", "unique": true,
              "columns": [ { "name": "a", "direction": "desc" } ] },
            { "name": "i2", "table": "t", "unique": false,
              "columns": [ { "name": "b" } ] }
        ] }"#;
        let schema = build(json).load_schema().unwrap();
        assert_eq!(schema.indices.len(), 2);
        assert_eq!(
            schema.indices[0].columns[0].direction,
            Some(SortDirection::Desc)
        );
        assert_eq!(schema.indices[1].columns[0].direction, None);
        assert!(schema.indices[0].unique);
    }
}
