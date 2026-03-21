//! PostgreSQL Emitter のコンフィグレーション

/// PostgreSQL Emitter の設定
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionConfig {
    /// キーワードを大文字で出力するかどうか
    pub uppercase_keywords: bool,
    /// 識別子をダブルクォートで囲むかどうか
    pub quote_identifiers: bool,
    /// インデントサイズ（スペース数）
    pub indent_size: usize,
    /// サポートされていない機能に対して警告コメントを出力するかどうか
    pub warn_unsupported: bool,
}

impl Default for EmissionConfig {
    fn default() -> Self {
        Self {
            uppercase_keywords: false,
            quote_identifiers: true,
            indent_size: 4,
            warn_unsupported: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EmissionConfig::default();
        assert!(!config.uppercase_keywords);
        assert!(config.quote_identifiers);
        assert_eq!(config.indent_size, 4);
    }

    #[test]
    fn test_custom_config() {
        let config = EmissionConfig {
            uppercase_keywords: true,
            quote_identifiers: false,
            indent_size: 2,
            warn_unsupported: false,
        };
        assert!(config.uppercase_keywords);
        assert!(!config.quote_identifiers);
        assert_eq!(config.indent_size, 2);
    }

    #[test]
    fn test_config_equality() {
        let config1 = EmissionConfig::default();
        let config2 = EmissionConfig::default();
        assert_eq!(config1, config2);
    }

    #[test]
    fn test_config_inequality() {
        let config1 = EmissionConfig::default();
        let config2 = EmissionConfig {
            uppercase_keywords: true,
            quote_identifiers: config1.quote_identifiers,
            indent_size: config1.indent_size,
            warn_unsupported: config1.warn_unsupported,
        };
        assert_ne!(config1, config2);
    }

    #[test]
    fn test_warn_unsupported_default() {
        let config = EmissionConfig::default();
        assert!(config.warn_unsupported);
    }
}
