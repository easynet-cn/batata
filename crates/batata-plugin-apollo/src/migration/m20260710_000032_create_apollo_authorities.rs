use sea_orm_migration::prelude::*;

use crate::migration::column_helper::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();

        manager
            .create_table(
                Table::create()
                    .table(ApolloAuthorities::Table)
                    .if_not_exists()
                    .col(unsigned_int(ApolloAuthorities::Id, backend).auto_increment().primary_key())
                    .col(string_len(ApolloAuthorities::Username, 64))
                    .col(string_len(ApolloAuthorities::Authority, 50))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_apollo_authorities_username")
                    .table(ApolloAuthorities::Table)
                    .col(ApolloAuthorities::Username)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ApolloAuthorities::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloAuthorities {
    Table,
    Id,
    Username,
    Authority,
}
