use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TenantCapacity::Table)
                    .if_not_exists()
                    .col(
                        big_unsigned(TenantCapacity::Id)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        string_len(TenantCapacity::TenantId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(unsigned(TenantCapacity::Quota).not_null().default(0))
                    .col(unsigned(TenantCapacity::Usage).not_null().default(0))
                    .col(unsigned(TenantCapacity::MaxSize).not_null().default(0))
                    .col(unsigned(TenantCapacity::MaxAggrCount).not_null().default(0))
                    .col(unsigned(TenantCapacity::MaxAggrSize).not_null().default(0))
                    .col(
                        unsigned(TenantCapacity::MaxHistoryCount)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        timestamp(TenantCapacity::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp(TenantCapacity::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_tenant_capacity_tenant_id")
                    .table(TenantCapacity::Table)
                    .col(TenantCapacity::TenantId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TenantCapacity::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TenantCapacity {
    Table,
    Id,
    TenantId,
    Quota,
    Usage,
    MaxSize,
    MaxAggrCount,
    MaxAggrSize,
    MaxHistoryCount,
    GmtCreate,
    GmtModified,
}
