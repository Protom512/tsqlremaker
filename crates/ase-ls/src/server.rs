//! LSP Server 実装
//!
//! tower-lsp を使用した Language Server のハンドラー実装。

use ase_ls_core::{
    analysis::DocumentAnalysis, code_actions, completion, definition, diagnostics, folding,
    formatting, hover, incremental::apply_content_change, line_index::LineIndex, references,
    rename, semantic_tokens, signature_help, symbols, workspace_symbols,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// SAP ASE Language Server
pub struct AseLanguageServer {
    client: Client,
    documents: Arc<RwLock<DocumentStore>>,
}

/// メモリ上のドキュメント管理
struct DocumentStore {
    /// URI → entry (Arc で共有、リクエストごとのcloneを回避)
    docs: std::collections::HashMap<String, DocumentEntry>,
}

/// ドキュメントの解析結果とメタデータ。
///
/// `version` は正確性とデバウンス/不変条件チェックのためのメタデータ保持であり、
/// AST の再利用ではない。本チケットではバージョンを記録するのみで、
/// 単調増加の強制は行わない（非単調な更新も許容する）。
struct DocumentEntry {
    analysis: Arc<DocumentAnalysis>,
    // インクリメンタル同期 (#128) の後続タスクでデバウンス/不変条件チェックに
    // 使用する。現時点ではメタデータとして記録するのみ（テストから参照）。
    //
    // Note: `version` is written by `upsert()` on every did_open/did_change
    // (including the content-equality short-circuit path) but is not yet READ
    // from production code — only from unit tests. Keeping `#[allow(dead_code)]`
    // avoids a clippy regression; the field becomes load-bearing in a later
    // task (debounce/invariant check) at which point this allow should be
    // removed (see Issue #130 approval conditions).
    #[allow(dead_code)]
    version: i32,
}

impl DocumentStore {
    fn new() -> Self {
        Self {
            docs: std::collections::HashMap::new(),
        }
    }

    /// Insert or replace a document's analysis with the given LSP document version.
    ///
    /// Returns `true` when a fresh `DocumentAnalysis` was built (rebuild), or
    /// `false` when the new text is byte-identical to the existing source — in
    /// that case the existing `Arc<DocumentAnalysis>` is reused as-is and only
    /// the LSP `version` metadata is advanced. This lets `did_change` skip the
    /// expensive full re-parse (Lexer + parse_with_errors + symbol table build)
    /// for keystrokes that produce no net content change (e.g. type-then-delete,
    /// or clients that resend unchanged content on focus events).
    fn upsert(&mut self, uri: &str, text: &str, version: i32) -> bool {
        if let Some(entry) = self.docs.get(uri) {
            if entry.analysis.source == text {
                // Content-equality short-circuit: reuse the existing analysis,
                // only advance the version metadata. No re-parse, no rebuild.
                self.docs.insert(
                    uri.to_string(),
                    DocumentEntry {
                        analysis: Arc::clone(&entry.analysis),
                        version,
                    },
                );
                return false;
            }
        }

        let analysis = Arc::new(DocumentAnalysis::new(text));
        self.docs
            .insert(uri.to_string(), DocumentEntry { analysis, version });
        true
    }

    /// 現在のドキュメントソースを取得。未登録の場合は空文字列。
    /// did_change でレンジパッチ適用前に read lock 下で呼び出す。
    fn get_source(&self, uri: &str) -> String {
        self.docs
            .get(uri)
            .map(|e| e.analysis.source.clone())
            .unwrap_or_default()
    }

    fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
    }
}

