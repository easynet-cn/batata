use config::Config;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AppState {
    pub app_config: Config,
    pub database_connection: DatabaseConnection,
    pub context_path: String,
    pub token_secret_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResult {
    pub timestamp: String,
    pub status: i32,
    pub error: String,
    pub message: String,
    pub path: String,
}
