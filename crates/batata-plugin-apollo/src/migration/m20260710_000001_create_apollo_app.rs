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
                    .table(ApolloApp::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloApp::Id, backend).auto_increment().primary_key())
                    .col(string_len_default(ApolloApp::AppId, 64, "default"))
                    .col(string_len_default(ApolloApp::Name, 500, "default"))
                    .col(string_len_default(ApolloApp::OrgId, 32, "default"))
                    .col(string_len_default(ApolloApp::OrgName, 64, "default"))
                    .col(string_len_default(ApolloApp::OwnerName, 500, "default"))
                    .col(string_len_default(ApolloApp::OwnerEmail, 500, "default"))
                    .col(bit(ApolloApp::IsDeleted, None))
                    .col(unsigned_big_int(ApolloApp::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloApp::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloApp::DataChangeCreatedTime))
                    .col(string_len_null(ApolloApp::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloApp::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_apollo_app_appid_deletedat")
                    .table(ApolloApp::Table)
                    .col(ApolloApp::AppId)
                    .col(ApolloApp::DeletedAt)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_app_datachange_lasttime")
                    .table(ApolloApp::Table)
                    .col(ApolloApp::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_app_name")
                    .table(ApolloApp::Table)
                    .col(ApolloApp::Name)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloApp::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloApp {
    Table,
    Id,
    AppId,
    Name,
    OrgId,
    OrgName,
    OwnerName,
    OwnerEmail,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
