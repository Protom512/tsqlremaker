//! LSP Server 実装
//!
//! tower-lsp を使用した Language Server のハンドラー実装。

use ase_ls_core::{
    code_actions, completion, definition, diagnostics, folding, formatting, hover, references,
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
    /// URI → ドキュメントテキスト
    docs: std::collections::HashMap<String, String>,
}

impl DocumentStore {
    fn new() -> Self {
        Self {
            docs: std::collections::HashMap::new(),
        }
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.docs.insert(uri.to_string(), text.to_string());
    }

    fn update(&mut self, uri: &str, text: &str) {
        self.docs.insert(uri.to_string(), text.to_string());
    }

    fn close(&mut self, uri: &str) {
        self.docs.remove(uri);
    }

    fn get(&self, uri: &str) -> Option<&str> {
        self.docs.get(uri).map(String::as_str)
    }
}

impl AseLanguageServer {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(DocumentStore::new())),
        }
    }

    /// ドキュメントの診断情報をパブリッシュする
    async fn publish_diagnostics_for(&self, uri: &Url) {
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(uri.as_str()) {
            let diags = diagnostics::diagnose_source(source);
            self.client
                .publish_diagnostics(uri.clone(), diags, None)
                .await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for AseLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: Some(PositionEncodingKind::UTF8),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        "@".to_string(),
                        " ".to_string(),
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
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
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
                            full: Some(SemanticTokensFullOptions::Delta { delta: Some(true) }),
                        },
                    ),
                ),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                rename_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: "ase-ls".to_string(),
                version: Some("0.1.0".to_string()),
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

        {
            let mut docs = self.documents.write().await;
            docs.open(uri.as_str(), &text);
        }

        self.publish_diagnostics_for(&uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        // FULL sync mode: use the last change
        if let Some(change) = params.content_changes.last() {
            let mut docs = self.documents.write().await;
            docs.update(uri.as_str(), &change.text);
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

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        let response = completion::complete_all();
        Ok(Some(response))
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(params.text_document.uri.as_str()) {
            Ok(symbols::document_symbols(source))
        } else {
            Ok(None)
        }
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(params.text_document.uri.as_str()) {
            Ok(Some(folding::folding_ranges(source)))
        } else {
            Ok(Some(Vec::new()))
        }
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(params.text_document.uri.as_str()) {
            Ok(Some(semantic_tokens::semantic_tokens_full(source)))
        } else {
            Ok(None)
        }
    }

    async fn semantic_tokens_range(
        &self,
        params: SemanticTokensRangeParams,
    ) -> Result<Option<SemanticTokensRangeResult>> {
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(params.text_document.uri.as_str()) {
            let result = semantic_tokens::semantic_tokens_full(source);
            match result {
                SemanticTokensResult::Tokens(tokens) => {
                    Ok(Some(SemanticTokensRangeResult::Tokens(tokens)))
                }
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(
            params
                .text_document_position_params
                .text_document
                .uri
                .as_str(),
        ) {
            Ok(hover::hover(
                source,
                params.text_document_position_params.position,
            ))
        } else {
            Ok(None)
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(params.text_document.uri.as_str()) {
            let edits = formatting::format(source);
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
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(params.text_document.uri.as_str()) {
            let edits = formatting::format(source);
            if edits.is_empty() {
                Ok(None)
            } else {
                Ok(Some(edits))
            }
        } else {
            Ok(None)
        }
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(
            params
                .text_document_position_params
                .text_document
                .uri
                .as_str(),
        ) {
            Ok(signature_help::signature_help(
                source,
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
        let uri = params.text_document_position_params.text_document.uri;
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(uri.as_str()) {
            let ranges = definition::definition_ranges(
                source,
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
        let uri = params.text_document_position.text_document.uri;
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(uri.as_str()) {
            let ranges = references::reference_ranges(
                source,
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
        let uri = params.text_document.uri;
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(uri.as_str()) {
            let actions = code_actions::code_actions(source, params.range, &uri);
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
        let uri = params.text_document_position.text_document.uri;
        let docs = self.documents.read().await;
        if let Some(source) = docs.get(uri.as_str()) {
            Ok(rename::rename(
                source,
                params.text_document_position.position,
                &params.new_name,
                &uri,
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

        for (uri_str, source) in &docs.docs {
            if let Ok(uri) = Url::parse(uri_str) {
                let symbols = workspace_symbols::workspace_symbols(source, &params.query, &uri);
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
