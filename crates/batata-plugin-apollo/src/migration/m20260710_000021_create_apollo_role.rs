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
                    .table(ApolloRole::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloRole::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloRole::RoleName, 256, ""))
                    .col(bit(ApolloRole::IsDeleted, None))
                    .col(unsigned_big_int(ApolloRole::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloRole::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloRole::DataChangeCreatedTime))
                    .col(string_len_null(ApolloRole::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloRole::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_role_rolename_deletedat")
                    .table(ApolloRole::Table)
                    .col(ApolloRole::RoleName)
                    .col(ApolloRole::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_role_datachange_lasttime")
                    .table(ApolloRole::Table)
                    .col(ApolloRole::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloRole::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloRole {
    Table,
    Id,
    RoleName,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
