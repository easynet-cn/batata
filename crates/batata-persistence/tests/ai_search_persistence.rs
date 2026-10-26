//! Behaviour tests for the AI search index persistence.
//!
//! These exercise the operations that are easy to get subtly wrong across
//! backends — upserts (MySQL `ON DUPLICATE KEY UPDATE` vs PostgreSQL
//! `ON CONFLICT ... DO UPDATE` vs RocksDB's single write per key) and
//! delete-then-reinsert chunk replacement.
//!
//! Runs on the embedded backend; set `DATABASE_URL` to run the same assertions
//! against a real database, which is the only way the dialect-specific SQL is
//! exercised:
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-persistence --test ai_search_persistence -- --nocapture
//! ```

use std::sync::Arc;

use batata_persistence::entity::{
    ai_resource_search_chunk, ai_resource_search_document, ai_resource_task,
};
use batata_persistence::model::{
    AiResourceSearchChunkInfo, AiResourceSearchDocumentInfo, AiResourceTaskInfo,
};
use batata_persistence::sea_orm::{
    ColumnTrait, ConnectOptions, Database, DatabaseConnection, EntityTrait, QueryFilter,
};
use batata_persistence::{EmbeddedPersistService, ExternalDbPersistService, PersistenceService};

/// The backend the test runs against: embedded, unless `DATABASE_URL` is set.
///
/// Only the external variant can be asked for a raw connection, which is what
/// the row-level cleanup needs; a fresh embedded store starts empty, so it
/// needs no cleanup at all.
///
/// The temp dir is owned but never read — it exists to keep the RocksDB
/// directory alive for as long as the store is.
#[allow(dead_code)]
enum Store {
    Embedded(Arc<EmbeddedPersistService>, tempfile::TempDir),
    External(Arc<ExternalDbPersistService>),
}

impl Store {
    async fn open() -> Self {
        match std::env::var("DATABASE_URL") {
            Ok(url) => Self::External(Arc::new(ExternalDbPersistService::new(
                connect_database(&url).await,
            ))),
            Err(_) => {
                let dir = tempfile::tempdir().expect("temp dir");
                let db = open_rocksdb(dir.path());
                Self::Embedded(Arc::new(EmbeddedPersistService::new(db)), dir)
            }
        }
    }

    /// The service every assertion goes through.
    fn service(&self) -> Arc<dyn PersistenceService> {
        match self {
            Self::Embedded(store, _) => store.clone(),
            Self::External(store) => store.clone(),
        }
    }

    fn backend(&self) -> String {
        match self {
            Self::Embedded(..) => "embedded".to_string(),
            Self::External(store) => format!("{:?}", store.db().get_database_backend()),
        }
    }

    /// Remove rows left by a previous run so the test is repeatable.
    async fn clean(&self) {
        let Self::External(store) = self else {
            return;
        };
        let conn = store.db();
        ai_resource_search_chunk::Entity::delete_many()
            .filter(ai_resource_search_chunk::Column::ResourceName.eq(R_NAME))
            .exec(conn)
            .await
            .expect("clean chunks");
        ai_resource_search_document::Entity::delete_many()
            .filter(ai_resource_search_document::Column::ResourceName.eq(R_NAME))
            .exec(conn)
            .await
            .expect("clean documents");
        ai_resource_task::Entity::delete_many()
            .filter(ai_resource_task::Column::TaskType.eq("search_index"))
            .exec(conn)
            .await
            .expect("clean tasks");
    }
}

/// Open a RocksDB instance with just the column families this test needs.
fn open_rocksdb(dir: &std::path::Path) -> Arc<rocksdb::DB> {
    use rocksdb::{ColumnFamilyDescriptor, Options};

    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    let names = [
        "batata_ai_resource",
        "batata_ai_resource_version",
        "batata_ai_resource_search_document",
        "batata_ai_resource_search_chunk",
        "batata_ai_resource_task",
    ];
    let descriptors: Vec<ColumnFamilyDescriptor> = names
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
        .collect();

    Arc::new(
        rocksdb::DB::open_cf_descriptors(&options, dir, descriptors)
            .expect("open RocksDB with AI column families"),
    )
}

/// Connect to the test database, failing fast with an actionable message.
///
/// A stopped Podman container leaves the forwarded port *accepting* TCP while
/// nothing completes the handshake, so a plain `Database::connect` blocks for
/// the default ~30s and then reports a pool timeout.
async fn connect_database(url: &str) -> DatabaseConnection {
    use std::time::Duration;
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(3))
        .acquire_timeout(Duration::from_secs(3));
    match Database::connect(options).await {
        Ok(connection) => connection,
        Err(error) => panic!(
            "cannot reach the test database at {url}: {error}\n\
             hint: containers do not come back on their own after a machine \
             restart or a podman machine stop — try `podman start mysql postgres`"
        ),
    }
}

const NS: &str = "public";
const R_TYPE: &str = "skill";
const R_NAME: &str = "probe-skill";
const R_VERSION: &str = "1.0.0";

fn document() -> AiResourceSearchDocumentInfo {
    AiResourceSearchDocumentInfo {
        id: 0,
        namespace_id: NS.to_string(),
        resource_type: R_TYPE.to_string(),
        resource_name: R_NAME.to_string(),
        resource_version: R_VERSION.to_string(),
        display_name: "Probe Skill".to_string(),
        description: Some("initial".to_string()),
        tags: Some("[\"a\"]".to_string()),
        capabilities: None,
        representative_queries: None,
        metadata: None,
        source_digest: "digest-1".to_string(),
        status: "pending".to_string(),
        generate_mode: "auto".to_string(),
        gmt_create: None,
        gmt_modified: None,
    }
}

