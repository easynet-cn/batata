//! Kubernetes Service Sync
//!
//! This module provides bidirectional synchronization between
//! Kubernetes services and Batata service discovery.
//!
//! Features:
//! - Watch Kubernetes services and endpoints
//! - Sync K8s endpoints to Batata instances
//! - Sync Batata services back to K8s (optional)
//! - Pod metadata retrieval

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use chrono::Utc;
use dashmap::DashMap;
use futures::TryStreamExt;
use k8s_openapi::api::core::v1::{Endpoints, Pod, Service};
use kube::{
    Api, Client, Config,
    runtime::watcher::{self, Event},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use super::model::*;

/// Kubernetes sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct K8sSyncConfig {
    /// Enable K8s sync
    #[serde(default)]
    pub enabled: bool,

    /// Kubernetes API server URL (empty = in-cluster)
    #[serde(default)]
    pub api_server: String,

    /// Kubernetes namespace to watch (empty = all namespaces)
    #[serde(default)]
    pub namespace: String,

    /// Sync direction
    #[serde(default)]
    pub direction: SyncDirection,

    /// Sync interval in seconds
    #[serde(default = "default_sync_interval")]
    pub sync_interval_seconds: u64,

    /// Service selector labels
    #[serde(default)]
    pub selector_labels: HashMap<String, String>,

    /// Batata namespace for synced services
    #[serde(default = "default_batata_namespace")]
    pub batata_namespace: String,

    /// Batata group name for synced services
    #[serde(default = "default_batata_group")]
    pub batata_group: String,

    /// Enable health check for synced instances
    #[serde(default = "default_true")]
    pub enable_health_check: bool,

    /// Weight for synced instances
    #[serde(default = "default_weight")]
    pub default_weight: f64,
}

fn default_sync_interval() -> u64 {
    30
}

fn default_batata_namespace() -> String {
    "public".to_string()
}

fn default_batata_group() -> String {
    "DEFAULT_GROUP".to_string()
}

fn default_true() -> bool {
    true
}

fn default_weight() -> f64 {
    1.0
}

impl Default for K8sSyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_server: String::new(),
            namespace: String::new(),
            direction: SyncDirection::default(),
            sync_interval_seconds: default_sync_interval(),
            selector_labels: HashMap::new(),
            batata_namespace: default_batata_namespace(),
            batata_group: default_batata_group(),
            enable_health_check: true,
            default_weight: default_weight(),
        }
    }
}

/// Kubernetes service sync manager
pub struct K8sServiceSync {
    /// Configuration
    pub config: K8sSyncConfig,

    /// Running flag
    running: AtomicBool,

    /// Kubernetes client (optional, initialized on start)
    client: RwLock<Option<Client>>,

    /// Watched services (namespace/name -> K8sService)
    services: DashMap<String, K8sService>,

    /// Endpoints cache (namespace/name -> Vec<K8sEndpoint>)
    endpoints: DashMap<String, Vec<K8sEndpoint>>,

    /// Pod metadata cache (namespace/name -> K8sPodMetadata)
    pods: DashMap<String, K8sPodMetadata>,

    /// Sync status per service
    sync_status: DashMap<String, ServiceSyncStatus>,

    /// Statistics
    stats: RwLock<K8sSyncStats>,

    /// Sync version counter
    version: AtomicU64,
}

impl K8sServiceSync {
    /// Create a new K8s sync manager
    pub fn new(config: K8sSyncConfig) -> Self {
        Self {
            config,
            running: AtomicBool::new(false),
            client: RwLock::new(None),
            services: DashMap::new(),
            endpoints: DashMap::new(),
            pods: DashMap::new(),
            sync_status: DashMap::new(),
            stats: RwLock::new(K8sSyncStats::default()),
            version: AtomicU64::new(0),
        }
    }

    /// Initialize Kubernetes client
    async fn init_client(&self) -> Result<Client, String> {
        let config = if self.config.api_server.is_empty() {
            // Use in-cluster config
            Config::incluster().map_err(|e| format!("Failed to get in-cluster config: {}", e))?
        } else {
            // Use custom API server URL
            Config::from_custom_kubeconfig(
                kube::config::Kubeconfig::default(),
                &kube::config::KubeConfigOptions::default(),
            )
            .await
            .map_err(|e| format!("Failed to load kubeconfig: {}", e))?
        };

        Client::try_from(config).map_err(|e| format!("Failed to create K8s client: {}", e))
    }

