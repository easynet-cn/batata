// Raft state machine implementation
// Applies committed log entries to the application state using RocksDB

// Allow many arguments for Raft apply operations - parameters are logically coupled
#![allow(clippy::too_many_arguments)]
// Allow complex types for snapshot data structures
#![allow(clippy::type_complexity)]

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
use tracing::{debug, error, info};

use super::request::{RaftRequest, RaftResponse};
use super::types::{NodeId, TypeConfig};

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
pub const CF_CONFIG: &str = "config";
pub const CF_CONFIG_HISTORY: &str = "config_history";
pub const CF_CONFIG_GRAY: &str = "config_gray";
pub const CF_NAMESPACE: &str = "namespace";
pub const CF_USERS: &str = "users";
pub const CF_ROLES: &str = "roles";
pub const CF_PERMISSIONS: &str = "permissions";
pub const CF_INSTANCES: &str = "instances";
pub const CF_LOCKS: &str = "locks";
pub const CF_CONSUL_KV: &str = "consul_kv";
pub const CF_CONSUL_ACL: &str = "consul_acl";
pub const CF_CONSUL_SESSIONS: &str = "consul_sessions";
pub const CF_CONSUL_QUERIES: &str = "consul_queries";
const CF_META: &str = "meta";

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
}

impl RocksStateMachine {
    /// Get a reference to the underlying RocksDB instance
    pub fn db(&self) -> Arc<DB> {
        self.db.clone()
    }

