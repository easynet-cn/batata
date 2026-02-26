//! Apollo advanced features models
//!
//! Models for namespace locking, gray release, access keys, and metrics.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// Namespace Lock
// ============================================================================

/// Namespace lock status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NamespaceLock {
    /// Namespace name
    pub namespace_name: String,
    /// Whether the namespace is locked
    pub is_locked: bool,
    /// User who locked the namespace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_by: Option<String>,
    /// Lock timestamp (milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_time: Option<i64>,
    /// Lock expiration timestamp (milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire_time: Option<i64>,
}

impl NamespaceLock {
    /// Create an unlocked namespace lock status
    pub fn unlocked(namespace_name: String) -> Self {
        Self {
            namespace_name,
            is_locked: false,
            locked_by: None,
            lock_time: None,
            expire_time: None,
        }
    }

    /// Create a locked namespace lock status
    pub fn locked(namespace_name: String, locked_by: String, lock_time: i64, ttl_ms: i64) -> Self {
        Self {
            namespace_name,
            is_locked: true,
            locked_by: Some(locked_by),
            lock_time: Some(lock_time),
            expire_time: Some(lock_time + ttl_ms),
        }
    }

    /// Check if the lock has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expire_time) = self.expire_time {
            let now = chrono::Utc::now().timestamp_millis();
            now > expire_time
        } else {
            true
        }
    }
}

/// Request to acquire a namespace lock
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquireLockRequest {
    /// User requesting the lock
    pub locked_by: String,
    /// Lock TTL in milliseconds (default: 5 minutes)
    #[serde(default = "default_lock_ttl")]
    pub ttl_ms: i64,
}

fn default_lock_ttl() -> i64 {
    5 * 60 * 1000 // 5 minutes
}

// ============================================================================
// Gray Release
// ============================================================================

/// Gray release rule
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrayReleaseRule {
    /// Rule name
    pub name: String,
    /// Application ID
    pub app_id: String,
    /// Cluster name
    pub cluster_name: String,
    /// Namespace name
    pub namespace_name: String,
    /// Branch name for gray release
    pub branch_name: String,
    /// Gray release rules (IP list, labels, etc.)
    pub rules: Vec<GrayRule>,
    /// Release status
    pub release_status: GrayReleaseStatus,
}

/// Individual gray rule
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrayRule {
    /// Rule type (ip, label, percentage)
    pub rule_type: GrayRuleType,
    /// Rule value (IP list, label key=value, percentage)
    pub value: String,
}

/// Gray rule types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GrayRuleType {
    /// Match by client IP
    Ip,
    /// Match by label
    Label,
    /// Match by percentage
    Percentage,
}

/// Gray release status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GrayReleaseStatus {
    /// Gray release is active
    Active,
    /// Gray release is merged to main
    Merged,
    /// Gray release is abandoned
    Abandoned,
}

/// Request to create a gray release
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGrayReleaseRequest {
    /// Branch name
    pub branch_name: String,
    /// Gray rules
    pub rules: Vec<GrayRule>,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Created by
    pub created_by: String,
}

// ============================================================================
// Access Key Authentication
// ============================================================================

/// Access key for API authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessKey {
    /// Access key ID
    pub id: String,
    /// Secret (only returned on creation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// Application ID this key is associated with
    pub app_id: String,
    /// Whether the key is enabled
    pub enabled: bool,
    /// Created timestamp
    pub create_time: i64,
    /// Last used timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_time: Option<i64>,
}

impl AccessKey {
    /// Create a new access key
    pub fn new(app_id: String) -> Self {
        let id = uuid::Uuid::new_v4().to_string().replace("-", "");
        let secret = uuid::Uuid::new_v4().to_string().replace("-", "");
        let now = chrono::Utc::now().timestamp_millis();

        Self {
            id,
            secret: Some(secret),
            app_id,
            enabled: true,
            create_time: now,
            last_used_time: None,
        }
    }

