//! Add `source` column to `users` table.
//!
//! Tracks the identity provider that owns each account so the auth layer
//! can reject password-login attempts for OAuth/LDAP-provisioned users.
//!
//! Existing rows are NULL after the ALTER and are treated as "local" by
//! the application code (see `normalize_user_source`).

use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column_if_not_exists(string_len_null(Users::Source, 20))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::Source)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Source,
}
