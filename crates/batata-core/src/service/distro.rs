//! Distro protocol implementation for ephemeral data synchronization
//!
//! The Distro protocol is used to sync ephemeral data (like service instances) across cluster nodes.
//! This is the AP (Availability, Partition tolerance) mode in Batata (Nacos-compatible), used for ephemeral instances.

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
    model::Member,
    remote::model::{RequestTrait, ResponseTrait},
};
use batata_server_api::distro::{
    DistroDataBatchSyncRequest, DistroDataBatchSyncResponse, DistroDataItem,
    DistroDataSnapshotRequest, DistroDataSnapshotResponse, DistroDataSyncRequest,
    DistroDataSyncResponse, DistroDataVerifyRequest, DistroDataVerifyResponse,
};

/// Default maximum number of items packed into one `DistroDataBatchSyncRequest`.
///
/// Mirrors Nacos `DistroProtocol.syncToTarget` batching behavior. Tuned to
/// keep the gRPC payload under typical defaults (~4MB) for service-instance
/// payloads while still amortizing per-RPC overhead. Override via
/// `batata.cluster.distro.batch_sync_size` (or the Nacos-compatible alias
/// `nacos.core.protocol.distro.data.sync.batchSize`).
pub const DISTRO_BATCH_SIZE: usize = 50;

/// Maximum number of times a single sync task is retried before it is
/// dropped. Bounding retries keeps a permanently unreachable peer from
/// pinning a task (and its memory) in the queue forever; the verify cycle
/// re-detects any remaining divergence once the peer is reachable again.
pub const MAX_SYNC_RETRIES: u32 = 100;

/// Wire-level fallback name for a `Custom` distro data type whose
/// `custom_type_name` was omitted by an older peer.
///
/// All decode paths must use the SAME value: bucketing is keyed by the custom
/// type name, so "" on one side and "CUSTOM" on the other routes the same
/// payload into two different buckets.
pub const DEFAULT_CUSTOM_TYPE_NAME: &str = "CUSTOM";

/// Decode an optional `custom_type_name` from the wire into a bucket name.
/// Both senders and receivers must use this single helper.
pub fn custom_type_name(name: Option<String>) -> String {
    name.unwrap_or_else(|| DEFAULT_CUSTOM_TYPE_NAME.to_string())
}

/// Decode a distro payload into the `String` required by the wire format.
///
/// Built-in producers always use `serde_json::to_vec` (guaranteed UTF-8), but
/// `DistroData.content` is `Vec<u8>` and plugins may supply arbitrary bytes.
/// Substituting `""` for invalid UTF-8 would broadcast an empty config or
/// instance to every peer during the unconditional `sync_round` broadcast, so
/// refuse the item and let the caller surface the error.
pub fn distro_content_to_string(content: Vec<u8>, key: &str) -> Result<String, String> {
    String::from_utf8(content).map_err(|e| {
        error!(key = %key, "Refusing distro payload with non-UTF-8 content");
        format!("distro payload for key {key:?} is not valid UTF-8: {e}")
    })
}

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
    /// Maximum number of items per `DistroDataBatchSyncRequest`.
    ///
    /// Defaults to [`DISTRO_BATCH_SIZE`]. Larger batches reduce RPC count but
    /// increase per-message size and worst-case retry cost. Set to `1` to
    /// disable batching (each key sent in its own RPC).
    pub batch_sync_size: usize,
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
            batch_sync_size: DISTRO_BATCH_SIZE,
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
            batch_sync_size: config.distro_batch_sync_size(),
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
    /// Creates a new instance.
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
    /// Creates a new instance with a locally generated version.
    ///
    /// Only appropriate on the **producing** side. A node that receives data
    /// must keep the version that came off the wire — see `with_version`.
    pub fn new(data_type: DistroDataType, key: String, content: Vec<u8>, source: String) -> Self {
        Self {
            data_type,
            key,
            content,
            version: chrono::Utc::now().timestamp_millis(),
            source,
        }
    }

    /// Creates a new instance carrying `version` as produced by `source`.
    ///
    /// Used on the receiving side so the replica can later answer whether it
    /// is up to date with that source. Stamping the local clock here instead
    /// would fabricate a version nobody can compare against.
    pub fn with_version(
        data_type: DistroDataType,
        key: String,
        content: Vec<u8>,
        source: String,
        version: i64,
    ) -> Self {
        Self {
            data_type,
            key,
            content,
            version,
            source,
        }
    }
}

/// Sync task for delayed synchronization
#[derive(Clone, Debug)]
pub struct DistroSyncTask {
    /// The `data_type` field.
    pub data_type: DistroDataType,
    /// The `key` field.
    pub key: String,
    /// The `target_address` field.
    pub target_address: String,
    /// The `scheduled_time` field.
    pub scheduled_time: i64,
    /// The `retry_count` field.
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

    /// Return only the local version for `key`, as needed by a verify round.
    ///
    /// The default keeps every existing handler correct by delegating to
    /// [`DistroDataHandler::get_data`]. Handlers whose `get_data` builds a full
    /// sync payload should override this with a cheap lookup: a verify round
    /// runs for every key on every tick and only ever reads `DistroData::version`.
    async fn get_version(&self, key: &str) -> Option<i64> {
        self.get_data(key).await.map(|data| data.version)
    }

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
    /// The `sync_success_total` field.
    pub sync_success_total: u64,
    /// The `sync_failure_total` field.
    pub sync_failure_total: u64,
    /// The `verify_success_total` field.
    pub verify_success_total: u64,
    /// The `verify_failure_total` field.
    pub verify_failure_total: u64,
    /// The `pending_sync_tasks` field.
    pub pending_sync_tasks: usize,
    /// The `initialized` field.
    pub initialized: bool,
    /// The `member_count` field.
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

/// Members that take part in the responsibility hash ring.
///
/// The rule is "everything except `Down`", and it has to be the **only** rule
/// in the codebase. The mapper is shared: `DistroProtocol::mapper()` is handed
/// to the naming health check (`HealthCheckResponsibleInterceptor` decides
/// which node probes an instance from exactly this ring), so two different
/// filters would mean two different owners for the same key and probing would
/// jump between nodes whenever a member flickered.
///
/// Upstream builds the same list with `UP || SUSPICIOUS`
/// (`DistroMapper.onEvent`), for the same underlying reason: a single failed
/// health check flips a member to `Suspicious`, and shrinking the ring
/// redistributes responsibility, which makes healthy replicas look "not
/// responsible" for keys they still hold. batata additionally keeps
/// `Starting` / `Isolation` in the ring so a booting or isolated cluster still
/// distributes responsibility instead of every node falling back to
/// `is_responsible() == true` (the empty-ring answer) and probing everything.
fn ring_members(members: &DashMap<String, Member>) -> Vec<String> {
    members
        .iter()
        .filter(|e| !matches!(e.value().state, batata_api::model::NodeState::Down))
        .map(|e| e.key().clone())
        .collect()
}

/// Install the current member set as the responsibility ring on `mapper`.
///
/// Both refresh sites — the member-change listener (`refresh_mapper`) and the
/// verify loop — go through this so they cannot drift apart again. They did
/// once: the verify loop filtered on `Up` alone, so a member flickering to
/// `Suspicious` shrank the ring every ~5s and re-partitioned every key while
/// `refresh_mapper` still had the full ring.
fn install_ring(members: &DashMap<String, Member>, mapper: &DistroMapper) {
    mapper.update_members(ring_members(members));
}

/// A verify RPC whose JSON body was encoded once and shared across every
/// peer. `DistroDataVerifyRequest` owns a `HashMap<String, i64>` and
/// `ClusterClientManager::send_request` takes the request by value, so the
/// naive implementation cloned (and re-serialized) the whole key->version
/// map once per peer. All peers receive the identical payload, so we encode
/// it a single time and replay the bytes through this lightweight
/// `RequestTrait` adapter. The bytes and the `request_type` are exactly those
/// a normal `DistroDataVerifyRequest` would produce, so the wire format is
/// unchanged.
struct SharedVerifyRequest {
    request_type: &'static str,
    body: Arc<Vec<u8>>,
}

impl Serialize for SharedVerifyRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // `body()` is overridden, so serde never serializes this value; this
        // impl only exists to satisfy the `Serialize` bound on send_request.
        serializer.serialize_unit()
    }
}

