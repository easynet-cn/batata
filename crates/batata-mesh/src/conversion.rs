//! Nacos to xDS Resource Conversion
//!
//! This module provides conversion functions to transform Nacos service
//! discovery data into xDS resources for Envoy/Istio consumption.

use std::collections::HashMap;
use std::net::SocketAddr;

use crate::xds::types::{
    Cluster, ClusterLoadAssignment, Endpoint, HealthCheckConfig, HealthCheckType, HealthStatus,
    LbPolicy, Locality,
};

#[cfg(test)]
use crate::xds::types::DiscoveryType;

/// Service instance from Nacos naming service
#[derive(Debug, Clone)]
pub struct NacosInstance {
    /// Instance ID
    pub instance_id: String,
    /// IP address
    pub ip: String,
    /// Port
    pub port: u16,
    /// Weight (0-100)
    pub weight: f64,
    /// Whether the instance is healthy
    pub healthy: bool,
    /// Whether the instance is enabled
    pub enabled: bool,
    /// Whether the instance is ephemeral
    pub ephemeral: bool,
    /// Cluster name
    pub cluster_name: String,
    /// Service name
    pub service_name: String,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Service from Nacos naming service
#[derive(Debug, Clone)]
pub struct NacosService {
    /// Namespace ID
    pub namespace_id: String,
    /// Group name
    pub group_name: String,
    /// Service name
    pub service_name: String,
    /// Protect threshold (0.0-1.0)
    pub protect_threshold: f64,
    /// Metadata
    pub metadata: HashMap<String, String>,
    /// Instances
    pub instances: Vec<NacosInstance>,
}

impl NacosService {
    /// Get the full service name (group@@service)
    pub fn full_name(&self) -> String {
        if self.group_name.is_empty() || self.group_name == "DEFAULT_GROUP" {
            self.service_name.clone()
        } else {
            format!("{}@@{}", self.group_name, self.service_name)
        }
    }

    /// Get the xDS cluster name
    pub fn xds_cluster_name(&self) -> String {
        if self.namespace_id.is_empty() || self.namespace_id == "public" {
            self.full_name()
        } else {
            format!("{}/{}", self.namespace_id, self.full_name())
        }
    }
}

/// Convert Nacos service to xDS Cluster
pub fn service_to_cluster(service: &NacosService) -> Cluster {
    let cluster_name = service.xds_cluster_name();

    let mut cluster = Cluster::new_eds(&cluster_name);

    // Set load balancing policy based on metadata
    if let Some(lb_policy) = service.metadata.get("lb_policy") {
        cluster.lb_policy = match lb_policy.to_lowercase().as_str() {
            "round_robin" | "roundrobin" => LbPolicy::RoundRobin,
            "least_request" | "leastrequest" | "least_conn" => LbPolicy::LeastRequest,
            "random" => LbPolicy::Random,
            "ring_hash" | "ringhash" | "consistent_hash" => LbPolicy::RingHash,
            "maglev" => LbPolicy::Maglev,
            _ => LbPolicy::RoundRobin,
        };
    }

    // Set connect timeout from metadata
    if let Some(timeout) = service.metadata.get("connect_timeout_ms")
        && let Ok(ms) = timeout.parse::<u64>()
    {
        cluster.connect_timeout_ms = ms;
    }

    // Add health check if configured
    if service
        .metadata
        .get("health_check")
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        let health_check = HealthCheckConfig {
            check_type: match service
                .metadata
                .get("health_check_type")
                .map(|s| s.as_str())
            {
                Some("http") => {
                    let path = service
                        .metadata
                        .get("health_check_path")
                        .cloned()
                        .unwrap_or_else(|| "/health".to_string());
                    HealthCheckType::Http {
                        path,
                        expected_statuses: vec![200],
                    }
                }
                Some("grpc") => HealthCheckType::Grpc { service_name: None },
                _ => HealthCheckType::Tcp,
            },
            ..Default::default()
        };
        cluster.health_checks.push(health_check);
    }

    // Copy relevant metadata
    for (key, value) in &service.metadata {
        if key.starts_with("envoy.") || key.starts_with("istio.") {
            cluster.metadata.insert(key.clone(), value.clone());
        }
    }

    cluster
}

/// Convert Nacos service to xDS ClusterLoadAssignment (EDS)
pub fn service_to_cluster_load_assignment(service: &NacosService) -> ClusterLoadAssignment {
    let cluster_name = service.xds_cluster_name();
    let mut cla = ClusterLoadAssignment::new(&cluster_name);

    // Group instances by cluster (Nacos cluster, not xDS cluster)
    let mut cluster_instances: HashMap<String, Vec<&NacosInstance>> = HashMap::new();

    for instance in &service.instances {
        if !instance.enabled {
            continue;
        }

        cluster_instances
            .entry(instance.cluster_name.clone())
            .or_default()
            .push(instance);
    }

    // Convert each Nacos cluster to a locality
    for (nacos_cluster, instances) in cluster_instances {
        let locality = cluster_to_locality(&nacos_cluster, service);
        let endpoints: Vec<Endpoint> = instances
            .iter()
            .filter_map(|inst| instance_to_endpoint(inst))
            .collect();

        if !endpoints.is_empty() {
            // Calculate locality weight as sum of instance weights
            let weight = endpoints.iter().map(|e| e.weight).sum::<u32>().max(1);
            cla.add_locality(locality, endpoints, weight);
        }
    }

    cla
}

