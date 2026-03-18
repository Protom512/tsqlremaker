//! # PostgreSQL Emitter
//!
//! PostgreSQL 方言の SQL を生成する Emitter ライブラリ。
//!
//! ## 概要
//!
//! このライブラリは、Common SQL AST を入力として受け取り、
//! PostgreSQL 方言の SQL 文字列を出力します。
//!
//! ## 使用例
//!
//! 現在は式、関数、識別子のマッパーが使用可能です：
//!
//! ```rust
//! use postgresql_emitter::{ExpressionEmitter, FunctionMapper};
//! use tsql_parser::common::{CommonExpression, CommonLiteral};
//!
//! // 式をPostgreSQL SQLに変換
//! let expr = CommonExpression::Literal(CommonLiteral::Integer(42));
//! let sql = ExpressionEmitter::emit(&expr);
//! assert_eq!(sql, "42");
//!
//! // 関数名のマッピング
//! let func_name = FunctionMapper::map_function_name("GETDATE");
//! assert_eq!(func_name, Some("CURRENT_TIMESTAMP".to_string()));
//! ```
//!
//! ## 機能
//!
//! - Common SQL AST からの PostgreSQL SQL 生成
//! - データ型の変換
//! - 関数の変換
//! - T-SQL 固有構文の変換

#![warn(missing_docs)]
// workspace.lints から clippy 設定を継承
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

mod config;
mod error;
pub mod mappers;

pub use config::EmissionConfig;
pub use error::EmitError;

// よく使うマッパーを再エクスポート
pub use mappers::ExpressionEmitter;
pub use mappers::FunctionMapper;

/// PostgreSQL Emitter
///
/// Common SQL AST を PostgreSQL SQL に変換します。
#[derive(Debug)]
pub struct PostgreSqlEmitter {
    /// 出力バッファ
    buffer: String,
    /// インデントレベル
    indent_level: usize,
    /// コンフィグ
    config: EmissionConfig,
}

impl PostgreSqlEmitter {
    /// 新しい Emitter を作成
    ///
    /// # Arguments
    ///
    /// * `config` - Emitter の設定
    #[must_use]
    pub const fn new(config: EmissionConfig) -> Self {
        Self {
            buffer: String::new(),
            indent_level: 0,
            config,
        }
    }

    /// コンフィグを取得
    #[must_use]
    pub const fn config(&self) -> &EmissionConfig {
        &self.config
    }

    /// バッファをクリア
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.indent_level = 0;
    }

    /// 現在のインデントを取得
    fn current_indent(&self) -> String {
        " ".repeat(self.indent_level * self.config.indent_size)
    }

    /// バッファに文字列を追加
    fn write(&mut self, s: &str) {
        self.buffer.push_str(s);
    }

    /// 改行を追加
    fn writeln(&mut self) {
        self.buffer.push('\n');
    }

    /// インデントを追加
    fn write_indent(&mut self) {
        if self.config.quote_identifiers {
            let indent = self.current_indent();
            self.buffer.push_str(&indent);
        }
    }

    /// インデントを増やす
    fn inc_indent(&mut self) {
        self.indent_level += 1;
    }

    /// インデントを減らす
    fn dec_indent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }
}

impl Default for PostgreSqlEmitter {
    fn default() -> Self {
        Self::new(EmissionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_emitter() {
        let config = EmissionConfig::default();
        let emitter = PostgreSqlEmitter::new(config);
        assert_eq!(emitter.indent_level, 0);
        assert!(emitter.buffer.is_empty());
    }

    #[test]
    fn test_default_emitter() {
        let emitter = PostgreSqlEmitter::default();
        assert_eq!(emitter.config().quote_identifiers, true);
        assert_eq!(emitter.config().indent_size, 4);
    }

    #[test]
    fn test_reset() {
        let mut emitter = PostgreSqlEmitter::default();
        emitter.write("SELECT 1");
        emitter.writeln();
        assert!(!emitter.buffer.is_empty());

        emitter.reset();
        assert!(emitter.buffer.is_empty());
        assert_eq!(emitter.indent_level, 0);
    }

    #[test]
    fn test_current_indent() {
        let mut emitter = PostgreSqlEmitter::new(EmissionConfig {
            quote_identifiers: true,
            uppercase_keywords: false,
            indent_size: 2,
        });

        assert_eq!(emitter.current_indent(), "");

        emitter.indent_level = 1;
        assert_eq!(emitter.current_indent(), "  ");

        emitter.indent_level = 2;
        assert_eq!(emitter.current_indent(), "    ");
    }
}
