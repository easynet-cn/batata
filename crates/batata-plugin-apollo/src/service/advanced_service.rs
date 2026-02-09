//! Apollo advanced features service
//!
//! Provides namespace locking, gray release, access keys, and client metrics.
//! All state is persisted in Apollo-specific database tables.

use std::sync::Arc;

use batata_persistence::entity::{apollo_access_key, apollo_instance, apollo_namespace_lock};
use sea_orm::{DatabaseConnection, Set};

use crate::model::{
    AccessKey, AcquireLockRequest, ClientConnection, ClientMetrics, CreateGrayReleaseRequest,
    GrayReleaseRule, GrayReleaseStatus, NamespaceLock,
};
use crate::repository::{
    access_key_repository, gray_release_repository, instance_repository, lock_repository,
    namespace_repository,
};

/// Default lock TTL in milliseconds (5 minutes)
const DEFAULT_LOCK_TTL_MS: i64 = 5 * 60 * 1000;

/// Apollo advanced features service
pub struct ApolloAdvancedService {
    db: Arc<DatabaseConnection>,
}

impl ApolloAdvancedService {
    /// Create a new ApolloAdvancedService
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Namespace Lock Operations
    // ========================================================================

    /// Get namespace lock status
    pub async fn get_lock_status(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
    ) -> NamespaceLock {
        let inst = namespace_repository::find_instance(&self.db, app_id, cluster, namespace).await;

        let ns_id = match inst {
            Ok(Some(ns)) => ns.id,
            _ => return NamespaceLock::unlocked(namespace.to_string()),
        };

        match lock_repository::find_by_namespace(&self.db, ns_id).await {
            Ok(Some(lock)) => {
                let lock_result = NamespaceLock {
                    namespace_name: namespace.to_string(),
                    is_locked: true,
                    locked_by: lock.locked_by,
                    lock_time: lock.lock_time.map(|t| t.and_utc().timestamp_millis()),
                    expire_time: lock.expire_time.map(|t| t.and_utc().timestamp_millis()),
                };
                if lock_result.is_expired() {
                    let _ = lock_repository::release(&self.db, ns_id).await;
                    NamespaceLock::unlocked(namespace.to_string())
                } else {
                    lock_result
                }
            }
            _ => NamespaceLock::unlocked(namespace.to_string()),
        }
    }

    /// Acquire a namespace lock
    pub async fn acquire_lock(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
        req: AcquireLockRequest,
    ) -> Result<NamespaceLock, String> {
        let inst = namespace_repository::find_instance(&self.db, app_id, cluster, namespace)
            .await
            .map_err(|e| e.to_string())?;

        let ns = inst.ok_or_else(|| "Namespace not found".to_string())?;

        // Check existing lock
        if let Ok(Some(existing)) = lock_repository::find_by_namespace(&self.db, ns.id).await {
            let existing_lock = NamespaceLock {
                namespace_name: namespace.to_string(),
                is_locked: true,
                locked_by: existing.locked_by.clone(),
                lock_time: existing.lock_time.map(|t| t.and_utc().timestamp_millis()),
                expire_time: existing.expire_time.map(|t| t.and_utc().timestamp_millis()),
            };
            if !existing_lock.is_expired() && existing.locked_by.as_deref() != Some(&req.locked_by)
            {
                return Err(format!(
                    "Namespace is locked by {}",
                    existing.locked_by.as_deref().unwrap_or("unknown")
                ));
            }
            // Release expired or own lock
            let _ = lock_repository::release(&self.db, ns.id).await;
        }

        let now = chrono::Utc::now();
        let ttl = if req.ttl_ms > 0 {
            req.ttl_ms
        } else {
            DEFAULT_LOCK_TTL_MS
        };
        let expire = now + chrono::Duration::milliseconds(ttl);

        let model = apollo_namespace_lock::ActiveModel {
            namespace_id: Set(ns.id),
            locked_by: Set(Some(req.locked_by.clone())),
            lock_time: Set(Some(now.naive_utc())),
            expire_time: Set(Some(expire.naive_utc())),
            created_by: Set(Some(req.locked_by.clone())),
            ..Default::default()
        };

        lock_repository::acquire(&self.db, model)
            .await
            .map_err(|e| e.to_string())?;

        Ok(NamespaceLock::locked(
            namespace.to_string(),
            req.locked_by,
            now.timestamp_millis(),
            ttl,
        ))
    }

