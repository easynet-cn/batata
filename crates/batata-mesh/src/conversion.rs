//! Batata/Nacos to xDS Resource Conversion
//!
//! This module provides conversion functions to transform Batata service
//! discovery data into xDS resources for Envoy/Istio consumption.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use crate::mcp::types::{
    ConnectionPoolSettings, Destination, DestinationRule, DestinationRuleSpec, HttpRetry,
    HttpRoute, HttpRouteDestination, LoadBalancerSettings, OutlierDetection, PortSelector,
    ResourceMetadata, SimpleLb, Subset, TcpSettings, TrafficPolicy, VirtualService,
    VirtualServiceSpec,
};
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

/// Convert a Nacos service to an Istio VirtualService.
///
/// Creates default routing rules based on service metadata. The `domain_suffix`
/// is appended to the service name to form the DNS-style host (e.g., "svc.cluster.local").
/// Timeout and retry configuration are read from service metadata keys
/// `istio.timeout` and `istio.retries` respectively.
pub fn service_to_virtual_service(service: &NacosService, domain_suffix: &str) -> VirtualService {
    let host = format!("{}.{}", service.service_name, domain_suffix);
    let port = service
        .instances
        .first()
        .map(|i| i.port as u32)
        .unwrap_or(80);

    // Check for timeout configuration
    let timeout = service.metadata.get("istio.timeout").cloned();

    // Check for retry configuration
    let retries = service
        .metadata
        .get("istio.retries")
        .and_then(|s| s.parse::<i32>().ok())
        .map(|attempts| HttpRetry {
            attempts,
            per_try_timeout: service.metadata.get("istio.per_try_timeout").cloned(),
            retry_on: Some("5xx,reset,connect-failure".to_string()),
        });

    VirtualService {
        metadata: ResourceMetadata {
            name: service.service_name.clone(),
            namespace: if service.namespace_id.is_empty() || service.namespace_id == "public" {
                "default".to_string()
            } else {
                service.namespace_id.clone()
            },
            labels: {
                let mut labels = HashMap::new();
                labels.insert("app".to_string(), service.service_name.clone());
                labels.insert("source".to_string(), "batata".to_string());
                labels
            },
            annotations: HashMap::new(),
            resource_version: String::new(),
        },
        spec: VirtualServiceSpec {
            hosts: vec![host.clone()],
            http: vec![HttpRoute {
                name: Some("default".to_string()),
                route: vec![HttpRouteDestination {
                    destination: Destination {
                        host: host.clone(),
                        port: Some(PortSelector { number: Some(port) }),
                        subset: None,
                    },
                    weight: Some(100),
                    headers: None,
                }],
                timeout,
                retries,
                ..Default::default()
            }],
            ..Default::default()
        },
    }
}

