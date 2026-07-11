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
                    .table(ApolloPermission::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloPermission::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloPermission::PermissionType, 32, ""))
                    .col(string_len_default(ApolloPermission::TargetId, 256, ""))
                    .col(bit(ApolloPermission::IsDeleted, None))
                    .col(unsigned_big_int(ApolloPermission::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloPermission::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloPermission::DataChangeCreatedTime))
                    .col(string_len_null(ApolloPermission::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloPermission::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_permission_targetid_permissiontype_deletedat")
                    .table(ApolloPermission::Table)
                    .col(ApolloPermission::TargetId)
                    .col(ApolloPermission::PermissionType)
                    .col(ApolloPermission::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_permission_datachange_lasttime")
                    .table(ApolloPermission::Table)
                    .col(ApolloPermission::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloPermission::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloPermission {
    Table,
    Id,
    PermissionType,
    TargetId,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