fn chunk(suffix: &str) -> AiResourceSearchChunkInfo {
    AiResourceSearchChunkInfo {
        id: 0,
        document_id: 0,
        namespace_id: NS.to_string(),
        resource_type: R_TYPE.to_string(),
        resource_name: R_NAME.to_string(),
        resource_version: R_VERSION.to_string(),
        chunk_type: "description".to_string(),
        chunk_text: format!("text-{suffix}"),
        canonical_text: format!("canonical-{suffix}"),
        language: None,
        chunk_hash: format!("hash-{suffix}"),
        metadata: None,
        status: "enabled".to_string(),
        gmt_create: None,
        gmt_modified: None,
    }
}

fn task(key: &str, next_execute_at: i64) -> AiResourceTaskInfo {
    AiResourceTaskInfo {
        task_key: key.to_string(),
        namespace_id: NS.to_string(),
        task_type: "search_index".to_string(),
        task_stage: "base_index".to_string(),
        status: "pending".to_string(),
        task_payload: "{}".to_string(),
        task_result: None,
        retry_count: 0,
        revision: 1,
        lease_token: 0,
        next_execute_at,
        lease_expire_at: None,
        last_error: None,
        gmt_create: None,
        gmt_modified: None,
    }
}

#[tokio::test]
async fn search_index_persistence_round_trip() {
    let store = Store::open().await;
    let db = store.service();
    println!("--- backend: {} ---", store.backend());

    store.clean().await;

    // ---- search document: insert then upsert-update -------------------------
    let id = db
        .search_document_upsert(&document())
        .await
        .expect("document upsert (insert)");
    assert!(id > 0, "insert must return a positive id");

    let mut updated = document();
    updated.display_name = "Probe Skill v2".to_string();
    updated.source_digest = "digest-2".to_string();
    let id2 = db
        .search_document_upsert(&updated)
        .await
        .expect("document upsert (update)");
    assert_eq!(id, id2, "upsert must not create a second row");

    let found = db
        .search_document_find(NS, R_TYPE, R_NAME, R_VERSION)
        .await
        .expect("document find")
        .expect("document must exist");
    assert_eq!(found.display_name, "Probe Skill v2");
    assert_eq!(found.source_digest, "digest-2");
    println!("ok: document upsert id={id}");

    // ---- chunks: replace twice, second replaces rather than appends ---------
    let written = db
        .search_chunk_replace(NS, R_TYPE, R_NAME, R_VERSION, &[chunk("a"), chunk("b")])
        .await
        .expect("chunk replace");
    assert_eq!(written, 2);

    let chunks = db
        .search_chunk_list(NS, R_TYPE, R_NAME, R_VERSION)
        .await
        .expect("chunk list");
    assert_eq!(chunks.len(), 2, "expected two chunks");
    assert!(
        chunks.iter().all(|c| c.document_id == id),
        "chunks must reference the document id"
    );

    let written = db
        .search_chunk_replace(NS, R_TYPE, R_NAME, R_VERSION, &[chunk("c")])
        .await
        .expect("chunk replace (second)");
    assert_eq!(written, 1);
    let chunks = db
        .search_chunk_list(NS, R_TYPE, R_NAME, R_VERSION)
        .await
        .expect("chunk list (second)");
    assert_eq!(chunks.len(), 1, "replace must not append");
    assert_eq!(chunks[0].chunk_hash, "hash-c");
    println!("ok: chunk replace");

    // ---- tasks: upsert, find due, delete -----------------------------------
    let now = chrono::Utc::now().timestamp_millis();
    db.task_upsert(&task("due-task", now - 1_000))
        .await
        .expect("task upsert (due)");
    db.task_upsert(&task("future-task", now + 60_000))
        .await
        .expect("task upsert (future)");

    let found = db
        .task_find("due-task")
        .await
        .expect("task find")
        .expect("task must exist");
    assert_eq!(found.task_stage, "base_index");

    // Upsert again with a new stage; must update in place.
    let mut advanced = task("due-task", now - 1_000);
    advanced.task_stage = "llm_enhancement".to_string();
    db.task_upsert(&advanced)
        .await
        .expect("task upsert (update)");
    let found = db
        .task_find("due-task")
        .await
        .expect("task find")
        .expect("task must exist");
    assert_eq!(
        found.task_stage, "llm_enhancement",
        "upsert must update stage"
    );

    let due = db
        .task_find_due("search_index", now, 100)
        .await
        .expect("task find due");
    assert_eq!(due.len(), 1, "only the due task should be returned");
    assert_eq!(due[0].task_key, "due-task");
    println!("ok: task find_due returned {}", due.len());

    let deleted = db.task_delete("due-task").await.expect("task delete");
    assert_eq!(deleted, 1);
    assert!(db.task_find("due-task").await.expect("task find").is_none());
    println!("ok: task delete");

    // ---- cleanup -----------------------------------------------------------
    assert_eq!(
        db.search_document_delete(NS, R_TYPE, R_NAME, R_VERSION)
            .await
            .expect("document delete"),
        1
    );
    println!("ok: document delete");
}
