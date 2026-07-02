//! Common SQL AST - データ型ノード
//!
//! 方言非依存のSQLデータ型（DataType）ノードを定義する。

/// Common SQL データ型
///
/// 全てのSQL方言で共通する（または共通化可能な）データ型種別を表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonDataType {
    /// TINYINT (1バイト整数)
    TinyInt,
    /// SMALLINT (2バイト整数)
    SmallInt,
    /// INT / INTEGER (4バイト整数)
    Int,
    /// BIGINT (8バイト整数)
    BigInt,
    /// DECIMAL / NUMERIC (固定小数点数)
    Decimal {
        /// 精度（全体の桁数）
        precision: Option<u8>,
        /// スケール（小数点以下の桁数）
        scale: Option<u8>,
    },
    /// NUMERIC (DECIMALの別名)
    Numeric {
        /// 精度（全体の桁数）
        precision: Option<u8>,
        /// スケール（小数点以下の桁数）
        scale: Option<u8>,
    },
    /// REAL (単精度浮動小数点数 - MySQLではDOUBLE)
    Real,
    /// DOUBLE PRECISION (倍精度浮動小数点数)
    DoublePrecision,
    /// FLOAT (浮動小数点数)
    Float {
        /// 精度
        precision: Option<u8>,
    },
    /// CHAR (固定長文字列)
    Char {
        /// 文字列長
        length: Option<u64>,
    },
    /// VARCHAR (可変長文字列)
    VarChar {
        /// 最大文字列長
        length: Option<u64>,
    },
    /// TEXT (長いテキスト)
    Text,
    /// NCHAR (固定長Unicode文字列 - MySQLではCHARとして扱う)
    NChar {
        /// 文字列長
        length: Option<u64>,
    },
    /// NVARCHAR (可変長Unicode文字列 - MySQLではVARCHARとして扱う)
    NVarChar {
        /// 最大文字列長
        length: Option<u64>,
    },
    /// DATE (日付型)
    Date,
    /// TIME (時刻型)
    Time {
        /// 小数秒精度
        precision: Option<u8>,
    },
    /// DATETIME (日時型)
    DateTime {
        /// 小数秒精度
        precision: Option<u8>,
    },
    /// TIMESTAMP (タイムスタンプ型)
    Timestamp {
        /// 小数秒精度
        precision: Option<u8>,
    },
    /// BINARY (固定長バイナリ)
    Binary {
        /// バイト長
        length: Option<u64>,
    },
    /// VARBINARY (可変長バイナリ)
    VarBinary {
        /// 最大バイト長
        length: Option<u64>,
    },
    /// BLOB (バイナリラージオブジェクト)
    Blob,
    /// BOOLEAN / BOOL (真理値 - MySQLではTINYINT(1))
    Boolean,
    /// UUID / UNIQUEIDENTIFIER (MySQLではCHAR(36))
    Uuid,
    /// JSON (JSON型 - MySQL 5.7.8+)
    Json,
}
