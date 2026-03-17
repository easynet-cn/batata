pub use sea_orm_migration::prelude::*;

mod m20250317_000001_create_config_info;
mod m20250317_000002_create_config_info_gray;
mod m20250317_000003_create_config_tags_relation;
mod m20250317_000004_create_group_capacity;
mod m20250317_000005_create_his_config_info;
mod m20250317_000006_create_tenant_capacity;
mod m20250317_000007_create_tenant_info;
mod m20250317_000008_create_users;
mod m20250317_000009_create_roles;
mod m20250317_000010_create_permissions;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250317_000001_create_config_info::Migration),
            Box::new(m20250317_000002_create_config_info_gray::Migration),
            Box::new(m20250317_000003_create_config_tags_relation::Migration),
            Box::new(m20250317_000004_create_group_capacity::Migration),
            Box::new(m20250317_000005_create_his_config_info::Migration),
            Box::new(m20250317_000006_create_tenant_capacity::Migration),
            Box::new(m20250317_000007_create_tenant_info::Migration),
            Box::new(m20250317_000008_create_users::Migration),
            Box::new(m20250317_000009_create_roles::Migration),
            Box::new(m20250317_000010_create_permissions::Migration),
        ]
    }
}
