//! common-sql AST ↔ `CatalogSchema` conversions (design §7).
//!
//! Bidirectional mapping between the catalog data model ([`crate::catalog`])
//! and the verified `common_sql::ast` DDL nodes. The conversion is shaped by
//! the field correspondence in design §7:
//!
//! - `CatalogColumn.data_type` ↔ `ColumnDef.data_type` (direct copy, same type)
//! - `CatalogColumn.nullable` ↔ `ColumnDef.nullable`
//! - `CatalogColumn.constraints` ↔ `ColumnDef.constraints`
//! - `CatalogColumn.identity: bool` ↔ presence of `ColumnConstraint::AutoIncrement`
//!
//! The identity ↔ `AutoIncrement` mapping is intentionally symmetric so that
//! `CatalogTable → CreateTableStatement → CatalogTable` round-trips losslessly
//! (the core invariant exercised by the tests).

use common_sql::ast::{
    ColumnConstraint, ColumnDef, CreateIndexStatement, CreateTableStatement, Identifier,
    IndexColumn, QualifiedName, Span, TableOptions,
};

use crate::catalog::{CatalogColumn, CatalogIndex, CatalogTable};

/// Converts a `CreateTableStatement` (common-sql) into a [`CatalogTable`].
///
/// The qualified table name is collapsed to its bare object name
/// (`QualifiedName::name()`); schema qualification is not represented on
/// `CatalogTable` (design §3.2). `ColumnConstraint::AutoIncrement` is reflected
/// onto `CatalogColumn.identity`.
#[must_use]
pub fn create_table_to_catalog(stmt: &CreateTableStatement) -> CatalogTable {
    let columns = stmt
        .columns
        .iter()
        .map(|col| {
            let identity = col
                .constraints
                .iter()
                .any(|c| matches!(c, ColumnConstraint::AutoIncrement));
            CatalogColumn {
                name: col.name.value().to_string(),
                data_type: col.data_type.clone(),
                nullable: col.nullable,
                default: col.default.clone(),
                raw_default: None,
                identity,
                constraints: col
                    .constraints
                    .iter()
                    .filter(|c| !matches!(c, ColumnConstraint::AutoIncrement))
                    .cloned()
                    .collect(),
            }
        })
        .collect();
    CatalogTable {
        name: stmt.name.name().to_string(),
        columns,
        constraints: stmt.constraints.clone(),
    }
}

/// Converts a [`CatalogTable`] into a `CreateTableStatement` (common-sql).
///
/// `CatalogColumn.identity == true` is reified as a `ColumnConstraint::AutoIncrement`
/// (added when not already present), preserving the symmetric mapping with
/// [`create_table_to_catalog`]. The table name becomes an unqualified
/// `QualifiedName` (schema `None`).
#[must_use]
pub fn catalog_to_create_table(table: &CatalogTable) -> CreateTableStatement {
    let columns = table
        .columns
        .iter()
        .map(|col| {
            let mut constraints = col.constraints.clone();
            if col.identity
                && !constraints
                    .iter()
                    .any(|c| matches!(c, ColumnConstraint::AutoIncrement))
            {
                constraints.push(ColumnConstraint::AutoIncrement);
            }
            ColumnDef {
                span: Span::default(),
                name: Identifier::new(col.name.clone()),
                data_type: col.data_type.clone(),
                nullable: col.nullable,
                default: col.default.clone(),
                constraints,
            }
        })
        .collect();
    CreateTableStatement {
        span: Span::default(),
        if_not_exists: false,
        temporary: false,
        name: QualifiedName::new(None, table.name.clone()),
        columns,
        constraints: table.constraints.clone(),
        options: TableOptions::default(),
    }
}

