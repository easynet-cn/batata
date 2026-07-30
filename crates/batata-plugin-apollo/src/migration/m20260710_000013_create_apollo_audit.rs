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
                    .table(ApolloAudit::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloAudit::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloAudit::AuditKey, 64, "default"))
                    .col(string_len_default(ApolloAudit::EntityName, 50, "default"))
                    .col(string_len_default(ApolloAudit::EntityId, 50, "default"))
                    .col(string_len_default(ApolloAudit::OpName, 50, "default"))
                    .col(date_time(ApolloAudit::OpTime))
                    .col(string_len_default(ApolloAudit::OpBy, 64, "default"))
                    .col(string_len_default(ApolloAudit::OpClientIp, 64, "default"))
                    .col(long_text_null(ApolloAudit::Detail, backend))
                    .col(string_len_default(ApolloAudit::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloAudit::DataChangeCreatedTime))
                    .col(string_len_null(ApolloAudit::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloAudit::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_audit_datachange_lasttime")
                    .table(ApolloAudit::Table)
                    .col(ApolloAudit::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloAudit::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloAudit {
    Table,
    Id,
    AuditKey,
    EntityName,
    EntityId,
    OpName,
    OpTime,
    OpBy,
    OpClientIp,
    Detail,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
