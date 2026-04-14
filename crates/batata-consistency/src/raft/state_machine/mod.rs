// Raft state machine implementation
// Applies committed log entries to the application state using RocksDB

// Allow many arguments for Raft apply operations - parameters are logically coupled
#![allow(clippy::too_many_arguments)]
// Allow complex types for snapshot data structures
#![allow(clippy::type_complexity)]

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

/// RocksDB-based state machine for Raft.
///
/// Fields are `pub(super)` so apply-handler submodules (`apply_config`,
/// `apply_lock`, …) can touch DB + write options without going through an
/// accessor that clones `Arc<DB>` on every call.
pub struct RocksStateMachine {
    pub(super) db: Arc<DB>,
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
    pub(super) write_opts: rocksdb::WriteOptions,
}

/// Serialize a JSON value directly to bytes, avoiding intermediate String allocation.
///
/// `pub(super)` so apply-handler submodules can share this helper.
#[inline]
pub(super) fn json_to_bytes(value: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

mod apply_config;
mod apply_lock;
mod keys;
mod records;
pub use records::{
    StoredConfig, StoredConfigGray, StoredConfigHistory, StoredInstance, StoredNamespace,
    StoredPermission, StoredRole, StoredUser,
};

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
    pub(super) fn cf_config(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONFIG)
            .expect("CF_CONFIG must exist - database may be corrupted")
    }

    pub(super) fn cf_namespace(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_NAMESPACE)
            .expect("CF_NAMESPACE must exist - database may be corrupted")
    }

    pub(super) fn cf_users(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_USERS)
            .expect("CF_USERS must exist - database may be corrupted")
    }

    pub(super) fn cf_roles(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_ROLES)
            .expect("CF_ROLES must exist - database may be corrupted")
    }

    pub(super) fn cf_permissions(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_PERMISSIONS)
            .expect("CF_PERMISSIONS must exist - database may be corrupted")
    }

    pub(super) fn cf_instances(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_INSTANCES)
            .expect("CF_INSTANCES must exist - database may be corrupted")
    }

    pub(super) fn cf_locks(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_LOCKS)
            .expect("CF_LOCKS must exist - database may be corrupted")
    }

    pub(super) fn cf_config_history(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONFIG_HISTORY)
            .expect("CF_CONFIG_HISTORY must exist - database may be corrupted")
    }

    pub(super) fn cf_config_gray(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONFIG_GRAY)
            .expect("CF_CONFIG_GRAY must exist - database may be corrupted")
    }

    fn cf_meta(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_META)
            .expect("CF_META must exist - database may be corrupted")
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
                source,
            } => self.apply_user_create(&username, &password_hash, enabled, &source),

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
                        hook.on_deregister(&namespace_id, &group_name, &service_name, &instance_id);
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
        source: &str,
    ) -> RaftResponse {
        let key = Self::user_key(username);
        let now = chrono::Utc::now().timestamp_millis();
        let stored = StoredUser {
            username: username.to_string(),
            password_hash: password_hash.to_string(),
            enabled,
            created_time: now,
            modified_time: now,
            source: source.to_string(),
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("Failed to encode user: {}", e)),
        };
        match self
            .db
            .put_cf_opt(self.cf_users(), key.as_bytes(), &encoded, &self.write_opts)
        {
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
        match self
            .db
            .put_cf_opt(self.cf_users(), key.as_bytes(), &encoded, &self.write_opts)
        {
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
        match self
            .db
            .put_cf_opt(self.cf_roles(), key.as_bytes(), &encoded, &self.write_opts)
        {
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
        let mut stored: StoredInstance = match self.db.get_cf(self.cf_instances(), key.as_bytes()) {
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

    /// Reentrant lock semantics (P0-2 fix): same owner acquiring the same
    /// lock twice must increment `lock_count` and require matching releases
    /// to fully unlock. Previously acquire+acquire+release would drop the
    /// lock — incompatible with Nacos AtomicLockService semantics.
    #[tokio::test]
    async fn test_lock_reentrant_semantics() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(tmp.path())
            .await
            .expect("state machine");

        // First acquire: fresh lock, depth = 1
        let resp = sm.apply_lock_acquire("public", "k1", "owner-a", 60_000, 1, None);
        assert!(resp.success, "first acquire must succeed");
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["lock_count"].as_u64(), Some(1));
        assert_eq!(v["fence_token"].as_u64(), Some(1));
        assert_eq!(v["owner"].as_str(), Some("owner-a"));

        // Second acquire by same owner: depth = 2, same fence token
        let resp = sm.apply_lock_acquire("public", "k1", "owner-a", 60_000, 99, None);
        assert!(resp.success, "reentrant acquire must succeed");
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["lock_count"].as_u64(), Some(2));
        assert_eq!(
            v["fence_token"].as_u64(),
            Some(1),
            "fence token must remain stable across reentrant acquires"
        );

        // Third acquire by same owner: depth = 3
        let resp = sm.apply_lock_acquire("public", "k1", "owner-a", 60_000, 77, None);
        assert!(resp.success);
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["lock_count"].as_u64(), Some(3));

        // Different owner is rejected while held
        let resp = sm.apply_lock_acquire("public", "k1", "owner-b", 60_000, 5, None);
        assert!(!resp.success, "different owner must fail to acquire");

        // Release once: depth 3 -> 2, still locked
        let resp = sm.apply_lock_release("public", "k1", "owner-a", None);
        assert!(resp.success);
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["lock_count"].as_u64(), Some(2));
        assert_eq!(v["state"].as_str(), Some("Locked"));

        // Different owner still rejected
        let resp = sm.apply_lock_acquire("public", "k1", "owner-b", 60_000, 5, None);
        assert!(!resp.success);

        // Release again: depth 2 -> 1, still locked
        let resp = sm.apply_lock_release("public", "k1", "owner-a", None);
        assert!(resp.success);

        // Final release: depth 1 -> 0, lock becomes Free
        let resp = sm.apply_lock_release("public", "k1", "owner-a", None);
        assert!(resp.success);

        // Now another owner can acquire
        let resp = sm.apply_lock_acquire("public", "k1", "owner-b", 60_000, 42, None);
        assert!(resp.success, "new owner must acquire after full release");
        let v: serde_json::Value = serde_json::from_slice(&resp.data.unwrap()).unwrap();
        assert_eq!(v["owner"].as_str(), Some("owner-b"));
        assert_eq!(v["lock_count"].as_u64(), Some(1));
        assert_eq!(
            v["fence_token"].as_u64(),
            Some(42),
            "new owner gets its own fence token"
        );
    }

    /// Legacy entries without `lock_count` field must be treated as depth=1
    /// so existing on-disk Raft logs upgrade cleanly.
    #[tokio::test]
    async fn test_lock_legacy_entry_depth_one() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sm = RocksStateMachine::new(tmp.path())
            .await
            .expect("state machine");

        // Write a legacy-shaped value directly (no lock_count field).
        let key = RocksStateMachine::lock_key("public", "legacy");
        let now = chrono::Utc::now().timestamp_millis();
        let legacy = serde_json::json!({
            "namespace": "public",
            "name": "legacy",
            "owner": "owner-a",
            "state": "Locked",
            "fence_token": 7,
            "ttl_ms": 60000,
            "acquired_at": now,
            "expires_at": now + 60000,
            "renewal_count": 0,
            "owner_metadata": null,
        });
        sm.db
            .put_cf_opt(
                sm.cf_locks(),
                key.as_bytes(),
                serde_json::to_vec(&legacy).unwrap(),
                &sm.write_opts,
            )
            .unwrap();

        // Single release must fully free it (legacy depth is 1).
        let resp = sm.apply_lock_release("public", "legacy", "owner-a", None);
        assert!(resp.success);
        let resp = sm.apply_lock_acquire("public", "legacy", "owner-b", 60_000, 8, None);
        assert!(resp.success, "legacy entry must fully release in one call");
    }
}
