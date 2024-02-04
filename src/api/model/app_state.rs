use sea_orm::DatabaseConnection;

#[derive(Debug, Clone)]
pub struct AppState {
    pub conns: Vec<DatabaseConnection>,
}
