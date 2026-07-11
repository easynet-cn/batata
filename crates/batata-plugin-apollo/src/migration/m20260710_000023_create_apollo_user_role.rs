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
                    .table(ApolloUserRole::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloUserRole::Id, backend).auto_increment().primary_key())
                    .col(string_len_null(ApolloUserRole::UserId, 128))
                    .col(unsigned_int_null(ApolloUserRole::RoleId, backend))
                    .col(bit(ApolloUserRole::IsDeleted, None))
                    .col(unsigned_big_int(ApolloUserRole::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloUserRole::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloUserRole::DataChangeCreatedTime))
                    .col(string_len_null(ApolloUserRole::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloUserRole::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_user_role_userid_roleid_deletedat")
                    .table(ApolloUserRole::Table)
                    .col(ApolloUserRole::UserId)
                    .col(ApolloUserRole::RoleId)
                    .col(ApolloUserRole::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_user_role_datachange_lasttime")
                    .table(ApolloUserRole::Table)
                    .col(ApolloUserRole::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_user_role_roleid")
                    .table(ApolloUserRole::Table)
                    .col(ApolloUserRole::RoleId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloUserRole::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloUserRole {
    Table,
    Id,
    UserId,
    RoleId,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
