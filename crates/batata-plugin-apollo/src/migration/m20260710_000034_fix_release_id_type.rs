use sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let conn = manager.get_connection();

        match backend {
            sea_orm::DatabaseBackend::MySql => {
                conn.execute(Statement::from_string(
                    backend,
                    "ALTER TABLE apollo_release MODIFY COLUMN release_id BIGINT NULL DEFAULT NULL".to_string(),
                ))
                .await?;
            }
            sea_orm::DatabaseBackend::Postgres => {
                conn.execute(Statement::from_string(
                    backend,
                    "ALTER TABLE apollo_release ALTER COLUMN release_id TYPE BIGINT".to_string(),
                ))
                .await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
