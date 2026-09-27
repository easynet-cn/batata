//! Real-database round-trip test for the MCP server service.
//!
//! Verifies the migration from config-backed storage to `ai_resource` /
//! `ai_resource_version`: id stability, versioning, latest-version advance,
//! in-place update, tool payload preservation and deletion.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-ai --test mcp_persistence -- --ignored --nocapture
//!
//! DATABASE_URL="postgres://postgres:devterry@127.0.0.1:5432/batata_ai_test" \
//!   cargo test -p batata-ai --test mcp_persistence -- --ignored --nocapture
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use batata_ai::model::{
    McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport,
};
use batata_ai::{McpServerIndex, McpServerOperationService};
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};
use batata_persistence::ExternalDbPersistService;

const NS: &str = "public";
const NAME: &str = "probe-mcp";

fn registration(version: &str, tool: &str) -> McpServerRegistration {
    McpServerRegistration {
        name: NAME.to_string(),
        display_name: "Probe MCP".to_string(),
        description: format!("desc-{version}"),
        namespace: NS.to_string(),
        version: version.to_string(),
        endpoint: "http://localhost:8080".to_string(),
        server_type: McpServerType::Http,
        transport: McpTransport::default(),
        capabilities: vec![McpCapability::Tool],
        tools: vec![McpTool {
            name: tool.to_string(),
            description: format!("tool-{tool}"),
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

/// Remove rows left by a previous run so the test is repeatable.
async fn clean(store: &ExternalDbPersistService) {
    let conn = store.db();
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(NAME))
        .exec(conn)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(NAME))
        .exec(conn)
        .await
        .expect("clean resources");
}

#[tokio::test]
#[ignore]
async fn mcp_server_round_trip() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = Database::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect {url}: {e}"));
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let index = Arc::new(McpServerIndex::new());
    let svc = McpServerOperationService::new(store.clone(), index.clone());
    println!("--- backend: {:?} ---", store.db().get_database_backend());

    clean(&store).await;
    index.refresh(store.as_ref() as &dyn batata_persistence::PersistenceService)
        .await;

    // ---- create ------------------------------------------------------------
    let id = svc
        .create_mcp_server(NS, &registration("1.0.0", "alpha"))
        .await
        .expect("create");
    assert!(!id.is_empty());

    assert!(
        svc.create_mcp_server(NS, &registration("1.0.0", "alpha"))
            .await
            .is_err(),
        "duplicate creation must fail"
    );
    println!("ok: create id={id}");

    // ---- read back ---------------------------------------------------------
    let server = svc
        .get_mcp_server_detail(NS, None, Some(NAME), None)
        .await
        .expect("get")
        .expect("server must exist");
    assert_eq!(server.name, NAME);
    assert_eq!(server.version, "1.0.0");
    assert_eq!(server.id, id, "id must be stable across a read");
    assert_eq!(server.tools.len(), 1, "tools must survive the round trip");
    assert_eq!(server.tools[0].name, "alpha");
    println!("ok: read latest (tools preserved)");

    // ---- add a second version ---------------------------------------------
    svc.update_mcp_server(NS, &registration("2.0.0", "beta"))
        .await
        .expect("add version");

    let latest = svc
        .get_mcp_server_detail(NS, None, Some(NAME), None)
        .await
        .expect("get")
        .expect("server must exist");
    assert_eq!(latest.version, "2.0.0", "latest must advance");
    assert_eq!(latest.tools[0].name, "beta");

    let old = svc
        .get_mcp_server_detail(NS, None, Some(NAME), Some("1.0.0"))
        .await
        .expect("get")
        .expect("old version must remain readable");
    assert_eq!(old.version, "1.0.0");
    assert_eq!(old.tools[0].name, "alpha", "old version keeps its own tools");
    println!("ok: versioning");

    // ---- update an existing version in place -------------------------------
    let mut updated = registration("2.0.0", "gamma");
    updated.description = "changed".to_string();
    svc.update_mcp_server(NS, &updated).await.expect("update");

    let reread = svc
        .get_mcp_server_detail(NS, None, Some(NAME), Some("2.0.0"))
        .await
        .expect("get")
        .expect("server must exist");
    assert_eq!(reread.description, "changed");
    assert_eq!(reread.tools[0].name, "gamma");
    println!("ok: update in place");

    // ---- index refresh rebuilds from ai_resource ---------------------------
    let fresh = Arc::new(McpServerIndex::new());
    fresh
        .refresh(store.as_ref() as &dyn batata_persistence::PersistenceService)
        .await;
    let entry = fresh
        .get_by_name(NS, NAME)
        .expect("index refresh must rebuild the entry from ai_resource");
    assert_eq!(entry.name, NAME);
    assert_eq!(entry.latest_published_version, "2.0.0");
    assert_eq!(entry.version_count, 2, "index must see both versions");
    println!("ok: index refresh from ai_resource");

    // ---- delete one version ------------------------------------------------
    svc.delete_mcp_server(NS, Some(NAME), None, Some("1.0.0"))
        .await
        .expect("delete version");
    assert!(
        svc.get_mcp_server_detail(NS, None, Some(NAME), Some("1.0.0"))
            .await
            .expect("get")
            .is_none(),
        "deleted version must be gone"
    );
    assert!(
        svc.get_mcp_server_detail(NS, None, Some(NAME), Some("2.0.0"))
            .await
            .expect("get")
            .is_some(),
        "remaining version must survive"
    );
    println!("ok: delete single version");

    // ---- delete the whole server -------------------------------------------
    svc.delete_mcp_server(NS, Some(NAME), None, None)
        .await
        .expect("delete server");
    assert!(
        svc.get_mcp_server_detail(NS, None, Some(NAME), None)
            .await
            .expect("get")
            .is_none(),
        "server must be gone"
    );
    println!("ok: delete server");
}
