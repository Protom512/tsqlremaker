//! 構文のコンバーター
//!
//! T-SQL 固有の構文を MySQL 構文に変換します。

use crate::MySqlEmitter;
use tsql_parser::common::{CommonExpression, CommonIdentifier};

/// 構文コンバーター
///
/// T-SQL 固有の構文を MySQL 構文に変換します。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxConverter;

impl SyntaxConverter {
    /// TOP n を LIMIT n に変換
    ///
    /// # Arguments
    ///
    /// * `limit` - LIMIT 値を表す式
    /// * `emitter` - Emitter（式を文字列化するために使用）
    ///
    /// # Returns
    ///
    /// LIMIT 句文字列
    pub fn convert_top_to_limit(
        limit: &CommonExpression,
        emitter: &mut MySqlEmitter,
    ) -> Result<String, crate::EmitError> {
        let limit_str = emitter.visit_expression(limit)?;
        Ok(format!("LIMIT {}", limit_str))
    }

    /// SELECT @var = expr を SET @var = (SELECT expr) に変換
    ///
    /// # Arguments
    ///
    /// * `variable` - 変数名
    /// * `expr` - 代入する式
    /// * `emitter` - Emitter
    ///
    /// # Returns
    ///
    /// SET 文文字列
    pub fn convert_variable_assignment(
        variable: &CommonIdentifier,
        expr: &CommonExpression,
        emitter: &mut MySqlEmitter,
    ) -> Result<String, crate::EmitError> {
        let var_str = &variable.name;
        let expr_str = emitter.visit_expression(expr)?;
        Ok(format!("SET @{} = ({})", var_str, expr_str))
    }

    /// 一時テーブル名を変換
    ///
    /// # Arguments
    ///
    /// * `name` - テーブル名
    ///
    /// # Returns
    ///
    /// (変換後の名前, グローバル一時テーブルかどうか)
    ///
    /// ## 変換ルール
    ///
    /// - `#temp_table` → `temp_table` (ローカル一時テーブル)
    /// - `##global_temp` → `global_temp` (グローバル一時テーブル)
    /// - `regular_table` → `regular_table` (通常テーブル、変更なし)
    #[must_use]
    pub fn convert_temp_table(name: &str) -> (String, bool) {
        if let Some(stripped) = name.strip_prefix('#') {
            // 先頭の # を削除
            if let Some(global_name) = stripped.strip_prefix('#') {
                // ##global_temp → global_temp (グローバル一時テーブル)
                (global_name.to_string(), true)
            } else {
                // #temp_table → temp_table (ローカル一時テーブル)
                (stripped.to_string(), false)
            }
        } else {
            // 通常テーブル
            (name.to_string(), false)
        }
    }

    /// 一時テーブルかどうかを判定
    ///
    /// # Arguments
    ///
    /// * `name` - テーブル名
    ///
    /// # Returns
    ///
    /// 一時テーブルの場合は true
    #[must_use]
    pub fn is_temp_table(name: &str) -> bool {
        name.starts_with('#')
    }

    /// グローバル一時テーブルかどうかを判定
    ///
    /// # Arguments
    ///
    /// * `name` - テーブル名
    ///
    /// # Returns
    ///
    /// グローバル一時テーブルの場合は true
    #[must_use]
    pub fn is_global_temp_table(name: &str) -> bool {
        name.starts_with("##")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsql_parser::common::{CommonExpression, CommonIdentifier, CommonLiteral};

    fn create_literal_int(n: i64) -> CommonExpression {
        CommonExpression::Literal(CommonLiteral::Integer(n))
    }

    #[test]
    fn test_convert_temp_table_local() {
        let (converted, is_global) = SyntaxConverter::convert_temp_table("#temp_users");
        assert_eq!(converted, "temp_users");
        assert!(!is_global);
    }

    #[test]
    fn test_convert_temp_table_global() {
        let (converted, is_global) = SyntaxConverter::convert_temp_table("##global_temp");
        assert_eq!(converted, "global_temp");
        assert!(is_global);
    }

    #[test]
    fn test_convert_temp_table_regular() {
        let (converted, is_global) = SyntaxConverter::convert_temp_table("users");
        assert_eq!(converted, "users");
        assert!(!is_global);
    }

    #[test]
    fn test_convert_temp_table_empty() {
        let (converted, is_global) = SyntaxConverter::convert_temp_table("#");
        assert_eq!(converted, "");
        assert!(!is_global);
    }

    #[test]
    fn test_is_temp_table() {
        assert!(SyntaxConverter::is_temp_table("#temp"));
        assert!(SyntaxConverter::is_temp_table("##global"));
        assert!(!SyntaxConverter::is_temp_table("regular"));
    }

    #[test]
    fn test_is_global_temp_table() {
        assert!(SyntaxConverter::is_global_temp_table("##global"));
        assert!(!SyntaxConverter::is_global_temp_table("#local"));
        assert!(!SyntaxConverter::is_global_temp_table("regular"));
    }

    #[test]
    fn test_convert_top_to_limit() {
        let limit = create_literal_int(10);
        let mut emitter = crate::MySqlEmitter::default();
        let result = SyntaxConverter::convert_top_to_limit(&limit, &mut emitter);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "LIMIT 10");
    }
}
