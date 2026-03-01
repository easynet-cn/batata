//! Application state management
//!
//! This module defines the central application state shared across all handlers.

use std::{any::Any, collections::HashMap, sync::Arc};

use batata_auth::service::oauth::OAuthService;
use batata_consistency::RaftNode;
use batata_core::cluster::ServerMemberManager;
use batata_persistence::PersistenceService;

use crate::console::datasource::ConsoleDataSource;

use super::server_status::ServerStatusManager;

use super::{
    config::Configuration,
    constants::{
        AUTH_ADMIN_REQUEST, AUTH_ENABLED, AUTH_SYSTEM_TYPE, CONFIG_RENTENTION_DAYS_PROPERTY_STATE,
        CONSUL_ENABLED_STATE, CONSUL_PORT_STATE, DATASOURCE_PLATFORM_PROPERTY_STATE,
        DEFAULT_CLUSTER_QUOTA, DEFAULT_GROUP_QUOTA, DEFAULT_MAX_AGGR_COUNT, DEFAULT_MAX_AGGR_SIZE,
        DEFAULT_MAX_SIZE, FUNCTION_MODE_STATE, IS_CAPACITY_LIMIT_CHECK, IS_HEALTH_CHECK,
        IS_MANAGE_CAPACITY, MAX_CONTENT, MAX_HEALTH_CHECK_FAIL_COUNT,
        NACOS_PLUGIN_DATASOURCE_LOG_STATE, NACOS_VERSION, NOTIFY_CONNECT_TIMEOUT,
        NOTIFY_SOCKET_TIMEOUT, SERVER_PORT_STATE, STARTUP_MODE_STATE,
    },
};

/// Application state shared across all handlers
///
/// For merged/server deployment:
/// - server_member_manager and persistence are Some
/// - console_datasource uses LocalDataSource (wraps the database)
///
/// For console-only remote deployment:
/// - server_member_manager and persistence are None
/// - console_datasource uses RemoteDataSource (HTTP calls to server)
pub struct AppState {
    pub configuration: Configuration,
    pub server_member_manager: Option<Arc<ServerMemberManager>>,
    pub config_subscriber_manager: Arc<batata_core::ConfigSubscriberManager>,
    pub console_datasource: Arc<dyn ConsoleDataSource>,
    /// OAuth2/OIDC service for external authentication
    pub oauth_service: Option<Arc<OAuthService>>,
    /// Unified persistence service (SQL, embedded RocksDB, or distributed Raft)
    pub persistence: Option<Arc<dyn PersistenceService>>,
    /// Health check manager for tracking instance heartbeats and expiration
    /// Stored as `Arc<dyn Any>` to avoid circular dependency (actual type: `batata_naming::healthcheck::HealthCheckManager`)
    pub health_check_manager: Option<Arc<dyn Any + Send + Sync>>,
    /// Raft consensus node (only in DistributedEmbedded mode)
    pub raft_node: Option<Arc<RaftNode>>,
    /// Server lifecycle status (Starting → Up / Down)
    pub server_status: Arc<ServerStatusManager>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("configuration", &self.configuration)
            .field(
                "server_member_manager",
                &self.server_member_manager.is_some(),
            )
            .field("config_subscriber_manager", &"<ConfigSubscriberManager>")
            .field("console_datasource", &"<dyn ConsoleDataSource>")
            .field("oauth_service", &self.oauth_service.is_some())
            .field("persistence", &self.persistence.is_some())
            .field("health_check_manager", &self.health_check_manager.is_some())
            .field("raft_node", &self.raft_node.is_some())
            .field("server_status", &self.server_status)
            .finish()
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            configuration: self.configuration.clone(),
            server_member_manager: self.server_member_manager.clone(),
            config_subscriber_manager: self.config_subscriber_manager.clone(),
            console_datasource: self.console_datasource.clone(),
            oauth_service: self.oauth_service.clone(),
            persistence: self.persistence.clone(),
            health_check_manager: self.health_check_manager.clone(),
            raft_node: self.raft_node.clone(),
            server_status: self.server_status.clone(),
        }
    }
}

impl AppState {
    // ========================================================================
    // Persistence Service Access
    // ========================================================================

    /// Get the persistence service (panics if not available)
    pub fn persistence(&self) -> &dyn PersistenceService {
        self.persistence
            .as_ref()
            .expect("Persistence service not available")
            .as_ref()
    }

    /// Try to get the persistence service
    pub fn try_persistence(&self) -> Option<&dyn PersistenceService> {
        self.persistence.as_deref()
    }

    // ========================================================================
    // Cluster Management
    // ========================================================================

    /// Try to get server member manager, returns None if not available
    pub fn try_member_manager(&self) -> Option<&Arc<ServerMemberManager>> {
        self.server_member_manager.as_ref()
    }

