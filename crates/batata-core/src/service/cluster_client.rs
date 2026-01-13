// Cluster gRPC client for inter-node communication
// Provides methods to communicate with other cluster nodes via gRPC

use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::Duration,
};

use dashmap::DashMap;
use serde::Serialize;
use tonic::transport::Channel;
use tracing::{debug, info, warn};

use batata_api::{
    grpc::{Metadata, Payload, request_client::RequestClient},
    model::Member,
    remote::model::{LABEL_SOURCE, LABEL_SOURCE_CLUSTER, RequestTrait},
};
use prost_types::Any;

/// Configuration for cluster client
#[derive(Clone, Debug)]
pub struct ClusterClientConfig {
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Maximum retry count
    pub max_retries: u32,
    /// Retry delay
    pub retry_delay: Duration,
    /// Idle connection timeout - connections unused for this duration will be removed
    pub idle_timeout: Duration,
}

impl Default for ClusterClientConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(5),
            max_retries: 3,
            retry_delay: Duration::from_millis(500),
            idle_timeout: Duration::from_secs(300), // 5 minutes default
        }
    }
}

impl ClusterClientConfig {
    /// Create a ClusterClientConfig from application Configuration
    pub fn from_configuration(config: &crate::model::Configuration) -> Self {
        Self {
            connect_timeout: Duration::from_millis(config.cluster_connect_timeout_ms()),
            request_timeout: Duration::from_millis(config.cluster_request_timeout_ms()),
            max_retries: config.cluster_max_retries(),
            retry_delay: Duration::from_millis(config.cluster_retry_delay_ms()),
            idle_timeout: Duration::from_millis(config.cluster_idle_timeout_ms()),
        }
    }
}

/// A gRPC client connection to a cluster node
pub struct ClusterConnection {
    pub address: String,
    pub grpc_address: String,
    pub client: RequestClient<Channel>,
    pub created_at: i64,
    /// Last used timestamp - uses AtomicI64 for lock-free access
    pub last_used: AtomicI64,
}

impl ClusterConnection {
    /// Update the last used timestamp (lock-free)
    pub fn update_last_used(&self) {
        self.last_used.store(chrono::Utc::now().timestamp_millis(), Ordering::Relaxed);
    }

    /// Get the last used timestamp (lock-free)
    pub fn get_last_used(&self) -> i64 {
        self.last_used.load(Ordering::Relaxed)
    }
}

/// Cluster client manager
/// Manages connections to other cluster nodes
pub struct ClusterClientManager {
    config: ClusterClientConfig,
    connections: Arc<DashMap<String, Arc<ClusterConnection>>>,
    local_address: String,
}

impl ClusterClientManager {
    pub fn new(local_address: String, config: ClusterClientConfig) -> Self {
        Self {
            config,
            connections: Arc::new(DashMap::new()),
            local_address,
        }
    }

    /// Calculate the cluster gRPC port from the main port
    fn calculate_grpc_port(main_port: u16) -> u16 {
        main_port + 1001 // Cluster gRPC port offset
    }

    /// Get gRPC address from member address
    fn get_grpc_address(address: &str) -> Option<String> {
        let parts: Vec<&str> = address.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let ip = parts[0];
        let main_port: u16 = parts[1].parse().ok()?;
        let grpc_port = Self::calculate_grpc_port(main_port);

        Some(format!("http://{}:{}", ip, grpc_port))
    }

    /// Get or create a connection to a cluster node
    pub async fn get_connection(
        &self,
        address: &str,
    ) -> Result<Arc<ClusterConnection>, Box<dyn std::error::Error + Send + Sync>> {
        // Don't connect to self
        if address == self.local_address {
            return Err("Cannot connect to self".into());
        }

        // Return existing connection if available
        if let Some(conn) = self.connections.get(address) {
            conn.update_last_used();
            return Ok(conn.clone());
        }

        // Create new connection
        let grpc_address = Self::get_grpc_address(address)
            .ok_or_else(|| format!("Invalid address format: {}", address))?;

        info!("Creating cluster connection to {}", grpc_address);

        let channel = Channel::from_shared(grpc_address.clone())?
            .connect_timeout(self.config.connect_timeout)
            .timeout(self.config.request_timeout)
            .connect()
            .await?;

        let client = RequestClient::new(channel);

        let connection = Arc::new(ClusterConnection {
            address: address.to_string(),
            grpc_address,
            client,
            created_at: chrono::Utc::now().timestamp_millis(),
            last_used: AtomicI64::new(chrono::Utc::now().timestamp_millis()),
        });

        self.connections
            .insert(address.to_string(), connection.clone());

        Ok(connection)
    }

