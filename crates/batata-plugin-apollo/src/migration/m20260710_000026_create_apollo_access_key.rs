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
                    .table(ApolloAccessKey::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloAccessKey::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloAccessKey::AppId, 64, "default"))
                    .col(string_len_default(ApolloAccessKey::Secret, 128, ""))
                    .col(unsigned_tiny_int(ApolloAccessKey::Mode, backend).default(0))
                    .col(bit(ApolloAccessKey::IsEnabled, None))
                    .col(bit(ApolloAccessKey::IsDeleted, None))
                    .col(unsigned_big_int(ApolloAccessKey::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloAccessKey::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloAccessKey::DataChangeCreatedTime))
                    .col(string_len_default(ApolloAccessKey::DataChangeLastModifiedBy, 64, ""))
                    .col(date_time_on_update(ApolloAccessKey::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_access_key_appid_secret_deletedat")
                    .table(ApolloAccessKey::Table)
                    .col(ApolloAccessKey::AppId)
                    .col(ApolloAccessKey::Secret)
                    .col(ApolloAccessKey::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_access_key_datachange_lasttime")
                    .table(ApolloAccessKey::Table)
                    .col(ApolloAccessKey::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloAccessKey::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloAccessKey {
    Table,
    Id,
    AppId,
    Secret,
    Mode,
    IsEnabled,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