    /// Release a namespace lock
    pub async fn release_lock(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
        user: &str,
    ) -> Result<(), String> {
        let inst = namespace_repository::find_instance(&self.db, app_id, cluster, namespace)
            .await
            .map_err(|e| e.to_string())?;

        let ns = match inst {
            Some(ns) => ns,
            None => return Ok(()),
        };

        if let Ok(Some(existing)) = lock_repository::find_by_namespace(&self.db, ns.id).await {
            let existing_lock = NamespaceLock {
                namespace_name: namespace.to_string(),
                is_locked: true,
                locked_by: existing.locked_by.clone(),
                lock_time: existing.lock_time.map(|t| t.and_utc().timestamp_millis()),
                expire_time: existing.expire_time.map(|t| t.and_utc().timestamp_millis()),
            };
            if existing.locked_by.as_deref() != Some(user) && !existing_lock.is_expired() {
                return Err("Cannot release lock owned by another user".to_string());
            }
        }

        lock_repository::release(&self.db, ns.id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
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
        _env: &str,
        req: CreateGrayReleaseRequest,
    ) -> anyhow::Result<GrayReleaseRule> {
        let rules_json = serde_json::to_string(&req.rules)?;

        let model = batata_persistence::entity::apollo_gray_release_rule::ActiveModel {
            app_id: Set(app_id.to_string()),
            cluster_name: Set(cluster.to_string()),
            namespace_name: Set(namespace.to_string()),
            branch_name: Set(req.branch_name.clone()),
            rules: Set(Some(rules_json)),
            release_id: Set(0),
            branch_status: Set(0), // Active
            created_by: Set(Some(req.created_by.clone())),
            last_modified_by: Set(Some(req.created_by)),
            ..Default::default()
        };
        gray_release_repository::create(&self.db, model).await?;

        Ok(GrayReleaseRule {
            name: format!("{}-gray-{}", namespace, req.branch_name),
            app_id: app_id.to_string(),
            cluster_name: cluster.to_string(),
            namespace_name: namespace.to_string(),
            branch_name: req.branch_name,
            rules: req.rules,
            release_status: GrayReleaseStatus::Active,
        })
    }

    /// Get gray release for a namespace
    pub async fn get_gray_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
    ) -> Option<GrayReleaseRule> {
        let rules =
            gray_release_repository::find_by_namespace(&self.db, app_id, cluster, namespace)
                .await
                .ok()?;

        rules.into_iter().next().map(|r| {
            let parsed_rules = r
                .rules
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();

            GrayReleaseRule {
                name: format!("{}-gray-{}", namespace, r.branch_name),
                app_id: r.app_id,
                cluster_name: r.cluster_name,
                namespace_name: r.namespace_name,
                branch_name: r.branch_name,
                rules: parsed_rules,
                release_status: match r.branch_status {
                    1 => GrayReleaseStatus::Merged,
                    2 => GrayReleaseStatus::Abandoned,
                    _ => GrayReleaseStatus::Active,
                },
            }
        })
    }

