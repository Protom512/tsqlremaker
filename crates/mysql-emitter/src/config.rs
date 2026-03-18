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
mod tests {
    use super::*;

    #[test]
    fn test_new_config() {
        let config = EmitterConfig::new(true, 2);
        assert_eq!(config.format, true);
        assert_eq!(config.indent_size, 2);
    }

    #[test]
    fn test_compact_config() {
        let config = EmitterConfig::compact();
        assert_eq!(config.format, false);
        assert_eq!(config.indent_size, 0);
    }

    #[test]
    fn test_formatted_config() {
        let config = EmitterConfig::formatted();
        assert_eq!(config.format, true);
        assert_eq!(config.indent_size, 4);
    }

    #[test]
    fn test_default_config() {
        let config = EmitterConfig::default();
        assert_eq!(config.format, true);
        assert_eq!(config.indent_size, 4);
    }
}