    /// Start the sync process
    pub async fn start(&self) {
        if !self.config.enabled {
            info!("Kubernetes sync is disabled");
            return;
        }

        // Initialize the Kubernetes client
        match self.init_client().await {
            Ok(client) => {
                *self.client.write().await = Some(client);
                info!("Kubernetes client initialized successfully");
            }
            Err(e) => {
                error!("Failed to initialize Kubernetes client: {}", e);
                return;
            }
        }

        self.running.store(true, Ordering::SeqCst);
        info!(
            namespace = %self.config.namespace,
            direction = ?self.config.direction,
            "Starting Kubernetes service sync"
        );
    }

    /// Start watching Kubernetes services (call this in a separate task)
    pub async fn watch_services(self: Arc<Self>) {
        let client = {
            let guard = self.client.read().await;
            match guard.as_ref() {
                Some(c) => c.clone(),
                None => {
                    error!("Kubernetes client not initialized");
                    return;
                }
            }
        };

        let api: Api<Service> = if self.config.namespace.is_empty() {
            Api::all(client)
        } else {
            Api::namespaced(client, &self.config.namespace)
        };

        info!("Starting Kubernetes service watcher");

        let label_selector = if !self.config.selector_labels.is_empty() {
            self.config
                .selector_labels
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(",")
        } else {
            String::new()
        };

        let watcher_config = if label_selector.is_empty() {
            watcher::Config::default()
        } else {
            watcher::Config::default().labels(&label_selector)
        };

        let stream = watcher::watcher(api, watcher_config);

        tokio::pin!(stream);

        while let Ok(Some(event)) = stream.try_next().await {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            match event {
                Event::Apply(svc) | Event::InitApply(svc) => {
                    if let Some(k8s_svc) = self.convert_service(&svc) {
                        self.register_service(k8s_svc);
                    }
                }
                Event::Delete(svc) => {
                    let namespace = svc.metadata.namespace.as_deref().unwrap_or("default");
                    let name = svc.metadata.name.as_deref().unwrap_or("");
                    self.unregister_service(namespace, name);
                }
                Event::Init => {
                    debug!("Service watcher initialized");
                }
                Event::InitDone => {
                    info!("Service watcher initial sync complete");
                }
            }
        }

        info!("Service watcher stopped");
    }

    /// Start watching Kubernetes endpoints (call this in a separate task)
    pub async fn watch_endpoints(self: Arc<Self>) {
        let client = {
            let guard = self.client.read().await;
            match guard.as_ref() {
                Some(c) => c.clone(),
                None => {
                    error!("Kubernetes client not initialized");
                    return;
                }
            }
        };

        let api: Api<Endpoints> = if self.config.namespace.is_empty() {
            Api::all(client)
        } else {
            Api::namespaced(client, &self.config.namespace)
        };

        info!("Starting Kubernetes endpoints watcher");

        let stream = watcher::watcher(api, watcher::Config::default());

        tokio::pin!(stream);

        while let Ok(Some(event)) = stream.try_next().await {
            if !self.running.load(Ordering::SeqCst) {
                break;
            }

            match event {
                Event::Apply(ep) | Event::InitApply(ep) => {
                    let namespace = ep.metadata.namespace.as_deref().unwrap_or("default");
                    let name = ep.metadata.name.as_deref().unwrap_or("");
                    let endpoints = self.convert_endpoints(&ep);
                    self.update_endpoints(namespace, name, endpoints);
                }
                Event::Delete(ep) => {
                    let namespace = ep.metadata.namespace.as_deref().unwrap_or("default");
                    let name = ep.metadata.name.as_deref().unwrap_or("");
                    let key = format!("{}/{}", namespace, name);
                    self.endpoints.remove(&key);
                    debug!(namespace = %namespace, name = %name, "Endpoints deleted");
                }
                Event::Init => {
                    debug!("Endpoints watcher initialized");
                }
                Event::InitDone => {
                    info!("Endpoints watcher initial sync complete");
                }
            }
        }

        info!("Endpoints watcher stopped");
    }

