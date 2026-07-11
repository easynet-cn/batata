use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

use crate::migration::column_helper::{bit, date_time, date_time_on_update, string_len_default, string_len_null, unsigned_big_int, unsigned_int};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        
        manager
            .create_table(
                Table::create()
                    .table(ApolloNamespace::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloNamespace::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloNamespace::AppId, 64, "default"))
                    .col(string_len_default(ApolloNamespace::ClusterName, 500, "default"))
                    .col(string_len_default(ApolloNamespace::NamespaceName, 500, "default"))
                    .col(bit(ApolloNamespace::IsDeleted, None))
                    .col(unsigned_big_int(ApolloNamespace::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloNamespace::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloNamespace::DataChangeCreatedTime))
                    .col(string_len_null(ApolloNamespace::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloNamespace::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        // 联合唯一索引：MySQL需要前缀长度（cluster_name(128)）避免索引长度超过3072字节限制
        // PostgreSQL/SQLite不支持前缀长度语法，直接使用完整列
        match backend {
            DatabaseBackend::MySql => {
                manager.get_connection().execute_unprepared(
                    "CREATE UNIQUE INDEX uk_apollo_ns_appid_clustername_namespacename_deletedat ON apollo_namespace (app_id, cluster_name(128), namespace_name(128), deleted_at)"
                ).await?;
            }
            _ => {
                manager
                    .create_index(
                        Index::create()
                            .name("uk_apollo_ns_appid_clustername_namespacename_deletedat")
                            .table(ApolloNamespace::Table)
                            .col(ApolloNamespace::AppId)
                            .col(ApolloNamespace::ClusterName)
                            .col(ApolloNamespace::NamespaceName)
                            .col(ApolloNamespace::DeletedAt)
                            .unique()
                            .if_not_exists()
                            .to_owned(),
                    )
                    .await?;
            }
        }

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_ns_datachange_lasttime")
                    .table(ApolloNamespace::Table)
                    .col(ApolloNamespace::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_ns_namespacename")
                    .table(ApolloNamespace::Table)
                    .col(ApolloNamespace::NamespaceName)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloNamespace::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloNamespace {
    Table,
    Id,
    AppId,
    ClusterName,
    NamespaceName,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
