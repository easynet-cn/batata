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
                    .table(ApolloReleaseMessage::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloReleaseMessage::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloReleaseMessage::Message, 256, ""))
                    .col(date_time(ApolloReleaseMessage::DataChangeCreatedTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_release_message_createdtime")
                    .table(ApolloReleaseMessage::Table)
                    .col(ApolloReleaseMessage::DataChangeCreatedTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloReleaseMessage::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloReleaseMessage {
    Table,
    Id,
    Message,
    DataChangeCreatedTime,
}
