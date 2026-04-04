//! `BatataClient` — unified entry point for all SDK operations.
//!
//! Creates and manages gRPC, HTTP, and server list components from a single
//! [`ClientConfig`]. This is the recommended way to use the SDK.
//!
//! # Example
//! ```no_run
//! use batata_client::{BatataClient, ClientConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = ClientConfig::new("127.0.0.1:8848")
//!     .with_auth("nacos", "nacos");
//!
//! let mut client = BatataClient::new(config).await?;
//!
//! // Config operations via gRPC
//! let config_svc = client.config_service().await?;
//! let content = config_svc.get_config("app.yaml", "DEFAULT_GROUP", "").await?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use crate::ClientConfig;
use crate::api::BatataApiClient;
use crate::config::BatataConfigService;
use crate::grpc::GrpcClient;
use crate::http::BatataHttpClient;
use crate::naming::BatataNamingService;
use crate::server_list::ServerListManager;

/// Unified Batata SDK client.
///
/// Manages shared components (server list, auth) and provides
/// typed access to Config, Naming, and Admin API services.
pub struct BatataClient {
    config: ClientConfig,
    server_list: Arc<ServerListManager>,
    grpc_config: Option<Arc<GrpcClient>>,
    grpc_naming: Option<Arc<GrpcClient>>,
    http: Option<Arc<BatataHttpClient>>,
}

impl BatataClient {
    /// Create a new BatataClient from unified configuration.
    ///
    /// Initializes the server list manager but does NOT connect gRPC or HTTP
    /// until the respective services are first accessed (lazy initialization).
    pub async fn new(config: ClientConfig) -> anyhow::Result<Self> {
        let server_list = Arc::new(ServerListManager::from_addrs(
            &config.server_addrs,
            config.server_max_failures,
        ));

        // Start address server refresh if configured
        if let Some(ref endpoint) = config.address_server_url
            && config.server_list_refresh_interval.as_millis() > 0
        {
            server_list.start_refresh_task(endpoint.clone(), config.server_list_refresh_interval);
        }

        Ok(Self {
            config,
            server_list,
            grpc_config: None,
            grpc_naming: None,
            http: None,
        })
    }

    /// Get (or create) the Config gRPC service.
    pub async fn config_service(&mut self) -> anyhow::Result<BatataConfigService> {
        if self.grpc_config.is_none() {
            let mut cfg = self.config.clone();
            cfg.module = "config".to_string();
            let grpc = Arc::new(GrpcClient::from_config(&cfg)?);
            grpc.connect().await?;
            self.grpc_config = Some(grpc);
        }
        // Safe: we just ensured grpc_config is Some above
        let grpc = Arc::clone(
            self.grpc_config
                .as_ref()
                .expect("grpc_config just initialized"),
        );
        Ok(BatataConfigService::new(grpc))
    }

    /// Get (or create) the Naming gRPC service.
    pub async fn naming_service(&mut self) -> anyhow::Result<BatataNamingService> {
        if self.grpc_naming.is_none() {
            let mut cfg = self.config.clone();
            cfg.module = "naming".to_string();
            let grpc = Arc::new(GrpcClient::from_config(&cfg)?);
            grpc.connect().await?;
            self.grpc_naming = Some(grpc);
        }
        // Safe: we just ensured grpc_naming is Some above
        let grpc = Arc::clone(
            self.grpc_naming
                .as_ref()
                .expect("grpc_naming just initialized"),
        );
        if self.config.naming_push_empty_protection {
            Ok(BatataNamingService::with_empty_protection(grpc, true))
        } else {
            Ok(BatataNamingService::new(grpc))
        }
    }

    /// Create a new HTTP API client for admin/maintainer operations.
    ///
    /// Each call creates a new `BatataApiClient` backed by a shared HTTP client.
    pub async fn api_client(&mut self) -> anyhow::Result<BatataApiClient> {
        if self.http.is_none() {
            let http = BatataHttpClient::from_config(&self.config).await?;
            self.http = Some(Arc::new(http));
        }
        // BatataApiClient needs an owned BatataHttpClient — create via from_config
        let http = BatataHttpClient::from_config(&self.config).await?;
        Ok(BatataApiClient::new(http))
    }

    /// Get the shared server list manager.
    pub fn server_list(&self) -> &Arc<ServerListManager> {
        &self.server_list
    }

    /// Get the configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Shutdown all services and release connections.
    pub fn shutdown(&mut self) {
        self.grpc_config = None;
        self.grpc_naming = None;
        self.http = None;
    }
}