/// Convert Nacos cluster name to xDS Locality
fn cluster_to_locality(nacos_cluster: &str, service: &NacosService) -> Locality {
    // Try to parse cluster name as region/zone/sub-zone
    let parts: Vec<&str> = nacos_cluster.split('/').collect();

    match parts.len() {
        1 => {
            // Just a cluster name - use as zone
            Locality {
                region: service.metadata.get("region").cloned().unwrap_or_default(),
                zone: nacos_cluster.to_string(),
                sub_zone: String::new(),
            }
        }
        2 => Locality {
            region: parts[0].to_string(),
            zone: parts[1].to_string(),
            sub_zone: String::new(),
        },
        _ => Locality {
            region: parts[0].to_string(),
            zone: parts[1].to_string(),
            sub_zone: parts[2..].join("/"),
        },
    }
}

/// Convert Nacos instance to xDS Endpoint
fn instance_to_endpoint(instance: &NacosInstance) -> Option<Endpoint> {
    // Parse address
    let addr: SocketAddr = format!("{}:{}", instance.ip, instance.port).parse().ok()?;

    // Convert health status
    let health_status = if instance.healthy {
        HealthStatus::Healthy
    } else {
        HealthStatus::Unhealthy
    };

    // Convert weight (Nacos uses 0-100, xDS uses 1-128)
    let weight = ((instance.weight * 1.28).round() as u32).clamp(1, 128);

    let mut endpoint = Endpoint::new(addr)
        .with_weight(weight)
        .with_health_status(health_status);

    // Copy relevant metadata
    let mut metadata = HashMap::new();
    for (key, value) in &instance.metadata {
        if key.starts_with("envoy.") || key.starts_with("istio.") {
            metadata.insert(key.clone(), value.clone());
        }
    }
    if !metadata.is_empty() {
        endpoint = endpoint.with_metadata(metadata);
    }

    // Set priority from metadata
    if let Some(priority) = instance.metadata.get("priority")
        && let Ok(p) = priority.parse::<u32>()
    {
        endpoint = endpoint.with_priority(p);
    }

    Some(endpoint)
}

/// Convert multiple services to a collection of clusters
pub fn services_to_clusters(services: &[NacosService]) -> Vec<Cluster> {
    services.iter().map(service_to_cluster).collect()
}

/// Convert multiple services to a collection of cluster load assignments
pub fn services_to_endpoints(services: &[NacosService]) -> Vec<ClusterLoadAssignment> {
    services
        .iter()
        .map(service_to_cluster_load_assignment)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_instance(ip: &str, port: u16, healthy: bool) -> NacosInstance {
        NacosInstance {
            instance_id: format!("{}#{}#{}", ip, port, "test-service"),
            ip: ip.to_string(),
            port,
            weight: 100.0,
            healthy,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            service_name: "test-service".to_string(),
            metadata: HashMap::new(),
        }
    }

    fn create_test_service() -> NacosService {
        NacosService {
            namespace_id: "public".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            service_name: "test-service".to_string(),
            protect_threshold: 0.0,
            metadata: HashMap::new(),
            instances: vec![
                create_test_instance("192.168.1.1", 8080, true),
                create_test_instance("192.168.1.2", 8080, true),
                create_test_instance("192.168.1.3", 8080, false),
            ],
        }
    }

    #[test]
    fn test_service_to_cluster() {
        let service = create_test_service();
        let cluster = service_to_cluster(&service);

        assert_eq!(cluster.name, "test-service");
        assert_eq!(cluster.discovery_type, DiscoveryType::Eds);
        assert_eq!(cluster.lb_policy, LbPolicy::RoundRobin);
    }

    #[test]
    fn test_service_to_cluster_load_assignment() {
        let service = create_test_service();
        let cla = service_to_cluster_load_assignment(&service);

        assert_eq!(cla.cluster_name, "test-service");
        assert_eq!(cla.total_count(), 3);
        assert_eq!(cla.healthy_count(), 2);
    }

    #[test]
    fn test_instance_to_endpoint() {
        let instance = create_test_instance("192.168.1.1", 8080, true);
        let endpoint = instance_to_endpoint(&instance).unwrap();

        assert_eq!(endpoint.address.port(), 8080);
        assert!(endpoint.health_status.is_available());
    }

    #[test]
    fn test_service_full_name() {
        let mut service = create_test_service();
        assert_eq!(service.full_name(), "test-service");

        service.group_name = "custom-group".to_string();
        assert_eq!(service.full_name(), "custom-group@@test-service");
    }

    #[test]
    fn test_service_xds_cluster_name() {
        let mut service = create_test_service();
        assert_eq!(service.xds_cluster_name(), "test-service");

        service.namespace_id = "custom-ns".to_string();
        assert_eq!(service.xds_cluster_name(), "custom-ns/test-service");
    }
}
