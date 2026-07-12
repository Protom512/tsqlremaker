//! Go to Definition provider
//!
//! カーソル位置のシンボルから定義箇所へナビゲーションを提供する。
//! シンボルテーブルを使用して定義箇所を検索する。
//! - 変数参照 → DECLARE文
//! - テーブル参照 → CREATE TABLE定義
//! - プロシージャ参照 → CREATE PROCEDURE定義
//! - ビュー参照 → CREATE VIEW定義
//! - インデックス参照 → CREATE INDEX定義

use crate::analysis::DocumentAnalysis;
use crate::symbol_store::{SymbolEntry, SymbolStore};
use crate::symbol_table::SymbolTableBuilder;

use lsp_types::{Location, Position, Range, Url};
use tsql_token::TokenKind;

/// カーソル位置のシンボルの定義箇所を検索する（DocumentAnalysis利用）
#[must_use]
pub fn definition_ranges_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
) -> Vec<Range> {
    let (target_kind, target_text) = match analysis.find_token_at_position(position) {
        Some((t, _)) => (t.kind, &*t.text),
        None => return Vec::new(),
    };

    if target_kind == TokenKind::LocalVar {
        find_variable_definition(&analysis.symbol_table, target_text)
    } else {
        find_object_definition(&analysis.symbol_table, target_text)
    }
}

/// 変数定義を検索する
fn find_variable_definition(table: &crate::symbol_table::SymbolTable, name: &str) -> Vec<Range> {
    let mut results = Vec::new();

    // トップレベル変数
    if let Some(var) = SymbolTableBuilder::find_variable(table, name) {
        results.push(var.range);
    }

    // プロシージャボディ内の変数
    for proc in table.procedures.values() {
        for var in &proc.body_variables {
            if var.name.eq_ignore_ascii_case(name) {
                results.push(var.range);
            }
        }
        // パラメータも検索
        for param in &proc.parameters {
            if param.name.eq_ignore_ascii_case(name) {
                results.push(param.range);
            }
        }
    }

    results
}

/// オブジェクト定義（テーブル、プロシージャ、ビュー、インデックス、トリガー）を検索する
fn find_object_definition(table: &crate::symbol_table::SymbolTable, name: &str) -> Vec<Range> {
    let mut results = Vec::new();

    if let Some(tbl) = SymbolTableBuilder::find_table(table, name) {
        results.push(tbl.range);
    }
    if let Some(proc) = SymbolTableBuilder::find_procedure(table, name) {
        results.push(proc.range);
    }
    if let Some(view) = SymbolTableBuilder::find_view(table, name) {
        results.push(view.range);
    }
    if let Some(idx) = SymbolTableBuilder::find_index(table, name) {
        results.push(idx.range);
    }
    if let Some(trigger) = SymbolTableBuilder::find_trigger(table, name) {
        results.push(trigger.range);
    }

    results
}

/// cross-file 版 Go to Definition。
///
/// カーソル位置のシンボルから定義箇所を検索する。検索順序:
///
/// 1. **文書内優先（フォールバック）**: 開いている文書の `symbol_table` を
///    [`definition_ranges_with_analysis`] と同じロジックで検索し、ヒットすれば
///    その範囲を `current_uri` の [`Location`] として返す。変数 (`@var`) は
///    常にこのパスのみ（スコープは文書ローカル）。
/// 2. **cross-file 背景インデックス**: 文書内にオブジェクト定義が無い場合、
///    `store`（[`SymbolStore`]）から他ファイルの CREATE 定義を検索して返す。
///    このとき変数カテゴリのエントリは文書ローカル制約により除外する。
///
/// # 引数
///
/// - `store`: ワークスペース全体の cross-file シンボルインデックス。
/// - `analysis`: カーソル位置の文書の解析結果。
/// - `current_uri`: カーソル位置の文書の URI（文書内ヒットの Location に使用）。
/// - `position`: カーソル位置。
///
/// # 戻り値
///
/// 定義 [`Location`] のリスト。文書内ヒットがあればそれら（`current_uri` 配下）
/// のみを返す（背景インデックスは参照しない）。文書内に無ければ背景インデックスの
/// エントリ（変数を除く）をすべて返す。いずれもヒットしなければ空 `Vec`。
///
/// # graceful degradation
///
/// 不完全 SQL の場合も [`DocumentAnalysis`] は部分 AST + tolerant symbol table を
/// 持つため、`definition_ranges_with_analysis` のフォールバック経路がそのまま
/// 機能する。トークンが取れない位置では空 `Vec` を返す。
#[must_use]
pub fn definition_locations(
    store: &SymbolStore,
    analysis: &DocumentAnalysis,
    current_uri: &Url,
    position: Position,
) -> Vec<Location> {
    let (target_kind, target_text) = match analysis.find_token_at_position(position) {
        Some((t, _)) => (t.kind, &*t.text),
        None => return Vec::new(),
    };

    // 変数は常に文書ローカル（スコープ）。背景インデックスは参照しない。
    if target_kind == TokenKind::LocalVar {
        let ranges = find_variable_definition(&analysis.symbol_table, target_text);
        return ranges_to_locations(ranges, current_uri);
    }

    // オブジェクト: まず文書内 symbol_table を優先。
    let local_ranges = find_object_definition(&analysis.symbol_table, target_text);
    if !local_ranges.is_empty() {
        return ranges_to_locations(local_ranges, current_uri);
    }

    // 文書内に無ければ cross-file 背景インデックス（変数を除く）。
    store
        .lookup(target_text)
        .iter()
        .filter(|e| !is_variable_entry(e))
        .map(|e| Location {
            uri: e.uri.clone(),
            range: e.range,
        })
        .collect()
}