/// Converts a [`CatalogIndex`] into a `CreateIndexStatement` (common-sql).
///
/// The index and table names become an `Identifier` / unqualified
/// `QualifiedName` respectively.
#[must_use]
pub fn catalog_to_create_index(idx: &CatalogIndex) -> CreateIndexStatement {
    let columns: Vec<IndexColumn> = idx
        .columns
        .iter()
        .map(|c| IndexColumn {
            name: Identifier::new(c.name.value().to_string()),
            direction: c.direction,
        })
        .collect();
    CreateIndexStatement {
        span: Span::default(),
        unique: idx.unique,
        if_not_exists: false,
        name: Identifier::new(idx.name.clone()),
        table: QualifiedName::new(None, idx.table.clone()),
        columns,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::DataType;

    fn sample_catalog_table() -> CatalogTable {
        CatalogTable {
            name: "users".to_string(),
            columns: vec![
                CatalogColumn {
                    name: "id".to_string(),
                    data_type: DataType::BigInt,
                    nullable: false,
                    default: None,
                    raw_default: None,
                    identity: true,
                    constraints: vec![ColumnConstraint::PrimaryKey],
                },
                CatalogColumn {
                    name: "email".to_string(),
                    data_type: DataType::VarChar { length: Some(255) },
                    nullable: true,
                    default: None,
                    raw_default: None,
                    identity: false,
                    constraints: vec![ColumnConstraint::Unique],
                },
            ],
            constraints: vec![],
        }
    }

    // --- create_table_to_catalog ---

    #[test]
    fn create_table_to_catalog_maps_name_and_columns() {
        let stmt = catalog_to_create_table(&sample_catalog_table());
        let catalog = create_table_to_catalog(&stmt);
        assert_eq!(catalog.name, "users");
        assert_eq!(catalog.columns.len(), 2);
        assert_eq!(catalog.columns[0].name, "id");
        assert_eq!(catalog.columns[0].data_type, DataType::BigInt);
        assert!(!catalog.columns[0].nullable);
    }

    #[test]
    fn create_table_to_catalog_reflects_auto_increment_into_identity() {
        let mut stmt = catalog_to_create_table(&sample_catalog_table());
        // Ensure the id column carries AutoIncrement via the catalog identity flag.
        stmt.columns[0].constraints.clear();
        stmt.columns[0]
            .constraints
            .push(ColumnConstraint::AutoIncrement);
        let catalog = create_table_to_catalog(&stmt);
        assert!(catalog.columns[0].identity);
    }

    #[test]
    fn create_table_to_catalog_drops_schema_qualification() {
        let mut stmt = catalog_to_create_table(&sample_catalog_table());
        stmt.name = QualifiedName::new(Some("dbo".to_string()), "users".to_string());
        let catalog = create_table_to_catalog(&stmt);
        assert_eq!(catalog.name, "users");
    }

    // --- catalog_to_create_table ---

    #[test]
    fn catalog_to_create_table_reifies_identity_as_auto_increment() {
        let stmt = catalog_to_create_table(&sample_catalog_table());
        let id_col = stmt
            .columns
            .iter()
            .find(|c| c.name.value() == "id")
            .unwrap();
        assert!(id_col
            .constraints
            .iter()
            .any(|c| matches!(c, ColumnConstraint::AutoIncrement)));
    }

    #[test]
    fn catalog_to_create_table_does_not_duplicate_auto_increment() {
        // Pre-existing AutoIncrement must not be duplicated when identity is true.
        let mut table = sample_catalog_table();
        table.columns[0].constraints = vec![ColumnConstraint::AutoIncrement];
        let stmt = catalog_to_create_table(&table);
        let id_col = stmt
            .columns
            .iter()
            .find(|c| c.name.value() == "id")
            .unwrap();
        let auto_count = id_col
            .constraints
            .iter()
            .filter(|c| matches!(c, ColumnConstraint::AutoIncrement))
            .count();
        assert_eq!(auto_count, 1);
    }

    #[test]
    fn catalog_to_create_table_emits_unqualified_name() {
        let stmt = catalog_to_create_table(&sample_catalog_table());
        assert_eq!(stmt.name.name(), "users");
        assert!(stmt.name.schema().is_none());
    }

    // --- Round-trip (core invariant, design §9) ---
    // Per estimate condition #5: assert *name string* equality, not schema
    // qualification (CatalogTable is unqualified by design §3.2).

    #[test]
    fn roundtrip_catalog_to_ast_to_catalog_preserves_names_and_types() {
        let original = sample_catalog_table();
        let roundtrip = create_table_to_catalog(&catalog_to_create_table(&original));
        assert_eq!(roundtrip.name, original.name);
        assert_eq!(roundtrip.columns.len(), original.columns.len());
        for (got, want) in roundtrip.columns.iter().zip(original.columns.iter()) {
            assert_eq!(got.name, want.name);
            assert_eq!(got.data_type, want.data_type);
            assert_eq!(got.nullable, want.nullable);
            assert_eq!(got.identity, want.identity);
            assert_eq!(got.constraints, want.constraints);
        }
    }

    #[test]
    fn roundtrip_preserves_identity_flag() {
        let mut original = sample_catalog_table();
        original.columns[0].identity = true;
        original.columns[0].constraints.clear();
        let roundtrip = create_table_to_catalog(&catalog_to_create_table(&original));
        assert!(roundtrip.columns[0].identity);
    }

    #[test]
    fn roundtrip_preserves_multiple_data_types() {
        let table = CatalogTable {
            name: "t".to_string(),
            columns: vec![
                CatalogColumn {
                    name: "a".to_string(),
                    data_type: DataType::Int,
                    nullable: false,
                    default: None,
                    raw_default: None,
                    identity: false,
                    constraints: vec![],
                },
                CatalogColumn {
                    name: "b".to_string(),
                    data_type: DataType::VarChar { length: None },
                    nullable: true,
                    default: None,
                    raw_default: None,
                    identity: false,
                    constraints: vec![],
                },
            ],
            constraints: vec![],
        };
        let roundtrip = create_table_to_catalog(&catalog_to_create_table(&table));
        assert_eq!(roundtrip.columns[0].data_type, DataType::Int);
        assert_eq!(
            roundtrip.columns[1].data_type,
            DataType::VarChar { length: None }
        );
    }

    // --- catalog_to_create_index ---

    #[test]
    fn catalog_to_create_index_maps_basic_fields() {
        let idx = CatalogIndex {
            name: "idx_users_email".to_string(),
            table: "users".to_string(),
            columns: vec![IndexColumn {
                name: Identifier::new("email".to_string()),
                direction: None,
            }],
            unique: true,
        };
        let stmt = catalog_to_create_index(&idx);
        assert_eq!(stmt.name.value(), "idx_users_email");
        assert_eq!(stmt.table.name(), "users");
        assert!(stmt.unique);
        assert_eq!(stmt.columns.len(), 1);
        assert_eq!(stmt.columns[0].name.value(), "email");
    }

    #[test]
    fn catalog_to_create_index_preserves_direction() {
        use common_sql::ast::SortDirection;
        let idx = CatalogIndex {
            name: "i".to_string(),
            table: "t".to_string(),
            columns: vec![IndexColumn {
                name: Identifier::new("c".to_string()),
                direction: Some(SortDirection::Desc),
            }],
            unique: false,
        };
        let stmt = catalog_to_create_index(&idx);
        assert_eq!(stmt.columns[0].direction, Some(SortDirection::Desc));
    }

    #[test]
    fn catalog_to_create_index_empty_columns_allowed() {
        let idx = CatalogIndex {
            name: "empty".to_string(),
            table: "t".to_string(),
            columns: vec![],
            unique: false,
        };
        let stmt = catalog_to_create_index(&idx);
        assert!(stmt.columns.is_empty());
        assert!(stmt.table.schema().is_none());
    }

    // --- ast import sanity (guards against accidental shadowing) ---

    #[test]
    fn create_table_type_alias_resolves() {
        // Compile-time check that the imported `CreateTableStatement` type is
        // the expected common-sql node (guards against accidental shadowing).
        let _: CreateTableStatement = catalog_to_create_table(&sample_catalog_table());
    }
}
