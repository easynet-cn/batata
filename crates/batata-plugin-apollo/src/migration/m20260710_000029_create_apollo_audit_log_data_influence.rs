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
                    .table(ApolloAuditLogDataInfluence::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloAuditLogDataInfluence::Id, backend).auto_increment().primary_key())
                    .col(char_len_default(ApolloAuditLogDataInfluence::SpanId, 32, "", backend))
                    .col(string_len_default(ApolloAuditLogDataInfluence::InfluenceEntityId, 50, "0"))
                    .col(string_len_default(ApolloAuditLogDataInfluence::InfluenceEntityName, 50, "default"))
                    .col(string_len_null(ApolloAuditLogDataInfluence::FieldName, 50))
                    .col(string_len_null(ApolloAuditLogDataInfluence::FieldOldValue, 500))
                    .col(string_len_null(ApolloAuditLogDataInfluence::FieldNewValue, 500))
                    .col(bit(ApolloAuditLogDataInfluence::IsDeleted, None))
                    .col(unsigned_big_int(ApolloAuditLogDataInfluence::DeletedAt, backend).default(0))
                    .col(string_len_null(ApolloAuditLogDataInfluence::DataChangeCreatedBy, 64))
                    .col(date_time(ApolloAuditLogDataInfluence::DataChangeCreatedTime))
                    .col(string_len_default(ApolloAuditLogDataInfluence::DataChangeLastModifiedBy, 64, ""))
                    .col(date_time_on_update(ApolloAuditLogDataInfluence::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_audit_log_data_influence_spanid")
                    .table(ApolloAuditLogDataInfluence::Table)
                    .col(ApolloAuditLogDataInfluence::SpanId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_audit_log_data_influence_datachange_createdtime")
                    .table(ApolloAuditLogDataInfluence::Table)
                    .col(ApolloAuditLogDataInfluence::DataChangeCreatedTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_audit_log_data_influence_entityid")
                    .table(ApolloAuditLogDataInfluence::Table)
                    .col(ApolloAuditLogDataInfluence::InfluenceEntityId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloAuditLogDataInfluence::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloAuditLogDataInfluence {
    Table,
    Id,
    SpanId,
    InfluenceEntityId,
    InfluenceEntityName,
    FieldName,
    FieldOldValue,
    FieldNewValue,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
