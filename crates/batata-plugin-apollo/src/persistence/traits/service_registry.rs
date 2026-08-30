use async_trait::async_trait;

/// One row of the upstream `ServiceRegistry` table (database-discovery mode).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceRegistryEntry {
    /// The `id` field.
    pub id: i32,
    /// The `service_name` field.
    pub service_name: String,
    /// The `uri` field.
    pub uri: String,
    /// The `cluster` field.
    pub cluster: String,
    /// The `metadata` field.
    pub metadata: Option<String>,
    /// millis; upstream compares `DataChange_LastTime > now - window`.
    pub data_change_last_time: i64,
}

#[async_trait]
/// Defines the `ServiceRegistryPersistence` trait.
pub trait ServiceRegistryPersistence: Send + Sync {
    /// Upstream `ServiceRegistryService.saveIfNotExistByServiceNameAndUri` +
    /// heartbeat: insert when (service_name, uri) is new, otherwise refresh
    /// `last_time` so discovery keeps considering this instance alive.
    async fn heartbeat(
        &self,
        service_name: &str,
        uri: &str,
        cluster: &str,
    ) -> anyhow::Result<()>;

    /// Remove the registration on graceful shutdown.
    async fn deregister(&self, service_name: &str, uri: &str) -> anyhow::Result<()>;

    /// Live instances of one service: rows whose last heartbeat is within
    /// `window_secs` of now (upstream default health-check interval 61s).
    async fn find_alive(
        &self,
        service_name: &str,
        window_secs: i64,
    ) -> anyhow::Result<Vec<ServiceRegistryEntry>>;
}
