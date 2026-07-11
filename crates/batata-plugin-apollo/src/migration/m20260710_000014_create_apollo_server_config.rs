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
                    .table(ApolloServerConfig::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloServerConfig::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloServerConfig::Key, 64, "default"))
                    .col(string_len_default(ApolloServerConfig::Value, 2048, "default"))
                    .col(string_len_null(ApolloServerConfig::Comment, 1024))
                    .col(bit(ApolloServerConfig::IsDeleted, None))
                    .col(unsigned_big_int(ApolloServerConfig::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloServerConfig::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloServerConfig::DataChangeCreatedTime))
                    .col(string_len_null(ApolloServerConfig::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloServerConfig::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_server_config_key_deletedat")
                    .table(ApolloServerConfig::Table)
                    .col(ApolloServerConfig::Key)
                    .col(ApolloServerConfig::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_server_config_datachange_lasttime")
                    .table(ApolloServerConfig::Table)
                    .col(ApolloServerConfig::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloServerConfig::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloServerConfig {
    Table,
    Id,
    Key,
    Value,
    Comment,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
