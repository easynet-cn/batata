#![allow(dead_code)]

use std::sync::Arc;

use batata_consistency::raft::state_machine::RocksStateMachine;
use batata_plugin_apollo::persistence::embedded::EmbeddedApolloPersistence;
use batata_plugin_apollo::persistence::sql::SqlApolloPersistence;
use batata_plugin_apollo::persistence::traits::ApolloPersistenceService;

/// Open a temporary RocksDB (with all Apollo column families registered) and
/// return an `EmbeddedApolloPersistence` backed by it. Used by self-contained
/// persistence + route tests so they do not need a running server.
pub async fn make_embedded_persistence() -> Arc<dyn ApolloPersistenceService> {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let sm = RocksStateMachine::with_options_and_cfs(tmp.path(), None, None, &[])
        .await
        .expect("open RocksStateMachine");
    // Keep the on-disk database alive for the test's lifetime.
    std::mem::forget(tmp);
    Arc::new(EmbeddedApolloPersistence::new(sm.db()))
}

/// Same as [`make_embedded_persistence`] but at a caller-chosen path (needed
/// for restart-recovery tests that reopen the same directory).
pub async fn make_embedded_persistence_at(
    path: &std::path::Path,
) -> Arc<dyn ApolloPersistenceService> {
    let sm = RocksStateMachine::with_options_and_cfs(path, None, None, &[])
        .await
        .expect("open RocksStateMachine");
    Arc::new(EmbeddedApolloPersistence::new(sm.db()))
}

/// Optional SQL persistence. Returns `None` unless `TEST_DATABASE_URL` is set.
///
/// NOTE: batata has no SQLite support, so this requires a live MySQL/PostgreSQL
/// (mirrors `batata-integration-tests`'s `TestDatabase`). SQL tests self-skip
/// when the env var is absent.
pub async fn make_sql_persistence() -> Option<Arc<dyn ApolloPersistenceService>> {
    use sea_orm_migration::MigratorTrait;
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let conn = sea_orm::Database::connect(url.as_str()).await.ok()?;
    batata_plugin_apollo::migration::ApolloMigrator::up(&conn, None)
        .await
        .ok()?;
    Some(Arc::new(SqlApolloPersistence::new(conn)))
}
