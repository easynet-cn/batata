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
                    .table(ApolloConsumerAudit::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloConsumerAudit::Id, backend).auto_increment().primary_key())
                    .col(unsigned_int_null(ApolloConsumerAudit::ConsumerId, backend))
                    .col(string_len_default(ApolloConsumerAudit::Uri, 1024, ""))
                    .col(string_len_default(ApolloConsumerAudit::Method, 16, ""))
                    .col(date_time(ApolloConsumerAudit::DataChangeCreatedTime))
                    .col(date_time_on_update(ApolloConsumerAudit::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_consumer_audit_datachange_lasttime")
                    .table(ApolloConsumerAudit::Table)
                    .col(ApolloConsumerAudit::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_consumer_audit_consumerid")
                    .table(ApolloConsumerAudit::Table)
                    .col(ApolloConsumerAudit::ConsumerId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloConsumerAudit::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloConsumerAudit {
    Table,
    Id,
    ConsumerId,
    Uri,
    Method,
    DataChangeCreatedTime,
    DataChangeLastTime,
}
