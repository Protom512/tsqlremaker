//! Completion 生成
//!
//! SQL キーワード、データ型、組み込み関数の補完候補を提供する。

use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionResponse, InsertTextFormat,
};
use std::sync::LazyLock;

use tsql_parser::ast::{Statement, TableReference};

use crate::config::CompletionConfig;
use crate::symbol_table::{ColumnSymbol, SymbolTable, SymbolTableBuilder, TableSymbol};

/// 全補完候補のグローバルキャッシュ。初回アクセス時のみ構築される。
static COMPLETE_ALL_CACHE: LazyLock<CompletionResponse> = LazyLock::new(build_complete_all);

/// キーワード補完のグローバルキャッシュ。
static COMPLETE_KEYWORDS_CACHE: LazyLock<CompletionResponse> =
    LazyLock::new(build_complete_keywords);

/// Label shown in the detail field for T-SQL keyword completion items.
const KEYWORD_DETAIL: &str = "T-SQL Keyword";

/// 閾値: 補完候補数がこの件数以上の場合、`is_incomplete = true` を立てる。
///
/// `complete_all()` が返す静的リストのアイテム数と一致する。これと同数以上の件数を
/// 返すコンテキスト（例: 全候補を返す Expression 分岐、多数のテーブルがある
/// Table 分岐）は、クライアントに「更なるタイピングで再クエリせよ」と通知する
/// ため `is_incomplete = true` を設定する (#54 Task 5)。
///
/// `complete_all()` の件数はコンパイル時には未知なため、初回構築時に計測して
/// `LazyLock` でキャッシュする。
static INCOMPLETE_THRESHOLD: LazyLock<usize> = LazyLock::new(|| match &*COMPLETE_ALL_CACHE {
    CompletionResponse::List(list) => list.items.len(),
    CompletionResponse::Array(items) => items.len(),
});

/// 関数名とパラメータリストからLSP snippet形式のinsert_textを生成する
///
/// `DocEntry.params`（クリーンなパラメータ名配列）を直接使用し、
/// syntax文字列のブラケット表記（`[, style]`等）による問題を回避する。
///
/// # Examples
/// - `build_function_snippet("SUBSTRING", &["expression", "start", "length"])`
///   → `SUBSTRING(${1:expression}, ${2:start}, ${3:length})`
/// - `build_function_snippet("GETDATE", &[])` → `GETDATE()`
#[must_use]
pub(crate) fn build_function_snippet(name: &str, params: &[&str]) -> String {
    if params.is_empty() {
        return format!("{name}()");
    }
    let placeholders: Vec<String> = params
        .iter()
        .enumerate()
        .map(|(i, p)| format!("${{{}:{p}}}", i + 1))
        .collect();
    format!("{name}({})", placeholders.join(", "))
}

/// syntax文字列がカンマ区切りの括弧構文かどうかを判定する
///
/// カンマ区切りではない関数（`CAST(expr AS type)`等）や
/// 括弧なしの関数（`IDENTITY`等）はsnippetプレースホルダー生成に
/// 適さないためfalseを返す。
#[must_use]
fn is_comma_separated_syntax(syntax: &str) -> bool {
    if let (Some(open), Some(close)) = (syntax.find('('), syntax.rfind(')')) {
        if open < close {
            let inner = &syntax[open + 1..close];
            return !inner.contains(" AS ") && !inner.contains('\'') && !inner.contains('|');
        }
    }
    false
}

/// 全ての補完候補を返す（キャッシュ済み）
///
/// 内部の `Lazy` static から参照を返す。呼び出し元で所有権が必要な場合は
/// `.clone()` すること。
#[must_use]
pub fn complete_all() -> &'static CompletionResponse {
    &COMPLETE_ALL_CACHE
}

/// 全ての補完候補を構築する（内部実装）
fn build_complete_all() -> CompletionResponse {
    let mut items = Vec::new();

    // Keywords from db_docs
    for entry in crate::db_docs::keywords() {
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(KEYWORD_DETAIL.to_string()),
            ..CompletionItem::default()
        });
    }

    // Datatypes from db_docs
    for entry in crate::db_docs::datatypes() {
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some(entry.description.to_string()),
            ..CompletionItem::default()
        });
    }

    // Functions from db_docs — snippet or plain text depending on syntax
    for entry in crate::db_docs::functions() {
        let (insert_text, format) = if is_comma_separated_syntax(entry.syntax) {
            (
                build_function_snippet(entry.name, entry.params),
                lsp_types::InsertTextFormat::SNIPPET,
            )
        } else {
            // Non-comma syntax (e.g., CAST(expr AS type)) — plain text
            (
                entry.syntax.to_string(),
                lsp_types::InsertTextFormat::PLAIN_TEXT,
            )
        };
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(format!("{} — {}", entry.syntax, entry.description)),
            insert_text: Some(insert_text),
            insert_text_format: Some(format),
            ..CompletionItem::default()
        });
    }

    // System variables from db_docs
    for entry in crate::db_docs::system_variables() {
        items.push(CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some(entry.description.to_string()),
            ..CompletionItem::default()
        });
    }

    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

