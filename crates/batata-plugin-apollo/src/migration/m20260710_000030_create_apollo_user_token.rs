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
                    .table(ApolloUserToken::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloUserToken::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloUserToken::UserId, 64, ""))
                    .col(string_len_default(ApolloUserToken::Name, 128, ""))
                    .col(string_len_default(ApolloUserToken::TokenPrefix, 32, ""))
                    .col(string_len_default(ApolloUserToken::TokenHash, 128, ""))
                    .col(string_len_null(ApolloUserToken::Scopes, 4096))
                    .col(ColumnDef::new(ApolloUserToken::RateLimit).integer().not_null().default(0))
                    .col(ColumnDef::new(ApolloUserToken::Expires).date_time().not_null())
                    .col(datetime_null(ApolloUserToken::LastUsedTime))
                    .col(string_len_null(ApolloUserToken::LastUsedIp, 64))
                    .col(string_len_null(ApolloUserToken::LastUsedUserAgent, 512))
                    .col(datetime_null(ApolloUserToken::RevokedAt))
                    .col(string_len_null(ApolloUserToken::RevokedBy, 64))
                    .col(bit(ApolloUserToken::IsDeleted, None))
                    .col(unsigned_big_int(ApolloUserToken::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloUserToken::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloUserToken::DataChangeCreatedTime))
                    .col(string_len_null(ApolloUserToken::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloUserToken::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_user_token_tokenprefix_deletedat")
                    .table(ApolloUserToken::Table)
                    .col(ApolloUserToken::TokenPrefix)
                    .col(ApolloUserToken::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_user_token_userid")
                    .table(ApolloUserToken::Table)
                    .col(ApolloUserToken::UserId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_user_token_tokenhash")
                    .table(ApolloUserToken::Table)
                    .col(ApolloUserToken::TokenHash)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_user_token_datachange_lasttime")
                    .table(ApolloUserToken::Table)
                    .col(ApolloUserToken::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloUserToken::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloUserToken {
    Table,
    Id,
    UserId,
    Name,
    TokenPrefix,
    TokenHash,
    Scopes,
    RateLimit,
    Expires,
    LastUsedTime,
    LastUsedIp,
    LastUsedUserAgent,
    RevokedAt,
    RevokedBy,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
