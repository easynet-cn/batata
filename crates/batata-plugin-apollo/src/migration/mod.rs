use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::{MigratorTrait, prelude::*};
use sea_query::{Alias, DynIden, Expr, Query, SeaRc, SelectStatement};

/// Column name helper utilities for migration statements.
pub mod column_helper;
mod m20260710_000001_create_apollo_app;
mod m20260710_000002_create_apollo_app_namespace;
mod m20260710_000003_create_apollo_cluster;
mod m20260710_000004_create_apollo_namespace;
mod m20260710_000005_create_apollo_item;
mod m20260710_000006_create_apollo_release;
mod m20260710_000007_create_apollo_commit;
mod m20260710_000008_create_apollo_release_message;
mod m20260710_000009_create_apollo_gray_release_rule;
mod m20260710_000010_create_apollo_instance;
mod m20260710_000011_create_apollo_instance_config;
mod m20260710_000012_create_apollo_namespace_lock;
mod m20260710_000013_create_apollo_audit;
mod m20260710_000014_create_apollo_server_config;
mod m20260710_000015_create_apollo_consumer;
mod m20260710_000016_create_apollo_consumer_audit;
mod m20260710_000017_create_apollo_consumer_role;
mod m20260710_000018_create_apollo_consumer_token;
mod m20260710_000019_create_apollo_favorite;
mod m20260710_000020_create_apollo_permission;
mod m20260710_000021_create_apollo_role;
mod m20260710_000022_create_apollo_role_permission;
mod m20260710_000023_create_apollo_user_role;
mod m20260710_000024_create_apollo_users;
mod m20260710_000025_create_apollo_release_history;
mod m20260710_000026_create_apollo_access_key;
mod m20260710_000027_create_apollo_service_registry;
mod m20260710_000028_create_apollo_audit_log;
mod m20260710_000029_create_apollo_audit_log_data_influence;
mod m20260710_000030_create_apollo_user_token;
mod m20260710_000031_create_apollo_user_token_audit;
mod m20260710_000032_create_apollo_authorities;
mod m20260710_000033_seed_initial_data;
mod m20260710_000034_fix_release_id_type;
mod m20260710_000035_add_namespace_fields;

/// Represents the `ApolloMigrator` entity.
pub struct ApolloMigrator;

#[async_trait::async_trait]
impl MigratorTrait for ApolloMigrator {
    fn migration_table_name() -> DynIden {
        SeaRc::new(Alias::new("seaql_migrations_apollo"))
    }

    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260710_000001_create_apollo_app::Migration),
            Box::new(m20260710_000002_create_apollo_app_namespace::Migration),
            Box::new(m20260710_000003_create_apollo_cluster::Migration),
            Box::new(m20260710_000004_create_apollo_namespace::Migration),
            Box::new(m20260710_000005_create_apollo_item::Migration),
            Box::new(m20260710_000006_create_apollo_release::Migration),
            Box::new(m20260710_000007_create_apollo_commit::Migration),
            Box::new(m20260710_000008_create_apollo_release_message::Migration),
            Box::new(m20260710_000009_create_apollo_gray_release_rule::Migration),
            Box::new(m20260710_000010_create_apollo_instance::Migration),
            Box::new(m20260710_000011_create_apollo_instance_config::Migration),
            Box::new(m20260710_000012_create_apollo_namespace_lock::Migration),
            Box::new(m20260710_000013_create_apollo_audit::Migration),
            Box::new(m20260710_000014_create_apollo_server_config::Migration),
            Box::new(m20260710_000015_create_apollo_consumer::Migration),
            Box::new(m20260710_000016_create_apollo_consumer_audit::Migration),
            Box::new(m20260710_000017_create_apollo_consumer_role::Migration),
            Box::new(m20260710_000018_create_apollo_consumer_token::Migration),
            Box::new(m20260710_000019_create_apollo_favorite::Migration),
            Box::new(m20260710_000020_create_apollo_permission::Migration),
            Box::new(m20260710_000021_create_apollo_role::Migration),
            Box::new(m20260710_000022_create_apollo_role_permission::Migration),
            Box::new(m20260710_000023_create_apollo_user_role::Migration),
            Box::new(m20260710_000024_create_apollo_users::Migration),
            Box::new(m20260710_000025_create_apollo_release_history::Migration),
            Box::new(m20260710_000026_create_apollo_access_key::Migration),
            Box::new(m20260710_000027_create_apollo_service_registry::Migration),
            Box::new(m20260710_000028_create_apollo_audit_log::Migration),
            Box::new(m20260710_000029_create_apollo_audit_log_data_influence::Migration),
            Box::new(m20260710_000030_create_apollo_user_token::Migration),
            Box::new(m20260710_000031_create_apollo_user_token_audit::Migration),
            Box::new(m20260710_000032_create_apollo_authorities::Migration),
            Box::new(m20260710_000033_seed_initial_data::Migration),
            Box::new(m20260710_000034_fix_release_id_type::Migration),
            Box::new(m20260710_000035_add_namespace_fields::Migration),
        ]
    }
}

const APOLLO_MIGRATION_LOCK_NAME: &str = "apollo_migration";
const APOLLO_MIGRATION_LOCK_KEY: i64 = 0x6170_6f6c_6c6f_4d49u64 as i64;

/// Upper bound on how long a node waits for a peer to finish migrating.
///
/// Must stay well below the cluster startup readiness window so a stuck lock
/// surfaces as an error instead of an unresponsive node.
const APOLLO_MIGRATION_LOCK_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);
/// Backoff between two lock attempts.
const APOLLO_MIGRATION_LOCK_RETRY: std::time::Duration = std::time::Duration::from_millis(200);

