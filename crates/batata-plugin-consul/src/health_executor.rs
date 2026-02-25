// Health Check Executor
// Implements periodic execution of HTTP/TCP/GRPC health checks for Consul compatibility
// Uses HealthActor for lock-free status updates

use std::sync::Arc;
use std::time::Duration;

use tokio::time::sleep;

use crate::health::ConsulHealthService;
use crate::health_actor::HealthActorHandle;
use batata_naming::service::NamingService;

/// Health check executor that runs periodic checks
pub struct HealthCheckExecutor {
    health_service: Arc<ConsulHealthService>,
    health_actor: HealthActorHandle,
    naming_service: Arc<NamingService>,
    http_client: reqwest::Client,
}

impl HealthCheckExecutor {
    pub fn new(
        health_service: Arc<ConsulHealthService>,
        health_actor: HealthActorHandle,
        naming_service: Arc<NamingService>,
    ) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            health_service,
            health_actor,
            naming_service,
            http_client,
        }
    }

    /// Start the health check executor
    pub async fn start(&self) {
        tracing::info!("Starting Consul health check executor");

        let executor = self.clone();
        tokio::spawn(async move {
            executor.run_loop().await;
        });
    }

    /// Main execution loop
    async fn run_loop(&self) {
        use std::collections::HashMap;
        use tokio::time::Instant;

        // Track last execution time for each check
        let mut last_executed: HashMap<String, Instant> = HashMap::new();

        loop {
            // Collect all check information from actor (lock-free)
            let configs = self.health_actor.get_all_configs().await;

            tracing::debug!("Evaluating {} health checks for execution", configs.len());

            let now = Instant::now();

            for (check_id, config) in configs {
                let interval_secs = config
                    .interval
                    .as_ref()
                    .and_then(|t| parse_duration(t))
                    .unwrap_or(10);

                // Check if this check should be executed now
                let should_execute = match last_executed.get(&check_id) {
                    Some(last_time) => {
                        let elapsed = now.duration_since(*last_time).as_secs();
                        elapsed >= interval_secs
                    }
                    None => true, // Execute immediately on first run
                };

                if should_execute {
                    tracing::debug!(
                        "Executing check: id={}, type={}, interval={}s",
                        check_id,
                        config.check_type,
                        interval_secs
                    );

                    last_executed.insert(check_id.clone(), now);

                    // Execute check based on type
                    match config.check_type.as_str() {
                        "http" => self.execute_http_check(&check_id, &config).await,
                        "tcp" => self.execute_tcp_check(&check_id, &config).await,
                        "grpc" => self.execute_grpc_check(&check_id).await,
                        _ => tracing::warn!("Unsupported check type: {}", config.check_type),
                    }

                    tracing::info!("Check completed: {}", check_id);
                }
            }

            // Wait before next evaluation (1 second)
            sleep(Duration::from_secs(1)).await;
        }
    }

    /// Sync instance health status in NamingService based on check status
    fn sync_instance_health(&self, service_id: &str, service_name: &str, status: &str) {
        if service_id.is_empty() || service_name.is_empty() {
            return;
        }

        let healthy = status == "passing";

        // Parse service_id to extract instance information
        // Try different formats:
        // 1. service-ip-port (e.g., "file-service-192.168.3.47-8005")
        // 2. namespace@@group@@service@@ip@@port@@cluster
        let (namespace, group, ip, port, cluster) = if service_id.contains("@@") {
            let parts: Vec<&str> = service_id.split("@@").collect();
            if parts.len() >= 5 {
                let port: i32 = parts[4].parse().unwrap_or(0);
                let cluster = if parts.len() > 5 {
                    parts[5].to_string()
                } else {
                    "DEFAULT".to_string()
                };
                (
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[3].to_string(),
                    port,
                    cluster,
                )
            } else {
                (
                    "public".to_string(),
                    "DEFAULT_GROUP".to_string(),
                    "".to_string(),
                    0,
                    "DEFAULT".to_string(),
                )
            }
        } else if service_id.contains("-") {
            // Try to parse as service-ip-port format
            let last_dash = match service_id.rfind('-') {
                Some(idx) => idx,
                None => return,
            };
            let port_str = &service_id[last_dash + 1..];
            let port: i32 = match port_str.parse() {
                Ok(p) => p,
                Err(_) => return,
            };
            let prefix = &service_id[..last_dash];
            let last_dash2 = prefix.rfind('-');
            if let Some(idx) = last_dash2 {
                let ip = prefix[idx + 1..].to_string();
                // service_name is passed as a parameter, use it directly
                (
                    "public".to_string(),
                    "DEFAULT_GROUP".to_string(),
                    ip,
                    port,
                    "DEFAULT".to_string(),
                )
            } else {
                (
                    "public".to_string(),
                    "DEFAULT_GROUP".to_string(),
                    "".to_string(),
                    0,
                    "DEFAULT".to_string(),
                )
            }
        } else {
            // Can't parse, skip update
            return;
        };

        if !ip.is_empty() && port > 0 {
            let _ = self.naming_service.update_instance_health(
                &namespace,
                &group,
                service_name,
                &ip,
                port,
                &cluster,
                healthy,
            );

            tracing::debug!(
                "Synced instance health: service={}, ip={}, port={}, healthy={}",
                service_name,
                ip,
                port,
                healthy
            );
        }
    }

    /// Execute HTTP health check
    async fn execute_http_check(&self, check_id: &str, config: &crate::health_actor::CheckConfig) {
        tracing::info!("HTTP check {} called", check_id);

        let Some(ref url) = config.http else {
            tracing::warn!("No HTTP URL configured for check: {}", check_id);
            return;
        };

        // Parse timeout (default 10s)
        let timeout_secs = config
            .timeout
            .as_ref()
            .and_then(|t| parse_duration(t))
            .unwrap_or(10);

        tracing::info!(
            "HTTP check {} starting: url={}, timeout={}s",
            check_id,
            url,
            timeout_secs
        );

        // Execute HTTP request with timeout using client
        let result = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            self.http_client.get(url).send(),
        )
        .await;

        tracing::info!("HTTP check {} got result: {:?}", check_id, result.is_ok());

        match result {
            Ok(Ok(response)) => {
                tracing::info!(
                    "HTTP check {} inside Ok(Ok), status code: {:?}",
                    check_id,
                    response.status()
                );
                let status_code = response.status();
                let status = if status_code.is_success() {
                    "passing"
                } else {
                    "critical"
                };

                let output = if status == "passing" {
                    format!("HTTP check passed: {}", url)
                } else {
                    format!("HTTP check failed: {} returned {}", url, status_code)
                };

                tracing::info!(
                    "HTTP check {} updating status: status={}, output={}",
                    check_id,
                    status,
                    output
                );

                // Use actor message passing to update status (lock-free)
                let result = self
                    .health_actor
                    .update_status(
                        check_id.to_string(),
                        status.to_string(),
                        Some(output.clone()),
                    )
                    .await;

                match result {
                    Ok(()) => tracing::info!(
                        "HTTP check {}: status={}, http_status={}, output={}",
                        check_id,
                        status,
                        status_code,
                        output
                    ),
                    Err(e) => {
                        tracing::error!("HTTP check {} failed to update status: {}", check_id, e)
                    }
                }

                // Sync instance health status to NamingService
                self.sync_instance_health(&config.service_id, &config.service_name, status);
            }
            Ok(Err(e)) => {
                tracing::info!("HTTP check {} inside Ok(Err), error: {:?}", check_id, e);
                let output = format!("HTTP check error: {} - {}", url, e);

                let result = self
                    .health_actor
                    .update_status(
                        check_id.to_string(),
                        "critical".to_string(),
                        Some(output.clone()),
                    )
                    .await;

                match result {
                    Ok(()) => {
                        tracing::warn!("HTTP check {} failed: {}, output={}", check_id, e, output)
                    }
                    Err(e) => {
                        tracing::error!("HTTP check {} failed to update status: {}", check_id, e)
                    }
                }

                // Sync instance health status to NamingService
                self.sync_instance_health(&config.service_id, &config.service_name, "critical");
            }
            Err(_) => {
                tracing::info!("HTTP check {} inside Err(timeout)", check_id);
                let output = format!("HTTP check timeout: {}", url);

                let result = self
                    .health_actor
                    .update_status(
                        check_id.to_string(),
                        "critical".to_string(),
                        Some(output.clone()),
                    )
                    .await;

                match result {
                    Ok(()) => tracing::warn!("HTTP check {} timeout, output={}", check_id, output),
                    Err(e) => {
                        tracing::error!("HTTP check {} failed to update status: {}", check_id, e)
                    }
                }

                // Sync instance health status to NamingService
                self.sync_instance_health(&config.service_id, &config.service_name, "critical");
            }
        }
    }

    /// Execute TCP health check
    async fn execute_tcp_check(&self, check_id: &str, config: &crate::health_actor::CheckConfig) {
        let Some(ref addr) = config.tcp else {
            tracing::warn!("No TCP address configured for check: {}", check_id);
            return;
        };

        // Parse timeout (default 10s)
        let timeout_secs = config
            .timeout
            .as_ref()
            .and_then(|t| parse_duration(t))
            .unwrap_or(10);

        // Try to connect with timeout
        let result = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            tokio::net::TcpStream::connect(addr),
        )
        .await;

        match result {
            Ok(Ok(_stream)) => {
                let output = format!("TCP check passed: {}", addr);

                let _ = self
                    .health_actor
                    .update_status(check_id.to_string(), "passing".to_string(), Some(output))
                    .await;

                tracing::debug!("TCP check {}: passed", check_id);

                // Sync instance health status to NamingService
                self.sync_instance_health(&config.service_id, &config.service_name, "passing");
            }
            Ok(Err(e)) => {
                let output = format!("TCP check failed: {} - {}", addr, e);

                let _ = self
                    .health_actor
                    .update_status(check_id.to_string(), "critical".to_string(), Some(output))
                    .await;

                tracing::warn!("TCP check {} failed: {}", check_id, e);

                // Sync instance health status to NamingService
                self.sync_instance_health(&config.service_id, &config.service_name, "critical");
            }
            Err(_) => {
                let output = format!("TCP check timeout: {}", addr);

                let _ = self
                    .health_actor
                    .update_status(check_id.to_string(), "critical".to_string(), Some(output))
                    .await;

                tracing::warn!("TCP check {} timeout", check_id);

                // Sync instance health status to NamingService
                self.sync_instance_health(&config.service_id, &config.service_name, "critical");
            }
        }
    }

    /// Execute GRPC health check
    async fn execute_grpc_check(&self, check_id: &str) {
        // For now, mark as warning since GRPC checks are not fully implemented
        let output = "GRPC checks are not fully implemented yet".to_string();

        let _ = self
            .health_actor
            .update_status(check_id.to_string(), "warning".to_string(), Some(output))
            .await;

        tracing::warn!("GRPC check {} not fully implemented", check_id);
    }
}

impl Clone for HealthCheckExecutor {
    fn clone(&self) -> Self {
        Self {
            health_service: Arc::clone(&self.health_service),
            health_actor: self.health_actor.clone(),
            naming_service: Arc::clone(&self.naming_service),
            http_client: self.http_client.clone(),
        }
    }
}

/// Parse duration string (e.g., "10s", "30s", "1m") to seconds
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
