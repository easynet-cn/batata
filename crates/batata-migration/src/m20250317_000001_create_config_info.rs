use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ConfigInfo::Table)
                    .if_not_exists()
                    .col(big_integer(ConfigInfo::Id).auto_increment().primary_key())
                    .col(string_len(ConfigInfo::DataId, 255).not_null())
                    .col(string_len_null(ConfigInfo::GroupId, 128))
                    .col(text(ConfigInfo::Content).not_null())
                    .col(string_len_null(ConfigInfo::Md5, 32))
                    .col(
                        timestamp(ConfigInfo::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp(ConfigInfo::GmtModified)
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

        // Unique index: data_id + group_id + tenant_id
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

        manager
            .create_index(
                Index::create()
                    .name("idx_config_tenant_id")
                    .table(ConfigInfo::Table)
                    .col(ConfigInfo::TenantId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_config_app_name")
                    .table(ConfigInfo::Table)
                    .col(ConfigInfo::AppName)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_config_gmt_modified")
                    .table(ConfigInfo::Table)
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