impl AseLanguageServer {
    /// Create a new language server instance with the given LSP client.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentStore::new())),
        }
    }

    /// ドキュメントの診断情報をパブリッシュする
    async fn publish_diagnostics_for(&self, uri: &Url) {
        if let Some(analysis) = self.get_analysis(uri).await {
            let diags = diagnostics::diagnose(&analysis);
            self.client
                .publish_diagnostics(uri.clone(), diags, None)
                .await;
        }
    }

    /// URIに対応するDocumentAnalysisを取得する
    async fn get_analysis(&self, uri: &Url) -> Option<Arc<DocumentAnalysis>> {
        let docs = self.documents.read().await;
        docs.docs
            .get(uri.as_str())
            .map(|entry| entry.analysis.clone())
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for AseLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(PositionEncodingKind::UTF8),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        String::from("."),
                        String::from("@"),
                        String::from(" "),
                    ]),
                    all_commit_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    completion_item: None,
                }),
                document_symbol_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec![String::from("("), String::from(",")]),
                    retrigger_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                            legend: semantic_tokens::semantic_tokens_legend(),
                            range: Some(true),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    ),
                ),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                })),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: String::from("ase-ls"),
                version: Some(String::from("0.1.0")),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ASE Language Server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;

        {
            let mut docs = self.documents.write().await;
            let _rebuilt = docs.upsert(uri.as_str(), &text, version);
        }

        self.publish_diagnostics_for(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;

        // インクリメンタル同期 (#128): 現在のソースを read lock で取得し、
        // ロック解放後にレンジパッチを純粋計算で順次適用してから write lock で再構築。
        // 未登録ドキュメントでも空文字列からの適用で正しく初期化される。
        let current_source = {
            let docs = self.documents.read().await;
            docs.get_source(uri.as_str())
        };

        let new_source = params
            .content_changes
            .iter()
            .fold(current_source, |source, change| {
                let index = LineIndex::new(&source);
                apply_content_change(&source, &index, change)
            });

        {
            let mut docs = self.documents.write().await;
            let _rebuilt = docs.upsert(uri.as_str(), &new_source, version);
        }

        self.publish_diagnostics_for(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        {
            let mut docs = self.documents.write().await;
            docs.close(uri.as_str());
        }
        // ドキュメントクローズ時に診断をクリア
        self.client
            .publish_diagnostics(uri.clone(), Vec::new(), None)
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        if let Some(analysis) = self.get_analysis(uri).await {
            // カーソル直前までの行テキストから補完コンテキストを推定 (#126)。
            // LSP position.character は UTF-16 単位だが、ASCII 主体の SQL では
            // 文字数で安全にプレフィックスを取り出せる。
            let line = analysis.get_line(position.line);
            let prefix: String = line.chars().take(position.character as usize).collect();
            Ok(Some(completion::complete_for_context(
                &prefix,
                &analysis.symbol_table,
            )))
        } else {
            Ok(Some(completion::complete_all().clone()))
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = &params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            Ok(symbols::document_symbols_with_analysis(&analysis))
        } else {
            Ok(None)
        }
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = &params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            Ok(Some(folding::folding_ranges_with_analysis(&analysis)))
        } else {
            Ok(Some(Vec::new()))
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = &params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            Ok(Some(semantic_tokens::semantic_tokens_full_with_analysis(
                &analysis,
            )))
        } else {
            Ok(None)
        }
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let uri = &params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            Ok(Some(semantic_tokens::semantic_tokens_range_with_analysis(
                &analysis,
                params.range,
            )))
        } else {
            Ok(None)
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            Ok(hover::hover_with_analysis(
                &analysis,
                params.text_document_position_params.position,
            ))
        } else {
            Ok(None)
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = &params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            let edits = formatting::format(&analysis.source);
            if edits.is_empty() {
                Ok(None)
            } else {
                Ok(Some(edits))
            }
        } else {
            Ok(None)
        }
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = &params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            // 選択範囲のみを整形した単一 TextEdit を返す (#129)。
            Ok(formatting::format_range(&analysis.source, params.range).map(|edit| vec![edit]))
        } else {
            Ok(None)
        }
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = &params.text_document_position_params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            Ok(signature_help::signature_help_with_analysis(
                &analysis,
                params.text_document_position_params.position,
            ))
        } else {
            Ok(None)
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = &params.text_document_position_params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            let ranges = definition::definition_ranges_with_analysis(
                &analysis,
                params.text_document_position_params.position,
            );
            if ranges.is_empty() {
                Ok(None)
            } else {
                let locations: Vec<Location> = ranges
                    .into_iter()
                    .map(|range| Location {
                        uri: uri.clone(),
                        range,
                    })
                    .collect();
                Ok(Some(GotoDefinitionResponse::Array(locations)))
            }
        } else {
            Ok(None)
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = &params.text_document_position.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            let ranges = references::reference_ranges_with_analysis(
                &analysis,
                params.text_document_position.position,
                params.context.include_declaration,
            );
            if ranges.is_empty() {
                Ok(None)
            } else {
                let locations: Vec<Location> = ranges
                    .into_iter()
                    .map(|range| Location {
                        uri: uri.clone(),
                        range,
                    })
                    .collect();
                Ok(Some(locations))
            }
        } else {
            Ok(None)
        }
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = &params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            let actions = code_actions::code_actions_with_analysis(&analysis, params.range, uri);
            if actions.is_empty() {
                Ok(None)
            } else {
                Ok(Some(actions))
            }
        } else {
            Ok(None)
        }
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = &params.text_document_position.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            Ok(rename::rename_with_analysis(
                &analysis,
                params.text_document_position.position,
                &params.new_name,
                uri,
            ))
        } else {
            Ok(None)
        }
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = &params.text_document.uri;
        if let Some(analysis) = self.get_analysis(uri).await {
            Ok(rename::prepare_rename_with_analysis(
                &analysis,
                params.position,
            ))
        } else {
            Ok(None)
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let docs = self.documents.read().await;
        let mut all_symbols = Vec::new();

        for (uri_str, entry) in &docs.docs {
            if let Ok(uri) = Url::parse(uri_str) {
                let symbols = workspace_symbols::workspace_symbols_with_analysis(
                    &entry.analysis,
                    &params.query,
                    &uri,
                );
                all_symbols.extend(symbols);
            }
        }

        if all_symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(all_symbols))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_stores_version_metadata() {
        // First upsert always rebuilds (no prior entry to be equal to). Version
        // is recorded as metadata and the analysis is freshly built.
        let mut store = DocumentStore::new();
        let rebuilt = store.upsert("file:///a.sql", "SELECT 1", 0);

        assert!(rebuilt, "first upsert must report a rebuild");
        let entry = store.docs.get("file:///a.sql").expect("entry should exist");
        assert_eq!(entry.version, 0);
        assert_eq!(entry.analysis.source, "SELECT 1");
    }

    #[test]
    fn test_upsert_overwrite_replaces_version_and_analysis() {
        // Different text forces a real rebuild: a fresh Arc<DocumentAnalysis> is
        // constructed and both version and analysis source are replaced. This is
        // the "real rebuild" path that the short-circuit must NOT suppress.
        let mut store = DocumentStore::new();
        let _ = store.upsert("file:///a.sql", "SELECT 1", 0);
        let before = store
            .docs
            .get("file:///a.sql")
            .expect("entry should exist")
            .analysis
            .clone();

        let rebuilt = store.upsert("file:///a.sql", "CREATE TABLE t (id INT)", 3);

        assert!(
            rebuilt,
            "different text must trigger a real rebuild (no short-circuit)"
        );
        let entry = store.docs.get("file:///a.sql").expect("entry should exist");
        assert_eq!(entry.version, 3);
        assert_eq!(entry.analysis.source, "CREATE TABLE t (id INT)");
        // A new analysis was built, so the Arc handle must differ from the prior
        // one — proving the overwrite truly rebuilt rather than reused.
        assert!(
            !Arc::ptr_eq(&before, &entry.analysis),
            "overwrite with different text must allocate a fresh analysis Arc"
        );
    }

    #[test]
    fn test_version_recorded_but_not_monotonic_enforced() {
        // Per accepted invariant: version is recorded only; non-monotonic updates
        // are tolerated gracefully (debounce/metadata use case, not validation).
        // The short-circuit keys on source equality, NOT version monotonicity,
        // so a non-monotonic version with different text still rebuilds and the
        // out-of-order version (5 -> 2) is recorded without enforcement.
        let mut store = DocumentStore::new();
        let _ = store.upsert("file:///a.sql", "SELECT 1", 5);
        let rebuilt = store.upsert("file:///a.sql", "SELECT 2", 2);

        assert!(
            rebuilt,
            "different text must rebuild regardless of version ordering"
        );
        let entry = store.docs.get("file:///a.sql").expect("entry should exist");
        assert_eq!(entry.version, 2);
    }

    #[test]
    fn test_close_removes_entry() {
        let mut store = DocumentStore::new();
        store.upsert("file:///a.sql", "SELECT 1", 0);
        assert!(store.docs.contains_key("file:///a.sql"));

        store.close("file:///a.sql");
        assert!(!store.docs.contains_key("file:///a.sql"));
    }

    #[test]
    fn test_entry_holds_shared_analysis_arc() {
        // The analysis must remain behind an Arc so request handlers can clone
        // the handle cheaply without rebuilding the analysis.
        let mut store = DocumentStore::new();
        store.upsert("file:///a.sql", "SELECT 1", 0);

        let entry = store.docs.get("file:///a.sql").expect("entry should exist");
        let cloned: Arc<DocumentAnalysis> = entry.analysis.clone();
        assert_eq!(cloned.source, "SELECT 1");
    }

    #[test]
    fn test_symbol_loop_iterates_entries() {
        let mut store = DocumentStore::new();
        store.upsert("file:///a.sql", "CREATE TABLE users (id INT)", 0);
        store.upsert("file:///b.sql", "CREATE TABLE orders (id INT)", 1);

        // Simulate the symbol() loop: iterate entries and read .analysis
        let mut table_count = 0;
        for entry in store.docs.values() {
            // Access the analysis through the entry — proves the loop compiles
            // and yields each stored analysis.
            if entry.analysis.source.contains("CREATE TABLE") {
                table_count += 1;
            }
        }
        assert_eq!(table_count, 2);
    }

    // --- upsert() content-equality short-circuit (#130) ---
    //
    // Issue #130: did_change called upsert() unconditionally, rebuilding the
    // full DocumentAnalysis (Lexer + parse_with_errors + symbol table) on every
    // keystroke even when the net content did not change. The fix is a
    // content-equality short-circuit: when the new text equals the existing
    // source, reuse the existing Arc<DocumentAnalysis> and only advance version.
    //
    // These tests prove the contract via two complementary signals:
    //   1. upsert() returns bool `rebuilt` (public contract).
    //   2. Arc identity (Arc::as_ptr equality) is preserved on no-op rebuild
    //      and changes on real rebuild. Arc::ptr_eq is the canonical way to
    //      observe whether a fresh allocation happened; it asserts the
    //      short-circuit behavior directly rather than an implementation detail.

    #[test]
    fn test_upsert_same_source_returns_false_and_preserves_arc_identity() {
        let mut store = DocumentStore::new();
        store.upsert("file:///a.sql", "SELECT 1", 0);

        // Capture the Arc identity before the second upsert.
        let before = store
            .docs
            .get("file:///a.sql")
            .expect("entry should exist after first upsert");
        let ptr_before = Arc::as_ptr(&before.analysis);

        // Same source, new version — should be a no-op rebuild.
        let rebuilt = store.upsert("file:///a.sql", "SELECT 1", 7);

        assert!(
            !rebuilt,
            "upsert must return false (no rebuild) when source is unchanged"
        );

        let after = store
            .docs
            .get("file:///a.sql")
            .expect("entry should still exist after second upsert");
        let ptr_after = Arc::as_ptr(&after.analysis);

        // The same Arc allocation must be reused — no re-parse happened.
        assert!(
            std::ptr::eq(ptr_before, ptr_after),
            "Arc identity must be preserved when source is unchanged (no-op rebuild)"
        );

        // Version metadata must still advance even though analysis was reused.
        assert_eq!(
            after.version, 7,
            "version must advance to the new value on a no-op rebuild"
        );
    }

    #[test]
    fn test_upsert_different_source_returns_true_and_changes_arc_identity() {
        let mut store = DocumentStore::new();
        store.upsert("file:///a.sql", "SELECT 1", 0);

        let before = store
            .docs
            .get("file:///a.sql")
            .expect("entry should exist after first upsert");
        let ptr_before = Arc::as_ptr(&before.analysis);

        // Different source — a real rebuild must happen.
        let rebuilt = store.upsert("file:///a.sql", "SELECT 2", 1);

        assert!(
            rebuilt,
            "upsert must return true (rebuilt) when source changes"
        );

        let after = store
            .docs
            .get("file:///a.sql")
            .expect("entry should still exist after second upsert");
        let ptr_after = Arc::as_ptr(&after.analysis);

        assert!(
            !std::ptr::eq(ptr_before, ptr_after),
            "Arc identity must change when a real rebuild happens"
        );
        assert_eq!(after.version, 1);
        assert_eq!(after.analysis.source, "SELECT 2");
    }

    #[test]
    fn test_upsert_first_insert_always_rebuilds() {
        // A brand-new document has no prior source to be equal to, so the first
        // upsert must always report a rebuild (returns true).
        let mut store = DocumentStore::new();
        let rebuilt = store.upsert("file:///a.sql", "SELECT 1", 0);
        assert!(rebuilt, "first upsert must always rebuild");
    }

    #[test]
    fn test_upsert_noop_then_change_then_noop_round_trip() {
        // Simulate a keystroke sequence: type a char, then delete it (net no
        // change), then make a real edit. This exercises the short-circuit
        // interleaved with real rebuilds — the UC-1 scenario for #130.
        let mut store = DocumentStore::new();
        let base = "CREATE TABLE users (id INT)";

        assert!(store.upsert("file:///a.sql", base, 0)); // initial build
        assert!(!store.upsert("file:///a.sql", base, 1)); // type-then-delete: noop
        assert!(store.upsert(
            "file:///a.sql",
            "CREATE TABLE users (id INT, name VARCHAR(10))",
            2
        )); // real edit
        assert!(!store.upsert(
            "file:///a.sql",
            "CREATE TABLE users (id INT, name VARCHAR(10))",
            3
        )); // another noop

        let entry = store.docs.get("file:///a.sql").expect("entry exists");
        assert_eq!(entry.version, 3);
        assert!(entry.analysis.source.contains("name VARCHAR(10)"));
    }
}