    /// Get server member manager (panics if not available)
    pub fn member_manager(&self) -> &Arc<ServerMemberManager> {
        self.server_member_manager
            .as_ref()
            .expect("Server member manager not available in remote console mode")
    }

    // ========================================================================
    // State Snapshots for API Responses
    // ========================================================================

    /// Get configuration state as a HashMap for API responses
    pub fn config_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(15);

        state.insert(
            DATASOURCE_PLATFORM_PROPERTY_STATE.to_string(),
            Some(self.configuration.datasource_platform()),
        );
        state.insert(
            NACOS_PLUGIN_DATASOURCE_LOG_STATE.to_string(),
            Some(format!("{}", self.configuration.plugin_datasource_log())),
        );
        state.insert(
            NOTIFY_CONNECT_TIMEOUT.to_string(),
            Some(format!("{}", self.configuration.notify_connect_timeout())),
        );
        state.insert(
            NOTIFY_SOCKET_TIMEOUT.to_string(),
            Some(format!("{}", self.configuration.notify_socket_timeout())),
        );
        state.insert(
            IS_HEALTH_CHECK.to_string(),
            Some(format!("{}", self.configuration.is_health_check())),
        );
        state.insert(
            MAX_HEALTH_CHECK_FAIL_COUNT.to_string(),
            Some(format!(
                "{}",
                self.configuration.max_health_check_fail_count()
            )),
        );
        state.insert(
            MAX_CONTENT.to_string(),
            Some(format!("{}", self.configuration.max_content())),
        );
        state.insert(
            IS_MANAGE_CAPACITY.to_string(),
            Some(format!("{}", self.configuration.is_manage_capacity())),
        );
        state.insert(
            IS_CAPACITY_LIMIT_CHECK.to_string(),
            Some(format!("{}", self.configuration.is_capacity_limit_check())),
        );
        state.insert(
            DEFAULT_CLUSTER_QUOTA.to_string(),
            Some(format!("{}", self.configuration.default_cluster_quota())),
        );
        state.insert(
            DEFAULT_GROUP_QUOTA.to_string(),
            Some(format!("{}", self.configuration.default_group_quota())),
        );
        state.insert(
            DEFAULT_MAX_SIZE.to_string(),
            Some(format!("{}", self.configuration.default_max_size())),
        );
        state.insert(
            DEFAULT_MAX_AGGR_COUNT.to_string(),
            Some(format!("{}", self.configuration.default_max_aggr_count())),
        );
        state.insert(
            DEFAULT_MAX_AGGR_SIZE.to_string(),
            Some(format!("{}", self.configuration.default_max_aggr_size())),
        );
        state.insert(
            CONFIG_RENTENTION_DAYS_PROPERTY_STATE.to_string(),
            Some(format!("{}", self.configuration.config_rentention_days())),
        );

        state
    }

    /// Get authentication state as a HashMap for API responses
    pub fn auth_state(&self, is_admin_request: bool) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(3);

        state.insert(
            AUTH_ENABLED.to_string(),
            Some(format!("{}", self.configuration.auth_enabled())),
        );
        state.insert(
            AUTH_SYSTEM_TYPE.to_string(),
            Some(self.configuration.auth_system_type()),
        );
        state.insert(
            AUTH_ADMIN_REQUEST.to_string(),
            Some(format!("{}", is_admin_request)),
        );

        state
    }

    /// Get environment state as a HashMap for API responses
    pub fn env_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(5);

        state.insert(
            STARTUP_MODE_STATE.to_string(),
            Some(self.configuration.startup_mode()),
        );
        state.insert(
            FUNCTION_MODE_STATE.to_string(),
            self.configuration.function_mode(),
        );
        state.insert(
            NACOS_VERSION.to_string(),
            Some(self.configuration.version()),
        );
        state.insert(
            SERVER_PORT_STATE.to_string(),
            Some(format!("{}", self.configuration.server_main_port())),
        );
        state.insert(
            "server_status".to_string(),
            Some(self.server_status.status().to_string()),
        );

        state
    }

    /// Get console state as a HashMap for API responses
    pub fn console_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(2);

        state.insert(
            "console_ui_enabled".to_string(),
            Some(format!("{}", self.configuration.console_ui_enabled())),
        );
        state.insert(
            "login_page_enabled".to_string(),
            Some(format!("{}", self.configuration.auth_console_enabled())),
        );

        state
    }

    /// Get plugin state as a HashMap for API responses
    pub fn plugin_state(&self) -> HashMap<String, Option<String>> {
        let mut state = HashMap::with_capacity(4);

        state.insert(
            CONSUL_ENABLED_STATE.to_string(),
            Some(format!("{}", self.configuration.consul_enabled())),
        );
        state.insert(
            CONSUL_PORT_STATE.to_string(),
            Some(format!("{}", self.configuration.consul_server_port())),
        );
        state
    }
}
