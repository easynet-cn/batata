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
                    .table(ApolloItem::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloItem::Id, backend).auto_increment().primary_key())
                    .col(unsigned_int(ApolloItem::NamespaceId, backend).default(0))
                    .col(string_len_default(ApolloItem::Key, 128, "default"))
                    .col(tiny_int(ApolloItem::Type, backend).default(0))
                    .col(long_text(ApolloItem::Value, backend))
                    .col(string_len_null(ApolloItem::Comment, 1024))
                    .col(unsigned_int(ApolloItem::LineNum, backend).default(0))
                    .col(bit(ApolloItem::IsDeleted, None))
                    .col(unsigned_big_int(ApolloItem::DeletedAt, backend).default(0))
                    .col(string_len_default(ApolloItem::DataChangeCreatedBy, 64, "default"))
                    .col(date_time(ApolloItem::DataChangeCreatedTime))
                    .col(string_len_null(ApolloItem::DataChangeLastModifiedBy, 64))
                    .col(date_time_on_update(ApolloItem::DataChangeLastTime))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_item_namespaceid")
                    .table(ApolloItem::Table)
                    .col(ApolloItem::NamespaceId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_item_datachange_lasttime")
                    .table(ApolloItem::Table)
                    .col(ApolloItem::DataChangeLastTime)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloItem::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloItem {
    Table,
    Id,
    NamespaceId,
    Key,
    Type,
    Value,
    Comment,
    LineNum,
    IsDeleted,
    DeletedAt,
    DataChangeCreatedBy,
    DataChangeCreatedTime,
    DataChangeLastModifiedBy,
    DataChangeLastTime,
}
