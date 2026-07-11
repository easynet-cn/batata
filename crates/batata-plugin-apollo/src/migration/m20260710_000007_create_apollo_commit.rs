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
                    .table(ApolloCommit::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloCommit::Id, backend).auto_increment().primary_key())
                    .col(long_text(ApolloCommit::ChangeSets, backend))
                    .col(string_len_default(ApolloCommit::AppId, 64, "default"))
                    .col(string_len_default(ApolloCommit::ClusterName, 500, "default"))
                    .col(string_len_default(ApolloCommit::NamespaceName, 500, "default"))
                    .col(string_len_null(ApolloCommit::Comment, 500))
                    .col(bit(ApolloCommit::IsDeleted, None))
                    .col(unsigned_big_int(ApolloCommit::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloCommit::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloCommit::DataChangeCreatedTime))
                    .col(string_len_null(ApolloCommit::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloCommit::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_commit_datachange_lasttime")
                    .table(ApolloCommit::Table)
                    .col(ApolloCommit::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_commit_appid")
                    .table(ApolloCommit::Table)
                    .col(ApolloCommit::AppId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_commit_clustername")
                    .table(ApolloCommit::Table)
                    .col(ApolloCommit::ClusterName)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_commit_namespacename")
                    .table(ApolloCommit::Table)
                    .col(ApolloCommit::NamespaceName)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloCommit::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloCommit {
    Table,
    Id,
    ChangeSets,
    AppId,
    ClusterName,
    NamespaceName,
    Comment,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
