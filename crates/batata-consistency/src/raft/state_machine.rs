// Raft state machine implementation
// Applies committed log entries to the application state using RocksDB

// Allow many arguments for Raft apply operations - parameters are logically coupled
#![allow(clippy::too_many_arguments)]
// Allow complex types for snapshot data structures
#![allow(clippy::type_complexity)]

use md5::Digest;
use std::hash::Hasher;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use openraft::storage::{RaftSnapshotBuilder, RaftStateMachine, Snapshot};
use openraft::{
    Entry, EntryPayload, ErrorSubject, ErrorVerb, LogId, OptionalSend, SnapshotMeta, StorageError,
    StoredMembership,
};
use rocksdb::{BlockBasedOptions, ColumnFamily, ColumnFamilyDescriptor, DB, Options};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::naming_hook::{SharedNamingHook, new_shared_naming_hook};
use super::plugin::{PluginRegistry, RaftPluginHandler};
use super::request::{RaftRequest, RaftResponse};
use super::types::{NodeId, TypeConfig};

/// Compute a checksum over data bytes using SipHash (stable, in std)
fn compute_checksum(data: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(data);
    hasher.finish()
}

/// Size of the checksum trailer appended to snapshot data (8 bytes for u64)
const SNAPSHOT_CHECKSUM_SIZE: usize = std::mem::size_of::<u64>();
/// Magic header for bincode-format snapshots (backward-compatible with legacy JSON)
const SNAPSHOT_MAGIC: &[u8] = b"BSNA";

/// Helper to create StorageError for state machine operations
fn sm_error(
    e: impl std::error::Error + Send + Sync + 'static,
    verb: ErrorVerb,
) -> StorageError<NodeId> {
    StorageError::from_io_error(
        ErrorSubject::StateMachine,
        verb,
        std::io::Error::other(e.to_string()),
    )
}

// Column family names for state machine
pub const CF_CONFIG: &str = "batata_config";
pub const CF_CONFIG_HISTORY: &str = "batata_config_history";
pub const CF_CONFIG_GRAY: &str = "batata_config_gray";
pub const CF_NAMESPACE: &str = "batata_namespace";
pub const CF_USERS: &str = "batata_users";
pub const CF_ROLES: &str = "batata_roles";
pub const CF_PERMISSIONS: &str = "batata_permissions";
pub const CF_INSTANCES: &str = "batata_instances";
pub const CF_LOCKS: &str = "batata_locks";
pub const CF_AI_RESOURCE: &str = "batata_ai_resource";
pub const CF_AI_RESOURCE_VERSION: &str = "batata_ai_resource_version";
pub const CF_PIPELINE_EXECUTION: &str = "batata_pipeline_execution";
const CF_META: &str = "batata_meta";

// Meta keys
const KEY_LAST_APPLIED: &[u8] = b"last_applied";
const KEY_LAST_MEMBERSHIP: &[u8] = b"last_membership";

/// RocksDB-based state machine for Raft
pub struct RocksStateMachine {
    db: Arc<DB>,
    /// Last applied log ID
    last_applied: RwLock<Option<LogId<NodeId>>>,
    /// Last membership configuration
    last_membership: RwLock<StoredMembership<NodeId, openraft::BasicNode>>,
    /// Registry of plugin handlers for PluginWrite operations.
    /// Wrapped in Arc so the registry can be shared with RaftNode for
    /// post-initialization plugin registration (after state_machine is moved into Raft).
    plugin_registry: Arc<RwLock<PluginRegistry>>,
    /// Apply-back hook for persistent naming instances. Registered after
    /// construction (see `RaftNode::register_naming_hook`). Invoked after
    /// `apply_instance_*` succeeds to keep `NamingService` DashMap in sync.
    naming_hook: SharedNamingHook,
    /// Pre-configured WriteOptions for state-machine writes (sync, WAL settings).
    write_opts: rocksdb::WriteOptions,
}

/// Serialize a JSON value directly to bytes, avoiding intermediate String allocation.
#[inline]
fn json_to_bytes(value: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

/// Typed persistent instance record stored in `CF_INSTANCES`.
///
/// Serialized with `bincode` instead of `serde_json` to avoid the
/// per-apply 1.5µs decode tax measured in `raft_serialization_bench`.
/// `metadata` stays as a pre-serialized JSON blob so this struct can be
/// emitted without a second pass over the user's key/value pairs — the
/// hook consumers parse the metadata JSON themselves if they need it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredInstance {
    pub namespace_id: String,
    pub group_name: String,
    pub service_name: String,
    pub instance_id: String,
    pub ip: String,
    pub port: u16,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub metadata: String,
    pub cluster_name: String,
    pub registered_time: i64,
    #[serde(default)]
    pub modified_time: i64,
}

/// Typed config record stored in `CF_CONFIG`.
///
/// Replaces the previous `serde_json::Value` dynamic dispatch which
/// dominated `apply_config_publish` CPU cost. Reader paths decode this
/// then re-emit as `serde_json::Value` for backward-compatible API.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredConfig {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    #[serde(default)]
    pub config_type: Option<String>,
    #[serde(default)]
    pub app_name: Option<String>,
    #[serde(default)]
    pub config_tags: Option<String>,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default, rename = "use")]
    pub r#use: Option<String>,
    #[serde(default)]
    pub effect: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub encrypted_data_key: Option<String>,
    #[serde(default)]
    pub src_user: Option<String>,
    #[serde(default)]
    pub src_ip: Option<String>,
    pub created_time: i64,
    pub modified_time: i64,
}

/// Typed config history record stored in `CF_CONFIG_HISTORY`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredConfigHistory {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    #[serde(default)]
    pub src_user: Option<String>,
    #[serde(default)]
    pub src_ip: Option<String>,
    pub op_type: String,
    pub publish_type: String,
    pub gray_name: String,
    pub ext_info: String,
    pub encrypted_data_key: String,
    pub created_time: i64,
    pub modified_time: i64,
}

/// Typed gray config record stored in `CF_CONFIG_GRAY`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredConfigGray {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub content: String,
    pub md5: String,
    pub app_name: String,
    pub gray_name: String,
    pub gray_rule: String,
    pub encrypted_data_key: String,
    pub src_user: String,
    pub src_ip: String,
    pub created_time: i64,
    pub modified_time: i64,
}

/// Typed value for CF_NAMESPACE entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredNamespace {
    pub namespace_id: String,
    pub namespace_name: String,
    #[serde(default)]
    pub namespace_desc: Option<String>,
    #[serde(default)]
    pub created_time: i64,
    #[serde(default)]
    pub modified_time: i64,
}

/// Typed value for CF_USERS entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredUser {
    pub username: String,
    pub password_hash: String,
    pub enabled: bool,
    #[serde(default)]
    pub created_time: i64,
    #[serde(default)]
    pub modified_time: i64,
}

/// Typed value for CF_ROLES entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredRole {
    pub role: String,
    pub username: String,
    #[serde(default)]
    pub created_time: i64,
}

/// Typed value for CF_PERMISSIONS entries (bincode-encoded).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StoredPermission {
    pub role: String,
    pub resource: String,
    pub action: String,
    #[serde(default)]
    pub created_time: i64,
}

impl RocksStateMachine {
    /// Get a reference to the underlying RocksDB instance
    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    /// Get a shared reference to the plugin registry.
    ///
    /// This allows `RaftNode` to hold a handle to the registry so plugins
    /// can be registered after the state machine has been moved into OpenRaft.
    pub fn plugin_registry(&self) -> Arc<RwLock<PluginRegistry>> {
        self.plugin_registry.clone()
    }

    /// Get a shared handle to the naming apply hook slot.
    ///
    /// Registration happens after both `RaftNode` and `NamingService` are
    /// constructed at server startup. Mirrors `plugin_registry` pattern.
    pub fn naming_hook(&self) -> SharedNamingHook {
        self.naming_hook.clone()
    }


