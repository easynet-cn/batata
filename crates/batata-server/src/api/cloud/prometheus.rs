//! Prometheus HTTP Service Discovery
//!
//! This module provides an HTTP endpoint compatible with Prometheus
//! HTTP-based service discovery (http_sd_config).
//!
//! Features:
//! - HTTP SD compatible endpoint
//! - Service metadata to Prometheus labels conversion
//! - Configurable label mapping

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use actix_web::{HttpResponse, get, web};
use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::debug;

use super::model::*;
use crate::model::response::RestResult;

/// Prometheus service discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrometheusSDConfig {
    /// Enable Prometheus SD endpoint
    #[serde(default)]
    pub enabled: bool,

    /// Default label prefix
    #[serde(default = "default_label_prefix")]
    pub label_prefix: String,

    /// Include instance metadata as labels
    #[serde(default = "default_true")]
    pub include_metadata: bool,

    /// Only include healthy instances
    #[serde(default = "default_true")]
    pub healthy_only: bool,

    /// Custom label mappings
    #[serde(default)]
    pub label_mappings: Vec<LabelMapping>,
}

fn default_label_prefix() -> String {
    "__meta_batata_".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for PrometheusSDConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            label_prefix: default_label_prefix(),
            include_metadata: true,
            healthy_only: true,
            label_mappings: vec![],
        }
    }
}

/// Prometheus service discovery provider
pub struct PrometheusServiceDiscovery {
    /// Configuration
    config: PrometheusSDConfig,

    /// Statistics
    stats: RwLock<PrometheusSDStats>,

    /// Request counter
    request_count: AtomicU64,

    /// Cache of target groups by namespace (for future use)
    #[allow(dead_code)]
    cache: DashMap<String, Vec<PrometheusTargetGroup>>,

    /// Cache timestamp (for future use)
    #[allow(dead_code)]
    cache_time: AtomicU64,

    /// Cache TTL in milliseconds (for future use)
    #[allow(dead_code)]
    cache_ttl_ms: u64,
}

impl PrometheusServiceDiscovery {
    /// Create a new Prometheus SD provider
    pub fn new(config: PrometheusSDConfig) -> Self {
        Self {
            config,
            stats: RwLock::new(PrometheusSDStats::default()),
            request_count: AtomicU64::new(0),
            cache: DashMap::new(),
            cache_time: AtomicU64::new(0),
            cache_ttl_ms: 5000, // 5 second cache
        }
    }

    /// Generate target groups from services
    pub fn generate_targets(
        &self,
        services: &[ServiceWithInstances],
        query: &PrometheusSDQuery,
    ) -> Vec<PrometheusTargetGroup> {
        let mut target_groups = Vec::new();

        for service in services {
            // Filter by namespace
            if let Some(ref ns) = query.namespace {
                if &service.namespace != ns {
                    continue;
                }
            }

            // Filter by group
            if let Some(ref group) = query.group {
                if &service.group_name != group {
                    continue;
                }
            }

            // Filter by service name pattern
            if let Some(ref pattern) = query.service_pattern {
                if !matches_pattern(&service.service_name, pattern) {
                    continue;
                }
            }

            // Generate targets from instances
            let mut targets = Vec::new();
            for instance in &service.instances {
                // Filter by health status
                if query.healthy_only && !instance.healthy {
                    continue;
                }

                // Filter by cluster
                if let Some(ref cluster) = query.cluster {
                    if &instance.cluster_name != cluster {
                        continue;
                    }
                }

                targets.push(format!("{}:{}", instance.ip, instance.port));
            }

            if targets.is_empty() {
                continue;
            }

            // Build labels
            let mut labels = HashMap::new();

            // Standard labels
            labels.insert("job".to_string(), service.service_name.clone());
            labels.insert(
                format!("{}namespace", query.label_prefix),
                service.namespace.clone(),
            );
            labels.insert(
                format!("{}group", query.label_prefix),
                service.group_name.clone(),
            );
            labels.insert(
                format!("{}service", query.label_prefix),
                service.service_name.clone(),
            );

            // Add instance metadata as labels if enabled
            if query.include_metadata {
                // Use metadata from first instance for service-level labels
                if let Some(first) = service.instances.first() {
                    labels.insert(
                        format!("{}cluster", query.label_prefix),
                        first.cluster_name.clone(),
                    );

                    // Add custom metadata
                    for (k, v) in &first.metadata {
                        let label_key =
                            format!("{}meta_{}", query.label_prefix, sanitize_label_name(k));
                        labels.insert(label_key, v.clone());
                    }
                }
            }

            // Apply custom label mappings
            for mapping in &self.config.label_mappings {
                if let Some(value) = self.get_source_value(&mapping.source, service) {
                    let transformed = self.apply_transform(&value, mapping.transform);
                    labels.insert(mapping.target.clone(), transformed);
                }
            }

            target_groups.push(PrometheusTargetGroup { targets, labels });
        }

        target_groups
    }

