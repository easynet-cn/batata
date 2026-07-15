use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ApolloNamespace::Table)
                    .add_column(ColumnDef::new(ApolloNamespace::Format).string_len(32).not_null().default("properties"))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ApolloNamespace::Table)
                    .add_column(ColumnDef::new(ApolloNamespace::IsPublic).boolean().not_null().default(false))
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ApolloNamespace::Table)
                    .add_column(ColumnDef::new(ApolloNamespace::Comment).string_len(500).null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ApolloNamespace::Table)
                    .drop_column(ApolloNamespace::Comment)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ApolloNamespace::Table)
                    .drop_column(ApolloNamespace::IsPublic)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ApolloNamespace::Table)
                    .drop_column(ApolloNamespace::Format)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ApolloNamespace {
    Table,
    Format,
    IsPublic,
    Comment,
}
