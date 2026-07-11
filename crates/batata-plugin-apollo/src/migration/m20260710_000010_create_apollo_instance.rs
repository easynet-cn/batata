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
                    .table(ApolloInstance::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloInstance::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloInstance::AppId, 64, "default"))
                    .col(string_len_default(ApolloInstance::ClusterName, 32, "default"))
                    .col(string_len_default(ApolloInstance::DataCenter, 64, "default"))
                    .col(string_len_default(ApolloInstance::Ip, 32, ""))
                    .col(date_time(ApolloInstance::DataChangeCreatedTime))
                    .col(date_time_on_update(ApolloInstance::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_instance_unique_key")
                    .table(ApolloInstance::Table)
                    .col(ApolloInstance::AppId)
                    .col(ApolloInstance::ClusterName)
                    .col(ApolloInstance::Ip)
                    .col(ApolloInstance::DataCenter)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_instance_ip")
                    .table(ApolloInstance::Table)
                    .col(ApolloInstance::Ip)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_instance_datachange_lasttime")
                    .table(ApolloInstance::Table)
                    .col(ApolloInstance::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloInstance::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloInstance {
    Table,
    Id,
    AppId,
    ClusterName,
    DataCenter,
    Ip,
    DataChangeCreatedTime,
    DataChangeLastTime,
}
