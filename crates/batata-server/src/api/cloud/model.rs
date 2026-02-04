//! Cloud Native Integration Data Models
//!
//! Data models for Kubernetes sync and Prometheus service discovery.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// =============================================================================
// Kubernetes Models
// =============================================================================

/// Kubernetes service representation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sService {
    /// Service name
    pub name: String,

    /// Kubernetes namespace
    pub namespace: String,

    /// Service type (ClusterIP, NodePort, LoadBalancer)
    pub service_type: String,

    /// Cluster IP
    pub cluster_ip: Option<String>,

    /// External IPs
    #[serde(default)]
    pub external_ips: Vec<String>,

    /// Service ports
    #[serde(default)]
    pub ports: Vec<K8sServicePort>,

    /// Labels
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Annotations
    #[serde(default)]
    pub annotations: HashMap<String, String>,

    /// Creation timestamp
    pub created_at: Option<String>,
}

/// Kubernetes service port
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sServicePort {
    /// Port name
    pub name: Option<String>,

    /// Protocol (TCP, UDP)
    pub protocol: String,

    /// Service port
    pub port: i32,

    /// Target port
    pub target_port: String,

    /// Node port (for NodePort/LoadBalancer)
    pub node_port: Option<i32>,
}

/// Kubernetes endpoint representation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sEndpoint {
    /// Pod IP
    pub ip: String,

    /// Pod name
    pub pod_name: String,

    /// Node name
    pub node_name: Option<String>,

    /// Target port
    pub port: i32,

    /// Protocol
    pub protocol: String,

    /// Ready status
    pub ready: bool,

    /// Pod labels
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Kubernetes pod metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sPodMetadata {
    /// Pod name
    pub name: String,

    /// Pod namespace
    pub namespace: String,

    /// Pod IP
    pub pod_ip: Option<String>,

    /// Node name
    pub node_name: Option<String>,

    /// Labels
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Annotations
    #[serde(default)]
    pub annotations: HashMap<String, String>,

    /// Container ports
    #[serde(default)]
    pub container_ports: Vec<ContainerPort>,

    /// Pod phase
    pub phase: String,

    /// Ready condition
    pub ready: bool,
}

/// Container port definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerPort {
    /// Port name
    pub name: Option<String>,

    /// Container port number
    pub container_port: i32,

    /// Protocol (TCP, UDP)
    pub protocol: String,
}

/// Sync direction for bidirectional sync
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncDirection {
    /// Sync from Kubernetes to Batata
    #[default]
    K8sToBatata,
    /// Sync from Batata to Kubernetes
    BatataToK8s,
    /// Bidirectional sync
    Bidirectional,
}

/// Sync status for a service
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSyncStatus {
    /// Service name
    pub service_name: String,

    /// Namespace
    pub namespace: String,

    /// Sync direction
    pub direction: SyncDirection,

    /// Last sync timestamp
    pub last_sync: i64,

    /// Sync status
    pub status: SyncState,

    /// Error message (if any)
    pub error: Option<String>,

    /// Number of synced instances
    pub instance_count: u32,
}

/// Sync state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncState {
    /// Not started
    #[default]
    Pending,
    /// Currently syncing
    Syncing,
    /// Sync completed successfully
    Synced,
    /// Sync failed
    Failed,
}

/// K8s sync statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sSyncStats {
    /// Total services watched
    pub services_watched: u32,

    /// Successfully synced services
    pub services_synced: u32,

    /// Failed sync count
    pub sync_failures: u32,

    /// Total endpoints synced
    pub endpoints_synced: u32,

    /// Last sync timestamp
    pub last_sync: Option<i64>,

    /// Sync by namespace
    pub by_namespace: HashMap<String, u32>,
}

// =============================================================================
// Prometheus Models
// =============================================================================

/// Prometheus HTTP service discovery target group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrometheusTargetGroup {
    /// List of targets (host:port)
    pub targets: Vec<String>,

    /// Labels for this target group
    pub labels: HashMap<String, String>,
}

/// Prometheus service discovery query parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrometheusSDQuery {
    /// Filter by namespace
    pub namespace: Option<String>,

    /// Filter by group name
    pub group: Option<String>,

    /// Filter by cluster name
    pub cluster: Option<String>,

    /// Filter by service name pattern
    pub service_pattern: Option<String>,

    /// Only healthy instances
    #[serde(default = "default_true")]
    pub healthy_only: bool,

    /// Include metadata labels
    #[serde(default = "default_true")]
    pub include_metadata: bool,

    /// Label prefix for metadata
    #[serde(default = "default_label_prefix")]
    pub label_prefix: String,
}

impl Default for PrometheusSDQuery {
    fn default() -> Self {
        Self {
            namespace: None,
            group: None,
            cluster: None,
            service_pattern: None,
            healthy_only: default_true(),
            include_metadata: default_true(),
            label_prefix: default_label_prefix(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_label_prefix() -> String {
    "__meta_batata_".to_string()
}

/// Prometheus label mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelMapping {
    /// Source field (from Instance/Service)
    pub source: String,

    /// Target label name
    pub target: String,

    /// Optional transformation
    pub transform: Option<LabelTransform>,
}

/// Label transformation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelTransform {
    /// No transformation
    #[default]
    None,
    /// Lowercase
    Lowercase,
    /// Uppercase
    Uppercase,
    /// Replace dots with underscores
    SanitizeDots,
}

/// Prometheus discovery statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrometheusSDStats {
    /// Total target groups served
    pub target_groups_served: u64,

    /// Total targets served
    pub targets_served: u64,

    /// Requests count
    pub requests_count: u64,

    /// Last request timestamp
    pub last_request: Option<i64>,

    /// Targets by namespace
    pub by_namespace: HashMap<String, u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_target_group_serialization() {
        let group = PrometheusTargetGroup {
            targets: vec!["10.0.0.1:8080".to_string(), "10.0.0.2:8080".to_string()],
            labels: {
                let mut labels = HashMap::new();
                labels.insert("job".to_string(), "my-service".to_string());
                labels.insert("__meta_batata_namespace".to_string(), "default".to_string());
                labels
            },
        };

        let json = serde_json::to_string(&group).unwrap();
        assert!(json.contains("10.0.0.1:8080"));
        assert!(json.contains("my-service"));
    }

    #[test]
    fn test_k8s_service_serialization() {
        let svc = K8sService {
            name: "api-gateway".to_string(),
            namespace: "default".to_string(),
            service_type: "ClusterIP".to_string(),
            cluster_ip: Some("10.96.0.1".to_string()),
            external_ips: vec![],
            ports: vec![K8sServicePort {
                name: Some("http".to_string()),
                protocol: "TCP".to_string(),
                port: 80,
                target_port: "8080".to_string(),
                node_port: None,
            }],
            labels: HashMap::new(),
            annotations: HashMap::new(),
            created_at: None,
        };

        let json = serde_json::to_string(&svc).unwrap();
        assert!(json.contains("api-gateway"));
        assert!(json.contains("ClusterIP"));
    }

    #[test]
    fn test_sync_direction_default() {
        let dir = SyncDirection::default();
        assert_eq!(dir, SyncDirection::K8sToBatata);
    }

    #[test]
    fn test_prometheus_sd_query_defaults() {
        let query = PrometheusSDQuery::default();
        assert!(query.healthy_only);
        assert!(query.include_metadata);
        assert_eq!(query.label_prefix, "__meta_batata_");
    }
}
