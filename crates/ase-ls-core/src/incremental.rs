//! インクリメンタルドキュメント同期のためのレンジパッチ適用。
//!
//! LSP の `TextDocumentSyncKind::Incremental` において、
//! `TextDocumentContentChangeEvent` をソース文字列に適用する純粋関数を提供する。
//!
//! ## 設計
//!
//! - 入力: 現在のソース全体 + 1件の `TextDocumentContentChangeEvent`
//! - 出力: 適用後の新しいソース文字列
//! - byte-offset <-> line/character 変換は [`LineIndex`](crate::line_index::LineIndex) に委譲
//! - 文字列スライスは常に `str::get(start..end)` により境界安全。
//!   Rust の `&str[i..j]` インデックススライスは非文字境界でパニックするため、
//!   ライブラリコードでは使用しない（本クレートの lint ルール）。

use lsp_types::{Position, TextDocumentContentChangeEvent};

use crate::line_index::LineIndex;

/// 1件のコンテンツ変更イベントをソースに適用し、新しいソースを返す。
///
/// # 引数
///
/// - `source`: 変更前のドキュメント全文
/// - `index`: `source` から構築した [`LineIndex`]（byte-offset <-> position 変換用）
/// - `change`: 適用するコンテンツ変更イベント
///
/// # 挙動
///
/// - `change.range == None` の場合はフル置換（`change.text` をそのまま返す）
/// - `change.range == Some(r)` の場合は、`r` を byte-offset に変換し、
///   該当範囲を `change.text` で置換する
///
/// # 安全性
///
/// 範囲指定の byte-offset が文字境界に一致しない可能性があるため、
/// 境界クリップと `str::get(start..end)` による安全なスライスを行う。
/// `start > end` の場合は `start` を `end` にクランプする。
/// パニックしない（インデックススライス不使用）。
#[must_use]
pub fn apply_content_change(
    source: &str,
    index: &LineIndex,
    change: &TextDocumentContentChangeEvent,
) -> String {
    // range=None はフル置換（LSP 仕様）
    let Some(range) = change.range.as_ref() else {
        return change.text.clone();
    };

    let start_offset = position_to_byte_offset(source, index, range.start);
    let end_offset = position_to_byte_offset(source, index, range.end);

    // start > end の場合は純挿入として扱うため start を end にクランプ
    let (start, end) = if start_offset > end_offset {
        (end_offset, end_offset)
    } else {
        (start_offset, end_offset)
    };

    // 境界安全なスライス: 非 UTF-8 文字境界なら直前の文字境界まで詰める
    let prefix = source.get(..start).unwrap_or("");
    let suffix = source.get(end..).unwrap_or("");

    let mut result = String::with_capacity(prefix.len() + change.text.len() + suffix.len());
    result.push_str(prefix);
    result.push_str(&change.text);
    result.push_str(suffix);
    result
}

