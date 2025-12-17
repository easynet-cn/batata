// Raft state machine implementation
// Applies committed log entries to the application state using RocksDB

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use openraft::storage::{RaftSnapshotBuilder, RaftStateMachine, Snapshot};
use openraft::{
    Entry, EntryPayload, ErrorSubject, ErrorVerb, LogId, OptionalSend, SnapshotMeta, StorageError, StoredMembership,
};
use rocksdb::{ColumnFamily, ColumnFamilyDescriptor, Options, DB};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::request::{RaftRequest, RaftResponse};
use super::types::{NodeId, SnapshotData, TypeConfig};

/// Helper to create StorageError for state machine operations
fn sm_error(e: impl std::error::Error + Send + Sync + 'static, verb: ErrorVerb) -> StorageError<NodeId> {
    StorageError::from_io_error(
        ErrorSubject::StateMachine,
        verb,
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string()),
    )
}

// Column family names for state machine
const CF_CONFIG: &str = "config";
const CF_NAMESPACE: &str = "namespace";
const CF_USERS: &str = "users";
const CF_ROLES: &str = "roles";
const CF_PERMISSIONS: &str = "permissions";
const CF_INSTANCES: &str = "instances";
const CF_META: &str = "meta";

// Meta keys
const KEY_LAST_APPLIED: &[u8] = b"last_applied";
const KEY_LAST_MEMBERSHIP: &[u8] = b"last_membership";
const KEY_SNAPSHOT_INDEX: &[u8] = b"snapshot_index";

/// RocksDB-based state machine for Raft
pub struct RocksStateMachine {
    db: Arc<DB>,
    /// Last applied log ID
    last_applied: RwLock<Option<LogId<NodeId>>>,
    /// Last membership configuration
    last_membership: RwLock<StoredMembership<NodeId, openraft::BasicNode>>,
}