    /// Hide the secret (for listing)
    pub fn without_secret(mut self) -> Self {
        self.secret = None;
        self
    }
}

/// Request to create an access key
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccessKeyRequest {
    /// Application ID
    pub app_id: String,
}

/// Signature verification request
#[derive(Debug, Clone, Deserialize)]
pub struct SignatureVerifyRequest {
    /// Access key ID
    pub access_key_id: String,
    /// Signature
    pub signature: String,
    /// Timestamp
    pub timestamp: i64,
    /// Request path
    pub path: String,
}

// ============================================================================
// Client Metrics
// ============================================================================

/// Apollo client connection info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConnection {
    /// Client IP
    pub ip: String,
    /// Application ID
    pub app_id: String,
    /// Cluster name
    pub cluster: String,
    /// Namespaces being watched
    pub namespaces: Vec<String>,
    /// Connection time
    pub connect_time: i64,
    /// Last heartbeat time
    pub last_heartbeat: i64,
    /// Notification ID for each namespace
    pub notification_ids: HashMap<String, i64>,
}

impl ClientConnection {
    /// Create a new client connection
    pub fn new(ip: String, app_id: String, cluster: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            ip,
            app_id,
            cluster,
            namespaces: Vec::new(),
            connect_time: now,
            last_heartbeat: now,
            notification_ids: HashMap::new(),
        }
    }

    /// Update heartbeat
    pub fn heartbeat(&mut self) {
        self.last_heartbeat = chrono::Utc::now().timestamp_millis();
    }

    /// Add a watched namespace
    pub fn watch_namespace(&mut self, namespace: String, notification_id: i64) {
        if !self.namespaces.contains(&namespace) {
            self.namespaces.push(namespace.clone());
        }
        self.notification_ids.insert(namespace, notification_id);
    }
}

