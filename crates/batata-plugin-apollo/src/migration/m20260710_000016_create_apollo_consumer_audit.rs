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
                    .col(unsigned_int(ApolloConsumerAudit::ConsumerId, backend))
                    .col(string_len_default(ApolloConsumerAudit::OpName, 50, "default"))
                    .col(date_time(ApolloConsumerAudit::OpTime))
                    .col(string_len_default(ApolloConsumerAudit::OpBy, 64, "default"))
                    .col(string_len_default(ApolloConsumerAudit::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloConsumerAudit::DataChangeCreatedTime))
                    .col(string_len_null(ApolloConsumerAudit::DataChangeLastModifiedBy, 64))
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
    OpName,
    OpTime,
    OpBy,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
