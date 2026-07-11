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
                    .table(ApolloConsumer::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloConsumer::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloConsumer::AppId, 64, "default"))
                    .col(string_len_default(ApolloConsumer::Name, 500, "default"))
                    .col(string_len_default(ApolloConsumer::OrgId, 32, "default"))
                    .col(string_len_default(ApolloConsumer::OrgName, 64, "default"))
                    .col(string_len_default(ApolloConsumer::OwnerName, 500, "default"))
                    .col(string_len_default(ApolloConsumer::OwnerEmail, 500, "default"))
                    .col(bit(ApolloConsumer::IsDeleted, None))
                    .col(unsigned_big_int(ApolloConsumer::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloConsumer::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloConsumer::DataChangeCreatedTime))
                    .col(string_len_null(ApolloConsumer::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloConsumer::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_consumer_appid_deletedat")
                    .table(ApolloConsumer::Table)
                    .col(ApolloConsumer::AppId)
                    .col(ApolloConsumer::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_consumer_datachange_lasttime")
                    .table(ApolloConsumer::Table)
                    .col(ApolloConsumer::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloConsumer::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloConsumer {
    Table,
    Id,
    AppId,
    Name,
    OrgId,
    OrgName,
    OwnerName,
    OwnerEmail,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
