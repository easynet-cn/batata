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
                    .table(ApolloReleaseHistory::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloReleaseHistory::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloReleaseHistory::AppId, 64, "default"))
                    .col(string_len_default(ApolloReleaseHistory::ClusterName, 32, "default"))
                    .col(string_len_default(ApolloReleaseHistory::NamespaceName, 32, "default"))
                    .col(string_len_default(ApolloReleaseHistory::BranchName, 32, "default"))
                    .col(unsigned_int(ApolloReleaseHistory::ReleaseId, backend).default(0))
                    .col(unsigned_int(ApolloReleaseHistory::PreviousReleaseId, backend).default(0))
                    .col(tiny_int(ApolloReleaseHistory::Operation, backend).default(0))
                    .col(long_text(ApolloReleaseHistory::OperationContext, backend))
                    .col(bit(ApolloReleaseHistory::IsDeleted, None))
                    .col(unsigned_big_int(ApolloReleaseHistory::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloReleaseHistory::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloReleaseHistory::DataChangeCreatedTime))
                    .col(string_len_null(ApolloReleaseHistory::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloReleaseHistory::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ix_apollo_release_history_namespace")
                    .table(ApolloReleaseHistory::Table)
                    .col(ApolloReleaseHistory::AppId)
                    .col(ApolloReleaseHistory::ClusterName)
                    .col(ApolloReleaseHistory::NamespaceName)
                    .col(ApolloReleaseHistory::BranchName)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ix_apollo_release_history_releaseid")
                    .table(ApolloReleaseHistory::Table)
                    .col(ApolloReleaseHistory::ReleaseId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ix_apollo_release_history_datachange_lasttime")
                    .table(ApolloReleaseHistory::Table)
                    .col(ApolloReleaseHistory::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ix_apollo_release_history_previousreleaseid")
                    .table(ApolloReleaseHistory::Table)
                    .col(ApolloReleaseHistory::PreviousReleaseId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloReleaseHistory::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloReleaseHistory {
    Table,
    Id,
    AppId,
    ClusterName,
    NamespaceName,
    BranchName,
    ReleaseId,
    PreviousReleaseId,
    Operation,
    OperationContext,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