    /// Register a plugin handler for processing PluginWrite operations.
    ///
    /// The plugin's column families must already exist in RocksDB — pass them
    /// via `extra_cf_names` when creating the state machine, or via
    /// `PluginContext::get("extra_cf_names")` before Raft startup.
    pub async fn register_plugin(&self, handler: Arc<dyn RaftPluginHandler>) -> Result<(), String> {
        // Verify all required CFs exist
        for cf_name in handler.column_families() {
            if self.db.cf_handle(&cf_name).is_none() {
                return Err(format!(
                    "Column family '{}' required by plugin '{}' does not exist in RocksDB. \
                     Pass it via extra_cf_names when creating the state machine.",
                    cf_name,
                    handler.plugin_id()
                ));
            }
        }
        self.plugin_registry.write().await.register(handler)
    }

    /// Create a new RocksDB state machine with default configuration
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError<NodeId>> {
        Self::with_options(path, None, None).await
    }

    /// Create a new RocksDB state machine with pre-configured RocksDB options.
    ///
    /// Pass `None` for either parameter to use sensible defaults.
    /// This allows callers to customize RocksDB tuning via application configuration
    /// without introducing a dependency on the configuration crate.
    pub async fn with_options<P: AsRef<Path>>(
        path: P,
        custom_db_opts: Option<Options>,
        custom_cf_opts: Option<Options>,
    ) -> Result<Self, StorageError<NodeId>> {
        Self::with_options_and_cfs(path, custom_db_opts, custom_cf_opts, &[]).await
    }

    /// Create a new RocksDB state machine with custom options and extra column families.
    ///
    /// `extra_cf_names` are additional column families (e.g., from plugins) that
    /// will be created alongside the core CFs. The same `cf_opts` are applied to all.
    /// `custom_history_cf_opts` allows separate tuning for history CF (append-only workload).
    /// `custom_write_opts` configures sync/WAL behavior for state-machine writes.
    pub async fn with_options_and_cfs<P: AsRef<Path>>(
        path: P,
        custom_db_opts: Option<Options>,
        custom_cf_opts: Option<Options>,
        extra_cf_names: &[String],
    ) -> Result<Self, StorageError<NodeId>> {
        Self::with_full_options(
            path,
            custom_db_opts,
            custom_cf_opts,
            None,
            None,
            extra_cf_names,
        )
        .await
    }

