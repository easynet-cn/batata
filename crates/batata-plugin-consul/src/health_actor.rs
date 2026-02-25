// Health Check Actor - Message-based concurrency pattern
// Separates check configuration from dynamic health status
// Eliminates read/write lock contention through actor pattern

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::{info, warn};

use crate::model::{CheckRegistration, HealthCheck};

/// Health check configuration (static, read-mostly)
#[derive(Debug, Clone)]
pub struct CheckConfig {
    pub check_id: String,
    pub name: String,
    pub service_id: String,
    pub service_name: String,
    pub check_type: String,
    pub http: Option<String>,
    pub tcp: Option<String>,
    pub grpc: Option<String>,
    pub interval: Option<String>,
    pub timeout: Option<String>,
    pub ttl_seconds: Option<u64>,
    pub deregister_after_secs: Option<u64>,
    pub initial_status: String, // Initial status when check is registered (default: "critical")
}

/// Health check status (dynamic, write-frequent)
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub status: String, // "passing", "critical", "warning"
    pub output: String,
    pub last_updated: i64,
    pub critical_time: Option<i64>, // Timestamp when status became critical
}

/// Actor messages
#[derive(Debug)]
pub enum HealthActorMessage {
    /// Register a new check configuration
    RegisterCheck {
        check_config: CheckConfig,
        respond_to: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Deregister a check
    DeregisterCheck {
        check_id: String,
        respond_to: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Update health check status
    UpdateStatus {
        check_id: String,
        status: String,
        output: Option<String>,
        respond_to: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Get check configuration
    GetConfig {
        check_id: String,
        respond_to: tokio::sync::oneshot::Sender<Option<CheckConfig>>,
    },
    /// Get health status
    GetStatus {
        check_id: String,
        respond_to: tokio::sync::oneshot::Sender<Option<HealthStatus>>,
    },
    /// Get all check configurations
    GetAllConfigs {
        respond_to: tokio::sync::oneshot::Sender<Vec<(String, CheckConfig)>>,
    },
    /// Get all health statuses
    GetAllStatuses {
        respond_to: tokio::sync::oneshot::Sender<Vec<(String, HealthStatus)>>,
    },
    /// Get checks by status
    GetByStatus {
        status: String,
        respond_to: tokio::sync::oneshot::Sender<Vec<HealthCheck>>,
    },
    /// Get checks for a service
    GetServiceChecks {
        service_id: String,
        respond_to: tokio::sync::oneshot::Sender<Vec<HealthCheck>>,
    },
}

/// Check configuration registry (static, read-mostly)
pub struct CheckConfigRegistry {
    configs: Arc<DashMap<String, CheckConfig>>,
    service_checks: Arc<DashMap<String, Vec<String>>>,
}

impl CheckConfigRegistry {
    pub fn new() -> Self {
        Self {
            configs: Arc::new(DashMap::new()),
            service_checks: Arc::new(DashMap::new()),
        }
    }

    pub fn register(&self, config: CheckConfig) {
        let check_id = config.check_id.clone();
        let service_id = config.service_id.clone();

        self.configs.insert(check_id.clone(), config);

        // Update service_checks mapping
        if !service_id.is_empty() {
            self.service_checks
                .entry(service_id)
                .or_default()
                .push(check_id);
        }
    }

    pub fn deregister(&self, check_id: &str) -> Option<CheckConfig> {
        let config = self.configs.remove(check_id).map(|(_, c)| c);

        // Remove from service_checks mapping
        if let Some(ref config) = config
            && !config.service_id.is_empty()
            && let Some(mut checks) = self.service_checks.get_mut(&config.service_id)
        {
            checks.retain(|id| id != check_id);
        }

        config
    }

    pub fn get(&self, check_id: &str) -> Option<CheckConfig> {
        self.configs.get(check_id).map(|c| c.clone())
    }

    pub fn get_all(&self) -> Vec<(String, CheckConfig)> {
        self.configs
            .iter()
            .map(|entry| {
                let k = entry.key().clone();
                let v = entry.value().clone();
                (k, v)
            })
            .collect()
    }

    pub fn get_service_check_ids(&self, service_id: &str) -> Vec<String> {
        self.service_checks
            .get(service_id)
            .map(|v| v.clone())
            .unwrap_or_default()
    }
}

/// Health status store (dynamic, write-frequent)
pub struct HealthStatusStore {
    statuses: Arc<DashMap<String, HealthStatus>>,
}

impl HealthStatusStore {
    pub fn new() -> Self {
        Self {
            statuses: Arc::new(DashMap::new()),
        }
    }

    pub fn update(&self, check_id: &str, status: String, output: Option<String>) {
        // Get old status to track critical time
        let existing = self.statuses.get(check_id);
        let critical_time = match (existing, status.as_str()) {
            (Some(old), "critical") => old.critical_time.or(Some(current_timestamp())),
            (_, "critical") => Some(current_timestamp()),
            _ => None, // Non-critical status clears critical_time
        };

        let health_status = HealthStatus {
            status,
            output: output.unwrap_or_default(),
            last_updated: current_timestamp(),
            critical_time,
        };
        self.statuses.insert(check_id.to_string(), health_status);
    }

    pub fn get(&self, check_id: &str) -> Option<HealthStatus> {
        self.statuses.get(check_id).map(|s| s.clone())
    }

    pub fn get_all(&self) -> Vec<(String, HealthStatus)> {
        self.statuses
            .iter()
            .map(|entry| {
                let k = entry.key().clone();
                let v = entry.value().clone();
                (k, v)
            })
            .collect()
    }

    pub fn get_by_status(&self, status_filter: &str) -> Vec<String> {
        self.statuses
            .iter()
            .filter(|entry| entry.value().status == status_filter)
            .map(|entry| entry.key().clone())
            .collect()
    }
}

/// Health Status Actor - Coordinates config and status stores via messages
pub struct HealthStatusActor {
    config_registry: CheckConfigRegistry,
    status_store: HealthStatusStore,
    rx: UnboundedReceiver<HealthActorMessage>,
}

impl HealthStatusActor {
    pub fn new(rx: UnboundedReceiver<HealthActorMessage>) -> Self {
        Self {
            config_registry: CheckConfigRegistry::new(),
            status_store: HealthStatusStore::new(),
            rx,
        }
    }

    /// Run the actor message loop
    pub async fn run(mut self) {
        info!("HealthStatusActor started");
        while let Some(msg) = self.rx.recv().await {
            self.handle_message(msg).await;
        }
        warn!("HealthStatusActor stopped");
    }

    async fn handle_message(&mut self, msg: HealthActorMessage) {
        match msg {
            HealthActorMessage::RegisterCheck {
                check_config,
                respond_to,
            } => {
                let check_id = check_config.check_id.clone();
                let initial_status = check_config.initial_status.clone();

                self.config_registry.register(check_config);

                // Initialize health status with initial status
                self.status_store
                    .update(&check_id, initial_status.clone(), None);

                info!(
                    "Registered check: id={}, initial_status={}",
                    check_id, initial_status
                );
                let _ = respond_to.send(Ok(()));
            }

            HealthActorMessage::DeregisterCheck {
                check_id,
                respond_to,
            } => {
                let result = self
                    .config_registry
                    .deregister(&check_id)
                    .map(|_| ())
                    .ok_or_else(|| format!("Check not found: {}", check_id));
                let _ = respond_to.send(result);
            }

            HealthActorMessage::UpdateStatus {
                check_id,
                status,
                output,
                respond_to,
            } => {
                self.status_store.update(&check_id, status, output);
                let _ = respond_to.send(Ok(()));
            }

            HealthActorMessage::GetConfig {
                check_id,
                respond_to,
            } => {
                let config = self.config_registry.get(&check_id);
                let _ = respond_to.send(config);
            }

            HealthActorMessage::GetStatus {
                check_id,
                respond_to,
            } => {
                let status = self.status_store.get(&check_id);
                let _ = respond_to.send(status);
            }

            HealthActorMessage::GetAllConfigs { respond_to } => {
                let configs = self.config_registry.get_all();
                let _ = respond_to.send(configs);
            }

            HealthActorMessage::GetAllStatuses { respond_to } => {
                let statuses = self.status_store.get_all();
                let _ = respond_to.send(statuses);
            }

            HealthActorMessage::GetByStatus { status, respond_to } => {
                let check_ids = self.status_store.get_by_status(&status);

                // Build HealthCheck objects by merging config + status
                let mut checks = Vec::new();
                for check_id in check_ids {
                    if let Some(config) = self.config_registry.get(&check_id)
                        && let Some(status_data) = self.status_store.get(&check_id)
                    {
                        checks.push(HealthCheck {
                            node: "batata-node".to_string(),
                            check_id: config.check_id,
                            name: config.name,
                            status: status_data.status,
                            notes: String::new(),
                            output: status_data.output,
                            service_id: config.service_id,
                            service_name: config.service_name,
                            service_tags: None,
                            check_type: config.check_type,
                            interval: config.interval.clone(),
                            timeout: config.timeout.clone(),
                            create_index: Some(1),
                            modify_index: Some(1),
                        });
                    }
                }
                let _ = respond_to.send(checks);
            }

            HealthActorMessage::GetServiceChecks {
                service_id,
                respond_to,
            } => {
                let check_ids = self.config_registry.get_service_check_ids(&service_id);

                let mut checks = Vec::new();
                for check_id in check_ids {
                    if let Some(config) = self.config_registry.get(&check_id)
                        && let Some(status_data) = self.status_store.get(&check_id)
                    {
                        checks.push(HealthCheck {
                            node: "batata-node".to_string(),
                            check_id: config.check_id,
                            name: config.name,
                            status: status_data.status,
                            notes: String::new(),
                            output: status_data.output,
                            service_id: config.service_id,
                            service_name: config.service_name,
                            service_tags: None,
                            check_type: config.check_type,
                            interval: config.interval.clone(),
                            timeout: config.timeout.clone(),
                            create_index: Some(1),
                            modify_index: Some(1),
                        });
                    }
                }
                let _ = respond_to.send(checks);
            }
        }
    }
}

/// Actor handle for sending messages
#[derive(Clone)]
pub struct HealthActorHandle {
    tx: UnboundedSender<HealthActorMessage>,
}

impl HealthActorHandle {
    pub fn new(tx: UnboundedSender<HealthActorMessage>) -> Self {
        Self { tx }
    }

    pub async fn register_check(&self, check_config: CheckConfig) -> Result<(), String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::RegisterCheck {
            check_config,
            respond_to: tx,
        };
        let _ = self.tx.send(msg);
        rx.await.map_err(|e| format!("Send error: {}", e))?
    }

    pub async fn deregister_check(&self, check_id: String) -> Result<(), String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::DeregisterCheck {
            check_id,
            respond_to: tx,
        };
        let _ = self.tx.send(msg);
        rx.await.map_err(|e| format!("Send error: {}", e))?
    }

    pub async fn update_status(
        &self,
        check_id: String,
        status: String,
        output: Option<String>,
    ) -> Result<(), String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::UpdateStatus {
            check_id,
            status,
            output,
            respond_to: tx,
        };
        let _ = self.tx.send(msg);
        rx.await.map_err(|e| format!("Send error: {}", e))?
    }

