//! Apollo Config compatibility plugin for Batata
//!
//! This crate provides Apollo Config client API compatibility, allowing Apollo clients
//! (Java, Go, .NET, etc.) to connect to Batata without modification.

pub mod api;
pub mod mapping;
pub mod model;
pub mod repository;
pub mod service;

// Re-export commonly used types
pub use api::{
    apollo_advanced_routes, apollo_config_routes, apollo_config_routes_with_prefix,
    apollo_openapi_routes, configure_apollo_routes,
};
pub use mapping::{
    ApolloMappingContext, from_nacos_data_id, generate_release_key, parse_properties,
    to_nacos_data_id, to_nacos_group, to_nacos_namespace, to_properties_string,
};
pub use model::{
    AccessKey, AcquireLockRequest, ApolloApp, ApolloConfig, ApolloConfigNotification, ApolloItem,
    ApolloNamespace, ApolloNotificationMessages, ApolloRelease, ClientConnection, ClientMetrics,
    ConfigFormat, ConfigQueryParams, CreateAppRequest, CreateGrayReleaseRequest,
    CreateNamespaceRequest, GrayReleaseRule, GrayReleaseStatus, GrayRule, NamespaceLock,
    NotificationQueryParams, NotificationRequest, WatchedKey,
};
pub use service::{
    ApolloAdvancedService, ApolloBranchService, ApolloConfigService, ApolloNotificationService,
    ApolloOpenApiService, ConfigChangeEvent,
};
