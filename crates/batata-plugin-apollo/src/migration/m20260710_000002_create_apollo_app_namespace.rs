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
                    .table(ApolloAppNamespace::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloAppNamespace::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloAppNamespace::Name, 32, ""))
                    .col(string_len_default(ApolloAppNamespace::AppId, 64, ""))
                    .col(string_len_default(ApolloAppNamespace::Format, 32, "properties"))
                    .col(bit(ApolloAppNamespace::IsPublic, None))
                    .col(string_len_default(ApolloAppNamespace::Comment, 64, ""))
                    .col(bit(ApolloAppNamespace::IsDeleted, None))
                    .col(unsigned_big_int(ApolloAppNamespace::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloAppNamespace::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloAppNamespace::DataChangeCreatedTime))
                    .col(string_len_null(ApolloAppNamespace::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloAppNamespace::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_appns_appid_name_deletedat")
                    .table(ApolloAppNamespace::Table)
                    .col(ApolloAppNamespace::AppId)
                    .col(ApolloAppNamespace::Name)
                    .col(ApolloAppNamespace::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_appns_name_appid")
                    .table(ApolloAppNamespace::Table)
                    .col(ApolloAppNamespace::Name)
                    .col(ApolloAppNamespace::AppId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_appns_datachange_lasttime")
                    .table(ApolloAppNamespace::Table)
                    .col(ApolloAppNamespace::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloAppNamespace::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloAppNamespace {
    Table,
    Id,
    Name,
    AppId,
    Format,
    IsPublic,
    Comment,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
