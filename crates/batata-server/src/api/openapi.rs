//! OpenAPI documentation for Batata API
//!
//! This module provides auto-generated Swagger/OpenAPI documentation for the REST APIs.

use utoipa::OpenApi;

/// Main OpenAPI documentation structure
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Batata API",
        version = "3.1.0",
        description = "Nacos-compatible service discovery and configuration management API",
        license(name = "Apache-2.0", url = "https://www.apache.org/licenses/LICENSE-2.0"),
        contact(
            name = "Batata Team",
            url = "https://github.com/easynet-cn/batata"
        )
    ),
    servers(
        (url = "http://localhost:8848", description = "Main API Server"),
        (url = "http://localhost:8081", description = "Console Server")
    ),
    tags(
        (name = "config", description = "Configuration management APIs"),
        (name = "naming", description = "Service discovery APIs"),
        (name = "auth", description = "Authentication APIs"),
        (name = "namespace", description = "Namespace management APIs"),
        (name = "cluster", description = "Cluster management APIs"),
        (name = "health", description = "Health check APIs"),
        (name = "console", description = "Console management APIs")
    ),
    paths(
        // Config APIs (V2)
        crate::api::openapi::config::get_config,
        crate::api::openapi::config::publish_config,
        crate::api::openapi::config::delete_config,
        crate::api::openapi::config::get_config_list,
        // Naming APIs (V2)
        crate::api::openapi::naming::register_instance,
        crate::api::openapi::naming::deregister_instance,
        crate::api::openapi::naming::get_instance_list,
        crate::api::openapi::naming::get_service_list,
        // Auth APIs (V3)
        crate::api::openapi::auth::login,
        // Health APIs (V3 Console)
        crate::api::openapi::health::liveness,
        crate::api::openapi::health::readiness,
    ),
    components(
        schemas(
            ConfigInfo,
            ConfigForm,
            InstanceInfo,
            ServiceInfo,
            LoginRequest,
            LoginResponse,
            HealthStatus,
        )
    )
)]
pub struct ApiDoc;

// Schema definitions for OpenAPI documentation
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Configuration information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "dataId": "app.properties",
    "group": "DEFAULT_GROUP",
    "tenant": "public",
    "content": "server.port=8080",
    "type": "properties"
}))]
pub struct ConfigInfo {
    /// Configuration data ID
    #[serde(rename = "dataId")]
    pub data_id: String,
    /// Configuration group
    pub group: String,
    /// Namespace/tenant
    pub tenant: String,
    /// Configuration content
    pub content: String,
    /// Configuration type (properties, yaml, json, xml, text)
    #[serde(rename = "type")]
    pub config_type: String,
}

/// Configuration form for publishing
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigForm {
    /// Configuration data ID
    #[serde(rename = "dataId")]
    pub data_id: String,
    /// Configuration group
    pub group: String,
    /// Namespace/tenant
    pub tenant: Option<String>,
    /// Configuration content
    pub content: String,
    /// Configuration type
    #[serde(rename = "type")]
    pub config_type: Option<String>,
}

/// Service instance information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "instanceId": "192.168.1.1#8080#DEFAULT#DEFAULT_GROUP@@test-service",
    "ip": "192.168.1.1",
    "port": 8080,
    "weight": 1.0,
    "healthy": true,
    "enabled": true,
    "ephemeral": true,
    "clusterName": "DEFAULT"
}))]
pub struct InstanceInfo {
    /// Instance ID
    #[serde(rename = "instanceId")]
    pub instance_id: String,
    /// Instance IP address
    pub ip: String,
    /// Instance port
    pub port: i32,
    /// Instance weight for load balancing
    pub weight: f64,
    /// Whether the instance is healthy
    pub healthy: bool,
    /// Whether the instance is enabled
    pub enabled: bool,
    /// Whether the instance is ephemeral
    pub ephemeral: bool,
    /// Cluster name
    #[serde(rename = "clusterName")]
    pub cluster_name: String,
    /// Instance metadata
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

/// Service information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServiceInfo {
    /// Service name
    pub name: String,
    /// Group name
    #[serde(rename = "groupName")]
    pub group_name: String,
    /// Cluster list
    pub clusters: String,
    /// Service hosts/instances
    pub hosts: Vec<InstanceInfo>,
}