impl RocksStateMachine {
    /// Create a new RocksDB state machine
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StorageError<NodeId>> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let cf_opts = Options::default();

        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_CONFIG, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_NAMESPACE, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_USERS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_ROLES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_PERMISSIONS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_INSTANCES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_META, cf_opts),
        ];

        let db = DB::open_cf_descriptors(&db_opts, path, cfs)
            .map_err(|e| sm_error(e, ErrorVerb::Read))?;

        let sm = Self {
            db: Arc::new(db),
            last_applied: RwLock::new(None),
            last_membership: RwLock::new(StoredMembership::default()),
        };

        // Load cached values
        tokio::runtime::Handle::current().block_on(async {
            sm.load_cached_values().await
        })?;

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
            let log_id: LogId<NodeId> = serde_json::from_slice(&bytes)
                .map_err(|e| sm_error(e, ErrorVerb::Read))?;
            *self.last_applied.write().await = Some(log_id);
        }

        // Load last membership
        if let Some(bytes) = self
            .db
            .get_cf(self.cf_meta(), KEY_LAST_MEMBERSHIP)
            .map_err(|e| sm_error(e, ErrorVerb::Read))?
        {
            let membership: StoredMembership<NodeId, openraft::BasicNode> = serde_json::from_slice(&bytes)
                .map_err(|e| sm_error(e, ErrorVerb::Read))?;
            *self.last_membership.write().await = membership;
        }

        Ok(())
    }

    /// Get column family handles
    fn cf_config(&self) -> &ColumnFamily {
        self.db.cf_handle(CF_CONFIG).expect("CF_CONFIG must exist")
    }

    fn cf_namespace(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_NAMESPACE)
            .expect("CF_NAMESPACE must exist")
    }

    fn cf_users(&self) -> &ColumnFamily {
        self.db.cf_handle(CF_USERS).expect("CF_USERS must exist")
    }

    fn cf_roles(&self) -> &ColumnFamily {
        self.db.cf_handle(CF_ROLES).expect("CF_ROLES must exist")
    }

    fn cf_permissions(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_PERMISSIONS)
            .expect("CF_PERMISSIONS must exist")
    }

    fn cf_instances(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_INSTANCES)
            .expect("CF_INSTANCES must exist")
    }

    fn cf_meta(&self) -> &ColumnFamily {
        self.db.cf_handle(CF_META).expect("CF_META must exist")
    }

    /// Generate config key
    fn config_key(data_id: &str, group: &str, tenant: &str) -> String {
        format!("{}@@{}@@{}", tenant, group, data_id)
    }

    /// Generate namespace key
    fn namespace_key(namespace_id: &str) -> String {
        namespace_id.to_string()
    }

    /// Generate user key
    fn user_key(username: &str) -> String {
        username.to_string()
    }

    /// Generate role key
    fn role_key(role: &str, username: &str) -> String {
        format!("{}@@{}", role, username)
    }

    /// Generate permission key
    fn permission_key(role: &str, resource: &str, action: &str) -> String {
        format!("{}@@{}@@{}", role, resource, action)
    }

    /// Generate instance key
    fn instance_key(
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
                &data_id, &group, &tenant, &content, config_type, app_name, tag, desc, src_user,
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

            RaftRequest::ConfigHistoryInsert { .. } => {
                // Config history is typically stored separately
                RaftResponse::success()
            }

            RaftRequest::ConfigTagsUpdate { .. } => {
                // Config tags are stored with config
                RaftResponse::success()
            }

            RaftRequest::ConfigTagsDelete { .. } => RaftResponse::success(),

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
            } => {
                self.apply_instance_deregister(&namespace_id, &group_name, &service_name, &instance_id)
            }

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

        match self
            .db
            .put_cf(self.cf_config(), key.as_bytes(), value.to_string().as_bytes())
        {
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

        match self
            .db
            .put_cf(self.cf_namespace(), key.as_bytes(), value.to_string().as_bytes())
        {
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

        match self
            .db
            .put_cf(self.cf_namespace(), key.as_bytes(), value.to_string().as_bytes())
        {
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

        match self
            .db
            .put_cf(self.cf_users(), key.as_bytes(), value.to_string().as_bytes())
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

        // Read existing user
        let existing = match self.db.get_cf(self.cf_users(), key.as_bytes()) {
            Ok(Some(bytes)) => {
                serde_json::from_slice::<serde_json::Value>(&bytes).ok()
            }
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

        match self
            .db
            .put_cf(self.cf_users(), key.as_bytes(), value.to_string().as_bytes())
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

        match self
            .db
            .put_cf(self.cf_roles(), key.as_bytes(), value.to_string().as_bytes())
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

        match self
            .db
            .put_cf(self.cf_permissions(), key.as_bytes(), value.to_string().as_bytes())
        {
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

        match self
            .db
            .put_cf(self.cf_instances(), key.as_bytes(), value.to_string().as_bytes())
        {
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
            Ok(Some(bytes)) => {
                serde_json::from_slice::<serde_json::Value>(&bytes).ok()
            }
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

        match self
            .db
            .put_cf(self.cf_instances(), key.as_bytes(), value.to_string().as_bytes())
        {
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
        let bytes = serde_json::to_vec(&log_id)
            .map_err(|e| sm_error(e, ErrorVerb::Write))?;

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
        let bytes = serde_json::to_vec(&membership)
            .map_err(|e| sm_error(e, ErrorVerb::Write))?;

        self.db
            .put_cf(self.cf_meta(), KEY_LAST_MEMBERSHIP, &bytes)
            .map_err(|e| sm_error(e, ErrorVerb::Write))?;

        *self.last_membership.write().await = membership;
        Ok(())
    }
}

impl RaftSnapshotBuilder<TypeConfig> for RocksStateMachine {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<NodeId>> {
        // For now, create a simple snapshot with last applied info
        let last_applied = self.last_applied.read().await.clone();
        let last_membership = self.last_membership.read().await.clone();

        let snapshot_id = format!(
            "snapshot-{}-{}",
            last_applied.map(|l| l.index).unwrap_or(0),
            chrono::Utc::now().timestamp_millis()
        );

        let meta = SnapshotMeta {
            last_log_id: last_applied,
            last_membership,
            snapshot_id,
        };

        // TODO: Implement full snapshot serialization
        let data = SnapshotData::new(Vec::new());

        Ok(Snapshot {
            meta,
            snapshot: Box::new(Cursor::new(data.data)),
        })
    }
}

impl RaftStateMachine<TypeConfig> for RocksStateMachine {
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, openraft::BasicNode>), StorageError<NodeId>> {
        let last_applied = self.last_applied.read().await.clone();
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

    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot<TypeConfig>>, StorageError<NodeId>> {
        // For now, return None as we don't persist snapshots
        // In production, this would load the latest snapshot from disk
        Ok(None)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        RocksStateMachine {
            db: self.db.clone(),
            last_applied: RwLock::new(self.last_applied.read().await.clone()),
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
        // TODO: Implement full snapshot installation
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
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_state_machine_basic() {
        let temp_dir = tempdir().unwrap();
        let sm = RocksStateMachine::new(temp_dir.path()).unwrap();

        // Initial state should be empty
        let (last_applied, _membership) = RocksStateMachine::applied_state(
            &mut RocksStateMachine {
                db: sm.db.clone(),
                last_applied: RwLock::new(None),
                last_membership: RwLock::new(StoredMembership::default()),
            },
        )
        .await
        .unwrap();
        assert!(last_applied.is_none());
    }

    #[test]
    fn test_key_generation() {
        let config_key = RocksStateMachine::config_key("test", "DEFAULT_GROUP", "public");
        assert_eq!(config_key, "public@@DEFAULT_GROUP@@test");

        let permission_key = RocksStateMachine::permission_key("admin", "public::*", "rw");
        assert_eq!(permission_key, "admin@@public::*@@rw");
    }
}
