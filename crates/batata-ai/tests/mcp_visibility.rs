//! Visibility enforcement test for the MCP service.
//!
//! This lives in its **own** test binary on purpose: `VisibilityPluginManager`
//! is a process-global singleton, and `with_visibility` only registers a
//! service when none is registered yet. Running in a separate binary guarantees
//! a fresh singleton so this test can install an auth-**enabled** service —
//! with auth disabled the advisor returns `BaseVisibilityPredicate::All` and
//! nothing would be filtered at all.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same cases
//! against a real database.

mod common;

use std::collections::HashMap;
use std::sync::Arc;

use batata_ai::model::{
    McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport,
};
use batata_ai::{McpServerIndex, McpServerOperationService};
use common::TestStore;

const NS: &str = "public";
const NAME: &str = "vis-mcp";

fn registration() -> McpServerRegistration {
    McpServerRegistration {
        name: NAME.to_string(),
        display_name: "Visibility MCP".to_string(),
        description: "private by default".to_string(),
        namespace: NS.to_string(),
        version: "1.0.0".to_string(),
        endpoint: "http://localhost:8080".to_string(),
        server_type: McpServerType::Http,
        transport: McpTransport::default(),
        capabilities: vec![McpCapability::Tool],
        tools: vec![McpTool {
            name: "echo".to_string(),
            description: "echo".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }],
        resources: vec![],
        prompts: vec![],
        metadata: HashMap::new(),
        tags: vec![],
        auto_fetch_tools: true,
        health_check: None,
    }
}

/// Build a service with visibility **enabled** (auth enabled, no auth plugin).
async fn setup() -> (TestStore, McpServerOperationService) {
    let store = TestStore::open().await;
    let index = Arc::new(McpServerIndex::new());
    // `auth_enabled = true` makes the advisor return `Public` for an anonymous
    // caller, which is what we need to observe filtering.
    let svc = McpServerOperationService::with_visibility(store.store(), index, None, true);
    store.clean(NAME).await;
    (store, svc)
}

/// A newly created server is private, so an anonymous caller must not see it —
/// and the reported total must shrink, not just the page contents.
#[tokio::test]
async fn private_server_hidden_from_anonymous_list() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration())
        .await
        .expect("create");

    let page = svc
        .list_mcp_servers(NS, Some(NAME), "accurate", 1, 20, None)
        .await;
    assert_eq!(
        page.total_count, 0,
        "a private server must be hidden from an anonymous caller"
    );

    store.clean(NAME).await;
}

/// Publishing the scope makes the server visible, and the change must take
/// effect immediately (the cached index entry has to be updated too).
#[tokio::test]
async fn public_server_visible_after_scope_change() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration())
        .await
        .expect("create");

    svc.update_mcp_server_scope(NS, NAME, "PUBLIC")
        .await
        .expect("make public");

    let page = svc
        .list_mcp_servers(NS, Some(NAME), "accurate", 1, 20, None)
        .await;
    assert_eq!(
        page.total_count, 1,
        "a public server must be visible to an anonymous caller"
    );

    store.clean(NAME).await;
}

/// A private server must not be readable by an anonymous caller.
#[tokio::test]
async fn private_server_detail_rejected_for_anonymous() {
    let (store, svc) = setup().await;
    svc.create_mcp_server(NS, &registration())
        .await
        .expect("create");

    let result = svc
        .get_mcp_server_detail(NS, None, Some(NAME), None, None)
        .await;
    assert!(
        result.is_err(),
        "reading a private server anonymously must fail"
    );

    store.clean(NAME).await;
}