    pub async fn get_config(&self, check_id: String) -> Option<CheckConfig> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::GetConfig {
            check_id,
            respond_to: tx,
        };
        let _ = self.tx.send(msg);
        rx.await.ok().flatten()
    }

    pub async fn get_status(&self, check_id: String) -> Option<HealthStatus> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::GetStatus {
            check_id,
            respond_to: tx,
        };
        let _ = self.tx.send(msg);
        rx.await.ok().flatten()
    }

    pub async fn get_all_configs(&self) -> Vec<(String, CheckConfig)> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::GetAllConfigs { respond_to: tx };
        let _ = self.tx.send(msg);
        rx.await.unwrap_or_default()
    }

    pub async fn get_all_status(&self) -> Vec<(String, HealthStatus)> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::GetAllStatuses { respond_to: tx };
        let _ = self.tx.send(msg);
        rx.await.unwrap_or_default()
    }

    pub async fn get_checks_by_status(&self, status: String) -> Vec<HealthCheck> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::GetByStatus {
            status,
            respond_to: tx,
        };
        let _ = self.tx.send(msg);
        rx.await.unwrap_or_default()
    }

    pub async fn get_service_checks(&self, service_id: String) -> Vec<HealthCheck> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::GetServiceChecks {
            service_id,
            respond_to: tx,
        };
        let _ = self.tx.send(msg);
        rx.await.unwrap_or_default()
    }

    /// Convert CheckRegistration to CheckConfig
    pub fn registration_to_config(registration: &CheckRegistration) -> CheckConfig {
        let ttl_seconds = registration.ttl.as_ref().and_then(|s| parse_duration(s));
        let deregister_after_secs = registration
            .deregister_critical_service_after
            .as_ref()
            .and_then(|s| parse_duration(s));

        // Get initial status (defaults to "critical" as per Consul behavior)
        let initial_status = registration
            .status
            .clone()
            .unwrap_or_else(|| "critical".to_string());

        CheckConfig {
            check_id: registration.effective_check_id(),
            name: registration.name.clone(),
            service_id: registration.service_id.clone().unwrap_or_default(),
            service_name: registration.service_name.clone().unwrap_or_default(),
            check_type: registration.check_type().to_string(),
            http: registration.http.clone(),
            tcp: registration.tcp.clone(),
            grpc: registration.grpc.clone(),
            interval: registration.interval.clone(),
            timeout: registration.timeout.clone(),
            ttl_seconds,
            deregister_after_secs,
            initial_status,
        }
    }
}