    /// Remove a connection
    pub fn remove_connection(&self, address: &str) {
        self.connections.remove(address);
        debug!("Removed cluster connection to {}", address);
    }

    /// Send a request to a cluster node
    pub async fn send_request<T: RequestTrait + Serialize + Sync>(
        &self,
        address: &str,
        request: T,
    ) -> Result<Payload, Box<dyn std::error::Error + Send + Sync>> {
        let mut last_error = None;

        for attempt in 0..self.config.max_retries {
            match self.try_send_request(address, &request).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    warn!(
                        "Failed to send request to {} (attempt {}/{}): {}",
                        address,
                        attempt + 1,
                        self.config.max_retries,
                        e
                    );
                    last_error = Some(e);

                    // Remove failed connection
                    self.remove_connection(address);

                    if attempt < self.config.max_retries - 1 {
                        tokio::time::sleep(self.config.retry_delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| "Unknown error".into()))
    }

    /// Try to send a request once
    async fn try_send_request<T: RequestTrait + Serialize>(
        &self,
        address: &str,
        request: &T,
    ) -> Result<Payload, Box<dyn std::error::Error + Send + Sync>> {
        let connection = self.get_connection(address).await?;

        let mut metadata = Metadata {
            r#type: request.request_type().to_string(),
            ..Default::default()
        };

        // Add cluster source label
        metadata
            .headers
            .insert(LABEL_SOURCE.to_string(), LABEL_SOURCE_CLUSTER.to_string());

        // Create payload manually since RequestTrait doesn't have into_payload
        let body = request.body();
        let payload = Payload {
            metadata: Some(metadata),
            body: Some(Any {
                type_url: String::default(),
                value: body,
            }),
        };

        let mut client = connection.client.clone();
        let response = client.request(payload).await?;

        connection.update_last_used();

        Ok(response.into_inner())
    }

    /// Broadcast a request to all cluster nodes
    pub async fn broadcast<T: RequestTrait + Serialize + Clone + Send + Sync + 'static>(
        &self,
        members: &[Member],
        request: T,
    ) -> Vec<(String, Result<Payload, String>)> {
        let mut handles = Vec::new();

        for member in members {
            // Skip self
            if member.address == self.local_address {
                continue;
            }

            let address = member.address.clone();
            let req = request.clone();
            let manager = self.clone_manager();

            let handle = tokio::spawn(async move {
                let result = manager.send_request(&address, req).await;
                (address, result.map_err(|e| e.to_string()))
            });

            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(result) = handle.await {
                results.push(result);
            }
        }

        results
    }

    /// Clone the manager for use in async tasks
    fn clone_manager(&self) -> ClusterClientManager {
        ClusterClientManager {
            config: self.config.clone(),
            connections: self.connections.clone(),
            local_address: self.local_address.clone(),
        }
    }

    /// Get all active connections
    pub fn get_all_connections(&self) -> Vec<String> {
        self.connections.iter().map(|e| e.key().clone()).collect()
    }

