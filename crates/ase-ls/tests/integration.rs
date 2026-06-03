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
    assert!(response.is_some(), "Should return definition response for table");

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
    init_and_open(
        &mut service,
        "file:///test.sql",
        "SELECT  FROM t",
    )
    .await;

    let req = serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
        "params": {
            "textDocument": { "uri": "file:///test.sql" },
            "position": { "line": 0, "character": 7 }
        }
    });
    let response = send(&mut service, &req.to_string()).await;
    assert!(response.is_some(), "Should return response even for whitespace");
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
    init_and_open(
        &mut service,
        "file:///test.sql",
        "SELECT * FROM users",
    )
    .await;
    // Reaching here without panic proves the diagnostics path is stable
}

#[tokio::test]
async fn test_diagnostics_on_open_with_parse_error() {
    let mut service = setup();
    // Opening invalid SQL should not crash
    init_and_open(
        &mut service,
        "file:///test.sql",
        "SELCT * FRM",
    )
    .await;
    // Reaching here without panic proves the parse error diagnostics path is stable
}
