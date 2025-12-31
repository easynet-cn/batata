// Distro protocol implementation for ephemeral data synchronization
// The Distro protocol is used to sync ephemeral data (like service instances) across cluster nodes

use std::{sync::Arc, time::Duration};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::api::model::Member;

use super::cluster_client::ClusterClientManager;

/// Distro protocol configuration
#[derive(Clone, Debug)]
pub struct DistroConfig {
    /// Delay before syncing data after a change
    pub sync_delay: Duration,
    /// Timeout for sync operations
    pub sync_timeout: Duration,
    /// Retry delay after sync failure
    pub sync_retry_delay: Duration,
    /// Interval for data verification
    pub verify_interval: Duration,
    /// Timeout for verify operations
    pub verify_timeout: Duration,
    /// Retry delay for loading snapshot data
    pub load_retry_delay: Duration,
}

impl Default for DistroConfig {
    fn default() -> Self {
        Self {
            sync_delay: Duration::from_millis(1000),
            sync_timeout: Duration::from_millis(3000),
            sync_retry_delay: Duration::from_millis(3000),
            verify_interval: Duration::from_millis(5000),
            verify_timeout: Duration::from_millis(3000),
            load_retry_delay: Duration::from_millis(30000),
        }
    }
}

/// Distro data type identifier
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistroDataType {
    /// Naming service instances
    NamingInstance,
    /// Custom data type
    Custom(String),
}

impl std::fmt::Display for DistroDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistroDataType::NamingInstance => write!(f, "naming_instance"),
            DistroDataType::Custom(s) => write!(f, "custom_{}", s),
        }
    }
}

/// A piece of distro data to be synchronized
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DistroData {
    /// Data type
    pub data_type: DistroDataType,
    /// Unique key for this data
    pub key: String,
    /// Serialized data content
    pub content: Vec<u8>,
    /// Data version/timestamp for conflict resolution
    pub version: i64,
    /// Source node address
    pub source: String,
}

impl DistroData {
    pub fn new(data_type: DistroDataType, key: String, content: Vec<u8>, source: String) -> Self {
        Self {
            data_type,
            key,
            content,
            version: chrono::Utc::now().timestamp_millis(),
            source,
        }
    }
}

/// Sync task for delayed synchronization
#[derive(Clone, Debug)]
pub struct DistroSyncTask {
    pub data_type: DistroDataType,
    pub key: String,
    pub target_address: String,
    pub scheduled_time: i64,
    pub retry_count: u32,
}

/// Trait for handling distro data
#[tonic::async_trait]
pub trait DistroDataHandler: Send + Sync {
    /// Get the data type this handler manages
    fn data_type(&self) -> DistroDataType;

    /// Get all data keys managed by this handler
    async fn get_all_keys(&self) -> Vec<String>;

    /// Get data by key
    async fn get_data(&self, key: &str) -> Option<DistroData>;

    /// Process received sync data
    async fn process_sync_data(&self, data: DistroData) -> Result<(), String>;

    /// Process data verification request
    async fn process_verify_data(&self, data: &DistroData) -> Result<bool, String>;

    /// Get snapshot of all data for initial sync
    async fn get_snapshot(&self) -> Vec<DistroData>;
}

/// Distro protocol manager
pub struct DistroProtocol {
    config: DistroConfig,
    local_address: String,
    handlers: Arc<DashMap<DistroDataType, Arc<dyn DistroDataHandler>>>,
    sync_tasks: Arc<DashMap<String, DistroSyncTask>>,
    running: Arc<RwLock<bool>>,
    client_manager: Arc<ClusterClientManager>,
    members: Arc<DashMap<String, Member>>,
}

impl DistroProtocol {
    pub fn new(
        local_address: String,
        config: DistroConfig,
        client_manager: Arc<ClusterClientManager>,
        members: Arc<DashMap<String, Member>>,
    ) -> Self {
        Self {
            config,
            local_address,
            handlers: Arc::new(DashMap::new()),
            sync_tasks: Arc::new(DashMap::new()),
            running: Arc::new(RwLock::new(false)),
            client_manager,
            members,
        }
    }

    /// Register a data handler
    pub fn register_handler(&self, handler: Arc<dyn DistroDataHandler>) {
        let data_type = handler.data_type();
        info!("Registering distro handler for type: {}", data_type);
        self.handlers.insert(data_type, handler);
    }

