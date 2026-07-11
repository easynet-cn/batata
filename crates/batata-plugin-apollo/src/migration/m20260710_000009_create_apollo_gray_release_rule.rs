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
                    .table(ApolloGrayReleaseRule::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloGrayReleaseRule::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloGrayReleaseRule::AppId, 64, "default"))
                    .col(string_len_default(ApolloGrayReleaseRule::ClusterName, 32, "default"))
                    .col(string_len_default(ApolloGrayReleaseRule::NamespaceName, 32, "default"))
                    .col(string_len_default(ApolloGrayReleaseRule::BranchName, 32, "default"))
                    .col(string_len_default(ApolloGrayReleaseRule::Rules, 16000, "[]"))
                    .col(unsigned_int(ApolloGrayReleaseRule::ReleaseId, backend).default(0))
                    .col(tiny_int_null(ApolloGrayReleaseRule::BranchStatus, backend))
                    .col(bit(ApolloGrayReleaseRule::IsDeleted, None))
                    .col(unsigned_big_int(ApolloGrayReleaseRule::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloGrayReleaseRule::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloGrayReleaseRule::DataChangeCreatedTime))
                    .col(string_len_null(ApolloGrayReleaseRule::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloGrayReleaseRule::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_gray_release_rule_datachange_lasttime")
                    .table(ApolloGrayReleaseRule::Table)
                    .col(ApolloGrayReleaseRule::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_gray_release_rule_namespace")
                    .table(ApolloGrayReleaseRule::Table)
                    .col(ApolloGrayReleaseRule::AppId)
                    .col(ApolloGrayReleaseRule::ClusterName)
                    .col(ApolloGrayReleaseRule::NamespaceName)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloGrayReleaseRule::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloGrayReleaseRule {
    Table,
    Id,
    AppId,
    ClusterName,
    NamespaceName,
    BranchName,
    Rules,
    ReleaseId,
    BranchStatus,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
