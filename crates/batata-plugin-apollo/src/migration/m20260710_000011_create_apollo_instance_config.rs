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
                    .table(ApolloInstanceConfig::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloInstanceConfig::Id, backend).auto_increment().primary_key())
                    .col(unsigned_int(ApolloInstanceConfig::InstanceId, backend))
                    .col(string_len_default(ApolloInstanceConfig::NamespaceName, 128, "default"))
                    .col(string_len_default(ApolloInstanceConfig::ClusterName, 32, "default"))
                    .col(string_len_default(ApolloInstanceConfig::ReleaseKey, 64, ""))
                    .col(long_text_null(ApolloInstanceConfig::Configurations, backend))
                    .col(string_len_default(ApolloInstanceConfig::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloInstanceConfig::DataChangeCreatedTime))
                    .col(string_len_null(ApolloInstanceConfig::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloInstanceConfig::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_instance_config_unique_key")
                    .table(ApolloInstanceConfig::Table)
                    .col(ApolloInstanceConfig::InstanceId)
                    .col(ApolloInstanceConfig::NamespaceName)
                    .col(ApolloInstanceConfig::ClusterName)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_instance_config_releasekey")
                    .table(ApolloInstanceConfig::Table)
                    .col(ApolloInstanceConfig::ReleaseKey)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_instance_config_datachange_lasttime")
                    .table(ApolloInstanceConfig::Table)
                    .col(ApolloInstanceConfig::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloInstanceConfig::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloInstanceConfig {
    Table,
    Id,
    InstanceId,
    NamespaceName,
    ClusterName,
    ReleaseKey,
    Configurations,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
