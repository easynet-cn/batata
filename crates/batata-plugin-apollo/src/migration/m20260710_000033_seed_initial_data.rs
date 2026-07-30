use sea_orm::{DatabaseBackend, Statement, Value, ConnectionTrait};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        let conn = manager.get_connection();

        seed_server_config(conn, backend).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

async fn seed_server_config<C: ConnectionTrait>(
    conn: &C,
    backend: DatabaseBackend,
) -> Result<(), DbErr> {
    let configs = vec![
        (
            "namespace.lock.switch",
            "false",
            "一次发布只能有一个人修改开关",
        ),
        (
            "item.key.length.limit",
            "128",
            "item key 最大长度限制",
        ),
        (
            "item.value.length.limit",
            "20000",
            "item value最大长度限制",
        ),
        (
            "config-service.cache.enabled",
            "false",
            "ConfigService是否开启缓存，开启后能提高性能，但是会增大内存消耗！",
        ),
        (
            "config-service.incremental.change.enabled",
            "false",
            "ConfigService是否开启增量配置同步客户端，开启后能提高性能，但是会增大内存消耗！",
        ),
    ];

    for (key, value, comment) in configs {
        match backend {
            DatabaseBackend::MySql => {
                conn.execute_raw(Statement::from_sql_and_values(
                    backend,
                    r#"INSERT IGNORE INTO `apollo_server_config` (`key`, `value`, `comment`, `data_change_created_by`) VALUES (?, ?, ?, 'default')"#,
                    vec![
                        Value::String(Some(key.to_string())),
                        Value::String(Some(value.to_string())),
                        Value::String(Some(comment.to_string())),
                    ],
                ))
                .await?;
            }
            DatabaseBackend::Postgres => {
                conn.execute_raw(Statement::from_sql_and_values(
                    backend,
                    r#"INSERT INTO "apollo_server_config" ("key", "value", "comment", "data_change_created_by") VALUES ($1, $2, $3, 'default') ON CONFLICT ("key") DO NOTHING"#,
                    vec![
                        Value::String(Some(key.to_string())),
                        Value::String(Some(value.to_string())),
                        Value::String(Some(comment.to_string())),
                    ],
                ))
                .await?;
            }
            DatabaseBackend::Sqlite => {
                conn.execute_raw(Statement::from_sql_and_values(
                    backend,
                    r#"INSERT OR IGNORE INTO "apollo_server_config" ("key", "value", "comment", "data_change_created_by") VALUES (?, ?, ?, 'default')"#,
                    vec![
                        Value::String(Some(key.to_string())),
                        Value::String(Some(value.to_string())),
                        Value::String(Some(comment.to_string())),
                    ],
                ))
                .await?;
            }
            _ => {}
        }
    }

    Ok(())
}

