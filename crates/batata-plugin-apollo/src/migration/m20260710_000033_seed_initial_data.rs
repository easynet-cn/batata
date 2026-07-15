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

async fn seed_server_config(
    conn: &dyn ConnectionTrait,
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
                conn.execute(Statement::from_sql_and_values(
                    backend,
                    r#"INSERT IGNORE INTO `apollo_server_config` (`key`, `value`, `comment`, `data_change_created_by`) VALUES (?, ?, ?, 'default')"#,
                    vec![
                        Value::String(Some(Box::new(key.to_string()))),
                        Value::String(Some(Box::new(value.to_string()))),
                        Value::String(Some(Box::new(comment.to_string()))),
                    ],
                ))
                .await?;
            }
            DatabaseBackend::Postgres => {
                conn.execute(Statement::from_sql_and_values(
                    backend,
                    r#"INSERT INTO "apollo_server_config" ("key", "value", "comment", "data_change_created_by") VALUES ($1, $2, $3, 'default') ON CONFLICT ("key", "deleted_at") DO NOTHING"#,
                    vec![
                        Value::String(Some(Box::new(key.to_string()))),
                        Value::String(Some(Box::new(value.to_string()))),
                        Value::String(Some(Box::new(comment.to_string()))),
                    ],
                ))
                .await?;
            }
            DatabaseBackend::Sqlite => {
                conn.execute(Statement::from_sql_and_values(
                    backend,
                    r#"INSERT OR IGNORE INTO "apollo_server_config" ("key", "value", "comment", "data_change_created_by") VALUES (?, ?, ?, 'default')"#,
                    vec![
                        Value::String(Some(Box::new(key.to_string()))),
                        Value::String(Some(Box::new(value.to_string()))),
                        Value::String(Some(Box::new(comment.to_string()))),
                    ],
                ))
                .await?;
            }
        }
    }

    Ok(())
}

async fn seed_admin_user(
    conn: &dyn ConnectionTrait,
    backend: DatabaseBackend,
) -> Result<(), DbErr> {
    let username = "apollo";
    let password = "$2a$10$7r20uS.BQ9uBpf3Baj3uQOZvMVvB1RN3PYoKE94gtz2.WAOuiiwXS";
    let display_name = "apollo";
    let email = "apollo@acme.com";

    match backend {
        DatabaseBackend::MySql => {
            conn.execute(Statement::from_sql_and_values(
                backend,
                r#"INSERT IGNORE INTO `apollo_users` (`username`, `password`, `user_display_name`, `email`, `enabled`) VALUES (?, ?, ?, ?, 1)"#,
                vec![
                    Value::String(Some(Box::new(username.to_string()))),
                    Value::String(Some(Box::new(password.to_string()))),
                    Value::String(Some(Box::new(display_name.to_string()))),
                    Value::String(Some(Box::new(email.to_string()))),
                ],
            ))
            .await?;
        }
        DatabaseBackend::Postgres => {
            conn.execute(Statement::from_sql_and_values(
                backend,
                r#"INSERT INTO "apollo_users" ("username", "password", "user_display_name", "email", "enabled") VALUES ($1, $2, $3, $4, 1) ON CONFLICT ("username") DO NOTHING"#,
                vec![
                    Value::String(Some(Box::new(username.to_string()))),
                    Value::String(Some(Box::new(password.to_string()))),
                    Value::String(Some(Box::new(display_name.to_string()))),
                    Value::String(Some(Box::new(email.to_string()))),
                ],
            ))
            .await?;
        }
        DatabaseBackend::Sqlite => {
            conn.execute(Statement::from_sql_and_values(
                backend,
                r#"INSERT OR IGNORE INTO "apollo_users" ("username", "password", "user_display_name", "email", "enabled") VALUES (?, ?, ?, ?, 1)"#,
                vec![
                    Value::String(Some(Box::new(username.to_string()))),
                    Value::String(Some(Box::new(password.to_string()))),
                    Value::String(Some(Box::new(display_name.to_string()))),
                    Value::String(Some(Box::new(email.to_string()))),
                ],
            ))
            .await?;
        }
    }

    Ok(())
}