    /// Create a new RocksDB state machine with full customization including
    /// separate history CF options and WriteOptions.
    pub async fn with_full_options<P: AsRef<Path>>(
        path: P,
        custom_db_opts: Option<Options>,
        custom_cf_opts: Option<Options>,
        custom_history_cf_opts: Option<Options>,
        custom_write_opts: Option<rocksdb::WriteOptions>,
        extra_cf_names: &[String],
    ) -> Result<Self, StorageError<NodeId>> {
        let db_opts = custom_db_opts.unwrap_or_else(|| {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);
            opts.set_write_buffer_size(128 * 1024 * 1024);
            opts.set_max_write_buffer_number(4);
            opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
            opts.set_bottommost_compression_type(rocksdb::DBCompressionType::Zstd);
            opts.set_level_compaction_dynamic_level_bytes(true);
            opts.increase_parallelism(std::cmp::max(
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4) as i32
                    / 2,
                2,
            ));
            opts.set_max_background_jobs(4);
            opts
        });

        let cf_opts = custom_cf_opts.unwrap_or_else(|| {
            let mut opts = Options::default();
            opts.set_write_buffer_size(128 * 1024 * 1024);
            opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
            opts.set_bottommost_compression_type(rocksdb::DBCompressionType::Zstd);
            let mut block_opts = BlockBasedOptions::default();
            let cache = rocksdb::Cache::new_lru_cache(256 * 1024 * 1024);
            block_opts.set_block_cache(&cache);
            block_opts.set_bloom_filter(10.0, false);
            opts.set_block_based_table_factory(&block_opts);
            opts
        });

        let history_cf_opts = custom_history_cf_opts.unwrap_or_else(|| cf_opts.clone());

        let mut cfs = vec![
            ColumnFamilyDescriptor::new(CF_CONFIG, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONFIG_HISTORY, history_cf_opts),
            ColumnFamilyDescriptor::new(CF_CONFIG_GRAY, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_NAMESPACE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_USERS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_ROLES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_PERMISSIONS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_INSTANCES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_LOCKS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_AI_RESOURCE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_AI_RESOURCE_VERSION, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_PIPELINE_EXECUTION, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_META, cf_opts.clone()),
        ];

        // Add plugin column families
        for cf_name in extra_cf_names {
            cfs.push(ColumnFamilyDescriptor::new(cf_name, cf_opts.clone()));
        }

        let db = DB::open_cf_descriptors(&db_opts, path, cfs)
            .map_err(|e| sm_error(e, ErrorVerb::Read))?;

        let sm = Self {
            db: Arc::new(db),
            last_applied: RwLock::new(None),
            last_membership: RwLock::new(StoredMembership::default()),
            plugin_registry: Arc::new(RwLock::new(PluginRegistry::new())),
            naming_hook: new_shared_naming_hook(),
            write_opts: custom_write_opts.unwrap_or_default(),
        };

        // Load cached values asynchronously
        sm.load_cached_values().await?;

        info!("RocksDB state machine initialized");
        Ok(sm)
    }

    /// Load cached values from storage.
    async fn load_cached_values(&self) -> Result<(), StorageError<NodeId>> {
        // Load last applied
        if let Some(bytes) = self
            .db
            .get_cf(self.cf_meta(), KEY_LAST_APPLIED)
            .map_err(|e| sm_error(e, ErrorVerb::Read))?
        {
            let log_id: LogId<NodeId> =
                bincode::deserialize(&bytes).map_err(|e| sm_error(e, ErrorVerb::Read))?;
            *self.last_applied.write().await = Some(log_id);
        }

        // Load last membership
        if let Some(bytes) = self
            .db
            .get_cf(self.cf_meta(), KEY_LAST_MEMBERSHIP)
            .map_err(|e| sm_error(e, ErrorVerb::Read))?
        {
            let membership: StoredMembership<NodeId, openraft::BasicNode> =
                bincode::deserialize(&bytes).map_err(|e| sm_error(e, ErrorVerb::Read))?;
            *self.last_membership.write().await = membership;
        }

        Ok(())
    }

    /// Get column family handles
    /// These expect() calls are acceptable because:
    /// 1. Column families are created during DB initialization
    /// 2. If they don't exist, it indicates severe DB corruption that cannot be recovered
    /// 3. The application cannot function without these column families
    fn cf_config(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONFIG)
            .expect("CF_CONFIG must exist - database may be corrupted")
    }

    fn cf_namespace(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_NAMESPACE)
            .expect("CF_NAMESPACE must exist - database may be corrupted")
    }

    fn cf_users(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_USERS)
            .expect("CF_USERS must exist - database may be corrupted")
    }

    fn cf_roles(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_ROLES)
            .expect("CF_ROLES must exist - database may be corrupted")
    }

    fn cf_permissions(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_PERMISSIONS)
            .expect("CF_PERMISSIONS must exist - database may be corrupted")
    }

    fn cf_instances(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_INSTANCES)
            .expect("CF_INSTANCES must exist - database may be corrupted")
    }

    fn cf_locks(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_LOCKS)
            .expect("CF_LOCKS must exist - database may be corrupted")
    }

    fn cf_config_history(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONFIG_HISTORY)
            .expect("CF_CONFIG_HISTORY must exist - database may be corrupted")
    }

    #[allow(dead_code)]
    fn cf_config_gray(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONFIG_GRAY)
            .expect("CF_CONFIG_GRAY must exist - database may be corrupted")
    }

    fn cf_meta(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_META)
            .expect("CF_META must exist - database may be corrupted")
    }

    /// Generate config key
    pub fn config_key(data_id: &str, group: &str, tenant: &str) -> String {
        format!("{}@@{}@@{}", tenant, group, data_id)
    }

    /// Generate config history key
    pub fn config_history_key(data_id: &str, group: &str, tenant: &str, id: u64) -> String {
        format!("{}@@{}@@{}@@{}", tenant, group, data_id, id)
    }

    /// Generate config gray key
    pub fn config_gray_key(data_id: &str, group: &str, tenant: &str, gray_name: &str) -> String {
        format!("{}@@{}@@{}@@{}", tenant, group, data_id, gray_name)
    }

    /// Generate namespace key
    pub fn namespace_key(namespace_id: &str) -> String {
        namespace_id.to_string()
    }

    /// Generate user key
    pub fn user_key(username: &str) -> String {
        username.to_string()
    }

    /// Generate role key
    pub fn role_key(role: &str, username: &str) -> String {
        format!("{}@@{}", role, username)
    }

    /// Generate permission key
    pub fn permission_key(role: &str, resource: &str, action: &str) -> String {
        format!("{}@@{}@@{}", role, resource, action)
    }

    /// Generate instance key
    pub fn instance_key(
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
    ) -> String {
        format!(
            "{}@@{}@@{}@@{}",
            namespace_id, group_name, service_name, instance_id
        )
    }

    /// Generate lock key
    pub fn lock_key(namespace: &str, name: &str) -> String {
        format!("{}::{}", namespace, name)
    }

    /// Apply a single request to the state machine
    async fn apply_request(&self, request: RaftRequest, log_index: u64) -> RaftResponse {
        match request {
            RaftRequest::ConfigPublish(payload) => {
                let crate::raft::request::ConfigPublishPayload {
                    data_id,
                    group,
                    tenant,
                    content,
                    md5,
                    config_type,
                    app_name,
                    tag,
                    desc,
                    src_user,
                    src_ip,
                    r#use,
                    effect,
                    schema,
                    encrypted_data_key,
                    cas_md5,
                    history,
                } = *payload;
                self.apply_config_publish_batched(
                    &data_id,
                    &group,
                    &tenant,
                    &content,
                    &md5,
                    config_type,
                    app_name,
                    tag,
                    desc,
                    src_user,
                    src_ip,
                    r#use,
                    effect,
                    schema,
                    encrypted_data_key,
                    cas_md5.as_deref(),
                    history,
                )
            }

            RaftRequest::ConfigRemove {
                data_id,
                group,
                tenant,
                history,
            } => self.apply_config_remove_batched(&data_id, &group, &tenant, history),

            RaftRequest::ConfigGrayPublish(payload) => {
                let crate::raft::request::ConfigGrayPublishPayload {
                    data_id,
                    group,
                    tenant,
                    content,
                    gray_name,
                    gray_rule,
                    app_name,
                    encrypted_data_key,
                    src_user,
                    src_ip,
                    cas_md5,
                } = *payload;
                self.apply_config_gray_publish(
                    &data_id,
                    &group,
                    &tenant,
                    &content,
                    &gray_name,
                    &gray_rule,
                    app_name,
                    encrypted_data_key,
                    src_user,
                    src_ip,
                    cas_md5,
                )
            }

            RaftRequest::ConfigGrayRemove {
                data_id,
                group,
                tenant,
                gray_name,
            } => self.apply_config_gray_remove(&data_id, &group, &tenant, &gray_name),

            RaftRequest::NamespaceCreate {
                namespace_id,
                namespace_name,
                namespace_desc,
            } => self.apply_namespace_create(&namespace_id, &namespace_name, namespace_desc),

            RaftRequest::NamespaceUpdate {
                namespace_id,
                namespace_name,
                namespace_desc,
            } => self.apply_namespace_update(&namespace_id, &namespace_name, namespace_desc),

            RaftRequest::NamespaceDelete { namespace_id } => {
                self.apply_namespace_delete(&namespace_id)
            }

            RaftRequest::UserCreate {
                username,
                password_hash,
                enabled,
            } => self.apply_user_create(&username, &password_hash, enabled),

            RaftRequest::UserUpdate {
                username,
                password_hash,
                enabled,
            } => self.apply_user_update(&username, password_hash, enabled),

            RaftRequest::UserDelete { username } => self.apply_user_delete(&username),

            RaftRequest::RoleCreate { role, username } => self.apply_role_create(&role, &username),

            RaftRequest::RoleDelete { role, username } => self.apply_role_delete(&role, &username),

            RaftRequest::PermissionGrant {
                role,
                resource,
                action,
            } => self.apply_permission_grant(&role, &resource, &action),

            RaftRequest::PermissionRevoke {
                role,
                resource,
                action,
            } => self.apply_permission_revoke(&role, &resource, &action),

            RaftRequest::ConfigHistoryInsert(payload) => {
                let crate::raft::request::ConfigHistoryInsertPayload {
                    id,
                    data_id,
                    group,
                    tenant,
                    content,
                    md5,
                    app_name,
                    src_user,
                    src_ip,
                    op_type,
                    publish_type,
                    gray_name,
                    ext_info,
                    encrypted_data_key,
                    created_time,
                    last_modified_time,
                } = *payload;
                self.apply_config_history_insert(
                    id,
                    &data_id,
                    &group,
                    &tenant,
                    &content,
                    &md5,
                    app_name,
                    src_user,
                    src_ip,
                    &op_type,
                    publish_type,
                    gray_name,
                    ext_info,
                    encrypted_data_key,
                    created_time,
                    last_modified_time,
                )
            }

            RaftRequest::ConfigTagsUpdate {
                data_id,
                group,
                tenant,
                tag,
                ..
            } => self.apply_config_tags_update(&data_id, &group, &tenant, &tag),

            RaftRequest::ConfigTagsDelete {
                data_id,
                group,
                tenant,
                tag,
            } => self.apply_config_tags_delete(&data_id, &group, &tenant, &tag),

            RaftRequest::PersistentInstanceRegister(payload) => {
                let crate::raft::request::PersistentInstanceRegisterPayload {
                    namespace_id,
                    group_name,
                    service_name,
                    instance_id,
                    ip,
                    port,
                    weight,
                    healthy,
                    enabled,
                    metadata,
                    cluster_name,
                } = *payload;
                let resp = self.apply_instance_register(
                    &namespace_id,
                    &group_name,
                    &service_name,
                    &instance_id,
                    &ip,
                    port,
                    weight,
                    healthy,
                    enabled,
                    &metadata,
                    &cluster_name,
                );
                if resp.success {
                    if let Some(hook) = self.naming_hook.read().await.as_ref() {
                        hook.on_register(
                            &namespace_id,
                            &group_name,
                            &service_name,
                            &instance_id,
                            &ip,
                            port,
                            weight,
                            healthy,
                            enabled,
                            &metadata,
                            &cluster_name,
                        );
                    }
                }
                resp
            }

            RaftRequest::PersistentInstanceDeregister {
                namespace_id,
                group_name,
                service_name,
                instance_id,
            } => {
                let resp = self.apply_instance_deregister(
                    &namespace_id,
                    &group_name,
                    &service_name,
                    &instance_id,
                );
                if resp.success {
                    if let Some(hook) = self.naming_hook.read().await.as_ref() {
                        hook.on_deregister(
                            &namespace_id,
                            &group_name,
                            &service_name,
                            &instance_id,
                        );
                    }
                }
                resp
            }

            RaftRequest::PersistentInstanceUpdate(payload) => {
                let crate::raft::request::PersistentInstanceUpdatePayload {
                    namespace_id,
                    group_name,
                    service_name,
                    instance_id,
                    ip,
                    port,
                    weight,
                    healthy,
                    enabled,
                    metadata,
                } = *payload;
                let ip_c = ip.clone();
                let metadata_c = metadata.clone();
                let resp = self.apply_instance_update(
                    &namespace_id,
                    &group_name,
                    &service_name,
                    &instance_id,
                    ip,
                    port,
                    weight,
                    healthy,
                    enabled,
                    metadata,
                );
                if resp.success {
                    if let Some(hook) = self.naming_hook.read().await.as_ref() {
                        hook.on_update(
                            &namespace_id,
                            &group_name,
                            &service_name,
                            &instance_id,
                            ip_c.as_deref(),
                            port,
                            weight,
                            healthy,
                            enabled,
                            metadata_c.as_deref(),
                        );
                    }
                }
                resp
            }

            RaftRequest::Noop => RaftResponse::success(),

            // Lock operations (ADV-005)
            RaftRequest::LockAcquire {
                namespace,
                name,
                owner,
                ttl_ms,
                fence_token,
                owner_metadata,
            } => self.apply_lock_acquire(
                &namespace,
                &name,
                &owner,
                ttl_ms,
                fence_token,
                owner_metadata,
            ),

            RaftRequest::LockRelease {
                namespace,
                name,
                owner,
                fence_token,
            } => self.apply_lock_release(&namespace, &name, &owner, fence_token),

            RaftRequest::LockRenew {
                namespace,
                name,
                owner,
                ttl_ms,
            } => self.apply_lock_renew(&namespace, &name, &owner, ttl_ms),

            RaftRequest::LockForceRelease { namespace, name } => {
                self.apply_lock_force_release(&namespace, &name)
            }

            RaftRequest::LockExpire { namespace, name } => {
                self.apply_lock_expire(&namespace, &name)
            }

            RaftRequest::PluginWrite {
                plugin_id,
                op_type,
                payload,
            } => {
                let registry = self.plugin_registry.read().await;
                if let Some(handler) = registry.get(&plugin_id) {
                    handler.apply(&self.db, &op_type, &payload, log_index)
                } else {
                    warn!(
                        plugin_id = %plugin_id,
                        op_type = %op_type,
                        "No plugin handler registered — PluginWrite ignored"
                    );
                    RaftResponse::success()
                }
            }
        }
    }

    // Config operations

    /// Publish config and optionally insert history in a single atomic WriteBatch.
    /// This replaces the previous two-step approach (apply_config_publish + apply_config_history_insert)
    /// with a single RocksDB WriteBatch, reducing from 2 fsyncs to 1 and ensuring atomicity.
    /// The op_type (Insert/Update) is determined here from the existing value, removing the
    /// need for a pre-write read in the distributed persistence layer.
    #[allow(clippy::too_many_arguments)]
    fn apply_config_publish_batched(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        md5: &str,
        config_type: Option<String>,
        app_name: Option<String>,
        tag: Option<String>,
        desc: Option<String>,
        src_user: Option<String>,
        src_ip: Option<String>,
        r#use: Option<String>,
        effect: Option<String>,
        schema: Option<String>,
        encrypted_data_key: Option<String>,
        cas_md5: Option<&str>,
        history: Option<super::request::ConfigHistoryInfo>,
    ) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);
        let now = chrono::Utc::now().timestamp_millis();

        // Single read: fetch existing StoredConfig for CAS check, created_time
        // preservation, and op_type determination (Insert vs Update).
        let existing_stored: Option<StoredConfig> =
            match self.db.get_cf(self.cf_config(), key.as_bytes()) {
                Ok(Some(bytes)) => bincode::deserialize(&bytes).ok(),
                Ok(None) => None,
                Err(e) => {
                    if cas_md5.is_some() {
                        return RaftResponse::failure(format!("CAS check failed: {}", e));
                    }
                    None
                }
            };

        // CAS (Compare-And-Swap) check
        if let Some(expected_md5) = cas_md5 {
            match &existing_stored {
                Some(existing) => {
                    if existing.md5 != expected_md5 {
                        return RaftResponse::failure(format!(
                            "CAS conflict: expected md5={}, actual md5={}",
                            expected_md5, existing.md5
                        ));
                    }
                }
                None => {
                    if !expected_md5.is_empty() {
                        return RaftResponse::failure(
                            "CAS conflict: config does not exist".to_string(),
                        );
                    }
                }
            }
        }

        // Determine op_type from existing value (eliminates pre-write read in distributed layer)
        let is_update = existing_stored.is_some();

        // Preserve created_time from existing config on update
        let created_time = existing_stored
            .as_ref()
            .map(|c| c.created_time)
            .unwrap_or(now);

        let stored = StoredConfig {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            content: content.to_string(),
            md5: md5.to_string(),
            config_type,
            app_name: app_name.clone(),
            config_tags: tag,
            desc,
            r#use,
            effect,
            schema,
            encrypted_data_key: encrypted_data_key.clone(),
            src_user: src_user.clone(),
            src_ip: src_ip.clone(),
            created_time,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };

        // Build a single WriteBatch for config + history (1 fsync instead of 2)
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(self.cf_config(), key.as_bytes(), encoded);

        // If history info is provided, add history entry to the same batch
        if let Some(hi) = history {
            let op_type = if hi.op_type.is_empty() {
                if is_update { "U" } else { "I" }.to_string()
            } else {
                hi.op_type
            };
            let history_key = Self::config_history_key(data_id, group, tenant, now as u64);
            let history = StoredConfigHistory {
                id: now,
                data_id: data_id.to_string(),
                group: group.to_string(),
                tenant: tenant.to_string(),
                content: content.to_string(),
                md5: md5.to_string(),
                app_name: app_name.clone().unwrap_or_default(),
                src_user: src_user.clone(),
                src_ip: src_ip.clone(),
                op_type,
                publish_type: hi.publish_type.unwrap_or_else(|| "formal".to_string()),
                gray_name: String::new(),
                ext_info: hi.ext_info.unwrap_or_default(),
                encrypted_data_key: encrypted_data_key.clone().unwrap_or_default(),
                created_time: now,
                modified_time: now,
            };
            let history_encoded = match bincode::serialize(&history) {
                Ok(b) => b,
                Err(e) => return RaftResponse::failure(format!("history encode error: {}", e)),
            };
            batch.put_cf(
                self.cf_config_history(),
                history_key.as_bytes(),
                history_encoded,
            );
        }

        match self.db.write_opt(batch, &self.write_opts) {
            Ok(_) => {
                debug!("Config published: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to publish config: {}", e);
                RaftResponse::failure(format!("Failed to publish config: {}", e))
            }
        }
    }

    /// Remove config and optionally insert delete history in a single atomic WriteBatch.
    fn apply_config_remove_batched(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        history: Option<Box<super::request::ConfigDeleteHistoryInfo>>,
    ) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);
        let now = chrono::Utc::now().timestamp_millis();

        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(self.cf_config(), key.as_bytes());

        if let Some(hi) = history.map(|b| *b) {
            let history_key = Self::config_history_key(data_id, group, tenant, now as u64);
            let history = StoredConfigHistory {
                id: now,
                data_id: data_id.to_string(),
                group: group.to_string(),
                tenant: tenant.to_string(),
                content: hi.content,
                md5: hi.md5,
                app_name: hi.app_name,
                src_user: Some(hi.src_user),
                src_ip: Some(hi.src_ip),
                op_type: "D".to_string(),
                publish_type: "formal".to_string(),
                gray_name: String::new(),
                ext_info: hi.ext_info,
                encrypted_data_key: hi.encrypted_data_key,
                created_time: now,
                modified_time: now,
            };
            let encoded = match bincode::serialize(&history) {
                Ok(b) => b,
                Err(e) => return RaftResponse::failure(format!("history encode error: {}", e)),
            };
            batch.put_cf(
                self.cf_config_history(),
                history_key.as_bytes(),
                encoded,
            );
        }

        match self.db.write_opt(batch, &self.write_opts) {
            Ok(_) => {
                debug!("Config removed: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to remove config: {}", e);
                RaftResponse::failure(format!("Failed to remove config: {}", e))
            }
        }
    }

    // Gray config operations
    #[allow(clippy::too_many_arguments)]
    fn apply_config_gray_publish(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        gray_name: &str,
        gray_rule: &str,
        app_name: Option<String>,
        encrypted_data_key: Option<String>,
        src_user: Option<String>,
        src_ip: Option<String>,
        cas_md5: Option<String>,
    ) -> RaftResponse {
        let key = Self::config_gray_key(data_id, group, tenant, gray_name);
        let now = chrono::Utc::now().timestamp_millis();
        let md5_val = const_hex::encode(md5::Md5::digest(content.as_bytes()));

        // CAS check: if cas_md5 provided, verify current gray MD5 matches
        if let Some(ref expected_md5) = cas_md5 {
            let cf = self.cf_config_gray();
            match self.db.get_cf(cf, key.as_bytes()) {
                Ok(Some(bytes)) => {
                    if let Ok(existing) = bincode::deserialize::<StoredConfigGray>(&bytes) {
                        let current_md5 = existing.md5.as_str();
                        if current_md5 != expected_md5.as_str() {
                            return RaftResponse::failure(format!(
                                "CAS conflict: expected md5={}, actual md5={}",
                                expected_md5, current_md5
                            ));
                        }
                    }
                }
                Ok(None) => {
                    if !expected_md5.is_empty() {
                        return RaftResponse::failure(
                            "CAS conflict: gray config does not exist".to_string(),
                        );
                    }
                }
                Err(e) => {
                    return RaftResponse::failure(format!("CAS check failed: {}", e));
                }
            }
        }

        let stored = StoredConfigGray {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            content: content.to_string(),
            md5: md5_val,
            app_name: app_name.unwrap_or_default(),
            gray_name: gray_name.to_string(),
            gray_rule: gray_rule.to_string(),
            encrypted_data_key: encrypted_data_key.unwrap_or_default(),
            src_user: src_user.unwrap_or_default(),
            src_ip: src_ip.unwrap_or_default(),
            created_time: now,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };

        match self.db.put_cf_opt(
            self.cf_config_gray(),
            key.as_bytes(),
            encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Gray config published: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to publish gray config: {}", e);
                RaftResponse::failure(format!("Failed to publish gray config: {}", e))
            }
        }
    }

    fn apply_config_gray_remove(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        gray_name: &str,
    ) -> RaftResponse {
        let cf = self.cf_config_gray();

        // If gray_name is specified, delete only that specific gray config
        if !gray_name.is_empty() {
            let key = Self::config_gray_key(data_id, group, tenant, gray_name);
            match self.db.delete_cf_opt(cf, key.as_bytes(), &self.write_opts) {
                Ok(_) => {
                    debug!("Gray config removed: {}", key);
                    return RaftResponse::success();
                }
                Err(e) => {
                    error!("Failed to remove gray config: {}", e);
                    return RaftResponse::failure(format!("Failed to remove gray config: {}", e));
                }
            }
        }

        // Scan and delete all gray configs for this data_id/group/tenant
        let prefix = format!("{}@@{}@@{}@@", tenant, group, data_id);

        let mut keys_to_delete = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, _)) => {
                    let key_str = String::from_utf8_lossy(&key);
                    if !key_str.starts_with(&prefix) {
                        break;
                    }
                    keys_to_delete.push(key.to_vec());
                }
                Err(e) => {
                    error!("Failed to iterate gray configs: {}", e);
                    return RaftResponse::failure(format!("Failed to iterate gray configs: {}", e));
                }
            }
        }

        if keys_to_delete.is_empty() {
            return RaftResponse::success();
        }

        let mut batch = rocksdb::WriteBatch::default();
        for key in &keys_to_delete {
            batch.delete_cf(cf, key);
        }

        match self.db.write_opt(batch, &self.write_opts) {
            Ok(_) => {
                debug!(
                    "Gray configs removed for {}/{}/{}: {} entries",
                    tenant,
                    group,
                    data_id,
                    keys_to_delete.len()
                );
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to remove gray configs: {}", e);
                RaftResponse::failure(format!("Failed to remove gray configs: {}", e))
            }
        }
    }

    // Config history operations
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn apply_config_history_insert(
        &self,
        id: i64,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        md5: &str,
        app_name: Option<String>,
        src_user: Option<String>,
        src_ip: Option<String>,
        op_type: &str,
        publish_type: Option<String>,
        gray_name: Option<String>,
        ext_info: Option<String>,
        encrypted_data_key: Option<String>,
        created_time: i64,
        last_modified_time: i64,
    ) -> RaftResponse {
        let key = Self::config_history_key(data_id, group, tenant, id as u64);
        let stored = StoredConfigHistory {
            id,
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            content: content.to_string(),
            md5: md5.to_string(),
            app_name: app_name.unwrap_or_default(),
            src_user,
            src_ip,
            op_type: op_type.to_string(),
            publish_type: publish_type.unwrap_or_else(|| "formal".to_string()),
            gray_name: gray_name.unwrap_or_default(),
            ext_info: ext_info.unwrap_or_default(),
            encrypted_data_key: encrypted_data_key.unwrap_or_default(),
            created_time,
            modified_time: last_modified_time,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };

        match self.db.put_cf_opt(
            self.cf_config_history(),
            key.as_bytes(),
            encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Config history inserted: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to insert config history: {}", e);
                RaftResponse::failure(format!("Failed to insert config history: {}", e))
            }
        }
    }

    // Config tags operations - tags are stored as comma-separated in the config entry
    fn apply_config_tags_update(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        tag: &str,
    ) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);

        let mut stored: StoredConfig = match self.db.get_cf(self.cf_config(), key.as_bytes()) {
            Ok(Some(bytes)) => match bincode::deserialize(&bytes) {
                Ok(s) => s,
                Err(e) => return RaftResponse::failure(format!("decode error: {}", e)),
            },
            _ => return RaftResponse::failure("Config not found for tag update"),
        };
        stored.config_tags = Some(tag.to_string());
        stored.modified_time = chrono::Utc::now().timestamp_millis();

        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };
        match self.db.put_cf_opt(self.cf_config(), key.as_bytes(), encoded, &self.write_opts) {
            Ok(_) => {
                debug!("Config tags updated: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to update config tags: {}", e);
                RaftResponse::failure(format!("Failed to update config tags: {}", e))
            }
        }
    }

    fn apply_config_tags_delete(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        _tag: &str,
    ) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);

        let mut stored: StoredConfig = match self.db.get_cf(self.cf_config(), key.as_bytes()) {
            Ok(Some(bytes)) => match bincode::deserialize(&bytes) {
                Ok(s) => s,
                Err(e) => return RaftResponse::failure(format!("decode error: {}", e)),
            },
            _ => return RaftResponse::failure("Config not found for tag delete"),
        };
        stored.config_tags = Some(String::new());
        stored.modified_time = chrono::Utc::now().timestamp_millis();

        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };
        match self.db.put_cf_opt(self.cf_config(), key.as_bytes(), encoded, &self.write_opts) {
            Ok(_) => {
                debug!("Config tags deleted: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to delete config tags: {}", e);
                RaftResponse::failure(format!("Failed to delete config tags: {}", e))
            }
        }
    }

    // Namespace operations
    fn apply_namespace_create(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: Option<String>,
    ) -> RaftResponse {
        let key = Self::namespace_key(namespace_id);
        let now = chrono::Utc::now().timestamp_millis();
        let stored = StoredNamespace {
            namespace_id: namespace_id.to_string(),
            namespace_name: namespace_name.to_string(),
            namespace_desc,
            created_time: now,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => {
                return RaftResponse::failure(format!("Failed to encode namespace: {}", e));
            }
        };
        match self.db.put_cf_opt(
            self.cf_namespace(),
            key.as_bytes(),
            &encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Namespace created: {}", namespace_id);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to create namespace: {}", e);
                RaftResponse::failure(format!("Failed to create namespace: {}", e))
            }
        }
    }

    fn apply_namespace_update(
        &self,
        namespace_id: &str,
        namespace_name: &str,
        namespace_desc: Option<String>,
    ) -> RaftResponse {
        let key = Self::namespace_key(namespace_id);
        let now = chrono::Utc::now().timestamp_millis();

        // Preserve created_time from existing entry if present.
        let created_time = match self.db.get_cf(self.cf_namespace(), key.as_bytes()) {
            Ok(Some(bytes)) => bincode::deserialize::<StoredNamespace>(&bytes)
                .map(|s| s.created_time)
                .unwrap_or(now),
            _ => now,
        };

        let stored = StoredNamespace {
            namespace_id: namespace_id.to_string(),
            namespace_name: namespace_name.to_string(),
            namespace_desc,
            created_time,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => {
                return RaftResponse::failure(format!("Failed to encode namespace: {}", e));
            }
        };
        match self.db.put_cf_opt(
            self.cf_namespace(),
            key.as_bytes(),
            &encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Namespace updated: {}", namespace_id);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to update namespace: {}", e);
                RaftResponse::failure(format!("Failed to update namespace: {}", e))
            }
        }
    }

    fn apply_namespace_delete(&self, namespace_id: &str) -> RaftResponse {
        let key = Self::namespace_key(namespace_id);

        match self
            .db
            .delete_cf_opt(self.cf_namespace(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("Namespace deleted: {}", namespace_id);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to delete namespace: {}", e);
                RaftResponse::failure(format!("Failed to delete namespace: {}", e))
            }
        }
    }

    // User operations
    fn apply_user_create(
        &self,
        username: &str,
        password_hash: &str,
        enabled: bool,
    ) -> RaftResponse {
        let key = Self::user_key(username);
        let now = chrono::Utc::now().timestamp_millis();
        let stored = StoredUser {
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            enabled,
            created_time: now,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("Failed to encode user: {}", e)),
        };
        match self.db.put_cf_opt(
            self.cf_users(),
            key.as_bytes(),
            &encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("User created: {}", username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to create user: {}", e);
                RaftResponse::failure(format!("Failed to create user: {}", e))
            }
        }
    }

    fn apply_user_update(
        &self,
        username: &str,
        password_hash: Option<String>,
        enabled: Option<bool>,
    ) -> RaftResponse {
        let key = Self::user_key(username);

        let mut stored: StoredUser = match self.db.get_cf(self.cf_users(), key.as_bytes()) {
            Ok(Some(bytes)) => match bincode::deserialize(&bytes) {
                Ok(s) => s,
                Err(e) => {
                    return RaftResponse::failure(format!("Failed to decode user: {}", e));
                }
            },
            _ => return RaftResponse::failure("User not found"),
        };

        if let Some(hash) = password_hash {
            stored.password_hash = hash;
        }
        if let Some(en) = enabled {
            stored.enabled = en;
        }
        stored.modified_time = chrono::Utc::now().timestamp_millis();

        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("Failed to encode user: {}", e)),
        };
        match self.db.put_cf_opt(
            self.cf_users(),
            key.as_bytes(),
            &encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("User updated: {}", username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to update user: {}", e);
                RaftResponse::failure(format!("Failed to update user: {}", e))
            }
        }
    }

    fn apply_user_delete(&self, username: &str) -> RaftResponse {
        let key = Self::user_key(username);

        match self
            .db
            .delete_cf_opt(self.cf_users(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("User deleted: {}", username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to delete user: {}", e);
                RaftResponse::failure(format!("Failed to delete user: {}", e))
            }
        }
    }

    // Role operations
    fn apply_role_create(&self, role: &str, username: &str) -> RaftResponse {
        let key = Self::role_key(role, username);
        let stored = StoredRole {
            role: role.to_string(),
            username: username.to_string(),
            created_time: chrono::Utc::now().timestamp_millis(),
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("Failed to encode role: {}", e)),
        };
        match self.db.put_cf_opt(
            self.cf_roles(),
            key.as_bytes(),
            &encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Role assigned: {} -> {}", role, username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to assign role: {}", e);
                RaftResponse::failure(format!("Failed to assign role: {}", e))
            }
        }
    }

    fn apply_role_delete(&self, role: &str, username: &str) -> RaftResponse {
        let key = Self::role_key(role, username);

        match self
            .db
            .delete_cf_opt(self.cf_roles(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("Role removed: {} -> {}", role, username);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to remove role: {}", e);
                RaftResponse::failure(format!("Failed to remove role: {}", e))
            }
        }
    }

    // Permission operations
    fn apply_permission_grant(&self, role: &str, resource: &str, action: &str) -> RaftResponse {
        let key = Self::permission_key(role, resource, action);
        let stored = StoredPermission {
            role: role.to_string(),
            resource: resource.to_string(),
            action: action.to_string(),
            created_time: chrono::Utc::now().timestamp_millis(),
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("Failed to encode permission: {}", e)),
        };
        match self.db.put_cf_opt(
            self.cf_permissions(),
            key.as_bytes(),
            &encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Permission granted: {} -> {}:{}", role, resource, action);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to grant permission: {}", e);
                RaftResponse::failure(format!("Failed to grant permission: {}", e))
            }
        }
    }

    fn apply_permission_revoke(&self, role: &str, resource: &str, action: &str) -> RaftResponse {
        let key = Self::permission_key(role, resource, action);

        match self
            .db
            .delete_cf_opt(self.cf_permissions(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("Permission revoked: {} -> {}:{}", role, resource, action);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to revoke permission: {}", e);
                RaftResponse::failure(format!("Failed to revoke permission: {}", e))
            }
        }
    }

    // Instance operations
    fn apply_instance_register(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
        ip: &str,
        port: u16,
        weight: f64,
        healthy: bool,
        enabled: bool,
        metadata: &str,
        cluster_name: &str,
    ) -> RaftResponse {
        let key = Self::instance_key(namespace_id, group_name, service_name, instance_id);
        let now = chrono::Utc::now().timestamp_millis();
        let stored = StoredInstance {
            namespace_id: namespace_id.to_string(),
            group_name: group_name.to_string(),
            service_name: service_name.to_string(),
            instance_id: instance_id.to_string(),
            ip: ip.to_string(),
            port,
            weight,
            healthy,
            enabled,
            metadata: metadata.to_string(),
            cluster_name: cluster_name.to_string(),
            registered_time: now,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to bincode-encode StoredInstance: {}", e);
                return RaftResponse::failure(format!("encode error: {}", e));
            }
        };

        match self.db.put_cf_opt(
            self.cf_instances(),
            key.as_bytes(),
            encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Instance registered: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to register instance: {}", e);
                RaftResponse::failure(format!("Failed to register instance: {}", e))
            }
        }
    }

    fn apply_instance_deregister(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
    ) -> RaftResponse {
        let key = Self::instance_key(namespace_id, group_name, service_name, instance_id);

        match self
            .db
            .delete_cf_opt(self.cf_instances(), key.as_bytes(), &self.write_opts)
        {
            Ok(_) => {
                debug!("Instance deregistered: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to deregister instance: {}", e);
                RaftResponse::failure(format!("Failed to deregister instance: {}", e))
            }
        }
    }

    fn apply_instance_update(
        &self,
        namespace_id: &str,
        group_name: &str,
        service_name: &str,
        instance_id: &str,
        ip: Option<String>,
        port: Option<u16>,
        weight: Option<f64>,
        healthy: Option<bool>,
        enabled: Option<bool>,
        metadata: Option<String>,
    ) -> RaftResponse {
        let key = Self::instance_key(namespace_id, group_name, service_name, instance_id);

        // Read existing instance as typed StoredInstance (bincode)
        let mut stored: StoredInstance = match self.db.get_cf(self.cf_instances(), key.as_bytes())
        {
            Ok(Some(bytes)) => match bincode::deserialize(&bytes) {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to decode StoredInstance: {}", e);
                    return RaftResponse::failure(format!("decode error: {}", e));
                }
            },
            _ => return RaftResponse::failure("Instance not found"),
        };

        if let Some(ip) = ip {
            stored.ip = ip;
        }
        if let Some(port) = port {
            stored.port = port;
        }
        if let Some(weight) = weight {
            stored.weight = weight;
        }
        if let Some(healthy) = healthy {
            stored.healthy = healthy;
        }
        if let Some(enabled) = enabled {
            stored.enabled = enabled;
        }
        if let Some(metadata) = metadata {
            stored.metadata = metadata;
        }
        stored.modified_time = chrono::Utc::now().timestamp_millis();

        let encoded = match bincode::serialize(&stored) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to encode StoredInstance: {}", e);
                return RaftResponse::failure(format!("encode error: {}", e));
            }
        };

        match self.db.put_cf_opt(
            self.cf_instances(),
            key.as_bytes(),
            encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Instance updated: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to update instance: {}", e);
                RaftResponse::failure(format!("Failed to update instance: {}", e))
            }
        }
    }

    // Lock operations (ADV-005)
    fn apply_lock_acquire(
        &self,
        namespace: &str,
        name: &str,
        owner: &str,
        ttl_ms: u64,
        fence_token: u64,
        owner_metadata: Option<String>,
    ) -> RaftResponse {
        let key = Self::lock_key(namespace, name);
        let now = chrono::Utc::now().timestamp_millis();
        let expires_at = now + ttl_ms as i64;

        // Check if lock exists and is held
        if let Ok(Some(bytes)) = self.db.get_cf(self.cf_locks(), key.as_bytes())
            && let Ok(existing) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            // Check if lock is still valid (not expired)
            if let Some(exp) = existing["expires_at"].as_i64()
                && exp > now
            {
                // Lock is held, check if same owner
                if existing["owner"].as_str() == Some(owner) {
                    // Same owner, renew the lock
                    let value = serde_json::json!({
                        "namespace": namespace,
                        "name": name,
                        "owner": owner,
                        "state": "Locked",
                        "fence_token": fence_token,
                        "ttl_ms": ttl_ms,
                        "acquired_at": existing["acquired_at"],
                        "expires_at": expires_at,
                        "renewal_count": existing["renewal_count"].as_u64().unwrap_or(0),
                        "owner_metadata": owner_metadata,
                    });

                    match self.db.put_cf_opt(
                        self.cf_locks(),
                        key.as_bytes(),
                        json_to_bytes(&value),
                        &self.write_opts,
                    ) {
                        Ok(_) => {
                            debug!("Lock re-acquired by same owner: {}", key);
                            return RaftResponse::success_with_data(
                                serde_json::to_vec(&value).unwrap_or_default(),
                            );
                        }
                        Err(e) => {
                            error!("Failed to acquire lock: {}", e);
                            return RaftResponse::failure(format!("Failed to acquire lock: {}", e));
                        }
                    }
                }
                // Lock is held by different owner
                return RaftResponse::failure(format!(
                    "Lock is held by {}",
                    existing["owner"].as_str().unwrap_or("unknown")
                ));
            }
        }

        // Lock is free or expired, acquire it
        let value = serde_json::json!({
            "namespace": namespace,
            "name": name,
            "owner": owner,
            "state": "Locked",
            "fence_token": fence_token,
            "ttl_ms": ttl_ms,
            "acquired_at": now,
            "expires_at": expires_at,
            "renewal_count": 0,
            "owner_metadata": owner_metadata,
        });

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&value),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock acquired: {}", key);
                RaftResponse::success_with_data(serde_json::to_vec(&value).unwrap_or_default())
            }
            Err(e) => {
                error!("Failed to acquire lock: {}", e);
                RaftResponse::failure(format!("Failed to acquire lock: {}", e))
            }
        }
    }

    fn apply_lock_release(
        &self,
        namespace: &str,
        name: &str,
        owner: &str,
        fence_token: Option<u64>,
    ) -> RaftResponse {
        let key = Self::lock_key(namespace, name);

        // Get existing lock
        let existing = match self.db.get_cf(self.cf_locks(), key.as_bytes()) {
            Ok(Some(bytes)) => serde_json::from_slice::<serde_json::Value>(&bytes).ok(),
            _ => None,
        };

        let Some(existing) = existing else {
            return RaftResponse::failure("Lock not found");
        };

        // Check owner
        if existing["owner"].as_str() != Some(owner) {
            return RaftResponse::failure("Not the lock owner");
        }

        // Check fence token if provided
        if let Some(expected_token) = fence_token
            && existing["fence_token"].as_u64() != Some(expected_token)
        {
            return RaftResponse::failure("Fence token mismatch");
        }

        // Release the lock
        let value = serde_json::json!({
            "namespace": namespace,
            "name": name,
            "owner": null,
            "state": "Free",
            "fence_token": existing["fence_token"],
            "ttl_ms": existing["ttl_ms"],
            "acquired_at": null,
            "expires_at": null,
            "renewal_count": 0,
            "owner_metadata": null,
        });

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&value),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock released: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to release lock: {}", e);
                RaftResponse::failure(format!("Failed to release lock: {}", e))
            }
        }
    }

    fn apply_lock_renew(
        &self,
        namespace: &str,
        name: &str,
        owner: &str,
        ttl_ms: Option<u64>,
    ) -> RaftResponse {
        let key = Self::lock_key(namespace, name);
        let now = chrono::Utc::now().timestamp_millis();

        // Get existing lock
        let existing = match self.db.get_cf(self.cf_locks(), key.as_bytes()) {
            Ok(Some(bytes)) => serde_json::from_slice::<serde_json::Value>(&bytes).ok(),
            _ => None,
        };

        let Some(mut existing) = existing else {
            return RaftResponse::failure("Lock not found");
        };

        // Check owner
        if existing["owner"].as_str() != Some(owner) {
            return RaftResponse::failure("Not the lock owner");
        }

        // Check if lock is still valid
        if let Some(exp) = existing["expires_at"].as_i64()
            && exp <= now
        {
            return RaftResponse::failure("Lock has expired");
        }

        // Update TTL and expires_at
        let new_ttl = ttl_ms.unwrap_or_else(|| existing["ttl_ms"].as_u64().unwrap_or(30000));
        let expires_at = now + new_ttl as i64;
        let renewal_count = existing["renewal_count"].as_u64().unwrap_or(0) + 1;

        existing["ttl_ms"] = serde_json::json!(new_ttl);
        existing["expires_at"] = serde_json::json!(expires_at);
        existing["renewal_count"] = serde_json::json!(renewal_count);

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&existing),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock renewed: {} (renewal #{})", key, renewal_count);
                RaftResponse::success_with_data(serde_json::to_vec(&existing).unwrap_or_default())
            }
            Err(e) => {
                error!("Failed to renew lock: {}", e);
                RaftResponse::failure(format!("Failed to renew lock: {}", e))
            }
        }
    }

    fn apply_lock_force_release(&self, namespace: &str, name: &str) -> RaftResponse {
        let key = Self::lock_key(namespace, name);

        // Force release regardless of owner
        let value = serde_json::json!({
            "namespace": namespace,
            "name": name,
            "owner": null,
            "state": "Free",
            "fence_token": 0,
            "ttl_ms": 0,
            "acquired_at": null,
            "expires_at": null,
            "renewal_count": 0,
            "owner_metadata": null,
        });

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&value),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock force released: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to force release lock: {}", e);
                RaftResponse::failure(format!("Failed to force release lock: {}", e))
            }
        }
    }

    fn apply_lock_expire(&self, namespace: &str, name: &str) -> RaftResponse {
        let key = Self::lock_key(namespace, name);

        // Check existence without deserialization (saves JSON parse overhead)
        match self.db.get_cf(self.cf_locks(), key.as_bytes()) {
            Ok(Some(_)) => {}                    // Key exists, proceed to expire
            _ => return RaftResponse::success(), // Nothing to expire
        }

        let value = serde_json::json!({
            "namespace": namespace,
            "name": name,
            "owner": null,
            "state": "Expired",
            "fence_token": 0,
            "ttl_ms": 0,
            "acquired_at": null,
            "expires_at": null,
            "renewal_count": 0,
            "owner_metadata": null,
        });

        match self.db.put_cf_opt(
            self.cf_locks(),
            key.as_bytes(),
            json_to_bytes(&value),
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Lock expired: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to expire lock: {}", e);
                RaftResponse::failure(format!("Failed to expire lock: {}", e))
            }
        }
    }

    /// Save last applied log ID
    async fn save_last_applied(&self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        let bytes = bincode::serialize(&log_id).map_err(|e| sm_error(e, ErrorVerb::Write))?;

        self.db
            .put_cf(self.cf_meta(), KEY_LAST_APPLIED, &bytes)
            .map_err(|e| sm_error(e, ErrorVerb::Write))?;

        *self.last_applied.write().await = Some(log_id);
        Ok(())
    }

    /// Save membership
    async fn save_membership(
        &self,
        membership: StoredMembership<NodeId, openraft::BasicNode>,
    ) -> Result<(), StorageError<NodeId>> {
        let bytes = bincode::serialize(&membership).map_err(|e| sm_error(e, ErrorVerb::Write))?;

        self.db
            .put_cf(self.cf_meta(), KEY_LAST_MEMBERSHIP, &bytes)
            .map_err(|e| sm_error(e, ErrorVerb::Write))?;

        *self.last_membership.write().await = membership;
        Ok(())
    }
}

