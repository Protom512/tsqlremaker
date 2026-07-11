//! Find References provider
//!
//! カーソル位置のシンボルの全参照箇所を検索する。
//! - 変数: DECLARE + 全使用箇所
//! - テーブル: CREATE TABLE + SELECT/INSERT/UPDATE/DELETE内の参照
//! - プロシージャ: CREATE PROCEDURE + EXEC呼び出し
//! - ビュー: CREATE VIEW + SELECT内の参照

use crate::analysis::DocumentAnalysis;
use crate::symbol_store::{SymbolEntry, SymbolStore};
use crate::token_matches_symbol;
use lsp_types::{Location, Position, Range, Url};
use std::sync::Arc;
use tsql_token::TokenKind;

/// カーソル位置のシンボルの全参照箇所を検索する（DocumentAnalysis利用）
#[must_use]
pub fn reference_ranges_with_analysis(
    analysis: &DocumentAnalysis,
    position: Position,
    include_declaration: bool,
) -> Vec<Range> {
    let (target_kind, target_text) = match analysis.find_token_at_position(position) {
        Some((t, _)) => (t.kind, &*t.text),
        None => return Vec::new(),
    };

    let is_var = target_kind == TokenKind::LocalVar;

    let mut refs = Vec::new();

    for token in &analysis.tokens {
        if token_matches_symbol(token.kind, &token.text, target_text, is_var) {
            let range = analysis
                .line_index
                .offset_to_range(token.span.start, token.span.end);

            let is_declaration = !include_declaration
                && is_definition_token(&analysis.source, token.span.start as usize, is_var);

            if include_declaration || !is_declaration {
                refs.push(range);
            }
        }
    }

    refs.dedup_by(|a, b| a.start == b.start && a.end == b.end);
    refs
}

