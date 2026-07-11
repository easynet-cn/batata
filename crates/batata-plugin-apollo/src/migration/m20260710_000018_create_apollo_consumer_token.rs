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
                    .table(ApolloConsumerToken::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloConsumerToken::Id, backend).auto_increment().primary_key())
                    .col(unsigned_int_null(ApolloConsumerToken::ConsumerId, backend))
                    .col(string_len_default(ApolloConsumerToken::Token, 128, ""))
                    .col(datetime_default(ApolloConsumerToken::Expires, "2099-01-01 00:00:00"))
                    .col(bit(ApolloConsumerToken::IsDeleted, None))
                    .col(unsigned_big_int(ApolloConsumerToken::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloConsumerToken::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloConsumerToken::DataChangeCreatedTime))
                    .col(string_len_null(ApolloConsumerToken::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloConsumerToken::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_consumer_token_token_deletedat")
                    .table(ApolloConsumerToken::Table)
                    .col(ApolloConsumerToken::Token)
                    .col(ApolloConsumerToken::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_consumer_token_datachange_lasttime")
                    .table(ApolloConsumerToken::Table)
                    .col(ApolloConsumerToken::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloConsumerToken::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloConsumerToken {
    Table,
    Id,
    ConsumerId,
    Token,
    Expires,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
