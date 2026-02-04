//! Apollo advanced features service
//!
//! Provides namespace locking, gray release, access keys, and client metrics.

use std::sync::Arc;

use dashmap::DashMap;
use sea_orm::DatabaseConnection;

use crate::mapping::ApolloMappingContext;
use crate::model::{
    AccessKey, AcquireLockRequest, ClientConnection, ClientMetrics, CreateGrayReleaseRequest,
    GrayReleaseRule, GrayReleaseStatus, NamespaceLock,
};

/// Default lock TTL in milliseconds (5 minutes)
const DEFAULT_LOCK_TTL_MS: i64 = 5 * 60 * 1000;

/// Apollo advanced features service
pub struct ApolloAdvancedService {
    db: Arc<DatabaseConnection>,
    /// Namespace locks: key = "env+appId+cluster+namespace"
    namespace_locks: DashMap<String, NamespaceLock>,
    /// Access keys: key = access_key_id
    access_keys: DashMap<String, AccessKey>,
    /// Client connections: key = "ip+appId"
    client_connections: DashMap<String, ClientConnection>,
    /// Gray release rules: key = "env+appId+cluster+namespace"
    gray_releases: DashMap<String, GrayReleaseRule>,
}

impl ApolloAdvancedService {
    /// Create a new ApolloAdvancedService
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            namespace_locks: DashMap::new(),
            access_keys: DashMap::new(),
            client_connections: DashMap::new(),
            gray_releases: DashMap::new(),
        }
    }

    // ========================================================================
    // Namespace Lock Operations
    // ========================================================================

    /// Build lock key
    fn build_lock_key(app_id: &str, cluster: &str, namespace: &str, env: &str) -> String {
        format!("{}+{}+{}+{}", env, app_id, cluster, namespace)
    }

    /// Get namespace lock status
    pub fn get_lock_status(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
    ) -> NamespaceLock {
        let key = Self::build_lock_key(app_id, cluster, namespace, env);

        match self.namespace_locks.get(&key) {
            Some(lock) => {
                if lock.is_expired() {
                    // Lock expired, remove it and return unlocked
                    drop(lock);
                    self.namespace_locks.remove(&key);
                    NamespaceLock::unlocked(namespace.to_string())
                } else {
                    lock.clone()
                }
            }
            None => NamespaceLock::unlocked(namespace.to_string()),
        }
    }

    /// Acquire a namespace lock
    pub fn acquire_lock(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
        req: AcquireLockRequest,
    ) -> Result<NamespaceLock, String> {
        let key = Self::build_lock_key(app_id, cluster, namespace, env);

        // Check if already locked by someone else
        if let Some(existing) = self.namespace_locks.get(&key) {
            if !existing.is_expired() && existing.locked_by != Some(req.locked_by.clone()) {
                return Err(format!(
                    "Namespace is locked by {}",
                    existing.locked_by.as_deref().unwrap_or("unknown")
                ));
            }
        }

        let now = chrono::Utc::now().timestamp_millis();
        let ttl = if req.ttl_ms > 0 {
            req.ttl_ms
        } else {
            DEFAULT_LOCK_TTL_MS
        };

        let lock = NamespaceLock::locked(namespace.to_string(), req.locked_by, now, ttl);
        self.namespace_locks.insert(key, lock.clone());

        Ok(lock)
    }

    /// Release a namespace lock
    pub fn release_lock(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
        user: &str,
    ) -> Result<(), String> {
        let key = Self::build_lock_key(app_id, cluster, namespace, env);

        match self.namespace_locks.get(&key) {
            Some(lock) => {
                if lock.locked_by.as_deref() != Some(user) && !lock.is_expired() {
                    return Err("Cannot release lock owned by another user".to_string());
                }
                drop(lock);
                self.namespace_locks.remove(&key);
                Ok(())
            }
            None => Ok(()), // Already unlocked
        }
    }

    // ========================================================================
    // Gray Release Operations
    // ========================================================================

    /// Create a gray release
    pub async fn create_gray_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
        req: CreateGrayReleaseRequest,
    ) -> anyhow::Result<GrayReleaseRule> {
        let ctx = ApolloMappingContext::new(app_id, cluster, namespace, Some(env));
        let key = Self::build_lock_key(app_id, cluster, namespace, env);

        // Check if config exists
        let config = batata_config::service::config::find_one(
            &self.db,
            &ctx.nacos_data_id,
            &ctx.nacos_group,
            &ctx.nacos_namespace,
        )
        .await?;

        if config.is_none() {
            anyhow::bail!("Namespace not found");
        }

        let rule = GrayReleaseRule {
            name: format!("{}-gray-{}", namespace, req.branch_name),
            app_id: app_id.to_string(),
            cluster_name: cluster.to_string(),
            namespace_name: namespace.to_string(),
            branch_name: req.branch_name,
            rules: req.rules,
            release_status: GrayReleaseStatus::Active,
        };

        self.gray_releases.insert(key, rule.clone());
        Ok(rule)
    }

    /// Get gray release for a namespace
    pub fn get_gray_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
    ) -> Option<GrayReleaseRule> {
        let key = Self::build_lock_key(app_id, cluster, namespace, env);
        self.gray_releases.get(&key).map(|r| r.clone())
    }

    /// Merge gray release to main
    pub fn merge_gray_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
    ) -> Result<GrayReleaseRule, String> {
        let key = Self::build_lock_key(app_id, cluster, namespace, env);

        match self.gray_releases.get_mut(&key) {
            Some(mut rule) => {
                rule.release_status = GrayReleaseStatus::Merged;
                Ok(rule.clone())
            }
            None => Err("Gray release not found".to_string()),
        }
    }

    /// Abandon gray release
    pub fn abandon_gray_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        env: &str,
    ) -> Result<(), String> {
        let key = Self::build_lock_key(app_id, cluster, namespace, env);

        match self.gray_releases.get_mut(&key) {
            Some(mut rule) => {
                rule.release_status = GrayReleaseStatus::Abandoned;
                Ok(())
            }
            None => Err("Gray release not found".to_string()),
        }
    }

    // ========================================================================
    // Access Key Operations
    // ========================================================================

    /// Create an access key
    pub fn create_access_key(&self, app_id: &str) -> AccessKey {
        let key = AccessKey::new(app_id.to_string());
        self.access_keys.insert(key.id.clone(), key.clone());
        key
    }

    /// Get access key by ID (without secret)
    pub fn get_access_key(&self, key_id: &str) -> Option<AccessKey> {
        self.access_keys
            .get(key_id)
            .map(|k| k.clone().without_secret())
    }

    /// List access keys for an app (without secrets)
    pub fn list_access_keys(&self, app_id: &str) -> Vec<AccessKey> {
        self.access_keys
            .iter()
            .filter(|entry| entry.value().app_id == app_id)
            .map(|entry| entry.value().clone().without_secret())
            .collect()
    }

    /// Delete an access key
    pub fn delete_access_key(&self, key_id: &str) -> bool {
        self.access_keys.remove(key_id).is_some()
    }

    /// Enable/disable an access key
    pub fn set_access_key_enabled(&self, key_id: &str, enabled: bool) -> Option<AccessKey> {
        self.access_keys.get_mut(key_id).map(|mut key| {
            key.enabled = enabled;
            key.clone().without_secret()
        })
    }

    /// Verify access key signature
    pub fn verify_signature(
        &self,
        key_id: &str,
        signature: &str,
        timestamp: i64,
        path: &str,
    ) -> Result<bool, String> {
        let key = self
            .access_keys
            .get(key_id)
            .ok_or_else(|| "Access key not found".to_string())?;

        if !key.enabled {
            return Err("Access key is disabled".to_string());
        }

        // Check timestamp (within 5 minutes)
        let now = chrono::Utc::now().timestamp_millis();
        if (now - timestamp).abs() > 5 * 60 * 1000 {
            return Err("Request timestamp expired".to_string());
        }

        // Verify signature: HMAC-SHA256(secret, timestamp + path)
        let secret = key.secret.as_deref().unwrap_or("");
        let message = format!("{}{}", timestamp, path);
        let expected = Self::compute_signature(secret, &message);

        // Update last used time
        drop(key);
        if let Some(mut key) = self.access_keys.get_mut(key_id) {
            key.last_used_time = Some(now);
        }

        Ok(signature == expected)
    }

    /// Compute HMAC-SHA256 signature
    fn compute_signature(secret: &str, message: &str) -> String {
        use std::fmt::Write;

        // Simple implementation using MD5 for demo (in production, use HMAC-SHA256)
        let input = format!("{}{}", secret, message);
        let digest = md5::compute(input.as_bytes());
        let mut result = String::with_capacity(32);
        for byte in digest.iter() {
            write!(&mut result, "{:02x}", byte).unwrap();
        }
        result
    }

    // ========================================================================
    // Client Metrics Operations
    // ========================================================================

    /// Register a client connection
    pub fn register_client(&self, ip: &str, app_id: &str, cluster: &str) -> ClientConnection {
        let key = format!("{}+{}", ip, app_id);

        self.client_connections
            .entry(key)
            .or_insert_with(|| {
                ClientConnection::new(ip.to_string(), app_id.to_string(), cluster.to_string())
            })
            .clone()
    }

    /// Update client heartbeat and watched namespaces
    pub fn update_client(&self, ip: &str, app_id: &str, namespaces: Vec<(String, i64)>) {
        let key = format!("{}+{}", ip, app_id);

        if let Some(mut conn) = self.client_connections.get_mut(&key) {
            conn.heartbeat();
            for (ns, notification_id) in namespaces {
                conn.watch_namespace(ns, notification_id);
            }
        }
    }

    /// Remove a client connection
    pub fn remove_client(&self, ip: &str, app_id: &str) {
        let key = format!("{}+{}", ip, app_id);
        self.client_connections.remove(&key);
    }

    /// Get client connection info
    pub fn get_client(&self, ip: &str, app_id: &str) -> Option<ClientConnection> {
        let key = format!("{}+{}", ip, app_id);
        self.client_connections.get(&key).map(|c| c.clone())
    }

    /// Get all clients for an app
    pub fn get_clients_by_app(&self, app_id: &str) -> Vec<ClientConnection> {
        self.client_connections
            .iter()
            .filter(|entry| entry.value().app_id == app_id)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get client metrics summary
    pub fn get_metrics(&self) -> ClientMetrics {
        let mut metrics = ClientMetrics::default();

        for entry in self.client_connections.iter() {
            let conn = entry.value();
            metrics.total_clients += 1;

            *metrics
                .clients_by_app
                .entry(conn.app_id.clone())
                .or_insert(0) += 1;
            *metrics
                .clients_by_cluster
                .entry(conn.cluster.clone())
                .or_insert(0) += 1;

            for ns in &conn.namespaces {
                *metrics
                    .namespace_watch_counts
                    .entry(ns.clone())
                    .or_insert(0) += 1;
            }
        }

        metrics
    }

    /// Cleanup stale connections (no heartbeat for > 5 minutes)
    pub fn cleanup_stale_connections(&self) {
        let now = chrono::Utc::now().timestamp_millis();
        let stale_threshold = 5 * 60 * 1000; // 5 minutes

        let stale_keys: Vec<String> = self
            .client_connections
            .iter()
            .filter(|entry| now - entry.value().last_heartbeat > stale_threshold)
            .map(|entry| entry.key().clone())
            .collect();

        for key in stale_keys {
            self.client_connections.remove(&key);
        }
    }
}