/// cross-file 版 Find All References。
///
/// [`definition_locations`](crate::definition::definition_locations) と対称な API だが、
/// **設計上の非対称性**がある: [`SymbolStore`] は **定義 (CREATE TABLE/PROC/VIEW/INDEX/
/// TRIGGER + 変数宣言)** のみをインデックスする。Find All References が欲しいのは
/// **使用箇所** (他ファイルの SELECT/INSERT/UPDATE/DELETE/EXEC 中の参照) であり、
/// これはストアに存在しない。したがって `reference_locations` は
/// [`SymbolStore::lookup`] を使わず、各文書のトークンをスキャンして使用箇所を集める。
///
/// # 引数
///
/// - `store`: cross-file 定義インデックス。`include_declaration` 時に CREATE 定義の
///   [`Location`] を得るためにのみ使用する（使用箇所の収集には使わない）。
/// - `current_analysis`: カーソル位置の文書の解析結果。
/// - `current_uri`: カーソル位置の文書の URI。
/// - `position`: カーソル位置。
/// - `include_declaration`: 定義 (CREATE / DECLARE) を結果に含めるか。
/// - `docs`: 検索対象となる全既知文書のスナップショット
///   `(uri, analysis)` のリスト。呼び出し側 (server.rs) がロック順序を守って
///   構築したスナップショットを渡す（本関数は純粋関数としてユニットテスト可能）。
///
/// # 戻り値
///
/// 重複を除去した使用箇所 [`Location`] のリスト。
///
/// # 変数パス（`@var`）
///
/// 変数は常に文書ローカル（スコープ）。他ファイルに同名変数があってもクロスファイル
/// しない（[`definition_locations`](crate::definition::definition_locations) の変数短絡と
/// 同じ）。`docs` を走査せず、`current_analysis` の参照のみを `current_uri` 配下で返す。
///
/// # オブジェクトパス
///
/// 1. **カレント文書の使用箇所**: [`reference_ranges_with_analysis`] を再利用。
/// 2. **他文書の使用箇所**: `docs` スナップショットを走査し、各トークンについて
///    [`token_matches_symbol`] + [`is_definition_token`] で使用箇所を判定・収集。
/// 3. **定義の追加**: `include_declaration` かつ定義が別ファイルに存在する場合、
///    [`SymbolStore::lookup`] から CREATE 定義の [`Location`] を追加する。
///
/// # 計算量
///
/// 他文書の使用箇所は `docs` の全トークンを走査するため **O(D × T)**（D=文書数、
/// T=平均トークン数）。ワークスペースが巨大な場合は名前→文書の逆引きインデックスが
/// 将来の最適化候補となる（現状は線形走査で十分）。
///
/// # graceful degradation
///
/// 不完全 SQL の場合も [`DocumentAnalysis`] は部分トークンを持つため、トークン走査が
/// そのまま機能する。トークンが取れない位置では空 `Vec` を返す。
///
/// # Panics
///
/// この関数はパニックしない（ロック取得は呼び出し側、`Url` の再パースも呼び出し側で済）。
#[must_use]
pub fn reference_locations(
    store: &SymbolStore,
    current_analysis: &DocumentAnalysis,
    current_uri: &Url,
    position: Position,
    include_declaration: bool,
    docs: &[(Url, Arc<DocumentAnalysis>)],
) -> Vec<Location> {
    let (target_kind, target_text) = match current_analysis.find_token_at_position(position) {
        Some((t, _)) => (t.kind, &*t.text),
        None => return Vec::new(),
    };

    // 変数は文書ローカル（スコープ）。クロスファイルしない。
    if target_kind == TokenKind::LocalVar {
        let ranges =
            reference_ranges_with_analysis(current_analysis, position, include_declaration);
        return ranges_to_locations(ranges, current_uri);
    }

    let search_upper = target_text.to_ascii_uppercase();
    let mut locations = Vec::new();

    // (1) カレント文書の使用箇所（reference_ranges_with_analysis は include_declaration
    //     を見て定義トークンを除外/包含する）。
    for range in reference_ranges_with_analysis(current_analysis, position, include_declaration) {
        locations.push(Location {
            uri: current_uri.clone(),
            range,
        });
    }

    // (2) 他文書の使用箇所: docs スナップショットを走査してトークンレベルで使用箇所を収集。
    for (uri, analysis) in docs {
        // カレント文書は (1) で処理済み。重複回避のためスキップ。
        if uri == current_uri {
            continue;
        }
        for token in &analysis.tokens {
            if !token_matches_symbol(token.kind, &token.text, &search_upper, false) {
                continue;
            }
            let is_def = is_definition_token(&analysis.source, token.span.start as usize, false);
            // include_declaration でなければ他ファイルの定義トークンも含めない。
            if is_def && !include_declaration {
                continue;
            }
            locations.push(Location {
                uri: uri.clone(),
                range: analysis
                    .line_index
                    .offset_to_range(token.span.start, token.span.end),
            });
        }
    }

    // (3) 定義の追加: include_declaration で、かつ定義がストア内の別ファイル（カレント
    //     文書以外）に存在する場合、CREATE 定義 Location を追加する。カレント文書内の
    //     定義は (1) で既に含まれているため、別ファイルのもののみ追加する。
    if include_declaration {
        for entry in store.lookup(target_text).iter() {
            if is_variable_entry(entry) {
                continue;
            }
            if &entry.uri == current_uri {
                // カレント文書の定義は (1) で追加済み。
                continue;
            }
            locations.push(Location {
                uri: entry.uri.clone(),
                range: entry.range,
            });
        }
    }

    dedup_locations(&mut locations);
    locations
}

/// [`SymbolEntry`] が変数カテゴリかどうか。クロスファイル候補から変数を除外するために使用。
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

/// 同一 `(uri, range)` の [`Location`] を隣接重複として除去する。
///
/// 呼び出し側が事前ソートしないため完全な全域重複除去ではないが、(1) の
/// `reference_ranges_with_analysis` が既に文書内で `dedup` 済みの連続した範囲を返すこと、
/// およびストア由来の定義 Location が直前のカレント文書トークンと重なる典型的なケースを
/// 吸収するのに十分である（`Vec::dedup_by` は O(n)）。
fn dedup_locations(locations: &mut Vec<Location>) {
    locations.dedup_by(|a, b| {
        a.uri == b.uri && a.range.start == b.range.start && a.range.end == b.range.end
    });
}