    /// Close all connections
    pub fn close_all(&self) {
        self.connections.clear();
        info!("Closed all cluster connections");
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Clean up idle connections that haven't been used for longer than idle_timeout
    /// This is now lock-free and doesn't require async
    pub fn cleanup_idle_connections(&self) -> usize {
        let now = chrono::Utc::now().timestamp_millis();
        let idle_timeout_ms = self.config.idle_timeout.as_millis() as i64;
        let mut removed = 0;

        // Collect keys to remove (lock-free atomic read)
        let keys_to_remove: Vec<String> = {
            let mut keys = Vec::new();
            for entry in self.connections.iter() {
                let last_used = entry.value().get_last_used();
                if now - last_used > idle_timeout_ms {
                    keys.push(entry.key().clone());
                }
            }
            keys
        };

        // Remove stale connections
        for key in keys_to_remove {
            self.connections.remove(&key);
            debug!("Removed idle cluster connection to {} (idle timeout)", key);
            removed += 1;
        }

        if removed > 0 {
            info!(
                "Cleaned up {} idle cluster connections (threshold: {:?})",
                removed, self.config.idle_timeout
            );
        }

        removed
    }

    /// Start a background task that periodically cleans up idle connections
    pub fn start_cleanup_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let cleanup_interval = self.config.idle_timeout / 2; // Check at half the idle timeout

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                interval.tick().await;
                self.cleanup_idle_connections();
            }
        })
    }
}

/// Cluster request sender helper
pub struct ClusterRequestSender {
    client_manager: Arc<ClusterClientManager>,
}

impl ClusterRequestSender {
    pub fn new(client_manager: Arc<ClusterClientManager>) -> Self {
        Self { client_manager }
    }

    /// Send config change sync to a specific node
    pub async fn send_config_change_sync(
        &self,
        address: &str,
        data_id: &str,
        group: &str,
        tenant: &str,
        last_modified: i64,
    ) -> Result<Payload, Box<dyn std::error::Error + Send + Sync>> {
        use batata_api::config::ConfigChangeClusterSyncRequest;

        let mut request = ConfigChangeClusterSyncRequest::new();
        request.config_request.data_id = data_id.to_string();
        request.config_request.group = group.to_string();
        request.config_request.tenant = tenant.to_string();
        request.last_modified = last_modified;

        self.client_manager.send_request(address, request).await
    }

    /// Broadcast config change to all cluster nodes
    pub async fn broadcast_config_change(
        &self,
        members: &[Member],
        data_id: &str,
        group: &str,
        tenant: &str,
        last_modified: i64,
    ) -> Vec<(String, Result<Payload, String>)> {
        use batata_api::config::ConfigChangeClusterSyncRequest;

        let mut request = ConfigChangeClusterSyncRequest::new();
        request.config_request.data_id = data_id.to_string();
        request.config_request.group = group.to_string();
        request.config_request.tenant = tenant.to_string();
        request.last_modified = last_modified;

        self.client_manager.broadcast(members, request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_grpc_port() {
        assert_eq!(ClusterClientManager::calculate_grpc_port(8848), 9849);
        assert_eq!(ClusterClientManager::calculate_grpc_port(8849), 9850);
    }

    #[test]
    fn test_get_grpc_address() {
        let addr = ClusterClientManager::get_grpc_address("192.168.1.1:8848");
        assert_eq!(addr, Some("http://192.168.1.1:9849".to_string()));

        let addr = ClusterClientManager::get_grpc_address("invalid");
        assert_eq!(addr, None);
    }

    #[test]
    fn test_cluster_client_config_default() {
        let config = ClusterClientConfig::default();
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.request_timeout, Duration::from_secs(5));
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay, Duration::from_millis(500));
        assert_eq!(config.idle_timeout, Duration::from_secs(300));
    }

    #[test]
    fn test_cluster_client_manager_new() {
        let config = ClusterClientConfig::default();
        let manager = ClusterClientManager::new("127.0.0.1:8848".to_string(), config);
        assert_eq!(manager.connection_count(), 0);
        assert!(manager.get_all_connections().is_empty());
    }

    #[test]
    fn test_cleanup_idle_connections_empty() {
        let config = ClusterClientConfig::default();
        let manager = ClusterClientManager::new("127.0.0.1:8848".to_string(), config);
        let removed = manager.cleanup_idle_connections();
        assert_eq!(removed, 0);
    }

    #[tokio::test]
    async fn test_cannot_connect_to_self() {
        let config = ClusterClientConfig::default();
        let manager = ClusterClientManager::new("127.0.0.1:8848".to_string(), config);

        let result = manager.get_connection("127.0.0.1:8848").await;
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Cannot connect to self"));
    }
}
