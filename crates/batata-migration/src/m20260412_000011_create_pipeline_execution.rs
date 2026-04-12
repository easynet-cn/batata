use sea_orm_migration::{prelude::*, schema::*};
use crate::column_helper::long_text;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PipelineExecution::Table)
                    .if_not_exists()
                    .col(string_len(PipelineExecution::ExecutionId, 64).primary_key())
                    .col(string_len(PipelineExecution::ResourceType, 32).not_null())
                    .col(string_len(PipelineExecution::ResourceName, 256).not_null())
                    .col(string_len_null(PipelineExecution::NamespaceId, 128))
                    .col(string_len_null(PipelineExecution::Version, 64))
                    .col(string_len(PipelineExecution::Status, 32).not_null())
                    .col(long_text(PipelineExecution::Pipeline, manager.get_database_backend()).not_null())
                    .col(big_integer(PipelineExecution::CreateTime).not_null())
                    .col(big_integer(PipelineExecution::UpdateTime).not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PipelineExecution::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum PipelineExecution {
    Table,
    ExecutionId,
    ResourceType,
    ResourceName,
    NamespaceId,
    Version,
    Status,
    Pipeline,
    CreateTime,
    UpdateTime,
}
