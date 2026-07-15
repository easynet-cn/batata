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
                    .table(ApolloAuditLog::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloAuditLog::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloAuditLog::TraceId, 32, ""))
                    .col(string_len_default(ApolloAuditLog::SpanId, 32, ""))
                    .col(string_len_null(ApolloAuditLog::ParentSpanId, 32))
                    .col(string_len_null(ApolloAuditLog::FollowsFromSpanId, 32))
                    .col(string_len_default(ApolloAuditLog::Operator, 64, "anonymous"))
                    .col(string_len_default(ApolloAuditLog::OpType, 50, "default"))
                    .col(string_len_default(ApolloAuditLog::OpName, 150, "default"))
                    .col(string_len_null(ApolloAuditLog::Description, 200))
                    .col(bit(ApolloAuditLog::IsDeleted, None))
                    .col(unsigned_big_int(ApolloAuditLog::DeletedAt, backend).default(0))
                    .col(string_len_null(ApolloAuditLog::DataChangeCreatedBy, 64))
                    .col(date_time(ApolloAuditLog::DataChangeCreatedTime))
                    .col(string_len_default(ApolloAuditLog::DataChangeLastModifiedBy, 64, ""))
                    .col(date_time_on_update(ApolloAuditLog::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_audit_log_traceid")
                    .table(ApolloAuditLog::Table)
                    .col(ApolloAuditLog::TraceId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_audit_log_opname")
                    .table(ApolloAuditLog::Table)
                    .col(ApolloAuditLog::OpName)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_audit_log_datachange_createdtime")
                    .table(ApolloAuditLog::Table)
                    .col(ApolloAuditLog::DataChangeCreatedTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_audit_log_operator")
                    .table(ApolloAuditLog::Table)
                    .col(ApolloAuditLog::Operator)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloAuditLog::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloAuditLog {
    Table,
    Id,
    TraceId,
    SpanId,
    ParentSpanId,
    FollowsFromSpanId,
    Operator,
    OpType,
    OpName,
    Description,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
