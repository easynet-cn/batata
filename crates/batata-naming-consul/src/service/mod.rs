//! In-memory Consul naming service implementation
//!
//! DashMap-based concurrent service registry with Consul-native semantics:
//! - Agent model for local service registration
//! - Catalog view for cluster-wide service discovery
//! - Health checks with tri-state (passing/warning/critical)
//! - Blocking query support via monotonic index
//! - NO namespace/group/cluster (Nacos) concepts

mod agent;
mod catalog;
mod health;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;

use crate::model::{AgentService, HealthCheck};

/// Consul index provider — monotonic counter for blocking queries
#[derive(Debug)]
pub struct IndexProvider {
    current: AtomicU64,
}

impl IndexProvider {
    pub fn new() -> Self {
        Self {
            current: AtomicU64::new(1),
        }
    }

    /// Get current index
    pub fn current(&self) -> u64 {
        self.current.load(Ordering::Relaxed)
    }

    /// Increment and return new index
    pub fn next(&self) -> u64 {
        self.current.fetch_add(1, Ordering::Relaxed) + 1
    }
}

impl Default for IndexProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Consul datacenter configuration
#[derive(Debug, Clone)]
pub struct DatacenterConfig {
    /// Local datacenter name
    pub datacenter: String,
    /// Primary datacenter (for federation)
    pub primary_datacenter: String,
    /// Local node name
    pub node_name: String,
    /// Local node ID
    pub node_id: String,
}

impl Default for DatacenterConfig {
    fn default() -> Self {
        Self {
            datacenter: "dc1".to_string(),
            primary_datacenter: "dc1".to_string(),
            node_name: "batata-node".to_string(),
            node_id: uuid_v4(),
        }
    }
}

/// Internal service storage — native Consul types, no Nacos mapping
#[derive(Debug, Clone)]
struct StoredService {
    service: AgentService,
    checks: Vec<HealthCheck>,
    /// Raft index when registered
    create_index: u64,
    /// Raft index when last modified
    modify_index: u64,
}

/// In-memory Consul naming service
///
/// All Consul-native types stored directly — no JSON-stuffed metadata,
/// no forced namespace/group mapping.
#[derive(Clone)]
pub struct ConsulNamingServiceImpl {
    /// Key: service_id, Value: stored service with checks
    services: Arc<DashMap<String, StoredService>>,
    /// Key: check_id, Value: health check state
    checks: Arc<DashMap<String, HealthCheck>>,
    /// Monotonic index for blocking queries
    index: Arc<IndexProvider>,
    /// Datacenter configuration
    dc_config: DatacenterConfig,
    /// Notification handles for blocking queries
    notifiers: Arc<DashMap<String, Arc<tokio::sync::Notify>>>,
}

impl ConsulNamingServiceImpl {
    pub fn new(dc_config: DatacenterConfig) -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            checks: Arc::new(DashMap::new()),
            index: Arc::new(IndexProvider::new()),
            dc_config,
            notifiers: Arc::new(DashMap::new()),
        }
    }

    /// Get current Raft index
    pub fn current_index(&self) -> u64 {
        self.index.current()
    }

    /// Get datacenter config
    pub fn dc_config(&self) -> &DatacenterConfig {
        &self.dc_config
    }

    /// Get or create a notification handle for blocking queries
    /// (Used by blocking query implementation)
    #[allow(dead_code)]
    fn get_notify(&self, key: &str) -> Arc<tokio::sync::Notify> {
        self.notifiers
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Notify::new()))
            .clone()
    }

    /// Notify waiters that a service changed
    fn notify_service_change(&self, service_name: &str) {
        if let Some(notify) = self.notifiers.get(service_name) {
            notify.notify_waiters();
        }
        // Also notify the global "services" key for list_services watchers
        if let Some(notify) = self.notifiers.get("_services") {
            notify.notify_waiters();
        }
    }
}

impl Default for ConsulNamingServiceImpl {
    fn default() -> Self {
        Self::new(DatacenterConfig::default())
    }
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:032x}", t)
}
