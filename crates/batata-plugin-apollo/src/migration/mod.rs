use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::{prelude::*, MigratorTrait};
use sea_query::{Alias, DynIden, SeaRc};

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
        ]
    }
}

const APOLLO_MIGRATION_LOCK_NAME: &str = "apollo_migration";
const APOLLO_MIGRATION_LOCK_KEY: i64 = 0x6170_6f6c_6c6f_4d49u64 as i64;

pub async fn run_apollo_migrations_with_lock(
    db: &sea_orm::DatabaseConnection,
) -> Result<(), sea_orm::DbErr> {
    let backend = db.get_database_backend();
    tracing::info!("Running Apollo database migrations (acquiring advisory lock)...");

    acquire_migration_lock(db, backend).await?;
    let result = ApolloMigrator::up(db, None).await;

    if let Err(e) = release_migration_lock(db, backend).await {
        tracing::warn!("Failed to release Apollo migration advisory lock: {}", e);
    }

    result?;
    tracing::info!("Apollo database migrations completed successfully");
    Ok(())
}

async fn acquire_migration_lock(
    db: &sea_orm::DatabaseConnection,
    backend: sea_orm::DatabaseBackend,
) -> Result<(), sea_orm::DbErr> {
    match backend {
        DatabaseBackend::MySql => {
            db.execute(Statement::from_string(
                backend,
                format!("SELECT GET_LOCK('{}', 300)", APOLLO_MIGRATION_LOCK_NAME),
            ))
            .await?;
        }
        DatabaseBackend::Postgres => {
            db.execute(Statement::from_string(
                backend,
                format!("SELECT pg_advisory_lock({})", APOLLO_MIGRATION_LOCK_KEY),
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {}
    }
    Ok(())
}

async fn release_migration_lock(
    db: &sea_orm::DatabaseConnection,
    backend: sea_orm::DatabaseBackend,
) -> Result<(), sea_orm::DbErr> {
    match backend {
        DatabaseBackend::MySql => {
            db.execute(Statement::from_string(
                backend,
                format!("SELECT RELEASE_LOCK('{}')", APOLLO_MIGRATION_LOCK_NAME),
            ))
            .await?;
        }
        DatabaseBackend::Postgres => {
            db.execute(Statement::from_string(
                backend,
                format!("SELECT pg_advisory_unlock({})", APOLLO_MIGRATION_LOCK_KEY),
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {}
    }
    Ok(())
}
