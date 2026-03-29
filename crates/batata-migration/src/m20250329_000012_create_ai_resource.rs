use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AiResource::Table)
                    .if_not_exists()
                    .col(
                        big_integer(AiResource::Id)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        timestamp(AiResource::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp(AiResource::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(string_len(AiResource::Name, 256).not_null())
                    .col(string_len(AiResource::Type, 32).not_null())
                    .col(string_len_null(AiResource::CDesc, 2048))
                    .col(string_len_null(AiResource::Status, 32))
                    .col(
                        string_len(AiResource::NamespaceId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(string_len_null(AiResource::BizTags, 1024))
                    .col(text_null(AiResource::Ext))
                    .col(
                        string_len(AiResource::CFrom, 256)
                            .not_null()
                            .default("local"),
                    )
                    .col(text_null(AiResource::VersionInfo))
                    .col(
                        big_integer(AiResource::MetaVersion)
                            .not_null()
                            .default(1),
                    )
                    .col(
                        string_len(AiResource::Scope, 16)
                            .not_null()
                            .default("PRIVATE"),
                    )
                    .col(
                        string_len(AiResource::Owner, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        big_integer(AiResource::DownloadCount)
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique key: namespace_id + name + type + c_from
        manager
            .create_index(
                Index::create()
                    .name("uk_ai_resource_ns_name_type")
                    .table(AiResource::Table)
                    .col(AiResource::NamespaceId)
                    .col(AiResource::Name)
                    .col(AiResource::Type)
                    .col(AiResource::CFrom)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_ai_resource_name")
                    .table(AiResource::Table)
                    .col(AiResource::Name)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_ai_resource_type")
                    .table(AiResource::Table)
                    .col(AiResource::Type)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_ai_resource_gmt_modified")
                    .table(AiResource::Table)
                    .col(AiResource::GmtModified)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AiResource::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AiResource {
    Table,
    Id,
    GmtCreate,
    GmtModified,
    Name,
    Type,
    CDesc,
    Status,
    NamespaceId,
    BizTags,
    Ext,
    CFrom,
    VersionInfo,
    MetaVersion,
    Scope,
    Owner,
    DownloadCount,
}