/// キーワード補完のみを返す（キャッシュ済み）
///
/// `complete_all()` と同様に `&'static` 参照を返し、不要な clone を回避する。
#[must_use]
pub fn complete_keywords() -> &'static CompletionResponse {
    &COMPLETE_KEYWORDS_CACHE
}

/// キーワード補完を構築する（内部実装）
fn build_complete_keywords() -> CompletionResponse {
    let items = crate::db_docs::keywords()
        .iter()
        .map(|entry| CompletionItem {
            label: entry.name.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(KEYWORD_DETAIL.to_string()),
            ..CompletionItem::default()
        })
        .collect();

    CompletionResponse::List(CompletionList {
        is_incomplete: false,
        items,
    })
}

/// 補完コンテキスト（カーソル直前のトークンから推定、#126 / #54）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompletionContext {
    /// テーブル名が期待される位置 (FROM / JOIN / INTO / UPDATE / TABLE の直後)。
    Table,
    /// 変数名の宣言位置 (`DECLARE @<name>`)。静的候補は無意味なため空。
    VariableDeclaration,
    /// カラム名が期待される位置 (`<alias>.` または `<table>.` の直後)。
    /// 補完対象カラムは [`complete_for_context`] が FROM 句のエイリアスから解決する。
    Column,
    /// 式が期待される位置 (SELECT / WHERE / SET / 値 等)。全候補を返す。
    Expression,
}

/// カーソル直前の行プレフィックスから補完コンテキストを推定する。
///
/// ホワイトスペース区切りの最終トークン（およびその直前トークン）から、
/// カラム位置(ドット直後)・テーブル名位置・変数宣言位置・式位置を判定する。
pub(crate) fn detect_context(prefix: &str) -> CompletionContext {
    // カラム位置: 末尾トークンが `<qualifier>.` (ドット終端)。
    if detect_column_trigger(prefix.trim_end()).is_some() {
        return CompletionContext::Column;
    }

    let upper = prefix.trim_end().to_uppercase();
    let tokens: Vec<&str> = upper.split_whitespace().collect();
    let table_kw = ["FROM", "JOIN", "INTO", "UPDATE", "TABLE"];

    let last_token = tokens.last().copied().unwrap_or("");

    // 変数宣言位置: "DECLARE" が最終トークン、または "@<name>" の直前が DECLARE。
    let is_declare =
        last_token == "DECLARE" || tokens.len() >= 2 && tokens[tokens.len() - 2] == "DECLARE";
    if is_declare {
        return CompletionContext::VariableDeclaration;
    }

    // テーブル名位置: 最終トークン、またはその直前がテーブル系キーワード。
    let last_is_table = table_kw.contains(&last_token);
    let prev_is_table = tokens.len() >= 2 && table_kw.contains(&tokens[tokens.len() - 2]);
    if last_is_table || prev_is_table {
        return CompletionContext::Table;
    }

    CompletionContext::Expression
}

/// プレフィックス末尾が `<qualifier>.` 形式なら修飾名 (alias/table) を返す。
///
/// 例: `SELECT u.` → `Some("u")`、`SELECT users.` → `Some("users")`、
/// `SELECT u.id` → `None` (ドットの後に既に文字がある)。
///
/// `trimmed_prefix` は末尾空白が除去済みであること。
fn detect_column_trigger(trimmed_prefix: &str) -> Option<&str> {
    // Last whitespace-delimited token (e.g. `SELECT u.` → `u.`).
    // `rsplit_whitespace` does not exist on `&str`; `split_whitespace().last()`
    // gives the same "final token" semantics without reversing.
    let last_token = trimmed_prefix.split_whitespace().next_back()?;
    let qualifier = last_token.strip_suffix('.')?;
    if qualifier.is_empty() {
        return None;
    }
    let valid = qualifier
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '#');
    if valid {
        Some(qualifier)
    } else {
        None
    }
}

/// SELECT 文の FROM 句（JOIN 含む）から `(alias_or_name, table_name)` の一覧を収集する。
///
/// `alias` があれば alias をキーに、なければテーブル名そのものをキーにする。
/// `from.tables` (カンマ区切り複数テーブル) と `from.joins` の両方を走査する。
fn collect_from_targets(stmts: &[Statement]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for stmt in stmts {
        collect_from_targets_in_stmt(stmt, &mut out);
    }
    out
}

