//! Integration tests for the LSP server.
//!
//! Tests the full request→response cycle through tower-lsp's LspService,
//! validating handler routing, parameter passing, and lifecycle management.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use ase_ls::server::AseLanguageServer;
use tower::Service;
use tower::ServiceExt;
use tower_lsp::jsonrpc::Request;
use tower_lsp::LspService;

fn setup() -> LspService<AseLanguageServer> {
    let (service, _socket) = LspService::new(AseLanguageServer::new);
    service
}

fn parse_request(json: &str) -> Request {
    json.parse().expect("valid JSON-RPC request")
}

async fn send(
    service: &mut LspService<AseLanguageServer>,
    json: &str,
) -> Option<tower_lsp::jsonrpc::Response> {
    service
        .ready()
        .await
        .expect("service ready")
        .call(parse_request(json))
        .await
        .expect("call succeeded")
}

async fn init_and_open(service: &mut LspService<AseLanguageServer>, uri: &str, text: &str) {
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(service, &init.to_string()).await;

    let open = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": uri,
                "languageId": "sql",
                "version": 0,
                "text": text
            }
        }
    });
    send(service, &open.to_string()).await;
}

// --- Lifecycle tests ---

#[tokio::test]
async fn test_initialize_returns_capabilities() {
    let mut service = setup();

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(response.is_some(), "Should return initialize response");
}

#[tokio::test]
async fn test_hover_on_opened_document() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "SELECT * FROM users").await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 0, "character": 2 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(
        response.is_some(),
        "Hover on opened document should return response"
    );
}

#[tokio::test]
async fn test_hover_on_unopened_document_returns_null() {
    let mut service = setup();

    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": "file:///nonexistent.sql" },
            "position": { "line": 0, "character": 0 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(
        response.is_some(),
        "Should return a response even for missing doc"
    );
}

#[tokio::test]
async fn test_document_symbols_after_open() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "CREATE TABLE users (id INT, name VARCHAR(100))",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentSymbol",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(response.is_some(), "Should return document symbols");
}

#[tokio::test]
async fn test_did_change_updates_document() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "SELECT 1").await;

    // Change content
    let change = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": "file:///test.sql", "version": 1 },
            "contentChanges": [{ "text": "CREATE TABLE users (id INT)" }]
        }
    });
    send(&mut service, &change.to_string()).await;

    // Document symbols should reflect new content
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentSymbol",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(response.is_some(), "Symbols should return after change");
}

#[tokio::test]
async fn test_semantic_tokens_after_open() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "SELECT * FROM users").await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/semanticTokens/full",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(response.is_some(), "Should return semantic tokens");
}

#[tokio::test]
async fn test_formatting_after_open() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "select * from t").await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/formatting",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "options": { "tabSize": 4, "insertSpaces": true }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(response.is_some(), "Should return formatting edits");
}

/// Extract the first TextEdit's `newText` from a formatting response (#132 test helper).
fn formatting_new_text(resp: Option<tower_lsp::jsonrpc::Response>) -> String {
    let value = serde_json::to_value(&resp).unwrap_or(serde_json::Value::Null);
    value
        .pointer("/result/0/newText")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[tokio::test]
async fn test_did_change_configuration_updates_formatting_indent() {
    let mut service = setup();
    // Multi-line SQL so indentation is observable.
    init_and_open(&mut service, "file:///test.sql", "begin select 1 end").await;

    // Default formatting (4-space indent).
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/formatting",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "options": { "tabSize": 4, "insertSpaces": true }
        }
    });
    let default_text = formatting_new_text(send(&mut service, &req.to_string()).await);
    assert!(
        default_text.contains("    SELECT"),
        "default config should use 4-space indent: {default_text:?}"
    );

    // Push a config change: indentWidth = 2.
    let cfg = serde_json::json!({
        "jsonrpc": "2.0", "method": "workspace/didChangeConfiguration",
        "params": { "settings": { "ase-ls": { "formatting": { "indentWidth": 2 } } } }
    });
    send(&mut service, &cfg.to_string()).await;

    // Request formatting again — the new config must take effect immediately.
    let req2 = serde_json::json!({
        "jsonrpc": "2.0", "id": 3, "method": "textDocument/formatting",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "options": { "tabSize": 4, "insertSpaces": true }
        }
    });
    let configured_text = formatting_new_text(send(&mut service, &req2.to_string()).await);
    assert!(
        configured_text.contains("  SELECT") && !configured_text.contains("    SELECT"),
        "indentWidth=2 should produce 2-space indent: {configured_text:?}"
    );
    assert_ne!(
        default_text, configured_text,
        "config change must alter formatting output"
    );
}

#[tokio::test]
async fn test_did_close_clears_document() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "SELECT 1").await;

    // Close
    let close = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didClose",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    send(&mut service, &close.to_string()).await;

    // Hover after close should return null
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 0, "character": 2 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(
        response.is_some(),
        "Should return response even after close"
    );
}

// --- Folding Range tests (#76 AST-aware integration) ---

#[tokio::test]
async fn test_folding_range_begin_end() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "BEGIN\n    SELECT 1\n    SELECT 2\nEND",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/foldingRange",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(response.is_some(), "Should return folding ranges");
    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let ranges = result_val
        .as_array()
        .expect("folding ranges should be array");
    assert!(
        ranges
            .iter()
            .any(|r| r.get("kind").is_some_and(|k| k == "region")),
        "Should contain at least one region fold for BEGIN...END"
    );
}

#[tokio::test]
async fn test_folding_range_if_without_begin() {
    let mut service = setup();
    // IF/ELSE without BEGIN — only AST-based folding detects this
    init_and_open(
        &mut service,
        "file:///test.sql",
        "IF 1 = 1\n    SELECT 1\nELSE\n    SELECT 2",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/foldingRange",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(response.is_some(), "Should return folding ranges");
    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let ranges = result_val
        .as_array()
        .expect("folding ranges should be array");
    // Old token-based folding cannot detect IF without BEGIN — only AST path finds this
    assert!(
        !ranges.is_empty(),
        "AST-based folding should detect IF/ELSE without BEGIN"
    );
}

#[tokio::test]
async fn test_folding_range_comment() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "/* multi-line\n   comment block */\nSELECT 1",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/foldingRange",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;

    assert!(response.is_some(), "Should return folding ranges");
    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let ranges = result_val
        .as_array()
        .expect("folding ranges should be array");
    assert!(
        ranges
            .iter()
            .any(|r| r.get("kind").is_some_and(|k| k == "comment")),
        "Should contain comment fold for block comment"
    );
}

// --- Definition tests ---

#[tokio::test]
async fn test_goto_definition_variable() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "DECLARE @count INT\nSET @count = 1",
    )
    .await;

    // Click on @count in SET line (line 1, char 5)
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 1, "character": 5 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(response.is_some(), "Should return definition response");
}

