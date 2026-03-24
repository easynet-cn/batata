// Console data source abstraction layer
// Provides a unified interface for console operations in both local and remote modes

pub mod local;
pub mod remote;

use batata_common::ClusterManager;

use std::sync::Arc;

// Re-export the ConsoleDataSource trait from server-common
pub use batata_server_common::console::datasource::ConsoleDataSource;

use batata_server_common::model::config::Configuration;

/// Create a console data source based on configuration
pub async fn create_datasource(
    configuration: &Configuration,
    cluster_manager: Option<Arc<dyn ClusterManager>>,
    config_subscriber_manager: Arc<dyn batata_common::ConfigSubscriptionService>,
    naming_service: Option<Arc<dyn batata_api::naming::NamingServiceProvider>>,
    persistence: Option<Arc<dyn batata_persistence::PersistenceService>>,
) -> anyhow::Result<Arc<dyn ConsoleDataSource>> {
    if configuration.is_console_remote_mode() {
        // Remote mode: use HTTP client to connect to server
        let remote_datasource = remote::RemoteDataSource::new(configuration).await?;
        Ok(Arc::new(remote_datasource))
    } else {
        // Local mode: direct PersistenceService access (works for all storage backends)
        let cm = cluster_manager
            .ok_or_else(|| anyhow::anyhow!("Cluster manager required for local console mode"))?;
        let persist = persistence.ok_or_else(|| {
            anyhow::anyhow!("Persistence service required for local console mode")
        })?;
        let local_datasource = local::LocalDataSource::new(
            persist,
            cm,
            config_subscriber_manager,
            configuration.clone(),
            naming_service,
        );
        Ok(Arc::new(local_datasource))
    }
}
