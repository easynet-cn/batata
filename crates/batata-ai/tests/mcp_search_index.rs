//! Real-database test for the AI resource search index.
//!
//! Covers the `base_index` projection end to end: scheduling a task, building
//! the document and chunks, and the `source_digest` short circuit that makes
//! rebuilds incremental.
//!
//! Ignored by default: needs a reachable database with the migrations applied.
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-ai --test mcp_search_index -- --ignored --nocapture
//!
//! DATABASE_URL="postgres://postgres:devterry@127.0.0.1:5432/batata_ai_test" \
//!   cargo test -p batata-ai --test mcp_search_index -- --ignored --nocapture
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use batata_ai::model::{
    McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport,
};
use batata_ai::search::consumer::{AiResourceIndexConsumer, TaskOutcome};
use batata_ai::search::query;
use batata_ai::search::task;
use batata_ai::search::service::AiResourceSearchService;
use batata_ai::{McpServerIndex, McpServerOperationService};
use batata_persistence::{AiResourcePersistence, ExternalDbPersistService};
use batata_persistence::entity::{
    ai_resource, ai_resource_search_chunk, ai_resource_search_document, ai_resource_task,
    ai_resource_version,
};
use batata_persistence::sea_orm::{ColumnTrait, Database, EntityTrait, QueryFilter};

const NS: &str = "public";
const NAME: &str = "search-mcp";

