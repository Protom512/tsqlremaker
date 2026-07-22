//! Catalog data models (design §3).
//!
//! Dialect-neutral representation of ASE catalog introspection results.
//! Conversion to/from `common_sql::ast` is performed by the [`crate::mapper`]
//! module.

use common_sql::ast::{ColumnConstraint, DataType, Expression, IndexColumn, TableConstraint};

/// カタログ全体 (1スキーマ分)。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatalogSchema {
    /// スキーマ名 (通常 "dbo")。
    pub schema_name: String,
    /// テーブル一覧 (テーブル名でソート済み)。
    pub tables: Vec<CatalogTable>,
    /// インデックス一覧 (テーブル名・インデックス名でソート済み)。
    pub indices: Vec<CatalogIndex>,
}

/// カタログから取得した1テーブル情報。
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogTable {
    /// テーブル名 (スキーマ修飾なし)。
    pub name: String,
    /// カラム一覧 (序数順)。
    pub columns: Vec<CatalogColumn>,
    /// テーブルレベル制約一覧。
    pub constraints: Vec<TableConstraint>,
}

/// カタログから取得した1カラム情報。
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogColumn {
    /// カラム名。
    pub name: String,
    /// データ型 (common-sql 表現)。
    pub data_type: DataType,
    /// NULL 許容 (true = NULL可)。
    pub nullable: bool,
    /// DEFAULT 式 (パース済みの場合。未パースの文字列の場合は None、別途 raw_default に保持)。
    pub default: Option<Expression>,
    /// DEFAULT 式の生文字列 (式パース失敗時のフォールバック)。
    pub raw_default: Option<String>,
    /// IDENTITY / AUTO_INCREMENT 指定。
    pub identity: bool,
    /// カラムレベル制約一覧。
    pub constraints: Vec<ColumnConstraint>,
}

/// カタログから取得した1インデックス情報。
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogIndex {
    /// インデックス名。
    pub name: String,
    /// 対象テーブル名。
    pub table: String,
    /// インデックス対象カラム (序数順、ソート方向付き)。
    pub columns: Vec<IndexColumn>,
    /// UNIQUE 指定。
    pub unique: bool,
}

/// ASE カタログ (または同等の JSON dump) からスキーマ情報を取得する契約。
///
/// `ase` feature が有効な場合のみ ase-rs を叩く実装 (T9) が提供される。
/// feature 無しビルドでは `JsonCatalogProvider` (T11 用) のみが利用可能。
pub trait CatalogProvider {
    /// スキーマ全体を取得する。
    ///
    /// # Errors
    /// カタログアクセス失敗 (接続エラー・権限不足等) の場合 `CatalogError` を返す。
    fn load_schema(&self) -> Result<CatalogSchema, CatalogError>;
}

