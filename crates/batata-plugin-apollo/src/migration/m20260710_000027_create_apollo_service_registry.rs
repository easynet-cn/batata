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
                    .table(ApolloServiceRegistry::Table)
                    .if_not_exists()
                    .col(
                        unsigned_int(ApolloServiceRegistry::Id, backend)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(string_len(ApolloServiceRegistry::ServiceName, 64))
                    .col(string_len(ApolloServiceRegistry::Uri, 64))
                    .col(string_len(ApolloServiceRegistry::Cluster, 64))
                    .col(string_len_default(ApolloServiceRegistry::Metadata, 1024, "{}"))
                    .col(date_time(ApolloServiceRegistry::DataChangeCreatedTime))
                    .col(date_time_on_update(ApolloServiceRegistry::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_service_registry_unique_key")
                    .table(ApolloServiceRegistry::Table)
                    .col(ApolloServiceRegistry::ServiceName)
                    .col(ApolloServiceRegistry::Uri)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_service_registry_datachange_lasttime")
                    .table(ApolloServiceRegistry::Table)
                    .col(ApolloServiceRegistry::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloServiceRegistry::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloServiceRegistry {
    Table,
    Id,
    ServiceName,
    Uri,
    Cluster,
    Metadata,
    DataChangeCreatedTime,
    DataChangeLastTime,
}