    /// Create a new RocksDB state machine
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError<NodeId>> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        // Performance optimizations
        // Write buffer size: 64MB for better write throughput
        db_opts.set_write_buffer_size(64 * 1024 * 1024);
        // Max write buffer number for write stall prevention
        db_opts.set_max_write_buffer_number(3);
        // Enable compression for storage efficiency
        db_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);

        // Block-based table options with block cache for read performance
        let mut block_opts = BlockBasedOptions::default();
        // 256MB block cache for read optimization
        let cache = rocksdb::Cache::new_lru_cache(256 * 1024 * 1024);
        block_opts.set_block_cache(&cache);
        // Bloom filter for faster lookups
        block_opts.set_bloom_filter(10.0, false);

        // Column family options with same optimizations
        let mut cf_opts = Options::default();
        cf_opts.set_write_buffer_size(64 * 1024 * 1024);
        cf_opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        cf_opts.set_block_based_table_factory(&block_opts);

        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_CONFIG, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONFIG_HISTORY, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONFIG_GRAY, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_NAMESPACE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_USERS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_ROLES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_PERMISSIONS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_INSTANCES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_LOCKS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONSUL_KV, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONSUL_ACL, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONSUL_SESSIONS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_CONSUL_QUERIES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_META, cf_opts),
        ];

        let db = DB::open_cf_descriptors(&db_opts, path, cfs)
            .map_err(|e| sm_error(e, ErrorVerb::Read))?;

        let sm = Self {
            db: Arc::new(db),
            last_applied: RwLock::new(None),
            last_membership: RwLock::new(StoredMembership::default()),
        };

        // Load cached values asynchronously
        sm.load_cached_values().await?;

        info!("RocksDB state machine initialized");
        Ok(sm)
    }

    /// Load cached values from storage
    async fn load_cached_values(&self) -> Result<(), StorageError<NodeId>> {
        // Load last applied
        if let Some(bytes) = self
            .db
            .get_cf(self.cf_meta(), KEY_LAST_APPLIED)
            .map_err(|e| sm_error(e, ErrorVerb::Read))?
        {
            let log_id: LogId<NodeId> =
                serde_json::from_slice(&bytes).map_err(|e| sm_error(e, ErrorVerb::Read))?;
            *self.last_applied.write().await = Some(log_id);
        }

        // Load last membership
        if let Some(bytes) = self
            .db
            .get_cf(self.cf_meta(), KEY_LAST_MEMBERSHIP)
            .map_err(|e| sm_error(e, ErrorVerb::Read))?
        {
            let membership: StoredMembership<NodeId, openraft::BasicNode> =
                serde_json::from_slice(&bytes).map_err(|e| sm_error(e, ErrorVerb::Read))?;
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
    async fn apply_request(&self, request: RaftRequest) -> RaftResponse {
        match request {
            RaftRequest::ConfigPublish {
                data_id,
                group,
                tenant,
                content,
                config_type,
                app_name,
                tag,
                desc,
                src_user,
            } => self.apply_config_publish(
                &data_id,
                &group,
                &tenant,
                &content,
                config_type,
                app_name,
                tag,
                desc,
                src_user,
            ),

            RaftRequest::ConfigRemove {
                data_id,
                group,
                tenant,
            } => self.apply_config_remove(&data_id, &group, &tenant),

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

            RaftRequest::ConfigHistoryInsert {
                id,
                data_id,
                group,
                tenant,
                content,
                md5,
                src_user,
                src_ip,
                op_type,
                created_time,
                last_modified_time,
            } => self.apply_config_history_insert(
                id,
                &data_id,
                &group,
                &tenant,
                &content,
                &md5,
                src_user,
                src_ip,
                &op_type,
                created_time,
                last_modified_time,
            ),

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

            RaftRequest::PersistentInstanceRegister {
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
            } => self.apply_instance_register(
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
            ),

            RaftRequest::PersistentInstanceDeregister {
                namespace_id,
                group_name,
                service_name,
                instance_id,
            } => self.apply_instance_deregister(
                &namespace_id,
                &group_name,
                &service_name,
                &instance_id,
            ),

            RaftRequest::PersistentInstanceUpdate {
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
            } => self.apply_instance_update(
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
            ),

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
        }
    }

    // Config operations
    fn apply_config_publish(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        config_type: Option<String>,
        app_name: Option<String>,
        tag: Option<String>,
        desc: Option<String>,
        src_user: Option<String>,
    ) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);
        let value = serde_json::json!({
            "data_id": data_id,
            "group": group,
            "tenant": tenant,
            "content": content,
            "config_type": config_type,
            "app_name": app_name,
            "tag": tag,
            "desc": desc,
            "src_user": src_user,
            "modified_time": chrono::Utc::now().timestamp_millis(),
        });

        match self.db.put_cf(
            self.cf_config(),
            key.as_bytes(),
            value.to_string().as_bytes(),
        ) {
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

    fn apply_config_remove(&self, data_id: &str, group: &str, tenant: &str) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);

        match self.db.delete_cf(self.cf_config(), key.as_bytes()) {
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

    // Config history operations
    #[allow(clippy::too_many_arguments)]
    fn apply_config_history_insert(
        &self,
        id: i64,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        md5: &str,
        src_user: Option<String>,
        src_ip: Option<String>,
        op_type: &str,
        created_time: i64,
        last_modified_time: i64,
    ) -> RaftResponse {
        let key = Self::config_history_key(data_id, group, tenant, id as u64);
        let value = serde_json::json!({
            "id": id,
            "data_id": data_id,
            "group": group,
            "tenant": tenant,
            "content": content,
            "md5": md5,
            "src_user": src_user,
            "src_ip": src_ip,
            "op_type": op_type,
            "created_time": created_time,
            "last_modified_time": last_modified_time,
        });

        match self.db.put_cf(
            self.cf_config_history(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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

        // Read existing config and update tags
        let existing = match self.db.get_cf(self.cf_config(), key.as_bytes()) {
            Ok(Some(bytes)) => serde_json::from_slice::<serde_json::Value>(&bytes).ok(),
            _ => None,
        };

        if let Some(mut existing) = existing {
            existing["config_tags"] = serde_json::json!(tag);
            existing["modified_time"] = serde_json::json!(chrono::Utc::now().timestamp_millis());

            match self.db.put_cf(
                self.cf_config(),
                key.as_bytes(),
                existing.to_string().as_bytes(),
            ) {
                Ok(_) => {
                    debug!("Config tags updated: {}", key);
                    RaftResponse::success()
                }
                Err(e) => {
                    error!("Failed to update config tags: {}", e);
                    RaftResponse::failure(format!("Failed to update config tags: {}", e))
                }
            }
        } else {
            RaftResponse::failure("Config not found for tag update")
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

        // Read existing config and clear tags
        let existing = match self.db.get_cf(self.cf_config(), key.as_bytes()) {
            Ok(Some(bytes)) => serde_json::from_slice::<serde_json::Value>(&bytes).ok(),
            _ => None,
        };

        if let Some(mut existing) = existing {
            existing["config_tags"] = serde_json::json!("");
            existing["modified_time"] = serde_json::json!(chrono::Utc::now().timestamp_millis());

            match self.db.put_cf(
                self.cf_config(),
                key.as_bytes(),
                existing.to_string().as_bytes(),
            ) {
                Ok(_) => {
                    debug!("Config tags deleted: {}", key);
                    RaftResponse::success()
                }
                Err(e) => {
                    error!("Failed to delete config tags: {}", e);
                    RaftResponse::failure(format!("Failed to delete config tags: {}", e))
                }
            }
        } else {
            RaftResponse::failure("Config not found for tag delete")
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
        let value = serde_json::json!({
            "namespace_id": namespace_id,
            "namespace_name": namespace_name,
            "namespace_desc": namespace_desc,
            "created_time": chrono::Utc::now().timestamp_millis(),
        });

        match self.db.put_cf(
            self.cf_namespace(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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
        let value = serde_json::json!({
            "namespace_id": namespace_id,
            "namespace_name": namespace_name,
            "namespace_desc": namespace_desc,
            "modified_time": chrono::Utc::now().timestamp_millis(),
        });

        match self.db.put_cf(
            self.cf_namespace(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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

        match self.db.delete_cf(self.cf_namespace(), key.as_bytes()) {
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
        let value = serde_json::json!({
            "username": username,
            "password_hash": password_hash,
            "enabled": enabled,
            "created_time": chrono::Utc::now().timestamp_millis(),
        });

        match self.db.put_cf(
            self.cf_users(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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

        // Read existing user
        let existing = match self.db.get_cf(self.cf_users(), key.as_bytes()) {
            Ok(Some(bytes)) => serde_json::from_slice::<serde_json::Value>(&bytes).ok(),
            _ => None,
        };

        let value = if let Some(mut existing) = existing {
            if let Some(hash) = password_hash {
                existing["password_hash"] = serde_json::json!(hash);
            }
            if let Some(en) = enabled {
                existing["enabled"] = serde_json::json!(en);
            }
            existing["modified_time"] = serde_json::json!(chrono::Utc::now().timestamp_millis());
            existing
        } else {
            return RaftResponse::failure("User not found");
        };

        match self.db.put_cf(
            self.cf_users(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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

        match self.db.delete_cf(self.cf_users(), key.as_bytes()) {
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
        let value = serde_json::json!({
            "role": role,
            "username": username,
            "created_time": chrono::Utc::now().timestamp_millis(),
        });

        match self.db.put_cf(
            self.cf_roles(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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

        match self.db.delete_cf(self.cf_roles(), key.as_bytes()) {
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
        let value = serde_json::json!({
            "role": role,
            "resource": resource,
            "action": action,
            "created_time": chrono::Utc::now().timestamp_millis(),
        });

        match self.db.put_cf(
            self.cf_permissions(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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

        match self.db.delete_cf(self.cf_permissions(), key.as_bytes()) {
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
        let value = serde_json::json!({
            "namespace_id": namespace_id,
            "group_name": group_name,
            "service_name": service_name,
            "instance_id": instance_id,
            "ip": ip,
            "port": port,
            "weight": weight,
            "healthy": healthy,
            "enabled": enabled,
            "metadata": metadata,
            "cluster_name": cluster_name,
            "registered_time": chrono::Utc::now().timestamp_millis(),
        });

        match self.db.put_cf(
            self.cf_instances(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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

        match self.db.delete_cf(self.cf_instances(), key.as_bytes()) {
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

        // Read existing instance
        let existing = match self.db.get_cf(self.cf_instances(), key.as_bytes()) {
            Ok(Some(bytes)) => serde_json::from_slice::<serde_json::Value>(&bytes).ok(),
            _ => None,
        };

        let value = if let Some(mut existing) = existing {
            if let Some(ip) = ip {
                existing["ip"] = serde_json::json!(ip);
            }
            if let Some(port) = port {
                existing["port"] = serde_json::json!(port);
            }
            if let Some(weight) = weight {
                existing["weight"] = serde_json::json!(weight);
            }
            if let Some(healthy) = healthy {
                existing["healthy"] = serde_json::json!(healthy);
            }
            if let Some(enabled) = enabled {
                existing["enabled"] = serde_json::json!(enabled);
            }
            if let Some(metadata) = metadata {
                existing["metadata"] = serde_json::json!(metadata);
            }
            existing["modified_time"] = serde_json::json!(chrono::Utc::now().timestamp_millis());
            existing
        } else {
            return RaftResponse::failure("Instance not found");
        };

        match self.db.put_cf(
            self.cf_instances(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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
        if let Ok(Some(bytes)) = self.db.get_cf(self.cf_locks(), key.as_bytes()) {
            if let Ok(existing) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                // Check if lock is still valid (not expired)
                if let Some(exp) = existing["expires_at"].as_i64() {
                    if exp > now {
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

                            match self.db.put_cf(
                                self.cf_locks(),
                                key.as_bytes(),
                                value.to_string().as_bytes(),
                            ) {
                                Ok(_) => {
                                    debug!("Lock re-acquired by same owner: {}", key);
                                    return RaftResponse::success_with_data(
                                        serde_json::to_vec(&value).unwrap_or_default(),
                                    );
                                }
                                Err(e) => {
                                    error!("Failed to acquire lock: {}", e);
                                    return RaftResponse::failure(format!(
                                        "Failed to acquire lock: {}",
                                        e
                                    ));
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

        match self.db.put_cf(
            self.cf_locks(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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
        if let Some(expected_token) = fence_token {
            if existing["fence_token"].as_u64() != Some(expected_token) {
                return RaftResponse::failure("Fence token mismatch");
            }
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

        match self.db.put_cf(
            self.cf_locks(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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
        if let Some(exp) = existing["expires_at"].as_i64() {
            if exp <= now {
                return RaftResponse::failure("Lock has expired");
            }
        }

        // Update TTL and expires_at
        let new_ttl = ttl_ms.unwrap_or_else(|| existing["ttl_ms"].as_u64().unwrap_or(30000));
        let expires_at = now + new_ttl as i64;
        let renewal_count = existing["renewal_count"].as_u64().unwrap_or(0) + 1;

        existing["ttl_ms"] = serde_json::json!(new_ttl);
        existing["expires_at"] = serde_json::json!(expires_at);
        existing["renewal_count"] = serde_json::json!(renewal_count);

        match self.db.put_cf(
            self.cf_locks(),
            key.as_bytes(),
            existing.to_string().as_bytes(),
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

        match self.db.put_cf(
            self.cf_locks(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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

        // Mark lock as expired
        let existing = match self.db.get_cf(self.cf_locks(), key.as_bytes()) {
            Ok(Some(bytes)) => serde_json::from_slice::<serde_json::Value>(&bytes).ok(),
            _ => None,
        };

        if existing.is_none() {
            return RaftResponse::success(); // Nothing to expire
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

        match self.db.put_cf(
            self.cf_locks(),
            key.as_bytes(),
            value.to_string().as_bytes(),
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
        let bytes = serde_json::to_vec(&log_id).map_err(|e| sm_error(e, ErrorVerb::Write))?;

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
        let bytes = serde_json::to_vec(&membership).map_err(|e| sm_error(e, ErrorVerb::Write))?;

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
            CF_CONSUL_KV,
            CF_CONSUL_ACL,
            CF_CONSUL_SESSIONS,
            CF_CONSUL_QUERIES,
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

        let data = serde_json::to_vec(&snapshot_data).map_err(|e| sm_error(e, ErrorVerb::Write))?;

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

        for entry in entries {
            let log_id = entry.log_id;

            let response = match entry.payload {
                EntryPayload::Normal(request) => self.apply_request(request).await,
                EntryPayload::Membership(membership) => {
                    let stored = StoredMembership::new(Some(log_id), membership);
                    self.save_membership(stored).await?;
                    RaftResponse::success()
                }
                EntryPayload::Blank => RaftResponse::success(),
            };

            self.save_last_applied(log_id).await?;
            responses.push(response);
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
        RocksStateMachine {
            db: self.db.clone(),
            last_applied: RwLock::new(*self.last_applied.read().await),
            last_membership: RwLock::new(self.last_membership.read().await.clone()),
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
            // Deserialize the snapshot data
            let snapshot_data: std::collections::HashMap<String, Vec<(Vec<u8>, Vec<u8>)>> =
                serde_json::from_slice(&data).map_err(|e| sm_error(e, ErrorVerb::Read))?;

            // Clear and restore each column family
            for (cf_name, cf_data) in snapshot_data {
                if let Some(cf) = self.db.cf_handle(&cf_name) {
                    // Delete existing data
                    let mut batch = rocksdb::WriteBatch::default();
                    let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
                    for (key, _) in iter.flatten() {
                        batch.delete_cf(cf, &key);
                    }
                    self.db
                        .write(batch)
                        .map_err(|e| sm_error(e, ErrorVerb::Write))?;

                    // Restore snapshot data
                    let mut batch = rocksdb::WriteBatch::default();
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