/// Create a new HealthStatusActor and return its handle
pub fn create_health_actor() -> HealthActorHandle {
    let (tx, rx) = unbounded_channel();
    let actor = HealthStatusActor::new(rx);
    tokio::spawn(async move {
        actor.run().await;
    });
    HealthActorHandle::new(tx)
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.ends_with('s') {
        s[..s.len() - 1].parse().ok()
    } else if s.ends_with('m') {
        s[..s.len() - 1].parse::<u64>().ok().map(|m| m * 60)
    } else if s.ends_with('h') {
        s[..s.len() - 1].parse::<u64>().ok().map(|h| h * 3600)
    } else {
        s.parse().ok()
    }
}

// ============================================================================
// Service Deregistration Monitor (Critical Check Auto-Remove)
// ============================================================================

/// Start the service deregistration monitor for critical health checks
pub async fn start_deregistration_monitor(
    health_actor: HealthActorHandle,
    naming_service: Option<Arc<dyn NamingServiceTrait>>,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        info!(
            "Starting service deregistration monitor with interval: {}s",
            interval_secs
        );
        loop {
            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
            reap_services(health_actor.clone(), naming_service.clone()).await;
        }
    });
}

/// Reap services that have been in critical state for too long
async fn reap_services(
    health_actor: HealthActorHandle,
    naming_service: Option<Arc<dyn NamingServiceTrait>>,
) {
    let configs = health_actor.get_all_configs().await;
    let statuses = health_actor.get_all_status().await;

    let now = current_timestamp();
    let mut deregistered_services = std::collections::HashSet::new();

    for (check_id, config) in configs {
        if let Some(deregister_after_secs) = config.deregister_after_secs {
            if let Some((_, status_data)) = statuses.iter().find(|(id, _)| id == &check_id) {
                if status_data.status == "critical" {
                    if let Some(critical_time) = status_data.critical_time {
                        let critical_duration_secs = now - critical_time;
                        if critical_duration_secs >= deregister_after_secs as i64 {
                            // Exceeded threshold, deregister the service
                            if !config.service_id.is_empty()
                                && !deregistered_services.contains(&config.service_id)
                            {
                                info!(
                                    "Deregistering service due to critical health: service={}, check={}, duration={}s, threshold={}s",
                                    config.service_id,
                                    check_id,
                                    critical_duration_secs,
                                    deregister_after_secs
                                );

                                if let Some(ns) = &naming_service {
                                    let _ = ns
                                        .deregister_instance(
                                            "public",
                                            "DEFAULT_GROUP",
                                            &config.service_name,
                                            &config.service_id,
                                            true, // ephemeral: service instances are typically ephemeral
                                        )
                                        .await;
                                }

                                deregistered_services.insert(config.service_id.clone());

                                // Also deregister the check
                                let _ = health_actor.deregister_check(check_id.clone()).await;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Trait for naming service operations (for decoupling)
#[async_trait::async_trait]
pub trait NamingServiceTrait: Send + Sync {
    async fn deregister_instance(
        &self,
        namespace: &str,
        group: &str,
        service_name: &str,
        instance_id: &str,
        ephemeral: bool,
    ) -> Result<(), String>;
}
