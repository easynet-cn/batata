//! Visibility enforcement test for the MCP service.
//!
//! This lives in its **own** test binary on purpose: `VisibilityPluginManager`
//! is a process-global singleton, and `with_visibility` only registers a
//! service when none is registered yet. Running in a separate binary guarantees
//! a fresh singleton so this test can install an auth-**enabled** service —
//! with auth disabled the advisor returns `BaseVisibilityPredicate::All` and
//! nothing would be filtered at all.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-ai --test mcp_visibility -- --ignored --nocapture
//!
//! DATABASE_URL="postgres://postgres:devterry@127.0.0.1:5432/batata_ai_test" \
//!   cargo test -p batata-ai --test mcp_visibility -- --ignored --nocapture
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use batata_ai::model::{
    McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport,
};
use batata_ai::{McpServerIndex, McpServerOperationService};
use batata_persistence::ExternalDbPersistService;
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};

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

async fn clean(store: &ExternalDbPersistService) {
    let db = store.db();
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(NAME))
        .exec(db)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(NAME))
        .exec(db)
        .await
        .expect("clean resources");
}

/// Build a service with visibility **enabled** (auth enabled, no auth plugin).
async fn setup() -> (Arc<ExternalDbPersistService>, McpServerOperationService) {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = Database::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect {url}: {e}"));
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let index = Arc::new(McpServerIndex::new());
    // `auth_enabled = true` makes the advisor return `Public` for an anonymous
    // caller, which is what we need to observe filtering.
    let svc = McpServerOperationService::with_visibility(store.clone(), index, None, true);
    clean(&store).await;
    (store, svc)
}

/// A newly created server is private, so an anonymous caller must not see it —
/// and the reported total must shrink, not just the page contents.
#[tokio::test]
#[ignore]
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

    clean(&store).await;
}

/// Publishing the scope makes the server visible, and the change must take
/// effect immediately (the cached index entry has to be updated too).
#[tokio::test]
#[ignore]
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

    clean(&store).await;
}

/// A private server must not be readable by an anonymous caller.
#[tokio::test]
#[ignore]
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

    clean(&store).await;
}