impl RaftSnapshotBuilder<TypeConfig> for RocksStateMachine {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<NodeId>> {
        let last_applied = *self.last_applied.read().await;
        let last_membership = self.last_membership.read().await.clone();

        let snapshot_id = format!(
            "snapshot-{}-{}",
            last_applied.map(|l| l.index).unwrap_or(0),
            chrono::Utc::now().timestamp_millis()
        );

        let meta = SnapshotMeta {
            last_log_id: last_applied,
            last_membership,
            snapshot_id: snapshot_id.clone(),
        };

        // Serialize all column family data
        let mut snapshot_data = std::collections::HashMap::new();

        for cf_name in [
            CF_CONFIG,
            CF_CONFIG_HISTORY,
            CF_CONFIG_GRAY,
            CF_NAMESPACE,
            CF_USERS,
            CF_ROLES,
            CF_PERMISSIONS,
            CF_INSTANCES,
            CF_LOCKS,
            CF_AI_RESOURCE,
            CF_AI_RESOURCE_VERSION,
            CF_PIPELINE_EXECUTION,
        ] {
            if let Some(cf) = self.db.cf_handle(cf_name) {
                let mut cf_data = Vec::new();
                let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
                for (key, value) in iter.flatten() {
                    cf_data.push((key.to_vec(), value.to_vec()));
                }
                snapshot_data.insert(cf_name.to_string(), cf_data);
            }
        }