/// Performs the `run_apollo_migrations_with_lock` operation.
///
/// All cluster nodes share one database, so several of them race to apply the
/// same migrations at startup. Two properties matter here:
///
/// * **Never block on the lock.** The lock lives on a pooled connection, and
///   `RELEASE_LOCK` / `pg_advisory_unlock` are no-ops when they come from a
///   different connection than the one that acquired it. A node that waits on
///   a lock nobody can release hangs forever (observed: 3-node cluster stuck
///   past the startup readiness window). Acquisition is therefore a *try* with
///   a bounded overall deadline.
/// * **A completed peer is the common case.** Before and during the wait the
///   pending set is re-read; as soon as another node has applied the
///   migrations this node returns without ever running DDL.
pub async fn run_apollo_migrations_with_lock(
    db: &sea_orm::DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
    let backend = db.get_database_backend();

    // Fast path: nothing to apply, so no lock contention at all. This is the
    // steady state on every restart.
    if pending_count(db).await? == 0 {
        return Ok(());
    }

    tracing::info!("Running Apollo database migrations (acquiring advisory lock)...");

    let deadline = std::time::Instant::now() + APOLLO_MIGRATION_LOCK_DEADLINE;
    loop {
        // A peer may have finished while we were waiting.
        if pending_count(db).await? == 0 {
            tracing::info!("Apollo database migrations already applied by a peer node");
            return Ok(());
        }

        match try_acquire_migration_lock(db, backend).await {
            Ok(true) => {
                let result = if pending_count(db).await? == 0 {
                    Ok(())
                } else {
                    ApolloMigrator::up(db, None).await
                };
                if let Err(e) = release_migration_lock(db, backend).await {
                    tracing::warn!("Failed to release Apollo migration advisory lock: {}", e);
                }
                result?;
                tracing::info!("Apollo database migrations completed successfully");
                return Ok(());
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("Failed to acquire Apollo migration advisory lock: {}", e);
            }
        }

        if std::time::Instant::now() >= deadline {
            return Err(sea_orm::DbErr::Custom(format!(
                "Timed out after {}s waiting for the Apollo migration lock",
                APOLLO_MIGRATION_LOCK_DEADLINE.as_secs()
            )));
        }
        tokio::time::sleep(APOLLO_MIGRATION_LOCK_RETRY).await;
    }
}

/// Number of migrations that have not been applied yet.
async fn pending_count(db: &sea_orm::DatabaseConnection) -> Result<usize, sea_orm::DbErr> {
    Ok(ApolloMigrator::get_pending_migrations_read_only(db)
        .await?
        .len())
}

/// Try to take the migration lock without waiting for it.
async fn try_acquire_migration_lock(
    db: &sea_orm::DatabaseConnection,
    backend: sea_orm::DatabaseBackend,
) -> Result<bool, sea_orm::DbErr> {
    match backend {
        DatabaseBackend::MySql => {
            // Timeout 0 makes GET_LOCK a non-blocking try-lock.
            let stmt = lock_select(&format!("GET_LOCK('{}', 0)", APOLLO_MIGRATION_LOCK_NAME));
            read_bool(db, &stmt, backend).await
        }
        DatabaseBackend::Postgres => {
            let stmt = lock_select(&format!(
                "pg_try_advisory_lock({})",
                APOLLO_MIGRATION_LOCK_KEY
            ));
            read_bool(db, &stmt, backend).await
        }
        // Embedded storage has a single writer; no coordination needed.
        DatabaseBackend::Sqlite => Ok(true),
        _ => Ok(true),
    }
}

/// `SELECT <expr> AS locked` — sea-orm's `query_one` takes a
/// `StatementBuilder`, so the raw lock call is wrapped in a `SelectStatement`.
fn lock_select(expr: &str) -> SelectStatement {
    Query::select()
        .expr_as(Expr::cust(expr.to_string()), Alias::new("locked"))
        .take()
}

/// Read the first column of a one-row statement as a boolean-ish value.
async fn read_bool(
    db: &sea_orm::DatabaseConnection,
    stmt: &SelectStatement,
    backend: sea_orm::DatabaseBackend,
) -> Result<bool, sea_orm::DbErr> {
    let row = match db.query_one(stmt).await? {
        Some(row) => row,
        None => return Ok(false),
    };
    match backend {
        // MySQL returns 1 / 0 / NULL.
        DatabaseBackend::MySql => {
            let value: Option<i32> = row.try_get("", "locked")?;
            Ok(value == Some(1))
        }
        // Postgres returns a bool.
        _ => {
            let value: Option<bool> = row.try_get("", "locked")?;
            Ok(value.unwrap_or(false))
        }
    }
}

async fn release_migration_lock(
    db: &sea_orm::DatabaseConnection,
    backend: sea_orm::DatabaseBackend,
) -> Result<(), sea_orm::DbErr> {
    match backend {
        DatabaseBackend::MySql => {
            db.execute_raw(Statement::from_string(
                backend,
                format!("SELECT RELEASE_LOCK('{}')", APOLLO_MIGRATION_LOCK_NAME),
            ))
            .await?;
        }
        DatabaseBackend::Postgres => {
            db.execute_raw(Statement::from_string(
                backend,
                format!("SELECT pg_advisory_unlock({})", APOLLO_MIGRATION_LOCK_KEY),
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {}
        _ => {}
    }
    Ok(())
}
