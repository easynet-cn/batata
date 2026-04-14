use std::collections::HashMap;
use std::sync::Arc;

use crate::{ClientConnectionManager, model::Connection};
use sysinfo::System;
use tonic::Status;
use tracing::{info, warn};

use crate::{
    api::{
        grpc::Payload,
        remote::model::{
            ClientDetectionResponse, ConnectResetResponse, HealthCheckResponse, Response,
            ResponseTrait, ServerCheckResponse, ServerLoaderInfoResponse, ServerReloadResponse,
            SetupAckResponse,
        },
    },
    handler::rpc::{AuthRequirement, PayloadHandler},
};

#[derive(Clone)]
pub struct HealthCheckHandler {}

#[tonic::async_trait]
impl PayloadHandler for HealthCheckHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        let response = HealthCheckResponse {
            response: Response::new(),
        };

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "HealthCheckRequest"
    }
}

#[derive(Clone)]
pub struct ServerCheckHandler {}

#[tonic::async_trait]
impl PayloadHandler for ServerCheckHandler {
    async fn handle(&self, connection: &Connection, _: &Payload) -> Result<Payload, Status> {
        let response = ServerCheckResponse {
            response: Response::new(),
            connection_id: connection.meta_info.connection_id.clone(),
            support_ability_negotiation: true,
        };

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ServerCheckRequest"
    }
}

#[derive(Clone)]
pub struct ConnectionSetupHandler {}

#[tonic::async_trait]
impl PayloadHandler for ConnectionSetupHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        // Connection setup is handled in the bi-stream service
        // Just return the payload as acknowledgment
        Ok(payload.clone())
    }

    fn can_handle(&self) -> &'static str {
        "ConnectionSetupRequest"
    }
}

// Handler for ClientDetectionRequest - detects client status
#[derive(Clone)]
pub struct ClientDetectionHandler {}

#[tonic::async_trait]
impl PayloadHandler for ClientDetectionHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        let response = ClientDetectionResponse::new();

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ClientDetectionRequest"
    }
}

// Handler for ServerLoaderInfoRequest - returns server load information
#[derive(Clone)]
pub struct ServerLoaderInfoHandler {
    pub connection_manager: Arc<dyn ClientConnectionManager>,
}

#[tonic::async_trait]
impl PayloadHandler for ServerLoaderInfoHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        let mut response = ServerLoaderInfoResponse::new();

        // Get real connection count from ConnectionManager
        let sdk_con_count = self.connection_manager.connection_count();

        // Get system metrics using sysinfo
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        // Calculate CPU usage (average across all cores)
        let cpu_usage = sys.cpus().iter().map(|cpu| cpu.cpu_usage()).sum::<f32>()
            / sys.cpus().len().max(1) as f32;

        // Calculate memory usage percentage
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let mem_usage = if total_memory > 0 {
            (used_memory as f64 / total_memory as f64 * 100.0) as f32
        } else {
            0.0
        };

        // Get system load average (1 minute)
        let load_avg = System::load_average().one;

        // Build metrics map with real values
        let mut metrics = HashMap::with_capacity(4);
        metrics.insert("sdkConCount".to_string(), sdk_con_count.to_string());
        metrics.insert("cpu".to_string(), format!("{:.1}", cpu_usage));
        metrics.insert("load".to_string(), format!("{:.2}", load_avg));
        metrics.insert("mem".to_string(), format!("{:.1}", mem_usage));
        response.loader_metrics = metrics;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ServerLoaderInfoRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }

    fn sign_type(&self) -> &'static str {
        "internal"
    }

    fn resource_type(&self) -> crate::ResourceType {
        crate::ResourceType::Internal
    }
}

// Handler for ServerReloadRequest - triggers server configuration reload
#[derive(Clone)]
pub struct ServerReloadHandler {
    /// Configuration file path (e.g., "conf/application.yml")
    pub config_path: String,
}

