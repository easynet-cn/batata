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
                    .table(ApolloFavorite::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloFavorite::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloFavorite::UserId, 32, "default"))
                    .col(string_len_default(ApolloFavorite::AppId, 64, "default"))
                    .col(unsigned_int(ApolloFavorite::Position, backend).default(10000))
                    .col(bit(ApolloFavorite::IsDeleted, None))
                    .col(unsigned_big_int(ApolloFavorite::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloFavorite::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloFavorite::DataChangeCreatedTime))
                    .col(string_len_null(ApolloFavorite::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloFavorite::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_favorite_userid_appid_deletedat")
                    .table(ApolloFavorite::Table)
                    .col(ApolloFavorite::UserId)
                    .col(ApolloFavorite::AppId)
                    .col(ApolloFavorite::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_favorite_appid")
                    .table(ApolloFavorite::Table)
                    .col(ApolloFavorite::AppId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_favorite_datachange_lasttime")
                    .table(ApolloFavorite::Table)
                    .col(ApolloFavorite::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloFavorite::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloFavorite {
    Table,
    Id,
    UserId,
    AppId,
    Position,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