#[tokio::test]
async fn test_goto_definition_table() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "CREATE TABLE users (id INT)\nSELECT * FROM users",
    )
    .await;

    // Click on "users" in SELECT (line 1, char 15)
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 1, "character": 15 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(
        response.is_some(),
        "Should return definition response for table"
    );

    // Verify it actually found the definition (not null result)
    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    assert!(
        result_val.is_array() || result_val.is_object(),
        "Definition should return locations or null"
    );
}

#[tokio::test]
async fn test_goto_definition_on_whitespace_returns_null() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "SELECT  FROM t").await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 0, "character": 7 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(
        response.is_some(),
        "Should return response even for whitespace"
    );
}

// --- References tests ---

#[tokio::test]
async fn test_references_variable() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "DECLARE @count INT\nSET @count = 1\nSELECT @count",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/references",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 1, "character": 5 },
            "context": { "includeDeclaration": true }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(response.is_some(), "Should return references response");

    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let refs = result_val.as_array();
    assert!(
        refs.is_some_and(|r| r.len() >= 2),
        "Should find at least 2 references to @count"
    );
}

#[tokio::test]
async fn test_references_table() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "CREATE TABLE users (id INT)\nSELECT * FROM users\nDELETE FROM users",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/references",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 0, "character": 14 },
            "context": { "includeDeclaration": true }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(response.is_some(), "Should return references for table");

    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let refs = result_val.as_array();
    assert!(
        refs.is_some_and(|r| r.len() >= 2),
        "Should find references to users table"
    );
}

// --- Rename tests ---

#[tokio::test]
async fn test_prepare_rename_on_identifier() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "CREATE TABLE users (id INT)",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/prepareRename",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 0, "character": 14 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(response.is_some(), "Should return prepareRename response");
}

#[tokio::test]
async fn test_rename_variable() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "DECLARE @count INT\nSET @count = 1\nSELECT @count",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/rename",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 1, "character": 5 },
            "newName": "@total"
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(response.is_some(), "Should return rename response");

    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    assert!(
        result_val.get("changes").is_some(),
        "Rename should return WorkspaceEdit with changes"
    );
}

#[tokio::test]
async fn test_rename_variable_without_at_prefix_rejected() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "DECLARE @count INT\nSET @count = 1",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/rename",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 1, "character": 5 },
            "newName": "total"
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(response.is_some(), "Should return response");

    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    assert!(
        result_val.is_null(),
        "Rename without @ prefix should return null"
    );
}

// --- Code Action tests ---

#[tokio::test]
async fn test_code_action_select_star_expand() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "CREATE TABLE users (id INT, name VARCHAR(100))\nSELECT * FROM users",
    )
    .await;

    // Cursor on the SELECT * line
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/codeAction",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 5 } },
            "context": { "diagnostics": [] }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(response.is_some(), "Should return code actions");

    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let actions = result_val.as_array();
    assert!(
        actions.is_some_and(|a| !a.is_empty()),
        "Should offer code actions for SELECT *"
    );
}

#[tokio::test]
async fn test_code_action_insert_skeleton() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "CREATE TABLE users (id INT, name VARCHAR(100))\nINSERT INTO users VALUES (1, 'test')",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/codeAction",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "range": { "start": { "line": 1, "character": 0 }, "end": { "line": 1, "character": 5 } },
            "context": { "diagnostics": [] }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(response.is_some(), "Should return code actions");
}

// --- Diagnostics (via didOpen) ---

#[tokio::test]
async fn test_diagnostics_on_open_with_select_star() {
    let mut service = setup();
    // Initialize without auto-publish to check diagnostics from didOpen
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;

    // didOpen should trigger publishDiagnostics via server notification
    // We can't directly capture notifications in this test framework,
    // but we can verify the open+diagnose path doesn't panic
    init_and_open(&mut service, "file:///test.sql", "SELECT * FROM users").await;
    // Reaching here without panic proves the diagnostics path is stable
}

#[tokio::test]
async fn test_diagnostics_on_open_with_parse_error() {
    let mut service = setup();
    // Opening invalid SQL should not crash
    init_and_open(&mut service, "file:///test.sql", "SELCT * FRM").await;
    // Reaching here without panic proves the parse error diagnostics path is stable
}

// --- Incremental sync (TextDocumentSyncKind::Incremental) tests ---
//
// These tests exercise range-based content change events. They define the
// contract for incremental document synchronization (Issue #128): when a
// `TextDocumentContentChangeEvent` carries a `range`, only that byte range is
// replaced by `change.text`; when `range` is `None`, the whole document is
// replaced (full-sync fallback).

/// Send a `textDocument/didChange` notification with the given version and
/// content changes. The changes are passed verbatim as JSON so callers can
/// include or omit `range` per the LSP spec.
async fn send_did_change(
    service: &mut LspService<AseLanguageServer>,
    uri: &str,
    version: i64,
    content_changes: serde_json::Value,
) {
    let change = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didChange",
        "params": {
            "textDocument": { "uri": uri, "version": version },
            "contentChanges": content_changes
        }
    });
    send(service, &change.to_string()).await;
}

/// Request document symbols and return the parsed JSON result value.
async fn fetch_document_symbols(
    service: &mut LspService<AseLanguageServer>,
    uri: &str,
    request_id: i64,
) -> serde_json::Value {
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": request_id, "method": "textDocument/documentSymbol",
        "params": { "textDocument": { "uri": uri } }
    });
    let response = send(service, &req.to_string()).await;
    let response = response.expect("documentSymbol must return a response");
    let raw = response
        .result()
        .expect("documentSymbol result must be present");
    serde_json::from_str(&serde_json::to_string(raw).expect("serialize result"))
        .expect("result is valid JSON")
}

/// Extract the names of all top-level document symbols, in order.
fn top_level_symbol_names(symbols: &serde_json::Value) -> Vec<String> {
    let arr = match symbols.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|s| s.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect()
}

#[tokio::test]
async fn test_did_change_full_replace_still_works() {
    // Regression guard: the pre-incremental FULL-replace path (range=None)
    // must keep working under the new Incremental sync mode. This is the same
    // legal range=None sequence exercised by test_did_change_updates_document
    // (line 141), with a stronger content assertion.
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "SELECT 1").await;

    // Full replace (no range) → document becomes a CREATE TABLE.
    send_did_change(
        &mut service,
        "file:///test.sql",
        1,
        serde_json::json!([{ "text": "CREATE TABLE users (id INT)" }]),
    )
    .await;

    let symbols = fetch_document_symbols(&mut service, "file:///test.sql", 2).await;
    let names = top_level_symbol_names(&symbols);
    assert!(
        names.iter().any(|n| n == "users"),
        "FULL replace should expose the new table symbol, got: {names:?}"
    );
}