    /// Get source value for label mapping
    fn get_source_value(&self, source: &str, service: &ServiceWithInstances) -> Option<String> {
        match source {
            "namespace" => Some(service.namespace.clone()),
            "group" => Some(service.group_name.clone()),
            "service" => Some(service.service_name.clone()),
            s if s.starts_with("metadata.") => {
                let key = &s[9..];
                service
                    .instances
                    .first()
                    .and_then(|i| i.metadata.get(key).cloned())
            }
            _ => None,
        }
    }

    /// Apply transformation to label value
    fn apply_transform(&self, value: &str, transform: Option<LabelTransform>) -> String {
        match transform {
            Some(LabelTransform::Lowercase) => value.to_lowercase(),
            Some(LabelTransform::Uppercase) => value.to_uppercase(),
            Some(LabelTransform::SanitizeDots) => value.replace('.', "_"),
            None | Some(LabelTransform::None) => value.to_string(),
        }
    }

    /// Update statistics
    pub async fn record_request(&self, target_groups: usize, targets: usize) {
        self.request_count.fetch_add(1, Ordering::SeqCst);

        let mut stats = self.stats.write().await;
        stats.requests_count += 1;
        stats.target_groups_served += target_groups as u64;
        stats.targets_served += targets as u64;
        stats.last_request = Some(Utc::now().timestamp_millis());
    }

    /// Get statistics
    pub async fn stats(&self) -> PrometheusSDStats {
        self.stats.read().await.clone()
    }

    /// Get configuration
    pub fn config(&self) -> &PrometheusSDConfig {
        &self.config
    }
}

impl Default for PrometheusServiceDiscovery {
    fn default() -> Self {
        Self::new(PrometheusSDConfig::default())
    }
}

/// Service with instances for target generation
#[derive(Debug, Clone)]
pub struct ServiceWithInstances {
    pub namespace: String,
    pub group_name: String,
    pub service_name: String,
    pub instances: Vec<InstanceInfo>,
}

/// Instance information for target generation
#[derive(Debug, Clone)]
pub struct InstanceInfo {
    pub ip: String,
    pub port: i32,
    pub healthy: bool,
    pub enabled: bool,
    pub cluster_name: String,
    pub metadata: HashMap<String, String>,
}

/// Sanitize a string to be a valid Prometheus label name
fn sanitize_label_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Simple wildcard pattern matching
fn matches_pattern(name: &str, pattern: &str) -> bool {
    if pattern.is_empty() || pattern == "*" {
        return true;
    }

    if pattern.starts_with('*') && pattern.ends_with('*') {
        let inner = &pattern[1..pattern.len() - 1];
        return name.contains(inner);
    }

    if pattern.starts_with('*') {
        let suffix = &pattern[1..];
        return name.ends_with(suffix);
    }

    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len() - 1];
        return name.starts_with(prefix);
    }

    name == pattern
}

// =============================================================================
// HTTP Handlers
// =============================================================================

/// Prometheus HTTP SD endpoint
/// Returns target groups in Prometheus http_sd_config format
#[get("/v1/cloud/prometheus/sd")]
pub async fn prometheus_sd(
    sd: web::Data<Arc<PrometheusServiceDiscovery>>,
    _query: web::Query<PrometheusSDQuery>,
) -> HttpResponse {
    debug!("Prometheus SD request received");

    // In a real implementation, this would:
    // 1. Get services from the naming service
    // 2. Convert to target groups using _query parameters
    // 3. Return JSON array

    // For now, return empty array (no services configured)
    let target_groups: Vec<PrometheusTargetGroup> = vec![];

    // Record statistics
    sd.record_request(target_groups.len(), 0).await;

    // Return in Prometheus http_sd format (array of target groups)
    HttpResponse::Ok().json(target_groups)
}