#[tonic::async_trait]
impl PayloadHandler for ServerReloadHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let client_ip = payload
            .metadata
            .as_ref()
            .map(|m| m.client_ip.as_str())
            .unwrap_or("");
        info!(
            "Server reload requested from client: {} ({}:{})",
            connection.meta_info.connection_id, client_ip, connection.meta_info.remote_port
        );

        // Attempt to reload configuration
        let reload_result = self.try_reload_config().await;

        match reload_result {
            Ok(message) => {
                info!("Server configuration reloaded successfully: {}", message);
                let mut response = ServerReloadResponse::new();
                response.response.result_code = 200;
                response.response.success = true;
                response.response.message = message;
                response.response.request_id = connection.meta_info.connection_id.clone();
                Ok(response.build_payload())
            }
            Err(e) => {
                warn!("Server configuration reload failed: {}", e);
                let mut response = ServerReloadResponse::new();
                response.response.result_code = 500;
                response.response.success = false;
                response.response.message = format!("Reload failed: {}", e);
                response.response.request_id = connection.meta_info.connection_id.clone();
                Ok(response.build_payload())
            }
        }
    }

    fn can_handle(&self) -> &'static str {
        "ServerReloadRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }

    fn sign_type(&self) -> &'static str {
        "internal"
    }

    fn resource_type(&self) -> crate::ResourceType {
        crate::ResourceType::Internal
    }
}

impl ServerReloadHandler {
    /// Attempt to reload configuration from file
    async fn try_reload_config(&self) -> anyhow::Result<String> {
        // Check if config file exists
        let config_path = std::path::Path::new(&self.config_path);
        if !config_path.exists() {
            return Err(anyhow::anyhow!(
                "Configuration file not found: {}",
                self.config_path
            ));
        }

        // Read and validate configuration
        let content = tokio::fs::read_to_string(&self.config_path).await?;

        // Basic validation: check for required sections
        if !content.contains("batata.server.main.port")
            && !content.contains("batata.server.main-port")
        {
            return Err(anyhow::anyhow!(
                "Invalid configuration: missing required 'batata.server.main.port' section"
            ));
        }

        // Note: Full hot-reload would require:
        // 1. Parse YAML/Properties into Configuration struct
        // 2. Update shared AppState (requires Arc<RwLock<Configuration>> wrapper)
        // 3. Notify dependent services (naming, config, auth, etc.)
        // 4. Handle configuration validation errors gracefully
        //
        // Current implementation validates configuration and logs the action.
        // For production use, the server should be restarted to apply configuration changes.

        info!("Configuration file validated successfully. To apply changes, restart the server.");

        Ok(format!(
            "Configuration validated successfully from {}",
            self.config_path
        ))
    }
}

// Handler for ConnectResetRequest - handles connection reset
#[derive(Clone)]
pub struct ConnectResetHandler {}

#[tonic::async_trait]
impl PayloadHandler for ConnectResetHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let client_ip = payload
            .metadata
            .as_ref()
            .map(|m| m.client_ip.as_str())
            .unwrap_or("");
        // Log the connection reset request
        info!(
            "Connection reset requested: {} from {}:{}",
            connection.meta_info.connection_id, client_ip, connection.meta_info.remote_port
        );

        // Connection cleanup is handled by the bi-stream service when the stream closes.
        // This handler acknowledges the reset request.
        let response = ConnectResetResponse::new();

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ConnectResetRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Write
    }

    fn sign_type(&self) -> &'static str {
        "internal"
    }

    fn resource_type(&self) -> crate::ResourceType {
        crate::ResourceType::Internal
    }
}

// Handler for SetupAckRequest - acknowledges connection setup
#[derive(Clone)]
pub struct SetupAckHandler {}

#[tonic::async_trait]
impl PayloadHandler for SetupAckHandler {
    async fn handle(
        &self,
        _connection: &Connection,
        _payload: &Payload,
    ) -> Result<Payload, Status> {
        let response = SetupAckResponse::new();

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "SetupAckRequest"
    }
}

// Handler for PushAckRequest - acknowledges server push
#[derive(Clone)]
pub struct PushAckHandler {}