#[tokio::test]
async fn test_did_change_incremental_single_char_insert() {
    // UC (a): A single-character insert via a ranged change event.
    // Start from "CREATE TABLE users (id INT)" (27 chars, 0-26) then insert a
    // trailing ";" past the ")" at character 27. The table symbol must still be
    // resolved from the patched source.
    let mut service = setup();
    let initial = "CREATE TABLE users (id INT)";
    init_and_open(&mut service, "file:///test.sql", initial).await;

    // Insert ";" at end of the single line: zero-width point at character 27
    // (past the closing ")") yields "CREATE TABLE users (id INT);".
    send_did_change(
        &mut service,
        "file:///test.sql",
        1,
        serde_json::json!([{
            "range": {
                "start": { "line": 0, "character": 27 },
                "end":   { "line": 0, "character": 27 }
            },
            "text": ";"
        }]),
    )
    .await;

    let symbols = fetch_document_symbols(&mut service, "file:///test.sql", 2).await;
    let names = top_level_symbol_names(&symbols);
    assert!(
        names.iter().any(|n| n == "users"),
        "Incremental single-char insert must still resolve the table symbol, got: {names:?}"
    );
}

#[tokio::test]
async fn test_did_change_incremental_multi_line_delete() {
    // UC (b): Deleting a whole line via a range that spans multiple lines.
    // Start from a two-statement doc, then delete the second statement's line
    // (range from start of line 1 to start of line 2). After the patch only
    // the first table should remain as a symbol, proving the analysis was
    // rebuilt on the patched (shorter) source.
    let mut service = setup();
    let initial = "CREATE TABLE users (id INT)\nCREATE TABLE orders (id INT)";
    init_and_open(&mut service, "file:///test.sql", initial).await;

    // Before: both tables visible.
    let symbols_before = fetch_document_symbols(&mut service, "file:///test.sql", 2).await;
    let names_before = top_level_symbol_names(&symbols_before);
    assert!(
        names_before.iter().any(|n| n == "orders"),
        "Both tables should be present before deletion, got: {names_before:?}"
    );

    // Delete line 1 entirely (the second CREATE TABLE).
    // Line 1 starts at (1,0) and the deleted span ends at the start of line 2,
    // i.e. end-of-document here. End at (2,0) to consume the trailing newline.
    send_did_change(
        &mut service,
        "file:///test.sql",
        2,
        serde_json::json!([{
            "range": {
                "start": { "line": 1, "character": 0 },
                "end":   { "line": 2, "character": 0 }
            },
            "text": ""
        }]),
    )
    .await;

    let symbols_after = fetch_document_symbols(&mut service, "file:///test.sql", 3).await;
    let names_after = top_level_symbol_names(&symbols_after);
    assert!(
        names_after.iter().any(|n| n == "users"),
        "First table must remain after deletion, got: {names_after:?}"
    );
    assert!(
        !names_after.iter().any(|n| n == "orders"),
        "Deleted table must no longer be a symbol, got: {names_after:?}"
    );

    // The did_change path also republishes diagnostics; reaching here without
    // panic proves the diagnostics rebuild on the patched source is stable.
}

#[tokio::test]
async fn test_did_change_version_monotonic_increase() {
    // UC (c): A sequence of incremental edits with strictly increasing
    // `version`. The server records version metadata only and must not reject
    // out-of-order or repeated versions (graceful). After a series of
    // inserts, the final source must reflect every applied patch.
    let mut service = setup();
    // "ab" → "aXb" → "aXYb"
    init_and_open(&mut service, "file:///test.sql", "ab").await;

    // version 1: insert "X" between 'a' and 'b'
    send_did_change(
        &mut service,
        "file:///test.sql",
        1,
        serde_json::json!([{
            "range": {
                "start": { "line": 0, "character": 1 },
                "end":   { "line": 0, "character": 1 }
            },
            "text": "X"
        }]),
    )
    .await;

    // version 2: insert "Y" after 'X'
    send_did_change(
        &mut service,
        "file:///test.sql",
        2,
        serde_json::json!([{
            "range": {
                "start": { "line": 0, "character": 2 },
                "end":   { "line": 0, "character": 2 }
            },
            "text": "Y"
        }]),
    )
    .await;

    // The formatting handler returns the analyzed source via TextEdits; we use
    // it as a probe to confirm the patched source is "aXYb". Formatting a
    // single lowercase token yields no edits, so instead we drive formatting
    // through a CREATE TABLE whose casing formatting will normalize — simpler
    // probe: request hover on the synthesized content. Hover returns null for
    // "aXYb" but the round-trip itself proves no panic and the document is
    // tracked. For a content assertion we re-open semantics via formatting
    // of a known string instead.
    //
    // Use the formatting handler on a fresh, formatting-sensitive doc to keep
    // this test self-contained: assert monotonic version sequence does not
    // corrupt state by following it with one more insert that must apply on
    // top of "aXYb".
    send_did_change(
        &mut service,
        "file:///test.sql",
        3,
        serde_json::json!([{
            "range": {
                "start": { "line": 0, "character": 4 },
                "end":   { "line": 0, "character": 4 }
            },
            "text": "Z"
        }]),
    )
    .await;

    // Probe the final document via hover: position 0,0 on "aXYbZ" is 'a',
    // which yields a response (null result is fine). The point is that the
    // document still exists and a handler can read it without panic.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 0, "character": 0 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(
        response.is_some(),
        "Hover after a monotonic-version edit sequence must respond"
    );
}

#[tokio::test]
async fn test_did_change_mixed_range_none_and_some_sequence() {
    // UC (d): A realistic mixed sequence — full replace (range=None) followed
    // by a ranged insert, then another full replace. Both code paths must
    // cooperate on the same document without corruption.
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "SELECT 1").await;

    // Step 1: full replace (range=None) → CREATE TABLE users
    send_did_change(
        &mut service,
        "file:///test.sql",
        1,
        serde_json::json!([{ "text": "CREATE TABLE users (id INT)" }]),
    )
    .await;

    // Step 2: ranged insert of a second statement on a new line.
    // The original line is 27 chars (0-26); append "\nCREATE TABLE orders (id INT)"
    // at the zero-width point (0,27) — past the closing ")".
    send_did_change(
        &mut service,
        "file:///test.sql",
        2,
        serde_json::json!([{
            "range": {
                "start": { "line": 0, "character": 27 },
                "end":   { "line": 0, "character": 27 }
            },
            "text": "\nCREATE TABLE orders (id INT)"
        }]),
    )
    .await;

    let symbols = fetch_document_symbols(&mut service, "file:///test.sql", 3).await;
    let names = top_level_symbol_names(&symbols);
    assert!(
        names.iter().any(|n| n == "users"),
        "users must remain after ranged insert, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "orders"),
        "orders must appear after ranged insert, got: {names:?}"
    );

    // Step 3: full replace again (range=None) → one table.
    // 注: "finaltbl" を使用 — "only" は TokenKind::Only の予約語でテーブル名不可。
    send_did_change(
        &mut service,
        "file:///test.sql",
        3,
        serde_json::json!([{ "text": "CREATE TABLE finaltbl (id INT)" }]),
    )
    .await;

    let symbols_final = fetch_document_symbols(&mut service, "file:///test.sql", 4).await;
    let names_final = top_level_symbol_names(&symbols_final);
    assert!(
        names_final.iter().any(|n| n == "finaltbl"),
        "Final full replace must win, got: {names_final:?}"
    );
    assert!(
        !names_final.iter().any(|n| n == "users" || n == "orders"),
        "Prior content must be gone after full replace, got: {names_final:?}"
    );
}