/// Login request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// Username
    pub username: String,
    /// Password
    pub password: String,
}

/// Login response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct LoginResponse {
    /// Access token
    #[serde(rename = "accessToken")]
    pub access_token: String,
    /// Token TTL in seconds
    #[serde(rename = "tokenTtl")]
    pub token_ttl: i64,
    /// Whether the user is a global admin
    #[serde(rename = "globalAdmin")]
    pub global_admin: bool,
    /// Username
    pub username: String,
}

/// Generic API result wrapper (used in JSON responses)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResult<T> {
    /// Result code (0 for success)
    pub code: i32,
    /// Result message
    pub message: String,
    /// Result data
    pub data: T,
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthStatus {
    /// Health status
    pub status: String,
}

// Path operation definitions
pub mod config {
    use super::*;

    /// Get configuration
    #[utoipa::path(
        get,
        path = "/nacos/v2/cs/config",
        tag = "config",
        params(
            ("dataId" = String, Query, description = "Configuration data ID"),
            ("group" = String, Query, description = "Configuration group"),
            ("namespaceId" = Option<String>, Query, description = "Namespace ID")
        ),
        responses(
            (status = 200, description = "Configuration content", body = String),
            (status = 404, description = "Configuration not found")
        )
    )]
    pub async fn get_config() {}

    /// Publish configuration
    #[utoipa::path(
        post,
        path = "/nacos/v2/cs/config",
        tag = "config",
        request_body = ConfigForm,
        responses(
            (status = 200, description = "Configuration published successfully", body = bool),
            (status = 400, description = "Invalid request"),
            (status = 403, description = "Permission denied")
        )
    )]
    pub async fn publish_config() {}

    /// Delete configuration
    #[utoipa::path(
        delete,
        path = "/nacos/v2/cs/config",
        tag = "config",
        params(
            ("dataId" = String, Query, description = "Configuration data ID"),
            ("group" = String, Query, description = "Configuration group"),
            ("namespaceId" = Option<String>, Query, description = "Namespace ID")
        ),
        responses(
            (status = 200, description = "Configuration deleted successfully", body = bool),
            (status = 404, description = "Configuration not found"),
            (status = 403, description = "Permission denied")
        )
    )]
    pub async fn delete_config() {}

    /// Search configuration by detail
    #[utoipa::path(
        get,
        path = "/nacos/v2/cs/config/searchDetail",
        tag = "config",
        params(
            ("dataId" = Option<String>, Query, description = "Configuration data ID pattern"),
            ("group" = Option<String>, Query, description = "Configuration group pattern"),
            ("namespaceId" = Option<String>, Query, description = "Namespace ID"),
            ("pageNo" = Option<i32>, Query, description = "Page number"),
            ("pageSize" = Option<i32>, Query, description = "Page size")
        ),
        responses(
            (status = 200, description = "Configuration list", body = Vec<ConfigInfo>)
        )
    )]
    pub async fn get_config_list() {}
}

pub mod naming {
    use super::*;

