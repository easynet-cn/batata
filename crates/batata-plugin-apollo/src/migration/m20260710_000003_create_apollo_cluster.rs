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
                    .table(ApolloCluster::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloCluster::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloCluster::Name, 32, ""))
                    .col(string_len_default(ApolloCluster::AppId, 64, ""))
                    .col(unsigned_int(ApolloCluster::ParentClusterId, backend).default(0))
                    .col(bit(ApolloCluster::IsDeleted, None))
                    .col(unsigned_big_int(ApolloCluster::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloCluster::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloCluster::DataChangeCreatedTime))
                    .col(string_len_null(ApolloCluster::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloCluster::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_cluster_appid_name_deletedat")
                    .table(ApolloCluster::Table)
                    .col(ApolloCluster::AppId)
                    .col(ApolloCluster::Name)
                    .col(ApolloCluster::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_cluster_parentclusterid")
                    .table(ApolloCluster::Table)
                    .col(ApolloCluster::ParentClusterId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_cluster_datachange_lasttime")
                    .table(ApolloCluster::Table)
                    .col(ApolloCluster::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloCluster::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloCluster {
    Table,
    Id,
    Name,
    AppId,
    ParentClusterId,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
