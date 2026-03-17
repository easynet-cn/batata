use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GroupCapacity::Table)
                    .if_not_exists()
                    .col(
                        big_unsigned(GroupCapacity::Id)
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        string_len(GroupCapacity::GroupId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(unsigned(GroupCapacity::Quota).not_null().default(0))
                    .col(unsigned(GroupCapacity::Usage).not_null().default(0))
                    .col(unsigned(GroupCapacity::MaxSize).not_null().default(0))
                    .col(unsigned(GroupCapacity::MaxAggrCount).not_null().default(0))
                    .col(unsigned(GroupCapacity::MaxAggrSize).not_null().default(0))
                    .col(
                        unsigned(GroupCapacity::MaxHistoryCount)
                            .not_null()
                            .default(0),
                    )
                    .col(
                        timestamp(GroupCapacity::GmtCreate)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        timestamp(GroupCapacity::GmtModified)
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uk_group_id")
                    .table(GroupCapacity::Table)
                    .col(GroupCapacity::GroupId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(GroupCapacity::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum GroupCapacity {
    Table,
    Id,
    GroupId,
    Quota,
    Usage,
    MaxSize,
    MaxAggrCount,
    MaxAggrSize,
    MaxHistoryCount,
    GmtCreate,
    GmtModified,
}