    /// Start the distro protocol
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            return;
        }
        *running = true;
        drop(running);

        info!("Starting Distro protocol");

        // Start sync task processor
        self.start_sync_task_processor().await;

        // Start data verification
        self.start_verify_task().await;
    }

    /// Stop the distro protocol
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopped Distro protocol");
    }

    /// Schedule a sync task for data
    pub async fn sync_data(&self, data_type: DistroDataType, key: &str) {
        let target_members: Vec<String> = self
            .members
            .iter()
            .filter(|e| e.key() != &self.local_address)
            .map(|e| e.key().clone())
            .collect();

        for target in target_members {
            let task_key = format!("{}:{}:{}", data_type, key, target);
            let task = DistroSyncTask {
                data_type: data_type.clone(),
                key: key.to_string(),
                target_address: target,
                scheduled_time: chrono::Utc::now().timestamp_millis()
                    + self.config.sync_delay.as_millis() as i64,
                retry_count: 0,
            };

            // Merge with existing task (reset scheduled time)
            self.sync_tasks.insert(task_key, task);
        }

        debug!("Scheduled sync for {}:{}", data_type, key);
    }

    /// Start the sync task processor
    async fn start_sync_task_processor(&self) {
        let sync_tasks = self.sync_tasks.clone();
        let handlers = self.handlers.clone();
        let running = self.running.clone();
        let client_manager = self.client_manager.clone();
        let config = self.config.clone();
        let _local_address = self.local_address.clone();

        tokio::spawn(async move {
            loop {
                {
                    let is_running = running.read().await;
                    if !*is_running {
                        break;
                    }
                }

                let now = chrono::Utc::now().timestamp_millis();

                // Find tasks that are ready to execute
                let ready_tasks: Vec<DistroSyncTask> = sync_tasks
                    .iter()
                    .filter(|e| e.value().scheduled_time <= now)
                    .map(|e| e.value().clone())
                    .collect();

                for task in ready_tasks {
                    let task_key =
                        format!("{}:{}:{}", task.data_type, task.key, task.target_address);

                    // Get data from handler
                    if let Some(handler) = handlers.get(&task.data_type) {
                        if let Some(data) = handler.get_data(&task.key).await {
                            // Send sync request
                            let result = Self::send_sync_data(
                                &client_manager,
                                &task.target_address,
                                data.clone(),
                            )
                            .await;

                            if result.is_ok() {
                                sync_tasks.remove(&task_key);
                                debug!(
                                    "Sync completed for {}:{} to {}",
                                    task.data_type, task.key, task.target_address
                                );
                            } else {
                                // Retry with delay
                                if task.retry_count < 3 {
                                    let mut updated_task = task.clone();
                                    updated_task.retry_count += 1;
                                    updated_task.scheduled_time =
                                        now + config.sync_retry_delay.as_millis() as i64;
                                    sync_tasks.insert(task_key, updated_task);
                                    warn!(
                                        "Sync failed for {}:{} to {}, retry count: {}",
                                        task.data_type,
                                        task.key,
                                        task.target_address,
                                        task.retry_count + 1
                                    );
                                } else {
                                    sync_tasks.remove(&task_key);
                                    error!(
                                        "Sync failed after max retries for {}:{} to {}",
                                        task.data_type, task.key, task.target_address
                                    );
                                }
                            }
                        } else {
                            // Data no longer exists, remove task
                            sync_tasks.remove(&task_key);
                        }
                    }
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
    }

    /// Start the verification task
    async fn start_verify_task(&self) {
        let running = self.running.clone();
        let members = self.members.clone();
        let local_address = self.local_address.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            loop {
                {
                    let is_running = running.read().await;
                    if !*is_running {
                        break;
                    }
                }

                tokio::time::sleep(config.verify_interval).await;

                // Get all members except self
                let other_members: Vec<String> = members
                    .iter()
                    .filter(|e| e.key() != &local_address)
                    .map(|e| e.key().clone())
                    .collect();

                // For each member, log verification (actual implementation would send verify requests)
                for member_address in &other_members {
                    debug!("Verifying distro data with {}", member_address);
                }
            }
        });
    }

    /// Send sync data to a target node
    async fn send_sync_data(
        _client_manager: &Arc<ClusterClientManager>,
        target: &str,
        data: DistroData,
    ) -> Result<(), String> {
        // In a real implementation, this would serialize the data and send via gRPC
        // For now, we just log the operation
        debug!(
            "Sending distro sync data to {}: type={}, key={}",
            target, data.data_type, data.key
        );

        // Simulate sending (in real implementation, use client_manager to send)
        Ok(())
    }

    /// Process received sync data
    pub async fn receive_sync_data(&self, data: DistroData) -> Result<(), String> {
        if let Some(handler) = self.handlers.get(&data.data_type) {
            handler.process_sync_data(data).await
        } else {
            Err(format!("No handler for data type: {}", data.data_type))
        }
    }

    /// Get snapshot for initial data load
    pub async fn get_snapshot(&self, data_type: &DistroDataType) -> Vec<DistroData> {
        if let Some(handler) = self.handlers.get(data_type) {
            handler.get_snapshot().await
        } else {
            vec![]
        }
    }

    /// Get pending sync task count
    pub fn pending_sync_count(&self) -> usize {
        self.sync_tasks.len()
    }
}

/// Default implementation for naming instance data
pub struct NamingInstanceDistroHandler {
    #[allow(dead_code)]
    local_address: String,
    // In a real implementation, this would hold a reference to NamingService
}

impl NamingInstanceDistroHandler {
    pub fn new(local_address: String) -> Self {
        Self { local_address }
    }
}

#[tonic::async_trait]
impl DistroDataHandler for NamingInstanceDistroHandler {
    fn data_type(&self) -> DistroDataType {
        DistroDataType::NamingInstance
    }

    async fn get_all_keys(&self) -> Vec<String> {
        // Would return all service instance keys
        vec![]
    }

    async fn get_data(&self, _key: &str) -> Option<DistroData> {
        // Would return the specific instance data
        None
    }

    async fn process_sync_data(&self, data: DistroData) -> Result<(), String> {
        // Would update local instance data
        debug!("Processing naming instance sync data: key={}", data.key);
        Ok(())
    }

    async fn process_verify_data(&self, _data: &DistroData) -> Result<bool, String> {
        // Would verify data integrity
        Ok(true)
    }

    async fn get_snapshot(&self) -> Vec<DistroData> {
        // Would return all instance data
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distro_data_type_display() {
        assert_eq!(
            DistroDataType::NamingInstance.to_string(),
            "naming_instance"
        );
        assert_eq!(
            DistroDataType::Custom("test".to_string()).to_string(),
            "custom_test"
        );
    }

    #[test]
    fn test_distro_data_creation() {
        let data = DistroData::new(
            DistroDataType::NamingInstance,
            "service-key".to_string(),
            vec![1, 2, 3],
            "192.168.1.1:8848".to_string(),
        );

        assert_eq!(data.key, "service-key");
        assert_eq!(data.source, "192.168.1.1:8848");
        assert!(data.version > 0);
    }
}