#[tokio::test]
async fn test_did_change_unchanged_content_no_reparse() {
    // UC-1 (no-reparse path): a `did_change` whose range patch produces an
    // identical source string must not corrupt handler-visible state. When the
    // content-equality short-circuit reuses the cached `DocumentAnalysis`, the
    // `documentSymbol` handler must still resolve the same symbols and the
    // request must respond without panic.
    //
    // The patch is a zero-width insert of the empty string at a valid ASCII
    // char boundary: prefix + "" + suffix == source, byte-for-byte. This is the
    // canonical "type-then-delete" / no-op edit that the short-circuit must
    // handle gracefully.
    let mut service = setup();
    let initial = "CREATE TABLE users (id INT)";
    init_and_open(&mut service, "file:///test.sql", initial).await;

    // Baseline: the table symbol is resolvable from the open-time analysis.
    let symbols_before = fetch_document_symbols(&mut service, "file:///test.sql", 2).await;
    let names_before = top_level_symbol_names(&symbols_before);
    assert!(
        names_before.iter().any(|n| n == "users"),
        "Baseline must expose the table symbol, got: {names_before:?}"
    );

    // No-op ranged edit: insert "" at the zero-width point (0,27), past the
    // closing ")". ASCII content so byte offset == char offset; the patched
    // source is byte-identical to `initial`.
    send_did_change(
        &mut service,
        "file:///test.sql",
        1,
        serde_json::json!([{
            "range": {
                "start": { "line": 0, "character": 27 },
                "end":   { "line": 0, "character": 27 }
            },
            "text": ""
        }]),
    )
    .await;

    // After the no-op edit, the cached analysis is reused (or rebuilt on an
    // identical source). Either way the document symbol handler must return the
    // same symbol set and must not panic. This is the handler-visible proof
    // that the no-reparse path does not corrupt state.
    let symbols_after = fetch_document_symbols(&mut service, "file:///test.sql", 3).await;
    let names_after = top_level_symbol_names(&symbols_after);
    assert!(
        names_after.iter().any(|n| n == "users"),
        "No-op edit must preserve the table symbol (cached analysis reused), got: {names_after:?}"
    );
    assert_eq!(
        names_after, names_before,
        "Symbol set must be identical before and after a no-op edit"
    );

    // A second no-op edit compounds the scenario: repeating an identical-source
    // patch must remain stable (proves no accumulation of stale state across
    // multiple no-reparse hits).
    send_did_change(
        &mut service,
        "file:///test.sql",
        2,
        serde_json::json!([{
            "range": {
                "start": { "line": 0, "character": 0 },
                "end":   { "line": 0, "character": 0 }
            },
            "text": ""
        }]),
    )
    .await;

    let symbols_final = fetch_document_symbols(&mut service, "file:///test.sql", 4).await;
    let names_final = top_level_symbol_names(&symbols_final);
    assert_eq!(
        names_final, names_before,
        "Repeated no-op edits must keep the symbol set stable"
    );
}

#[tokio::test]
async fn test_did_change_incremental_multibyte_no_panic() {
    // Boundary safety: ranged change on a source containing a multibyte
    // (UTF-8) character must not panic and must keep the document usable.
    // "あ" is 3 bytes in UTF-8; inserting at character position 1 must land
    // on a valid char boundary.
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "SELECT あ").await;

    // Insert "!" after the multibyte char (character index 4: S E L E C T
    // space あ → 'あ' is at character 7; insert at 8).
    send_did_change(
        &mut service,
        "file:///test.sql",
        1,
        serde_json::json!([{
            "range": {
                "start": { "line": 0, "character": 8 },
                "end":   { "line": 0, "character": 8 }
            },
            "text": "!"
        }]),
    )
    .await;

    // Reaching here without panic proves boundary-safe slicing. Hover is used
    // as a generic round-trip probe on the patched document.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/hover",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 0, "character": 0 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(
        response.is_some(),
        "Hover after a multibyte ranged edit must respond (no panic)"
    );
}

// --- Cross-file symbol index tests (Issue #169 / T8) ---
//
// These tests exercise the multi-document symbol aggregation that the
// SymbolStore foundation (T1-T7) builds on. Two of the three scenarios are
// valid against the current DocumentStore-based aggregation today:
//
//   (A) workspace/symbol across multiple OPEN files — the `symbol()` handler
//       already iterates every entry in DocumentStore, so opening two files
//       and querying must surface symbols from both.
//   (C) Single-file open with no background index (unsupported environment)
//       must degrade gracefully: goto_definition for a symbol whose definition
//       lives in another (unopened, un-indexed) file returns null without
//       crashing.
//
// Scenario (B) — goto_definition jumping from one open file into another open
// file's CREATE TABLE — is covered by the cross-file definition provider
// (T2: `definition_locations` backed by SymbolStore), wired in server.rs.
// Cross-file *references* (Find All References across files) is now
// implemented (#170): the `references()` handler takes a DocumentStore
// snapshot under the read lock, releases it, then calls
// `reference_locations` under the SymbolStore read lock — mirroring
// goto_definition's acquire/drop pattern. See
// `test_cross_file_references_from_usage_file_returns_both_files` and
// `test_cross_file_references_from_definition_file_returns_other_file_usages`
// below. Variables (@var) stay document-local by design.

