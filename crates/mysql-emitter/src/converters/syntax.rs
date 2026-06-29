//! 構文コンバーター
//!
//! T-SQL 固有の構文を MySQL 構文に変換します (design Req 4 / 11)。
//!
//! - `TOP n` → `LIMIT n` のためのリミット値文字列化
//!   (実際の LIMIT 句組み立ては [`crate::MySqlEmitter`] が `SelectStatement.limit`
//!   から直接行うため、本関数は式の文字列化ヘルパーとして残存)
//! - `SELECT @var = expr` → `SET @var = (SELECT expr)` の変数代入変換
//! - 一時テーブル名の変換（`#temp` → `temp`, `##global` → `global`）
//!
//! 共通 AST ([`common_sql::ast`]) のみを扱い、レガシー `tsql_parser` 型には依存しない。

use common_sql::ast::{Expression, Identifier};

use crate::{EmitError, MySqlEmitter};

/// 構文コンバーター
///
/// T-SQL 固有の構文要素を MySQL の対応する構文に変換します。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxConverter;

impl SyntaxConverter {
    /// `TOP n` のリミット値式を MySQL 文字列にシリアライズします。
    ///
    /// 設計契約（design.md）に基づき、[`MySqlEmitter`] を用いて式を描画します。
    /// `LIMIT` 句の組み立て自体は [`MySqlEmitter`] が `SelectStatement.limit` から
    /// 行うため、本関数は個別の式文字列化が必要な場面 (例: ユニットテストや
    /// 派生エミッタ) 向けのヘルパーです。
    ///
    /// # Arguments
    ///
    /// * `limit` - リミット値を表す共通 SQL 式
    /// * `emitter` - 式の文字列化に使用する Emitter
    ///
    /// # Errors
    ///
    /// 式内にサポート対象外ノードがあれば [`EmitError`] を返します。
    pub fn convert_top_to_limit(
        limit: &Expression,
        emitter: &mut MySqlEmitter,
    ) -> Result<String, EmitError> {
        emitter.emit_expression(limit)
    }

    /// `SELECT @var = expr` を `SET @var = (SELECT expr)` に変換します。
    ///
    /// T-SQL の SELECT 内変数代入（`SELECT @count = COUNT(*) FROM ...`）は
    /// MySQL では直接表現できないため、相関スカラサブクエリ形式の
    /// SET 文に変換します。
    ///
    /// # Arguments
    ///
    /// * `variable` - 代入先の変数識別子（例: `@count`）
    /// * `expr` - 代入する式の MySQL SQL 文字列表現
    ///
    /// # Returns
    ///
    /// 生成された `SET @var = (SELECT expr)` 文字列。
    #[must_use]
    pub fn convert_variable_assignment(variable: &Identifier, expr: &str) -> String {
        format!("SET @{} = (SELECT {})", variable.value(), expr)
    }

    /// 一時テーブル名を MySQL 形式に変換します。
    ///
    /// T-SQL の一時テーブルプレフィクスを除去し、グローバル一時テーブル
    /// かどうかのフラグを返します。
    ///
    /// - `#temp` → `temp`（ローカル一時テーブル、フラグ false）
    /// - `##global` → `global`（グローバル一時テーブル、フラグ true）
    /// - プレフィクスなし → そのまま（フラグ false）
    ///
    /// # Arguments
    ///
    /// * `name` - T-SQL の一時テーブル名（`#...` または `##...` を含む）
    ///
    /// # Returns
    ///
    /// `(converted_name, is_global_temp)` のタプル。
    /// `is_global_temp` は `##` プレフィクス（グローバル一時テーブル）の
    /// 場合に `true` となり、呼び出し元で警告コメントの生成に使用されます。
    #[must_use]
    pub fn convert_temp_table(name: &str) -> (String, bool) {
        if let Some(rest) = name.strip_prefix("##") {
            (rest.to_string(), true)
        } else if let Some(rest) = name.strip_prefix('#') {
            (rest.to_string(), false)
        } else {
            (name.to_string(), false)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::EmitterConfig;
    use common_sql::ast::identifier::Identifier;
    use common_sql::ast::literal::Literal;

    fn int_expr(n: i64) -> Expression {
        Expression::Literal(Literal::Integer(n))
    }

    fn id_expr(name: &str) -> Expression {
        Expression::Identifier(Identifier::new(name.to_string()))
    }

    fn emitter() -> MySqlEmitter {
        MySqlEmitter::new(EmitterConfig::default())
    }

    // ============================================================
    // convert_top_to_limit
    // ============================================================

    #[test]
    fn convert_top_to_limit_integer_literal() {
        let mut em = emitter();
        let result = SyntaxConverter::convert_top_to_limit(&int_expr(10), &mut em).unwrap();
        assert_eq!(result, "10");
    }

    #[test]
    fn convert_top_to_limit_large_number() {
        let mut em = emitter();
        let result = SyntaxConverter::convert_top_to_limit(&int_expr(1_000_000), &mut em).unwrap();
        assert_eq!(result, "1000000");
    }

    #[test]
    fn convert_top_to_limit_zero() {
        let mut em = emitter();
        let result = SyntaxConverter::convert_top_to_limit(&int_expr(0), &mut em).unwrap();
        assert_eq!(result, "0");
    }

    #[test]
    fn convert_top_to_limit_identifier() {
        let mut em = emitter();
        let result = SyntaxConverter::convert_top_to_limit(&id_expr("n"), &mut em).unwrap();
        assert_eq!(result, "`n`");
    }

    // ============================================================
    // convert_variable_assignment
    // ============================================================

    #[test]
    fn variable_assignment_basic() {
        let var = Identifier::new("count".to_string());
        let result = SyntaxConverter::convert_variable_assignment(&var, "COUNT(*)");
        assert_eq!(result, "SET @count = (SELECT COUNT(*))");
    }

    #[test]
    fn variable_assignment_preserves_expr() {
        let var = Identifier::new("total".to_string());
        let result = SyntaxConverter::convert_variable_assignment(&var, "price * qty");
        assert_eq!(result, "SET @total = (SELECT price * qty)");
    }

    // ============================================================
    // convert_temp_table
    // ============================================================

    #[test]
    fn temp_table_local_prefix() {
        let (name, is_global) = SyntaxConverter::convert_temp_table("#orders");
        assert_eq!(name, "orders");
        assert!(!is_global);
    }

    #[test]
    fn temp_table_global_prefix() {
        let (name, is_global) = SyntaxConverter::convert_temp_table("##global_tmp");
        assert_eq!(name, "global_tmp");
        assert!(is_global);
    }

    #[test]
    fn temp_table_no_prefix() {
        let (name, is_global) = SyntaxConverter::convert_temp_table("normal_table");
        assert_eq!(name, "normal_table");
        assert!(!is_global);
    }

    #[test]
    fn temp_table_single_hash_after_double() {
        // "##a" → double-hash wins (global), not single-hash.
        let (name, is_global) = SyntaxConverter::convert_temp_table("##a");
        assert_eq!(name, "a");
        assert!(is_global);
    }
}
