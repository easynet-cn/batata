use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TenantInfo::Table)
                    .if_not_exists()
                    .col(big_integer(TenantInfo::Id).auto_increment().primary_key())
                    .col(string_len(TenantInfo::Kp, 128).not_null())
                    .col(string_len(TenantInfo::TenantId, 128).not_null().default(""))
                    .col(
                        string_len(TenantInfo::TenantName, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(string_len_null(TenantInfo::TenantDesc, 256))
                    .col(string_len_null(TenantInfo::CreateSource, 32))
                    .col(big_integer(TenantInfo::GmtCreate).not_null())
                    .col(big_integer(TenantInfo::GmtModified).not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_tenant_info_kptenantid")
                    .table(TenantInfo::Table)
                    .col(TenantInfo::Kp)
                    .col(TenantInfo::TenantId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tenant_info_tenant_id")
                    .table(TenantInfo::Table)
                    .col(TenantInfo::TenantId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TenantInfo::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TenantInfo {
    Table,
    Id,
    Kp,
    TenantId,
    TenantName,
    TenantDesc,
    CreateSource,
    GmtCreate,
    GmtModified,
}
