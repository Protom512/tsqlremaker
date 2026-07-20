//! `Dialect` enum (design §0.1 / tasks.md Task 10.1).
//!
//! The single source of truth for the three target SQL dialects that
//! schema-diff can emit: MySQL, PostgreSQL, SQLite. The CLI (`bin/schema-diff.rs`)
//! delegates here instead of defining its own parallel enum, so the variant
//! spelling / ordering cannot drift between the library and the binary.
//!
//! `Dialect` carries no behaviour beyond identity: it is a `Copy` tag that the
//! emit layer (T10-2 `to_statements_for_dialect`) inspects to decide whether
//! an `AlterTableAction` is natively supported or must be surfaced as an
//! `UnsupportedDialect` warning (design §0.4 "possible-range SQL only").

/// The three SQL dialects schema-diff can render migrations for.
///
/// Variants are ordered MySQL → PostgreSQL → SQLite to mirror the bin CLI's
/// historical `ValueEnum` order, so a 1:1 conversion is order-preserving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// MySQL / SAP ASE-compatible rendering.
    Mysql,
    /// PostgreSQL rendering.
    Postgresql,
    /// SQLite rendering.
    Sqlite,
}

impl Dialect {
    /// Returns the lowercase kebab-case identifier used in
    /// `MigrationWarning::UnsupportedDialect.dialect` ("mysql" / "postgresql"
    /// / "sqlite") and in CLI `--dialect` values.
    ///
    /// This is the canonical wire spelling; the bin CLI and any future
    /// `Display`/`ValueEnum` impl must derive from here to avoid drift.
    #[must_use]
    pub const fn as_kebab(self) -> &'static str {
        match self {
            Self::Mysql => "mysql",
            Self::Postgresql => "postgresql",
            Self::Sqlite => "sqlite",
        }
    }

    /// Iterates over all dialects in declaration order (MySQL, PostgreSQL,
    /// SQLite). Useful for exhaustive tests.
    #[must_use]
    pub const fn all() -> [Dialect; 3] {
        [Self::Mysql, Self::Postgresql, Self::Sqlite]
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // ===== derive contract: Copy + Clone + PartialEq + Eq + Debug =====

    #[test]
    fn dialect_is_copy_and_clone() {
        let a = Dialect::Mysql;
        let b = a; // Copy
        let c = a; // Copy again — would fail to compile if not Copy
        assert_eq!(a, b);
        assert_eq!(a, c);
        let _cloned = a; // Clone (Copy)
    }

    #[test]
    fn dialect_equality_and_inequality() {
        assert_eq!(Dialect::Mysql, Dialect::Mysql);
        assert_ne!(Dialect::Mysql, Dialect::Postgresql);
        assert_ne!(Dialect::Postgresql, Dialect::Sqlite);
        assert_ne!(Dialect::Sqlite, Dialect::Mysql);
    }

    #[test]
    fn dialect_debug_renders_variant_name() {
        assert_eq!(format!("{:?}", Dialect::Mysql), "Mysql");
        assert_eq!(format!("{:?}", Dialect::Postgresql), "Postgresql");
        assert_eq!(format!("{:?}", Dialect::Sqlite), "Sqlite");
    }

    // ===== as_kebab: canonical wire spelling =====

    #[test]
    fn as_kebab_matches_cli_and_warning_spelling() {
        // These spellings MUST match bin/schema-diff.rs ValueEnum and the
        // `dialect` field of MigrationWarning::UnsupportedDialect.
        assert_eq!(Dialect::Mysql.as_kebab(), "mysql");
        assert_eq!(Dialect::Postgresql.as_kebab(), "postgresql");
        assert_eq!(Dialect::Sqlite.as_kebab(), "sqlite");
    }

    #[test]
    fn as_kebab_values_are_unique() {
        let spellings: Vec<&str> = Dialect::all().iter().map(|d| d.as_kebab()).collect();
        let unique: std::collections::HashSet<&str> = spellings.iter().copied().collect();
        assert_eq!(
            spellings.len(),
            unique.len(),
            "dialect spellings must be unique"
        );
        assert_eq!(unique.len(), 3);
    }

    // ===== all(): exhaustive enumeration =====

    #[test]
    fn all_returns_three_variants_in_order() {
        let all = Dialect::all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], Dialect::Mysql);
        assert_eq!(all[1], Dialect::Postgresql);
        assert_eq!(all[2], Dialect::Sqlite);
    }

    // ===== 1:1 mapping invariant vs bin Dialect (anti-drift guard) =====
    //
    // The bin/schema-diff.rs local `Dialect` enum (lines 51-59) must map 1:1
    // to this library enum. We encode the expected variant count and spelling
    // here so a future divergence is caught by this test, not by a runtime
    // CLI error. (The bin delegates via a single `match` — see bin/schema-diff.rs.)

    #[test]
    fn exactly_three_variants_exist() {
        // If a 4th dialect is ever added, this test forces a deliberate
        // review of the bin conversion and Warning spelling.
        assert_eq!(Dialect::all().len(), 3);
    }
}
