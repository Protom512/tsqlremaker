//! LSP Server 実装
//!
//! tower-lsp を使用した Language Server のハンドラー実装。

use crate::{error_taxonomy, panic_recovery};
use ase_ls_core::{
    analysis::DocumentAnalysis,
    code_actions, code_lens, completion,
    config::Config,
    definition, diagnostics, document_links, folding, formatting, hover,
    incremental::apply_content_change,
    inlay_hints,
    line_index::LineIndex,
    references, rename, semantic_tokens, signature_help,
    symbol_store::{DocumentSource, SymbolStore},
    symbols, workspace_symbols,
};
use serde_json::json;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

/// SAP ASE Language Server.
///
/// # Lock-ordering convention (DocumentStore BEFORE SymbolStore)
///
/// The server owns two `tokio::sync::RwLock`-guarded stores: `documents`
/// (`DocumentStore`) and `symbols` (`SymbolStore`). To avoid deadlock, any
/// handler that takes write locks on **both** MUST acquire the `DocumentStore`
/// write lock **before** the `SymbolStore` write lock. This ordering is enforced
/// by construction inside [`DocumentStores::with_both`] — the only place that
/// holds both write guards at once — so callers cannot invert it by accident.
/// Single-store handlers (`get_analysis`, `symbol`) take only one lock and are
/// unaffected.
pub struct AseLanguageServer {
    client: Client,
    /// The live editor buffers + per-document analysis.
    documents: Arc<RwLock<DocumentStore>>,
    /// Workspace-wide cross-file symbol index (Open/Live + Background entries).
    ///
    /// Kept in sync with `documents` by every `did_open` / `did_change` /
    /// `did_close` handler via [`DocumentStores::with_both`], and by
    /// [`LanguageServer::did_change_watched_files`] for on-disk `*.sql` files
    /// the editor has not opened (tagged `Background`).
    symbols: Arc<RwLock<SymbolStore>>,
    /// Workspace folders supplied by the client in `initialize`.
    ///
    /// Captured (no longer discarded) so background indexing and watched-files
    /// handling can scope `*.sql` discovery. Empty when the server runs in
    /// single-file mode (no `workspace_folders` in [`InitializeParams`]); in
    /// that mode the cross-file store stays empty and every handler gracefully
    /// falls back to its document-local behaviour (condition #6).
    workspace_folders: Arc<RwLock<Vec<WorkspaceFolder>>>,
    /// User configuration received via `workspace/didChangeConfiguration` (#132).
    ///
    /// Drives formatting (indent width, keyword case), diagnostics (`SELECT *`
    /// severity) and completion (snippet emission). Defaults reproduce the
    /// pre-#132 hardcoded behaviour, so an unconfigured server is unchanged.
    /// Read by every relevant handler under a short read lock and cloned out.
    config: Arc<RwLock<Config>>,
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

    /// Snapshot of every known document as `(Url, Arc<DocumentAnalysis>)` pairs.
    ///
    /// Designed for cross-file Find All References (#170): the `references()`
    /// handler needs to scan the tokens of *every* open/indexed document, but
    /// must NOT hold the `DocumentStore` `RwLock` read guard across the
    /// (potentially slow) cross-file scan. This method runs entirely under a
    /// short-lived read lock held by the caller, producing an owned `Vec` the
    /// caller can iterate freely once the guard is dropped.
    ///
    /// The snapshot is cheap: it clones each `Url` key and `Arc::clone`s each
    /// analysis — no analysis is rebuilt and no source `String` is deep-copied.
    /// A malformed URI key (which can only arise from a corrupted store, never
    /// from `upsert` — that stores `uri.as_str()` of an already-parsed `Url`)
    /// is skipped rather than panicking, keeping the lock-acquire path panic-free
    /// per the workspace lint policy.
    ///
    /// The returned `Url`s are re-parsed from the stored `String` keys rather
    /// than cached, because `DocumentStore` keys on `String` (the URI is parsed
    /// at the `did_open` / `did_change` boundary and stored as a string). This
    /// mirrors how `get_source` / `get_analysis` round-trip through `&str`.
    ///
    /// Note: `analyses_snapshot` is consumed by the `references()` handler for
    /// cross-file Find All References (#170). It snapshots every known document
    /// so the handler can scan usages workspace-wide without holding the
    /// `DocumentStore` read guard across the scan.
    fn analyses_snapshot(&self) -> Vec<(Url, Arc<DocumentAnalysis>)> {
        self.docs
            .iter()
            .filter_map(|(uri_str, entry)| {
                Url::parse(uri_str)
                    .ok()
                    .map(|url| (url, Arc::clone(&entry.analysis)))
            })
            .collect()
    }
}

/// Co-owned pair of the live document store and the cross-file symbol index.
///
/// This type exists to make the **lock-ordering convention** (DocumentStore
/// write-lock acquired BEFORE SymbolStore write-lock) a structural invariant
/// rather than caller discipline: the only method that holds both write guards
/// ([`Self::with_both`]) takes them in the prescribed order, so a did_open /
/// did_change / did_close handler cannot deadlock or invert the order.
///
/// `did_open` / `did_change` keep the `SymbolStore` in sync with the live
/// buffer by tagging the freshly-built analysis as `Open` / `Live`
/// (both shadow `Background`). `did_close` removes the live entry; if a
/// `Background` version for the same URI is still known (e.g. produced by a
/// background indexer in a later task), it is re-inserted so the symbol stays
/// addressable workspace-wide, otherwise the URI is fully dropped.
struct DocumentStores {
    documents: Arc<RwLock<DocumentStore>>,
    symbols: Arc<RwLock<SymbolStore>>,
}

impl DocumentStores {
    /// Acquire both write locks in the canonical order (documents then symbols)
    /// and run `f` with mutable access to both. Used by the sync handlers.
    async fn with_both<R>(&self, f: impl FnOnce(&mut DocumentStore, &mut SymbolStore) -> R) -> R {
        // DocumentStore write lock FIRST — see the lock-ordering convention on
        // [`AseLanguageServer`]. Acquiring the SymbolStore lock first would risk
        // deadlock against any future reader that takes them in this order.
        let mut docs = self.documents.write().await;
        let mut syms = self.symbols.write().await;
        f(&mut docs, &mut syms)
    }

    /// Sync handler for `did_open` / `did_change`.
    ///
    /// Rebuilds the document analysis (honouring the content-equality
    /// short-circuit in `DocumentStore::upsert`), then — only when a real
    /// rebuild happened — refreshes the `SymbolStore` entry for `uri` tagged
    /// `Open` (did_open) or `Live` (did_change). No-op rebuilds leave the
    /// symbol index untouched, mirroring the analysis reuse.
    async fn sync_live(&self, uri: &Url, text: &str, version: i32, source: DocumentSource) {
        self.with_both(|docs, syms| {
            let rebuilt = docs.upsert(uri.as_str(), text, version);
            if rebuilt {
                // upsert() with a rebuild always inserts an entry under `uri`,
                // so .get() cannot miss here; reach for get_mut to reborrow the
                // analysis without cloning the Arc.
                if let Some(entry) = docs.docs.get_mut(uri.as_str()) {
                    syms.upsert(uri, &entry.analysis, source);
                }
            }
        })
        .await;
    }

