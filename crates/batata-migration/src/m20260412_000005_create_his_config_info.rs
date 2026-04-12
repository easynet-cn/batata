use sea_orm_migration::{prelude::*, schema::*};
use crate::column_helper::{long_text, long_text_null};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(HisConfigInfo::Table)
                    .if_not_exists()
                    .col(big_integer(HisConfigInfo::Id).not_null())
                    .col(
                        big_integer(HisConfigInfo::Nid)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(string_len(HisConfigInfo::DataId, 255).not_null())
                    .col(string_len(HisConfigInfo::GroupId, 128).not_null())
                    .col(string_len_null(HisConfigInfo::AppName, 128))
                    .col(long_text(HisConfigInfo::Content, manager.get_database_backend()).not_null())
                    .col(string_len_null(HisConfigInfo::Md5, 32))
                    .col(
                        date_time(HisConfigInfo::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        date_time(HisConfigInfo::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(text_null(HisConfigInfo::SrcUser))
                    .col(string_len_null(HisConfigInfo::SrcIp, 50))
                    .col(char_len_null(HisConfigInfo::OpType, 10))
                    .col(
                        string_len(HisConfigInfo::TenantId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        string_len(HisConfigInfo::EncryptedDataKey, 1024)
                            .not_null()
                            .default(""),
                    )
                    .col(string_len_null(HisConfigInfo::PublishType, 50).default("formal"))
                    .col(string_len_null(HisConfigInfo::GrayName, 50))
                    .col(long_text_null(HisConfigInfo::ExtInfo, manager.get_database_backend()))
                    .to_owned(),
            )
            .await?;

        // KEY `idx_gmt_create` (`gmt_create`)
        manager
            .create_index(
                Index::create()
                    .name("idx_gmt_create")
                    .table(HisConfigInfo::Table)
                    .col(HisConfigInfo::GmtCreate)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_gmt_modified` (`gmt_modified`)
        manager
            .create_index(
                Index::create()
                    .name("idx_his_gmt_modified")
                    .table(HisConfigInfo::Table)
                    .col(HisConfigInfo::GmtModified)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_did` (`data_id`)
        manager
            .create_index(
                Index::create()
                    .name("idx_did")
                    .table(HisConfigInfo::Table)
                    .col(HisConfigInfo::DataId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(HisConfigInfo::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum HisConfigInfo {
    Table,
    Id,
    Nid,
    DataId,
    GroupId,
    AppName,
    Content,
    Md5,
    GmtCreate,
    GmtModified,
    SrcUser,
    SrcIp,
    OpType,
    TenantId,
    EncryptedDataKey,
    PublishType,
    GrayName,
    ExtInfo,
}
