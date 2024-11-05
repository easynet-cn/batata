use config::Config;
use sea_orm::DatabaseConnection;

#[derive(Debug, Clone)]
pub struct AppState {
    pub app_config: Config,
    pub database_connection: DatabaseConnection,
}