/// カタログ取得エラー。
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CatalogError {
    /// カタログアクセス失敗 (接続・クエリエラー)。
    #[error("catalog access failed: {message}")]
    AccessFailed {
        /// エラーメッセージ。
        message: String,
    },
    /// カタログ情報のパース失敗 (JSON 不正・型不整合)。
    #[error("catalog parse failed: {message}")]
    ParseFailed {
        /// エラーメッセージ。
        message: String,
    },
    /// 未対応の ASE 固有データ型・構造。
    #[error("unsupported catalog shape: {detail}")]
    UnsupportedCatalogShape {
        /// 詳細。
        detail: String,
    },
    /// 機能が未実装 (CTO condition #3: design.md が具体的カタログ問い合わせを
    /// 規定しない表面は T9 範囲外とし、明示的に表面化する)。
    ///
    /// T9 では `AseCatalogProvider::load_schema` のカタログイントロスペクション
    /// (sysobjects/syscolumns/sysindexes 読み出し) がこの状態になる
    /// (T9b follow-up で実装)。
    #[error("not implemented: {what}")]
    NotImplemented {
        /// 未実装の対象。
        what: String,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use common_sql::ast::{Identifier, Literal, SortDirection};

    // ---- helpers ----

    fn col(name: &str) -> CatalogColumn {
        CatalogColumn {
            name: name.to_string(),
            data_type: DataType::Int,
            nullable: true,
            default: None,
            raw_default: None,
            identity: false,
            constraints: vec![],
        }
    }

    fn idx_col(name: &str) -> IndexColumn {
        IndexColumn {
            name: Identifier::new(name.to_string()),
            direction: None,
        }
    }

    fn table(name: &str) -> CatalogTable {
        CatalogTable {
            name: name.to_string(),
            columns: vec![col("id")],
            constraints: vec![],
        }
    }

    // ===== CatalogSchema =====

    #[test]
    fn schema_default_is_empty() {
        let s = CatalogSchema::default();
        assert!(s.schema_name.is_empty());
        assert!(s.tables.is_empty());
        assert!(s.indices.is_empty());
    }

    #[test]
    fn schema_construct_with_all_fields() {
        let s = CatalogSchema {
            schema_name: "dbo".to_string(),
            tables: vec![table("users")],
            indices: vec![CatalogIndex {
                name: "idx_users_name".to_string(),
                table: "users".to_string(),
                columns: vec![idx_col("name")],
                unique: false,
            }],
        };
        assert_eq!(s.schema_name, "dbo");
        assert_eq!(s.tables.len(), 1);
        assert_eq!(s.indices.len(), 1);
    }

    #[test]
    fn schema_clone_is_equal() {
        let s = CatalogSchema {
            schema_name: "public".to_string(),
            tables: vec![table("t")],
            indices: vec![],
        };
        assert_eq!(s, s.clone());
    }

    #[test]
    fn schema_debug_contains_name() {
        let s = CatalogSchema {
            schema_name: "dbo".to_string(),
            tables: vec![],
            indices: vec![],
        };
        assert!(format!("{s:?}").contains("dbo"));
    }

    // ===== CatalogTable =====

    #[test]
    fn table_construct_and_access_fields() {
        let pk = TableConstraint::PrimaryKey {
            name: None,
            columns: vec![Identifier::new("id".to_string())],
        };
        let t = CatalogTable {
            name: "users".to_string(),
            columns: vec![col("id"), col("email")],
            constraints: vec![pk],
        };
        assert_eq!(t.name, "users");
        assert_eq!(t.columns.len(), 2);
        assert!(matches!(
            t.constraints[0],
            TableConstraint::PrimaryKey { .. }
        ));
    }

    #[test]
    fn table_name_is_schema_unqualified() {
        // design §3.2: table.name is a bare String.
        let t = CatalogTable {
            name: "orders".to_string(),
            columns: vec![],
            constraints: vec![],
        };
        assert_eq!(t.name, "orders");
        assert!(!t.name.contains('.'));
    }

    #[test]
    fn table_clone_is_equal() {
        let t = CatalogTable {
            name: "t".to_string(),
            columns: vec![col("c")],
            constraints: vec![],
        };
        assert_eq!(t, t.clone());
    }

    // ===== CatalogColumn =====

    #[test]
    fn column_default_shape_nullable_no_default() {
        let c = col("id");
        assert!(c.nullable);
        assert!(c.default.is_none());
        assert!(c.raw_default.is_none());
        assert!(!c.identity);
        assert!(c.constraints.is_empty());
    }

    #[test]
    fn column_with_default_expression_and_raw_default() {
        let c = CatalogColumn {
            name: "created_at".to_string(),
            data_type: DataType::DateTime { precision: None },
            nullable: false,
            default: Some(Expression::Literal(Literal::Integer(0))),
            raw_default: Some("getdate()".to_string()),
            identity: false,
            constraints: vec![],
        };
        assert!(c.default.is_some());
        assert_eq!(c.raw_default.as_deref(), Some("getdate()"));
        assert!(!c.nullable);
    }

    #[test]
    fn column_identity_flag_distinct_from_autoincrement_constraint() {
        // T6.6 feedback (b): identity bool and AutoIncrement constraint are
        // distinct here; mapper reconciles them bidirectionally.
        let identity_only = CatalogColumn {
            name: "id".to_string(),
            data_type: DataType::BigInt,
            nullable: false,
            default: None,
            raw_default: None,
            identity: true,
            constraints: vec![],
        };
        assert!(identity_only.identity);
        assert!(identity_only.constraints.is_empty());

        let constraint_only = CatalogColumn {
            name: "seq".to_string(),
            data_type: DataType::BigInt,
            nullable: false,
            default: None,
            raw_default: None,
            identity: false,
            constraints: vec![ColumnConstraint::AutoIncrement],
        };
        assert!(!constraint_only.identity);
        assert_eq!(constraint_only.constraints.len(), 1);
    }

    #[test]
    fn column_with_multiple_constraints() {
        let c = CatalogColumn {
            name: "code".to_string(),
            data_type: DataType::VarChar { length: Some(32) },
            nullable: false,
            default: None,
            raw_default: None,
            identity: false,
            constraints: vec![
                ColumnConstraint::Unique,
                ColumnConstraint::Check(Expression::Comparison {
                    left: Box::new(Expression::Identifier(Identifier::new("code".to_string()))),
                    op: common_sql::ast::ComparisonOperator::Ge,
                    right: Box::new(Expression::Literal(Literal::Integer(0))),
                }),
            ],
        };
        assert_eq!(c.constraints.len(), 2);
    }

    #[test]
    fn column_clone_is_equal() {
        let c = CatalogColumn {
            name: "x".to_string(),
            data_type: DataType::Int,
            nullable: false,
            default: None,
            raw_default: Some("0".to_string()),
            identity: true,
            constraints: vec![ColumnConstraint::PrimaryKey],
        };
        assert_eq!(c, c.clone());
    }

    // ===== CatalogIndex =====

    #[test]
    fn index_construct_non_unique() {
        let i = CatalogIndex {
            name: "idx_users_name".to_string(),
            table: "users".to_string(),
            columns: vec![idx_col("name")],
            unique: false,
        };
        assert_eq!(i.table, "users");
        assert_eq!(i.columns.len(), 1);
        assert!(!i.unique);
    }

    #[test]
    fn index_unique_multi_column_with_direction() {
        let i = CatalogIndex {
            name: "uk_users_last_first".to_string(),
            table: "users".to_string(),
            columns: vec![
                IndexColumn {
                    name: Identifier::new("last".to_string()),
                    direction: Some(SortDirection::Asc),
                },
                IndexColumn {
                    name: Identifier::new("first".to_string()),
                    direction: Some(SortDirection::Desc),
                },
            ],
            unique: true,
        };
        assert!(i.unique);
        assert_eq!(i.columns.len(), 2);
        assert_eq!(i.columns[1].direction, Some(SortDirection::Desc));
    }

    #[test]
    fn index_clone_is_equal() {
        let i = CatalogIndex {
            name: "i".to_string(),
            table: "t".to_string(),
            columns: vec![idx_col("c")],
            unique: true,
        };
        assert_eq!(i, i.clone());
    }

    // ===== CatalogProvider trait =====

    struct FakeProvider {
        schema: CatalogSchema,
    }

    impl CatalogProvider for FakeProvider {
        fn load_schema(&self) -> Result<CatalogSchema, CatalogError> {
            Ok(self.schema.clone())
        }
    }

    struct FailingProvider;

    impl CatalogProvider for FailingProvider {
        fn load_schema(&self) -> Result<CatalogSchema, CatalogError> {
            Err(CatalogError::AccessFailed {
                message: "connection refused".to_string(),
            })
        }
    }

    #[test]
    fn trait_is_dyn_compatible() {
        let provider: Box<dyn CatalogProvider> = Box::new(FakeProvider {
            schema: CatalogSchema::default(),
        });
        assert!(provider.load_schema().is_ok());
    }

    #[test]
    fn trait_load_schema_returns_snapshot() {
        let provider = FakeProvider {
            schema: CatalogSchema {
                schema_name: "dbo".to_string(),
                tables: vec![table("users")],
                indices: vec![],
            },
        };
        let loaded = provider.load_schema().unwrap();
        assert_eq!(loaded.schema_name, "dbo");
        assert_eq!(loaded.tables.len(), 1);
    }

    #[test]
    fn trait_load_schema_propagates_error() {
        let provider = FailingProvider;
        assert!(matches!(
            provider.load_schema().unwrap_err(),
            CatalogError::AccessFailed { .. }
        ));
    }

    // ===== CatalogError =====

    #[test]
    fn catalog_error_display_access_failed() {
        let err = CatalogError::AccessFailed {
            message: "connection refused".to_string(),
        };
        assert_eq!(err.to_string(), "catalog access failed: connection refused");
    }

    #[test]
    fn catalog_error_display_parse_failed() {
        let err = CatalogError::ParseFailed {
            message: "bad json".to_string(),
        };
        assert_eq!(err.to_string(), "catalog parse failed: bad json");
    }

    #[test]
    fn catalog_error_display_unsupported_shape() {
        let err = CatalogError::UnsupportedCatalogShape {
            detail: "image type".to_string(),
        };
        assert_eq!(err.to_string(), "unsupported catalog shape: image type");
    }

    #[test]
    fn catalog_error_is_std_error() {
        fn assert_error(_e: &dyn std::error::Error) {}
        let err = CatalogError::AccessFailed {
            message: "x".to_string(),
        };
        assert_error(&err);
    }

    #[test]
    fn catalog_error_clone_and_equality() {
        let a = CatalogError::ParseFailed {
            message: "m".to_string(),
        };
        assert_eq!(a, a.clone());
    }

    #[test]
    fn catalog_error_inequality_different_variants() {
        let a = CatalogError::AccessFailed {
            message: "m".to_string(),
        };
        let b = CatalogError::ParseFailed {
            message: "m".to_string(),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn catalog_error_display_distinguishes_variants() {
        let a = CatalogError::AccessFailed {
            message: "x".to_string(),
        }
        .to_string();
        let p = CatalogError::ParseFailed {
            message: "x".to_string(),
        }
        .to_string();
        let u = CatalogError::UnsupportedCatalogShape {
            detail: "x".to_string(),
        }
        .to_string();
        assert_ne!(a, p);
        assert_ne!(p, u);
        assert_ne!(a, u);
    }

    // ===== Integration: full schema =====

    #[test]
    fn full_schema_with_table_column_index_and_constraints() {
        let id_col = CatalogColumn {
            name: "id".to_string(),
            data_type: DataType::BigInt,
            nullable: false,
            default: None,
            raw_default: None,
            identity: true,
            constraints: vec![ColumnConstraint::PrimaryKey],
        };
        let users = CatalogTable {
            name: "users".to_string(),
            columns: vec![id_col],
            constraints: vec![TableConstraint::Unique {
                name: Some("uk_users_id".to_string()),
                columns: vec![Identifier::new("id".to_string())],
            }],
        };
        let schema = CatalogSchema {
            schema_name: "dbo".to_string(),
            tables: vec![users],
            indices: vec![CatalogIndex {
                name: "idx_users_id".to_string(),
                table: "users".to_string(),
                columns: vec![idx_col("id")],
                unique: true,
            }],
        };

        let provider = FakeProvider {
            schema: schema.clone(),
        };
        let loaded = provider.load_schema().unwrap();
        assert_eq!(loaded, schema);
        assert!(loaded.tables[0].columns[0].identity);
        assert!(loaded.indices[0].unique);
    }
}