        // Use bincode for compact binary serialization (3-5x smaller than JSON).
        // Prefix with magic bytes "BSNA" (Batata SNApshot) for format detection
        // during restore, ensuring backward compatibility with legacy JSON snapshots.
        let bincode_data =
            bincode::serialize(&snapshot_data).map_err(|e| sm_error(e, ErrorVerb::Write))?;
        let mut payload = Vec::with_capacity(SNAPSHOT_MAGIC.len() + bincode_data.len());
        payload.extend_from_slice(SNAPSHOT_MAGIC);
        payload.extend_from_slice(&bincode_data);

        // Append a 8-byte checksum trailer (SipHash) for integrity validation
        let checksum = compute_checksum(&payload);
        let mut data = payload;
        data.extend_from_slice(&checksum.to_le_bytes());

        info!("Built snapshot {} with {} bytes", snapshot_id, data.len());

        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(data)),
        })
    }
}

impl RaftStateMachine<TypeConfig> for RocksStateMachine {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<
        (
            Option<LogId<NodeId>>,
            StoredMembership<NodeId, openraft::BasicNode>,
        ),
        StorageError<NodeId>,
    > {
        let last_applied = *self.last_applied.read().await;
        let last_membership = self.last_membership.read().await.clone();
        Ok((last_applied, last_membership))
    }

