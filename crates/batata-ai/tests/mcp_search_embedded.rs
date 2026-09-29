//! Standalone-mode test: the AI search index on **embedded RocksDB**.
//!
//! Nacos standalone runs on Derby (an embedded *SQL* database), so its search
//! index works there for free. Batata standalone (embedded) has no SQL engine —
//! it is RocksDB. This test proves the whole loop works on that backend too:
//! publish → schedule → consume → search.

use std::collections::HashMap;
use std::sync::Arc;

use batata_ai::model::{
    McpCapability, McpServerRegistration, McpServerType, McpTool, McpTransport,
};
use batata_ai::search::consumer::{AiResourceIndexConsumer, TaskOutcome};
use batata_ai::search::query;
use batata_ai::search::task;
use batata_ai::{McpServerIndex, McpServerOperationService};
use batata_consistency::raft::state_machine::{
    CF_AI_RESOURCE, CF_AI_RESOURCE_SEARCH_CHUNK, CF_AI_RESOURCE_SEARCH_DOCUMENT,
    CF_AI_RESOURCE_TASK, CF_AI_RESOURCE_VERSION,
};
use batata_persistence::{EmbeddedPersistService, PersistenceService};
use rocksdb::{ColumnFamilyDescriptor, DB, Options};

const NS: &str = "public";
const NAME: &str = "embedded-mcp";

/// Open a RocksDB instance with just the AI column families.
fn open(dir: &std::path::Path) -> Arc<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let names = [
        CF_AI_RESOURCE,
        CF_AI_RESOURCE_VERSION,
        CF_AI_RESOURCE_SEARCH_DOCUMENT,
        CF_AI_RESOURCE_SEARCH_CHUNK,
        CF_AI_RESOURCE_TASK,
    ];
    let descriptors: Vec<ColumnFamilyDescriptor> = names
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
        .collect();

    Arc::new(
        DB::open_cf_descriptors(&opts, dir, descriptors).expect("open RocksDB with AI CFs"),
    )
}

fn registration(version: &str) -> McpServerRegistration {
    McpServerRegistration {
        name: NAME.to_string(),
        display_name: "Embedded MCP".to_string(),
        description: "wombat wrangler".to_string(),
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

#[tokio::test]
async fn embedded_backend_runs_the_whole_search_loop() {
    let dir = tempfile::tempdir().expect("temp dir");
    let db = open(dir.path());
    let store: Arc<dyn PersistenceService> = Arc::new(EmbeddedPersistService::new(db));

    let index = Arc::new(McpServerIndex::new());
    let svc = McpServerOperationService::new(store.clone(), index);
    let consumer = AiResourceIndexConsumer::new(store.clone());

    // ---- create and publish ------------------------------------------------
    svc.create_mcp_server(NS, &registration("1.0.0"))
        .await
        .expect("create server on RocksDB");
    svc.publish_mcp_server_version(NS, NAME, "1.0.0")
        .await
        .expect("publish on RocksDB");

    let key = task::task_key(NS, "mcp", NAME);
    let scheduled = store
        .task_find(&key)
        .await
        .expect("read task")
        .expect("publish must have scheduled a task");
    assert_eq!(scheduled.task_type, "search_index");
    assert_eq!(scheduled.status, "pending");
    println!("ok: task scheduled on RocksDB");

    // ---- consume -----------------------------------------------------------
    let outcomes = consumer.consume_once().await.expect("consume on RocksDB");
    assert!(
        outcomes.contains(&TaskOutcome::Completed),
        "the task must complete, got {outcomes:?}"
    );

    let document = store
        .search_document_find(NS, "mcp", NAME, "1.0.0")
        .await
        .expect("read document")
        .expect("document must exist on RocksDB");
    assert_eq!(document.status, "enabled");
    assert_eq!(document.source_digest.len(), 64);

    let chunks = store
        .search_chunk_list(NS, "mcp", NAME, "1.0.0")
        .await
        .expect("read chunks");
    assert!(!chunks.is_empty(), "chunks must exist on RocksDB");
    println!("ok: index built with {} chunks", chunks.len());

    // ---- search ------------------------------------------------------------
    let hits = query::search(store.as_ref(), NS, "wombat", &["mcp"], 1, 10)
        .await
        .expect("keyword search on RocksDB");
    assert_eq!(hits.total_count, 1, "the description must be found");
    assert_eq!(hits.page_items[0].resource_name, NAME);
    println!("ok: keyword search found the resource");

    // ---- a term that matches nothing ---------------------------------------
    let hits = query::search(store.as_ref(), NS, "zzzznomatchzzzz", &["mcp"], 1, 10)
        .await
        .expect("search");
    assert_eq!(hits.total_count, 0);
    println!("ok: non-matching query returned nothing");
}