#[tonic::async_trait]
impl PayloadHandler for PushAckHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        // PushAck doesn't require a response, just acknowledge
        Ok(payload.clone())
    }

    fn can_handle(&self) -> &'static str {
        "PushAckRequest"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::api::remote::model::{RequestTrait, ServerLoaderInfoRequest};
    use async_trait::async_trait;
    use batata_common::ConnectionInfo;

    /// Minimal `ClientConnectionManager` stub that reports a fixed connection
    /// count. Used to validate that `ServerLoaderInfoHandler::handle()`
    /// exposes the real count (not a hardcoded zero) in `sdkConCount`.
    struct StubConnManager {
        count: usize,
    }

    #[async_trait]
    impl ClientConnectionManager for StubConnManager {
        fn connection_count(&self) -> usize {
            self.count
        }
        fn has_connection(&self, _id: &str) -> bool {
            false
        }
        fn get_all_connection_ids(&self) -> Vec<String> {
            Vec::new()
        }
        fn get_connection_info(&self, _id: &str) -> Option<ConnectionInfo> {
            None
        }
        fn get_all_connection_infos(&self) -> Vec<ConnectionInfo> {
            Vec::new()
        }
        fn connections_for_ip(&self, _ip: &str) -> usize {
            0
        }
        async fn push_message(&self, _id: &str, _payload: Payload) -> bool {
            false
        }
        async fn push_message_to_many(&self, _ids: &[String], _payload: Payload) -> usize {
            0
        }
        async fn load_single(&self, _id: &str, _redirect: Option<&str>) -> bool {
            false
        }
        async fn load_count(&self, _target: usize, _redirect: Option<&str>) -> usize {
            0
        }
        fn touch_connection(&self, _id: &str) {}
    }

    #[tokio::test]
    async fn server_loader_info_handler_returns_real_metrics_shape() {
        let stub = Arc::new(StubConnManager { count: 42 });
        let handler = ServerLoaderInfoHandler {
            connection_manager: stub,
        };

        let conn = Connection::default();
        let request = ServerLoaderInfoRequest::new();
        let payload = request.to_payload(None);
        let response_payload = handler
            .handle(&conn, &payload)
            .await
            .expect("handler should succeed");

        // Decode response and validate every expected field is present with
        // real data (not zeros), matching Nacos ServerLoaderInfoResponse.
        // `ResponseTrait` has no `from_payload`; decode the JSON body directly.
        let bytes: &[u8] = response_payload
            .body
            .as_ref()
            .map(|b| b.value.as_slice())
            .unwrap_or(&[]);
        let response: ServerLoaderInfoResponse =
            serde_json::from_slice(bytes).expect("valid response JSON");

        // sdkConCount must reflect the stub's real value (not hardcoded 0).
        let sdk_count = response
            .loader_metrics
            .get("sdkConCount")
            .expect("sdkConCount must be present");
        assert_eq!(sdk_count, "42", "sdkConCount must be the real count");

        // cpu, load, and mem must be present and parseable as floats.
        let cpu = response
            .loader_metrics
            .get("cpu")
            .expect("cpu metric must be present");
        assert!(
            cpu.parse::<f32>().is_ok(),
            "cpu must be a parseable float, got '{}'",
            cpu
        );
        let load = response
            .loader_metrics
            .get("load")
            .expect("load metric must be present");
        assert!(
            load.parse::<f32>().is_ok(),
            "load must be a parseable float, got '{}'",
            load
        );
        let mem = response
            .loader_metrics
            .get("mem")
            .expect("mem metric must be present");
        assert!(
            mem.parse::<f32>().is_ok(),
            "mem must be a parseable float, got '{}'",
            mem
        );

        // Handler contract: internal auth, can_handle identity.
        assert_eq!(handler.can_handle(), "ServerLoaderInfoRequest");
        assert!(matches!(
            handler.auth_requirement(),
            AuthRequirement::Internal
        ));
    }
}
