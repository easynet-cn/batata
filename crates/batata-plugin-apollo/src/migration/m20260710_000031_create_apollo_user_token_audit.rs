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
                    .table(ApolloUserTokenAudit::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloUserTokenAudit::Id, backend).auto_increment().primary_key())
                    .col(unsigned_int_null(ApolloUserTokenAudit::TokenId, backend))
                    .col(string_len_default(ApolloUserTokenAudit::UserId, 64, ""))
                    .col(string_len_default(ApolloUserTokenAudit::Uri, 1024, ""))
                    .col(string_len_default(ApolloUserTokenAudit::Method, 16, ""))
                    .col(date_time(ApolloUserTokenAudit::DataChangeCreatedTime))
                    .col(date_time_on_update(ApolloUserTokenAudit::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_user_token_audit_datachange_lasttime")
                    .table(ApolloUserTokenAudit::Table)
                    .col(ApolloUserTokenAudit::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_user_token_audit_tokenid")
                    .table(ApolloUserTokenAudit::Table)
                    .col(ApolloUserTokenAudit::TokenId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_user_token_audit_userid")
                    .table(ApolloUserTokenAudit::Table)
                    .col(ApolloUserTokenAudit::UserId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloUserTokenAudit::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloUserTokenAudit {
    Table,
    Id,
    TokenId,
    UserId,
    Uri,
    Method,
    DataChangeCreatedTime,
    DataChangeLastTime,
}
