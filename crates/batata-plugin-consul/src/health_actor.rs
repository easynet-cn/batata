// Health Check Actor - Message-based concurrency pattern
// Separates check configuration from dynamic health status
// Eliminates read/write lock contention through actor pattern

use std::sync::Arc;
use std::time::UNIX_EPOCH;

use dashmap::DashMap;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender, UnboundedReceiver};
use tracing::{info, warn};

use crate::model::{HealthCheck, CheckRegistration};

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
}

/// Health check status (dynamic, write-frequent)
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub status: String,           // "passing", "critical", "warning"
    pub output: String,
    pub last_updated: i64,
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
        let health_status = HealthStatus {
            status,
            output: output.unwrap_or_default(),
            last_updated: current_timestamp(),
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
            HealthActorMessage::RegisterCheck { check_config, respond_to } => {
                self.config_registry.register(check_config);
                let _ = respond_to.send(Ok(()));
            }

            HealthActorMessage::DeregisterCheck { check_id, respond_to } => {
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

            HealthActorMessage::GetConfig { check_id, respond_to } => {
                let config = self.config_registry.get(&check_id);
                let _ = respond_to.send(config);
            }

            HealthActorMessage::GetStatus { check_id, respond_to } => {
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
                            create_index: Some(1),
                            modify_index: Some(1),
                        });
                    }
                }
                let _ = respond_to.send(checks);
            }

            HealthActorMessage::GetServiceChecks { service_id, respond_to } => {
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
        let msg = HealthActorMessage::DeregisterCheck { check_id, respond_to: tx };
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
        let msg = HealthActorMessage::GetConfig { check_id, respond_to: tx };
        let _ = self.tx.send(msg);
        rx.await.ok().flatten()
    }

    pub async fn get_status(&self, check_id: String) -> Option<HealthStatus> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::GetStatus { check_id, respond_to: tx };
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
        let msg = HealthActorMessage::GetByStatus { status, respond_to: tx };
        let _ = self.tx.send(msg);
        rx.await.unwrap_or_default()
    }

    pub async fn get_service_checks(&self, service_id: String) -> Vec<HealthCheck> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = HealthActorMessage::GetServiceChecks { service_id, respond_to: tx };
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

        CheckConfig {
            check_id: registration.effective_check_id(),
            name: registration.name.clone(),
            service_id: registration.service_id.clone().unwrap_or_default(),
            service_name: String::new(), // Will be filled later
            check_type: registration.check_type().to_string(),
            http: registration.http.clone(),
            tcp: registration.tcp.clone(),
            grpc: registration.grpc.clone(),
            interval: registration.interval.clone(),
            timeout: registration.timeout.clone(),
            ttl_seconds,
            deregister_after_secs,
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

use std::time::SystemTime;

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