/// 単一の Statement から FROM 対象を再帰収集する。
fn collect_from_targets_in_stmt(stmt: &Statement, out: &mut Vec<(String, String)>) {
    match stmt {
        Statement::Select(sel) => {
            if let Some(from) = &sel.from {
                for tr in &from.tables {
                    collect_from_table_ref(tr, out);
                }
                for join in &from.joins {
                    collect_from_table_ref(&join.table, out);
                }
            }
        }
        Statement::Block(b) => {
            for s in &b.statements {
                collect_from_targets_in_stmt(s, out);
            }
        }
        Statement::If(i) => {
            collect_from_targets_in_stmt(&i.then_branch, out);
            if let Some(e) = &i.else_branch {
                collect_from_targets_in_stmt(e, out);
            }
        }
        Statement::While(w) => collect_from_targets_in_stmt(&w.body, out),
        Statement::TryCatch(tc) => {
            for s in &tc.try_block.statements {
                collect_from_targets_in_stmt(s, out);
            }
            for s in &tc.catch_block.statements {
                collect_from_targets_in_stmt(s, out);
            }
        }
        // 他の文は FROM 句を持たないか、UPDATE/DELETE の対象は alias 解決に寄与しない
        _ => {}
    }
}

/// `TableReference` から alias/name → table_name の対応を収集する。
fn collect_from_table_ref(tr: &TableReference, out: &mut Vec<(String, String)>) {
    match tr {
        TableReference::Table { name, alias, .. } => {
            let table_name = name.name.clone();
            let key = alias
                .as_ref()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| table_name.clone());
            out.push((key, table_name));
        }
        TableReference::Joined { joins, .. } => {
            for join in joins {
                collect_from_table_ref(&join.table, out);
            }
        }
        TableReference::Subquery { .. } => {}
    }
}

/// ドット修飾子 (`alias.` / `table.`) から対象テーブルを解決し、そのカラムを返す。
///
/// 解決順序:
/// 1. FROM 句のエイリアス/テーブル名 (case-insensitive) から対応表を構築
/// 2. `qualifier` が単一のテーブルに一意に解決できれば、そのカラムを返す
/// 3. 解決できなければ空 (未知の alias)
///
/// あいまいな場合は呼び出し元 (`complete_for_context`) が全テーブルのカラムへ
/// フォールバックする。
fn resolve_columns_for_qualifier<'a>(
    qualifier: &str,
    stmts: &[Statement],
    symbol_table: &'a SymbolTable,
) -> Option<&'a TableSymbol> {
    let targets = collect_from_targets(stmts);
    // qualifier (case-insensitive) に一致するテーブル名を収集。
    let mut resolved: Vec<&str> = Vec::new();
    for (key, table_name) in &targets {
        if key.eq_ignore_ascii_case(qualifier) {
            resolved.push(table_name.as_str());
        }
    }
    // 一意に解決した場合のみ採用 (あいまいなら None → 呼び出し元フォールバック)。
    if resolved.len() == 1 {
        return SymbolTableBuilder::find_table(symbol_table, resolved[0]);
    }
    // FROM 句に無くても、qualifier が実テーブル名と直接一致すればそれを使う。
    SymbolTableBuilder::find_table(symbol_table, qualifier)
}

/// カラムシンボルから補完アイテムを構築する (kind=FIELD, detail=data_type)。
fn column_completion_item(col: &ColumnSymbol) -> CompletionItem {
    CompletionItem {
        label: col.name.clone(),
        kind: Some(CompletionItemKind::FIELD),
        detail: Some(format!("{}", col.data_type)),
        ..CompletionItem::default()
    }
}

/// カーソルコンテキストに応じた補完候補を返す (#126, #132, #54: `config` 駆動)。
///
/// * `Table` → シンボルテーブル内のテーブル名
/// * `VariableDeclaration` → 空 (新規変数名入力中)
/// * `Column` (`<alias>.` / `<table>.`) → FROM 句のエイリアスから対象テーブルを
///   解決し、そのカラム一覧 (kind=FIELD, detail=data_type) を返す。解決できな
///   ければ空。
/// * `Expression` → [`complete_all`] の全候補（`config.enable_snippets` が
///   `false` ならスニペットをプレーンテキストに展開）
///
/// # Arguments
///
/// * `prefix` - 行頭〜カーソル位置までのテキスト
/// * `symbol_table` - 現ドキュメントのシンボルテーブル (テーブル名参照用)
/// * `config` - 補完設定 (スニペット有効/無効)
/// * `statements` - 現ドキュメントの AST (FROM 句エイリアス解決用, #54)
#[must_use]
pub fn complete_for_context(
    prefix: &str,
    symbol_table: &SymbolTable,
    config: &CompletionConfig,
    statements: &[Statement],
) -> CompletionResponse {
    match detect_context(prefix) {
        CompletionContext::VariableDeclaration => CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items: Vec::new(),
        }),
        CompletionContext::Table => {
            let items: Vec<CompletionItem> = symbol_table
                .tables
                .values()
                .map(table_completion_item)
                .collect();
            CompletionResponse::List(CompletionList {
                is_incomplete: items.len() >= *INCOMPLETE_THRESHOLD,
                items,
            })
        }
        CompletionContext::Column => {
            // #54: `<alias>.` / `<table>.` → FROM 句から対象テーブルを解決し、
            // そのカラムを FIELD 補完候補として返す。
            let qualifier = detect_column_trigger(prefix.trim_end()).unwrap_or("");
            let items: Vec<CompletionItem> =
                match resolve_columns_for_qualifier(qualifier, statements, symbol_table) {
                    Some(table) => table.columns.iter().map(column_completion_item).collect(),
                    None => {
                        // 解決不能 (未知の alias) または文脈不足 → 候補なし。
                        Vec::new()
                    }
                };
            CompletionResponse::List(CompletionList {
                is_incomplete: items.len() >= *INCOMPLETE_THRESHOLD,
                items,
            })
        }
        CompletionContext::Expression => {
            // 変数・プロシージャパラメータを静的リストの前に prepend する (#54 Task 4)。
            // `SymbolTableBuilder::find_variable` と同様に @ プレフィックス付き名前を扱う。
            let dynamic = collect_variable_items(symbol_table);
            let dynamic_count = dynamic.len();
            let mut resp = apply_snippet_config(complete_all().clone(), config.enable_snippets);
            if let CompletionResponse::List(ref mut list) = resp {
                // 変数を先頭に挿入（in-scope シンボルをエディタが優先表示するよう）。
                let mut combined = std::mem::take(&mut list.items);
                let mut dynamic = dynamic;
                dynamic.append(&mut combined);
                list.items = dynamic;
                // 閾値判定は「変数 prepend 後」の総件数で行う (#54 Task 5)。
                list.is_incomplete = list.items.len() >= *INCOMPLETE_THRESHOLD;
            }
            let _ = dynamic_count; // 変数件数は将来のフィルタ拡張用に保持
            resp
        }
    }
}

