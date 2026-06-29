//! Emitter の設定

/// Emitter の設定
///
/// SQL 生成時のフォーマット動作を制御します。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmitterConfig {
    /// 整形するかどうか
    ///
    /// `true` の場合、改行とインデントを挿入して SQL を整形します。
    pub format: bool,
    /// インデントサイズ
    ///
    /// 各インデントレベルのスペース数。
    pub indent_size: usize,
}

impl EmitterConfig {
    /// 新しい設定を作成
    ///
    /// # Arguments
    ///
    /// * `format` - 整形するかどうか
    /// * `indent_size` - インデントサイズ
    #[must_use]
    pub const fn new(format: bool, indent_size: usize) -> Self {
        Self {
            format,
            indent_size,
        }
    }

    /// 整形なしの設定を作成
    #[must_use]
    pub const fn compact() -> Self {
        Self {
            format: false,
            indent_size: 0,
        }
    }

    /// デフォルト整形設定を作成
    #[must_use]
    pub const fn formatted() -> Self {
        Self {
            format: true,
            indent_size: 4,
        }
    }
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self::formatted()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_new_config() {
        let config = EmitterConfig::new(true, 2);
        assert!(config.format);
        assert_eq!(config.indent_size, 2);
    }

    #[test]
    fn test_compact_config() {
        let config = EmitterConfig::compact();
        assert!(!config.format);
        assert_eq!(config.indent_size, 0);
    }

    #[test]
    fn test_formatted_config() {
        let config = EmitterConfig::formatted();
        assert!(config.format);
        assert_eq!(config.indent_size, 4);
    }

    #[test]
    fn test_default_config() {
        let config = EmitterConfig::default();
        assert!(config.format);
        assert_eq!(config.indent_size, 4);
    }

    // --- Task 1.3 design contract conformance (field names + Default values) ---

    /// design.md 158-163: fields are exactly `format: bool` and `indent_size: usize`.
    /// Constructing via struct literal with these exact names must compile and work.
    #[test]
    fn test_struct_literal_field_names_match_design() {
        let config = EmitterConfig {
            format: false,
            indent_size: 8,
        };
        assert!(!config.format);
        assert_eq!(config.indent_size, 8);
    }

    /// Group0 acceptance: Default must be format:true / indent_size:4.
    #[test]
    fn test_default_matches_group0_acceptance() {
        let config = EmitterConfig::default();
        assert!(config.format);
        assert_eq!(config.indent_size, 4);
    }

    /// Default delegates to formatted() (single source of truth).
    #[test]
    fn test_default_equals_formatted() {
        assert_eq!(EmitterConfig::default(), EmitterConfig::formatted());
    }

    /// compact() disables formatting and zeroes indent.
    #[test]
    fn test_compact_disables_format_and_indent() {
        let config = EmitterConfig::compact();
        assert!(!config.format);
        assert_eq!(config.indent_size, 0);
    }

    /// new() is a const constructor storing both fields verbatim.
    #[test]
    fn test_new_preserves_both_fields() {
        let config = EmitterConfig::new(true, 2);
        assert!(config.format);
        assert_eq!(config.indent_size, 2);

        let config = EmitterConfig::new(false, 16);
        assert!(!config.format);
        assert_eq!(config.indent_size, 16);
    }
}
