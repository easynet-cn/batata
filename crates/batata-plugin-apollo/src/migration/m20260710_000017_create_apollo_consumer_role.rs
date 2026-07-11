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
                    .table(ApolloConsumerRole::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloConsumerRole::Id, backend).auto_increment().primary_key())
                    .col(unsigned_int_null(ApolloConsumerRole::ConsumerId, backend))
                    .col(unsigned_int_null(ApolloConsumerRole::RoleId, backend))
                    .col(bit(ApolloConsumerRole::IsDeleted, None))
                    .col(unsigned_big_int(ApolloConsumerRole::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloConsumerRole::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloConsumerRole::DataChangeCreatedTime))
                    .col(string_len_null(ApolloConsumerRole::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloConsumerRole::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_consumer_role_consumerid_roleid_deletedat")
                    .table(ApolloConsumerRole::Table)
                    .col(ApolloConsumerRole::ConsumerId)
                    .col(ApolloConsumerRole::RoleId)
                    .col(ApolloConsumerRole::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_consumer_role_datachange_lasttime")
                    .table(ApolloConsumerRole::Table)
                    .col(ApolloConsumerRole::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_consumer_role_roleid")
                    .table(ApolloConsumerRole::Table)
                    .col(ApolloConsumerRole::RoleId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloConsumerRole::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloConsumerRole {
    Table,
    Id,
    ConsumerId,
    RoleId,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