/// Get Prometheus SD statistics
#[get("/v1/cloud/prometheus/stats")]
pub async fn prometheus_stats(sd: web::Data<Arc<PrometheusServiceDiscovery>>) -> HttpResponse {
    let stats = sd.stats().await;
    HttpResponse::Ok().json(RestResult::ok(Some(stats)))
}

/// Get Prometheus SD configuration
#[get("/v1/cloud/prometheus/config")]
pub async fn prometheus_config(sd: web::Data<Arc<PrometheusServiceDiscovery>>) -> HttpResponse {
    HttpResponse::Ok().json(RestResult::ok(Some(sd.config().clone())))
}

/// Configure Prometheus routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(prometheus_sd)
        .service(prometheus_stats)
        .service(prometheus_config);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_services() -> Vec<ServiceWithInstances> {
        vec![
            ServiceWithInstances {
                namespace: "public".to_string(),
                group_name: "DEFAULT_GROUP".to_string(),
                service_name: "api-gateway".to_string(),
                instances: vec![
                    InstanceInfo {
                        ip: "10.0.0.1".to_string(),
                        port: 8080,
                        healthy: true,
                        enabled: true,
                        cluster_name: "DEFAULT".to_string(),
                        metadata: {
                            let mut m = HashMap::new();
                            m.insert("version".to_string(), "1.0.0".to_string());
                            m
                        },
                    },
                    InstanceInfo {
                        ip: "10.0.0.2".to_string(),
                        port: 8080,
                        healthy: true,
                        enabled: true,
                        cluster_name: "DEFAULT".to_string(),
                        metadata: HashMap::new(),
                    },
                ],
            },
            ServiceWithInstances {
                namespace: "public".to_string(),
                group_name: "DEFAULT_GROUP".to_string(),
                service_name: "user-service".to_string(),
                instances: vec![
                    InstanceInfo {
                        ip: "10.0.1.1".to_string(),
                        port: 9000,
                        healthy: true,
                        enabled: true,
                        cluster_name: "PROD".to_string(),
                        metadata: HashMap::new(),
                    },
                    InstanceInfo {
                        ip: "10.0.1.2".to_string(),
                        port: 9000,
                        healthy: false, // Unhealthy
                        enabled: true,
                        cluster_name: "PROD".to_string(),
                        metadata: HashMap::new(),
                    },
                ],
            },
        ]
    }

    #[test]
    fn test_generate_targets_basic() {
        let sd = PrometheusServiceDiscovery::default();
        let services = create_test_services();
        let query = PrometheusSDQuery::default();

        let groups = sd.generate_targets(&services, &query);

        assert_eq!(groups.len(), 2);

        // First service should have 2 healthy targets
        let api_gateway = groups
            .iter()
            .find(|g| g.labels.get("job") == Some(&"api-gateway".to_string()))
            .unwrap();
        assert_eq!(api_gateway.targets.len(), 2);
        assert!(api_gateway.targets.contains(&"10.0.0.1:8080".to_string()));

        // Second service should have only 1 healthy target (healthy_only default true)
        let user_service = groups
            .iter()
            .find(|g| g.labels.get("job") == Some(&"user-service".to_string()))
            .unwrap();
        assert_eq!(user_service.targets.len(), 1);
    }

    #[test]
    fn test_generate_targets_with_unhealthy() {
        let sd = PrometheusServiceDiscovery::default();
        let services = create_test_services();
        let query = PrometheusSDQuery {
            healthy_only: false,
            ..Default::default()
        };

        let groups = sd.generate_targets(&services, &query);

        // User service should now have 2 targets (including unhealthy)
        let user_service = groups
            .iter()
            .find(|g| g.labels.get("job") == Some(&"user-service".to_string()))
            .unwrap();
        assert_eq!(user_service.targets.len(), 2);
    }

    #[test]
    fn test_generate_targets_namespace_filter() {
        let sd = PrometheusServiceDiscovery::default();
        let services = create_test_services();
        let query = PrometheusSDQuery {
            namespace: Some("nonexistent".to_string()),
            ..Default::default()
        };

        let groups = sd.generate_targets(&services, &query);

        assert!(groups.is_empty());
    }

    #[test]
    fn test_generate_targets_service_pattern() {
        let sd = PrometheusServiceDiscovery::default();
        let services = create_test_services();
        let query = PrometheusSDQuery {
            service_pattern: Some("api-*".to_string()),
            ..Default::default()
        };

        let groups = sd.generate_targets(&services, &query);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].labels.get("job").unwrap(), "api-gateway");
    }

    #[test]
    fn test_generate_targets_cluster_filter() {
        let sd = PrometheusServiceDiscovery::default();
        let services = create_test_services();
        let query = PrometheusSDQuery {
            cluster: Some("PROD".to_string()),
            ..Default::default()
        };

        let groups = sd.generate_targets(&services, &query);

        // Only user-service has PROD cluster
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].labels.get("job").unwrap(), "user-service");
    }

    #[test]
    fn test_labels_include_metadata() {
        let sd = PrometheusServiceDiscovery::default();
        let services = create_test_services();
        let query = PrometheusSDQuery {
            include_metadata: true,
            ..Default::default()
        };

        let groups = sd.generate_targets(&services, &query);

        let api_gateway = groups
            .iter()
            .find(|g| g.labels.get("job") == Some(&"api-gateway".to_string()))
            .unwrap();

        // Should have standard labels
        assert!(api_gateway.labels.contains_key("__meta_batata_namespace"));
        assert!(api_gateway.labels.contains_key("__meta_batata_group"));
        assert!(api_gateway.labels.contains_key("__meta_batata_cluster"));

        // Should have metadata labels
        assert_eq!(
            api_gateway.labels.get("__meta_batata_meta_version"),
            Some(&"1.0.0".to_string())
        );
    }

    #[test]
    fn test_sanitize_label_name() {
        assert_eq!(sanitize_label_name("simple"), "simple");
        assert_eq!(sanitize_label_name("with-dash"), "with_dash");
        assert_eq!(sanitize_label_name("with.dot"), "with_dot");
        assert_eq!(sanitize_label_name("with spaces"), "with_spaces");
        assert_eq!(sanitize_label_name("MixedCase123"), "MixedCase123");
    }

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("api-gateway", "api-*"));
        assert!(matches_pattern("api-gateway", "*-gateway"));
        assert!(matches_pattern("api-gateway", "*-gate*"));
        assert!(matches_pattern("api-gateway", "*"));
        assert!(matches_pattern("api-gateway", "api-gateway"));
        assert!(!matches_pattern("api-gateway", "user-*"));
    }

    #[test]
    fn test_label_transform() {
        let sd = PrometheusServiceDiscovery::default();

        assert_eq!(
            sd.apply_transform("Hello", Some(LabelTransform::Lowercase)),
            "hello"
        );
        assert_eq!(
            sd.apply_transform("Hello", Some(LabelTransform::Uppercase)),
            "HELLO"
        );
        assert_eq!(
            sd.apply_transform("a.b.c", Some(LabelTransform::SanitizeDots)),
            "a_b_c"
        );
        assert_eq!(sd.apply_transform("Hello", None), "Hello");
    }

    #[tokio::test]
    async fn test_statistics() {
        let sd = PrometheusServiceDiscovery::default();

        sd.record_request(5, 10).await;
        sd.record_request(3, 8).await;

        let stats = sd.stats().await;

        assert_eq!(stats.requests_count, 2);
        assert_eq!(stats.target_groups_served, 8);
        assert_eq!(stats.targets_served, 18);
        assert!(stats.last_request.is_some());
    }

    #[test]
    fn test_config_defaults() {
        let config = PrometheusSDConfig::default();

        assert!(config.enabled);
        assert_eq!(config.label_prefix, "__meta_batata_");
        assert!(config.include_metadata);
        assert!(config.healthy_only);
    }

    #[test]
    fn test_custom_label_mappings() {
        let config = PrometheusSDConfig {
            label_mappings: vec![LabelMapping {
                source: "namespace".to_string(),
                target: "env".to_string(),
                transform: Some(LabelTransform::Lowercase),
            }],
            ..Default::default()
        };

        let sd = PrometheusServiceDiscovery::new(config);
        let services = create_test_services();
        let query = PrometheusSDQuery::default();

        let groups = sd.generate_targets(&services, &query);

        // All groups should have the custom "env" label
        for group in &groups {
            assert!(group.labels.contains_key("env"));
            assert_eq!(group.labels.get("env"), Some(&"public".to_string()));
        }
    }
}