    /// Merge gray release to main
    pub async fn merge_gray_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
    ) -> Result<GrayReleaseRule, String> {
        let rules =
            gray_release_repository::find_by_namespace(&self.db, app_id, cluster, namespace)
                .await
                .map_err(|e| e.to_string())?;

        let rule = rules
            .into_iter()
            .next()
            .ok_or_else(|| "Gray release not found".to_string())?;

        gray_release_repository::update_status(&self.db, rule.id, 1) // Merged
            .await
            .map_err(|e| e.to_string())?;

        let parsed_rules = rule
            .rules
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        Ok(GrayReleaseRule {
            name: format!("{}-gray-{}", namespace, rule.branch_name),
            app_id: rule.app_id,
            cluster_name: rule.cluster_name,
            namespace_name: rule.namespace_name,
            branch_name: rule.branch_name,
            rules: parsed_rules,
            release_status: GrayReleaseStatus::Merged,
        })
    }

    /// Abandon gray release
    pub async fn abandon_gray_release(
        &self,
        app_id: &str,
        cluster: &str,
        namespace: &str,
        _env: &str,
    ) -> Result<(), String> {
        let rules =
            gray_release_repository::find_by_namespace(&self.db, app_id, cluster, namespace)
                .await
                .map_err(|e| e.to_string())?;

        let rule = rules
            .into_iter()
            .next()
            .ok_or_else(|| "Gray release not found".to_string())?;

        gray_release_repository::update_status(&self.db, rule.id, 2) // Abandoned
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    // ========================================================================
    // Access Key Operations
    // ========================================================================

    /// Create an access key
    pub async fn create_access_key(&self, app_id: &str) -> AccessKey {
        let key = AccessKey::new(app_id.to_string());

        let model = apollo_access_key::ActiveModel {
            app_id: Set(app_id.to_string()),
            secret: Set(key.secret.clone().unwrap_or_default()),
            mode: Set(Some("STS".to_string())),
            is_enabled: Set(true),
            created_by: Set(Some(app_id.to_string())),
            ..Default::default()
        };

        let _ = access_key_repository::create(&self.db, model).await;
        key
    }

    /// Get access key by ID (without secret)
    pub async fn get_access_key(&self, key_id: &str) -> Option<AccessKey> {
        let id: i64 = key_id.parse().ok()?;
        let model = access_key_repository::find_by_id(&self.db, id)
            .await
            .ok()??;
        Some(Self::model_to_access_key(model).without_secret())
    }

    /// List access keys for an app (without secrets)
    pub async fn list_access_keys(&self, app_id: &str) -> Vec<AccessKey> {
        let models = access_key_repository::find_by_app(&self.db, app_id)
            .await
            .unwrap_or_default();
        models
            .into_iter()
            .map(|m| Self::model_to_access_key(m).without_secret())
            .collect()
    }

    /// Delete an access key
    pub async fn delete_access_key(&self, key_id: &str) -> bool {
        let id: i64 = match key_id.parse() {
            Ok(id) => id,
            Err(_) => return false,
        };
        access_key_repository::soft_delete(&self.db, id)
            .await
            .unwrap_or(false)
    }

    /// Enable/disable an access key
    pub async fn set_access_key_enabled(&self, key_id: &str, enabled: bool) -> Option<AccessKey> {
        let id: i64 = key_id.parse().ok()?;
        access_key_repository::set_enabled(&self.db, id, enabled)
            .await
            .ok()?;
        let model = access_key_repository::find_by_id(&self.db, id)
            .await
            .ok()??;
        Some(Self::model_to_access_key(model).without_secret())
    }

    /// Verify access key signature
    pub fn verify_signature(
        &self,
        _key_id: &str,
        signature: &str,
        timestamp: i64,
        path: &str,
    ) -> Result<bool, String> {
        // Check timestamp (within 5 minutes)
        let now = chrono::Utc::now().timestamp_millis();
        if (now - timestamp).abs() > 5 * 60 * 1000 {
            return Err("Request timestamp expired".to_string());
        }

        // For now, just verify format is correct
        // In production, look up key and verify HMAC
        Ok(!signature.is_empty() && !path.is_empty())
    }

    // ========================================================================
    // Client Metrics Operations
    // ========================================================================

    /// Register a client connection
    pub async fn register_client(&self, ip: &str, app_id: &str, cluster: &str) -> ClientConnection {
        let model = apollo_instance::ActiveModel {
            app_id: Set(app_id.to_string()),
            cluster_name: Set(cluster.to_string()),
            ip: Set(ip.to_string()),
            created_by: Set(Some(ip.to_string())),
            ..Default::default()
        };
        let _ = instance_repository::upsert_instance(&self.db, model).await;

        ClientConnection::new(ip.to_string(), app_id.to_string(), cluster.to_string())
    }

    /// Get all clients for an app
    pub async fn get_clients_by_app(&self, app_id: &str) -> Vec<ClientConnection> {
        let instances = instance_repository::find_instances_by_app(&self.db, app_id)
            .await
            .unwrap_or_default();

        instances
            .into_iter()
            .map(|inst| ClientConnection::new(inst.ip, inst.app_id, inst.cluster_name))
            .collect()
    }

    /// Get client metrics summary
    pub async fn get_metrics(&self) -> ClientMetrics {
        // Return empty metrics for now - full implementation requires
        // querying instance and instance_config tables
        ClientMetrics::default()
    }

    /// Cleanup stale connections
    pub async fn cleanup_stale_connections(&self) {
        // No-op for DB-backed implementation since instances are persistent
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    fn model_to_access_key(
        model: batata_persistence::entity::apollo_access_key::Model,
    ) -> AccessKey {
        AccessKey {
            id: model.id.to_string(),
            secret: Some(model.secret),
            app_id: model.app_id,
            enabled: model.is_enabled,
            create_time: model
                .created_time
                .map(|t| t.and_utc().timestamp_millis())
                .unwrap_or(0),
            last_used_time: None,
        }
    }
}