/// Expression コンテキストで prepend すべき変数/パラメータ補完アイテムを収集する。
///
/// `SymbolTableBuilder::find_variable` の @ プレフィックス扱いを踏襲し、
/// 1. トップレベルの `DECLARE` 変数 (`symbol_table.variables`)
/// 2. 全プロシージャのパラメータ (`ProcedureSymbol.parameters`)
/// 3. 全プロシージャボディ内の `DECLARE` 変数 (`ProcedureSymbol.body_variables`)
///
/// これらをマージする。同名（case-insensitive）の変数は最初の出現で重複排除する。
///
/// LSP は現在カーソルがどのプロシージャスコープ内にあるかを判定しないため、
/// ドキュメント内の全プロシージャのパラメータ/ボディ変数を候補に出す
/// （スコープ解決の厳密化は非スコープ・将来 L-XL）。
fn collect_variable_items(symbol_table: &SymbolTable) -> Vec<CompletionItem> {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    let mut items: Vec<CompletionItem> = Vec::new();

    // 1. トップレベル DECLARE 変数
    for var in symbol_table.variables.values() {
        if seen.insert(var.name.to_uppercase()) {
            items.push(variable_completion_item(
                &var.name,
                &var.data_type.to_string(),
                false,
            ));
        }
    }

    // 2 & 3. プロシージャのパラメータ + ボディ変数
    for proc in symbol_table.procedures.values() {
        for param in &proc.parameters {
            if seen.insert(param.name.to_uppercase()) {
                items.push(variable_completion_item(
                    &param.name,
                    &param.data_type.to_string(),
                    param.is_output,
                ));
            }
        }
        for body_var in &proc.body_variables {
            if seen.insert(body_var.name.to_uppercase()) {
                items.push(variable_completion_item(
                    &body_var.name,
                    &body_var.data_type.to_string(),
                    false,
                ));
            }
        }
    }

    items
}

/// 変数/パラメータから補完アイテムを構築する。
///
/// `label` には @ プレフィックス付きの名前をそのまま使用し、
/// `detail` にはデータ型（と OUTPUT マーカー）を含める。
fn variable_completion_item(name: &str, data_type: &str, is_output: bool) -> CompletionItem {
    let detail = if is_output {
        format!("{data_type} (OUTPUT)")
    } else {
        data_type.to_string()
    };
    CompletionItem {
        label: name.to_string(),
        kind: Some(CompletionItemKind::VARIABLE),
        detail: Some(detail),
        ..CompletionItem::default()
    }
}

/// 補完候補リストのスニペット挙動を `enable_snippets` に従って調整する (#132)。
///
/// `enable_snippets == true` なら何もしない（キャッシュ済みリストをそのまま返す・
/// pre-#132 挙動）。`false` の場合、スニペット形式（`InsertTextFormat::SNIPPET`）の
/// 関数補完をプレーンテキスト（`name()`）に展開する。キーワード/型/変数は元々
/// スニペットではないため影響を受けない。
#[must_use]
pub fn apply_snippet_config(resp: CompletionResponse, enable_snippets: bool) -> CompletionResponse {
    if enable_snippets {
        return resp;
    }
    let CompletionResponse::List(list) = resp else {
        return resp;
    };
    let items = list
        .items
        .into_iter()
        .map(|mut item| {
            if item.insert_text_format == Some(InsertTextFormat::SNIPPET) {
                // スニペットを関数名＋括弧のプレーンテキストに置換。
                item.insert_text = Some(format!("{}()", item.label));
                item.insert_text_format = Some(InsertTextFormat::PLAIN_TEXT);
            }
            item
        })
        .collect();
    CompletionResponse::List(CompletionList {
        is_incomplete: list.is_incomplete,
        items,
    })
}