/// Check if `haystack` ends with `suffix`, comparing ASCII characters case-insensitively.
#[inline]
fn ends_with_ignore_ascii_case(haystack: &str, suffix: &str) -> bool {
    if suffix.len() > haystack.len() {
        return false;
    }
    let haystack_bytes = haystack.as_bytes();
    let suffix_bytes = suffix.as_bytes();
    haystack_bytes[haystack.len() - suffix.len()..]
        .iter()
        .zip(suffix_bytes)
        .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

/// トークンが定義箇所かどうかを判定する
fn is_definition_token(source: &str, span_start: usize, is_var: bool) -> bool {
    let before = &source[..span_start];
    let trimmed = before.trim_end();

    if is_var {
        // 変数定義: DECLARE @var
        if ends_with_ignore_ascii_case(trimmed, "DECLARE") || trimmed.ends_with(',') {
            return true;
        }
    } else {
        // テーブル/プロシージャ/ビュー/インデックス/トリガー定義: CREATE [OBJECT] name
        if ends_with_ignore_ascii_case(trimmed, "CREATE TABLE")
            || ends_with_ignore_ascii_case(trimmed, "CREATE PROCEDURE")
            || ends_with_ignore_ascii_case(trimmed, "CREATE VIEW")
            || ends_with_ignore_ascii_case(trimmed, "CREATE INDEX")
            || ends_with_ignore_ascii_case(trimmed, "CREATE UNIQUE INDEX")
            || ends_with_ignore_ascii_case(trimmed, "CREATE TRIGGER")
        {
            return true;
        }
    }
    false
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

    // --- reference_ranges_with_analysis tests ---

    #[test]
    fn test_references_with_analysis_variable() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "DECLARE @count INT\nSET @count = 1\nSELECT @count",
        );
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 1,
                character: 5,
            },
            true,
        );
        assert_eq!(ranges.len(), 3);
    }

    #[test]
    fn test_references_with_analysis_table() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE users (id INT)\nSELECT * FROM users",
        );
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            },
            true,
        );
        assert!(ranges.len() >= 2);
    }

    #[test]
    fn test_references_with_analysis_empty_source() {
        let analysis = crate::analysis::DocumentAnalysis::new("");
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 0,
            },
            true,
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_references_with_analysis_no_token() {
        let analysis = crate::analysis::DocumentAnalysis::new("SELECT  FROM t");
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 7,
            },
            true,
        );
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_references_with_analysis_exclude_declaration() {
        let analysis = crate::analysis::DocumentAnalysis::new(
            "CREATE TABLE users (id INT)\nSELECT * FROM users",
        );
        let ranges = reference_ranges_with_analysis(
            &analysis,
            Position {
                line: 0,
                character: 14,
            },
            false,
        );
        assert!(!ranges.is_empty());
        for range in &ranges {
            assert_ne!(range.start.line, 0, "Definition should be excluded");
        }
    }

    #[test]
    fn test_is_definition_unique_index() {
        // CREATE UNIQUE INDEX idx ON t(c) — idx should be recognized as definition
        assert!(is_definition_token(
            "CREATE TABLE t (c INT)\nCREATE UNIQUE INDEX idx ON t (c)",
            "CREATE TABLE t (c INT)\nCREATE UNIQUE INDEX ".len(),
            false,
        ));
    }

    #[test]
    fn test_is_definition_trigger() {
        // CREATE TRIGGER trg ... — trg should be recognized as definition
        assert!(is_definition_token(
            "CREATE TABLE t (c INT)\nCREATE TRIGGER trg ON t FOR INSERT AS BEGIN END",
            "CREATE TABLE t (c INT)\nCREATE TRIGGER ".len(),
            false,
        ));
    }

    #[test]
    fn test_is_definition_regular_index() {
        // CREATE INDEX idx — still recognized
        assert!(is_definition_token(
            "CREATE TABLE t (c INT)\nCREATE INDEX idx ON t (c)",
            "CREATE TABLE t (c INT)\nCREATE INDEX ".len(),
            false,
        ));
    }

    #[test]
    fn test_is_not_definition_select_reference() {
        // SELECT FROM users — users is NOT a definition
        assert!(!is_definition_token(
            "CREATE TABLE users (id INT)\nSELECT * FROM ",
            "CREATE TABLE users (id INT)\nSELECT * FROM ".len(),
            false,
        ));
    }

    #[test]
    fn test_is_definition_variable_in_declare() {
        // DECLARE @count — @count IS a definition
        assert!(is_definition_token(
            "DECLARE @count INT",
            "DECLARE ".len(),
            true
        ));
    }

    #[test]
    fn test_ends_with_ignore_ascii_case() {
        assert!(ends_with_ignore_ascii_case("CREATE TABLE", "TABLE"));
        assert!(ends_with_ignore_ascii_case("create table", "TABLE"));
        assert!(ends_with_ignore_ascii_case("CREATE table", "CREATE TABLE"));
        assert!(!ends_with_ignore_ascii_case("CREATE", "CREATE TABLE"));
        assert!(ends_with_ignore_ascii_case("DECLARE", "DECLARE"));
        assert!(!ends_with_ignore_ascii_case("DECLARE @x", "DECLARE"));
    }

    // ===== reference_locations (cross-file) =====

    fn u(p: &str) -> Url {
        Url::parse(p).unwrap()
    }

    fn analysis_of(src: &str) -> Arc<DocumentAnalysis> {
        Arc::new(DocumentAnalysis::new(src))
    }

    #[test]
    fn test_cross_ref_variable_is_document_local() {
        // UC4: 変数は文書ローカル。別ファイルに同名変数があっても、カレント文書内の
        // 参照のみを current_uri 配下で返す。docs に別ファイルがあっても無視する。
        let current = analysis_of("DECLARE @count INT\nSET @count = 1\nSELECT @count");
        let other = analysis_of("DECLARE @count INT\nSET @count = 99");
        let docs = vec![
            (u("file:///cur.sql"), current.clone()),
            (u("file:///other.sql"), other),
        ];
        let mut store = SymbolStore::new();
        index_background(&mut store, &u("file:///other.sql"), "DECLARE @count INT");

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 2,
                character: 8,
            },
            true,
            &docs,
        );
        // 変数はすべてカレント文書内 (DECLARE + SET + SELECT = 3)。
        assert!(
            locs.iter().all(|l| l.uri == u("file:///cur.sql")),
            "variable must not cross-resolve"
        );
        assert_eq!(locs.len(), 3, "DECLARE + SET + SELECT in current doc");
    }

    #[test]
    fn test_cross_ref_table_usages_across_files() {
        // UC: テーブルの使用箇所が複数の open ファイルにまたがる。
        // cur.sql: CREATE TABLE + SELECT (2 箇所)
        // other.sql: SELECT FROM shared_tbl (1 使用箇所)
        let cur_src = "CREATE TABLE shared_tbl (id INT)\nSELECT * FROM shared_tbl";
        let current = analysis_of(cur_src);
        let other = analysis_of("SELECT * FROM shared_tbl WHERE id = 1");
        let docs = vec![
            (u("file:///cur.sql"), current.clone()),
            (u("file:///other.sql"), other),
        ];
        let mut store = SymbolStore::new();
        index_background(&mut store, &u("file:///cur.sql"), cur_src);

        // カーソルは cur.sql の CREATE TABLE 上 (include_declaration = true)
        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 17, // "shared_tbl" in CREATE TABLE
            },
            true,
            &docs,
        );
        let uris: Vec<Url> = locs.iter().map(|l| l.uri.clone()).collect();
        assert!(
            uris.contains(&u("file:///cur.sql")),
            "should include current-doc usages"
        );
        assert!(
            uris.contains(&u("file:///other.sql")),
            "should include cross-file usage"
        );
    }

    #[test]
    fn test_cross_ref_include_declaration_adds_definition_from_store() {
        // UC: include_declaration=true で、定義が別ファイルにある場合、store.lookup から
        // CREATE 定義 Location を追加する。
        let current = analysis_of("SELECT * FROM remote_tbl");
        let def = analysis_of("CREATE TABLE remote_tbl (id INT)");
        let docs = vec![
            (u("file:///cur.sql"), current.clone()),
            (u("file:///schema.sql"), def.clone()),
        ];
        let mut store = SymbolStore::new();
        index_background(
            &mut store,
            &u("file:///schema.sql"),
            "CREATE TABLE remote_tbl (id INT)",
        );

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 16, // "remote_tbl" in SELECT
            },
            true,
            &docs,
        );
        let def_locs: Vec<&Location> = locs
            .iter()
            .filter(|l| l.uri == u("file:///schema.sql"))
            .collect();
        assert!(
            !def_locs.is_empty(),
            "include_declaration should add cross-file CREATE definition"
        );
    }

    #[test]
    fn test_cross_ref_exclude_declaration_drops_definition() {
        // UC: include_declaration=false で、他ファイルの CREATE 定義トークンを含めない。
        let current = analysis_of("SELECT * FROM remote_tbl");
        let def = analysis_of("CREATE TABLE remote_tbl (id INT)");
        let docs = vec![
            (u("file:///cur.sql"), current.clone()),
            (u("file:///schema.sql"), def),
        ];
        let mut store = SymbolStore::new();
        index_background(
            &mut store,
            &u("file:///schema.sql"),
            "CREATE TABLE remote_tbl (id INT)",
        );

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 16,
            },
            false,
            &docs,
        );
        // schema.sql の CREATE 定義 (line 0) は含まれない。
        let schema_def: Vec<&Location> = locs
            .iter()
            .filter(|l| l.uri == u("file:///schema.sql"))
            .collect();
        assert!(
            schema_def.is_empty(),
            "include_declaration=false must not include cross-file definition"
        );
        // カレント文書の使用箇所は残る。
        assert!(
            locs.iter().any(|l| l.uri == u("file:///cur.sql")),
            "current-doc usage should remain"
        );
    }

    #[test]
    fn test_cross_ref_nonexistent_returns_empty() {
        // UC: どこにも定義されておらず、カーソル位置が唯一の出現箇所の名前。
        // カーソルトークン自体は「使用」として扱われるため（文書ローカル版
        // reference_ranges_with_analysis と同じ振る舞い）、カレント文書の
        // その1件のみを返す。別ファイルの無関係なトークンは一切含まれない。
        let current = analysis_of("SELECT * FROM nowhere");
        let other = analysis_of("SELECT * FROM real_table");
        let docs = vec![
            (u("file:///cur.sql"), current.clone()),
            (u("file:///other.sql"), other),
        ];
        let store = SymbolStore::new();

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 16,
            },
            true,
            &docs,
        );
        // カレント文書の 'nowhere' 1件のみ。other.sql の real_table は無関係。
        assert_eq!(locs.len(), 1, "only the cursor occurrence should match");
        assert_eq!(locs[0].uri, u("file:///cur.sql"));
        assert!(
            locs.iter().all(|l| l.uri == u("file:///cur.sql")),
            "unrelated files must not contribute references"
        );
    }

    #[test]
    fn test_cross_ref_case_insensitive() {
        // UC: 大文字小文字無視で他ファイルの使用箇所を収集する。
        let current = analysis_of("SELECT * FROM Users");
        let other = analysis_of("INSERT INTO users VALUES (1)");
        let docs = vec![
            (u("file:///cur.sql"), current.clone()),
            (u("file:///other.sql"), other),
        ];
        let mut store = SymbolStore::new();
        index_background(
            &mut store,
            &u("file:///cur.sql"),
            "CREATE TABLE Users (id INT)",
        );

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 16, // "Users"
            },
            true,
            &docs,
        );
        let uris: Vec<Url> = locs.iter().map(|l| l.uri.clone()).collect();
        assert!(
            uris.contains(&u("file:///other.sql")),
            "case-insensitive cross-file usage"
        );
    }

    #[test]
    fn test_cross_ref_procedure_exec_usage() {
        // UC: プロシージャの EXEC 呼び出しが他ファイルで拾える。
        let current = analysis_of("CREATE PROCEDURE my_proc AS BEGIN SELECT 1 END");
        let other = analysis_of("EXEC my_proc");
        let docs = vec![
            (u("file:///cur.sql"), current.clone()),
            (u("file:///caller.sql"), other),
        ];
        let mut store = SymbolStore::new();
        index_background(
            &mut store,
            &u("file:///cur.sql"),
            "CREATE PROCEDURE my_proc AS BEGIN SELECT 1 END",
        );

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 18, // "my_proc" in CREATE PROCEDURE
            },
            true,
            &docs,
        );
        let uris: Vec<Url> = locs.iter().map(|l| l.uri.clone()).collect();
        assert!(
            uris.contains(&u("file:///caller.sql")),
            "EXEC usage in other file should be collected"
        );
    }

    #[test]
    fn test_cross_ref_no_token_at_position_returns_empty() {
        // UC: トークンが取れない位置 → 空。クラッシュしない。
        let current = analysis_of("SELECT  FROM t");
        let docs = vec![(u("file:///cur.sql"), current.clone())];
        let store = SymbolStore::new();

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 7,
            },
            true,
            &docs,
        );
        assert!(locs.is_empty());
    }

    #[test]
    fn test_cross_ref_empty_source_returns_empty() {
        let current = analysis_of("");
        let docs = vec![(u("file:///cur.sql"), current.clone())];
        let store = SymbolStore::new();

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 0,
            },
            true,
            &docs,
        );
        assert!(locs.is_empty());
    }

    #[test]
    fn test_cross_ref_incomplete_sql_graceful() {
        // UC: 不完全 SQL でもクラッシュせず、使用箇所を返す。
        let current = analysis_of("SELECT * FROM un");
        let docs = vec![(u("file:///cur.sql"), current.clone())];
        let mut store = SymbolStore::new();
        index_background(&mut store, &u("file:///s.sql"), "CREATE TABLE un (id INT)");

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 0,
                character: 15,
            },
            true,
            &docs,
        );
        // カレント文書の使用箇所 (SELECT FROM un) が 1 つ。ストアの別ファイル定義は
        // include_declaration=true なので追加される。
        assert!(
            !locs.is_empty(),
            "graceful: should still return current usage"
        );
        assert!(
            locs.iter().any(|l| l.uri == u("file:///cur.sql")),
            "current-doc usage present"
        );
    }

    #[test]
    fn test_cross_ref_definition_in_current_doc_not_duplicated() {
        // UC: 定義がカレント文書内にある場合、(1) reference_ranges_with_analysis が
        // 既に含めているため、store.lookup から同じ Location を二重に追加しない。
        let cur_src = "CREATE TABLE dup (id INT)\nSELECT * FROM dup";
        let current = analysis_of(cur_src);
        let docs = vec![(u("file:///cur.sql"), current.clone())];
        let mut store = SymbolStore::new();
        index_background(&mut store, &u("file:///cur.sql"), cur_src);

        let locs = reference_locations(
            &store,
            &current,
            &u("file:///cur.sql"),
            Position {
                line: 1,
                character: 16, // "dup" in SELECT
            },
            true,
            &docs,
        );
        // CREATE 定義 (line 0) と SELECT 使用箇所 (line 1)。同じ位置が2回現れないこと。
        let line0_count = locs
            .iter()
            .filter(|l| l.uri == u("file:///cur.sql") && l.range.start.line == 0)
            .count();
        assert_eq!(line0_count, 1, "current-doc definition must not duplicate");
    }
}