/// Client metrics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct ClientMetrics {
    /// Total connected clients
    pub total_clients: u64,
    /// Clients by application
    pub clients_by_app: HashMap<String, u64>,
    /// Clients by cluster
    pub clients_by_cluster: HashMap<String, u64>,
    /// Namespace watch counts
    pub namespace_watch_counts: HashMap<String, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_lock_unlocked() {
        let lock = NamespaceLock::unlocked("application".to_string());
        assert!(!lock.is_locked);
        assert!(lock.locked_by.is_none());
        assert!(lock.lock_time.is_none());
        assert!(lock.expire_time.is_none());
    }

    #[test]
    fn test_namespace_lock_locked() {
        let now = chrono::Utc::now().timestamp_millis();
        let lock =
            NamespaceLock::locked("application".to_string(), "user1".to_string(), now, 60000);
        assert!(lock.is_locked);
        assert_eq!(lock.locked_by, Some("user1".to_string()));
        assert!(!lock.is_expired());
        assert_eq!(lock.expire_time, Some(now + 60000));
    }

    #[test]
    fn test_namespace_lock_expired() {
        let past = chrono::Utc::now().timestamp_millis() - 120000; // 2 minutes ago
        let lock =
            NamespaceLock::locked("application".to_string(), "user1".to_string(), past, 60000);
        assert!(lock.is_expired());
    }

    #[test]
    fn test_namespace_lock_no_expire_is_expired() {
        // A lock with no expire_time is considered expired
        let lock = NamespaceLock {
            namespace_name: "test".to_string(),
            is_locked: true,
            locked_by: Some("user1".to_string()),
            lock_time: Some(chrono::Utc::now().timestamp_millis()),
            expire_time: None,
        };
        assert!(lock.is_expired());
    }

    #[test]
    fn test_namespace_lock_serialization() {
        let lock = NamespaceLock::unlocked("app".to_string());
        let json = serde_json::to_string(&lock).unwrap();
        assert!(json.contains("\"namespaceName\":\"app\""));
        assert!(json.contains("\"isLocked\":false"));
        // None fields should be skipped
        assert!(!json.contains("lockedBy"));
    }

    #[test]
    fn test_acquire_lock_request() {
        let json = r#"{"lockedBy": "admin", "ttlMs": 300000}"#;
        let req: AcquireLockRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.locked_by, "admin");
        assert_eq!(req.ttl_ms, 300000);
    }

    #[test]
    fn test_acquire_lock_request_default_ttl() {
        let json = r#"{"lockedBy": "admin"}"#;
        let req: AcquireLockRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.ttl_ms, 5 * 60 * 1000); // 5 minutes default
    }

    #[test]
    fn test_access_key_creation() {
        let key = AccessKey::new("test-app".to_string());
        assert!(!key.id.is_empty());
        assert!(key.secret.is_some());
        assert_eq!(key.app_id, "test-app");
        assert!(key.enabled);
        assert!(key.create_time > 0);
        assert!(key.last_used_time.is_none());
    }

    #[test]
    fn test_access_key_id_format() {
        let key = AccessKey::new("app1".to_string());
        // UUID without dashes = 32 hex chars
        assert_eq!(key.id.len(), 32);
        assert!(key.id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_access_key_secret_format() {
        let key = AccessKey::new("app1".to_string());
        let secret = key.secret.unwrap();
        assert_eq!(secret.len(), 32);
        assert!(secret.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_access_key_without_secret() {
        let key = AccessKey::new("test-app".to_string()).without_secret();
        assert!(key.secret.is_none());
        // Other fields preserved
        assert_eq!(key.app_id, "test-app");
        assert!(key.enabled);
    }

    #[test]
    fn test_access_key_serialization_with_secret() {
        let key = AccessKey::new("app1".to_string());
        let json = serde_json::to_string(&key).unwrap();
        assert!(json.contains("\"secret\":"));
        assert!(json.contains("\"appId\":\"app1\""));
    }

    #[test]
    fn test_access_key_serialization_without_secret() {
        let key = AccessKey::new("app1".to_string()).without_secret();
        let json = serde_json::to_string(&key).unwrap();
        // Secret should be skipped when None
        assert!(!json.contains("\"secret\""));
    }

    #[test]
    fn test_client_connection() {
        let mut conn = ClientConnection::new(
            "192.168.1.1".to_string(),
            "test-app".to_string(),
            "default".to_string(),
        );
        conn.watch_namespace("application".to_string(), 100);
        assert_eq!(conn.namespaces.len(), 1);
        assert_eq!(conn.notification_ids.get("application"), Some(&100));
    }

    #[test]
    fn test_client_connection_duplicate_watch() {
        let mut conn = ClientConnection::new(
            "10.0.0.1".to_string(),
            "app1".to_string(),
            "default".to_string(),
        );
        conn.watch_namespace("ns1".to_string(), 1);
        conn.watch_namespace("ns1".to_string(), 2); // Same namespace, updated id
        assert_eq!(conn.namespaces.len(), 1); // No duplicate
        assert_eq!(conn.notification_ids.get("ns1"), Some(&2)); // Updated
    }

    #[test]
    fn test_client_connection_multiple_namespaces() {
        let mut conn = ClientConnection::new(
            "10.0.0.1".to_string(),
            "app1".to_string(),
            "default".to_string(),
        );
        conn.watch_namespace("ns1".to_string(), 1);
        conn.watch_namespace("ns2".to_string(), 2);
        conn.watch_namespace("ns3".to_string(), 3);
        assert_eq!(conn.namespaces.len(), 3);
    }

    #[test]
    fn test_client_connection_heartbeat() {
        let mut conn = ClientConnection::new(
            "10.0.0.1".to_string(),
            "app1".to_string(),
            "default".to_string(),
        );
        let initial_heartbeat = conn.last_heartbeat;
        // Small sleep to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(10));
        conn.heartbeat();
        assert!(conn.last_heartbeat >= initial_heartbeat);
    }

    #[test]
    fn test_gray_rule_serialization() {
        let rule = GrayRule {
            rule_type: GrayRuleType::Ip,
            value: "192.168.1.0/24".to_string(),
        };
        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains("\"ruleType\":\"ip\""));
        assert!(json.contains("\"value\":\"192.168.1.0/24\""));
    }

    #[test]
    fn test_gray_rule_types() {
        let ip_rule = GrayRule {
            rule_type: GrayRuleType::Ip,
            value: "10.0.0.1".to_string(),
        };
        let label_rule = GrayRule {
            rule_type: GrayRuleType::Label,
            value: "env=staging".to_string(),
        };
        let pct_rule = GrayRule {
            rule_type: GrayRuleType::Percentage,
            value: "20".to_string(),
        };

        assert_eq!(ip_rule.rule_type, GrayRuleType::Ip);
        assert_eq!(label_rule.rule_type, GrayRuleType::Label);
        assert_eq!(pct_rule.rule_type, GrayRuleType::Percentage);
    }

    #[test]
    fn test_gray_rule_deserialization() {
        let json = r#"{"ruleType":"label","value":"env=prod"}"#;
        let rule: GrayRule = serde_json::from_str(json).unwrap();
        assert_eq!(rule.rule_type, GrayRuleType::Label);
        assert_eq!(rule.value, "env=prod");
    }

    #[test]
    fn test_gray_release_rule_serialization() {
        let rule = GrayReleaseRule {
            name: "app-gray-branch1".to_string(),
            app_id: "app1".to_string(),
            cluster_name: "default".to_string(),
            namespace_name: "application".to_string(),
            branch_name: "branch1".to_string(),
            rules: vec![GrayRule {
                rule_type: GrayRuleType::Ip,
                value: "10.0.0.1".to_string(),
            }],
            release_status: GrayReleaseStatus::Active,
        };
        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains("\"releaseStatus\":\"active\""));
        assert!(json.contains("\"branchName\":\"branch1\""));
    }

    #[test]
    fn test_gray_release_status_variants() {
        let active = serde_json::to_string(&GrayReleaseStatus::Active).unwrap();
        let merged = serde_json::to_string(&GrayReleaseStatus::Merged).unwrap();
        let abandoned = serde_json::to_string(&GrayReleaseStatus::Abandoned).unwrap();
        assert_eq!(active, "\"active\"");
        assert_eq!(merged, "\"merged\"");
        assert_eq!(abandoned, "\"abandoned\"");
    }

    #[test]
    fn test_create_gray_release_request() {
        let json = r#"{
            "branchName": "gray-v2",
            "rules": [{"ruleType": "ip", "value": "10.0.0.1"}],
            "comment": "Testing gray",
            "createdBy": "admin"
        }"#;
        let req: CreateGrayReleaseRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.branch_name, "gray-v2");
        assert_eq!(req.rules.len(), 1);
        assert_eq!(req.created_by, "admin");
    }

    #[test]
    fn test_signature_verify_request() {
        let json = r#"{
            "access_key_id": "key123",
            "signature": "abc123",
            "timestamp": 1700000000000,
            "path": "/configs/app1/default/application"
        }"#;
        let req: SignatureVerifyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.access_key_id, "key123");
        assert_eq!(req.timestamp, 1700000000000);
    }

    #[test]
    fn test_client_metrics_default() {
        let metrics = ClientMetrics::default();
        assert_eq!(metrics.total_clients, 0);
        assert!(metrics.clients_by_app.is_empty());
        assert!(metrics.clients_by_cluster.is_empty());
        assert!(metrics.namespace_watch_counts.is_empty());
    }

    #[test]
    fn test_create_access_key_request() {
        let json = r#"{"appId": "test-app"}"#;
        let req: CreateAccessKeyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.app_id, "test-app");
    }
}