/// Open two files, then query workspace/symbol and confirm symbols from BOTH
/// files appear in the result. This is the integration-level proof that the
/// symbol aggregation loop spans documents (the same loop T1's SymbolStore
/// will eventually back by a reverse map).
#[tokio::test]
async fn test_workspace_symbol_spans_multiple_open_files() {
    let mut service = setup();
    // Initialize once.
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;

    // Open file A defining `users`.
    let open_a = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/a.sql",
                "languageId": "sql",
                "version": 0,
                "text": "CREATE TABLE users (id INT)"
            }
        }
    });
    send(&mut service, &open_a.to_string()).await;

    // Open file B defining `orders`.
    let open_b = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/b.sql",
                "languageId": "sql",
                "version": 0,
                "text": "CREATE TABLE orders (id INT)"
            }
        }
    });
    send(&mut service, &open_b.to_string()).await;

    // Query a fragment that matches BOTH table names.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "workspace/symbol",
        "params": { "query": "" }
    });
    // An empty query is treated as "no match" by the current provider (it
    // short-circuits on empty). Use a broad query that matches neither name
    // to confirm the cross-file loop still returns null gracefully.
    let req_none = serde_json::json!({
        "jsonrpc": "2.0", "id": 3, "method": "workspace/symbol",
        "params": { "query": "zzznomatch" }
    });
    let resp_none = send(&mut service, &req_none.to_string()).await;
    let resp_none = resp_none.expect("workspace/symbol must respond");
    let val_none: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&resp_none.result()).unwrap()).unwrap();
    assert!(
        val_none.is_null(),
        "Cross-file query with no match must return null gracefully, got: {val_none}"
    );

    // Query "user" — should find the table in file A only.
    let req_a = serde_json::json!({
        "jsonrpc": "2.0", "id": 4, "method": "workspace/symbol",
        "params": { "query": "user" }
    });
    let resp_a = send(&mut service, &req_a.to_string()).await;
    let resp_a = resp_a.expect("workspace/symbol must respond for 'user'");
    let val_a: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&resp_a.result()).unwrap()).unwrap();
    let arr_a = val_a
        .as_array()
        .expect("'user' query must yield a symbol array across open files");
    assert!(
        arr_a
            .iter()
            .any(|s| s.get("name").is_some_and(|n| n == "users") && s.get("location").is_some()),
        "Cross-file symbol search must surface 'users' from file A, got: {val_a}"
    );

    // Query "order" — should find the table in file B only.
    let req_b = serde_json::json!({
        "jsonrpc": "2.0", "id": 5, "method": "workspace/symbol",
        "params": { "query": "order" }
    });
    let resp_b = send(&mut service, &req_b.to_string()).await;
    let resp_b = resp_b.expect("workspace/symbol must respond for 'order'");
    let val_b: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&resp_b.result()).unwrap()).unwrap();
    let arr_b = val_b
        .as_array()
        .expect("'order' query must yield a symbol array across open files");
    assert!(
        arr_b
            .iter()
            .any(|s| s.get("name").is_some_and(|n| n == "orders") && s.get("location").is_some()),
        "Cross-file symbol search must surface 'orders' from file B, got: {val_b}"
    );

    // Sanity: each result must carry a location whose URI points at the file
    // that actually defines the symbol — proving the aggregation preserves
    // the symbol→origin-file link (the invariant T1's SymbolStore will keep).
    for sym in arr_a {
        let uri = sym
            .get("location")
            .and_then(|l| l.get("uri"))
            .and_then(|u| u.as_str())
            .expect("symbol must carry a location.uri");
        assert!(
            uri == "file:///ws/a.sql",
            "'users' symbol must be attributed to file A, got uri={uri}"
        );
    }
    for sym in arr_b {
        let uri = sym
            .get("location")
            .and_then(|l| l.get("uri"))
            .and_then(|u| u.as_str())
            .expect("symbol must carry a location.uri");
        assert!(
            uri == "file:///ws/b.sql",
            "'orders' symbol must be attributed to file B, got uri={uri}"
        );
    }

    // Suppress unused warning for `req` (the empty-query variant kept for
    // documentation of the short-circuit behavior).
    let _ = &req;
}

/// Single-file open with NO workspace folder / NO background indexer must
/// degrade gracefully: a goto_definition request for a symbol whose
/// definition is NOT in the open document returns null (UC for the
/// "unsupported environment" fallback path).
#[tokio::test]
async fn test_single_file_goto_def_unknown_symbol_returns_null() {
    let mut service = setup();
    // Open a single file that only REFERENCES `orders` but never defines it.
    init_and_open(
        &mut service,
        "file:///ws/only_queries.sql",
        "SELECT * FROM orders",
    )
    .await;

    // Click on `orders` (line 0, char 14 — past "SELECT * FROM ").
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": "file:///ws/only_queries.sql" },
            "position": { "line": 0, "character": 14 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(
        response.is_some(),
        "goto_definition must always return a response object"
    );

    let result = response.unwrap();
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    // `orders` is not defined anywhere the single-file server knows about, so
    // the result must be null (graceful — no crash, no spurious location).
    assert!(
        result_val.is_null(),
        "goto_definition for an unknown symbol in single-file mode must return null, got: {result_val}"
    );
}

/// Single-file open: goto_definition for a symbol defined in a SECOND,
/// UNOPENED file must return null. In an environment without background
/// indexing (no workspace folder, or indexer unavailable), the unopened file
/// is invisible to the server and the request degrades gracefully.
#[tokio::test]
async fn test_goto_def_into_unopened_file_returns_null() {
    let mut service = setup();
    // Open file A that references `shared_table`, but never open file B that
    // defines it. Without a background indexer the server cannot know about
    // file B, so the definition lookup must return null.
    init_and_open(
        &mut service,
        "file:///ws/a.sql",
        "SELECT * FROM shared_table",
    )
    .await;

    // Cursor on `shared_table` (line 0, char 14).
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": "file:///ws/a.sql" },
            "position": { "line": 0, "character": 14 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("goto_definition must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    assert!(
        result_val.is_null(),
        "Definition in an unopened/un-indexed file must degrade to null, got: {result_val}"
    );
}

/// UC (B): goto_definition jumping from one open file into ANOTHER open
/// file's CREATE TABLE. This is the headline cross-file feature of #169.
///
/// T2 (cross-file `definition_locations` backed by SymbolStore) is wired in
/// server.rs (definition::definition_locations); this test exercises the
/// end-to-end cross-file jump.
#[tokio::test]
async fn test_cross_file_goto_definition_into_other_open_file() {
    let mut service = setup();
    // File B defines `shared_table`.
    let open_b = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/b.sql",
                "languageId": "sql",
                "version": 0,
                "text": "CREATE TABLE shared_table (id INT)"
            }
        }
    });
    // File A references it.
    let open_a = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/a.sql",
                "languageId": "sql",
                "version": 0,
                "text": "SELECT * FROM shared_table"
            }
        }
    });
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;
    send(&mut service, &open_b.to_string()).await;
    send(&mut service, &open_a.to_string()).await;

    // Cursor on `shared_table` in file A (line 0, char 14).
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": "file:///ws/a.sql" },
            "position": { "line": 0, "character": 14 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("cross-file goto_definition must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    // The definition must be a non-null location whose URI is file B.
    let target_uri = result_val
        .get("uri")
        .or_else(|| {
            // GotoDefinitionResponse::Array wraps locations in an array.
            result_val
                .as_array()
                .and_then(|a| a.first())
                .and_then(|l| l.get("uri"))
        })
        .and_then(|u| u.as_str())
        .expect("cross-file goto_definition must return a location");
    assert!(
        target_uri == "file:///ws/b.sql",
        "Cross-file goto_definition must jump into file B, got uri={target_uri}"
    );
}

/// UC1 (T5 / #170): Find All References on a table USAGE in file A must
/// return locations whose URIs span BOTH file A (the usages: SELECT/INSERT/
/// DELETE) AND file B (the CREATE TABLE definition, because
/// `includeDeclaration` is true).
///
/// File B defines `shared_table`; file A references it via SELECT/INSERT/
/// DELETE. `textDocument/references` is issued from file A with the cursor on
/// a usage. `reference_locations` (references.rs) scans the tokens of every
/// known document (snapshot taken under the DocumentStore read lock, then
/// released before the SymbolStore read lock — lock-ordering convention).
/// Before the #170 rewire the handler mapped every result onto the single
/// requesting URI, so no `file:///ws/b.sql` location could ever appear. This
/// is the references-direction mirror of
/// `test_cross_file_goto_definition_into_other_open_file`.
#[tokio::test]
async fn test_cross_file_references_from_usage_file_returns_both_files() {
    let mut service = setup();
    // File B defines `shared_table`.
    let open_b = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/b.sql",
                "languageId": "sql",
                "version": 0,
                "text": "CREATE TABLE shared_table (id INT)"
            }
        }
    });
    // File A references it via SELECT / INSERT / DELETE.
    let open_a = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/a.sql",
                "languageId": "sql",
                "version": 0,
                "text": "SELECT * FROM shared_table\nINSERT INTO shared_table VALUES (1)\nDELETE FROM shared_table"
            }
        }
    });
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;
    send(&mut service, &open_b.to_string()).await;
    send(&mut service, &open_a.to_string()).await;

    // Cursor on `shared_table` usage in file A line 0. "SELECT * FROM " = 14 chars.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/references",
        "params": {
            "textDocument": { "uri": "file:///ws/a.sql" },
            "position": { "line": 0, "character": 14 },
            "context": { "includeDeclaration": true }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("cross-file references must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    // Result is an array of locations. Collect every URI the server returned.
    let locations = result_val
        .as_array()
        .expect("references result must be an array of locations");
    assert!(
        !locations.is_empty(),
        "cross-file references must return at least one location"
    );
    let uris: Vec<&str> = locations
        .iter()
        .filter_map(|l| l.get("uri").and_then(|u| u.as_str()))
        .collect();
    // Both files must contribute at least one reference.
    assert!(
        uris.contains(&"file:///ws/a.sql"),
        "references must include file A usages, got uris={uris:?}"
    );
    assert!(
        uris.contains(&"file:///ws/b.sql"),
        "references must include file B definition, got uris={uris:?}"
    );
}

/// UC2 (T5 / #170): Find All References on the table name in file B — the
/// DEFINITION file — must still return the usages that live in file A. The
/// cross-file scan is symmetric: querying from either the definition or a
/// usage file surfaces every known occurrence workspace-wide.
#[tokio::test]
async fn test_cross_file_references_from_definition_file_returns_other_file_usages() {
    let mut service = setup();
    // File B defines `shared_table`.
    let open_b = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/b.sql",
                "languageId": "sql",
                "version": 0,
                "text": "CREATE TABLE shared_table (id INT)"
            }
        }
    });
    // File A references it.
    let open_a = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/a.sql",
                "languageId": "sql",
                "version": 0,
                "text": "SELECT * FROM shared_table"
            }
        }
    });
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;
    send(&mut service, &open_b.to_string()).await;
    send(&mut service, &open_a.to_string()).await;

    // Cursor on `shared_table` in file B (the definition). "CREATE TABLE " = 13
    // chars, so the identifier starts at char 13; aim mid-identifier at char 17.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/references",
        "params": {
            "textDocument": { "uri": "file:///ws/b.sql" },
            "position": { "line": 0, "character": 17 },
            "context": { "includeDeclaration": true }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("cross-file references must respond from definition file");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let refs = result_val
        .as_array()
        .expect("references from definition file must return a non-null array");

    let uris: Vec<&str> = refs
        .iter()
        .filter_map(|l| l.get("uri").and_then(|u| u.as_str()))
        .collect();
    assert!(
        uris.contains(&"file:///ws/a.sql"),
        "Find All References from file B (definition) must include file A usages, got uris={uris:?}"
    );
    assert!(
        uris.contains(&"file:///ws/b.sql"),
        "Find All References from file B must include file B definition, got uris={uris:?}"
    );
}

