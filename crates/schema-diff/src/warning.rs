//! `MigrationWarning` (design §2.6).
//!
//! Warnings surfaced during diff derivation or migration generation. These are
//! advisory: policy A (continue, do not halt) is applied for destructive
//! changes, so callers decide whether to proceed.

/// 差分導出・マイグレーション生成過程の警告。
///
/// All fields are owned `String` (no references) so the value can be surfaced
/// to CLI/stderr without lifetime entanglement.
#[derive(Debug, Clone, PartialEq)]
pub enum MigrationWarning {
    /// 破壊的変更 (方針 A: 継続、停止しない)。
    Destructive {
        /// 人間可読な位置情報 ("table.column" 形式)。
        target: String,
        /// 変更内容の説明。
        detail: String,
    },
    /// 対象方言がネイティブ非サポート (SQLite の ALTER COLUMN 等)。
    UnsupportedDialect {
        /// 方言名 ("sqlite" / "postgresql" / "mysql")。
        dialect: String,
        /// 非対応操作の説明。
        operation: String,
    },
}

impl MigrationWarning {
    /// Constructs a `Destructive` warning.
    #[must_use]
    pub fn destructive(target: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Destructive {
            target: target.into(),
            detail: detail.into(),
        }
    }

    /// Constructs an `UnsupportedDialect` warning.
    #[must_use]
    pub fn unsupported_dialect(dialect: impl Into<String>, operation: impl Into<String>) -> Self {
        Self::UnsupportedDialect {
            dialect: dialect.into(),
            operation: operation.into(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn destructive_warning_carries_target_and_detail() {
        let w = MigrationWarning::destructive("users.email", "VARCHAR(255) -> VARCHAR(100)");
        match &w {
            MigrationWarning::Destructive { target, detail } => {
                assert_eq!(target, "users.email");
                assert_eq!(detail, "VARCHAR(255) -> VARCHAR(100)");
            }
            other => panic!("expected Destructive, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_dialect_warning_carries_dialect_and_operation() {
        let w = MigrationWarning::unsupported_dialect("sqlite", "ALTER COLUMN type change");
        match &w {
            MigrationWarning::UnsupportedDialect { dialect, operation } => {
                assert_eq!(dialect, "sqlite");
                assert_eq!(operation, "ALTER COLUMN type change");
            }
            other => panic!("expected UnsupportedDialect, got {other:?}"),
        }
    }

    #[test]
    fn warning_clone_equality() {
        let w = MigrationWarning::destructive("t.c", "narrowing");
        assert_eq!(w, w.clone());
    }

    #[test]
    fn destructive_and_unsupported_are_distinct() {
        let d = MigrationWarning::destructive("a", "b");
        let u = MigrationWarning::unsupported_dialect("sqlite", "op");
        assert_ne!(d, u);
    }
}