    /// Register service instance
    #[utoipa::path(
        post,
        path = "/nacos/v2/ns/instance",
        tag = "naming",
        params(
            ("serviceName" = String, Query, description = "Service name"),
            ("ip" = String, Query, description = "Instance IP"),
            ("port" = i32, Query, description = "Instance port"),
            ("groupName" = Option<String>, Query, description = "Group name"),
            ("namespaceId" = Option<String>, Query, description = "Namespace ID"),
            ("weight" = Option<f64>, Query, description = "Instance weight"),
            ("enabled" = Option<bool>, Query, description = "Whether instance is enabled"),
            ("healthy" = Option<bool>, Query, description = "Whether instance is healthy"),
            ("ephemeral" = Option<bool>, Query, description = "Whether instance is ephemeral"),
            ("clusterName" = Option<String>, Query, description = "Cluster name")
        ),
        responses(
            (status = 200, description = "Instance registered successfully", body = String),
            (status = 400, description = "Invalid request")
        )
    )]
    pub async fn register_instance() {}

    /// Deregister service instance
    #[utoipa::path(
        delete,
        path = "/nacos/v2/ns/instance",
        tag = "naming",
        params(
            ("serviceName" = String, Query, description = "Service name"),
            ("ip" = String, Query, description = "Instance IP"),
            ("port" = i32, Query, description = "Instance port"),
            ("groupName" = Option<String>, Query, description = "Group name"),
            ("namespaceId" = Option<String>, Query, description = "Namespace ID"),
            ("clusterName" = Option<String>, Query, description = "Cluster name")
        ),
        responses(
            (status = 200, description = "Instance deregistered successfully", body = String),
            (status = 404, description = "Instance not found")
        )
    )]
    pub async fn deregister_instance() {}

    /// Get instance list
    #[utoipa::path(
        get,
        path = "/nacos/v2/ns/instance/list",
        tag = "naming",
        params(
            ("serviceName" = String, Query, description = "Service name"),
            ("groupName" = Option<String>, Query, description = "Group name"),
            ("namespaceId" = Option<String>, Query, description = "Namespace ID"),
            ("clusters" = Option<String>, Query, description = "Cluster names"),
            ("healthyOnly" = Option<bool>, Query, description = "Only return healthy instances")
        ),
        responses(
            (status = 200, description = "Instance list", body = ServiceInfo),
            (status = 404, description = "Service not found")
        )
    )]
    pub async fn get_instance_list() {}

    /// Get service list
    #[utoipa::path(
        get,
        path = "/nacos/v2/ns/service/list",
        tag = "naming",
        params(
            ("pageNo" = Option<i32>, Query, description = "Page number"),
            ("pageSize" = Option<i32>, Query, description = "Page size"),
            ("groupName" = Option<String>, Query, description = "Group name"),
            ("namespaceId" = Option<String>, Query, description = "Namespace ID")
        ),
        responses(
            (status = 200, description = "Service list")
        )
    )]
    pub async fn get_service_list() {}
}

pub mod auth {
    use super::*;

    /// User login
    #[utoipa::path(
        post,
        path = "/v3/auth/user/login",
        tag = "auth",
        params(
            ("username" = String, Query, description = "Username"),
            ("password" = String, Query, description = "Password")
        ),
        responses(
            (status = 200, description = "Login successful", body = LoginResponse),
            (status = 401, description = "Invalid credentials")
        )
    )]
    pub async fn login() {}
}

pub mod health {
    use super::*;

    /// Liveness check
    #[utoipa::path(
        get,
        path = "/v3/console/health/liveness",
        tag = "health",
        responses(
            (status = 200, description = "Service is alive", body = HealthStatus)
        )
    )]
    pub async fn liveness() {}

    /// Readiness check
    #[utoipa::path(
        get,
        path = "/v3/console/health/readiness",
        tag = "health",
        responses(
            (status = 200, description = "Service is ready", body = HealthStatus),
            (status = 503, description = "Service is not ready")
        )
    )]
    pub async fn readiness() {}
}

/// Configure Swagger UI for the actix-web app
#[cfg(feature = "swagger")]
pub fn configure_swagger(cfg: &mut actix_web::web::ServiceConfig) {
    use utoipa_swagger_ui::SwaggerUi;

    cfg.service(
        SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", ApiDoc::openapi()),
    );
}

/// Configure Swagger UI for the actix-web app (no-op when swagger feature is disabled)
#[cfg(not(feature = "swagger"))]
pub fn configure_swagger(_cfg: &mut actix_web::web::ServiceConfig) {
    // No-op - Swagger UI is disabled
}
