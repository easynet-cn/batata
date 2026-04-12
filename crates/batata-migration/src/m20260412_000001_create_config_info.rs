use sea_orm_migration::{prelude::*, schema::*};

use crate::column_helper::long_text;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        manager
            .create_table(
                Table::create()
                    .table(ConfigInfo::Table)
                    .if_not_exists()
                    .col(big_integer(ConfigInfo::Id).auto_increment().primary_key())
                    .col(string_len(ConfigInfo::DataId, 255).not_null())
                    .col(string_len_null(ConfigInfo::GroupId, 128))
                    .col(long_text(ConfigInfo::Content, backend))
                    .col(string_len_null(ConfigInfo::Md5, 32))
                    .col(
                        date_time(ConfigInfo::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        date_time(ConfigInfo::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(text_null(ConfigInfo::SrcUser))
                    .col(string_len_null(ConfigInfo::SrcIp, 50))
                    .col(string_len_null(ConfigInfo::AppName, 128))
                    .col(string_len(ConfigInfo::TenantId, 128).not_null().default(""))
                    .col(string_len_null(ConfigInfo::CDesc, 256))
                    .col(string_len_null(ConfigInfo::CUse, 64))
                    .col(string_len_null(ConfigInfo::Effect, 64))
                    .col(string_len_null(ConfigInfo::Type, 64))
                    .col(text_null(ConfigInfo::CSchema))
                    .col(
                        string_len(ConfigInfo::EncryptedDataKey, 1024)
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        // UNIQUE KEY `uk_configinfo_datagrouptenant` (`data_id`,`group_id`,`tenant_id`)
        manager
            .create_index(
                Index::create()
                    .name("uk_configinfo_datagrouptenant")
                    .table(ConfigInfo::Table)
                    .col(ConfigInfo::DataId)
                    .col(ConfigInfo::GroupId)
                    .col(ConfigInfo::TenantId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_configinfo_tenant_group_modified` (`tenant_id`,`group_id`,`gmt_modified`)
        manager
            .create_index(
                Index::create()
                    .name("idx_configinfo_tenant_group_modified")
                    .table(ConfigInfo::Table)
                    .col(ConfigInfo::TenantId)
                    .col(ConfigInfo::GroupId)
                    .col(ConfigInfo::GmtModified)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ConfigInfo::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ConfigInfo {
    Table,
    Id,
    DataId,
    GroupId,
    Content,
    Md5,
    GmtCreate,
    GmtModified,
    SrcUser,
    SrcIp,
    AppName,
    TenantId,
    CDesc,
    CUse,
    Effect,
    Type,
    CSchema,
    EncryptedDataKey,
}