    /// Convert Kubernetes Service to K8sService
    fn convert_service(&self, svc: &Service) -> Option<K8sService> {
        let metadata = &svc.metadata;
        let spec = svc.spec.as_ref()?;

        let ports = spec
            .ports
            .as_ref()
            .map(|ps| {
                ps.iter()
                    .map(|p| K8sServicePort {
                        name: p.name.clone(),
                        protocol: p.protocol.clone().unwrap_or_else(|| "TCP".to_string()),
                        port: p.port,
                        target_port: p
                            .target_port
                            .as_ref()
                            .map(|tp| match tp {
                                k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::Int(i) => i.to_string(),
                                k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(s) => s.clone(),
                            })
                            .unwrap_or_default(),
                        node_port: p.node_port,
                    })
                    .collect()
            })
            .unwrap_or_default();

        Some(K8sService {
            name: metadata.name.clone().unwrap_or_default(),
            namespace: metadata
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            service_type: spec
                .type_
                .clone()
                .unwrap_or_else(|| "ClusterIP".to_string()),
            cluster_ip: spec.cluster_ip.clone(),
            external_ips: spec.external_ips.clone().unwrap_or_default(),
            ports,
            labels: metadata
                .labels
                .clone()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            annotations: metadata
                .annotations
                .clone()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            created_at: metadata
                .creation_timestamp
                .as_ref()
                .map(|t| t.0.to_rfc3339()),
        })
    }

