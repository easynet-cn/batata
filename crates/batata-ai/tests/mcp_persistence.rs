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
use batata_persistence::ExternalDbPersistService;
use batata_persistence::entity::{ai_resource, ai_resource_version};
use batata_persistence::sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};

const NS: &str = "public";
const NAME: &str = "probe-mcp";
/// Separate server name so the two tests in this file can run concurrently.
const LIFECYCLE_NAME: &str = "probe-mcp-lifecycle";

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

/// Registration bound to [`LIFECYCLE_NAME`].
fn lifecycle_registration(version: &str) -> McpServerRegistration {
    let mut reg = registration(version, "tool");
    reg.name = LIFECYCLE_NAME.to_string();
    reg
}

/// draft → reviewing → online, then offline/online, redraft and force-publish.
#[tokio::test]
#[ignore]
async fn mcp_version_lifecycle() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let conn = Database::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect {url}: {e}"));
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let index = Arc::new(McpServerIndex::new());
    let svc = McpServerOperationService::new(store.clone(), index);
    println!("--- backend: {:?} ---", store.db().get_database_backend());

    // Clean up rows from a previous run, using the same filter the shared
    // helper applies to NAME.
    let db = store.db();
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(LIFECYCLE_NAME))
        .exec(db)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(LIFECYCLE_NAME))
        .exec(db)
        .await
        .expect("clean resources");

    // ---- published baseline ------------------------------------------------
    svc.create_mcp_server(NS, &lifecycle_registration("1.0.0"))
        .await
        .expect("create server");

    // ---- draft → reviewing ------------------------------------------------
    let draft = svc
        .create_mcp_server_draft(NS, &lifecycle_registration("2.0.0"), false)
        .await
        .expect("create draft");
    assert_eq!(draft.summary.status, "draft");

    let submitted = svc
        .submit_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
        .await
        .expect("submit");
    assert_eq!(submitted.summary.status, "reviewing");
    assert_eq!(submitted.reviewing_version.as_deref(), Some("2.0.0"));
    assert_eq!(submitted.editing_version, None);
    println!("ok: submit (draft → reviewing)");

    // Submitting a non-draft must fail.
    assert!(
        svc.submit_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
            .await
            .is_err(),
        "submitting a reviewing version must fail"
    );

    // ---- reviewing → online -----------------------------------------------
    let published = svc
        .publish_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
        .await
        .expect("publish");
    assert_eq!(published.summary.status, "online");
    assert_eq!(published.online_count, Some(2));
    assert_eq!(
        published
            .labels
            .as_ref()
            .and_then(|l| l.get("latest").cloned()),
        Some("2.0.0".to_string()),
        "the published version must become latest"
    );
    assert_eq!(published.reviewing_version, None);
    println!("ok: publish (reviewing → online)");

    // ---- online → offline --------------------------------------------------
    let offlined = svc
        .offline_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
        .await
        .expect("offline");
    assert_eq!(offlined.summary.status, "offline");
    assert_eq!(
        offlined
            .labels
            .as_ref()
            .and_then(|l| l.get("latest").cloned()),
        Some("1.0.0".to_string()),
        "the latest label must fall back to the remaining online version"
    );
    println!("ok: offline (latest falls back)");

    // Taking an offline version offline again must fail.
    assert!(
        svc.offline_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
            .await
            .is_err()
    );

    // ---- offline → online --------------------------------------------------
    let back = svc
        .online_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
        .await
        .expect("online");
    assert_eq!(back.summary.status, "online");
    assert_eq!(
        back.labels.as_ref().and_then(|l| l.get("latest").cloned()),
        Some("2.0.0".to_string())
    );
    println!("ok: online (offline → online)");

    // Bringing an online version online again must fail.
    assert!(
        svc.online_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
            .await
            .is_err()
    );

    // ---- redraft and force-publish ----------------------------------------
    let redrafted = svc
        .redraft_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
        .await
        .expect("redraft");
    assert_eq!(redrafted.summary.status, "draft");
    assert_eq!(redrafted.editing_version.as_deref(), Some("2.0.0"));
    println!("ok: redraft (online → draft)");

    // A draft cannot be published through the normal path.
    assert!(
        svc.publish_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
            .await
            .is_err(),
        "publishing a draft directly must fail"
    );

    // force-publish bypasses the review state check.
    let forced = svc
        .force_publish_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
        .await
        .expect("force publish");
    assert_eq!(forced.summary.status, "online");
    assert_eq!(
        forced
            .labels
            .as_ref()
            .and_then(|l| l.get("latest").cloned()),
        Some("2.0.0".to_string())
    );
    println!("ok: force-publish");

    // ---- labels ------------------------------------------------------------
    let mut labels = std::collections::HashMap::new();
    labels.insert("stable".to_string(), "1.0.0".to_string());
    let stored = svc
        .update_mcp_server_labels(NS, LIFECYCLE_NAME, labels)
        .await
        .expect("update labels");
    assert_eq!(
        stored.get("stable").map(String::as_str),
        Some("1.0.0"),
        "custom label must be stored"
    );
    assert_eq!(
        stored.get("latest").map(String::as_str),
        Some("2.0.0"),
        "the server-managed latest label must survive a custom-label update"
    );
    println!("ok: update labels (latest preserved)");

    // A label pointing at a version that is not online must be rejected.
    let mut bad = std::collections::HashMap::new();
    bad.insert("gone".to_string(), "9.9.9".to_string());
    assert!(
        svc.update_mcp_server_labels(NS, LIFECYCLE_NAME, bad)
            .await
            .is_err(),
        "labelling a non-online version must fail"
    );
    println!("ok: reject label on non-online version");

    // ---- status and scope --------------------------------------------------
    svc.update_mcp_server_status(NS, LIFECYCLE_NAME, false)
        .await
        .expect("disable");
    // Disabling is resource-level metadata; the server is still resolvable.
    assert!(
        svc.get_mcp_server_version(NS, LIFECYCLE_NAME, "2.0.0")
            .await
            .expect("get version")
            .is_some()
    );
    svc.update_mcp_server_status(NS, LIFECYCLE_NAME, true)
        .await
        .expect("enable");
    println!("ok: status enable/disable");

    svc.update_mcp_server_scope(NS, LIFECYCLE_NAME, "PUBLIC")
        .await
        .expect("set public scope");
    assert!(
        svc.update_mcp_server_scope(NS, LIFECYCLE_NAME, "BOGUS")
            .await
            .is_err(),
        "an unknown scope must be rejected"
    );
    println!("ok: scope update and validation");

    // ---- cleanup -----------------------------------------------------------
    svc.delete_mcp_server(NS, Some(LIFECYCLE_NAME), None, None)
        .await
        .expect("delete server");
    println!("ok: cleanup");
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
    index
        .refresh(store.as_ref() as &dyn batata_persistence::PersistenceService)
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
    assert_eq!(
        old.tools[0].name, "alpha",
        "old version keeps its own tools"
    );
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

    // ---- version list and detail -------------------------------------------
    // Version 1.0.0 was deleted above, so only 2.0.0 remains.
    let versions = svc
        .list_mcp_server_versions(NS, NAME, 1, 10)
        .await
        .expect("list versions");
    assert_eq!(versions.total_count, 1);
    let latest_entry = versions
        .page_items
        .iter()
        .find(|v| v.latest == Some(true))
        .expect("one version must be marked latest");
    assert_eq!(latest_entry.version, "2.0.0");

    let detail = svc
        .get_mcp_server_version(NS, NAME, "2.0.0")
        .await
        .expect("get version")
        .expect("version must exist");
    assert_eq!(detail.summary.version, "2.0.0");
    assert_eq!(detail.summary.status, "online");
    assert_eq!(detail.online_count, Some(1));
    assert_eq!(
        detail
            .labels
            .as_ref()
            .and_then(|l| l.get("latest").cloned()),
        Some("2.0.0".to_string()),
        "latest label must be exposed on the detail"
    );
    assert!(
        detail.server_specification.is_some(),
        "server specification must be returned"
    );
    assert!(!detail.writable, "a published version is not writable");
    println!("ok: version list and detail");

    // ---- draft lifecycle ---------------------------------------------------
    let draft = svc
        .create_mcp_server_draft(NS, &registration("3.0.0", "draft-tool"), false)
        .await
        .expect("create draft");
    assert_eq!(draft.summary.version, "3.0.0");
    assert_eq!(draft.summary.status, "draft");
    assert!(draft.writable, "a draft is writable");
    assert_eq!(draft.editing_version.as_deref(), Some("3.0.0"));
    assert_eq!(
        draft
            .tool_specification
            .as_ref()
            .and_then(|t| t.tools.first())
            .map(|t| t.name.as_str()),
        Some("draft-tool")
    );
    println!("ok: create draft");

    // A second draft without overwrite must be rejected.
    assert!(
        svc.create_mcp_server_draft(NS, &registration("4.0.0", "other"), false)
            .await
            .is_err(),
        "creating a second draft without overwrite must fail"
    );

    // With overwrite it replaces the existing draft.
    let replaced = svc
        .create_mcp_server_draft(NS, &registration("4.0.0", "other"), true)
        .await
        .expect("create draft with overwrite");
    assert_eq!(replaced.summary.version, "4.0.0");
    assert_eq!(replaced.editing_version.as_deref(), Some("4.0.0"));
    let versions = svc
        .list_mcp_server_versions(NS, NAME, 1, 10)
        .await
        .expect("list versions");
    assert!(
        !versions.page_items.iter().any(|v| v.version == "3.0.0"),
        "overwrite must remove the previous draft"
    );
    println!("ok: overwrite draft");

    // Update the draft in place.
    let mut updated_draft = registration("4.0.0", "renamed-tool");
    updated_draft.description = "draft changed".to_string();
    let updated = svc
        .update_mcp_server_draft(NS, &updated_draft)
        .await
        .expect("update draft");
    assert_eq!(
        updated.summary.description.as_deref(),
        Some("draft changed")
    );
    assert_eq!(
        updated
            .tool_specification
            .as_ref()
            .and_then(|t| t.tools.first())
            .map(|t| t.name.as_str()),
        Some("renamed-tool")
    );
    println!("ok: update draft");

    // Delete the draft; the editing marker is cleared.
    svc.delete_mcp_server_draft(NS, NAME, "4.0.0")
        .await
        .expect("delete draft");
    assert!(
        svc.get_mcp_server_version(NS, NAME, "4.0.0")
            .await
            .expect("get version")
            .is_none()
    );
    let after = svc
        .get_mcp_server_version(NS, NAME, "2.0.0")
        .await
        .expect("get version")
        .expect("published version survives");
    assert_eq!(
        after.editing_version, None,
        "deleting the draft must clear editingVersion"
    );
    println!("ok: delete draft");

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
