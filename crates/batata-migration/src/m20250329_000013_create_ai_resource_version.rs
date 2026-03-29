use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AiResourceVersion::Table)
                    .if_not_exists()
                    .col(
                        big_integer(AiResourceVersion::Id)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        timestamp(AiResourceVersion::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp(AiResourceVersion::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(string_len(AiResourceVersion::Type, 32).not_null())
                    .col(string_len_null(AiResourceVersion::Author, 128))
                    .col(string_len(AiResourceVersion::Name, 256).not_null())
                    .col(string_len_null(AiResourceVersion::CDesc, 2048))
                    .col(string_len(AiResourceVersion::Status, 32).not_null())
                    .col(string_len(AiResourceVersion::Version, 64).not_null())
                    .col(
                        string_len(AiResourceVersion::NamespaceId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(text_null(AiResourceVersion::Storage))
                    .col(text_null(AiResourceVersion::PublishPipelineInfo))
                    .col(
                        big_integer(AiResourceVersion::DownloadCount)
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique key: namespace_id + name + type + version
        manager
            .create_index(
                Index::create()
                    .name("uk_ai_resource_ver_ns_name_type_ver")
                    .table(AiResourceVersion::Table)
                    .col(AiResourceVersion::NamespaceId)
                    .col(AiResourceVersion::Name)
                    .col(AiResourceVersion::Type)
                    .col(AiResourceVersion::Version)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_ai_resource_ver_name")
                    .table(AiResourceVersion::Table)
                    .col(AiResourceVersion::Name)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_ai_resource_ver_status")
                    .table(AiResourceVersion::Table)
                    .col(AiResourceVersion::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_ai_resource_ver_gmt_modified")
                    .table(AiResourceVersion::Table)
                    .col(AiResourceVersion::GmtModified)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AiResourceVersion::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AiResourceVersion {
    Table,
    Id,
    GmtCreate,
    GmtModified,
    Type,
    Author,
    Name,
    CDesc,
    Status,
    Version,
    NamespaceId,
    Storage,
    PublishPipelineInfo,
    DownloadCount,
}
