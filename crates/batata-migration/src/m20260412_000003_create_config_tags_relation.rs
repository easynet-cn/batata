use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ConfigTagsRelation::Table)
                    .if_not_exists()
                    .col(big_integer(ConfigTagsRelation::Id).not_null())
                    .col(string_len(ConfigTagsRelation::TagName, 128).not_null())
                    .col(string_len_null(ConfigTagsRelation::TagType, 64))
                    .col(string_len(ConfigTagsRelation::DataId, 255).not_null())
                    .col(string_len(ConfigTagsRelation::GroupId, 128).not_null())
                    .col(
                        string_len(ConfigTagsRelation::TenantId, 128)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        big_integer(ConfigTagsRelation::Nid)
                            .auto_increment()
                            .primary_key(),
                    )
                    .to_owned(),
            )
            .await?;

        // UNIQUE KEY `uk_configtagrelation_configidtag` (`id`,`tag_name`,`tag_type`)
        manager
            .create_index(
                Index::create()
                    .name("uk_configtagrelation_configidtag")
                    .table(ConfigTagsRelation::Table)
                    .col(ConfigTagsRelation::Id)
                    .col(ConfigTagsRelation::TagName)
                    .col(ConfigTagsRelation::TagType)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // KEY `idx_tenant_id` (`tenant_id`)
        manager
            .create_index(
                Index::create()
                    .name("idx_tenant_id")
                    .table(ConfigTagsRelation::Table)
                    .col(ConfigTagsRelation::TenantId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ConfigTagsRelation::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ConfigTagsRelation {
    Table,
    Id,
    TagName,
    TagType,
    DataId,
    GroupId,
    TenantId,
    Nid,
}
