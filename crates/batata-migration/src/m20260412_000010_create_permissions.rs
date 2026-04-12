use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Permissions::Table)
                    .if_not_exists()
                    .col(string_len(Permissions::Role, 50).not_null())
                    .col(string_len(Permissions::Resource, 128).not_null())
                    .col(string_len(Permissions::Action, 8).not_null())
                    .to_owned(),
            )
            .await?;

        // UNIQUE INDEX `uk_role_permission` (`role`,`resource`,`action`)
        manager
            .create_index(
                Index::create()
                    .name("uk_role_permission")
                    .table(Permissions::Table)
                    .col(Permissions::Role)
                    .col(Permissions::Resource)
                    .col(Permissions::Action)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Permissions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Permissions {
    Table,
    Role,
    Resource,
    Action,
}