/// LSP Position を byte-offset に変換し、必ず有効な UTF-8 文字境界にスナップする。
///
/// `LineIndex::position_to_offset` はバイトオフセットを返すが、
/// LSP の `Position` は行頭からの文字数ベースであり、マルチバイト文字の途中を指す場合がある。
/// 返り値を `source` 上の有効な UTF-8 文字境界に揃えることで、
/// 後続の `str::get(..)` が必ず成功するようにする。
fn position_to_byte_offset(source: &str, index: &LineIndex, position: Position) -> usize {
    let mut offset = index.position_to_offset(source, position);
    // source 末端でクリップ
    if offset > source.len() {
        offset = source.len();
    }
    // 非 UTF-8 文字境界なら直前の文字境界まで下げる
    while !source.is_char_boundary(offset) && offset > 0 {
        offset -= 1;
    }
    offset
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use lsp_types::Range;

    /// テスト用ヘルパー: source と change から新しいソースを生成する。
    /// 既存 LineIndex テストと同じ fixture パターンを再利用。
    fn apply(source: &str, range: Option<Range>, text: &str) -> String {
        let index = LineIndex::new(source);
        let change = TextDocumentContentChangeEvent {
            range,
            range_length: None,
            text: text.to_string(),
        };
        apply_content_change(source, &index, &change)
    }

    // ===== 正常系 (a)-(f) =====

    // (a) 単一行内の文字挿入: "SELECT 1" の "1" の前に "DISTINCT " を挿入
    #[test]
    fn test_insert_within_single_line() {
        let source = "SELECT 1";
        let result = apply(
            source,
            Some(Range::new(Position::new(0, 7), Position::new(0, 7))),
            "DISTINCT ",
        );
        assert_eq!(result, "SELECT DISTINCT 1");
    }

    // (b) 行末への文字追加: "SELECT *" の末尾に " FROM t" を追加
    #[test]
    fn test_append_at_end_of_line() {
        let source = "SELECT *";
        let result = apply(
            source,
            Some(Range::new(Position::new(0, 9), Position::new(0, 9))),
            " FROM t",
        );
        assert_eq!(result, "SELECT * FROM t");
    }

    // (c) 行中の範囲置換: "SELECT * FROM users" の "users" -> "orders"
    #[test]
    fn test_replace_range_within_line() {
        let source = "SELECT * FROM users";
        // "SELECT * FROM " = 14 文字、"users" は 14..19
        let result = apply(
            source,
            Some(Range::new(Position::new(0, 14), Position::new(0, 19))),
            "orders",
        );
        assert_eq!(result, "SELECT * FROM orders");
    }

    // (d) 複数行削除: 3行の内、2行目を行ごと削除
    #[test]
    fn test_delete_multiple_lines() {
        let source = "line1\nline2\nline3";
        // start=(1,0), end=(2,0) -> "line2\n" を除去
        let result = apply(
            source,
            Some(Range::new(Position::new(1, 0), Position::new(2, 0))),
            "",
        );
        assert_eq!(result, "line1\nline3");
    }

    // (e) 行全体置換: 2行目を別内容に置換
    #[test]
    fn test_replace_whole_line() {
        let source = "SELECT 1\nFROM t\nWHERE 1";
        // "FROM t" (6文字) -> "FROM users"
        let result = apply(
            source,
            Some(Range::new(Position::new(1, 0), Position::new(1, 6))),
            "FROM users",
        );
        assert_eq!(result, "SELECT 1\nFROM users\nWHERE 1");
    }

    // (f) range=None のフル置換
    #[test]
    fn test_full_replace_when_range_none() {
        let source = "old content";
        let result = apply(source, None, "brand new content");
        assert_eq!(result, "brand new content");
    }

    // ===== エッジケース (g)-(k) =====

    // (g) 末尾越え position のクランプ: 存在しない巨大行/列でもパニックせず末尾へ
    #[test]
    fn test_clamp_position_beyond_end() {
        let source = "abc";
        // 行・列ともに大幅に越える -> 末尾への挿入として扱われる
        let result = apply(
            source,
            Some(Range::new(Position::new(100, 100), Position::new(100, 100))),
            "X",
        );
        assert_eq!(result, "abcX");
    }

    // (h) マルチバイト(日本語)境界での挿入 -- パニックしない
    #[test]
    fn test_multibyte_boundary_insert_no_panic() {
        // "SELECT あ" -- "あ" は3バイト (offset 7..10)
        let source = "SELECT あ";
        // position(0,7) -> byte 7 ("あ" の直前)。ここに "X" を挿入
        let result = apply(
            source,
            Some(Range::new(Position::new(0, 7), Position::new(0, 7))),
            "X",
        );
        assert_eq!(result, "SELECT Xあ");
    }

    // (i) CRLF ソースでの range 適用
    #[test]
    fn test_crlf_source_range_apply() {
        // LineIndex は \r\n を2文字として扱う（\r は1文字、\n で改行）
        let source = "abc\r\ndef";
        // 行0: "abc\r\n" (5バイト)。行1 開始は byte 5。
        // 行1 "def" の先頭に "XY" を挿入
        let result = apply(
            source,
            Some(Range::new(Position::new(1, 0), Position::new(1, 0))),
            "XY",
        );
        assert_eq!(result, "abc\r\nXYdef");
    }

    // (j) 空文字列挿入（削除相当）
    #[test]
    fn test_empty_string_insert_is_delete() {
        let source = "SELECT X FROM t";
        // "X " を削除 (文字位置 7..9)
        let result = apply(
            source,
            Some(Range::new(Position::new(0, 7), Position::new(0, 9))),
            "",
        );
        assert_eq!(result, "SELECT FROM t");
    }

    // (k) start==end の純挿入
    #[test]
    fn test_start_equals_end_is_pure_insert() {
        let source = "SELECT 1";
        let result = apply(
            source,
            Some(Range::new(Position::new(0, 7), Position::new(0, 7))),
            "DISTINCT ",
        );
        assert_eq!(result, "SELECT DISTINCT 1");
    }

    // ===== 追加の堅牢性検証 =====

    // start > end の逆転 range でもパニックせず純挿入として扱う
    #[test]
    fn test_reversed_range_clamped_to_insert() {
        let source = "abcdef";
        let result = apply(
            source,
            Some(Range::new(Position::new(0, 4), Position::new(0, 2))),
            "Z",
        );
        // start(4) > end(2) -> start を end(2) にクランプ -> "ab" + "Z" + "cdef"
        assert_eq!(result, "abZcdef");
    }

    // マルチバイト文字の途中を指す position でも安全にクリップされる
    #[test]
    fn test_multibyte_midpoint_position_clamped() {
        // "あいう" -- 各文字3バイト
        let source = "あいう";
        let result = apply(
            source,
            Some(Range::new(Position::new(0, 1), Position::new(0, 1))),
            "X",
        );
        assert_eq!(result, "あXいう");
    }

    // 空ソースに対するフル置換
    #[test]
    fn test_empty_source_full_replace() {
        let result = apply("", None, "new");
        assert_eq!(result, "new");
    }

    // 空ソースに対する挿入 (range Some だが両端 0,0)
    #[test]
    fn test_empty_source_insert_at_origin() {
        let result = apply(
            "",
            Some(Range::new(Position::new(0, 0), Position::new(0, 0))),
            "abc",
        );
        assert_eq!(result, "abc");
    }

    // 既存 LineIndex fixture との一貫性: "SELECT * FROM users" で置換後も整合
    #[test]
    fn test_consistent_with_line_index_fixture() {
        // line_index.rs の test_single_line と同一 fixture
        let source = "SELECT * FROM users";
        let result = apply(
            source,
            Some(Range::new(Position::new(0, 0), Position::new(0, 6))),
            "INSERT",
        );
        assert_eq!(result, "INSERT * FROM users");
    }
}