/// Convert a Nacos service to an Istio DestinationRule.
///
/// Creates load balancing and connection pool configuration from service metadata.
/// The following metadata keys are recognized:
/// - `istio.lb_policy`: Load balancing algorithm (ROUND_ROBIN, LEAST_CONN, RANDOM)
/// - `istio.max_connections`: Maximum TCP connections
/// - `istio.connect_timeout`: TCP connection timeout (e.g., "5s")
/// - `istio.consecutive_errors`: Consecutive errors before outlier ejection
///
/// Subsets are automatically generated from non-DEFAULT Nacos cluster names.
pub fn service_to_destination_rule(service: &NacosService, domain_suffix: &str) -> DestinationRule {
    let host = format!("{}.{}", service.service_name, domain_suffix);

    // Parse load balancer from metadata
    let lb_policy = service
        .metadata
        .get("istio.lb_policy")
        .map(|p| match p.to_uppercase().as_str() {
            "ROUND_ROBIN" | "ROUNDROBIN" => SimpleLb::RoundRobin,
            "LEAST_CONN" | "LEAST_REQUEST" => SimpleLb::LeastConn,
            "RANDOM" => SimpleLb::Random,
            "PASSTHROUGH" => SimpleLb::Passthrough,
            _ => SimpleLb::RoundRobin,
        })
        .unwrap_or(SimpleLb::RoundRobin);

    // Parse connection pool from metadata
    let max_connections = service
        .metadata
        .get("istio.max_connections")
        .and_then(|v| v.parse::<i32>().ok());

    let connect_timeout = service.metadata.get("istio.connect_timeout").cloned();

    let connection_pool = if max_connections.is_some() || connect_timeout.is_some() {
        Some(ConnectionPoolSettings {
            tcp: Some(TcpSettings {
                max_connections,
                connect_timeout,
            }),
            http: None,
        })
    } else {
        None
    };

    // Parse outlier detection from metadata
    let outlier_detection = service
        .metadata
        .get("istio.consecutive_errors")
        .and_then(|v| v.parse::<i32>().ok())
        .map(|errors| OutlierDetection {
            consecutive_errors: Some(errors),
            interval: Some("10s".to_string()),
            base_ejection_time: Some("30s".to_string()),
            max_ejection_percent: Some(50),
            ..Default::default()
        });

    // Build subsets from unique cluster names (excluding DEFAULT)
    let cluster_names: HashSet<String> = service
        .instances
        .iter()
        .map(|i| i.cluster_name.clone())
        .collect();

    let subsets: Vec<Subset> = cluster_names
        .into_iter()
        .filter(|name| !name.is_empty() && name != "DEFAULT")
        .map(|name| Subset {
            name: name.clone(),
            labels: {
                let mut labels = HashMap::new();
                labels.insert("cluster".to_string(), name);
                labels
            },
            traffic_policy: None,
        })
        .collect();

    DestinationRule {
        metadata: ResourceMetadata {
            name: service.service_name.clone(),
            namespace: if service.namespace_id.is_empty() || service.namespace_id == "public" {
                "default".to_string()
            } else {
                service.namespace_id.clone()
            },
            labels: {
                let mut labels = HashMap::new();
                labels.insert("app".to_string(), service.service_name.clone());
                labels.insert("source".to_string(), "batata".to_string());
                labels
            },
            annotations: HashMap::new(),
            resource_version: String::new(),
        },
        spec: DestinationRuleSpec {
            host,
            traffic_policy: Some(TrafficPolicy {
                load_balancer: Some(LoadBalancerSettings {
                    simple: Some(lb_policy),
                    consistent_hash: None,
                }),
                connection_pool,
                outlier_detection,
                tls: None,
            }),
            subsets,
            export_to: vec![],
        },
    }
}

/// Convert multiple services to a collection of VirtualServices
pub fn services_to_virtual_services(
    services: &[NacosService],
    domain_suffix: &str,
) -> Vec<VirtualService> {
    services
        .iter()
        .map(|s| service_to_virtual_service(s, domain_suffix))
        .collect()
}

