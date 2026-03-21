//! SQLite Emitter のコンフィグレーション

/// SQLite Emitter の設定
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmitterConfig {
    /// キーワードを大文字で出力するかどうか
    pub uppercase_keywords: bool,
    /// 識別子をダブルクォートで囲むかどうか
    pub quote_identifiers: bool,
    /// インデントサイズ（スペース数）
    pub indent_size: usize,
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self {
            uppercase_keywords: false,
            quote_identifiers: true,
            indent_size: 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EmitterConfig::default();
        assert!(!config.uppercase_keywords);
        assert!(config.quote_identifiers);
        assert_eq!(config.indent_size, 4);
    }

    #[test]
    fn test_custom_config() {
        let config = EmitterConfig {
            uppercase_keywords: true,
            quote_identifiers: false,
            indent_size: 2,
        };
        assert!(config.uppercase_keywords);
        assert!(!config.quote_identifiers);
        assert_eq!(config.indent_size, 2);
    }

    #[test]
    fn test_config_equality() {
        let config1 = EmitterConfig::default();
        let config2 = EmitterConfig::default();
        assert_eq!(config1, config2);
    }

    #[test]
    fn test_config_inequality() {
        let config1 = EmitterConfig::default();
        let config2 = EmitterConfig {
            uppercase_keywords: true,
            ..config1
        };
        assert_ne!(config1, config2);
    }
}