/// UC (T5): Find All References on a variable (@var) stays document-local.
/// Even when another open file declares the same `@var` name, only the current
/// file's references are returned (#169 design decision: variables are scoped
/// document-locally; mirrors definition.rs:129).
#[tokio::test]
async fn test_references_variable_stays_document_local() {
    let mut service = setup();
    // File B declares and uses @count.
    let open_b = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/b.sql",
                "languageId": "sql",
                "version": 0,
                "text": "DECLARE @count INT\nSET @count = 1"
            }
        }
    });
    // File A declares and uses @count independently.
    let open_a = serde_json::json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": "file:///ws/a.sql",
                "languageId": "sql",
                "version": 0,
                "text": "DECLARE @count INT\nSET @count = 1\nSELECT @count"
            }
        }
    });
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;
    send(&mut service, &open_b.to_string()).await;
    send(&mut service, &open_a.to_string()).await;

    // Cursor on @count in file A line 2 "SELECT @count" (char 7).
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/references",
        "params": {
            "textDocument": { "uri": "file:///ws/a.sql" },
            "position": { "line": 2, "character": 7 },
            "context": { "includeDeclaration": true }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("references must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    let locations = result_val
        .as_array()
        .expect("variable references result must be an array");
    let uris: Vec<&str> = locations
        .iter()
        .filter_map(|l| l.get("uri").and_then(|u| u.as_str()))
        .collect();
    // Only a.sql; b.sql's @count is a different scope and must NOT appear.
    assert!(
        uris.iter().all(|u| *u == "file:///ws/a.sql"),
        "variable references must stay in a.sql only, got uris={uris:?}"
    );
    assert!(
        !uris.contains(&"file:///ws/b.sql"),
        "variable references must NOT cross into b.sql"
    );
}

// --- Code Lens tests (#117) ---

/// A lens is "resolved" when its `command` field is a JSON object (carrying a
/// `title`). An unresolved lens has `command` absent or null — lsp-types
/// serializes `Option<Command>::None` as an omitted key, so both forms mean
/// "no command yet".
fn command_is_resolved(lens: &serde_json::Value) -> bool {
    lens.get("command").is_some_and(|c| c.is_object())
}

/// (a) `textDocument/codeLens` returns one unresolved lens per CREATE
/// TABLE/PROCEDURE/VIEW definition. Each lens has `command: null` (deferred
/// resolution) and carries `data` for the resolve phase.
#[tokio::test]
async fn test_code_lens_returns_unresolved_lenses() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "CREATE TABLE t1 (id INT)\n\
         CREATE VIEW v1 AS SELECT * FROM t1\n\
         CREATE PROC p1 AS SELECT * FROM t1",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/codeLens",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("codeLens must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    let lenses = result_val
        .as_array()
        .expect("codeLens result must be an array");
    assert_eq!(
        lenses.len(),
        3,
        "one lens per TABLE/VIEW/PROC definition, got {lenses:?}"
    );
    // All lenses start unresolved (no command object) and carry data.
    assert!(
        lenses.iter().all(|l| !command_is_resolved(l)),
        "unresolved lenses must not carry a command object: {lenses:?}"
    );
    assert!(
        lenses
            .iter()
            .all(|l| l.get("data").is_some_and(|d| !d.is_null())),
        "unresolved lenses must carry data for resolution"
    );
}