/// Convert multiple services to a collection of DestinationRules
pub fn services_to_destination_rules(
    services: &[NacosService],
    domain_suffix: &str,
) -> Vec<DestinationRule> {
    services
        .iter()
        .map(|s| service_to_destination_rule(s, domain_suffix))
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

    #[test]
    fn test_virtual_service_generation() {
        let service = create_test_service();
        let vs = service_to_virtual_service(&service, "svc.cluster.local");

        assert_eq!(vs.spec.hosts, vec!["test-service.svc.cluster.local"]);
        assert_eq!(vs.spec.http.len(), 1);
        assert_eq!(vs.spec.http[0].route[0].weight, Some(100));
        assert_eq!(
            vs.spec.http[0].route[0].destination.host,
            "test-service.svc.cluster.local"
        );
        assert_eq!(
            vs.spec.http[0].route[0]
                .destination
                .port
                .as_ref()
                .unwrap()
                .number,
            Some(8080)
        );
        assert_eq!(vs.metadata.name, "test-service");
        assert_eq!(vs.metadata.namespace, "default");
    }

    #[test]
    fn test_virtual_service_with_timeout() {
        let mut service = create_test_service();
        service
            .metadata
            .insert("istio.timeout".to_string(), "30s".to_string());

        let vs = service_to_virtual_service(&service, "svc.cluster.local");
        assert_eq!(vs.spec.http[0].timeout, Some("30s".to_string()));
    }

    #[test]
    fn test_virtual_service_with_retries() {
        let mut service = create_test_service();
        service
            .metadata
            .insert("istio.retries".to_string(), "3".to_string());
        service
            .metadata
            .insert("istio.per_try_timeout".to_string(), "2s".to_string());

        let vs = service_to_virtual_service(&service, "svc.cluster.local");
        let retries = vs.spec.http[0].retries.as_ref().unwrap();
        assert_eq!(retries.attempts, 3);
        assert_eq!(retries.per_try_timeout, Some("2s".to_string()));
        assert_eq!(
            retries.retry_on,
            Some("5xx,reset,connect-failure".to_string())
        );
    }

    #[test]
    fn test_virtual_service_serialization() {
        let service = create_test_service();
        let vs = service_to_virtual_service(&service, "svc.cluster.local");

        let json = serde_json::to_string(&vs).unwrap();
        assert!(json.contains("test-service.svc.cluster.local"));

        let deserialized: crate::mcp::types::VirtualService = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.spec.hosts, vs.spec.hosts);
    }

    #[test]
    fn test_destination_rule_generation() {
        let service = create_test_service();
        let dr = service_to_destination_rule(&service, "svc.cluster.local");

        assert_eq!(dr.spec.host, "test-service.svc.cluster.local");
        assert_eq!(dr.metadata.name, "test-service");
        assert!(dr.spec.traffic_policy.is_some());

        let tp = dr.spec.traffic_policy.unwrap();
        assert_eq!(
            tp.load_balancer.unwrap().simple,
            Some(crate::mcp::types::SimpleLb::RoundRobin)
        );
    }

    #[test]
    fn test_destination_rule_with_metadata() {
        let mut service = create_test_service();
        service
            .metadata
            .insert("istio.lb_policy".to_string(), "LEAST_CONN".to_string());
        service
            .metadata
            .insert("istio.consecutive_errors".to_string(), "5".to_string());
        service
            .metadata
            .insert("istio.max_connections".to_string(), "512".to_string());
        service
            .metadata
            .insert("istio.connect_timeout".to_string(), "5s".to_string());

        let dr = service_to_destination_rule(&service, "svc.cluster.local");
        let tp = dr.spec.traffic_policy.unwrap();

        assert_eq!(
            tp.load_balancer.unwrap().simple,
            Some(crate::mcp::types::SimpleLb::LeastConn)
        );

        let od = tp.outlier_detection.unwrap();
        assert_eq!(od.consecutive_errors, Some(5));
        assert_eq!(od.interval, Some("10s".to_string()));
        assert_eq!(od.base_ejection_time, Some("30s".to_string()));
        assert_eq!(od.max_ejection_percent, Some(50));

        let cp = tp.connection_pool.unwrap();
        assert_eq!(cp.tcp.as_ref().unwrap().max_connections, Some(512));
        assert_eq!(
            cp.tcp.as_ref().unwrap().connect_timeout,
            Some("5s".to_string())
        );
    }

    #[test]
    fn test_destination_rule_subsets() {
        let mut service = create_test_service();
        service.instances.push(NacosInstance {
            instance_id: "instance-zone-a".to_string(),
            ip: "192.168.1.4".to_string(),
            port: 8080,
            weight: 100.0,
            healthy: true,
            enabled: true,
            ephemeral: true,
            cluster_name: "zone-a".to_string(),
            service_name: "test-service".to_string(),
            metadata: HashMap::new(),
        });

        let dr = service_to_destination_rule(&service, "svc.cluster.local");

        // Should have a subset for "zone-a" but not for "DEFAULT"
        assert!(!dr.spec.subsets.is_empty());
        assert!(dr.spec.subsets.iter().any(|s| s.name == "zone-a"));
        assert!(
            dr.spec
                .subsets
                .iter()
                .all(|s| s.labels.get("cluster") == Some(&s.name))
        );
        assert!(!dr.spec.subsets.iter().any(|s| s.name == "DEFAULT"));
    }

    #[test]
    fn test_services_to_virtual_services_batch() {
        let services = vec![create_test_service()];
        let result = services_to_virtual_services(&services, "svc.cluster.local");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spec.hosts[0], "test-service.svc.cluster.local");
    }

    #[test]
    fn test_services_to_destination_rules_batch() {
        let services = vec![create_test_service()];
        let result = services_to_destination_rules(&services, "svc.cluster.local");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spec.host, "test-service.svc.cluster.local");
    }

    #[test]
    fn test_virtual_service_empty_instances() {
        let mut service = create_test_service();
        service.instances.clear();

        let vs = service_to_virtual_service(&service, "svc.cluster.local");
        // Should default to port 80 when no instances
        assert_eq!(
            vs.spec.http[0].route[0]
                .destination
                .port
                .as_ref()
                .unwrap()
                .number,
            Some(80)
        );
    }

    #[test]
    fn test_destination_rule_no_outlier_without_metadata() {
        let service = create_test_service();
        let dr = service_to_destination_rule(&service, "svc.cluster.local");
        let tp = dr.spec.traffic_policy.unwrap();
        assert!(tp.outlier_detection.is_none());
        assert!(tp.connection_pool.is_none());
    }

    #[test]
    fn test_destination_rule_custom_namespace() {
        let mut service = create_test_service();
        service.namespace_id = "production".to_string();

        let dr = service_to_destination_rule(&service, "svc.cluster.local");
        assert_eq!(dr.metadata.namespace, "production");

        let vs = service_to_virtual_service(&service, "svc.cluster.local");
        assert_eq!(vs.metadata.namespace, "production");
    }
}
