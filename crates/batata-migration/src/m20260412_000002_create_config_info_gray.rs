use sea_orm_migration::{prelude::*, schema::*};
use crate::column_helper::long_text;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ConfigInfoGray::Table)
                    .if_not_exists()
                    .col(
                        big_integer(ConfigInfoGray::Id)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(string_len(ConfigInfoGray::DataId, 255).not_null())
                    .col(string_len(ConfigInfoGray::GroupId, 128).not_null())
                    .col(long_text(ConfigInfoGray::Content, manager.get_database_backend()).not_null())
                    .col(string_len_null(ConfigInfoGray::Md5, 32))
                    .col(text_null(ConfigInfoGray::SrcUser))
                    .col(string_len_null(ConfigInfoGray::SrcIp, 100))
                    .col(
                        date_time(ConfigInfoGray::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        date_time(ConfigInfoGray::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(string_len_null(ConfigInfoGray::AppName, 128))
                    .col(
                        string_len(ConfigInfoGray::TenantId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(string_len(ConfigInfoGray::GrayName, 128).not_null())
                    .col(text(ConfigInfoGray::GrayRule).not_null())
                    .col(
                        string_len(ConfigInfoGray::EncryptedDataKey, 256)
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        // UNIQUE KEY `uk_configinfogray_datagrouptenantgray`
        manager
            .create_index(
                Index::create()
                    .name("uk_configinfogray_datagrouptenantgray")
                    .table(ConfigInfoGray::Table)
                    .col(ConfigInfoGray::DataId)
                    .col(ConfigInfoGray::GroupId)
                    .col(ConfigInfoGray::TenantId)
                    .col(ConfigInfoGray::GrayName)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_dataid_gmt_modified` (`data_id`,`gmt_modified`)
        manager
            .create_index(
                Index::create()
                    .name("idx_dataid_gmt_modified")
                    .table(ConfigInfoGray::Table)
                    .col(ConfigInfoGray::DataId)
                    .col(ConfigInfoGray::GmtModified)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_gmt_modified` (`gmt_modified`)
        manager
            .create_index(
                Index::create()
                    .name("idx_gmt_modified")
                    .table(ConfigInfoGray::Table)
                    .col(ConfigInfoGray::GmtModified)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ConfigInfoGray::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ConfigInfoGray {
    Table,
    Id,
    DataId,
    GroupId,
    Content,
    Md5,
    SrcUser,
    SrcIp,
    GmtCreate,
    GmtModified,
    AppName,
    TenantId,
    GrayName,
    GrayRule,
    EncryptedDataKey,
}
