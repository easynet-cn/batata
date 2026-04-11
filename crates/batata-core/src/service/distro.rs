//! Distro protocol implementation for ephemeral data synchronization
//!
//! The Distro protocol is used to sync ephemeral data (like service instances) across cluster nodes.
//! This is the AP (Availability, Partition tolerance) mode in Nacos, used for ephemeral instances.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use batata_api::{
    distro::{
        DistroDataItem, DistroDataSnapshotRequest, DistroDataSnapshotResponse,
        DistroDataSyncRequest, DistroDataSyncResponse, DistroDataVerifyRequest,
        DistroDataVerifyResponse,
    },
    model::Member,
    remote::model::ResponseTrait,
};

use super::cluster_client::ClusterClientManager;
use super::datacenter::DatacenterManager;

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
    /// Max retry attempts for initial data loading from peers
    pub load_max_retries: u32,
    /// Whether to require successful initial data load before marking node ready.
    /// false (default, Nacos-compatible): start immediately, verify cycle fills data.
    /// true: node stays NOT_READY until initial sync completes.
    pub require_initial_load: bool,
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
            load_max_retries: 5,
            require_initial_load: false,
        }
    }
}

impl DistroConfig {
    /// Create from application Configuration
    pub fn from_configuration(config: &crate::model::Configuration) -> Self {
        Self {
            sync_delay: Duration::from_millis(config.distro_sync_delay_ms()),
            sync_timeout: Duration::from_millis(config.distro_sync_timeout_ms()),
            sync_retry_delay: Duration::from_millis(config.distro_sync_retry_delay_ms()),
            verify_interval: Duration::from_millis(config.distro_verify_interval_ms()),
            verify_timeout: Duration::from_millis(config.distro_verify_timeout_ms()),
            load_retry_delay: Duration::from_millis(config.distro_load_retry_delay_ms()),
            load_max_retries: config.distro_load_max_retries(),
            require_initial_load: config.distro_require_initial_load(),
        }
    }
}

/// Determines which cluster node is responsible for a given data key
///
/// Uses hash-based partitioning over a sorted list of healthy member addresses.
/// This provides consistent data ownership across the cluster.
pub struct DistroMapper {
    /// Current healthy member addresses (sorted for consistent hashing)
    healthy_members: Arc<std::sync::RwLock<Vec<String>>>,
}

impl DistroMapper {
    pub fn new() -> Self {
        Self {
            healthy_members: Arc::new(std::sync::RwLock::new(Vec::new())),
        }
    }

    /// Update the list of healthy members
    pub fn update_members(&self, members: Vec<String>) {
        let mut sorted = members;
        sorted.sort();
        *self
            .healthy_members
            .write()
            .unwrap_or_else(|e| e.into_inner()) = sorted;
    }

    /// Determine which member is responsible for a given key
    pub fn responsible_node(&self, key: &str) -> Option<String> {
        let members = self
            .healthy_members
            .read()
            .unwrap_or_else(|e| e.into_inner());
        if members.is_empty() {
            return None;
        }
        let hash = self.hash_key(key);
        let index = (hash as usize) % members.len();
        Some(members[index].clone())
    }

    /// Check if the local node is responsible for a key
    pub fn is_responsible(&self, key: &str, local_address: &str) -> bool {
        self.responsible_node(key)
            .map(|node| node == local_address)
            .unwrap_or(true) // If no members, always responsible (standalone)
    }

    /// Get the current list of healthy members
    pub fn get_members(&self) -> Vec<String> {
        self.healthy_members
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn hash_key(&self, key: &str) -> u32 {
        let mut hash: u32 = 0;
        for byte in key.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
        }
        hash
    }
}