    /// Sync handler for `did_close`.
    ///
    /// Drops the document from `DocumentStore` and removes its live (`Open` /
    /// `Live`) contribution from the `SymbolStore`. If `background_source`
    /// supplies on-disk contents for the same URI (background indexer, a later
    /// task), the `Background`-tagged version is re-inserted so the symbol
    /// remains addressable across the workspace; otherwise the URI is fully
    /// evicted from the symbol index.
    async fn sync_close(&self, uri: &Url, background_source: Option<&str>) {
        self.with_both(|docs, syms| {
            docs.close(uri.as_str());
            syms.close(uri);
            if let Some(src) = background_source {
                let analysis = DocumentAnalysis::new(src);
                syms.upsert(uri, &analysis, DocumentSource::Background);
            }
        })
        .await;
    }

    /// Build a `DocumentStores` from a pair of fresh, empty stores.
    ///
    /// Test-only constructor so unit tests can exercise the sync handlers and
    /// the watched-files incremental update without standing up a live
    /// [`Client`] (which tower-lsp only mints via [`LspService::new`]).
    #[cfg(test)]
    fn for_test() -> Self {
        Self {
            documents: Arc::new(RwLock::new(DocumentStore::new())),
            symbols: Arc::new(RwLock::new(SymbolStore::new())),
        }
    }
}

