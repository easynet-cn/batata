//! A `PersistenceService` for the AI route tests.
//!
//! These tests exist because upstream Nacos tests every AI controller against
//! a real database. Batata's ports did the same, which meant every one of them
//! had to be `#[ignore]`d: CI has no database, so 51 of them never ran at all.
//!
//! They do not actually need SQL. They exercise the HTTP layer and the service
//! layer above it, and the embedded backend implements the same
//! `PersistenceService` trait as the SQL one. So the default is now a
//! throwaway RocksDB store, and `DATABASE_URL` opts back into the real thing —
//! which is how the row-level assertions still get exercised:
//!
//! ```bash
//! DATABASE_URL="mysql://root:devterry@127.0.0.1:3306/batata_ai_test" \
//!   cargo test -p batata-server --test prompt_console_routes
//! ```

use std::sync::Arc;

use batata_persistence::{ExternalDbPersistService, PersistenceService};

/// Which backend a test landed on.
enum Backend {
    /// RocksDB, opened fresh for this one test.
    Embedded,
    /// A migrated database named by `DATABASE_URL`.
    External(Arc<ExternalDbPersistService>),
}

/// The store a test runs against, plus the handle it needs to clean up.
///
/// The temp dir is kept alive for as long as the store is: dropping it deletes
/// the RocksDB directory, which is what makes each test independent.
pub struct TestStore {
    store: Arc<dyn PersistenceService>,
    backend: Backend,
    _dir: Option<tempfile::TempDir>,
}

impl TestStore {
    /// Embedded RocksDB, or the database `DATABASE_URL` points at.
    pub async fn open() -> Self {
        match std::env::var("DATABASE_URL") {
            Ok(url) => {
                let connection = connect(&url).await;
                let store = Arc::new(ExternalDbPersistService::new(connection));
                Self {
                    store: store.clone(),
                    backend: Backend::External(store),
                    _dir: None,
                }
            }
            Err(_) => {
                let dir = tempfile::tempdir().expect("temp dir");
                let machine = batata_consistency::RocksStateMachine::new(dir.path())
                    .await
                    .expect("state machine");
                let store: Arc<dyn PersistenceService> = Arc::new(
                    batata_persistence::EmbeddedPersistService::from_state_machine(&machine),
                );
                Self {
                    store,
                    backend: Backend::Embedded,
                    _dir: Some(dir),
                }
            }
        }
    }

    /// The store to hand to a service or an `AppState`.
    pub fn store(&self) -> Arc<dyn PersistenceService> {
        self.store.clone()
    }

    /// The raw connection, if there is one. The embedded backend has no SQL
    /// engine, so row-level assertions have to be skipped there.
    pub fn db(&self) -> Option<&sea_orm::DatabaseConnection> {
        match &self.backend {
            Backend::External(store) => Some(store.db()),
            Backend::Embedded => None,
        }
    }

    /// True when row-level assertions are available.
    #[allow(dead_code)] // only the tests that check raw rows use this.
    pub fn has_sql(&self) -> bool {
        self.db().is_some()
    }

    /// Remove the fixtures a test created, so a shared database does not leak
    /// state between runs. A fresh embedded store needs nothing.
    pub async fn clean(&self, name: &str) {
        let Some(db) = self.db() else {
            return;
        };
        use batata_persistence::entity::{ai_resource, ai_resource_version};
        use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

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

    /// [`Self::clean`] plus the search index rows, for the tests that build one.
    #[allow(dead_code)] // only the search test builds an index.
    pub async fn clean_index(&self, namespace: &str, name: &str, resource_type: &str) {
        self.clean(name).await;
        let Some(db) = self.db() else {
            return;
        };
        use batata_persistence::entity::{
            ai_resource_search_chunk, ai_resource_search_document, ai_resource_task,
        };
        use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

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
        let task_key = batata_ai::search::task::task_key(namespace, resource_type, name);
        ai_resource_task::Entity::delete_many()
            .filter(ai_resource_task::Column::TaskKey.eq(task_key))
            .exec(db)
            .await
            .expect("clean tasks");
    }
}

/// Connect to the test database, failing fast with an actionable message.
///
/// A stopped container leaves the forwarded port *accepting* TCP while nothing
/// completes the handshake, so the default timeout turns a missing database
/// into half a minute of silence and then an error that reads like a code bug.
pub async fn connect(url: &str) -> sea_orm::DatabaseConnection {
    use std::time::Duration;

    let mut options = sea_orm::ConnectOptions::new(url.to_string());
    options
        .connect_timeout(Duration::from_secs(3))
        .acquire_timeout(Duration::from_secs(3));

    match sea_orm::Database::connect(options).await {
        Ok(connection) => connection,
        Err(error) => panic!(
            "cannot reach the test database at {url}: {error}\n\
             hint: containers do not come back on their own after a machine \
             restart or a podman machine stop — try `podman start mysql postgres`"
        ),
    }
}
