use sea_orm_migration::prelude::*;

use crate::migration::column_helper::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();

        manager
            .create_table(
                Table::create()
                    .table(ApolloNamespaceLock::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloNamespaceLock::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloNamespaceLock::AppId, 64, "default"))
                    .col(string_len_default(ApolloNamespaceLock::ClusterName, 32, "default"))
                    .col(string_len_default(ApolloNamespaceLock::NamespaceName, 128, "default"))
                    .col(string_len_default(ApolloNamespaceLock::LockedBy, 64, "default"))
                    .col(date_time(ApolloNamespaceLock::LockedAt))
                    .col(string_len_default(ApolloNamespaceLock::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloNamespaceLock::DataChangeCreatedTime))
                    .col(string_len_null(ApolloNamespaceLock::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloNamespaceLock::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_namespace_lock_app_cluster_namespace")
                    .table(ApolloNamespaceLock::Table)
                    .col(ApolloNamespaceLock::AppId)
                    .col(ApolloNamespaceLock::ClusterName)
                    .col(ApolloNamespaceLock::NamespaceName)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_namespace_lock_datachange_lasttime")
                    .table(ApolloNamespaceLock::Table)
                    .col(ApolloNamespaceLock::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloNamespaceLock::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloNamespaceLock {
    Table,
    Id,
    AppId,
    ClusterName,
    NamespaceName,
    LockedBy,
    LockedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