fn registration(version: &str) -> McpServerRegistration {
    McpServerRegistration {
        name: NAME.to_string(),
        display_name: "Search MCP".to_string(),
        description: "indexed server".to_string(),
        namespace: NS.to_string(),
        version: version.to_string(),
        endpoint: "http://localhost:8080".to_string(),
        server_type: McpServerType::Http,
        transport: McpTransport::default(),
        capabilities: vec![McpCapability::Tool],
        tools: vec![McpTool {
            name: "echo".to_string(),
            description: "echo a string".to_string(),
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

async fn clean(store: &ExternalDbPersistService, resource_type: &str, name: &str) {
    let db = store.db();
    ai_resource_search_chunk::Entity::delete_many()
        .filter(ai_resource_search_chunk::Column::ResourceName.eq(name))
        .exec(db)
        .await
        .expect("clean chunks");
    ai_resource_search_document::Entity::delete_many()
        .filter(ai_resource_search_document::Column::ResourceName.eq(name))
        .exec(db)
        .await
        .expect("clean documents");
    ai_resource_task::Entity::delete_many()
        .filter(ai_resource_task::Column::TaskKey.eq(task::task_key(NS, resource_type, name)))
        .exec(db)
        .await
        .expect("clean tasks");
    ai_resource_version::Entity::delete_many()
        .filter(ai_resource_version::Column::Name.eq(name))
        .exec(db)
        .await
        .expect("clean versions");
    ai_resource::Entity::delete_many()
        .filter(ai_resource::Column::Name.eq(name))
        .exec(db)
        .await
        .expect("clean resources");
}

#[tokio::test]
#[ignore]
async fn schedule_then_rebuild_then_skip() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = Database::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect {url}: {e}"));
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let index = Arc::new(McpServerIndex::new());
    let svc = McpServerOperationService::new(store.clone(), index);
    let search = AiResourceSearchService::new(store.clone());

    clean(&store, "mcp", NAME).await;
    svc.create_mcp_server(NS, &registration("1.0.0"))
        .await
        .expect("create server");

    let resource = store
        .ai_resource_find(NS, NAME, "mcp")
        .await
        .expect("find resource")
        .expect("resource must exist");
    let row = store
        .ai_resource_version_find(NS, NAME, "mcp", "1.0.0")
        .await
        .expect("find version")
        .expect("version must exist");

    // ---- schedule ----------------------------------------------------------
    search.schedule(NS, "mcp", NAME).await.expect("schedule");
    let key = task::task_key(NS, "mcp", NAME);
    let scheduled = store
        .task_find(&key)
        .await
        .expect("read task")
        .expect("task must exist");
    assert_eq!(scheduled.task_type, "search_index");
    assert_eq!(scheduled.task_stage, "base_index");
    assert_eq!(scheduled.status, "pending");
    println!("ok: scheduled task {}", scheduled.task_key);

    // ---- rebuild -----------------------------------------------------------
    let rebuilt = search
        .rebuild_mcp_version(&resource, &row)
        .await
        .expect("rebuild");
    assert!(rebuilt, "first rebuild must write the index");

    let document = store
        .search_document_find(NS, "mcp", NAME, "1.0.0")
        .await
        .expect("read document")
        .expect("document must exist");
    assert_eq!(document.status, "enabled", "no vector index: enabled at once");
    assert_eq!(document.generate_mode, "auto");
    assert_eq!(document.source_digest.len(), 64);
    assert_eq!(document.display_name, NAME);

    let chunks = store
        .search_chunk_list(NS, "mcp", NAME, "1.0.0")
        .await
        .expect("read chunks");
    assert!(!chunks.is_empty(), "chunks must be written");
    assert!(
        chunks.iter().any(|c| c.chunk_type == "description"),
        "a description chunk is always produced"
    );
    assert!(
        chunks.iter().any(|c| c.chunk_type == "mcp_content"),
        "MCP source content must be indexed"
    );
    assert!(
        chunks.iter().all(|c| c.document_id == document.id),
        "chunks must point at their document"
    );
    println!("ok: rebuilt document {} with {} chunks", document.id, chunks.len());

    // ---- rebuild again: incremental short circuit --------------------------
    let rebuilt_again = search
        .rebuild_mcp_version(&resource, &row)
        .await
        .expect("rebuild");
    assert!(
        !rebuilt_again,
        "an unchanged source digest must skip the rebuild"
    );
    println!("ok: unchanged rebuild skipped");

    clean(&store, "mcp", NAME).await;
}

/// The consumer must turn a scheduled task into a built index.
#[tokio::test]
#[ignore]
async fn consumer_builds_index_and_completes_task() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = Database::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect {url}: {e}"));
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let index = Arc::new(McpServerIndex::new());
    let svc = McpServerOperationService::new(store.clone(), index);
    let consumer = AiResourceIndexConsumer::new(store.clone());

    clean(&store, "mcp", NAME).await;

    // Publishing schedules the task on its own.
    svc.create_mcp_server(NS, &registration("1.0.0"))
        .await
        .expect("create server");
    svc.publish_mcp_server_version(NS, NAME, "1.0.0")
        .await
        .expect("publish");

    let key = task::task_key(NS, "mcp", NAME);
    let scheduled = store
        .task_find(&key)
        .await
        .expect("read task")
        .expect("publish must have scheduled a task");
    assert_eq!(scheduled.status, "pending");
    println!("ok: publish scheduled the task");

    // ---- consume -----------------------------------------------------------
    let outcomes = consumer.consume_once().await.expect("consume");
    let built = outcomes
        .iter()
        .filter(|o| **o == TaskOutcome::Completed)
        .count();
    assert!(built >= 1, "at least one task must complete: {outcomes:?}");

    let document = store
        .search_document_find(NS, "mcp", NAME, "1.0.0")
        .await
        .expect("read document")
        .expect("consumer must have built the document");
    assert_eq!(document.status, "enabled");
    let chunks = store
        .search_chunk_list(NS, "mcp", NAME, "1.0.0")
        .await
        .expect("read chunks");
    assert!(!chunks.is_empty());
    println!("ok: consumer built {} chunks", chunks.len());

    let finished = store
        .task_find(&key)
        .await
        .expect("read task")
        .expect("task must exist");
    assert_eq!(finished.status, "completed");
    assert!(
        finished.lease_expire_at.is_none(),
        "a completed task must release its lease"
    );

    // ---- consume again: completed tasks are skipped ------------------------
    let outcomes = consumer.consume_once().await.expect("consume");
    assert!(
        outcomes.iter().all(|o| *o == TaskOutcome::Skipped),
        "completed tasks must be skipped: {outcomes:?}"
    );
    println!("ok: second poll skipped the completed task");

    clean(&store, "mcp", NAME).await;
}

/// A task for a resource that no longer exists must be removed, not retried.
#[tokio::test]
#[ignore]
async fn consumer_removes_task_for_missing_resource() {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = Database::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect {url}: {e}"));
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let search = AiResourceSearchService::new(store.clone());
    let consumer = AiResourceIndexConsumer::new(store.clone());

    clean(&store, "mcp", NAME).await;

    // Schedule for a resource that was never created.
    search.schedule(NS, "mcp", NAME).await.expect("schedule");
    let key = task::task_key(NS, "mcp", NAME);

    let outcomes = consumer.consume_once().await.expect("consume");
    let removed = outcomes
        .iter()
        .filter(|o| **o == TaskOutcome::Removed)
        .count();
    assert!(removed >= 1, "missing resource must remove the task: {outcomes:?}");

    assert!(
        store.task_find(&key).await.expect("read task").is_none(),
        "the task must be gone"
    );
    println!("ok: task removed for missing resource");

    clean(&store, "mcp", NAME).await;
}

/// Keyword search must find the right resources and rank them by score.
///
/// This also exercises the raw SQL on both dialects, where PostgreSQL numbers
/// placeholders and MySQL uses `?`.
#[tokio::test]
#[ignore]
async fn keyword_search_finds_and_ranks() {
    const ALPHA: &str = "search-alpha";
    const BETA: &str = "search-beta";

    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| panic!("DATABASE_URL must be set for this ignored test"));
    let conn = Database::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect {url}: {e}"));
    let store = Arc::new(ExternalDbPersistService::new(conn));
    let index = Arc::new(McpServerIndex::new());
    let svc = McpServerOperationService::new(store.clone(), index);
    let search = AiResourceSearchService::new(store.clone());

    clean(&store, "mcp", ALPHA).await;
    clean(&store, "mcp", BETA).await;

    // Two servers: only ALPHA mentions "wombat".
    for (name, description) in [(ALPHA, "wombat wrangler"), (BETA, "platypus painter")] {
        let mut reg = registration("1.0.0");
        reg.name = name.to_string();
        reg.description = description.to_string();
        svc.create_mcp_server(NS, &reg).await.expect("create");

        let resource = store
            .ai_resource_find(NS, name, "mcp")
            .await
            .expect("find resource")
            .expect("resource must exist");
        let row = store
            .ai_resource_version_find(NS, name, "mcp", "1.0.0")
            .await
            .expect("find version")
            .expect("version must exist");
        search
            .rebuild_mcp_version(&resource, &row)
            .await
            .expect("rebuild");
    }

    // ---- a term unique to one resource -------------------------------------
    let page = query::search(&*store, NS, "wombat", &["mcp"], 1, 10)
        .await
        .expect("search");
    assert_eq!(page.total_count, 1, "only ALPHA mentions wombat");
    assert_eq!(page.page_items[0].resource_name, ALPHA);
    println!("ok: unique term matched one resource");

    // ---- a term shared by both ---------------------------------------------
    let page = query::search(&*store, NS, "mcp", &["mcp"], 1, 10)
        .await
        .expect("search");
    assert!(
        page.total_count >= 2,
        "both resources should match a shared term, got {}",
        page.total_count
    );
    println!("ok: shared term matched {} resources", page.total_count);

    // ---- pagination --------------------------------------------------------
    let first = query::search(&*store, NS, "mcp", &["mcp"], 1, 1)
        .await
        .expect("search");
    assert_eq!(first.page_items.len(), 1, "page size must be honoured");
    let second = query::search(&*store, NS, "mcp", &["mcp"], 2, 1)
        .await
        .expect("search");
    if second.page_items.is_empty() {
        assert!(
            first.page_items.is_empty() || first.total_count <= 1,
            "a second page must exist when there are more results"
        );
    } else {
        assert_ne!(
            first.page_items[0].resource_name, second.page_items[0].resource_name,
            "pages must not repeat the same resource"
        );
    }
    println!("ok: pagination honoured");

    // ---- an empty query returns nothing ------------------------------------
    let page = query::search(&*store, NS, "   ", &["mcp"], 1, 10)
        .await
        .expect("search");
    assert_eq!(page.total_count, 0, "a blank query must not match");
    println!("ok: blank query returned nothing");

    clean(&store, "mcp", ALPHA).await;
    clean(&store, "mcp", BETA).await;
}