impl RequestTrait for SharedVerifyRequest {
    fn headers(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }

    fn insert_headers(&mut self, _headers: std::collections::HashMap<String, String>) {}

    fn request_type(&self) -> &'static str {
        self.request_type
    }

    fn body(&self) -> Vec<u8>
    where
        Self: Serialize,
    {
        (*self.body).clone()
    }
}

/// Collect the local `key -> version` map for one verify round.
///
/// Uses [`DistroDataHandler::get_version`] rather than `get_data`: a handler's
/// `get_data` may clone and serialize every instance of a service, while a
/// verify round only needs the version counter. Keeping this in a small
/// function makes the hot-path wiring testable without starting the
/// background verify task.
async fn collect_verify_versions(
    handler: &Arc<dyn DistroDataHandler>,
    keys: &[String],
) -> std::collections::HashMap<String, i64> {
    let mut versions = std::collections::HashMap::with_capacity(keys.len());
    for key in keys {
        if let Some(version) = handler.get_version(key).await {
            versions.insert(key.clone(), version);
        }
    }
    versions
}

/// Remove pending sync tasks whose target is no longer a cluster member.
/// Called once per verify cycle so a departed peer cannot keep a task
/// retrying forever. A member that is still present but merely unreachable
/// is intentionally kept (retries are bounded by `MAX_SYNC_RETRIES`).
fn prune_stale_sync_targets(
    sync_tasks: &DashMap<String, DistroSyncTask>,
    members: &DashMap<String, Member>,
) {
    // Snapshot the member set first so we never hold a guard on one map
    // while locking the other (respects the documented lock ordering).
    let live: std::collections::HashSet<String> = members.iter().map(|e| e.key().clone()).collect();
    let stale: Vec<String> = sync_tasks
        .iter()
        .filter(|e| !live.contains(&e.value().target_address))
        .map(|e| e.key().clone())
        .collect();
    let pruned = stale.len();
    for key in stale {
        sync_tasks.remove(&key);
    }
    if pruned > 0 {
        debug!(
            "Pruned {} sync tasks targeting members no longer in the cluster",
            pruned
        );
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
    /// Handles of long-lived background tasks (member-change listener, sync
    /// task processor, verify loop). Aborted in `stop()` so they cannot
    /// outlive the protocol.
    task_handles: std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl DistroProtocol {
    /// Creates a new instance.
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
            task_handles: std::sync::Mutex::new(Vec::new()),
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
            task_handles: std::sync::Mutex::new(Vec::new()),
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

    /// Register a long-lived background task so `stop()` can abort it.
    fn track_task(&self, handle: tokio::task::JoinHandle<()>) {
        self.task_handles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(handle);
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

    /// Refresh the mapper with the current member list.
    ///
    /// Only `Down` members are excluded — see [`ring_members`] for why that
    /// rule must be the only one in the codebase.
    pub fn refresh_mapper(&self) {
        install_ring(&self.members, &self.mapper);
    }

    /// Members that this node considers to be in the responsibility ring.
    ///
    /// Exposed so callers that need to reason about responsibility (tests,
    /// admin surfaces) use the same list the mapper was built from.
    pub fn ring_members(&self) -> Vec<String> {
        ring_members(&self.members)
    }

    /// Subscribe to cluster member change events and immediately trigger
    /// `refresh_mapper` whenever the membership changes. The verify loop still
    /// runs its own periodic refresh as a safety net (so single missed events
    /// do not stall reconciliation), but event-driven refresh drops the
    /// typical convergence time from up to one verify_interval (~5s) down to
    /// milliseconds.
    ///
    /// Unlike the mapper refresh, this never deletes data. Nacos keeps a full
    /// replica set on every node, so a key that this node is no longer
    /// responsible for must still be served until the responsible node
    /// overwrites it.
    ///
    /// Safe to call from any context — spawns a background listener task.
    pub fn subscribe_member_changes(
        self: &Arc<Self>,
        publisher: Arc<super::member_event::MemberChangeEventPublisher>,
    ) {
        let mut rx = publisher.subscribe();
        let this = Arc::clone(self);
        self.track_task(tokio::spawn(async move {
            use super::member_event::MemberChangeType;
            loop {
                match rx.recv().await {
                    Ok(evt) => {
                        // Only act on changes that affect responsibility.
                        // Metadata updates don't change the healthy-member
                        // set so skip them to avoid unnecessary work.
                        if !matches!(
                            evt.change_type,
                            MemberChangeType::MemberJoin
                                | MemberChangeType::MemberLeave
                                | MemberChangeType::MemberStateChange
                        ) {
                            continue;
                        }
                        info!(
                            "Distro: membership change detected ({}), refreshing mapper",
                            evt.change_type
                        );
                        this.refresh_mapper();
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(
                            "Distro member-change listener lagged: {} events skipped; \
                             verify loop will reconcile on next cycle",
                            skipped
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        debug!("Distro member-change listener: channel closed, stopping");
                        break;
                    }
                }
            }
        }));
    }

    /// Drop all locally-stored keys that the local node is no longer
    /// responsible for.
    ///
    /// Not called automatically any more. Nacos keeps a full replica set on
    /// every node: a node serves every key it has, and `is_responsible()` is
    /// only used to decide who answers verification and who acts as the sync
    /// source. Deleting "non-responsible" replicas therefore removes data
    /// that is still being served, which makes instances disappear from query
    /// results whenever the hash ring shrinks (for example when one member is
    /// briefly unreachable). Both former call sites (the verify loop and the
    /// member-change listener) now only refresh the mapper.
    ///
    /// Kept as an explicit maintenance operation: it is the right tool when a
    /// member is permanently removed and its data must be reclaimed.
    ///
    /// Returns the total number of keys removed across all registered
    /// handlers.
    pub async fn cleanup_non_responsible_keys(&self) -> usize {
        let mut total_removed = 0usize;
        let handler_entries: Vec<Arc<dyn DistroDataHandler>> =
            self.handlers.iter().map(|e| e.value().clone()).collect();
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
        drop(running);

        // `running=false` lets the sync/verify loops exit at their next
        // tick, but the member-change listener blocks on `rx.recv().await`
        // and has no tick. Abort every tracked task so none outlive the
        // protocol.
        let handles: Vec<_> = self
            .task_handles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        for handle in handles {
            handle.abort();
        }

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

    /// Number of parallel workers that drain ready sync tasks.
    ///
    /// Mirrors upstream `NacosExecuteTaskExecuteEngine`, which dispatches to
    /// `ThreadUtils.getSuitableThreadCount(1)` workers (smallest power of two
    /// >= core count) and routes a task by its distro key, so one key is always
    /// > serialised onto one worker while distinct keys sync in parallel.
    /// > batata drained every ready task on a single serial loop, which meant a
    /// > newly registered key waited for the RPC round-trip of every older key
    /// > queued in the same tick before it was even sent — under a registration
    /// > burst that pushed a fresh key's broadcast hundreds of milliseconds past
    /// > its scheduled time.
    fn sync_worker_count() -> usize {
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(1);
        let mut workers = 1usize;
        while workers < cores {
            workers <<= 1;
        }
        workers
    }

    /// Pick the worker that owns a distro key.
    ///
    /// Upstream routes with `addTask(distroKey, task)` into
    /// `getWorker(tag)`, i.e. `tag.hashCode() % workers`, so the tag is the
    /// distro key *without* the target: a key's tasks all land on one worker
    /// (serial, so a peer never sees two versions of one key out of order)
    /// while different keys spread across workers.
    fn sync_shard_index(data_type: &DistroDataType, key: &str, worker_count: usize) -> usize {
        use std::hash::{Hash, Hasher};

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        // Hash the components directly instead of allocating a temporary
        // "type:key" String on every call (this runs on the 100ms sync tick).
        // Shard assignment is an internal, per-process routing decision that
        // is neither persisted nor sent on the wire, so only determinism
        // within one process is required.
        data_type.hash(&mut hasher);
        key.hash(&mut hasher);
        (hasher.finish() % worker_count as u64) as usize
    }

    /// Atomically claim sync tasks that are due at `now`.
    ///
    /// The scan and the removal are separate phases: a concurrent
    /// `sync_data()`/retry can replace a task with a newer `scheduled_time`
    /// between them. `claim_observed_sync_tasks` only removes a value whose
    /// `scheduled_time` still matches the observation, so a newer task is
    /// never dropped by an older claim (TOCTOU).
    fn claim_ready_sync_tasks(
        sync_tasks: &DashMap<String, DistroSyncTask>,
        now: i64,
    ) -> Vec<(String, DistroSyncTask)> {
        let observed: Vec<(String, i64)> = sync_tasks
            .iter()
            .filter(|e| e.value().scheduled_time <= now)
            .map(|e| (e.key().clone(), e.value().scheduled_time))
            .collect();
        Self::claim_observed_sync_tasks(sync_tasks, observed)
    }

    /// Remove exactly the tasks whose `scheduled_time` still equals the value
    /// observed during the scan. A plain `remove(&key)` would delete a newer
    /// task inserted after the scan.
    fn claim_observed_sync_tasks(
        sync_tasks: &DashMap<String, DistroSyncTask>,
        observed: Vec<(String, i64)>,
    ) -> Vec<(String, DistroSyncTask)> {
        observed
            .into_iter()
            .filter_map(|(key, observed_time)| {
                sync_tasks.remove_if(&key, |_k, v| v.scheduled_time == observed_time)
            })
            .collect()
    }

    /// Send one ready sync task, re-queueing it with backoff when the RPC fails.
    ///
    /// `now` is the tick timestamp shared by every task claimed in the same
    /// loop iteration so backoff is measured from one instant.
    #[allow(clippy::too_many_arguments)]
    async fn run_sync_task(
        handlers: &Arc<DashMap<DistroDataType, Arc<dyn DistroDataHandler>>>,
        client_manager: &Arc<ClusterClientManager>,
        config: &DistroConfig,
        counters: &Arc<DistroCounters>,
        sync_tasks: &Arc<DashMap<String, DistroSyncTask>>,
        task_key: String,
        task: DistroSyncTask,
        now: i64,
    ) {
        // Own the handler Arc and drop the DashMap shard read guard BEFORE
        // any await: holding it across the RPC (or the async get_data) would
        // block writers to this shard for the whole round-trip.
        let handler = handlers
            .get(&task.data_type)
            .map(|entry| Arc::clone(entry.value()));
        // Get data from handler and send sync request
        if let Some(handler) = handler
            && let Some(data) = handler.get_data(&task.key).await
        {
            // Send sync request (bounded by sync_timeout inside the helper)
            let result = Self::send_sync_data(
                client_manager,
                &task.target_address,
                data.clone(),
                config.sync_timeout,
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
                // Bound retries so a permanently unreachable peer cannot pin
                // a task (and its memory) in the queue forever. The verify
                // cycle re-detects any divergence once the peer is reachable
                // again.
                if task.retry_count >= MAX_SYNC_RETRIES {
                    warn!(
                        "Dropping sync task for {}:{} to {} after {} retries (max reached)",
                        task.data_type, task.key, task.target_address, task.retry_count
                    );
                    return;
                }
                // Nacos-compatible retry: capped exponential backoff. The task
                // stays in the queue until it succeeds, the retry budget is
                // exhausted, or the target leaves the cluster (pruned by the
                // verify loop).
                //
                // Backoff schedule (cap at 2^6 = 64x base):
                //   attempt 1: base * 2
                //   attempt 2: base * 4
                //   ...
                //   attempt 6+: base * 64 (ceiling)
                const BACKOFF_CEILING_SHIFT: u32 = 6;
                let mut updated_task = task.clone();
                updated_task.retry_count = updated_task.retry_count.saturating_add(1);
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
                        rc, task.data_type, task.key, task.target_address, backoff
                    );
                } else if rc <= 10 {
                    warn!(
                        "Sync retry {} for {}:{} to {} (backoff={}ms)",
                        rc, task.data_type, task.key, task.target_address, backoff
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

    /// Start the sync task processor
    async fn start_sync_task_processor(&self) {
        let sync_tasks = self.sync_tasks.clone();
        let handlers = self.handlers.clone();
        let running = self.running.clone();
        let client_manager = self.client_manager.clone();
        let config = self.config.clone();
        let _local_address = self.local_address.clone();
        let counters = self.counters.clone();

        self.track_task(tokio::spawn(async move {
            loop {
                {
                    let is_running = running.read().await;
                    if !*is_running {
                        break;
                    }
                }

                let now = chrono::Utc::now().timestamp_millis();

                // Atomically claim ready tasks. A concurrent sync_data()/retry
                // can replace a task with a newer scheduled_time between the
                // scan and the removal; claim_ready_sync_tasks verifies the
                // observed scheduled_time before removing (avoids TOCTOU).
                let ready_tasks = Self::claim_ready_sync_tasks(&sync_tasks, now);

                if !ready_tasks.is_empty() {
                    // Shard by distro key — upstream `addTask(distroKey, task)`
                    // routes a task to one worker by key, so tasks for the same
                    // key stay serial (no out-of-order overwrite on the peer)
                    // while distinct keys are sent in parallel. Sending the
                    // whole tick serially instead let one slow peer delay every
                    // unrelated key behind it.
                    let worker_count = Self::sync_worker_count();
                    let mut shards: Vec<Vec<(String, DistroSyncTask)>> =
                        vec![Vec::new(); worker_count];
                    for entry in ready_tasks {
                        let idx =
                            Self::sync_shard_index(&entry.1.data_type, &entry.1.key, worker_count);
                        shards[idx].push(entry);
                    }

                    let mut workers = Vec::with_capacity(shards.len());
                    for shard in shards {
                        if shard.is_empty() {
                            continue;
                        }
                        let handlers = handlers.clone();
                        let client_manager = client_manager.clone();
                        let config = config.clone();
                        let counters = counters.clone();
                        let sync_tasks = sync_tasks.clone();
                        workers.push(async move {
                            for (task_key, task) in shard {
                                Self::run_sync_task(
                                    &handlers,
                                    &client_manager,
                                    &config,
                                    &counters,
                                    &sync_tasks,
                                    task_key,
                                    task,
                                    now,
                                )
                                .await;
                            }
                        });
                    }
                    futures::future::join_all(workers).await;
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }));
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

        self.track_task(tokio::spawn(async move {
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
                //
                // MUST go through `install_ring` — the same rule as
                // `refresh_mapper()`. This loop used to filter on `Up` alone,
                // which shrank the ring the moment a member flickered to
                // `Suspicious` and re-partitioned every key; because the
                // mapper is shared with the naming health check, that also
                // moved instance probing to a different node.
                install_ring(&members, &mapper);

                // Evict sync tasks whose target has left the cluster so a
                // departed peer cannot keep a task retrying forever.
                prune_stale_sync_targets(&sync_tasks, &members);

                // NOTE: Distro keeps a full replica of every key on every node
                // (Nacos semantics) and this loop walks *all* of them, so
                // `is_responsible()` is NOT a gate here.
                //
                // Ownership of what gets published is decided one layer down,
                // by the handler: `NamingInstanceDistroHandler::get_data`
                // publishes only the instances this node's own clients
                // registered (no `_distro_remote` marker), the batata
                // counterpart of Nacos' `isResponsibleClient`. Keys the node
                // owns nothing for are still published — with an empty
                // instance list, which is how peers garbage-collect the
                // replicas this node previously replicated to them.
                //
                // Deleting keys the local node is not responsible for would
                // drop replicas that peers just pushed, so cross-node instance
                // lookups would fail on every non-responsible node.
                // Responsibility changes are reconciled by sync/verify, never
                // by deletion.

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

                    // Only versions are needed here; `get_data` would rebuild
                    // (and serialize) the full sync payload for every key.
                    let verify_data = collect_verify_versions(handler, &keys).await;

                    if verify_data.is_empty() {
                        continue;
                    }

                    // Build the verify request ONCE. Every peer receives the
                    // identical payload, so encoding it a single time and
                    // sharing the bytes avoids cloning (and re-serializing)
                    // the full key->version map once per peer.
                    let (api_data_type, custom_type_name) = match data_type {
                        DistroDataType::NamingInstance => (
                            batata_server_api::distro::DistroDataType::NamingInstance,
                            None,
                        ),
                        DistroDataType::Custom(name) => (
                            batata_server_api::distro::DistroDataType::Custom,
                            Some(name.clone()),
                        ),
                    };
                    let mut verify_request = DistroDataVerifyRequest::new();
                    verify_request.data_type = api_data_type;
                    verify_request.custom_type_name = custom_type_name;
                    verify_request.verify_data = verify_data; // move, no per-peer clone
                    // Versions are per-node counters: tell the peer whose
                    // counter it is being asked about.
                    verify_request.source = local_address.clone();
                    let request_type = verify_request.request_type();
                    let encoded = Arc::new(verify_request.body());
                    let verify_timeout = config.verify_timeout;

                    // Send verify requests to ALL peers in parallel
                    let mut handles = Vec::with_capacity(other_members.len());

                    for member_address in &other_members {
                        let cm = client_manager.clone();
                        let addr = member_address.clone();
                        let dt = data_type.clone();
                        let body = Arc::clone(&encoded);

                        // Each peer verification runs concurrently, bounded
                        // by verify_timeout so a hung peer cannot stall the
                        // whole verify cycle.
                        handles.push(tokio::spawn(async move {
                            let request = SharedVerifyRequest { request_type, body };
                            let send = tokio::time::timeout(
                                verify_timeout,
                                cm.send_request(&addr, request),
                            )
                            .await;
                            match send {
                                Ok(Ok(response)) => {
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
                                Ok(Err(e)) => {
                                    warn!("Failed to verify distro data with {}: {}", addr, e);
                                    None
                                }
                                Err(_) => {
                                    warn!(
                                        "Verify distro data with {} timed out after {:?}",
                                        addr, verify_timeout
                                    );
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
        }));
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
        let data_types: Vec<DistroDataType> =
            self.handlers.iter().map(|e| e.key().clone()).collect();

        for data_type in &data_types {
            let api_data_type = match data_type {
                DistroDataType::NamingInstance => {
                    batata_server_api::distro::DistroDataType::NamingInstance
                }
                DistroDataType::Custom(name) => {
                    // For custom types we need to use for_custom_type constructor
                    // but store the name for logging. The api_data_type is Custom.
                    let _ = name;
                    batata_server_api::distro::DistroDataType::Custom
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

                    // Bound the snapshot RPC so an unreachable peer cannot
                    // stall initial load until the next retry round.
                    let send = tokio::time::timeout(
                        self.config.sync_timeout,
                        self.client_manager.send_request(member_address, request),
                    )
                    .await;
                    match send {
                        Ok(Ok(response)) => {
                            if let Some(body) = response.body
                                && let Ok(snapshot_response) =
                                    serde_json::from_slice::<DistroDataSnapshotResponse>(
                                        &body.value,
                                    )
                            {
                                let item_count = snapshot_response.snapshot.len();
                                let mut applied = 0usize;
                                let mut failures = 0usize;
                                for item in snapshot_response.snapshot {
                                    let internal_data_type = match item.data_type {
                                        batata_server_api::distro::DistroDataType::NamingInstance => {
                                            DistroDataType::NamingInstance
                                        }
                                        batata_server_api::distro::DistroDataType::Custom => {
                                            DistroDataType::Custom(
                                                custom_type_name(item.custom_type_name.clone()),
                                            )
                                        }
                                    };

                                    let data = DistroData::with_version(
                                        internal_data_type,
                                        item.key,
                                        item.content.into_bytes(),
                                        item.source,
                                        item.version,
                                    );

                                    match self.receive_sync_data(data).await {
                                        Ok(()) => applied += 1,
                                        Err(e) => {
                                            failures += 1;
                                            warn!("Failed to process snapshot data: {}", e);
                                        }
                                    }
                                }

                                // A snapshot where EVERY item failed to apply
                                // must not count as a successful load: doing
                                // so marks the node initialised with no data
                                // and skips the remaining peers/retries.
                                if applied == 0 && failures > 0 {
                                    warn!(
                                        "Snapshot for type {} from {} had {} items but all \
                                         failed to apply; trying next peer",
                                        data_type, member_address, item_count
                                    );
                                } else {
                                    info!(
                                        "Loaded {} items for type {} from {} ({} failed)",
                                        applied, data_type, member_address, failures
                                    );
                                    loaded = true;
                                    break;
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            warn!(
                                "Failed to load snapshot from {}: {}, trying next peer",
                                member_address, e
                            );
                        }
                        Err(_) => {
                            warn!(
                                "Snapshot load from {} timed out after {:?}, trying next peer",
                                member_address, self.config.sync_timeout
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

    /// Send a batch of distro data items to a target node in a single RPC.
    ///
    /// Splits the input into chunks of at most `config.batch_sync_size` to
    /// keep individual messages bounded. Returns `Ok(())` if every chunk
    /// returned a `result_code == 200` response (failed_keys may still be
    /// non-empty — those are logged but not surfaced as a hard error so the
    /// caller can rely on the verify cycle for eventual repair).
    pub async fn sync_data_batch(
        &self,
        target: &str,
        items: Vec<DistroData>,
    ) -> Result<(), String> {
        if items.is_empty() {
            return Ok(());
        }
        let chunk_size = self.config.batch_sync_size.max(1);
        let total = items.len();
        let mut iter = items.into_iter();
        loop {
            let chunk: Vec<DistroData> = iter.by_ref().take(chunk_size).collect();
            if chunk.is_empty() {
                break;
            }
            Self::send_batch_sync_data(
                &self.client_manager,
                target,
                chunk,
                self.config.sync_timeout,
            )
            .await?;
        }
        debug!(
            "Batch sync to {} complete: {} items in chunks of {}",
            target, total, chunk_size
        );
        Ok(())
    }

    /// Send a single batch RPC. Caller is responsible for chunking.
    async fn send_batch_sync_data(
        client_manager: &Arc<ClusterClientManager>,
        target: &str,
        items: Vec<DistroData>,
        sync_timeout: Duration,
    ) -> Result<(), String> {
        let api_items: Vec<DistroDataItem> = items
            .into_iter()
            .map(|d| -> Result<DistroDataItem, String> {
                let (api_data_type, custom_type_name) = match &d.data_type {
                    DistroDataType::NamingInstance => (
                        batata_server_api::distro::DistroDataType::NamingInstance,
                        None,
                    ),
                    DistroDataType::Custom(name) => (
                        batata_server_api::distro::DistroDataType::Custom,
                        Some(name.clone()),
                    ),
                };
                Ok(DistroDataItem {
                    data_type: api_data_type,
                    custom_type_name,
                    content: distro_content_to_string(d.content, &d.key)?,
                    key: d.key,
                    version: d.version,
                    source: d.source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let count = api_items.len();
        let request = DistroDataBatchSyncRequest::with_items(api_items);
        let send =
            tokio::time::timeout(sync_timeout, client_manager.send_request(target, request)).await;
        match send {
            Ok(Ok(response)) => {
                if let Some(body) = response.body
                    && let Ok(batch_response) =
                        serde_json::from_slice::<DistroDataBatchSyncResponse>(&body.value)
                {
                    if batch_response.result_code() == 200 {
                        if !batch_response.failed_keys.is_empty() {
                            warn!(
                                "Distro batch sync to {}: {}/{} items failed: {:?}",
                                target,
                                batch_response.failed_keys.len(),
                                count,
                                batch_response.failed_keys
                            );
                        }
                        return Ok(());
                    }
                    return Err(format!(
                        "Distro batch sync failed: {}",
                        batch_response.response.message
                    ));
                }
                // A missing or malformed body is NOT proof of success.
                // Returning Ok would count the sync as done and drop the
                // task, leaving the replica silently divergent; surface an
                // Err so the existing retry path re-queues it.
                Err(format!(
                    "Distro batch sync to {}: missing or malformed response body",
                    target
                ))
            }
            Ok(Err(e)) => Err(format!("Failed to send distro batch sync: {}", e)),
            Err(_) => Err(format!(
                "Distro batch sync to {} timed out after {:?}",
                target, sync_timeout
            )),
        }
    }

    /// Send sync data to a target node via gRPC
    async fn send_sync_data(
        client_manager: &Arc<ClusterClientManager>,
        target: &str,
        data: DistroData,
        sync_timeout: Duration,
    ) -> Result<(), String> {
        debug!(
            "Sending distro sync data to {}: type={}, key={}",
            target, data.data_type, data.key
        );

        // Convert internal DistroData to API DistroDataItem
        let (api_data_type, custom_type_name) = match &data.data_type {
            DistroDataType::NamingInstance => (
                batata_server_api::distro::DistroDataType::NamingInstance,
                None,
            ),
            DistroDataType::Custom(name) => (
                batata_server_api::distro::DistroDataType::Custom,
                Some(name.clone()),
            ),
        };

        let data_item = DistroDataItem {
            data_type: api_data_type,
            custom_type_name,
            content: distro_content_to_string(data.content.clone(), &data.key)?,
            key: data.key.clone(),
            version: data.version,
            source: data.source.clone(),
        };

        // Create sync request
        let request = DistroDataSyncRequest::with_data(data_item);

        // Send via cluster client manager, bounded by sync_timeout.
        let send =
            tokio::time::timeout(sync_timeout, client_manager.send_request(target, request)).await;
        match send {
            Ok(Ok(response)) => {
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
                // A missing or malformed body is NOT proof of success; let
                // the retry path re-queue instead of silently diverging.
                Err(format!(
                    "Distro sync for key={} to {}: missing or malformed response body",
                    data.key, target
                ))
            }
            Ok(Err(e)) => Err(format!("Failed to send distro sync: {}", e)),
            Err(_) => Err(format!(
                "Distro sync for key={} to {} timed out after {:?}",
                data.key, target, sync_timeout
            )),
        }
    }

    /// Process received sync data
    pub async fn receive_sync_data(&self, data: DistroData) -> Result<(), String> {
        // Own the handler Arc and drop the shard read guard before awaiting.
        let handler = self
            .handlers
            .get(&data.data_type)
            .map(|entry| Arc::clone(entry.value()));
        if let Some(handler) = handler {
            handler.process_sync_data(data).await
        } else {
            Err(format!("No handler for data type: {}", data.data_type))
        }
    }

    /// Ask the handler for `data_type` whether the local replica already holds
    /// version `version` of `key`, as produced by `source`.
    ///
    /// Returns `false` (i.e. "please resync me") when the data type has no
    /// handler, when the handler errors, or when `source` is unknown — a false
    /// negative costs one extra sync, a false positive leaves the replica
    /// silently divergent.
    pub async fn is_in_sync_with(
        &self,
        data_type: &DistroDataType,
        key: &str,
        source: &str,
        version: i64,
    ) -> bool {
        // Own the handler Arc and drop the shard read guard before awaiting.
        let handler = self
            .handlers
            .get(data_type)
            .map(|entry| Arc::clone(entry.value()));
        let Some(handler) = handler else {
            return false;
        };
        let probe = DistroData::with_version(
            data_type.clone(),
            key.to_string(),
            Vec::new(),
            source.to_string(),
            version,
        );
        handler
            .process_verify_data(&probe)
            .await
            .unwrap_or_else(|e| {
                warn!("Verify probe failed for {} (source={}): {}", key, source, e);
                false
            })
    }

    /// Process a batch of received sync items.
    ///
    /// Each item is dispatched to its registered handler the same way
    /// `receive_sync_data` handles a single item. Items whose data type has
    /// no registered handler, or whose handler returns an error, are added
    /// to the returned `failed_keys` list. The remainder still applies —
    /// matching Nacos' best-effort batched apply semantics.
    pub async fn receive_batch_sync(&self, items: Vec<DistroData>) -> Vec<String> {
        let mut failed_keys = Vec::new();
        for data in items {
            let key = data.key.clone();
            match self.receive_sync_data(data).await {
                Ok(()) => {}
                Err(e) => {
                    warn!("Batch sync apply failed for key '{}': {}", key, e);
                    failed_keys.push(key);
                }
            }
        }
        failed_keys
    }

    /// Explicitly remove a key from the local store across all handlers.
    ///
    /// Called when this node has dispossessed responsibility for `key` — for
    /// example, when a verify round confirms the key is owned by another peer
    /// and the local copy has gone stale, or after a cluster membership change
    /// reassigned ownership. Idempotent: removing a non-existent key is fine.
    ///
    /// This is the per-key counterpart to `cleanup_non_responsible_keys`,
    /// useful when the dispossession is event-driven (single key) rather than
    /// triggered by a full membership refresh.
    pub async fn dispossess_key(&self, key: &str) -> usize {
        let mut removed = 0usize;
        let handler_entries: Vec<Arc<dyn DistroDataHandler>> =
            self.handlers.iter().map(|e| e.value().clone()).collect();
        for handler in handler_entries {
            if let Err(e) = handler.remove_data(key).await {
                warn!(
                    "dispossess_key: handler removal failed for '{}': {}",
                    key, e
                );
            } else {
                removed += 1;
            }
        }
        if removed > 0 {
            debug!(
                "dispossess_key: removed key '{}' from {} handler(s)",
                key, removed
            );
        }
        removed
    }

    /// Get snapshot for initial data load
    pub async fn get_snapshot(&self, data_type: &DistroDataType) -> Vec<DistroData> {
        // Own the handler Arc and drop the shard read guard before awaiting.
        let handler = self
            .handlers
            .get(data_type)
            .map(|entry| Arc::clone(entry.value()));
        if let Some(handler) = handler {
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
    fn test_distro_content_decodes_utf8() {
        let content = distro_content_to_string(b"{\"a\":1}".to_vec(), "service-key").unwrap();
        assert_eq!(content, "{\"a\":1}");
    }

    #[test]
    fn test_distro_content_rejects_non_utf8_instead_of_empty_string() {
        // unwrap_or_default() would silently broadcast "" to every peer.
        let err = distro_content_to_string(vec![0xff, 0xfe], "service-key").unwrap_err();
        assert!(err.contains("service-key"), "{err}");
        assert!(err.contains("UTF-8"), "{err}");
    }

    #[test]
    fn test_custom_type_fallback_is_shared_constant() {
        // Sender and receiver must bucket a missing custom name identically;
        // unwrap_or_default() on one side used to produce "" instead of "CUSTOM".
        assert_eq!(custom_type_name(None), "CUSTOM");
        assert_eq!(DEFAULT_CUSTOM_TYPE_NAME, "CUSTOM");
        assert_eq!(custom_type_name(Some("metrics".to_string())), "metrics");
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
    fn test_sync_shard_index_pins_one_key_to_one_worker() {
        // Upstream hashes the distro key (target excluded), so every task for
        // one key — one per peer — serialises on the same worker. Syncing the
        // same key to two peers in parallel could deliver them out of order.
        let workers = 8;
        let to_a =
            DistroProtocol::sync_shard_index(&DistroDataType::NamingInstance, "svc-a", workers);
        let to_b =
            DistroProtocol::sync_shard_index(&DistroDataType::NamingInstance, "svc-a", workers);
        assert_eq!(to_a, to_b, "one key must not be split across workers");

        // A different key may land anywhere, but always in range.
        let other =
            DistroProtocol::sync_shard_index(&DistroDataType::NamingInstance, "svc-b", workers);
        assert!(other < workers);

        // The data type is part of the key: same name under another type is a
        // different distro key.
        assert_eq!(
            DistroProtocol::sync_shard_index(
                &DistroDataType::Custom("ns".into()),
                "svc-a",
                workers
            ),
            DistroProtocol::sync_shard_index(
                &DistroDataType::Custom("ns".into()),
                "svc-a",
                workers
            )
        );
    }

    #[test]
    fn test_sync_worker_count_is_a_power_of_two_at_or_above_cores() {
        let workers = DistroProtocol::sync_worker_count();
        assert!(workers >= 1);
        assert_eq!(
            workers & (workers - 1),
            0,
            "{} is not a power of two",
            workers
        );

        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(1);
        assert!(
            workers >= cores,
            "{} workers cannot cover {} cores",
            workers,
            cores
        );
        // `getSuitableThreadCount(1)` stops at the first power of two that
        // reaches the core count, so doubling again must overshoot.
        assert!(workers < cores * 2);
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

    /// Build a `Member` for the ring tests. Only `address` and `state` matter
    /// here — the mapper hashes addresses.
    fn member_with_state(address: &str, state: batata_api::model::NodeState) -> Member {
        Member {
            ip: address.split(':').next().unwrap_or("").to_string(),
            port: 8848,
            state,
            extend_info: Arc::new(std::sync::RwLock::new(std::collections::BTreeMap::new())),
            address: address.to_string(),
            fail_access_cnt: 0,
        }
    }

    /// A `DistroProtocol` over an explicit member set. `ClusterClientManager`
    /// is inert until something is sent to it, so this needs no I/O.
    fn protocol_with_members(entries: Vec<(&str, batata_api::model::NodeState)>) -> DistroProtocol {
        use super::super::cluster_client::ClusterClientConfig;
        let members = Arc::new(DashMap::new());
        for (address, state) in entries {
            members.insert(address.to_string(), member_with_state(address, state));
        }
        DistroProtocol::new(
            "10.0.0.1:8848".to_string(),
            DistroConfig::default(),
            Arc::new(ClusterClientManager::new(
                "10.0.0.1:8848".to_string(),
                ClusterClientConfig::default(),
            )),
            members,
        )
    }

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    /// The ring rule: only a `Down` member leaves it.
    #[test]
    fn test_ring_members_excludes_only_down() {
        use batata_api::model::NodeState;
        let members = Arc::new(DashMap::new());
        for (address, state) in [
            ("10.0.0.1:8848", NodeState::Up),
            ("10.0.0.2:8848", NodeState::Suspicious),
            ("10.0.0.3:8848", NodeState::Starting),
            ("10.0.0.4:8848", NodeState::Isolation),
            ("10.0.0.5:8848", NodeState::Down),
        ] {
            members.insert(address.to_string(), member_with_state(address, state));
        }

        assert_eq!(
            ring_members(&members).len(),
            4,
            "a single failed health check must not shrink the ring"
        );
        assert!(!ring_members(&members).contains(&"10.0.0.5:8848".to_string()));
    }

    /// `refresh_mapper` and the verify loop must build the ring from the *same*
    /// list — the mapper is shared with the naming health check, so two rules
    /// would mean two owners for one key.
    #[test]
    fn test_refresh_mapper_matches_ring_members() {
        use batata_api::model::NodeState;
        let protocol = protocol_with_members(vec![
            ("10.0.0.1:8848", NodeState::Up),
            ("10.0.0.2:8848", NodeState::Suspicious),
            ("10.0.0.3:8848", NodeState::Up),
        ]);

        protocol.refresh_mapper();
        assert_eq!(
            sorted(protocol.mapper().get_members()),
            sorted(protocol.ring_members())
        );
        assert_eq!(
            protocol.mapper().get_members().len(),
            3,
            "the verify-loop rule must not drop the Suspicious member"
        );
    }

    /// A member flapping to `Suspicious` must not move any key's owner.
    #[test]
    fn test_suspicious_flap_does_not_move_responsibility() {
        use batata_api::model::NodeState;
        let protocol = protocol_with_members(vec![
            ("10.0.0.1:8848", NodeState::Up),
            ("10.0.0.2:8848", NodeState::Up),
            ("10.0.0.3:8848", NodeState::Up),
        ]);
        protocol.refresh_mapper();

        let keys: Vec<String> = (0..64).map(|i| format!("service-{}", i)).collect();
        let before: Vec<Option<String>> = keys
            .iter()
            .map(|k| protocol.mapper().responsible_node(k))
            .collect();

        // One failed health check.
        protocol.members.insert(
            "10.0.0.2:8848".to_string(),
            member_with_state("10.0.0.2:8848", NodeState::Suspicious),
        );
        protocol.refresh_mapper();

        let after: Vec<Option<String>> = keys
            .iter()
            .map(|k| protocol.mapper().responsible_node(k))
            .collect();
        assert_eq!(
            before, after,
            "a Suspicious member must stay in the ring: re-partitioning would also \
             move health-check probing to another node"
        );
    }

    /// A member that is really gone does shrink the ring — and every key must
    /// still land on exactly one of the survivors.
    #[test]
    fn test_down_member_hands_its_keys_to_the_survivors() {
        use batata_api::model::NodeState;
        let protocol = protocol_with_members(vec![
            ("10.0.0.1:8848", NodeState::Up),
            ("10.0.0.2:8848", NodeState::Up),
            ("10.0.0.3:8848", NodeState::Up),
        ]);
        protocol.refresh_mapper();

        protocol.members.insert(
            "10.0.0.3:8848".to_string(),
            member_with_state("10.0.0.3:8848", NodeState::Down),
        );
        protocol.refresh_mapper();

        let ring = protocol.ring_members();
        assert_eq!(ring.len(), 2, "a Down member must leave the ring");
        assert!(!ring.contains(&"10.0.0.3:8848".to_string()));

        for i in 0..64 {
            let key = format!("service-{}", i);
            let owner = protocol
                .mapper()
                .responsible_node(&key)
                .expect("every key must have an owner");
            assert!(ring.contains(&owner), "owner {owner} is not in the ring");
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

    /// Mock handler for testing that stores received items in a DashMap.
    /// Exercises the public `DistroDataHandler` surface without requiring
    /// a real NamingService.
    struct MockHandler {
        data: Arc<DashMap<String, DistroData>>,
        fail_keys: Arc<DashMap<String, ()>>,
        get_data_calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl MockHandler {
        fn new() -> Self {
            Self {
                data: Arc::new(DashMap::new()),
                fail_keys: Arc::new(DashMap::new()),
                get_data_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }
        }

        fn share(&self) -> Arc<DashMap<String, DistroData>> {
            self.data.clone()
        }
    }

    #[tonic::async_trait]
    impl DistroDataHandler for MockHandler {
        fn data_type(&self) -> DistroDataType {
            DistroDataType::NamingInstance
        }

        async fn get_all_keys(&self) -> Vec<String> {
            self.data.iter().map(|e| e.key().clone()).collect()
        }

        async fn get_data(&self, key: &str) -> Option<DistroData> {
            self.get_data_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.data.get(key).map(|e| e.value().clone())
        }

        async fn get_version(&self, key: &str) -> Option<i64> {
            // Cheap override: no payload clone, no serialization.
            self.data.get(key).map(|e| e.value().version)
        }

        async fn process_sync_data(&self, data: DistroData) -> Result<(), String> {
            if self.fail_keys.contains_key(&data.key) {
                return Err(format!("forced failure for {}", data.key));
            }
            self.data.insert(data.key.clone(), data);
            Ok(())
        }

        async fn process_verify_data(&self, data: &DistroData) -> Result<bool, String> {
            Ok(self
                .data
                .get(&data.key)
                .map(|local| local.version >= data.version)
                .unwrap_or(false))
        }

        async fn get_snapshot(&self) -> Vec<DistroData> {
            self.data.iter().map(|e| e.value().clone()).collect()
        }

        async fn remove_data(&self, key: &str) -> Result<(), String> {
            self.data.remove(key);
            Ok(())
        }
    }

    fn build_item(key: &str) -> DistroData {
        DistroData::new(
            DistroDataType::NamingInstance,
            key.to_string(),
            format!("{{\"k\":\"{}\"}}", key).into_bytes(),
            "10.0.0.1:8848".to_string(),
        )
    }

    /// Create a DistroProtocol wired to a mock handler with no real cluster
    /// client. `sync_data_batch` / `send_*` functions would fail on I/O, but
    /// all receive-side paths (`receive_batch_sync`, `dispossess_key`) work.
    fn test_protocol(local: &str) -> (Arc<DistroProtocol>, Arc<MockHandler>) {
        use crate::service::cluster_client::ClusterClientConfig;
        let client_manager = Arc::new(ClusterClientManager::new(
            local.to_string(),
            ClusterClientConfig::default(),
        ));
        let members = Arc::new(DashMap::new());
        let proto = Arc::new(DistroProtocol::new(
            local.to_string(),
            DistroConfig::default(),
            client_manager,
            members,
        ));
        let handler = Arc::new(MockHandler::new());
        proto.register_handler(handler.clone() as Arc<dyn DistroDataHandler>);
        (proto, handler)
    }

    #[tokio::test]
    async fn test_batch_sync_applies_all_items() {
        let (proto, handler) = test_protocol("10.0.0.1:8848");
        let items: Vec<DistroData> = (0..50).map(|i| build_item(&format!("svc-{}", i))).collect();

        let failed = proto.receive_batch_sync(items).await;
        assert!(
            failed.is_empty(),
            "no item should fail in a clean batch, got: {:?}",
            failed
        );
        let store = handler.share();
        assert_eq!(store.len(), 50, "all 50 items must be applied");
        for i in 0..50 {
            let key = format!("svc-{}", i);
            let stored = store.get(&key).unwrap();
            assert_eq!(stored.key, key);
            assert_eq!(stored.data_type, DistroDataType::NamingInstance);
        }
    }

    /// `is_in_sync_with` must delegate to the handler and answer "resync me"
    /// whenever it cannot prove the replica is current.
    #[tokio::test]
    async fn test_is_in_sync_with_delegates_to_handler() {
        let (proto, _handler) = test_protocol("10.0.0.1:8848");

        // Nothing applied yet -> stale.
        assert!(
            !proto
                .is_in_sync_with(&DistroDataType::NamingInstance, "svc-a", "10.0.0.2:8848", 1)
                .await
        );

        let mut applied = build_item("svc-a");
        applied.version = 5;
        proto.receive_sync_data(applied).await.unwrap();

        assert!(
            proto
                .is_in_sync_with(&DistroDataType::NamingInstance, "svc-a", "10.0.0.2:8848", 5)
                .await
        );
        assert!(
            !proto
                .is_in_sync_with(&DistroDataType::NamingInstance, "svc-a", "10.0.0.2:8848", 6)
                .await,
            "a newer version on the peer must be reported as stale"
        );
        assert!(
            !proto
                .is_in_sync_with(
                    &DistroDataType::Custom("nope".into()),
                    "svc-a",
                    "10.0.0.2:8848",
                    5
                )
                .await,
            "an unregistered data type cannot be proven in sync"
        );
    }

    #[tokio::test]
    async fn test_batch_sync_reports_per_item_failures() {
        let (proto, handler) = test_protocol("10.0.0.1:8848");
        handler.fail_keys.insert("svc-bad".to_string(), ());

        let items = vec![
            build_item("svc-ok-1"),
            build_item("svc-bad"),
            build_item("svc-ok-2"),
        ];
        let failed = proto.receive_batch_sync(items).await;
        assert_eq!(failed, vec!["svc-bad".to_string()]);
        // The surviving items still applied.
        let store = handler.share();
        assert!(store.contains_key("svc-ok-1"));
        assert!(store.contains_key("svc-ok-2"));
        assert!(!store.contains_key("svc-bad"));
    }

    #[tokio::test]
    async fn test_batch_sync_equivalent_to_single_item_loop() {
        // Property-style: feeding items through the batch path should
        // produce identical final state to feeding them one by one through
        // `receive_sync_data`.
        let (proto_batch, handler_batch) = test_protocol("10.0.0.1:8848");
        let (proto_single, handler_single) = test_protocol("10.0.0.1:8848");

        let items: Vec<DistroData> = (0..25)
            .map(|i| build_item(&format!("svc-{:02}", i)))
            .collect();

        // Batch path
        let failed_batch = proto_batch.receive_batch_sync(items.clone()).await;
        assert!(failed_batch.is_empty());

        // Single-item loop
        for item in items.iter().cloned() {
            proto_single.receive_sync_data(item).await.unwrap();
        }

        let a: std::collections::BTreeMap<String, Vec<u8>> = handler_batch
            .share()
            .iter()
            .map(|e| (e.key().clone(), e.value().content.clone()))
            .collect();
        let b: std::collections::BTreeMap<String, Vec<u8>> = handler_single
            .share()
            .iter()
            .map(|e| (e.key().clone(), e.value().content.clone()))
            .collect();
        assert_eq!(
            a, b,
            "batch and per-item paths must converge to the same state"
        );
        assert_eq!(a.len(), 25);
    }

    #[tokio::test]
    async fn test_dispossess_key_removes_local_copy() {
        // Simulate: node owned svc-old, cluster membership shifted, peer
        // now reports this node is no longer responsible. Dispossession
        // must drop the local copy so the next authoritative register
        // replicates fresh.
        let (proto, handler) = test_protocol("10.0.0.1:8848");
        proto
            .receive_sync_data(build_item("svc-keep"))
            .await
            .unwrap();
        proto
            .receive_sync_data(build_item("svc-drop"))
            .await
            .unwrap();
        assert_eq!(handler.share().len(), 2);

        let removed = proto.dispossess_key("svc-drop").await;
        assert_eq!(removed, 1, "exactly one handler should acknowledge removal");
        assert!(!handler.share().contains_key("svc-drop"));
        assert!(
            handler.share().contains_key("svc-keep"),
            "unrelated keys must be preserved"
        );

        // Idempotent: removing again is fine.
        let again = proto.dispossess_key("svc-drop").await;
        assert_eq!(again, 1, "removal is idempotent; still calls handler");
    }

    #[tokio::test]
    async fn test_verify_failure_triggers_dispossession_flow() {
        // Owner-side simulation: build a protocol whose verify says the
        // key is missing locally (version mismatch). After dispossession,
        // the handler reports zero keys and the next sync re-populates.
        let (proto, handler) = test_protocol("10.0.0.1:8848");
        proto
            .receive_sync_data(build_item("svc-stale"))
            .await
            .unwrap();

        // Dispossess the service as if verify-failure had fired.
        proto.dispossess_key("svc-stale").await;
        assert!(handler.get_all_keys().await.is_empty());

        // Next sync from the new owner populates it again.
        proto
            .receive_sync_data(build_item("svc-stale"))
            .await
            .unwrap();
        assert_eq!(handler.get_all_keys().await, vec!["svc-stale".to_string()]);
    }

    #[test]
    fn test_distro_batch_size_default() {
        assert_eq!(DISTRO_BATCH_SIZE, 50);
        let cfg = DistroConfig::default();
        assert_eq!(cfg.batch_sync_size, 50);
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

    fn sync_task(key: &str, scheduled_time: i64, retry_count: u32) -> DistroSyncTask {
        DistroSyncTask {
            data_type: DistroDataType::NamingInstance,
            key: key.to_string(),
            target_address: "10.0.0.2:8848".to_string(),
            scheduled_time,
            retry_count,
        }
    }

    /// The shared-body verify adapter must emit the exact bytes and request
    /// type of a normal `DistroDataVerifyRequest` (wire compatibility).
    #[test]
    fn test_shared_verify_request_preserves_wire_bytes_and_type() {
        use batata_api::remote::model::RequestTrait as _;
        let mut request = DistroDataVerifyRequest::new();
        request.verify_data.insert("k".to_string(), 7);
        request.source = "10.0.0.1:8848".to_string();
        let expected = request.body();
        let shared = SharedVerifyRequest {
            request_type: request.request_type(),
            body: Arc::new(expected.clone()),
        };
        assert_eq!(
            shared.body(),
            expected,
            "body() must replay the exact encoded bytes"
        );
        assert_eq!(shared.request_type(), request.request_type());
    }

    /// Tasks targeting a departed member are pruned; live ones are kept.
    #[test]
    fn test_prune_stale_sync_targets() {
        use batata_api::model::NodeState;
        let sync_tasks: DashMap<String, DistroSyncTask> = DashMap::new();
        let members: DashMap<String, Member> = DashMap::new();
        members.insert(
            "10.0.0.2:8848".to_string(),
            member_with_state("10.0.0.2:8848", NodeState::Up),
        );

        let mut live = sync_task("live", 0, 0);
        live.target_address = "10.0.0.2:8848".to_string();
        let mut stale = sync_task("stale", 0, 0);
        stale.target_address = "10.0.0.9:8848".to_string(); // departed member
        sync_tasks.insert("live".to_string(), live);
        sync_tasks.insert("stale".to_string(), stale);

        prune_stale_sync_targets(&sync_tasks, &members);

        assert!(
            sync_tasks.contains_key("live"),
            "tasks for live members must be kept"
        );
        assert!(
            !sync_tasks.contains_key("stale"),
            "tasks for departed members must be pruned"
        );
    }

    #[test]
    fn test_claim_ready_sync_tasks_only_claims_due_keys() {
        let tasks: DashMap<String, DistroSyncTask> = DashMap::new();
        tasks.insert("due".to_string(), sync_task("due", 50, 0));
        tasks.insert("future".to_string(), sync_task("future", 5000, 0));

        let claimed = DistroProtocol::claim_ready_sync_tasks(&tasks, 1000);
        assert_eq!(claimed.len(), 1, "only the due task must be claimed");
        assert_eq!(claimed[0].0, "due");
        assert!(!tasks.contains_key("due"));
        assert!(
            tasks.contains_key("future"),
            "a task not yet due must stay queued"
        );
    }

    #[test]
    fn test_claim_observed_sync_tasks_keeps_newer_replacement() {
        let tasks: DashMap<String, DistroSyncTask> = DashMap::new();
        // Guard: a value refreshed after the scan (newer scheduled_time)
        // must NOT be removed by a stale observation.
        tasks.insert("k".to_string(), sync_task("k", 9999, 0));
        let observed = vec![("k".to_string(), 50i64)];
        let claimed = DistroProtocol::claim_observed_sync_tasks(&tasks, observed);
        assert!(
            claimed.is_empty(),
            "a newer task must not be claimed by a stale observation"
        );
        assert_eq!(tasks.get("k").unwrap().scheduled_time, 9999);

        // Success path: an unchanged value IS claimed.
        let observed2 = vec![("k".to_string(), 9999i64)];
        let claimed2 = DistroProtocol::claim_observed_sync_tasks(&tasks, observed2);
        assert_eq!(claimed2.len(), 1);
        assert!(tasks.is_empty());
    }

    /// A failed sync below the cap is re-queued; at the cap it is dropped.
    #[tokio::test]
    async fn test_sync_task_bounded_retries() {
        let (proto, handler) = test_protocol("10.0.0.1:8848");
        // Non-UTF-8 content makes send_sync_data fail fast in
        // distro_content_to_string, with no network I/O.
        let mut bad = build_item("svc-a");
        bad.content = vec![0xff, 0xfe];
        handler.share().insert("svc-a".to_string(), bad);

        let sync_tasks: Arc<DashMap<String, DistroSyncTask>> = Arc::new(DashMap::new());
        let config = proto.config.clone();

        // Success path: below the cap, a failed sync is re-queued with an
        // incremented retry_count and a future scheduled_time.
        DistroProtocol::run_sync_task(
            &proto.handlers,
            &proto.client_manager,
            &config,
            &proto.counters,
            &sync_tasks,
            "task-below".to_string(),
            sync_task("svc-a", 0, 0),
            10_000,
        )
        .await;
        {
            let t = sync_tasks
                .get("task-below")
                .expect("a below-cap failure must be re-queued");
            assert_eq!(t.retry_count, 1);
            assert!(
                t.scheduled_time > 10_000,
                "backoff must push the retry into the future"
            );
        }

        // Guard: at the cap the task is dropped, not re-queued.
        DistroProtocol::run_sync_task(
            &proto.handlers,
            &proto.client_manager,
            &config,
            &proto.counters,
            &sync_tasks,
            "task-capped".to_string(),
            sync_task("svc-a", 0, MAX_SYNC_RETRIES),
            10_000,
        )
        .await;
        assert!(
            sync_tasks.get("task-capped").is_none(),
            "a task that exhausted its retry budget must be dropped"
        );
    }

    /// A guard that flips a flag when the tracked task's future is dropped,
    /// which is what `JoinHandle::abort` does.
    struct DropSignal(Arc<AtomicBool>);
    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    /// `stop()` must clear and abort tracked long-lived tasks.
    #[tokio::test]
    async fn test_stop_aborts_and_clears_tracked_tasks() {
        let (proto, _handler) = test_protocol("10.0.0.1:8848");
        let aborted = Arc::new(AtomicBool::new(false));
        let signal = DropSignal(aborted.clone());
        let handle = tokio::spawn(async move {
            let _signal = signal;
            std::future::pending::<()>().await;
        });
        proto.track_task(handle);
        assert_eq!(proto.task_handles.lock().unwrap().len(), 1);

        proto.stop().await; // no pending sync_tasks -> returns immediately

        assert!(
            proto.task_handles.lock().unwrap().is_empty(),
            "stop() must clear tracked task handles"
        );
        // Aborting the task drops its future, which drops DropSignal.
        for _ in 0..40 {
            if aborted.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(
            aborted.load(Ordering::SeqCst),
            "stop() must abort long-lived tracked tasks"
        );
    }

    /// Wrapper that deliberately does NOT override `get_version`, so the
    /// trait's default implementation is exercised.
    struct DefaultVersionHandler(MockHandler);

    #[tonic::async_trait]
    impl DistroDataHandler for DefaultVersionHandler {
        fn data_type(&self) -> DistroDataType {
            self.0.data_type()
        }
        async fn get_all_keys(&self) -> Vec<String> {
            self.0.get_all_keys().await
        }
        async fn get_data(&self, key: &str) -> Option<DistroData> {
            self.0.get_data(key).await
        }
        async fn process_sync_data(&self, data: DistroData) -> Result<(), String> {
            self.0.process_sync_data(data).await
        }
        async fn process_verify_data(&self, data: &DistroData) -> Result<bool, String> {
            self.0.process_verify_data(data).await
        }
        async fn get_snapshot(&self) -> Vec<DistroData> {
            self.0.get_snapshot().await
        }
        async fn remove_data(&self, key: &str) -> Result<(), String> {
            self.0.remove_data(key).await
        }
    }

    fn seed_verify_fixture(mock: &MockHandler) {
        for (key, version) in [("k1", 3i64), ("k2", 7), ("k3", 11)] {
            mock.share().insert(
                key.to_string(),
                DistroData::with_version(
                    DistroDataType::NamingInstance,
                    key.to_string(),
                    Vec::new(),
                    "peer:8848".to_string(),
                    version,
                ),
            );
        }
    }

    /// Verify rounds must use the cheap `get_version` port. Mutant check:
    /// reverting `collect_verify_versions` to `get_data` builds a payload for
    /// every key and this counter assertion fails.
    #[tokio::test]
    async fn verify_round_collects_versions_without_building_payloads() {
        let mock = MockHandler::new();
        seed_verify_fixture(&mock);
        let calls = mock.get_data_calls.clone();
        let handler: Arc<dyn DistroDataHandler> = Arc::new(mock);

        let keys = handler.get_all_keys().await;
        let versions = collect_verify_versions(&handler, &keys).await;
        assert_eq!(versions.len(), 3);
        assert_eq!(versions.get("k1"), Some(&3));
        assert_eq!(versions.get("k2"), Some(&7));
        assert_eq!(versions.get("k3"), Some(&11));
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "a verify round must not call get_data on a handler with a cheap version port"
        );
    }

    /// Success path for the new default trait method: handlers that do not
    /// override `get_version` must still report exactly `get_data().version`,
    /// and missing keys must stay absent from the verify map.
    #[tokio::test]
    async fn default_get_version_matches_get_data_and_skips_missing_keys() {
        let mock = MockHandler::new();
        seed_verify_fixture(&mock);
        let calls = mock.get_data_calls.clone();
        let handler: Arc<dyn DistroDataHandler> = Arc::new(DefaultVersionHandler(mock));

        assert_eq!(handler.get_version("k2").await, Some(7));
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(handler.get_version("missing").await.is_none());

        let keys = vec!["k1".to_string(), "k2".to_string(), "missing".to_string()];
        let versions = collect_verify_versions(&handler, &keys).await;
        assert_eq!(versions.len(), 2);
        assert_eq!(versions.get("k2"), Some(&7));
    }
}