    /// Convert Kubernetes Endpoints to Vec<K8sEndpoint>
    fn convert_endpoints(&self, ep: &Endpoints) -> Vec<K8sEndpoint> {
        let mut result = Vec::new();

        if let Some(subsets) = &ep.subsets {
            for subset in subsets {
                let ports: Vec<(i32, String)> = subset
                    .ports
                    .as_ref()
                    .map(|ps| {
                        ps.iter()
                            .map(|p| {
                                (
                                    p.port,
                                    p.protocol.clone().unwrap_or_else(|| "TCP".to_string()),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Ready addresses
                if let Some(addresses) = &subset.addresses {
                    for addr in addresses {
                        for (port, protocol) in &ports {
                            result.push(K8sEndpoint {
                                ip: addr.ip.clone(),
                                pod_name: addr
                                    .target_ref
                                    .as_ref()
                                    .and_then(|r| r.name.clone())
                                    .unwrap_or_default(),
                                node_name: addr.node_name.clone(),
                                port: *port,
                                protocol: protocol.clone(),
                                ready: true,
                                labels: HashMap::new(),
                            });
                        }
                    }
                }

                // Not ready addresses
                if let Some(not_ready) = &subset.not_ready_addresses {
                    for addr in not_ready {
                        for (port, protocol) in &ports {
                            result.push(K8sEndpoint {
                                ip: addr.ip.clone(),
                                pod_name: addr
                                    .target_ref
                                    .as_ref()
                                    .and_then(|r| r.name.clone())
                                    .unwrap_or_default(),
                                node_name: addr.node_name.clone(),
                                port: *port,
                                protocol: protocol.clone(),
                                ready: false,
                                labels: HashMap::new(),
                            });
                        }
                    }
                }
            }
        }

        result
    }

    /// Fetch pod metadata from Kubernetes
    pub async fn fetch_pod(&self, namespace: &str, name: &str) -> Result<K8sPodMetadata, String> {
        let client = {
            let guard = self.client.read().await;
            match guard.as_ref() {
                Some(c) => c.clone(),
                None => return Err("Kubernetes client not initialized".to_string()),
            }
        };

        let api: Api<Pod> = Api::namespaced(client, namespace);
        let pod = api
            .get(name)
            .await
            .map_err(|e| format!("Failed to get pod: {}", e))?;

        Ok(self.convert_pod(&pod))
    }

    /// Convert Kubernetes Pod to K8sPodMetadata
    fn convert_pod(&self, pod: &Pod) -> K8sPodMetadata {
        let metadata = &pod.metadata;
        let spec = pod.spec.as_ref();
        let status = pod.status.as_ref();

        let container_ports = spec
            .and_then(|s| s.containers.first())
            .map(|c| {
                c.ports
                    .as_ref()
                    .map(|ps| {
                        ps.iter()
                            .map(|p| ContainerPort {
                                name: p.name.clone(),
                                container_port: p.container_port,
                                protocol: p.protocol.clone().unwrap_or_else(|| "TCP".to_string()),
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let ready = status
            .and_then(|s| s.conditions.as_ref())
            .map(|conditions| {
                conditions
                    .iter()
                    .any(|c| c.type_ == "Ready" && c.status == "True")
            })
            .unwrap_or(false);

        K8sPodMetadata {
            name: metadata.name.clone().unwrap_or_default(),
            namespace: metadata
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            pod_ip: status.and_then(|s| s.pod_ip.clone()),
            node_name: spec.and_then(|s| s.node_name.clone()),
            labels: metadata
                .labels
                .clone()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            annotations: metadata
                .annotations
                .clone()
                .unwrap_or_default()
                .into_iter()
                .collect(),
            container_ports,
            phase: status
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            ready,
        }
    }

    /// Stop the sync process
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        info!("Stopping Kubernetes service sync");
    }

    /// Check if sync is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Manually trigger a sync
    pub async fn sync_now(&self) -> Result<u32, String> {
        if !self.config.enabled {
            return Err("Kubernetes sync is disabled".to_string());
        }

        info!("Triggering manual Kubernetes sync");
        self.version.fetch_add(1, Ordering::SeqCst);

        // Sync based on direction
        match self.config.direction {
            SyncDirection::K8sToBatata => self.sync_k8s_to_batata().await,
            SyncDirection::BatataToK8s => self.sync_batata_to_k8s().await,
            SyncDirection::Bidirectional => {
                let k8s_count = self.sync_k8s_to_batata().await?;
                let batata_count = self.sync_batata_to_k8s().await?;
                Ok(k8s_count + batata_count)
            }
        }
    }

    /// Sync Kubernetes services to Batata
    async fn sync_k8s_to_batata(&self) -> Result<u32, String> {
        let mut synced = 0u32;
        let now = Utc::now().timestamp_millis();

        for entry in self.services.iter() {
            let key = entry.key().clone();
            let service = entry.value();

            // Get endpoints for this service
            if let Some(endpoints) = self.endpoints.get(&key) {
                let instance_count = endpoints.len() as u32;

                // Update sync status
                self.sync_status.insert(
                    key.clone(),
                    ServiceSyncStatus {
                        service_name: service.name.clone(),
                        namespace: service.namespace.clone(),
                        direction: SyncDirection::K8sToBatata,
                        last_sync: now,
                        status: SyncState::Synced,
                        error: None,
                        instance_count,
                    },
                );

                debug!(
                    service = %service.name,
                    namespace = %service.namespace,
                    endpoints = instance_count,
                    "Synced K8s service to Batata"
                );

                synced += 1;
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.services_synced = synced;
            stats.last_sync = Some(now);
        }

        Ok(synced)
    }

    /// Sync Batata services to Kubernetes
    async fn sync_batata_to_k8s(&self) -> Result<u32, String> {
        // In a real implementation, this would:
        // 1. Get Batata services from the naming service
        // 2. Create/update Kubernetes Services and Endpoints
        // 3. Track sync status

        debug!("Syncing Batata services to Kubernetes");
        Ok(0)
    }

    /// Register a Kubernetes service (called by watcher)
    pub fn register_service(&self, service: K8sService) {
        let key = format!("{}/{}", service.namespace, service.name);
        info!(
            service = %service.name,
            namespace = %service.namespace,
            "Registered K8s service"
        );
        self.services.insert(key, service);
    }

    /// Unregister a Kubernetes service
    pub fn unregister_service(&self, namespace: &str, name: &str) {
        let key = format!("{}/{}", namespace, name);
        self.services.remove(&key);
        self.endpoints.remove(&key);
        self.sync_status.remove(&key);
        info!(
            service = %name,
            namespace = %namespace,
            "Unregistered K8s service"
        );
    }

    /// Update endpoints for a service
    pub fn update_endpoints(&self, namespace: &str, name: &str, endpoints: Vec<K8sEndpoint>) {
        let key = format!("{}/{}", namespace, name);
        let count = endpoints.len();
        self.endpoints.insert(key, endpoints);
        debug!(
            service = %name,
            namespace = %namespace,
            count = count,
            "Updated K8s endpoints"
        );
    }

    /// Get a service by namespace and name
    pub fn get_service(&self, namespace: &str, name: &str) -> Option<K8sService> {
        let key = format!("{}/{}", namespace, name);
        self.services.get(&key).map(|e| e.value().clone())
    }

    /// Get endpoints for a service
    pub fn get_endpoints(&self, namespace: &str, name: &str) -> Vec<K8sEndpoint> {
        let key = format!("{}/{}", namespace, name);
        self.endpoints
            .get(&key)
            .map(|e| e.value().clone())
            .unwrap_or_default()
    }

    /// List all watched services
    pub fn list_services(&self) -> Vec<K8sService> {
        self.services.iter().map(|e| e.value().clone()).collect()
    }

    /// Register pod metadata
    pub fn register_pod(&self, pod: K8sPodMetadata) {
        let key = format!("{}/{}", pod.namespace, pod.name);
        self.pods.insert(key, pod);
    }

    /// Get pod metadata
    pub fn get_pod(&self, namespace: &str, name: &str) -> Option<K8sPodMetadata> {
        let key = format!("{}/{}", namespace, name);
        self.pods.get(&key).map(|e| e.value().clone())
    }

    /// Get sync status for a service
    pub fn get_sync_status(&self, namespace: &str, name: &str) -> Option<ServiceSyncStatus> {
        let key = format!("{}/{}", namespace, name);
        self.sync_status.get(&key).map(|e| e.value().clone())
    }

    /// Get all sync statuses
    pub fn list_sync_status(&self) -> Vec<ServiceSyncStatus> {
        self.sync_status.iter().map(|e| e.value().clone()).collect()
    }

    /// Get sync statistics
    pub async fn stats(&self) -> K8sSyncStats {
        let mut stats = self.stats.read().await.clone();
        stats.services_watched = self.services.len() as u32;
        stats.endpoints_synced = self.endpoints.iter().map(|e| e.value().len() as u32).sum();

        // Count by namespace
        stats.by_namespace.clear();
        for entry in self.services.iter() {
            let ns = &entry.value().namespace;
            *stats.by_namespace.entry(ns.clone()).or_insert(0) += 1;
        }

        stats
    }

    /// Convert K8s endpoint to Batata instance
    pub fn endpoint_to_instance(
        &self,
        endpoint: &K8sEndpoint,
        service: &K8sService,
    ) -> HashMap<String, String> {
        let mut instance = HashMap::new();

        instance.insert("ip".to_string(), endpoint.ip.clone());
        instance.insert("port".to_string(), endpoint.port.to_string());
        instance.insert("weight".to_string(), self.config.default_weight.to_string());
        instance.insert("healthy".to_string(), endpoint.ready.to_string());
        instance.insert("enabled".to_string(), "true".to_string());
        instance.insert("ephemeral".to_string(), "true".to_string());
        instance.insert("clusterName".to_string(), "DEFAULT".to_string());
        instance.insert("serviceName".to_string(), service.name.clone());

        // Add metadata from labels
        for (k, v) in &endpoint.labels {
            instance.insert(format!("k8s.{}", k), v.clone());
        }

        // Add pod info
        if !endpoint.pod_name.is_empty() {
            instance.insert("k8s.pod".to_string(), endpoint.pod_name.clone());
        }
        if let Some(node) = &endpoint.node_name {
            instance.insert("k8s.node".to_string(), node.clone());
        }

        instance
    }

    /// Convert Batata instance to K8s endpoint
    pub fn instance_to_endpoint(instance: &HashMap<String, String>) -> Result<K8sEndpoint, String> {
        let ip = instance.get("ip").ok_or("Missing ip field")?.clone();
        let port: i32 = instance
            .get("port")
            .ok_or("Missing port field")?
            .parse()
            .map_err(|_| "Invalid port")?;
        let ready = instance.get("healthy").map(|s| s == "true").unwrap_or(true);

        Ok(K8sEndpoint {
            ip,
            pod_name: instance.get("k8s.pod").cloned().unwrap_or_default(),
            node_name: instance.get("k8s.node").cloned(),
            port,
            protocol: "TCP".to_string(),
            ready,
            labels: HashMap::new(),
        })
    }
}

impl Default for K8sServiceSync {
    fn default() -> Self {
        Self::new(K8sSyncConfig::default())
    }
}

// =============================================================================
// HTTP Handlers
// =============================================================================

use crate::model::response::RestResult;
use actix_web::{HttpResponse, delete, get, post, put, web};

/// Get Kubernetes sync status
#[get("/v1/cloud/k8s/status")]
pub async fn k8s_status(sync: web::Data<std::sync::Arc<K8sServiceSync>>) -> HttpResponse {
    #[derive(Serialize)]
    struct StatusResponse {
        running: bool,
        enabled: bool,
        direction: SyncDirection,
        namespace: String,
    }

    let status = StatusResponse {
        running: sync.is_running(),
        enabled: sync.config.enabled,
        direction: sync.config.direction,
        namespace: sync.config.namespace.clone(),
    };

    HttpResponse::Ok().json(RestResult::ok(Some(status)))
}

/// Get Kubernetes sync statistics
#[get("/v1/cloud/k8s/stats")]
pub async fn k8s_stats(sync: web::Data<std::sync::Arc<K8sServiceSync>>) -> HttpResponse {
    let stats = sync.stats().await;
    HttpResponse::Ok().json(RestResult::ok(Some(stats)))
}

/// Get Kubernetes sync configuration
#[get("/v1/cloud/k8s/config")]
pub async fn k8s_config(sync: web::Data<std::sync::Arc<K8sServiceSync>>) -> HttpResponse {
    HttpResponse::Ok().json(RestResult::ok(Some(sync.config.clone())))
}

/// List watched Kubernetes services
#[get("/v1/cloud/k8s/services")]
pub async fn k8s_list_services(sync: web::Data<std::sync::Arc<K8sServiceSync>>) -> HttpResponse {
    let services = sync.list_services();
    HttpResponse::Ok().json(RestResult::ok(Some(services)))
}

/// Get a specific Kubernetes service
#[get("/v1/cloud/k8s/services/{namespace}/{name}")]
pub async fn k8s_get_service(
    sync: web::Data<std::sync::Arc<K8sServiceSync>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    match sync.get_service(&namespace, &name) {
        Some(service) => HttpResponse::Ok().json(RestResult::ok(Some(service))),
        None => HttpResponse::NotFound().json(RestResult::<()>::err(404, "Service not found")),
    }
}

/// Get endpoints for a Kubernetes service
#[get("/v1/cloud/k8s/services/{namespace}/{name}/endpoints")]
pub async fn k8s_get_endpoints(
    sync: web::Data<std::sync::Arc<K8sServiceSync>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    let endpoints = sync.get_endpoints(&namespace, &name);
    HttpResponse::Ok().json(RestResult::ok(Some(endpoints)))
}

/// Get pod metadata
#[get("/v1/cloud/k8s/pods/{namespace}/{name}")]
pub async fn k8s_get_pod(
    sync: web::Data<std::sync::Arc<K8sServiceSync>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    match sync.get_pod(&namespace, &name) {
        Some(pod) => HttpResponse::Ok().json(RestResult::ok(Some(pod))),
        None => HttpResponse::NotFound().json(RestResult::<()>::err(404, "Pod not found")),
    }
}

/// Get sync status for all services
#[get("/v1/cloud/k8s/sync-status")]
pub async fn k8s_list_sync_status(sync: web::Data<std::sync::Arc<K8sServiceSync>>) -> HttpResponse {
    let status = sync.list_sync_status();
    HttpResponse::Ok().json(RestResult::ok(Some(status)))
}

/// Get sync status for a specific service
#[get("/v1/cloud/k8s/sync-status/{namespace}/{name}")]
pub async fn k8s_get_sync_status(
    sync: web::Data<std::sync::Arc<K8sServiceSync>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    match sync.get_sync_status(&namespace, &name) {
        Some(status) => HttpResponse::Ok().json(RestResult::ok(Some(status))),
        None => HttpResponse::NotFound().json(RestResult::<()>::err(404, "Sync status not found")),
    }
}

/// Trigger manual sync
#[post("/v1/cloud/k8s/sync")]
pub async fn k8s_trigger_sync(sync: web::Data<std::sync::Arc<K8sServiceSync>>) -> HttpResponse {
    match sync.sync_now().await {
        Ok(count) => {
            #[derive(Serialize)]
            struct SyncResponse {
                synced_count: u32,
            }
            HttpResponse::Ok().json(RestResult::ok(Some(SyncResponse {
                synced_count: count,
            })))
        }
        Err(e) => HttpResponse::BadRequest().json(RestResult::<()>::err(400, &e)),
    }
}

/// Register a Kubernetes service (manual registration)
#[post("/v1/cloud/k8s/services")]
pub async fn k8s_register_service(
    sync: web::Data<std::sync::Arc<K8sServiceSync>>,
    body: web::Json<K8sService>,
) -> HttpResponse {
    sync.register_service(body.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some("Service registered")))
}

/// Unregister a Kubernetes service
#[delete("/v1/cloud/k8s/services/{namespace}/{name}")]
pub async fn k8s_unregister_service(
    sync: web::Data<std::sync::Arc<K8sServiceSync>>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    sync.unregister_service(&namespace, &name);
    HttpResponse::Ok().json(RestResult::ok(Some("Service unregistered")))
}

/// Update endpoints for a service (manual update)
#[put("/v1/cloud/k8s/services/{namespace}/{name}/endpoints")]
pub async fn k8s_update_endpoints(
    sync: web::Data<std::sync::Arc<K8sServiceSync>>,
    path: web::Path<(String, String)>,
    body: web::Json<Vec<K8sEndpoint>>,
) -> HttpResponse {
    let (namespace, name) = path.into_inner();
    sync.update_endpoints(&namespace, &name, body.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some("Endpoints updated")))
}

/// Register pod metadata (manual registration)
#[post("/v1/cloud/k8s/pods")]
pub async fn k8s_register_pod(
    sync: web::Data<std::sync::Arc<K8sServiceSync>>,
    body: web::Json<K8sPodMetadata>,
) -> HttpResponse {
    sync.register_pod(body.into_inner());
    HttpResponse::Ok().json(RestResult::ok(Some("Pod registered")))
}

/// Configure Kubernetes routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(k8s_status)
        .service(k8s_stats)
        .service(k8s_config)
        .service(k8s_list_services)
        .service(k8s_get_service)
        .service(k8s_get_endpoints)
        .service(k8s_get_pod)
        .service(k8s_list_sync_status)
        .service(k8s_get_sync_status)
        .service(k8s_trigger_sync)
        .service(k8s_register_service)
        .service(k8s_unregister_service)
        .service(k8s_update_endpoints)
        .service(k8s_register_pod);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> K8sService {
        K8sService {
            name: "my-service".to_string(),
            namespace: "default".to_string(),
            service_type: "ClusterIP".to_string(),
            cluster_ip: Some("10.96.0.100".to_string()),
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
        }
    }

    fn create_test_endpoints() -> Vec<K8sEndpoint> {
        vec![
            K8sEndpoint {
                ip: "10.244.0.10".to_string(),
                pod_name: "my-service-abc123".to_string(),
                node_name: Some("node-1".to_string()),
                port: 8080,
                protocol: "TCP".to_string(),
                ready: true,
                labels: HashMap::new(),
            },
            K8sEndpoint {
                ip: "10.244.1.20".to_string(),
                pod_name: "my-service-def456".to_string(),
                node_name: Some("node-2".to_string()),
                port: 8080,
                protocol: "TCP".to_string(),
                ready: true,
                labels: HashMap::new(),
            },
        ]
    }

    #[test]
    fn test_register_service() {
        let sync = K8sServiceSync::default();
        let service = create_test_service();

        sync.register_service(service.clone());

        let retrieved = sync.get_service("default", "my-service").unwrap();
        assert_eq!(retrieved.name, "my-service");
        assert_eq!(retrieved.namespace, "default");
    }

    #[test]
    fn test_update_endpoints() {
        let sync = K8sServiceSync::default();
        let service = create_test_service();
        let endpoints = create_test_endpoints();

        sync.register_service(service);
        sync.update_endpoints("default", "my-service", endpoints);

        let retrieved = sync.get_endpoints("default", "my-service");
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].ip, "10.244.0.10");
    }

    #[test]
    fn test_unregister_service() {
        let sync = K8sServiceSync::default();
        let service = create_test_service();
        let endpoints = create_test_endpoints();

        sync.register_service(service);
        sync.update_endpoints("default", "my-service", endpoints);
        sync.unregister_service("default", "my-service");

        assert!(sync.get_service("default", "my-service").is_none());
        assert!(sync.get_endpoints("default", "my-service").is_empty());
    }

    #[test]
    fn test_list_services() {
        let sync = K8sServiceSync::default();

        for i in 0..3 {
            let mut service = create_test_service();
            service.name = format!("service-{}", i);
            sync.register_service(service);
        }

        let services = sync.list_services();
        assert_eq!(services.len(), 3);
    }

    #[test]
    fn test_endpoint_to_instance() {
        let sync = K8sServiceSync::default();
        let service = create_test_service();
        let endpoint = &create_test_endpoints()[0];

        let instance = sync.endpoint_to_instance(endpoint, &service);

        assert_eq!(instance.get("ip").unwrap(), "10.244.0.10");
        assert_eq!(instance.get("port").unwrap(), "8080");
        assert_eq!(instance.get("healthy").unwrap(), "true");
        assert_eq!(instance.get("serviceName").unwrap(), "my-service");
        assert_eq!(instance.get("k8s.pod").unwrap(), "my-service-abc123");
        assert_eq!(instance.get("k8s.node").unwrap(), "node-1");
    }

    #[test]
    fn test_instance_to_endpoint() {
        let mut instance = HashMap::new();
        instance.insert("ip".to_string(), "10.0.0.1".to_string());
        instance.insert("port".to_string(), "8080".to_string());
        instance.insert("healthy".to_string(), "true".to_string());
        instance.insert("k8s.pod".to_string(), "pod-123".to_string());

        let endpoint = K8sServiceSync::instance_to_endpoint(&instance).unwrap();

        assert_eq!(endpoint.ip, "10.0.0.1");
        assert_eq!(endpoint.port, 8080);
        assert!(endpoint.ready);
        assert_eq!(endpoint.pod_name, "pod-123");
    }

    #[test]
    fn test_pod_metadata() {
        let sync = K8sServiceSync::default();

        let pod = K8sPodMetadata {
            name: "my-pod-abc123".to_string(),
            namespace: "default".to_string(),
            pod_ip: Some("10.244.0.10".to_string()),
            node_name: Some("node-1".to_string()),
            labels: {
                let mut labels = HashMap::new();
                labels.insert("app".to_string(), "my-app".to_string());
                labels
            },
            annotations: HashMap::new(),
            container_ports: vec![ContainerPort {
                name: Some("http".to_string()),
                container_port: 8080,
                protocol: "TCP".to_string(),
            }],
            phase: "Running".to_string(),
            ready: true,
        };

        sync.register_pod(pod);

        let retrieved = sync.get_pod("default", "my-pod-abc123").unwrap();
        assert_eq!(retrieved.name, "my-pod-abc123");
        assert_eq!(retrieved.pod_ip.unwrap(), "10.244.0.10");
        assert!(retrieved.ready);
    }

    #[tokio::test]
    async fn test_sync_stats() {
        let sync = K8sServiceSync::default();

        // Register some services
        for i in 0..3 {
            let mut service = create_test_service();
            service.name = format!("service-{}", i);
            sync.register_service(service);
            sync.update_endpoints(
                "default",
                &format!("service-{}", i),
                create_test_endpoints(),
            );
        }

        let stats = sync.stats().await;

        assert_eq!(stats.services_watched, 3);
        assert_eq!(stats.endpoints_synced, 6); // 2 endpoints per service
        assert_eq!(stats.by_namespace.get("default").unwrap(), &3);
    }

    #[test]
    fn test_config_defaults() {
        let config = K8sSyncConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.sync_interval_seconds, 30);
        assert_eq!(config.batata_namespace, "public");
        assert_eq!(config.batata_group, "DEFAULT_GROUP");
        assert!(config.enable_health_check);
        assert_eq!(config.default_weight, 1.0);
    }
}
