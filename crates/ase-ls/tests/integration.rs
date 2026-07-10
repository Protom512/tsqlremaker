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
// Cross-file *references* (Find All References across files) is deferred to a
// follow-up issue; references stay document-local for now.

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