    async fn apply<I>(&mut self, entries: I) -> Result<Vec<RaftResponse>, StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + OptionalSend,
        I::IntoIter: OptionalSend,
    {
        let mut responses = Vec::new();
        let mut last_log_id: Option<LogId<NodeId>> = None;

        for entry in entries {
            let log_id = entry.log_id;

            let response = match entry.payload {
                EntryPayload::Normal(request) => self.apply_request(request, log_id.index).await,
                EntryPayload::Membership(membership) => {
                    let stored = StoredMembership::new(Some(log_id), membership);
                    self.save_membership(stored).await?;
                    RaftResponse::success()
                }
                EntryPayload::Blank => RaftResponse::success(),
            };

            last_log_id = Some(log_id);
            responses.push(response);
        }

        // Persist last_applied ONCE after the entire batch, not per-entry.
        // This reduces RocksDB writes from 2N to N+1 for a batch of N entries.
        if let Some(log_id) = last_log_id {
            self.save_last_applied(log_id).await?;
        }

        Ok(responses)
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfig>>, StorageError<NodeId>> {
        // For now, return None as we don't persist snapshots
        // In production, this would load the latest snapshot from disk
        Ok(None)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        // Snapshot builder uses default WriteOptions (snapshots don't need the same tuning)
        RocksStateMachine {
            db: self.db.clone(),
            last_applied: RwLock::new(*self.last_applied.read().await),
            last_membership: RwLock::new(self.last_membership.read().await.clone()),
            plugin_registry: Arc::new(RwLock::new(PluginRegistry::new())),
            naming_hook: self.naming_hook.clone(),
            write_opts: rocksdb::WriteOptions::default(),
        }
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<Cursor<Vec<u8>>>, StorageError<NodeId>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, openraft::BasicNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<NodeId>> {
        let data = snapshot.into_inner();

        if !data.is_empty() {
            // Validate snapshot integrity checksum
            let payload = if data.len() > SNAPSHOT_CHECKSUM_SIZE {
                let (payload, checksum_bytes) = data.split_at(data.len() - SNAPSHOT_CHECKSUM_SIZE);
                let expected = u64::from_le_bytes(checksum_bytes.try_into().map_err(|_| {
                    sm_error(
                        std::io::Error::other("Invalid checksum trailer size"),
                        ErrorVerb::Read,
                    )
                })?);
                let actual = compute_checksum(payload);
                if actual != expected {
                    error!(
                        "Snapshot checksum mismatch: expected {:#x}, got {:#x}",
                        expected, actual
                    );
                    return Err(sm_error(
                        std::io::Error::other("Snapshot checksum mismatch — data may be corrupted"),
                        ErrorVerb::Read,
                    ));
                }
                info!("Snapshot checksum verified ({:#x})", actual);
                payload
            } else {
                // Legacy snapshot without checksum — accept but warn
                warn!("Snapshot has no checksum trailer, skipping integrity check");
                &data
            };

            // Deserialize snapshot: detect format by magic header
            let snapshot_data: std::collections::HashMap<String, Vec<(Vec<u8>, Vec<u8>)>> =
                if payload.starts_with(SNAPSHOT_MAGIC) {
                    let bincode_payload = &payload[SNAPSHOT_MAGIC.len()..];
                    bincode::deserialize(bincode_payload)
                        .map_err(|e| sm_error(e, ErrorVerb::Read))?
                } else {
                    // Legacy JSON format (backward compatibility)
                    info!("Restoring legacy JSON-format snapshot");
                    serde_json::from_slice(payload).map_err(|e| sm_error(e, ErrorVerb::Read))?
                };

            // Atomically clear and restore each column family using a single WriteBatch
            // per CF. Uses delete_range for O(1) clearing instead of per-key deletion.
            for (cf_name, cf_data) in snapshot_data {
                if let Some(cf) = self.db.cf_handle(&cf_name) {
                    let mut batch = rocksdb::WriteBatch::default();

                    // Clear existing data with range delete (much faster than per-key).
                    // Keys are UTF-8 strings, so 0xFF as exclusive end covers all keys.
                    batch.delete_range_cf(&cf, vec![0u8], vec![0xFFu8, 0xFF, 0xFF, 0xFF]);

                    // Restore snapshot data in the same batch
                    for (key, value) in cf_data {
                        batch.put_cf(cf, &key, &value);
                    }

                    self.db
                        .write(batch)
                        .map_err(|e| sm_error(e, ErrorVerb::Write))?;
                }
            }
        }

        // Update metadata
        if let Some(log_id) = meta.last_log_id {
            self.save_last_applied(log_id).await?;
        }
        self.save_membership(meta.last_membership.clone()).await?;

        info!("Snapshot installed: {:?}", meta.snapshot_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let config_key = RocksStateMachine::config_key("test", "DEFAULT_GROUP", "public");
        assert_eq!(config_key, "public@@DEFAULT_GROUP@@test");

        let permission_key = RocksStateMachine::permission_key("admin", "public::*", "rw");
        assert_eq!(permission_key, "admin@@public::*@@rw");
    }

    #[test]
    fn test_config_key_format() {
        // Test edge cases
        let key = RocksStateMachine::config_key("", "", "");
        assert_eq!(key, "@@@@");

        let key_with_special = RocksStateMachine::config_key("data.id", "group-1", "ns:test");
        assert_eq!(key_with_special, "ns:test@@group-1@@data.id");
    }

    #[test]
    fn test_namespace_key() {
        let key = RocksStateMachine::namespace_key("public");
        assert_eq!(key, "public");

        let key_empty = RocksStateMachine::namespace_key("");
        assert_eq!(key_empty, "");
    }
}
