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
                    .table(ApolloRolePermission::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloRolePermission::Id, backend).auto_increment().primary_key())
                    .col(unsigned_int_null(ApolloRolePermission::RoleId, backend))
                    .col(unsigned_int_null(ApolloRolePermission::PermissionId, backend))
                    .col(bit(ApolloRolePermission::IsDeleted, None))
                    .col(unsigned_big_int(ApolloRolePermission::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloRolePermission::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloRolePermission::DataChangeCreatedTime))
                    .col(string_len_null(ApolloRolePermission::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloRolePermission::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_role_permission_roleid_permissionid_deletedat")
                    .table(ApolloRolePermission::Table)
                    .col(ApolloRolePermission::RoleId)
                    .col(ApolloRolePermission::PermissionId)
                    .col(ApolloRolePermission::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_role_permission_datachange_lasttime")
                    .table(ApolloRolePermission::Table)
                    .col(ApolloRolePermission::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_role_permission_permissionid")
                    .table(ApolloRolePermission::Table)
                    .col(ApolloRolePermission::PermissionId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloRolePermission::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloRolePermission {
    Table,
    Id,
    RoleId,
    PermissionId,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
