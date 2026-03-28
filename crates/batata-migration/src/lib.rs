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

/// Total number of migrations — update when adding new migrations
pub const MIGRATION_COUNT: usize = 10;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_count() {
        let migrations = Migrator::migrations();
        assert_eq!(
            migrations.len(),
            MIGRATION_COUNT,
            "Migration count constant ({}) does not match actual migrations ({}). \
             Update MIGRATION_COUNT when adding new migrations.",
            MIGRATION_COUNT,
            migrations.len()
        );
    }

    #[test]
    fn test_migration_names_are_unique() {
        let migrations = Migrator::migrations();
        let names: Vec<String> = migrations.iter().map(|m| m.name().to_string()).collect();

        let mut seen = std::collections::HashSet::new();
        for name in &names {
            assert!(
                seen.insert(name.clone()),
                "Duplicate migration name: {}",
                name
            );
        }
    }

    #[test]
    fn test_migration_names_are_chronologically_ordered() {
        let migrations = Migrator::migrations();
        let names: Vec<String> = migrations.iter().map(|m| m.name().to_string()).collect();

        for i in 1..names.len() {
            assert!(
                names[i] > names[i - 1],
                "Migrations are not in chronological order: '{}' should come after '{}'",
                names[i],
                names[i - 1]
            );
        }
    }

    #[test]
    fn test_migration_names_follow_convention() {
        let migrations = Migrator::migrations();
        for migration in &migrations {
            let name = migration.name().to_string();
            // Expected format: m<YYYYMMDD>_<sequence>_<description>
            assert!(
                name.starts_with("m202"),
                "Migration name '{}' does not follow convention m<YYYYMMDD>_<seq>_<desc>",
                name
            );
        }
    }

    #[test]
    fn test_core_tables_present() {
        let migrations = Migrator::migrations();
        let names: Vec<String> = migrations.iter().map(|m| m.name().to_string()).collect();
        let names_str = names.join(",");

        assert!(
            names_str.contains("config_info"),
            "Missing config_info migration"
        );
        assert!(names_str.contains("users"), "Missing users migration");
        assert!(names_str.contains("roles"), "Missing roles migration");
        assert!(
            names_str.contains("permissions"),
            "Missing permissions migration"
        );
        assert!(
            names_str.contains("tenant_info"),
            "Missing tenant_info (namespace) migration"
        );
    }
}
