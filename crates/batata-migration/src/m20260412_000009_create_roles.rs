use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Roles::Table)
                    .if_not_exists()
                    .col(string_len(Roles::Username, 50).not_null())
                    .col(string_len(Roles::Role, 50).not_null())
                    .to_owned(),
            )
            .await?;

        // UNIQUE INDEX `idx_user_role` (`username`, `role`)
        manager
            .create_index(
                Index::create()
                    .name("idx_user_role")
                    .table(Roles::Table)
                    .col(Roles::Username)
                    .col(Roles::Role)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Roles::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Roles {
    Table,
    Username,
    Role,
}
