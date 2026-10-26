//! Shared helpers for the AI tests.
//!
//! Upstream Nacos runs every AI test against the database the server uses.
//! Batata's ports did the same, so all of them were `#[ignore]`d and never ran
//! in CI. They do not need SQL: they exercise the service layer, and the
//! embedded backend implements the same `PersistenceService` trait. So the
//! default is now a throwaway RocksDB store, and `DATABASE_URL` opts back into
//! a real database — which is how the row-level assertions still get exercised.

// Every test file compiles its own copy of this module and uses a different
// subset of it, so unused helpers are the normal case here.
#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use batata_persistence::sea_orm::{ConnectOptions, Database, DatabaseConnection};
use batata_persistence::{ExternalDbPersistService, PersistenceService};

/// How long to wait before declaring the test database unreachable.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Connect to the test database, failing fast with an actionable message.
///
/// A stopped Podman container leaves the forwarded port *accepting* TCP while
/// nothing completes the handshake, so a plain `Database::connect` blocks for
/// the default ~30s and only then reports a pool timeout. Across a file with
/// fifteen tests that is several minutes of silence before anything fails, and
/// the resulting message reads like a code bug rather than a missing database.
///
/// A short timeout surfaces the same situation in seconds, and the panic names
/// the likely cause.
pub async fn connect_database(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(CONNECT_TIMEOUT)
        .acquire_timeout(CONNECT_TIMEOUT);

    match Database::connect(options).await {
        Ok(connection) => connection,
        Err(error) => panic!(
            "cannot reach the test database at {url}: {error}\n\
             hint: containers do not come back on their own after a machine \
             restart or a podman machine stop — try `podman start mysql postgres`"
        ),
    }
}

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
                let connection = connect_database(&url).await;
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

    /// The store to hand to a service.
    pub fn store(&self) -> Arc<dyn PersistenceService> {
        self.store.clone()
    }

    /// Name of the backend, for the "which store am I on" log lines.
    pub fn backend(&self) -> String {
        match &self.backend {
            Backend::Embedded => "embedded".to_string(),
            Backend::External(store) => format!("{:?}", store.db().get_database_backend()),
        }
    }

    /// The raw connection, if there is one. The embedded backend has no SQL
    /// engine, so row-level assertions have to be skipped there.
    pub fn db(&self) -> Option<&DatabaseConnection> {
        match &self.backend {
            Backend::External(store) => Some(store.db()),
            Backend::Embedded => None,
        }
    }

    /// Remove the fixtures a test created, so a shared database does not leak
    /// state between runs. A fresh embedded store needs nothing.
    pub async fn clean(&self, name: &str) {
        self.clean_in("public", name).await
    }

    /// [`Self::clean_in`] plus the search index rows, for the tests that build
    /// one.
    pub async fn clean_index(&self, namespace: &str, resource_type: &str, name: &str) {
        self.clean_in(namespace, name).await;
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

    /// [`Self::clean`] for a namespace other than the default one.
    pub async fn clean_in(&self, namespace: &str, name: &str) {
        let Some(db) = self.db() else {
            return;
        };
        use batata_persistence::entity::{ai_resource, ai_resource_version};
        use batata_persistence::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        ai_resource_version::Entity::delete_many()
            .filter(ai_resource_version::Column::Name.eq(name))
            .filter(ai_resource_version::Column::NamespaceId.eq(namespace))
            .exec(db)
            .await
            .expect("clean versions");
        ai_resource::Entity::delete_many()
            .filter(ai_resource::Column::Name.eq(name))
            .filter(ai_resource::Column::NamespaceId.eq(namespace))
            .exec(db)
            .await
            .expect("clean resources");
    }
}