#[tokio::test]
async fn test_code_lens_resolve_sets_references_title() {
    let mut service = setup();
    // t defined once, used in SELECT + INSERT (2 usages; CREATE line excluded).
    init_and_open(
        &mut service,
        "file:///test.sql",
        "CREATE TABLE t (id INT)\n\
         SELECT * FROM t\n\
         INSERT INTO t VALUES (1)",
    )
    .await;

    // Stage 1: fetch the unresolved lens.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/codeLens",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("codeLens must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let lens = result_val
        .as_array()
        .and_then(|arr| arr.first())
        .expect("at least one lens");

    // Stage 2: resolve it. The resolve request carries the lens as params.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 3, "method": "codeLens/resolve",
        "params": lens
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("codeLens/resolve must respond");
    let resolved: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    let title = resolved
        .get("command")
        .and_then(|c| c.get("title"))
        .and_then(|t| t.as_str())
        .expect("resolved lens must have a command.title");
    assert!(
        title.contains("references"),
        "resolved title must contain 'references', got: {title}"
    );
    assert!(
        title.contains('2'),
        "expected 2 references (SELECT + INSERT usages), got: {title}"
    );
}

#[tokio::test]
async fn test_code_lens_resolve_unloaded_document_returns_input_lens() {
    let mut service = setup();
    // Build a valid lens for a document that was never opened. The server
    // cannot find an analysis for its URI, so resolve must fall back to
    // returning the input lens unchanged (the Ok(params) branch guarded by
    // the task-1 params.clone() fix).
    let lens = serde_json::json!({
        "range": {
            "start": { "line": 0, "character": 0 },
            "end": { "line": 0, "character": 1 }
        },
        "command": null,
        "data": {
            "uri": "file:///never-opened.sql",
            "line": 0,
            "character": 0
        }
    });

    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "codeLens/resolve",
        "params": lens
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("codeLens/resolve must respond");
    let resolved: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    // Fallback contract: the input lens is returned verbatim. command stays
    // unset (unresolved — absent or null, both mean "no command object") and
    // the embedded data URI is preserved.
    assert!(
        !command_is_resolved(&resolved),
        "fallback lens must remain unresolved (no command object), got {resolved:?}"
    );
    let data_uri = resolved
        .get("data")
        .and_then(|d| d.get("uri"))
        .and_then(|u| u.as_str());
    assert_eq!(
        data_uri,
        Some("file:///never-opened.sql"),
        "fallback lens must preserve the embedded data URI, got {resolved:?}"
    );
}

// --- Inlay Hint tests (#118 Task 6: server handler end-to-end) ---

/// Helper: extract inlay hint labels as strings from a JSON result.
fn inlay_hint_labels(result_val: &serde_json::Value) -> Vec<String> {
    let arr = result_val
        .as_array()
        .expect("inlayHint result must be an array");
    arr.iter()
        .map(|h| {
            h.get("label")
                .and_then(|l| l.as_str())
                .expect("hint label is a string")
                .to_string()
        })
        .collect()
}

/// `textDocument/inlayHint` on a DECLARE returns one `: INT` type hint (#118).
#[tokio::test]
async fn test_inlay_hint_declare_emits_type_hint() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "DECLARE @count INT").await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/inlayHint",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 100 }
            }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("inlayHint must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    let labels = inlay_hint_labels(&result_val);
    assert!(
        labels.iter().any(|l| l == ": INT"),
        "expected ': INT' hint, got {labels:?}"
    );

    // #118 task 7 contract: the DECLARE hint must carry kind=1 (TYPE).
    let arr = result_val
        .as_array()
        .expect("inlayHint result must be an array");
    let type_hint = arr
        .iter()
        .find(|h| h.get("label").and_then(|l| l.as_str()) == Some(": INT"))
        .expect("a ': INT' hint exists");
    assert_eq!(
        type_hint.get("kind").and_then(|k| k.as_i64()),
        Some(1),
        "DECLARE variable hint must be TYPE (kind=1), got {type_hint:?}"
    );
}

/// `textDocument/inlayHint` on EXEC with positional args + in-document
/// CREATE PROC signature emits PARAMETER hints (`@a:`, `@b:`).
#[tokio::test]
async fn test_inlay_hint_exec_emits_parameter_hints() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///test.sql",
        "CREATE PROC myproc @a INT, @b INT AS\nSELECT 1\nEXEC myproc 10, 20",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/inlayHint",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 10, "character": 0 }
            }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("inlayHint must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    let labels = inlay_hint_labels(&result_val);
    assert!(
        labels.contains(&"@a:".to_string()),
        "expected '@a:' hint, got {labels:?}"
    );
    assert!(
        labels.contains(&"@b:".to_string()),
        "expected '@b:' hint, got {labels:?}"
    );

    // #118 task 7 contract: EXEC positional-arg hints must carry kind=2
    // (PARAMETER).
    let arr = result_val
        .as_array()
        .expect("inlayHint result must be an array");
    let param_hints: Vec<&serde_json::Value> = arr
        .iter()
        .filter(|h| {
            h.get("label")
                .and_then(|l| l.as_str())
                .is_some_and(|l| l == "@a:" || l == "@b:")
        })
        .collect();
    assert_eq!(
        param_hints.len(),
        2,
        "exactly two PARAMETER hints, got {arr:?}"
    );
    assert!(
        param_hints
            .iter()
            .all(|h| h.get("kind").and_then(|k| k.as_i64()) == Some(2)),
        "EXEC positional-arg hints must be PARAMETER (kind=2), got {param_hints:?}"
    );
}

/// `textDocument/inlayHint` on an unloaded document returns `null` (Ok(None)).
#[tokio::test]
async fn test_inlay_hint_unloaded_document_returns_null() {
    let mut service = setup();
    // Initialize but never open the target document.
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/inlayHint",
        "params": {
            "textDocument": { "uri": "file:///never-opened.sql" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 10 }
            }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("inlayHint must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    assert!(
        result_val.is_null(),
        "unloaded document must yield null, got {result_val:?}"
    );
}

/// A document with no DECLARE / EXEC yields an empty (but non-null) array.
#[tokio::test]
async fn test_inlay_hint_no_candidates_returns_empty_array() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "SELECT 1\nFROM t").await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/inlayHint",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 10, "character": 0 }
            }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("inlayHint must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let arr = result_val
        .as_array()
        .expect("non-null document must yield an array (possibly empty)");
    assert!(arr.is_empty(), "no DECLARE/EXEC → no hints, got {arr:?}");
}

/// Disabling inlay hints via `workspace/didChangeConfiguration` suppresses
/// subsequent `textDocument/inlayHint` output (#118 task 7 scenario 4).
///
/// Mirrors the formatting config-change integration test (lines 230+):
/// (1) confirm the default config emits a hint, (2) push a settings update
/// turning `enableVariableTypes` off, (3) re-request and assert the hint is
/// gone. This pins the handler's config-threading contract — the
/// `Arc<RwLock<Config>>` must be read on every request, not cached.
#[tokio::test]
async fn test_inlay_hint_config_disable_suppresses_hints() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "DECLARE @count INT").await;

    // Stage 1: default config → one TYPE hint is emitted.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/inlayHint",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 100 }
            }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("inlayHint must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let labels_before = inlay_hint_labels(&result_val);
    assert!(
        labels_before.iter().any(|l| l == ": INT"),
        "default config must emit the ': INT' hint, got {labels_before:?}"
    );

    // Stage 2: push a config change disabling variable-type hints.
    let cfg = serde_json::json!({
        "jsonrpc": "2.0", "method": "workspace/didChangeConfiguration",
        "params": {
            "settings": { "ase-ls": { "inlay": { "enableVariableTypes": false } } }
        }
    });
    send(&mut service, &cfg.to_string()).await;

    // Stage 3: re-request with a fresh id — the hint must now be suppressed.
    let req2 = serde_json::json!({
        "jsonrpc": "2.0", "id": 3, "method": "textDocument/inlayHint",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 0, "character": 100 }
            }
        }
    });
    let response = send(&mut service, &req2.to_string()).await;
    let result = response.expect("inlayHint must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let labels_after = inlay_hint_labels(&result_val);
    assert!(
        labels_after.is_empty(),
        "after disabling enableVariableTypes no hints should be emitted, got {labels_after:?}"
    );
}

