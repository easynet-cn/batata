// Console data source abstraction layer
// Provides a unified interface for console operations in both local and remote modes

pub mod embedded;
pub mod local;
pub mod remote;

use sea_orm::DatabaseConnection;

use batata_core::cluster::ServerMemberManager;

use std::sync::Arc;

// Re-export the ConsoleDataSource trait from server-common
pub use batata_server_common::console::datasource::ConsoleDataSource;

use batata_server_common::model::config::Configuration;

/// Create a console data source based on configuration
pub async fn create_datasource(
    configuration: &Configuration,
    database_connection: Option<DatabaseConnection>,
    server_member_manager: Option<Arc<ServerMemberManager>>,
    config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
    naming_service: Option<Arc<batata_naming::service::NamingService>>,
    persistence: Option<Arc<dyn batata_persistence::PersistenceService>>,
) -> anyhow::Result<Arc<dyn ConsoleDataSource>> {
    if configuration.is_console_remote_mode() {
        // Remote mode: use HTTP client to connect to server
        let remote_datasource = remote::RemoteDataSource::new(configuration).await?;
        Ok(Arc::new(remote_datasource))
    } else if let Some(db) = database_connection {
        // Local mode with external DB: direct database access
        let smm = server_member_manager.ok_or_else(|| {
            anyhow::anyhow!("Server member manager required for local console mode")
        })?;
        let local_datasource = local::LocalDataSource::new(
            db,
            smm,
            config_subscriber_manager,
            configuration.clone(),
            naming_service,
        );
        Ok(Arc::new(local_datasource))
    } else {
        // Embedded mode (standalone/distributed): use direct PersistenceService access
        let smm = server_member_manager.ok_or_else(|| {
            anyhow::anyhow!("Server member manager required for embedded console mode")
        })?;
        let persist = persistence.ok_or_else(|| {
            anyhow::anyhow!("Persistence service required for embedded console mode")
        })?;
        let embedded_ds = embedded::EmbeddedLocalDataSource::new(
            persist,
            smm,
            config_subscriber_manager,
            configuration.clone(),
            naming_service,
        );
        Ok(Arc::new(embedded_ds))
    }
}