impl Default for DistroMapper {
    fn default() -> Self {
        Self::new()
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

    /// Remove a key from local storage.
    ///
    /// Called by `DistroProtocol::cleanup_non_responsible_keys` when the
    /// local node is no longer responsible for the key (cluster membership
    /// changed, hash partitioning reassigned ownership to a different peer).
    /// Must be idempotent — calling remove on a non-existent key is fine.
    ///
    /// Default implementation does nothing, to avoid breaking existing
    /// handlers that don't distinguish owned vs. non-owned data.
    async fn remove_data(&self, _key: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Snapshot of distro protocol health metrics for monitoring
#[derive(Debug, Clone, serde::Serialize)]
pub struct DistroMetrics {
    pub sync_success_total: u64,
    pub sync_failure_total: u64,
    pub verify_success_total: u64,
    pub verify_failure_total: u64,
    pub pending_sync_tasks: usize,
    pub initialized: bool,
    pub member_count: usize,
}

/// Atomic counters for distro protocol operations
struct DistroCounters {
    sync_success: std::sync::atomic::AtomicU64,
    sync_failure: std::sync::atomic::AtomicU64,
    verify_success: std::sync::atomic::AtomicU64,
    verify_failure: std::sync::atomic::AtomicU64,
}

impl DistroCounters {
    fn new() -> Self {
        Self {
            sync_success: std::sync::atomic::AtomicU64::new(0),
            sync_failure: std::sync::atomic::AtomicU64::new(0),
            verify_success: std::sync::atomic::AtomicU64::new(0),
            verify_failure: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

/// Distro protocol manager
///
/// # Lock Ordering
///
/// To prevent deadlocks, locks MUST be acquired in the following order:
///
/// 1. `running` (RwLock) — controls lifecycle, acquired first
/// 2. `members` (DashMap) — cluster membership, read frequently
/// 3. `handlers` (DashMap) — handler registry, iterated during verify/sync
/// 4. `sync_tasks` (DashMap) — pending sync tasks, written by verify loop
/// 5. `mapper.healthy_members` (std::sync::RwLock) — updated on membership change
///
/// Within the verify loop, `handlers` is iterated first to collect entries,
/// then `members` is read for peer addresses. Never hold a `handlers` reference
/// while calling into `client_manager` (which may acquire its own connection locks).
pub struct DistroProtocol {
    config: DistroConfig,
    local_address: String,
    handlers: Arc<DashMap<DistroDataType, Arc<dyn DistroDataHandler>>>,
    sync_tasks: Arc<DashMap<String, DistroSyncTask>>,
    running: Arc<RwLock<bool>>,
    client_manager: Arc<ClusterClientManager>,
    members: Arc<DashMap<String, Member>>,
    /// Optional datacenter manager for locality-aware sync
    datacenter_manager: Option<Arc<DatacenterManager>>,
    /// Hash-based data partitioning mapper
    mapper: Arc<DistroMapper>,
    /// Whether the protocol has completed initial data loading
    initialized: Arc<AtomicBool>,
    /// Operation counters for monitoring
    counters: Arc<DistroCounters>,
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
            datacenter_manager: None,
            mapper: Arc::new(DistroMapper::new()),
            initialized: Arc::new(AtomicBool::new(false)),
            counters: Arc::new(DistroCounters::new()),
        }
    }

    /// Create a new DistroProtocol with datacenter awareness
    pub fn with_datacenter_manager(
        local_address: String,
        config: DistroConfig,
        client_manager: Arc<ClusterClientManager>,
        members: Arc<DashMap<String, Member>>,
        datacenter_manager: Arc<DatacenterManager>,
    ) -> Self {
        Self {
            config,
            local_address,
            handlers: Arc::new(DashMap::new()),
            sync_tasks: Arc::new(DashMap::new()),
            running: Arc::new(RwLock::new(false)),
            client_manager,
            members,
            datacenter_manager: Some(datacenter_manager),
            mapper: Arc::new(DistroMapper::new()),
            initialized: Arc::new(AtomicBool::new(false)),
            counters: Arc::new(DistroCounters::new()),
        }
    }

    /// Set the datacenter manager
    pub fn set_datacenter_manager(&mut self, manager: Arc<DatacenterManager>) {
        self.datacenter_manager = Some(manager);
    }

    /// Get the data partitioning mapper
    pub fn mapper(&self) -> &Arc<DistroMapper> {
        &self.mapper
    }

    /// Get the local node address
    pub fn local_address(&self) -> &str {
        &self.local_address
    }

    /// Check whether the protocol has completed initial data loading
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Relaxed)
    }

    /// Get a snapshot of distro protocol metrics for monitoring
    pub fn metrics(&self) -> DistroMetrics {
        DistroMetrics {
            sync_success_total: self.counters.sync_success.load(Ordering::Relaxed),
            sync_failure_total: self.counters.sync_failure.load(Ordering::Relaxed),
            verify_success_total: self.counters.verify_success.load(Ordering::Relaxed),
            verify_failure_total: self.counters.verify_failure.load(Ordering::Relaxed),
            pending_sync_tasks: self.sync_tasks.len(),
            initialized: self.initialized.load(Ordering::Relaxed),
            member_count: self.members.len(),
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

        // Update the mapper with current healthy members
        self.refresh_mapper();

        // Load initial data from peers (for new nodes joining the cluster)
        let load_success = self.load_initial_data().await;

        // Mark initialization based on load result and configuration.
        // Nacos-compatible (require_initial_load=false): always mark initialized,
        // verify cycle will fill missing data within seconds.
        // Strict mode (require_initial_load=true): only mark initialized on success.
        if load_success || !self.config.require_initial_load {
            self.initialized.store(true, Ordering::Relaxed);
            if !load_success {
                warn!(
                    "Distro initial data not fully loaded, node starting with partial data. \
                     Verification cycle will reconcile within {}ms.",
                    self.config.verify_interval.as_millis()
                );
            }
            info!("Distro protocol initialized (data_loaded={})", load_success);
        } else {
            warn!(
                "Distro initial data load failed and require_initial_load=true. \
                 Node will NOT serve distro queries until data is loaded via verification."
            );
        }

        // Start sync task processor
        self.start_sync_task_processor().await;

        // Start data verification
        self.start_verify_task().await;
    }

    /// Refresh the mapper with current healthy members from the server list
    pub fn refresh_mapper(&self) {
        let healthy: Vec<String> = self
            .members
            .iter()
            .filter(|e| matches!(e.value().state, batata_api::model::NodeState::Up))
            .map(|e| e.key().clone())
            .collect();
        self.mapper.update_members(healthy);
    }

    /// Drop all locally-stored keys that the local node is no longer
    /// responsible for after the most recent mapper refresh.
    ///
    /// Called after `refresh_mapper` (cluster membership changed). Without
    /// this, nodes keep stale data for keys now owned by other nodes —
    /// causing duplicate writes and inconsistent query results. Nacos 3.x
    /// achieves the same outcome via `DistroVerifyTimedTask` filtering on
    /// the current responsible set.
    ///
    /// Returns the total number of keys removed across all registered
    /// handlers.
    pub async fn cleanup_non_responsible_keys(&self) -> usize {
        let mut total_removed = 0usize;
        let handler_entries: Vec<Arc<dyn DistroDataHandler>> = self
            .handlers
            .iter()
            .map(|e| e.value().clone())
            .collect();
        for handler in handler_entries {
            let keys = handler.get_all_keys().await;
            for key in keys {
                if !self.mapper.is_responsible(&key, &self.local_address) {
                    if let Err(e) = handler.remove_data(&key).await {
                        warn!("Failed to remove non-responsible key '{}': {}", key, e);
                    } else {
                        total_removed += 1;
                    }
                }
            }
        }
        if total_removed > 0 {
            info!(
                "Distro cleanup: removed {} keys no longer owned by this node",
                total_removed
            );
        }
        total_removed
    }

    /// Stop the distro protocol, draining pending sync tasks first.
    ///
    /// Waits up to 5 seconds for in-flight sync tasks to complete before
    /// forcibly stopping. This prevents data loss during rolling restarts.
    pub async fn stop(&self) {
        info!(
            "Stopping Distro protocol (draining {} pending sync tasks)...",
            self.sync_tasks.len()
        );

        // Give pending tasks a brief window to complete
        let drain_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !self.sync_tasks.is_empty() && tokio::time::Instant::now() < drain_deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if !self.sync_tasks.is_empty() {
            warn!(
                "Distro protocol stopping with {} undrained sync tasks",
                self.sync_tasks.len()
            );
        }

        let mut running = self.running.write().await;
        *running = false;
        info!("Stopped Distro protocol");
    }

    /// Schedule a sync task for data
    ///
    /// If a DatacenterManager is configured, uses locality-aware sync:
    /// - Local datacenter members are synced immediately (with normal sync delay)
    /// - Remote datacenter members are synced with cross-DC delay
    pub async fn sync_data(&self, data_type: DistroDataType, key: &str) {
        let now = chrono::Utc::now().timestamp_millis();

        if let Some(ref dc_manager) = self.datacenter_manager {
            // Datacenter-aware sync: local first, then cross-DC with delay
            let local_targets = dc_manager.select_replication_targets(
                Some(&self.local_address),
                dc_manager.replication_factor(),
            );

            for member in local_targets {
                let task_key = format!("{}:{}:{}", data_type, key, member.address);
                let task = DistroSyncTask {
                    data_type: data_type.clone(),
                    key: key.to_string(),
                    target_address: member.address.clone(),
                    scheduled_time: now + self.config.sync_delay.as_millis() as i64,
                    retry_count: 0,
                };
                self.sync_tasks.insert(task_key, task);
            }

            // Schedule cross-DC sync with additional delay
            if dc_manager.is_cross_dc_replication_enabled() {
                let cross_dc_targets =
                    dc_manager.select_cross_dc_replication_targets(Some(&self.local_address));
                let cross_dc_delay = dc_manager.cross_dc_sync_delay_secs() * 1000;

                for member in cross_dc_targets {
                    let task_key = format!("{}:{}:{}", data_type, key, member.address);
                    let task = DistroSyncTask {
                        data_type: data_type.clone(),
                        key: key.to_string(),
                        target_address: member.address.clone(),
                        scheduled_time: now
                            + self.config.sync_delay.as_millis() as i64
                            + cross_dc_delay as i64,
                        retry_count: 0,
                    };
                    self.sync_tasks.insert(task_key, task);
                }
            }

            debug!(
                "Scheduled datacenter-aware sync for {}:{} (local: {}, cross-dc: {})",
                data_type,
                key,
                dc_manager.replication_factor(),
                dc_manager.is_cross_dc_replication_enabled()
            );
        } else {
            // Legacy mode: sync to all members
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
                    scheduled_time: now + self.config.sync_delay.as_millis() as i64,
                    retry_count: 0,
                };
                self.sync_tasks.insert(task_key, task);
            }

            debug!("Scheduled sync for {}:{}", data_type, key);
        }
    }

    /// Start the sync task processor
    async fn start_sync_task_processor(&self) {
        let sync_tasks = self.sync_tasks.clone();
        let handlers = self.handlers.clone();
        let running = self.running.clone();
        let client_manager = self.client_manager.clone();
        let config = self.config.clone();
        let _local_address = self.local_address.clone();
        let counters = self.counters.clone();

        tokio::spawn(async move {
            loop {
                {
                    let is_running = running.read().await;
                    if !*is_running {
                        break;
                    }
                }

                let now = chrono::Utc::now().timestamp_millis();

                // Atomically claim ready tasks: collect keys first, then remove.
                // Only clone keys (not values) during iteration to reduce overhead.
                let ready_keys: Vec<String> = sync_tasks
                    .iter()
                    .filter(|e| e.value().scheduled_time <= now)
                    .map(|e| e.key().clone())
                    .collect();
                let ready_tasks: Vec<(String, DistroSyncTask)> = ready_keys
                    .into_iter()
                    .filter_map(|key| sync_tasks.remove(&key))
                    .collect();

                for (task_key, task) in ready_tasks {
                    // Get data from handler and send sync request
                    if let Some(handler) = handlers.get(&task.data_type)
                        && let Some(data) = handler.get_data(&task.key).await
                    {
                        // Send sync request
                        let result = Self::send_sync_data(
                            &client_manager,
                            &task.target_address,
                            data.clone(),
                        )
                        .await;

                        if result.is_ok() {
                            counters.sync_success.fetch_add(1, Ordering::Relaxed);
                            debug!(
                                "Sync completed for {}:{} to {}",
                                task.data_type, task.key, task.target_address
                            );
                        } else {
                            counters.sync_failure.fetch_add(1, Ordering::Relaxed);
                            // Nacos-compatible retry: unlimited attempts with
                            // capped exponential backoff. The task stays in
                            // the queue until it succeeds OR the target is
                            // removed from cluster membership (handled by the
                            // verify loop's cleanup_non_responsible_keys /
                            // member refresh). Dropping after N attempts
                            // risks losing registration under a long network
                            // partition.
                            //
                            // Backoff schedule (cap at 2^6 = 64x base):
                            //   attempt 1: base * 2
                            //   attempt 2: base * 4
                            //   ...
                            //   attempt 6+: base * 64 (ceiling)
                            const BACKOFF_CEILING_SHIFT: u32 = 6;
                            let mut updated_task = task.clone();
                            updated_task.retry_count =
                                updated_task.retry_count.saturating_add(1);
                            let base_delay = config.sync_retry_delay.as_millis() as i64;
                            let shift = updated_task.retry_count.min(BACKOFF_CEILING_SHIFT);
                            let backoff = base_delay.saturating_mul(1i64 << shift);
                            // ~25% jitter to prevent thundering herd
                            let jitter = (now % (backoff / 4 + 1)).max(0);
                            updated_task.scheduled_time = now + backoff + jitter;
                            sync_tasks.insert(task_key, updated_task);
                            // Log level rises with retry count so chronic
                            // failures surface without spamming on transient ones.
                            let rc = task.retry_count + 1;
                            if rc <= 3 {
                                debug!(
                                    "Sync retry {} for {}:{} to {} (backoff={}ms)",
                                    rc,
                                    task.data_type,
                                    task.key,
                                    task.target_address,
                                    backoff
                                );
                            } else if rc <= 10 {
                                warn!(
                                    "Sync retry {} for {}:{} to {} (backoff={}ms)",
                                    rc,
                                    task.data_type,
                                    task.key,
                                    task.target_address,
                                    backoff
                                );
                            } else if rc.is_multiple_of(10) {
                                // Every 10th retry above 10, surface an error
                                error!(
                                    "Sync still failing after {} retries for {}:{} to {} — \
                                     check network connectivity to peer",
                                    rc, task.data_type, task.key, task.target_address
                                );
                            }
                        }
                    }
                    // Data no longer exists — task was already removed, nothing to do
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
    }

    /// Start the verification task - periodically verifies data consistency with peers.
    ///
    /// Sends verify requests to ALL peers in parallel using `tokio::spawn` per peer,
    /// then collects results and schedules sync tasks for any keys the peers are missing.
    async fn start_verify_task(&self) {
        let running = self.running.clone();
        let members = self.members.clone();
        let local_address = self.local_address.clone();
        let config = self.config.clone();
        let handlers = self.handlers.clone();
        let client_manager = self.client_manager.clone();
        let sync_tasks = self.sync_tasks.clone();
        let counters = self.counters.clone();
        let mapper = self.mapper.clone();

        tokio::spawn(async move {
            loop {
                {
                    let is_running = running.read().await;
                    if !*is_running {
                        break;
                    }
                }

                // Add jitter to prevent thundering herd when multiple nodes
                // start their verify loops at the same time
                let jitter_ms = {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    std::time::SystemTime::now().hash(&mut hasher);
                    hasher.finish() % 2000
                };
                tokio::time::sleep(config.verify_interval + Duration::from_millis(jitter_ms)).await;

                // Refresh responsibility mapping against current membership
                // before running verify. Without this, stale membership can
                // cause sync/verify targeting a down node, or keeping keys
                // owned by a new node. This replaces the need for an
                // external member-change event subscription.
                {
                    let healthy: Vec<String> = members
                        .iter()
                        .filter(|e| {
                            matches!(e.value().state, batata_api::model::NodeState::Up)
                        })
                        .map(|e| e.key().clone())
                        .collect();
                    mapper.update_members(healthy);
                }

                // Drop locally-stored keys this node is no longer responsible for.
                // Iterating handlers separately keeps the main verify loop below
                // unchanged and avoids holding extra locks during network I/O.
                let mut cleanup_removed = 0usize;
                let cleanup_entries: Vec<Arc<dyn DistroDataHandler>> = handlers
                    .iter()
                    .map(|e| e.value().clone())
                    .collect();
                for handler in cleanup_entries {
                    let keys = handler.get_all_keys().await;
                    for key in keys {
                        if !mapper.is_responsible(&key, &local_address) {
                            if handler.remove_data(&key).await.is_ok() {
                                cleanup_removed += 1;
                            }
                        }
                    }
                }
                if cleanup_removed > 0 {
                    info!(
                        "Distro verify-cycle cleanup: removed {} non-responsible keys",
                        cleanup_removed
                    );
                }

                // Get all members except self
                let other_members: Vec<String> = members
                    .iter()
                    .filter(|e| e.key() != &local_address)
                    .map(|e| e.key().clone())
                    .collect();

                if other_members.is_empty() {
                    continue;
                }

                // Collect handler data types and handlers before iteration
                let handler_entries: Vec<(DistroDataType, Arc<dyn DistroDataHandler>)> = handlers
                    .iter()
                    .map(|e| (e.key().clone(), e.value().clone()))
                    .collect();

                // For each registered handler, collect local key-version data
                for (data_type, handler) in &handler_entries {
                    let keys = handler.get_all_keys().await;
                    if keys.is_empty() {
                        continue;
                    }

                    // Build verify_data: key -> version
                    let mut verify_data = std::collections::HashMap::new();
                    for key in &keys {
                        if let Some(data) = handler.get_data(key).await {
                            verify_data.insert(key.clone(), data.version);
                        }
                    }

                    if verify_data.is_empty() {
                        continue;
                    }

                    // Send verify requests to ALL peers in parallel
                    let mut handles = Vec::with_capacity(other_members.len());

                    for member_address in &other_members {
                        let (api_data_type, custom_type_name) = match data_type {
                            DistroDataType::NamingInstance => {
                                (batata_api::distro::DistroDataType::NamingInstance, None)
                            }
                            DistroDataType::Custom(name) => (
                                batata_api::distro::DistroDataType::Custom,
                                Some(name.clone()),
                            ),
                        };

                        let mut request = DistroDataVerifyRequest::new();
                        request.data_type = api_data_type;
                        request.custom_type_name = custom_type_name;
                        request.verify_data = verify_data.clone();

                        let cm = client_manager.clone();
                        let addr = member_address.clone();
                        let dt = data_type.clone();

                        // Each peer verification runs concurrently
                        handles.push(tokio::spawn(async move {
                            match cm.send_request(&addr, request).await {
                                Ok(response) => {
                                    if let Some(body) = response.body
                                        && let Ok(verify_response) =
                                            serde_json::from_slice::<DistroDataVerifyResponse>(
                                                &body.value,
                                            )
                                    {
                                        if !verify_response.keys_need_sync.is_empty() {
                                            debug!(
                                                "Verify: {} needs {} keys synced for type {}",
                                                addr,
                                                verify_response.keys_need_sync.len(),
                                                dt
                                            );
                                        }
                                        return Some((addr, dt, verify_response.keys_need_sync));
                                    }
                                    None
                                }
                                Err(e) => {
                                    warn!("Failed to verify distro data with {}: {}", addr, e);
                                    None
                                }
                            }
                        }));
                    }

                    // Collect results from all parallel verify requests
                    let now = chrono::Utc::now().timestamp_millis();
                    for handle in handles {
                        match handle.await {
                            Ok(Some((addr, dt, keys_need_sync))) => {
                                counters.verify_success.fetch_add(1, Ordering::Relaxed);
                                for key in keys_need_sync {
                                    let task_key = format!("{}:{}:{}", dt, key, addr);
                                    let task = DistroSyncTask {
                                        data_type: dt.clone(),
                                        key,
                                        target_address: addr.clone(),
                                        scheduled_time: now + config.sync_delay.as_millis() as i64,
                                        retry_count: 0,
                                    };
                                    sync_tasks.insert(task_key, task);
                                }
                            }
                            _ => {
                                counters.verify_failure.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
        });
    }

    /// Load initial data from peers when this node starts (snapshot load).
    ///
    /// For each registered data type, tries every peer in order. If no peer
    /// succeeds, waits `load_retry_delay` and retries the full round up to
    /// `load_max_retries` times. Returns true if all data types loaded
    /// successfully, false if any failed.
    async fn load_initial_data(&self) -> bool {
        // Skip if no peers (standalone mode)
        let other_members: Vec<String> = self
            .members
            .iter()
            .filter(|e| e.key() != &self.local_address)
            .map(|e| e.key().clone())
            .collect();

        if other_members.is_empty() {
            debug!("No peers available, skipping initial data load");
            return true; // No peers = standalone, consider loaded
        }

        info!("Loading initial data from {} peers", other_members.len());
        let mut all_loaded = true;

        // For each registered handler, request snapshot from a peer
        let handler_entries: Vec<(DistroDataType, Arc<dyn DistroDataHandler>)> = self
            .handlers
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect();

        for (data_type, handler) in &handler_entries {
            let api_data_type = match data_type {
                DistroDataType::NamingInstance => {
                    batata_api::distro::DistroDataType::NamingInstance
                }
                DistroDataType::Custom(name) => {
                    // For custom types we need to use for_custom_type constructor
                    // but store the name for logging. The api_data_type is Custom.
                    let _ = name;
                    batata_api::distro::DistroDataType::Custom
                }
            };

            let mut loaded = false;

            for attempt in 0..self.config.load_max_retries {
                // Refresh member list on retry — new nodes may have joined
                let peers: Vec<String> = if attempt == 0 {
                    other_members.clone()
                } else {
                    self.members
                        .iter()
                        .filter(|e| e.key() != &self.local_address)
                        .map(|e| e.key().clone())
                        .collect()
                };

                if peers.is_empty() {
                    break;
                }

                // Try each peer until one succeeds
                for member_address in &peers {
                    let request = DistroDataSnapshotRequest::new(api_data_type.clone());

                    match self
                        .client_manager
                        .send_request(member_address, request)
                        .await
                    {
                        Ok(response) => {
                            if let Some(body) = response.body
                                && let Ok(snapshot_response) =
                                    serde_json::from_slice::<DistroDataSnapshotResponse>(
                                        &body.value,
                                    )
                            {
                                let item_count = snapshot_response.snapshot.len();
                                for item in snapshot_response.snapshot {
                                    let internal_data_type = match item.data_type {
                                        batata_api::distro::DistroDataType::NamingInstance => {
                                            DistroDataType::NamingInstance
                                        }
                                        batata_api::distro::DistroDataType::Custom => {
                                            DistroDataType::Custom(
                                                item.custom_type_name.clone().unwrap_or_default(),
                                            )
                                        }
                                    };

                                    let data = DistroData::new(
                                        internal_data_type,
                                        item.key,
                                        item.content.into_bytes(),
                                        item.source,
                                    );

                                    if let Err(e) = handler.process_sync_data(data).await {
                                        warn!("Failed to process snapshot data: {}", e);
                                    }
                                }

                                info!(
                                    "Loaded {} items for type {} from {}",
                                    item_count, data_type, member_address
                                );
                                loaded = true;
                                break;
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Failed to load snapshot from {}: {}, trying next peer",
                                member_address, e
                            );
                        }
                    }
                }

                if loaded {
                    break;
                }

                // All peers failed — wait and retry
                error!(
                    "Failed to load initial data for type {} from any peer (attempt {}/{}), \
                     retrying in {:?}",
                    data_type,
                    attempt + 1,
                    self.config.load_max_retries,
                    self.config.load_retry_delay,
                );
                tokio::time::sleep(self.config.load_retry_delay).await;
            }

            if !loaded {
                error!(
                    "CRITICAL: Could not load initial data for type {} after {} attempts. \
                     Node may serve incomplete data until verification cycle reconciles.",
                    data_type, self.config.load_max_retries,
                );
                all_loaded = false;
            }
        }
        all_loaded
    }

    /// Send sync data to a target node via gRPC
    async fn send_sync_data(
        client_manager: &Arc<ClusterClientManager>,
        target: &str,
        data: DistroData,
    ) -> Result<(), String> {
        debug!(
            "Sending distro sync data to {}: type={}, key={}",
            target, data.data_type, data.key
        );

        // Convert internal DistroData to API DistroDataItem
        let (api_data_type, custom_type_name) = match &data.data_type {
            DistroDataType::NamingInstance => {
                (batata_api::distro::DistroDataType::NamingInstance, None)
            }
            DistroDataType::Custom(name) => (
                batata_api::distro::DistroDataType::Custom,
                Some(name.clone()),
            ),
        };

        let data_item = DistroDataItem {
            data_type: api_data_type,
            custom_type_name,
            key: data.key.clone(),
            content: String::from_utf8(data.content.clone()).unwrap_or_default(),
            version: data.version,
            source: data.source.clone(),
        };

        // Create sync request
        let request = DistroDataSyncRequest::with_data(data_item);

        // Send via cluster client manager
        match client_manager.send_request(target, request).await {
            Ok(response) => {
                // Parse response to check if it's successful
                if let Some(body) = response.body
                    && let Ok(sync_response) =
                        serde_json::from_slice::<DistroDataSyncResponse>(&body.value)
                {
                    if sync_response.result_code() == 200 {
                        debug!("Distro sync successful for key={} to {}", data.key, target);
                        return Ok(());
                    } else {
                        return Err(format!(
                            "Distro sync failed: {}",
                            sync_response.response.message
                        ));
                    }
                }
                // If we can't parse the response, assume success
                Ok(())
            }
            Err(e) => Err(format!("Failed to send distro sync: {}", e)),
        }
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

    #[test]
    fn test_distro_config_defaults() {
        let config = DistroConfig::default();
        assert_eq!(config.sync_delay, Duration::from_millis(1000));
        assert_eq!(config.sync_timeout, Duration::from_millis(3000));
        assert_eq!(config.sync_retry_delay, Duration::from_millis(3000));
        assert_eq!(config.verify_interval, Duration::from_millis(5000));
        assert_eq!(config.verify_timeout, Duration::from_millis(3000));
        assert_eq!(config.load_retry_delay, Duration::from_millis(30000));
    }

    #[test]
    fn test_distro_data_type_equality() {
        assert_eq!(
            DistroDataType::NamingInstance,
            DistroDataType::NamingInstance
        );
        assert_eq!(
            DistroDataType::Custom("a".to_string()),
            DistroDataType::Custom("a".to_string())
        );
        assert_ne!(
            DistroDataType::NamingInstance,
            DistroDataType::Custom("naming_instance".to_string())
        );
        assert_ne!(
            DistroDataType::Custom("a".to_string()),
            DistroDataType::Custom("b".to_string())
        );
    }

    #[test]
    fn test_distro_data_serialization() {
        let data = DistroData {
            data_type: DistroDataType::NamingInstance,
            key: "test-key".to_string(),
            content: b"hello".to_vec(),
            version: 12345,
            source: "10.0.0.1:8848".to_string(),
        };

        let json = serde_json::to_string(&data).unwrap();
        let deserialized: DistroData = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.data_type, DistroDataType::NamingInstance);
        assert_eq!(deserialized.key, "test-key");
        assert_eq!(deserialized.content, b"hello");
        assert_eq!(deserialized.version, 12345);
        assert_eq!(deserialized.source, "10.0.0.1:8848");
    }

    #[test]
    fn test_distro_data_type_serialization() {
        let naming = DistroDataType::NamingInstance;
        let json = serde_json::to_string(&naming).unwrap();
        let deserialized: DistroDataType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, DistroDataType::NamingInstance);

        let custom = DistroDataType::Custom("mytype".to_string());
        let json = serde_json::to_string(&custom).unwrap();
        let deserialized: DistroDataType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, DistroDataType::Custom("mytype".to_string()));
    }

    #[test]
    fn test_distro_sync_task_creation() {
        let task = DistroSyncTask {
            data_type: DistroDataType::NamingInstance,
            key: "service-a".to_string(),
            target_address: "192.168.1.2:8848".to_string(),
            scheduled_time: 1000000,
            retry_count: 0,
        };

        assert_eq!(task.data_type, DistroDataType::NamingInstance);
        assert_eq!(task.key, "service-a");
        assert_eq!(task.target_address, "192.168.1.2:8848");
        assert_eq!(task.scheduled_time, 1000000);
        assert_eq!(task.retry_count, 0);
    }

    #[test]
    fn test_distro_data_type_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(DistroDataType::NamingInstance);
        set.insert(DistroDataType::Custom("a".to_string()));
        set.insert(DistroDataType::Custom("b".to_string()));
        // Duplicate should not increase size
        set.insert(DistroDataType::NamingInstance);

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_distro_mapper_empty() {
        let mapper = DistroMapper::new();
        assert!(mapper.responsible_node("any-key").is_none());
        // With no members, is_responsible defaults to true (standalone)
        assert!(mapper.is_responsible("any-key", "localhost:8848"));
    }

    #[test]
    fn test_distro_mapper_single_member() {
        let mapper = DistroMapper::new();
        mapper.update_members(vec!["10.0.0.1:8848".to_string()]);

        // With only one member, all keys map to that member
        assert_eq!(
            mapper.responsible_node("key-a"),
            Some("10.0.0.1:8848".to_string())
        );
        assert_eq!(
            mapper.responsible_node("key-b"),
            Some("10.0.0.1:8848".to_string())
        );
        assert!(mapper.is_responsible("key-a", "10.0.0.1:8848"));
        assert!(!mapper.is_responsible("key-a", "10.0.0.2:8848"));
    }

    #[test]
    fn test_distro_mapper_multiple_members() {
        let mapper = DistroMapper::new();
        let members = vec![
            "10.0.0.1:8848".to_string(),
            "10.0.0.2:8848".to_string(),
            "10.0.0.3:8848".to_string(),
        ];
        mapper.update_members(members.clone());

        // All keys should map to one of the members
        for i in 0..100 {
            let key = format!("service-{}", i);
            let node = mapper.responsible_node(&key).unwrap();
            assert!(members.contains(&node));
        }
    }

    #[test]
    fn test_distro_mapper_consistent_hashing() {
        let mapper = DistroMapper::new();
        mapper.update_members(vec![
            "10.0.0.1:8848".to_string(),
            "10.0.0.2:8848".to_string(),
            "10.0.0.3:8848".to_string(),
        ]);

        // Same key should always map to the same node
        let node1 = mapper.responsible_node("test-key").unwrap();
        let node2 = mapper.responsible_node("test-key").unwrap();
        assert_eq!(node1, node2);
    }

    #[test]
    fn test_distro_mapper_sorted_members() {
        let mapper = DistroMapper::new();
        // Insert in unsorted order
        mapper.update_members(vec![
            "10.0.0.3:8848".to_string(),
            "10.0.0.1:8848".to_string(),
            "10.0.0.2:8848".to_string(),
        ]);

        let members = mapper.get_members();
        assert_eq!(members[0], "10.0.0.1:8848");
        assert_eq!(members[1], "10.0.0.2:8848");
        assert_eq!(members[2], "10.0.0.3:8848");
    }

    #[test]
    fn test_distro_mapper_update_members() {
        let mapper = DistroMapper::new();
        mapper.update_members(vec![
            "10.0.0.1:8848".to_string(),
            "10.0.0.2:8848".to_string(),
        ]);
        assert_eq!(mapper.get_members().len(), 2);

        // Update with new member list
        mapper.update_members(vec![
            "10.0.0.1:8848".to_string(),
            "10.0.0.2:8848".to_string(),
            "10.0.0.3:8848".to_string(),
        ]);
        assert_eq!(mapper.get_members().len(), 3);
    }

    #[test]
    fn test_distro_mapper_distribution() {
        let mapper = DistroMapper::new();
        mapper.update_members(vec![
            "10.0.0.1:8848".to_string(),
            "10.0.0.2:8848".to_string(),
            "10.0.0.3:8848".to_string(),
        ]);

        // Check that keys are distributed across members (not all to one)
        let mut counts = std::collections::HashMap::new();
        for i in 0..300 {
            let key = format!("key-{}", i);
            let node = mapper.responsible_node(&key).unwrap();
            *counts.entry(node).or_insert(0u32) += 1;
        }

        // Each member should have at least some keys (not perfectly even, but distributed)
        assert_eq!(counts.len(), 3, "All 3 members should have keys assigned");
        for count in counts.values() {
            assert!(*count > 0, "Each member should have at least one key");
        }
    }

    #[test]
    fn test_distro_counters() {
        let counters = DistroCounters::new();
        assert_eq!(counters.sync_success.load(Ordering::Relaxed), 0);
        assert_eq!(counters.sync_failure.load(Ordering::Relaxed), 0);
        assert_eq!(counters.verify_success.load(Ordering::Relaxed), 0);
        assert_eq!(counters.verify_failure.load(Ordering::Relaxed), 0);

        counters.sync_success.fetch_add(3, Ordering::Relaxed);
        counters.sync_failure.fetch_add(1, Ordering::Relaxed);
        counters.verify_success.fetch_add(10, Ordering::Relaxed);
        counters.verify_failure.fetch_add(2, Ordering::Relaxed);

        assert_eq!(counters.sync_success.load(Ordering::Relaxed), 3);
        assert_eq!(counters.sync_failure.load(Ordering::Relaxed), 1);
        assert_eq!(counters.verify_success.load(Ordering::Relaxed), 10);
        assert_eq!(counters.verify_failure.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_distro_metrics_serialization() {
        let metrics = DistroMetrics {
            sync_success_total: 100,
            sync_failure_total: 5,
            verify_success_total: 200,
            verify_failure_total: 3,
            pending_sync_tasks: 2,
            initialized: true,
            member_count: 3,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("\"sync_success_total\":100"));
        assert!(json.contains("\"initialized\":true"));
        assert!(json.contains("\"member_count\":3"));
    }
}
