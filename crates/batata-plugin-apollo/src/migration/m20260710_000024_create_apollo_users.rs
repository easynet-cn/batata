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
                    .table(ApolloUsers::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloUsers::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloUsers::Username, 64, "default"))
                    .col(string_len_default(ApolloUsers::Password, 64, "default"))
                    .col(string_len_null(ApolloUsers::Email, 256))
                    .col(string_len_null(ApolloUsers::Enabled, 5))
                    .col(bit(ApolloUsers::IsDeleted, None))
                    .col(unsigned_big_int(ApolloUsers::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloUsers::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloUsers::DataChangeCreatedTime))
                    .col(string_len_null(ApolloUsers::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloUsers::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_users_username_deletedat")
                    .table(ApolloUsers::Table)
                    .col(ApolloUsers::Username)
                    .col(ApolloUsers::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_users_datachange_lasttime")
                    .table(ApolloUsers::Table)
                    .col(ApolloUsers::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloUsers::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloUsers {
    Table,
    Id,
    Username,
    Password,
    Email,
    Enabled,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