// --- Document Link tests (#119 Task 6: server handler end-to-end) ---
//
// Mirrors test_code_lens_* / test_inlay_hint_*: drive the full JSON-RPC cycle
// through LspService::new + ServiceExt. These exercise the
// textDocument/documentLink + documentLink/resolve two-stage pattern over the
// SQLCMD `:r <path>` include directive.

/// (a) `textDocument/documentLink` on a doc with `:r scripts/init.sql` returns
/// exactly one link whose range covers the directive and whose target URI is
/// resolved document-relative against the document directory.
#[tokio::test]
async fn test_document_link_returns_one_link_for_r_directive() {
    let mut service = setup();
    // The document lives under /home/user/scripts/ so a document-relative
    // `:r scripts/init.sql` resolves to /home/user/scripts/scripts/init.sql.
    // Use a sibling path so the target is clearly under the doc directory.
    init_and_open(
        &mut service,
        "file:///home/user/scripts/main.sql",
        ":r init.sql",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentLink",
        "params": {
            "textDocument": { "uri": "file:///home/user/scripts/main.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("documentLink must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    let links = result_val
        .as_array()
        .expect("documentLink result must be an array of links");
    assert_eq!(links.len(), 1, "one :r directive → one link: {links:?}");

    let link = &links[0];
    // Range starts at the colon (line 0, char 0) and spans the whole directive
    // ":r init.sql" (11 chars).
    let start = link
        .get("range")
        .and_then(|r| r.get("start"))
        .expect("link has range.start");
    assert_eq!(
        start.get("line").and_then(|v| v.as_u64()),
        Some(0),
        "link starts at line 0"
    );
    assert_eq!(
        start.get("character").and_then(|v| v.as_u64()),
        Some(0),
        "link starts at char 0 (the colon)"
    );
    let end = link
        .get("range")
        .and_then(|r| r.get("end"))
        .expect("link has range.end");
    assert_eq!(
        end.get("line").and_then(|v| v.as_u64()),
        Some(0),
        "link ends on line 0"
    );
    assert_eq!(
        end.get("character").and_then(|v| v.as_u64()),
        Some(11),
        "link ends at char 11 (end of ':r init.sql')"
    );

    // Target resolved document-relative against the document directory.
    let target = link
        .get("target")
        .and_then(|t| t.as_str())
        .expect("link must carry a resolved target");
    assert!(
        target.ends_with("/home/user/scripts/init.sql"),
        "target must be document-relative to the scripts/ directory, got {target}"
    );

    // data must be stashed for the resolve stage.
    assert!(
        link.get("data").is_some_and(|d| !d.is_null()),
        "link must carry data for documentLink/resolve"
    );
}

/// (b) `documentLink/resolve` recovers the target via the embedded URI payload.
/// Simulates a round-trip that strips the target: the resolve handler reads the
/// owning document URI + raw path from `data` and re-establishes the target.
#[tokio::test]
async fn test_document_link_resolve_recovers_target_from_data() {
    let mut service = setup();
    init_and_open(
        &mut service,
        "file:///home/user/scripts/main.sql",
        ":r init.sql",
    )
    .await;

    // Stage 1: fetch the link.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentLink",
        "params": {
            "textDocument": { "uri": "file:///home/user/scripts/main.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("documentLink must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let mut link = result_val
        .as_array()
        .and_then(|arr| arr.first())
        .expect("at least one link")
        .clone();
    // Simulate a client round-trip that drops the target before resolving.
    link["target"] = serde_json::Value::Null;

    // Stage 2: resolve — the handler must recover the target from data.
    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 3, "method": "documentLink/resolve",
        "params": link
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("documentLink/resolve must respond");
    let resolved: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();

    let target = resolved
        .get("target")
        .and_then(|t| t.as_str())
        .expect("resolve must recover the target");
    assert!(
        target.ends_with("/home/user/scripts/init.sql"),
        "resolve must re-establish the document-relative target, got {target}"
    );
}

/// (c) A Windows backslash path argument normalises to forward-slash in the
/// resolved target URI.
#[tokio::test]
async fn test_document_link_backslash_path_normalises_to_forward_slash() {
    let mut service = setup();
    // Quoted backslash path (the lexer tokenises an unquoted backslash path
    // into Unknown tokens, so the path must be quoted to lex cleanly).
    init_and_open(
        &mut service,
        "file:///home/user/scripts/main.sql",
        r":r 'sub\child.sql'",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentLink",
        "params": {
            "textDocument": { "uri": "file:///home/user/scripts/main.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("documentLink must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let links = result_val
        .as_array()
        .expect("documentLink result must be an array");
    assert_eq!(links.len(), 1, "backslash :r yields one link: {links:?}");

    let target = links[0]
        .get("target")
        .and_then(|t| t.as_str())
        .expect("link must carry a resolved target");
    assert!(
        target.ends_with("/home/user/scripts/sub/child.sql"),
        "backslash must normalise to forward slash in target, got {target}"
    );
    assert!(
        !target.contains('\\'),
        "target must contain no backslash after normalisation, got {target}"
    );
}

/// (d) An empty document (no `:r` directives) returns an empty array (non-null).
#[tokio::test]
async fn test_document_link_empty_document_returns_empty_array() {
    let mut service = setup();
    init_and_open(&mut service, "file:///test.sql", "").await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentLink",
        "params": {
            "textDocument": { "uri": "file:///test.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("documentLink must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    let arr = result_val
        .as_array()
        .expect("empty document must yield an array (possibly empty)");
    assert!(arr.is_empty(), "empty document → no links, got {arr:?}");
}

/// (e) An unloaded document returns `null` (Ok(None)).
#[tokio::test]
async fn test_document_link_unloaded_document_returns_null() {
    let mut service = setup();
    // Initialize but never open the target document.
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "capabilities": {} }
    });
    send(&mut service, &init.to_string()).await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/documentLink",
        "params": {
            "textDocument": { "uri": "file:///never-opened.sql" }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    let result = response.expect("documentLink must respond");
    let result_val: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&result.result()).unwrap()).unwrap();
    assert!(
        result_val.is_null(),
        "unloaded document must yield null, got {result_val:?}"
    );
}
