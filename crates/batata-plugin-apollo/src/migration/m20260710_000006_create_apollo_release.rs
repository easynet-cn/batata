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
                    .table(ApolloRelease::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloRelease::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloRelease::ReleaseKey, 64, ""))
                    .col(string_len_default(ApolloRelease::Name, 64, "default"))
                    .col(string_len_null(ApolloRelease::Comment, 256))
                    .col(string_len_default(ApolloRelease::AppId, 64, "default"))
                    .col(string_len_default(ApolloRelease::ClusterName, 500, "default"))
                    .col(string_len_default(ApolloRelease::NamespaceName, 500, "default"))
                    .col(long_text(ApolloRelease::Configurations, backend))
                    .col(unsigned_big_int_null(ApolloRelease::ReleaseId, backend))
                    .col(bit(ApolloRelease::IsAbandoned, None))
                    .col(bit(ApolloRelease::IsDeleted, None))
                    .col(unsigned_big_int(ApolloRelease::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloRelease::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloRelease::DataChangeCreatedTime))
                    .col(string_len_null(ApolloRelease::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloRelease::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_release_releasekey")
                    .table(ApolloRelease::Table)
                    .col(ApolloRelease::ReleaseKey)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_release_datachange_lasttime")
                    .table(ApolloRelease::Table)
                    .col(ApolloRelease::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_release_appid")
                    .table(ApolloRelease::Table)
                    .col(ApolloRelease::AppId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloRelease::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloRelease {
    Table,
    Id,
    ReleaseKey,
    Name,
    Comment,
    AppId,
    ClusterName,
    NamespaceName,
    Configurations,
    ReleaseId,
    IsAbandoned,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
