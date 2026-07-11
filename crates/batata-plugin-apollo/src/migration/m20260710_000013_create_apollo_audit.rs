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
                    .col(string_len_default(ApolloAudit::EntityName, 50, "default"))
                    .col(unsigned_int_null(ApolloAudit::EntityId, backend))
                    .col(string_len_default(ApolloAudit::OpName, 50, "default"))
                    .col(string_len_null(ApolloAudit::Comment, 500))
                    .col(bit(ApolloAudit::IsDeleted, None))
                    .col(unsigned_big_int(ApolloAudit::DeletedAt, backend).default(0))
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
    EntityName,
    EntityId,
    OpName,
    Comment,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