impl AseLanguageServer {
    /// Create a new language server instance with the given LSP client.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentStore::new())),
            symbols: Arc::new(RwLock::new(SymbolStore::new())),
            workspace_folders: Arc::new(RwLock::new(Vec::new())),
            config: Arc::new(RwLock::new(Config::default())),
        }
    }

    /// The bundled document + symbol stores, used by the sync handlers.
    ///
    /// Both `Arc`s are cheap to clone and share the same underlying locks, so
    /// building this view does not copy any document state.
    fn stores(&self) -> DocumentStores {
        DocumentStores {
            documents: Arc::clone(&self.documents),
            symbols: Arc::clone(&self.symbols),
        }
    }

    /// ドキュメントの診断情報をパブリッシュする
    async fn publish_diagnostics_for(&self, uri: &Url) {
        if let Some(analysis) = self.get_analysis(uri).await {
            // 設定（#132: SELECT * severity 等）を読んで診断に反映。
            let diag_config = self.config.read().await.diagnostics.clone();
            let diags = diagnostics::diagnose(&analysis, &diag_config);
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

    // ---- #139: error visibility + panic recovery helpers ----

    /// Run a feature's pure core computation with panic recovery and
    /// visibility, for handlers whose core fn takes only `&DocumentAnalysis`
    /// (plus `Copy` params captured by `compute`) and returns `Option<R>`
    /// (#139 taxonomy: A-paths stay silent, B1 panic → notify, B2 parse-err →
    /// log).
    ///
    /// `compute` is wrapped in [`AssertUnwindSafe`]: the core fns are pure
    /// (they do not mutate shared state observed by `catch_unwind`) and any
    /// lock a caller captures is tokio's (never poisons), so the blanket
    /// unwind-safety assertion is sound. Handlers that hold a lock guard
    /// across the call MUST scope-and-drop it before this `.await`s (see
    /// `goto_definition` / `references`); this helper itself never holds a
    /// server lock across its `notify_recoverable` await.
    async fn run_feature<R>(
        &self,
        feature: &'static str,
        uri: &Url,
        analysis: &DocumentAnalysis,
        compute: impl FnOnce(&DocumentAnalysis) -> Option<R>,
    ) -> Result<Option<R>> {
        let outcome = panic_recovery::guarded(feature, AssertUnwindSafe(|| compute(analysis)));
        self.feature_outcome(feature, uri, analysis, outcome).await
    }

    /// Map a guarded feature outcome into an LSP response with visibility
    /// (#139). See [`Self::run_feature`] for the taxonomy.
    async fn feature_outcome<R>(
        &self,
        feature: &'static str,
        uri: &Url,
        analysis: &DocumentAnalysis,
        outcome: std::result::Result<Option<R>, panic_recovery::CaughtPanic>,
    ) -> Result<Option<R>> {
        match outcome {
            Ok(Some(value)) => Ok(Some(value)),
            Ok(None) => {
                error_taxonomy::log_parse_errors_if_any(analysis, feature, uri.as_str());
                tracing::debug!(feature, uri = %uri, "feature produced no result");
                Ok(None)
            }
            Err(panic) => {
                self.notify_recoverable(error_taxonomy::from_panic(panic), feature)
                    .await;
                Ok(None)
            }
        }
    }

    /// Show a recoverable-error notification to the user (taxonomy B1).
    ///
    /// Only caught panics notify today; parse errors stay log-only (already
    /// surfaced as diagnostics — double-reporting would spam the status bar).
    async fn notify_recoverable(
        &self,
        cause: error_taxonomy::RecoverableCause,
        feature: &'static str,
    ) {
        self.client
            .show_message(MessageType::WARNING, cause.message(feature))
            .await;
    }

    /// 現在 open している全ドキュメントの URI スナップショットを返す。
    ///
    /// 設定変更 (#132) 後の診断再公開などで、ロックを長く保持せずに全文書へ
    /// アクセスするために使用する。
    async fn open_uris(&self) -> Vec<Url> {
        let docs = self.documents.read().await;
        docs.docs
            .keys()
            .filter_map(|s| Url::parse(s).ok())
            .collect()
    }

    /// 全 open ドキュメントの診断を再公開する（設定変更後の即時反映・#132）。
    ///
    /// 診断 severity はサーバー主導で publish されるため、`did_change_configuration`
    /// 後に呼び出して新しい設定を即座に反映する。フォーマット/補完は各リクエスト
    /// 時に設定を読むため再公開不要。
    async fn refresh_all_diagnostics(&self) {
        for uri in self.open_uris().await {
            self.publish_diagnostics_for(&uri).await;
        }
    }
}

// ---- T5: workspace symbol index wiring (#169) ----
//
// Pure helpers that decouple the watched-files / registration logic from the
// LSP client so they can be unit-tested without a live server. The server's
// `initialized()` calls [`watched_files_registration`] to build the
// `client/register` payload, and `did_change_watched_files()` drives
// [`apply_watched_file_events`] to keep the [`SymbolStore`] in sync with the
// on-disk world — never overwriting an Open/Live document (condition #6).

/// The glob pattern advertised to the client for watched-file registration.
///
/// `*.sql` (simple [`Pattern`] form — `Pattern` is a `String` newtype alias in
/// lsp-types 0.94). The client forwards only SQL file events. Matching on the
/// final path segment is case-insensitive on the client side; here we only
/// emit the literal pattern.
fn sql_watcher_glob() -> GlobPattern {
    GlobPattern::String(String::from("*.sql"))
}

/// Build the [`DidChangeWatchedFilesRegistrationOptions`] sent to the client
/// via `client/register` in [`LanguageServer::initialized`].
fn watched_files_registration_options() -> DidChangeWatchedFilesRegistrationOptions {
    DidChangeWatchedFilesRegistrationOptions {
        watchers: vec![FileSystemWatcher {
            glob_pattern: sql_watcher_glob(),
            // kind None => default WatchKind 7 (Create | Change | Delete).
            kind: None,
        }],
    }
}

/// Build the [`Registration`] for `workspace/didChangeWatchedFiles`.
///
/// The `register_options` carry the `*.sql` watcher; `method` is the
/// notification name the client must re-route to us.
fn watched_files_registration() -> Registration {
    Registration {
        id: String::from("ase-ls-watched-sql"),
        method: String::from("workspace/didChangeWatchedFiles"),
        register_options: Some(json!(watched_files_registration_options())),
    }
}

/// Returns `true` iff `uri`'s path ends with `.sql` (case-insensitive ASCII).
///
/// The `*.sql` glob is a client-side filter, not a guarantee — a misbehaving
/// client may still forward non-SQL events, so the handler re-checks before
/// touching the symbol store.
fn is_sql_uri(uri: &Url) -> bool {
    uri.path()
        .rsplit('/')
        .next()
        .map(|name| {
            name.rsplit_once('.')
                .map(|(_, ext)| ext.eq_ignore_ascii_case("sql"))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

/// Convert a `file://` URI to a local filesystem path, or `None` if the URI
/// is not a file URI or cannot be converted.
///
/// Used by [`LanguageServer::did_change_watched_files`] to map a watched-file
/// event back to the on-disk path it must read. Non-`file:` schemes (untitled
/// buffers, etc.) yield `None` and are silently skipped.
fn uri_to_path(uri: &Url) -> Option<std::path::PathBuf> {
    if uri.scheme() != "file" {
        return None;
    }
    uri.to_file_path().ok()
}

/// Abstracts "is this URI currently Open in the editor?".
///
/// In production this is backed by a read-lock snapshot of the live
/// [`DocumentStore`]; tests supply a fixed answer. The indirection lets
/// [`apply_watched_file_events`] be a pure function of `(store, events,
/// contents, open-set)` and therefore unit-testable without a server.
trait OpenUriPredicate {
    /// Returns `true` if `uri` is Open (live buffer authoritative).
    fn is_open(&self, uri: &Url) -> bool;
}

/// Test/standalone implementation of [`OpenUriPredicate`].
#[cfg(test)]
struct OpenUriChecker {
    open: std::collections::HashSet<String>,
}

#[cfg(test)]
impl OpenUriChecker {
    /// A predicate that reports no URI as open (pure background indexing).
    fn none_open() -> Self {
        Self {
            open: std::collections::HashSet::new(),
        }
    }

    /// A predicate that reports the given URIs as open.
    fn some_open(uris: Vec<String>) -> Self {
        Self {
            open: uris.into_iter().collect(),
        }
    }

    /// Build a predicate from a snapshot of the live `DocumentStore`'s keys.
    async fn from_documents(documents: &Arc<RwLock<DocumentStore>>) -> Self {
        // Async read: the watched-files handler and tests run inside a tokio
        // runtime, so blocking_read would panic. The key set is small and the
        // lock is held only for the snapshot.
        let open: std::collections::HashSet<String> =
            documents.read().await.docs.keys().cloned().collect();
        Self { open }
    }
}

#[cfg(test)]
impl OpenUriPredicate for OpenUriChecker {
    fn is_open(&self, uri: &Url) -> bool {
        self.open.contains(uri.as_str())
    }
}

/// Snapshot the open-URI set from the live [`DocumentStore`].
///
/// Production predicate for [`apply_pre_read_watched_file_events`]: a file
/// counts as Open iff it currently has a live entry in `documents`. Built once
/// per watched-files notification (cheap: clones only the string keys).
struct DocumentOpenSnapshot {
    open: std::collections::HashSet<String>,
}

impl DocumentOpenSnapshot {
    async fn from_documents(documents: &Arc<RwLock<DocumentStore>>) -> Self {
        let open = documents.read().await.docs.keys().cloned().collect();
        Self { open }
    }
}

impl OpenUriPredicate for DocumentOpenSnapshot {
    fn is_open(&self, uri: &Url) -> bool {
        self.open.contains(uri.as_str())
    }
}

/// Apply a batch of watched-file events whose on-disk contents have already
/// been read (the production path: reads happen on a `spawn_blocking` thread,
/// then the `(event, Option<contents>)` pairs are applied here under a single
/// write lock).
///
/// Semantics mirror [`apply_watched_file_events`]: CREATED/CHANGED re-index as
/// `Background` (skipped when the URI is Open), DELETED closes the URI,
/// non-`*.sql` URIs ignored, `None` contents is a silent no-op.
fn apply_pre_read_watched_file_events<P>(
    store: &mut SymbolStore,
    pairs: &[(FileEvent, Option<String>)],
    open: &P,
) where
    P: OpenUriPredicate + ?Sized,
{
    for (event, text) in pairs {
        // Re-filter: the glob is a client hint, not a contract.
        if !is_sql_uri(&event.uri) {
            continue;
        }
        match event.typ {
            FileChangeType::CREATED | FileChangeType::CHANGED => {
                if open.is_open(&event.uri) {
                    continue;
                }
                let Some(text) = text else {
                    continue;
                };
                let analysis = DocumentAnalysis::new(text);
                store.upsert(&event.uri, &analysis, DocumentSource::Background);
            }
            FileChangeType::DELETED => {
                store.close(&event.uri);
            }
            _ => {}
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for AseLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Capture the workspace folders the client advertised (previously
        // discarded via `_: InitializeParams`). They scope background `*.sql`
        // discovery. Absent (single-file mode) => empty vec; the cross-file
        // store then stays empty and every handler falls back to doc-local
        // behaviour (condition #6).
        if let Some(folders) = params.workspace_folders {
            *self.workspace_folders.write().await = folders;
        }
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
                // #117: code lens with deferred resolution. Lenses are returned
                // unresolved (definition ranges only) and the count is filled in
                // lazily by `codeLens/resolve`.
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(true),
                }),
                // #118: inlay hints (DECLARE variable types, EXEC parameter
                // names). MVP returns labels eagerly (resolve_provider = false);
                // lazy resolution is deferred until cross-file procedure lookup
                // is needed (L-XL).
                inlay_hint_provider: Some(OneOf::Right(InlayHintServerCapabilities::Options(
                    InlayHintOptions {
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: None,
                        },
                        resolve_provider: Some(false),
                    },
                ))),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                // #119: document links for SQLCMD `:r` file-include directives.
                // resolve_provider = Some(true) because the target URI needs the
                // owning document URI (the resolve request carries no
                // textDocument); `document_link` returns links with target: None
                // and `document_link/resolve` recovers the target from link.data.
                document_link_provider: Some(DocumentLinkOptions {
                    resolve_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                })),
                // Advertise workspace-folder support so the client sends us
                // `workspace_folders` in `initialize` and lets us register
                // `workspace/didChangeWatchedFiles` dynamically.
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Right(String::from(
                            "workspace/didChangeWorkspaceFolders",
                        ))),
                    }),
                    file_operations: None,
                }),
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

        // Dynamically register workspace/didChangeWatchedFiles with a '*.sql'
        // glob so the client forwards SQL file events. The protocol has no
        // static capability for this — it MUST be registered via
        // client/register (see DidChangeWatchedFilesClientCapabilities docs in
        // lsp-types 0.94). A registration failure (e.g. client lacks dynamic
        // registration) is logged and tolerated: the server still works in
        // single-file mode via the did_open/did_change sync path.
        if let Err(err) = self
            .client
            .register_capability(vec![watched_files_registration()])
            .await
        {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!("could not register watched-files: {err}"),
                )
                .await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        let version = params.text_document.version;

        // DocumentStore と SymbolStore を排他順序（documents → symbols）で同期。
        // Live バッファを Open タグで投入し、Background 版を上書きする。
        self.stores()
            .sync_live(&uri, &text, version, DocumentSource::Open)
            .await;

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

        // apply_content_change で生成された new_source を DocumentStore と
        // SymbolStore の両方へ伝播。Live タグで投入し、Background 版を上書きする。
        // 内容同一 short-circuit 時は SymbolStore も更新しない（analysis 再利用に整合）。
        self.stores()
            .sync_live(&uri, &new_source, version, DocumentSource::Live)
            .await;

        self.publish_diagnostics_for(&uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        // Live 版を削除し、Background 版（workspace 内の on-disk 内容）があれば
        // それを再投入する。現時点では背景インデックス（別タスク）が未実装のため
        // background_source = None で、URI は SymbolStore から完全に削除される。
        // 背景インデックス導入後はここへ on-disk 内容を渡すことで降格が成立する。
        self.stores().sync_close(&uri, None).await;
        // ドキュメントクローズ時に診断をクリア
        self.client
            .publish_diagnostics(uri.clone(), Vec::new(), None)
            .await;
    }

    /// `workspace/didChangeConfiguration` (#132): クライアントからの設定変更を
    /// 受け取り、即座に反映する。
    ///
    /// `params.settings` から [`Config`] を寛容に構築（欠損/不正値はデフォルト）
    /// し、`config` へ書き込んだうえで、診断 severity の変更を即時反映するため
    /// 全 open ドキュメントの診断を再公開する。フォーマット/補完は次回リクエスト
    /// 時に新しい設定が読まれるため追加の通知は不要。
    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        let new_config = Config::from_value(&params.settings);
        {
            let mut cfg = self.config.write().await;
            *cfg = new_config;
        }
        // 設定反映後、診断を即時再公開（severity 変更などをクライアントへ伝播）。
        self.refresh_all_diagnostics().await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        // On-disk `*.sql` changes from the client. We keep the SymbolStore in
        // sync with the background world: CREATED/CHANGED re-index the file
        // (tagged Background), DELETED evicts it. An Open/Live document always
        // wins — its live entry is never overwritten by the disk copy
        // (condition #6).
        //
        // File reads are blocking I/O, so they run on a spawn_blocking thread
        // (T6 / estimate risk #2): the on-disk reads happen off the async
        // executor, then the resulting `(event, Option<contents>)` pairs are
        // applied to the SymbolStore under a single write lock. Non-*.sql
        // events are filtered here too (the glob is a client hint, not a
        // guarantee). Failures (vanished file, non-UTF-8) map to `None` and
        // become silent no-ops.
        let events: Vec<FileEvent> = params
            .changes
            .into_iter()
            .filter(|ev| is_sql_uri(&ev.uri))
            .collect();
        if events.is_empty() {
            return;
        }

        // Snapshot the open-URI set under a read lock, then release it before
        // the blocking reads. This also enforces lock-ordering: we never hold
        // the DocumentStore lock while taking the SymbolStore write lock.
        let open_snapshot = DocumentOpenSnapshot::from_documents(&self.documents).await;

        // Read each event's on-disk contents on a blocking thread (DELETED
        // needs no read). The blocking task returns the (event, text) pairs;
        // a JoinError (task panic / cancellation) is logged and the whole
        // batch is dropped rather than panic the server.
        let read_pairs: Vec<(FileEvent, Option<String>)> =
            match tokio::task::spawn_blocking(move || {
                events
                    .into_iter()
                    .map(|ev| {
                        let text = if ev.typ == FileChangeType::DELETED {
                            None
                        } else {
                            uri_to_path(&ev.uri).and_then(|p| std::fs::read_to_string(&p).ok())
                        };
                        (ev, text)
                    })
                    .collect::<Vec<_>>()
            })
            .await
            {
                Ok(pairs) => pairs,
                Err(err) => {
                    self.client
                        .log_message(
                            MessageType::WARNING,
                            format!("watched-files read task failed: {err}"),
                        )
                        .await;
                    Vec::new()
                }
            };

        // Apply under a single SymbolStore write lock. open_snapshot enforces
        // Open/Live precedence; each pair's pre-read text is applied directly
        // (no further I/O under the lock).
        {
            let mut syms = self.symbols.write().await;
            apply_pre_read_watched_file_events(&mut syms, &read_pairs, &open_snapshot);
        }
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        // 補完設定（#132: スニペット有効/無効）を読む。
        let comp_config = self.config.read().await.completion.clone();
        if let Some(analysis) = self.get_analysis(&uri).await {
            // カーソル直前までの行テキストから補完コンテキストを推定 (#126)。
            // LSP position.character は UTF-16 単位だが、ASCII 主体の SQL では
            // 文字数で安全にプレフィックスを取り出せる。
            let prefix: String = analysis
                .get_line(position.line)
                .chars()
                .take(position.character as usize)
                .collect();
            // #139: pure core call を run_feature でラップ（B1 panic 回復 + B2 parse-err 可視化）。
            // comp_config は Clone して capture する（FnOnce の中で &comp_config を借るため）。
            let comp_config_for_closure = comp_config.clone();
            self.run_feature("completion", &uri, &analysis, move |a| {
                Some(completion::complete_for_context(
                    &prefix,
                    &a.symbol_table,
                    &comp_config_for_closure,
                ))
            })
            .await
        } else {
            Ok(Some(completion::apply_snippet_config(
                completion::complete_all().clone(),
                comp_config.enable_snippets,
            )))
        }
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #139: B1 panic 回復 + B2 parse-err 可視化。document_symbols_with_analysis は
            // 既に Option<DocumentSymbolResponse> を返す（空なら None: §1 A5 empty-by-contract）。
            self.run_feature("document_symbol", &uri, &analysis, |a| {
                symbols::document_symbols_with_analysis(a)
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #139: folding_ranges は Vec を返す。LSP 契約上、空でも空配列（non-null）
            // を返す（クライアントが null を「クリア」扱いするため）。従って空 vec も
            // Some(vec![]) として返し、run_feature はパニック時のみ None（B1 通知）。
            let result = self
                .run_feature("folding_range", &uri, &analysis, |a| {
                    Some(folding::folding_ranges_with_analysis(a))
                })
                .await?;
            Ok(Some(result.unwrap_or_default()))
        } else {
            Ok(Some(Vec::new()))
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #139: semantic_tokens_full は常に SemanticTokensResult を返す（空でも Some）。
            // パニック時のみ None となる（run_feature の B1 パス）。
            self.run_feature("semantic_tokens_full", &uri, &analysis, |a| {
                Some(semantic_tokens::semantic_tokens_full_with_analysis(a))
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let uri = params.text_document.uri;
        let range = params.range;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #139: range 版も full 版と同様に Some(Result) を返す。
            self.run_feature("semantic_tokens_range", &uri, &analysis, move |a| {
                Some(semantic_tokens::semantic_tokens_range_with_analysis(
                    a, range,
                ))
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #139: hover_with_analysis は既に Option<Hover> を返すのでそのまま。
            self.run_feature("hover", &uri, &analysis, move |a| {
                hover::hover_with_analysis(a, position)
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // フォーマット設定（#132: インデント幅・キーワード大小）を読む。
            let fmt_config = self.config.read().await.formatting.clone();
            // #139: format は Vec を返すので空を None・非空を Some に正規化（§1 A5 no-edits）。
            self.run_feature("formatting", &uri, &analysis, move |a| {
                let edits = formatting::format(&a.source, &fmt_config);
                if edits.is_empty() {
                    None
                } else {
                    Some(edits)
                }
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let range = params.range;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // フォーマット設定（#132）を読む。
            let fmt_config = self.config.read().await.formatting.clone();
            // 選択範囲のみを整形した単一 TextEdit を返す (#129)。
            // #139: format_range は Option<TextEdit> を返すので vec! に包んで Some/None。
            self.run_feature("range_formatting", &uri, &analysis, move |a| {
                formatting::format_range(&a.source, range, &fmt_config).map(|edit| vec![edit])
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #139: signature_help_with_analysis は既に Option<SignatureHelp> を返す。
            self.run_feature("signature_help", &uri, &analysis, move |a| {
                signature_help::signature_help_with_analysis(a, position)
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        // DocumentAnalysis が無ければ即座にフォールバック（単一ファイルモード等）。
        let Some(analysis) = self.get_analysis(&uri).await else {
            return Ok(None);
        };
        // SymbolStore を read lock で参照し、cross-file 版 goto definition へ。
        // 文書内定義が優先（definition_locations が current_uri 配下で解決）し、
        // 無ければ背景インデックスの CREATE 定義を返す。ストアが空（単一ファイル
        // モード）でも文書内フォールバックが機能する（条件 #6）。
        //
        // #139: definition_locations は lock guard を借用するため run_feature ではなく
        // panic_recovery::guarded + AssertUnwindSafe で直接ラップし、feature_outcome で
        // 可視化する（設計 §4: sync-call wrapper, lock は tokio のため poison しない）。
        // guard は closure 内でスコープ化し、notify_recoverable の await 前に解放する。
        let outcome = {
            let syms = self.symbols.read().await;
            panic_recovery::guarded(
                "goto_definition",
                AssertUnwindSafe(|| {
                    let locations =
                        definition::definition_locations(&syms, &analysis, &uri, position);
                    if locations.is_empty() {
                        None
                    } else {
                        Some(GotoDefinitionResponse::Array(locations))
                    }
                }),
            )
        };
        self.feature_outcome("goto_definition", &uri, &analysis, outcome)
            .await
    }

    /// Find All References — cross-file for objects, document-local for variables.
    ///
    /// Mirrors [`Self::goto_definition`]: acquire the `DocumentStore` read lock
    /// first (documents-before-symbols ordering — the canonical invariant on
    /// [`AseLanguageServer`]), snapshot every known document's analysis via
    /// [`DocumentStore::analyses_snapshot`], drop the lock, then take the
    /// `SymbolStore` read lock and delegate to
    /// [`references::reference_locations`].
    ///
    /// - **Objects** (table/proc/view/index/trigger): usages are scanned across
    ///   every known document; when `include_declaration` is set the CREATE
    ///   definitions from the store are added. Results span multiple files.
    /// - **Variables** (`@var`): scope is document-local — the pure-fn
    ///   short-circuits to `current_uri` only and never crosses files, even if
    ///   another open file declares a same-named variable (#169 design decision).
    ///
    /// Returns `None` on empty, `Some(Vec<Location>)` otherwise. In single-file
    /// mode (empty store/snapshot) the document-local fallback still works.
    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        // DocumentAnalysis が無ければ即座にフォールバック（単一ファイルモード等）。
        let Some(analysis) = self.get_analysis(&uri).await else {
            return Ok(None);
        };
        // ロック順序（documents-read-before-symbols-read）を遵守するため、
        // 最初に DocumentStore の read lock を取って全文書の解析スナップショットを
        // 取得してからロックを解放し、その後に SymbolStore の read lock を取得する。
        // これにより、クロスファイル参照走査中はどちらのロックも保持しない。
        // goto_definition ハンドラ(server.rs:843) と同じ acquire/drop パターン。
        let docs_snapshot = {
            let docs = self.documents.read().await;
            docs.analyses_snapshot()
        };
        let include_declaration = params.context.include_declaration;
        let position = params.text_document_position.position;
        // #139: reference_locations は SymbolStore guard を借用するため、goto_definition
        // と同様に guarded + AssertUnwindSafe + feature_outcome で直接ラップ（設計 §4）。
        // core fn は pure で tokio lock は poison しないため AssertUnwindSafe は健全。
        let outcome = {
            let syms = self.symbols.read().await;
            panic_recovery::guarded(
                "references",
                AssertUnwindSafe(|| {
                    let locations = references::reference_locations(
                        &syms,
                        &analysis,
                        &uri,
                        position,
                        include_declaration,
                        &docs_snapshot,
                    );
                    if locations.is_empty() {
                        None
                    } else {
                        Some(locations)
                    }
                }),
            )
        };
        self.feature_outcome("references", &uri, &analysis, outcome)
            .await
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let range = params.range;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #139: code_actions は Vec を返すので空を None・非空を Some に正規化（§1 A5）。
            // `uri` は run_feature にも渡すため参照キャプチャ（move しない）。
            self.run_feature("code_action", &uri, &analysis, |a| {
                let actions = code_actions::code_actions_with_analysis(a, range, &uri);
                if actions.is_empty() {
                    None
                } else {
                    Some(actions)
                }
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #117: 未解決レンズ（定義範囲 + data のみ）を返す。command は
            // `codeLens/resolve` で参照数を埋めるまで空。
            // #139: code_lenses は Vec を返す。LSP 契約上、空でも空配列（non-null）
            // を返す（クライアントが null を「クリア」扱いするため）。従って空 vec も
            // Some(vec![]) として返し、run_feature はパニック時のみ None（B1 通知）。
            // `uri` は run_feature にも渡すため参照キャプチャ（move しない）。
            let result = self
                .run_feature("code_lens", &uri, &analysis, |a| {
                    Some(code_lens::code_lenses(a, &uri))
                })
                .await?;
            Ok(Some(result.unwrap_or_default()))
        } else {
            Ok(None)
        }
    }

    async fn code_lens_resolve(&self, params: CodeLens) -> Result<CodeLens> {
        // resolve リクエストは textDocument を持たないため、URI は lens.data から復元。
        //
        // #139: このハンドラは Result<CodeLens>（Option ではない）を返すため run_feature
        // ではラップできない。設計 §2 A6 の resolve-fallthrough（入力レンズをそのまま返す）
        // を尊重しつつ、core 呼び出し(resolve_lens)を guarded で panic 回復する。
        // パニック時は B1 通知を発火し、入力レンズを未解決で返す（A6 と同じ見え方）。
        match code_lens::lens_uri(&params).and_then(|s| Url::parse(&s).ok()) {
            Some(uri) => {
                if let Some(analysis) = self.get_analysis(&uri).await {
                    // Option A: clone `params` at the move site so the fallthrough
                    // below can still return the original lens unresolved when
                    // resolve_lens returns None (malformed data / symbol gone).
                    // CodeLens derives Clone and the clone is cheap (a Range, an
                    // Option<Command>, an Option<serde_json::Value>); the borrow
                    // alternative would require changing the pure-fn signature,
                    // which the CTO approval explicitly disallowed.
                    let outcome = panic_recovery::guarded(
                        "code_lens_resolve",
                        AssertUnwindSafe(|| code_lens::resolve_lens(params.clone(), &analysis)),
                    );
                    match outcome {
                        Ok(Some(resolved)) => return Ok(resolved),
                        Ok(None) => {
                            // A6: 解決不能（data 不正・シンボル消失）。A-path なので silent。
                            // ただし parse_errors があれば B2 として log する（設計 §1 B2）。
                            error_taxonomy::log_parse_errors_if_any(
                                &analysis,
                                "code_lens_resolve",
                                uri.as_str(),
                            );
                        }
                        Err(panic) => {
                            // B1: パニック回復。通知して入力レンズを返す。
                            self.notify_recoverable(
                                error_taxonomy::from_panic(panic),
                                "code_lens_resolve",
                            )
                            .await;
                        }
                    }
                }
                // 解決できない（data 不正・文書未ロード・シンボル消失）場合は
                // 入力レンズをそのまま返し、クライアントは未解決表示とする。
                Ok(params)
            }
            None => Ok(params),
        }
    }

    /// `textDocument/documentLink` (#119): emit clickable links for SQLCMD `:r`
    /// file-include directives. Each link's target is resolved document-relative
    /// against the owning URI and stashed in `data` for `documentLink/resolve`.
    async fn document_link(&self, params: DocumentLinkParams) -> Result<Option<Vec<DocumentLink>>> {
        let uri = params.text_document.uri;
        let Some(analysis) = self.get_analysis(&uri).await else {
            // Unloaded document: nothing to link. Null lets the client clear
            // stale links.
            return Ok(None);
        };
        let cfg = self.config.read().await.document_link.clone();
        // #119/#139: document_links は Vec を返す。LSP 契約上、空ドキュメントでも
        // 空配列（non-null）を返す（クライアントが null を「クリア」扱いするため）。
        // 従って空 vec も Some(vec![]) として返し、run_feature はパニック時のみ
        // None を返す（B1 通知 → Ok(None)）。
        // `uri` は run_feature にも渡すため参照キャプチャ（cfg も親スコープの参照）。
        let result = self
            .run_feature("document_link", &uri, &analysis, |a| {
                Some(document_links::document_links(a, &uri, &cfg))
            })
            .await?;
        Ok(Some(result.unwrap_or_default()))
    }

    /// `documentLink/resolve` (#119): re-establish the target URI from the
    /// payload stashed in the link's `data` field. The resolve request carries
    /// no `textDocument`, so the owning document URI embedded in `data` is used
    /// to fetch the analysis. When the document is no longer loaded (or the
    /// payload is absent/malformed), the input link is returned unchanged so the
    /// client keeps the already-resolved target rather than dropping the link.
    ///
    /// #139: Result<DocumentLink>（Option ではない）を返すため run_feature は使えず、
    /// code_lens_resolve と同様に guarded + 手動 B1/B2 処理で panic 回復する（設計 §2 A6）。
    async fn document_link_resolve(&self, params: DocumentLink) -> Result<DocumentLink> {
        // Recover the owning document URI from the stashed payload to fetch the
        // analysis. resolve_document_link is a pure fn of (&link, &analysis).
        if let Some(uri_str) = document_links::link_uri(&params) {
            if let Ok(uri) = Url::parse(&uri_str) {
                if let Some(analysis) = self.get_analysis(&uri).await {
                    let outcome = panic_recovery::guarded(
                        "document_link_resolve",
                        AssertUnwindSafe(|| {
                            document_links::resolve_document_link(&params, &analysis)
                        }),
                    );
                    match outcome {
                        Ok(Some(resolved)) => return Ok(resolved),
                        Ok(None) => {
                            // A6: 再計算不能（payload 不正等）。A-path だが parse_err が
                            // あれば B2 log（設計 §1 B2）。
                            error_taxonomy::log_parse_errors_if_any(
                                &analysis,
                                "document_link_resolve",
                                uri.as_str(),
                            );
                        }
                        Err(panic) => {
                            // B1: パニック回復。通知して入力リンクを返す。
                            self.notify_recoverable(
                                error_taxonomy::from_panic(panic),
                                "document_link_resolve",
                            )
                            .await;
                        }
                    }
                }
            }
        }
        // Fallback: cannot recompute (doc unloaded / payload gone) — return the
        // link unchanged. document_links already populated `target` at
        // documentLink time, so the client keeps that target.
        Ok(params)
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        // #118: pure-function delegation. The handler is a thin adapter that
        // fetches the analysis, reads the inlay config, and forwards both to
        // `inlay_hints::inlay_hints` — no business logic lives here so the core
        // function stays fully unit-testable without tower-lsp I/O.
        let uri = params.text_document.uri;
        let Some(analysis) = self.get_analysis(&uri).await else {
            // Unloaded document: nothing to annotate. The null result lets the
            // client clear any stale hints.
            return Ok(None);
        };
        let inlay_cfg = self.config.read().await.inlay.clone();
        let range = Some(params.range);
        // #139: inlay_hints は Vec を返す。LSP 契約上、空でも空配列（non-null）を返す
        // （クライアントが null を「クリア」扱いするため）。従って空 vec も Some(vec![])
        // として返し、run_feature はパニック時のみ None（B1 通知）。
        let result = self
            .run_feature("inlay_hint", &uri, &analysis, move |a| {
                Some(inlay_hints::inlay_hints(a, range, &inlay_cfg))
            })
            .await?;
        Ok(Some(result.unwrap_or_default()))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let new_name = params.new_name;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #139: rename_with_analysis は Option<WorkspaceEdit> を返す。
            // `uri` は run_feature にも渡すため参照キャプチャ（new_name も親スコープの参照）。
            self.run_feature("rename", &uri, &analysis, |a| {
                rename::rename_with_analysis(a, position, &new_name, &uri)
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri;
        let position = params.position;
        if let Some(analysis) = self.get_analysis(&uri).await {
            // #139: prepare_rename_with_analysis は Option<PrepareRenameResponse> を返す。
            self.run_feature("prepare_rename", &uri, &analysis, move |a| {
                rename::prepare_rename_with_analysis(a, position)
            })
            .await
        } else {
            Ok(None)
        }
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        // Workspace symbol search now reads from the cross-file SymbolStore,
        // which aggregates Open/Live documents (kept in sync by did_open /
        // did_change) AND background-indexed files (did_change_watched_files).
        // In single-file mode the store still holds the one open document, so
        // symbol search keeps working (condition #6). The empty-query contract
        // (returns None) is preserved by workspace_symbols_with_store.
        //
        // #139: symbol は DocumentAnalysis を使わず SymbolStore のみを参照するため
        // run_feature は使えない。workspace_symbols_with_store を guarded で panic 回復
        // し、feature_outcome 風にマッピングする。但し analysis がないため parse_err
        // ログはスキップし、B1 パニック時のみ通知する（設計 §2: symbol は B2 非対象）。
        let query = params.query;
        let outcome = {
            let syms = self.symbols.read().await;
            panic_recovery::guarded(
                "symbol",
                AssertUnwindSafe(|| {
                    let results = workspace_symbols::workspace_symbols_with_store(&syms, &query);
                    if results.is_empty() {
                        None
                    } else {
                        Some(results)
                    }
                }),
            )
        };
        match outcome {
            Ok(value) => Ok(value),
            Err(panic) => {
                self.notify_recoverable(error_taxonomy::from_panic(panic), "symbol")
                    .await;
                Ok(None)
            }
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

    // --- analyses_snapshot() cross-file document set accessor (#170) ---
    //
    // Find All References (#170) needs to scan the tokens of EVERY known
    // document without holding the DocumentStore RwLock across the (potentially
    // slow) cross-file scan. analyses_snapshot() takes a read-locked snapshot:
    // it clones the Url keys and Arc::clones each analysis (cheap — no analysis
    // rebuild, no deep source clone), returning an owned Vec the caller can
    // iterate freely after the read guard is dropped.

    #[test]
    fn test_analyses_snapshot_empty_store() {
        let store = DocumentStore::new();
        let snapshot = store.analyses_snapshot();
        assert!(snapshot.is_empty(), "empty store must yield empty snapshot");
    }

    #[test]
    fn test_analyses_snapshot_returns_all_entries_as_url_arc_pairs() {
        let mut store = DocumentStore::new();
        store.upsert("file:///a.sql", "CREATE TABLE users (id INT)", 0);
        store.upsert("file:///b.sql", "SELECT * FROM users", 1);

        let snapshot = store.analyses_snapshot();

        // Both documents are present.
        assert_eq!(snapshot.len(), 2);

        // Keys are parsed Urls, not raw Strings.
        let uris: Vec<Url> = snapshot.iter().map(|(u, _)| u.clone()).collect();
        assert!(uris.contains(&url("file:///a.sql")));
        assert!(uris.contains(&url("file:///b.sql")));

        // Each analysis is addressable and shared (Arc) rather than rebuilt.
        let a = snapshot
            .iter()
            .find(|(u, _)| u.as_str() == "file:///a.sql")
            .map(|(_, a)| Arc::clone(a))
            .expect("a.sql analysis should be in snapshot");
        assert_eq!(a.source, "CREATE TABLE users (id INT)");
    }

    #[test]
    fn test_analyses_snapshot_shares_arc_identity_with_store() {
        // The snapshot must Arc::clone the existing analysis rather than
        // rebuild it, so a later reference_locations scan observes the SAME
        // analysis the store holds (no divergence, cheap clone).
        let mut store = DocumentStore::new();
        store.upsert("file:///a.sql", "SELECT 1", 0);

        let stored = store
            .docs
            .get("file:///a.sql")
            .expect("entry should exist")
            .analysis
            .clone();

        let snapshot = store.analyses_snapshot();
        let snap = snapshot
            .iter()
            .find(|(u, _)| u.as_str() == "file:///a.sql")
            .map(|(_, a)| Arc::clone(a))
            .expect("a.sql analysis should be in snapshot");

        assert!(
            Arc::ptr_eq(&stored, &snap),
            "snapshot must share Arc identity with the store (no rebuild)"
        );
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

    // --- T5: workspace symbol index wiring (#169) ---
    //
    // These tests cover the new cross-file infrastructure the server must wire:
    // workspace_folders retention, the *.sql glob registration payload, the
    // watched-files → SymbolStore incremental update with Open/Live precedence
    // over Background, and the cross-file workspace_symbol / goto_definition
    // switches. The pure helpers are unit-tested directly; the precedence rule
    // (condition #6: an Open document must never be overwritten by a Background
    // watched-files event) is asserted via the store.

    fn url(s: &str) -> Url {
        Url::parse(s).expect("valid url in test")
    }

    #[test]
    fn test_sql_glob_pattern_is_wildcard_relative() {
        // Condition: notify/register must advertise a '*.sql' glob so the client
        // only forwards SQL file events. We use the simple Pattern form.
        let pat = sql_watcher_glob();
        match pat {
            GlobPattern::String(p) => {
                assert_eq!(p.as_str(), "*.sql");
            }
            other => panic!("expected String glob, got {other:?}"),
        }
    }

    #[test]
    fn test_registration_options_advertise_sql_watcher() {
        let opts = watched_files_registration_options();
        assert_eq!(opts.watchers.len(), 1, "exactly one *.sql watcher");
        let watcher = &opts.watchers[0];
        assert!(
            matches!(&watcher.glob_pattern, GlobPattern::String(p) if p.as_str() == "*.sql"),
            "watcher glob must be *.sql"
        );
        // kind None => default WatchKind 7 (Create|Change|Delete).
        assert!(
            watcher.kind.is_none(),
            "watcher kind defaults to all events"
        );
    }

    #[test]
    fn test_apply_pre_read_events_indexes_created_file_as_background() {
        // A CREATED event for a SQL file the server has never seen must insert
        // its symbols into the SymbolStore tagged Background.
        let mut syms = SymbolStore::new();
        let pairs = vec![(
            FileEvent {
                uri: url("file:///ws/created.sql"),
                typ: FileChangeType::CREATED,
            },
            Some(String::from("CREATE TABLE new_table (id INT)")),
        )];
        apply_pre_read_watched_file_events(&mut syms, &pairs, &OpenUriChecker::none_open());
        let entries = syms.lookup("new_table");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, DocumentSource::Background);
        assert_eq!(entries[0].uri.as_str(), "file:///ws/created.sql");
    }

    #[test]
    fn test_apply_pre_read_events_changed_event_refreshes_entry() {
        // A CHANGED event re-analyzes on-disk contents and replaces the prior
        // contribution for that URI (idempotent re-index).
        let mut syms = SymbolStore::new();
        let uri = url("file:///ws/changed.sql");
        // Seed a stale Background entry.
        syms.upsert(
            &uri,
            &DocumentAnalysis::new("CREATE TABLE old_table (id INT)"),
            DocumentSource::Background,
        );
        let pairs = vec![(
            FileEvent {
                uri: uri.clone(),
                typ: FileChangeType::CHANGED,
            },
            Some(String::from("CREATE TABLE new_table (id INT)")),
        )];
        apply_pre_read_watched_file_events(&mut syms, &pairs, &OpenUriChecker::none_open());
        assert!(
            syms.lookup("old_table").is_empty(),
            "stale table must be evicted by re-index"
        );
        assert_eq!(syms.lookup("new_table").len(), 1);
    }

    #[test]
    fn test_apply_pre_read_events_deleted_event_evicts_uri() {
        // A DELETED event must remove the URI's contribution from the store.
        let mut syms = SymbolStore::new();
        let uri = url("file:///ws/deleted.sql");
        syms.upsert(
            &uri,
            &DocumentAnalysis::new("CREATE TABLE doomed (id INT)"),
            DocumentSource::Background,
        );
        let pairs = vec![(
            FileEvent {
                uri: uri.clone(),
                typ: FileChangeType::DELETED,
            },
            None,
        )];
        apply_pre_read_watched_file_events(&mut syms, &pairs, &OpenUriChecker::none_open());
        assert!(syms.lookup("doomed").is_empty());
    }

    #[test]
    fn test_apply_pre_read_events_skips_open_documents() {
        // Condition #6: when a file is Open in the editor, a watched-files
        // notification must NOT overwrite the live version. The CREATED/CHANGED
        // event is ignored entirely for open URIs.
        let mut syms = SymbolStore::new();
        let open_uri = url("file:///ws/open.sql");
        // Live Open entry — authoritative.
        syms.upsert(
            &open_uri,
            &DocumentAnalysis::new("CREATE TABLE live_table (id INT)"),
            DocumentSource::Open,
        );
        // The watched-files contents would introduce a DIFFERENT table.
        let pairs = vec![(
            FileEvent {
                uri: open_uri.clone(),
                typ: FileChangeType::CHANGED,
            },
            Some(String::from("CREATE TABLE disk_only_table (id INT)")),
        )];
        let checker = OpenUriChecker::some_open(vec![open_uri.as_str().to_string()]);
        apply_pre_read_watched_file_events(&mut syms, &pairs, &checker);
        // Live table intact; disk table NOT introduced.
        assert_eq!(syms.lookup("live_table").len(), 1);
        assert!(
            syms.lookup("disk_only_table").is_empty(),
            "watched-files must not overwrite an Open document"
        );
    }

    #[test]
    fn test_apply_pre_read_events_missing_contents_is_noop_for_create() {
        // If the read fails (race: file vanished), CREATED/CHANGED must not
        // panic and must leave the store untouched.
        let mut syms = SymbolStore::new();
        let pairs = vec![(
            FileEvent {
                uri: url("file:///ws/ghost.sql"),
                typ: FileChangeType::CREATED,
            },
            None,
        )];
        apply_pre_read_watched_file_events(&mut syms, &pairs, &OpenUriChecker::none_open());
        assert!(syms.is_empty());
    }

    #[test]
    fn test_apply_pre_read_events_non_sql_uri_is_ignored() {
        // Non-SQL URIs (e.g. .txt) must be ignored even if the client forwards
        // them — the glob is a hint, not a guarantee.
        let mut syms = SymbolStore::new();
        let pairs = vec![(
            FileEvent {
                uri: url("file:///ws/notes.txt"),
                typ: FileChangeType::CREATED,
            },
            Some(String::from("CREATE TABLE should_not_index (id INT)")),
        )];
        apply_pre_read_watched_file_events(&mut syms, &pairs, &OpenUriChecker::none_open());
        assert!(syms.is_empty(), "non-*.sql events must be ignored");
    }

    #[test]
    fn test_workspace_symbol_via_store_includes_background_entries() {
        // The symbol() handler must read from the SymbolStore, so background
        // (cross-file) tables are discoverable even when no document is open.
        let mut syms = SymbolStore::new();
        let uri = url("file:///ws/schema.sql");
        syms.upsert(
            &uri,
            &DocumentAnalysis::new("CREATE PROCEDURE compute_total AS BEGIN SELECT 1 END"),
            DocumentSource::Background,
        );
        let results = workspace_symbols::workspace_symbols_with_store(&syms, "compute");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "compute_total");
        assert_eq!(results[0].location.uri, uri);
    }

    #[tokio::test]
    async fn test_stores_sync_live_then_watch_close_restores_background() {
        // Integration of the precedence lifecycle:
        //   did_open (Open) -> watched CHANGED ignored -> did_close evicts live
        // This proves the Open/Live precedence helper used by the watched-files
        // handler and the sync handlers compose correctly. Built from raw Arcs
        // so the test does not need a live LSP Client.
        let stores = DocumentStores::for_test();
        let uri = url("file:///ws/x.sql");
        stores
            .sync_live(&uri, "CREATE TABLE t (id INT)", 0, DocumentSource::Open)
            .await;
        // While open, a watched CHANGED event must not overwrite.
        let pairs = vec![(
            FileEvent {
                uri: uri.clone(),
                typ: FileChangeType::CHANGED,
            },
            Some(String::from("CREATE TABLE disk_version (id INT)")),
        )];
        {
            let checker = OpenUriChecker::from_documents(&stores.documents).await;
            let mut syms = stores.symbols.write().await;
            apply_pre_read_watched_file_events(&mut syms, &pairs, &checker);
        }
        let syms = stores.symbols.read().await;
        assert_eq!(syms.lookup("t").len(), 1, "live entry preserved while open");
        assert!(
            syms.lookup("disk_version").is_empty(),
            "disk version must not shadow live"
        );
        drop(syms);
        // did_close fully evicts (no background source wired yet).
        stores.sync_close(&uri, None).await;
        let syms = stores.symbols.read().await;
        assert!(syms.lookup("t").is_empty(), "close evicts the live entry");
    }
}