/// [`SymbolEntry`] が変数カテゴリかどうか。cross-file 候補から変数を除外するために使用。
fn is_variable_entry(entry: &SymbolEntry) -> bool {
    use lsp_types::SymbolKind;
    entry.kind == SymbolKind::VARIABLE
}

/// `Vec<Range>` を単一 URI 配下の `Vec<Location>` に変換するヘルパ。
fn ranges_to_locations(ranges: Vec<Range>, uri: &Url) -> Vec<Location> {
    ranges
        .into_iter()
        .map(|range| Location {
            uri: uri.clone(),
            range,
        })
        .collect()
}

/// cross-file テストヘルパ: 1ファイル解析結果を背景インデックスとして store に登録。
#[cfg(test)]
fn index_background(store: &mut SymbolStore, uri: &Url, source: &str) {
    use crate::symbol_store::DocumentSource;
    let analysis = crate::analysis::DocumentAnalysis::new(source);
    store.upsert(uri, &analysis, DocumentSource::Background);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_definition_with_analysis_variable() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "DECLARE @count INT\nSET @count = 1\nSELECT @count",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 5,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_definition_with_analysis_table() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE users (id INT)\nSELECT * FROM users",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_definition_with_analysis_empty_source() {
        let analysis = crate::analysis::DocumentAnalysis::new("");
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 0,
            },
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_definition_with_analysis_no_token_at_position() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT  FROM t");
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_definition_with_analysis_procedure() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE PROCEDURE my_proc AS BEGIN RETURN 1 END",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 18,
            },
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_definition_with_analysis_index() {
        let analysis =
            crate::analysis::DocumentAnalysis::new("CREATE INDEX idx_name ON users (id)");
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            },
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_definition_with_analysis_variable_in_while() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "DECLARE @count INT\nWHILE @count < 10 BEGIN\n  SET @count = @count + 1\nEND",
        );
        // Click on @count inside WHILE condition
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 7,
            },
        );
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_definition_with_analysis_view() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE VIEW active_users AS SELECT * FROM users\nSELECT * FROM active_users",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 17,
            },
        );
        assert_eq!(ranges.len(), 1, "Should find view definition");
        assert_eq!(
            ranges[0].start.line, 0,
            "View definition should be on line 0"
        );
    }

    #[test]
    fn test_definition_with_analysis_trigger() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TRIGGER tr_test ON users FOR INSERT AS BEGIN SELECT 1 END",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 18,
            },
        );
        assert_eq!(ranges.len(), 1, "Should find trigger definition");
    }

    #[test]
    fn test_definition_case_insensitive() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE Users (id INT)\nSELECT * FROM users",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert_eq!(ranges.len(), 1, "Case-insensitive table lookup should work");
    }

    #[test]
    fn test_definition_multiple_object_types() {
        let source =
            "CREATE TABLE t (id INT)\nCREATE VIEW v AS SELECT * FROM t\nCREATE INDEX idx ON t (id)";
        let analysis = crate::analysis::DocumentAnalysis::new(source);
        // Table definition from line 1 "FROM t" — 't' is at char 31
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 31,
            },
        );
        assert!(
            !ranges.is_empty(),
            "Should find table definition from view's FROM clause"
        );
        // View definition — 'v' is at line 1, char 12
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 12,
            },
        );
        assert!(!ranges.is_empty(), "Should find view definition");
    }

    // ===== definition_locations (cross-file) =====

    fn u(p: &str) -> Url {
        Url::parse(p).unwrap()
    }

    #[test]
    fn test_cross_def_variable_is_document_local() {
        // UC: 変数は文書ローカル。背景ストアに同名変数があっても無視し、
        // 文書内の DECLARE だけを返す。
        let current = crate::analysis::DocumentAnalysis::new(
            "DECLARE @count INT\nSET @count = 1\nSELECT @count",
        );
        let mut store = SymbolStore::new();
        // 別ファイルに同名変数がある（背景インデックス）。
        index_background(&mut store, &u("file:///other.sql"), "DECLARE @count INT");

        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 2,
                character: 8,
            },
        );
        assert_eq!(locs.len(), 1, "variable resolves doc-locally only");
        assert_eq!(locs[0].uri, u("file:///cur.sql"));
        assert_eq!(locs[0].range.start.line, 0, "points at DECLARE line");
    }

    #[test]
    fn test_cross_def_variable_not_in_store_when_undeclared() {
        // 変数が文書内で未宣言の場合、背景ストアを参照せず空を返す。
        let current = crate::analysis::DocumentAnalysis::new("SELECT @ghost");
        let mut store = SymbolStore::new();
        index_background(&mut store, &u("file:///other.sql"), "DECLARE @ghost INT");

        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 8,
            },
        );
        assert!(
            locs.is_empty(),
            "undeclared variable must not cross-resolve"
        );
    }

    #[test]
    fn test_cross_def_table_from_other_file() {
        // UC: テーブルが文書内に無く、他ファイルの CREATE TABLE に解決する。
        let current = crate::analysis::DocumentAnalysis::new("SELECT * FROM remote_table");
        let mut store = SymbolStore::new();
        index_background(
            &mut store,
            &u("file:///schema.sql"),
            "CREATE TABLE remote_table (id INT)",
        );

        // 'remote_table' is at "SELECT * FROM " = 14 chars
        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 16,
            },
        );
        assert_eq!(locs.len(), 1, "should resolve cross-file table");
        assert_eq!(locs[0].uri, u("file:///schema.sql"));
    }

    #[test]
    fn test_cross_def_procedure_from_other_file() {
        // UC: プロシージャが他ファイルの CREATE PROCEDURE に解決する。
        let current = crate::analysis::DocumentAnalysis::new("EXEC do_thing");
        let mut store = SymbolStore::new();
        index_background(
            &mut store,
            &u("file:///procs.sql"),
            "CREATE PROCEDURE do_thing AS BEGIN SELECT 1 END",
        );

        // 'do_thing' starts at "EXEC " = 5
        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 7,
            },
        );
        assert_eq!(locs.len(), 1, "should resolve cross-file procedure");
        assert_eq!(locs[0].uri, u("file:///procs.sql"));
    }

    #[test]
    fn test_cross_def_view_from_other_file() {
        // UC: ビューが他ファイルの CREATE VIEW に解決する。
        let current = crate::analysis::DocumentAnalysis::new("SELECT * FROM v_report");
        let mut store = SymbolStore::new();
        index_background(
            &mut store,
            &u("file:///views.sql"),
            "CREATE VIEW v_report AS SELECT 1",
        );

        // 'v_report' is at "SELECT * FROM " = 14
        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 16,
            },
        );
        assert_eq!(locs.len(), 1, "should resolve cross-file view");
        assert_eq!(locs[0].uri, u("file:///views.sql"));
    }

    #[test]
    fn test_cross_def_nonexistent_returns_empty() {
        // UC: 存在しない名前 → 空。文書内・ストアともにヒットしない。
        let current = crate::analysis::DocumentAnalysis::new("SELECT * FROM nowhere");
        let store = SymbolStore::new();

        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 16,
            },
        );
        assert!(locs.is_empty());
    }

    #[test]
    fn test_cross_def_incomplete_sql_graceful() {
        // UC: 不完全 SQL でもクラッシュせず、フォールバックが機能する。
        let current = crate::analysis::DocumentAnalysis::new("SELECT * FROM un");
        let mut store = SymbolStore::new();
        index_background(&mut store, &u("file:///s.sql"), "CREATE TABLE un (id INT)");

        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 15,
            },
        );
        // 'un' matches the background-indexed table.
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].uri, u("file:///s.sql"));
    }

    #[test]
    fn test_cross_def_empty_source_returns_empty() {
        let current = crate::analysis::DocumentAnalysis::new("");
        let store = SymbolStore::new();

        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 0,
            },
        );
        assert!(locs.is_empty());
    }

    #[test]
    fn test_cross_def_no_token_at_position_returns_empty() {
        let current = crate::analysis::DocumentAnalysis::new("SELECT  FROM t");
        let store = SymbolStore::new();

        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 7,
            },
        );
        assert!(locs.is_empty());
    }

    #[test]
    fn test_cross_def_document_local_takes_precedence_over_store() {
        // 文書内に同名オブジェクトがあれば、ストアの背景エントリより優先される。
        let current =
            crate::analysis::DocumentAnalysis::new("CREATE TABLE dup (id INT)\nSELECT * FROM dup");
        let mut store = SymbolStore::new();
        // 別ファイルにも同名テーブル（背景）。
        index_background(
            &mut store,
            &u("file:///bg.sql"),
            "CREATE TABLE dup (id INT)",
        );

        // 'dup' on line 1 "FROM dup" at char 14
        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 1,
                character: 16,
            },
        );
        // Document-local wins: returns current uri only.
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].uri, u("file:///cur.sql"));
    }

    #[test]
    fn test_cross_def_aggregates_multiple_store_entries() {
        // ストアに同名オブジェクトが複数ファイルにある場合、すべて返す。
        let current = crate::analysis::DocumentAnalysis::new("SELECT * FROM shared");
        let mut store = SymbolStore::new();
        index_background(
            &mut store,
            &u("file:///a.sql"),
            "CREATE TABLE shared (id INT)",
        );
        index_background(
            &mut store,
            &u("file:///b.sql"),
            "CREATE TABLE shared (id INT)",
        );

        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 16,
            },
        );
        assert_eq!(locs.len(), 2);
        let uris: Vec<Url> = locs.iter().map(|l| l.uri.clone()).collect();
        assert!(uris.contains(&u("file:///a.sql")));
        assert!(uris.contains(&u("file:///b.sql")));
    }

    #[test]
    fn test_cross_def_case_insensitive() {
        let current = crate::analysis::DocumentAnalysis::new("SELECT * FROM Users");
        let mut store = SymbolStore::new();
        index_background(
            &mut store,
            &u("file:///s.sql"),
            "CREATE TABLE users (id INT)",
        );

        let locs = definition_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 16,
            },
        );
        assert_eq!(locs.len(), 1);
    }

    // ===== temporary table (#temp / ##global) definition jump =====
    //
    // definition_ranges_with_analysis は find_token_at_position と symbol_table を直接
    // 使用し、token_matches_symbol を経由しない。そのため #temp の TempTable トークンは
    // 以前から解決可能だが、以下のテストで回帰（Parser/Lexer/symbol_table の変更による
    // 壊れ）を検知する。

    #[test]
    fn test_definition_with_analysis_local_temp_table() {
        // UC-1: CREATE TABLE #temp ... SELECT * FROM #temp で SELECT 側から CREATE 行へ
        // 定義ジャンプできること。lexer は text="#temp" の TempTable トークンを生成し、
        // ddl が同名で symbol_table に登録する（is_temporary=true）。
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE #temp (id INT)\nSELECT * FROM #temp",
        );
        // SELECT FROM "#temp" は "SELECT * FROM " = 14 文字目から開始
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 14,
            },
        );
        assert_eq!(ranges.len(), 1, "local #temp definition should resolve");
        assert_eq!(ranges[0].start.line, 0, "should jump to CREATE TABLE line");
        // text に '#' が含まれていることをテストフィクスチャで固定化
        let (tok, _) = analysis
            .find_token_at_position(Position {
                line: 1,
                character: 14,
            })
            .expect("token at #temp");
        assert_eq!(tok.kind, TokenKind::TempTable);
        assert_eq!(&*tok.text, "#temp");
    }

    #[test]
    fn test_definition_with_analysis_global_temp_table() {
        // UC-2 前提: グローバル一時テーブル ##global も定義ジャンプできること。
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE ##global (id INT)\nSELECT * FROM ##global",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 15,
            },
        );
        assert_eq!(ranges.len(), 1, "global ##global definition should resolve");
        assert_eq!(ranges[0].start.line, 0);
        let (tok, _) = analysis
            .find_token_at_position(Position {
                line: 1,
                character: 15,
            })
            .expect("token at ##global");
        assert_eq!(tok.kind, TokenKind::GlobalTempTable);
        assert_eq!(&*tok.text, "##global");
    }

    #[test]
    fn test_definition_with_analysis_temp_table_case_insensitive() {
        // #Temp と #temp は大文字小文字無視で同一シンボルに解決すること。
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE #Temp (id INT)\nSELECT * FROM #temp",
        );
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 14,
            },
        );
        assert_eq!(ranges.len(), 1, "case-insensitive #temp lookup");
        assert_eq!(ranges[0].start.line, 0);
    }

    #[test]
    fn test_definition_with_analysis_undefined_temp_table_returns_empty() {
        // UC-3: 未定義の #not_defined は定義が無いため空を返し、クラッシュしないこと。
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT * FROM #not_defined");
        let ranges = definition_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            },
        );
        assert!(ranges.is_empty(), "undefined #temp must not crash");
    }
}
