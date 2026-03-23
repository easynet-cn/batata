// Console data source abstraction layer
// Provides a unified interface for console operations in both local and remote modes

pub mod embedded;
pub mod local;
pub mod remote;

use sea_orm::DatabaseConnection;

use batata_common::ClusterManager;

use std::sync::Arc;

// Re-export the ConsoleDataSource trait from server-common
pub use batata_server_common::console::datasource::ConsoleDataSource;

use batata_server_common::model::config::Configuration;

/// Create a console data source based on configuration
pub async fn create_datasource(
    configuration: &Configuration,
    database_connection: Option<DatabaseConnection>,
    cluster_manager: Option<Arc<dyn ClusterManager>>,
    config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
    naming_service: Option<Arc<dyn batata_api::naming::NamingServiceProvider>>,
    persistence: Option<Arc<dyn batata_persistence::PersistenceService>>,
) -> anyhow::Result<Arc<dyn ConsoleDataSource>> {
    if configuration.is_console_remote_mode() {
        // Remote mode: use HTTP client to connect to server
        let remote_datasource = remote::RemoteDataSource::new(configuration).await?;
        Ok(Arc::new(remote_datasource))
    } else if let Some(db) = database_connection {
        // Local mode with external DB: direct database access
        let cm = cluster_manager
            .ok_or_else(|| anyhow::anyhow!("Cluster manager required for local console mode"))?;
        let local_datasource = local::LocalDataSource::new(
            db,
            cm,
            config_subscriber_manager,
            configuration.clone(),
            naming_service,
        );
        Ok(Arc::new(local_datasource))
    } else {
        // Embedded mode (standalone/distributed): use direct PersistenceService access
        let cm = cluster_manager
            .ok_or_else(|| anyhow::anyhow!("Cluster manager required for embedded console mode"))?;
        let persist = persistence.ok_or_else(|| {
            anyhow::anyhow!("Persistence service required for embedded console mode")
        })?;
        let embedded_ds = embedded::EmbeddedLocalDataSource::new(
            persist,
            cm,
            config_subscriber_manager,
            configuration.clone(),
            naming_service,
        );
        Ok(Arc::new(embedded_ds))
    }
}