/// テーブルシンボルから補完アイテムを構築する。
fn table_completion_item(t: &TableSymbol) -> CompletionItem {
    CompletionItem {
        label: t.name.clone(),
        kind: Some(CompletionItemKind::STRUCT),
        detail: Some(
            if t.is_temporary {
                "Temporary table"
            } else {
                "Table"
            }
            .to_string(),
        ),
        ..CompletionItem::default()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::CompletionConfig;
    use crate::symbol_table::SymbolTableBuilder;
    use lsp_types::InsertTextFormat;

    #[test]
    fn test_complete_all_has_items() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                assert!(!list.items.is_empty());
                assert!(!list.is_incomplete);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_includes_select() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let has_select = list.items.iter().any(|i| i.label == "SELECT");
                assert!(has_select);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_includes_types() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let has_int = list.items.iter().any(|i| i.label == "INT");
                assert!(has_int);
                let has_varchar = list.items.iter().any(|i| i.label == "VARCHAR");
                assert!(has_varchar);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_includes_functions() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let has_getdate = list.items.iter().any(|i| i.label == "GETDATE");
                assert!(has_getdate);
                let has_convert = list.items.iter().any(|i| i.label == "CONVERT");
                assert!(has_convert);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_function_has_detail() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let getdate = list.items.iter().find(|i| i.label == "GETDATE");
                assert!(getdate.is_some());
                let item = getdate.unwrap();
                assert!(item.detail.is_some());
                assert!(item.insert_text.is_some());
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_function_snippet_format() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let substring = list.items.iter().find(|i| i.label == "SUBSTRING");
                assert!(substring.is_some());
                let item = substring.unwrap();
                assert_eq!(item.insert_text_format, Some(InsertTextFormat::SNIPPET));
                // Should have placeholder syntax
                let insert = item.insert_text.as_ref().unwrap();
                assert!(
                    insert.contains("${1:"),
                    "Expected snippet placeholder, got: {}",
                    insert
                );
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_build_snippet_with_params() {
        let result = build_function_snippet("SUBSTRING", &["expression", "start", "length"]);
        assert_eq!(
            result,
            "SUBSTRING(${1:expression}, ${2:start}, ${3:length})"
        );
    }

    #[test]
    fn test_build_snippet_no_params() {
        let result = build_function_snippet("GETDATE", &[]);
        assert_eq!(result, "GETDATE()");
    }

    #[test]
    fn test_build_snippet_single_param() {
        let result = build_function_snippet("COUNT", &["expression"]);
        assert_eq!(result, "COUNT(${1:expression})");
    }

    #[test]
    fn test_build_snippet_optional_params_clean() {
        // CONVERT has optional "style" param in syntax but params field is clean
        let result = build_function_snippet("CONVERT", &["type", "expression", "style"]);
        assert_eq!(result, "CONVERT(${1:type}, ${2:expression}, ${3:style})");
        assert!(
            !result.contains('['),
            "No brackets should appear in snippet"
        );
    }

    #[test]
    fn test_complete_keywords() {
        let response = complete_keywords();
        match response {
            CompletionResponse::List(list) => {
                assert!(!list.items.is_empty());
                // Should be keywords only
                let all_keywords = list
                    .items
                    .iter()
                    .all(|i| i.kind == Some(CompletionItemKind::KEYWORD));
                assert!(all_keywords);
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_keywords_is_static_ref() {
        let a = complete_keywords() as *const CompletionResponse;
        let b = complete_keywords() as *const CompletionResponse;
        // Same static address — no clone
        assert_eq!(a, b);
    }

    #[test]
    fn test_cast_uses_plain_text() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let cast = list.items.iter().find(|i| i.label == "CAST");
                assert!(cast.is_some());
                let item = cast.unwrap();
                assert_eq!(
                    item.insert_text_format,
                    Some(InsertTextFormat::PLAIN_TEXT),
                    "CAST should use PLAIN_TEXT, not SNIPPET"
                );
                let text = item.insert_text.as_ref().unwrap();
                assert!(
                    text.contains(" AS "),
                    "CAST insert_text should preserve AS syntax, got: {text}"
                );
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_is_comma_separated_syntax() {
        assert!(is_comma_separated_syntax(
            "SUBSTRING(expression, start, length)"
        ));
        assert!(is_comma_separated_syntax("GETDATE()"));
        assert!(!is_comma_separated_syntax("CAST(expression AS type)"));
        assert!(!is_comma_separated_syntax("IDENTITY")); // no parens
        assert!(!is_comma_separated_syntax("OBJECT_ID('object_name')")); // quotes
        assert!(!is_comma_separated_syntax(
            "COUNT([DISTINCT] expression | *)"
        )); // pipe
    }

    #[test]
    fn test_identity_no_empty_parens() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let identity = list.items.iter().find(|i| {
                    i.label == "IDENTITY" && i.kind == Some(CompletionItemKind::FUNCTION)
                });
                assert!(identity.is_some(), "IDENTITY function should exist");
                let item = identity.unwrap();
                assert_eq!(
                    item.insert_text_format,
                    Some(InsertTextFormat::PLAIN_TEXT),
                    "IDENTITY should use PLAIN_TEXT"
                );
                let text = item.insert_text.as_ref().unwrap();
                assert!(
                    !text.ends_with("()"),
                    "IDENTITY should not have empty parens, got: {text}"
                );
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_cache_returns_same_instance() {
        let a = complete_all();
        let b = complete_all();
        match (&a, &b) {
            (CompletionResponse::List(la), CompletionResponse::List(lb)) => {
                assert_eq!(la.items.len(), lb.items.len());
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_complete_keywords_cache_returns_same_count() {
        let a = complete_keywords();
        let b = complete_keywords();
        match (a, b) {
            (CompletionResponse::List(la), CompletionResponse::List(lb)) => {
                assert_eq!(la.items.len(), lb.items.len());
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn test_complete_all_includes_system_variables() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let has_rowcount = list.items.iter().any(|i| {
                    i.label == "@@ROWCOUNT" && i.kind == Some(CompletionItemKind::VARIABLE)
                });
                assert!(has_rowcount, "Should include @@ROWCOUNT system variable");
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_complete_all_no_duplicate_labels() {
        let response = complete_all();
        match response {
            CompletionResponse::List(list) => {
                let mut labels: Vec<&str> = list.items.iter().map(|i| i.label.as_str()).collect();
                labels.sort();
                let deduped: Vec<&str> = labels
                    .windows(2)
                    .filter(|w| w[0] == w[1])
                    .map(|w| w[0])
                    .collect();
                // Keywords may appear as both keyword and function (e.g., SELECT)
                // so allow some duplicates but verify it's not excessive
                assert!(
                    deduped.len() <= 5,
                    "Too many duplicate labels: {:?}",
                    deduped
                );
            }
            _ => panic!("Expected List response"),
        }
    }

    #[test]
    fn test_is_comma_separated_syntax_edge_cases() {
        assert!(!is_comma_separated_syntax(""));
        assert!(is_comma_separated_syntax("()")); // empty parens still match pattern
        assert!(is_comma_separated_syntax("F()")); // single char func
    }

    // ----- #126: context-aware completion ---------------------------------

    #[test]
    fn detect_context_table_after_from() {
        assert_eq!(detect_context("SELECT * FROM "), CompletionContext::Table);
    }

    #[test]
    fn detect_context_table_while_typing_name() {
        // "FROM u" — user started typing the table name
        assert_eq!(detect_context("SELECT * FROM u"), CompletionContext::Table);
    }

    #[test]
    fn detect_context_table_after_join_and_into() {
        assert_eq!(
            detect_context("SELECT * FROM a JOIN "),
            CompletionContext::Table
        );
        assert_eq!(detect_context("INSERT INTO "), CompletionContext::Table);
        assert_eq!(detect_context("UPDATE "), CompletionContext::Table);
    }

    #[test]
    fn detect_context_variable_declaration() {
        assert_eq!(
            detect_context("DECLARE @"),
            CompletionContext::VariableDeclaration
        );
        // while typing the variable name
        assert_eq!(
            detect_context("DECLARE @co"),
            CompletionContext::VariableDeclaration
        );
        // after DECLARE + space, before the '@'
        assert_eq!(
            detect_context("DECLARE "),
            CompletionContext::VariableDeclaration
        );
    }

    #[test]
    fn detect_context_expression_positions() {
        assert_eq!(detect_context("SELECT "), CompletionContext::Expression);
        assert_eq!(
            detect_context("SELECT * FROM users WHERE "),
            CompletionContext::Expression
        );
        assert_eq!(detect_context(""), CompletionContext::Expression);
    }

    #[test]
    fn complete_for_context_returns_table_names() {
        let st = SymbolTableBuilder::build("CREATE TABLE users (id INT)");
        let resp = complete_for_context("SELECT * FROM ", &st, &CompletionConfig::default(), &[]);
        match resp {
            CompletionResponse::List(list) => {
                assert!(
                    list.items.iter().any(|i| i.label == "users"),
                    "FROM context should offer table names"
                );
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn complete_for_context_variable_decl_is_empty() {
        let st = SymbolTableBuilder::build("");
        let resp = complete_for_context("DECLARE @", &st, &CompletionConfig::default(), &[]);
        match resp {
            CompletionResponse::List(list) => {
                assert!(
                    list.items.is_empty(),
                    "Variable declaration should not offer static items"
                );
            }
            _ => panic!("Expected List"),
        }
    }

    #[test]
    fn complete_for_context_expression_returns_full_list() {
        let st = SymbolTableBuilder::build("");
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        match resp {
            CompletionResponse::List(list) => {
                // Expression context returns the full cached list (e.g. SELECT keyword).
                assert!(list.items.iter().any(|i| i.label == "SELECT"));
            }
            _ => panic!("Expected List"),
        }
    }

    // === configuration-driven snippets (#132) ===

    #[test]
    fn config_snippets_disabled_strips_function_placeholders() {
        let st = SymbolTableBuilder::build("");
        let cfg = CompletionConfig {
            enable_snippets: false,
            ..Default::default()
        };
        let resp = complete_for_context("SELECT ", &st, &cfg, &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        // A comma-separated function (e.g. SUBSTRING) must be plain text, not a snippet.
        let substring = list.items.iter().find(|i| i.label == "SUBSTRING");
        let substring =
            substring.unwrap_or_else(|| panic!("SUBSTRING should be in completion list: {list:?}"));
        assert_eq!(
            substring.insert_text_format,
            Some(InsertTextFormat::PLAIN_TEXT),
            "snippets disabled → plain text"
        );
        assert_eq!(substring.insert_text.as_deref(), Some("SUBSTRING()"));
    }

    #[test]
    fn config_snippets_enabled_keeps_default_behaviour() {
        let st = SymbolTableBuilder::build("");
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        let substring = list
            .items
            .iter()
            .find(|i| i.label == "SUBSTRING")
            .expect("SUBSTRING present");
        // Default (enabled) keeps the snippet with placeholders.
        assert_eq!(
            substring.insert_text_format,
            Some(InsertTextFormat::SNIPPET)
        );
        assert!(substring
            .insert_text
            .as_deref()
            .unwrap_or("")
            .contains("${1:"));
    }

    // === is_incomplete threshold (#54 Task 5) ===

    #[test]
    fn incomplete_threshold_is_positive() {
        let threshold = *INCOMPLETE_THRESHOLD;
        // Threshold must be a sensible positive constant (e.g. >100 per the task spec).
        assert!(
            threshold > 0,
            "INCOMPLETE_THRESHOLD must be positive, got {threshold}"
        );
    }

    #[test]
    fn incomplete_threshold_matches_static_complete_all_count() {
        let threshold = *INCOMPLETE_THRESHOLD;
        // The threshold is defined as the count of the static complete_all() result.
        let count = match complete_all() {
            CompletionResponse::List(list) => list.items.len(),
            _ => panic!("Expected List"),
        };
        assert_eq!(
            threshold, count,
            "INCOMPLETE_THRESHOLD should equal complete_all() item count"
        );
    }

    #[test]
    fn expression_context_sets_incomplete_when_exceeds_threshold() {
        let threshold = *INCOMPLETE_THRESHOLD;
        // The Expression branch returns the full static list whose count equals
        // the threshold — exceeding the threshold-1 boundary means the client
        // should re-query on further typing.
        let st = SymbolTableBuilder::build("");
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(
            list.items.len() > threshold.saturating_sub(1),
            "Expression branch should return at least {} items",
            threshold
        );
        assert!(
            list.is_incomplete,
            "Expression context with > {} items must signal is_incomplete=true so clients re-query",
            threshold.saturating_sub(1)
        );
    }

    #[test]
    fn table_context_small_does_not_set_incomplete() {
        let threshold = *INCOMPLETE_THRESHOLD;
        // A small number of tables (below threshold) must NOT set is_incomplete.
        let st = SymbolTableBuilder::build("CREATE TABLE users (id INT)");
        let resp = complete_for_context("SELECT * FROM ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(list.items.len() < threshold);
        assert!(
            !list.is_incomplete,
            "Table context with few items must not set is_incomplete"
        );
    }

    #[test]
    fn table_context_many_tables_sets_incomplete() {
        let threshold = *INCOMPLETE_THRESHOLD;
        // Synthesize a symbol table with more tables than the threshold.
        let mut sql = String::new();
        for i in 0..(threshold + 5) {
            sql.push_str(&format!("CREATE TABLE t{i} (id INT) "));
        }
        let st = SymbolTableBuilder::build(&sql);
        let resp = complete_for_context("SELECT * FROM ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(
            list.items.len() > threshold,
            "Expected more than {threshold} table items"
        );
        assert!(
            list.is_incomplete,
            "Table context exceeding threshold must signal is_incomplete=true"
        );
    }

    #[test]
    fn variable_declaration_context_not_incomplete() {
        // Empty list is below threshold → is_incomplete stays false.
        let st = SymbolTableBuilder::build("");
        let resp = complete_for_context("DECLARE @", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(list.items.is_empty());
        assert!(
            !list.is_incomplete,
            "Empty VariableDeclaration context must not set is_incomplete"
        );
    }

    // ----- #54 Task 4: Variable completion in Expression context -------------

    #[test]
    fn expression_context_prepends_declared_variables() {
        let st = SymbolTableBuilder::build("DECLARE @count INT\nDECLARE @name VARCHAR(50)");
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        let count = list
            .items
            .iter()
            .find(|i| i.label == "@count")
            .expect("declared @count should be offered in Expression context");
        assert_eq!(count.kind, Some(CompletionItemKind::VARIABLE));
        assert!(count.detail.as_deref().unwrap_or("").contains("INT"));
        let name = list
            .items
            .iter()
            .find(|i| i.label == "@name")
            .expect("declared @name should be offered in Expression context");
        assert_eq!(name.kind, Some(CompletionItemKind::VARIABLE));
        assert!(name.detail.as_deref().unwrap_or("").contains("VARCHAR"));
    }

    #[test]
    fn expression_context_prepends_variables_after_at_prefix() {
        let st = SymbolTableBuilder::build("DECLARE @total INT");
        let resp = complete_for_context("SELECT @", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(
            list.items.iter().any(|i| i.label == "@total"),
            "`@` prefix should still surface declared variables"
        );
    }

    #[test]
    fn expression_context_prepends_variables_in_set_value_position() {
        let st = SymbolTableBuilder::build("DECLARE @a INT\nDECLARE @b INT");
        let resp = complete_for_context("SET @a = ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(list.items.iter().any(|i| i.label == "@a"));
        assert!(list.items.iter().any(|i| i.label == "@b"));
    }

    #[test]
    fn expression_context_prepends_procedure_parameters() {
        let st = SymbolTableBuilder::build(
            "CREATE PROCEDURE my_proc @p1 INT, @p2 VARCHAR(20) OUTPUT AS BEGIN SELECT 1 END",
        );
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        let p1 = list
            .items
            .iter()
            .find(|i| i.label == "@p1")
            .expect("procedure parameter @p1 should be offered");
        assert_eq!(p1.kind, Some(CompletionItemKind::VARIABLE));
        assert!(p1.detail.as_deref().unwrap_or("").contains("INT"));
        let p2 = list
            .items
            .iter()
            .find(|i| i.label == "@p2")
            .expect("procedure parameter @p2 should be offered");
        assert_eq!(p2.kind, Some(CompletionItemKind::VARIABLE));
        assert!(p2.detail.as_deref().unwrap_or("").contains("VARCHAR"));
    }

    #[test]
    fn expression_context_prepends_procedure_body_variables() {
        let st = SymbolTableBuilder::build(
            "CREATE PROCEDURE my_proc @p1 INT AS BEGIN DECLARE @local VARCHAR(10) SELECT 1 END",
        );
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(
            list.items.iter().any(|i| i.label == "@local"),
            "procedure body variable @local should be offered"
        );
        assert!(
            list.items.iter().any(|i| i.label == "@p1"),
            "procedure parameter @p1 should still be offered"
        );
    }

    #[test]
    fn expression_context_deduplicates_variable_names() {
        let st = SymbolTableBuilder::build(
            "DECLARE @dup INT\nCREATE PROCEDURE p AS BEGIN DECLARE @dup INT END",
        );
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        let dup_count = list.items.iter().filter(|i| i.label == "@dup").count();
        assert_eq!(
            dup_count, 1,
            "duplicate @dup entries should be collapsed, found {dup_count}"
        );
    }

    #[test]
    fn expression_context_variables_precede_static_items() {
        let st = SymbolTableBuilder::build("DECLARE @first INT");
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        let first_idx = list
            .items
            .iter()
            .position(|i| i.label == "@first")
            .expect("@first should be present");
        let select_idx = list
            .items
            .iter()
            .position(|i| i.label == "SELECT")
            .expect("SELECT keyword should still be present");
        assert!(
            first_idx < select_idx,
            "variable @first (idx {first_idx}) must precede static SELECT (idx {select_idx})"
        );
    }

    #[test]
    fn expression_context_preserves_static_list_alongside_variables() {
        let st = SymbolTableBuilder::build("DECLARE @x INT");
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(list.items.iter().any(|i| i.label == "@x"));
        assert!(list.items.iter().any(|i| i.label == "SELECT"));
        assert!(list.items.iter().any(|i| i.label == "GETDATE"));
        assert!(list.items.iter().any(|i| i.label == "INT"));
    }

    #[test]
    fn expression_context_no_user_variables_yields_plain_static_list() {
        let st = SymbolTableBuilder::build("");
        let resp = complete_for_context("SELECT ", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(
            !list
                .items
                .iter()
                .any(|i| i.kind == Some(CompletionItemKind::VARIABLE)
                    && i.label.starts_with('@')
                    && !i.label.starts_with("@@")),
            "no user variables should be present when none are declared"
        );
        assert!(list.items.iter().any(|i| i.label == "@@ROWCOUNT"));
    }

    #[test]
    fn variable_declaration_context_still_empty_with_variables_present() {
        let st = SymbolTableBuilder::build("DECLARE @count INT");
        let resp = complete_for_context("DECLARE @", &st, &CompletionConfig::default(), &[]);
        let CompletionResponse::List(list) = resp else {
            panic!("Expected List");
        };
        assert!(
            list.items.is_empty(),
            "VariableDeclaration context must stay empty (Task 4 scopes Expression only)"
        );
    }
}
