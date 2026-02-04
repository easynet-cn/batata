//! Apollo Config compatibility plugin for Batata
//!
//! This crate provides Apollo Config client API compatibility, allowing Apollo clients
//! (Java, Go, .NET, etc.) to connect to Batata without modification.
//!
//! ## Supported APIs
//!
//! ### Config Service API (Client SDK)
//!
//! - `GET /configs/{appId}/{clusterName}/{namespace}` - Get configuration
//! - `GET /configfiles/{appId}/{clusterName}/{namespace}` - Get config as plain text
//! - `GET /configfiles/json/{appId}/{clusterName}/{namespace}` - Get config as JSON
//! - `GET /notifications/v2` - Long polling for configuration updates
//!
//! ## Concept Mapping
//!
//! Apollo and Nacos use different terminology:
//!
//! | Apollo | Nacos | Mapping Strategy |
//! |--------|-------|------------------|
//! | `env` | `namespace` | Direct mapping |
//! | `appId` | Part of `dataId` | `{appId}+{namespace}` → `dataId` |
//! | `cluster` | `group` | Direct mapping |
//! | `namespace` | Part of `dataId` | `{appId}+{namespace}` → `dataId` |
//! | `releaseKey` | `md5` | Generate Apollo-style releaseKey |
//!
//! ## Usage
//!
//! ```ignore
//! use batata_plugin_apollo::{apollo_config_routes, ApolloNotificationService};
//! use std::sync::Arc;
//!
//! // Create notification service
//! let notification_service = Arc::new(ApolloNotificationService::new(db.clone()));
//!
//! // Configure routes
//! App::new()
//!     .app_data(web::Data::new(db.clone()))
//!     .app_data(web::Data::new(notification_service))
//!     .service(apollo_config_routes())
//! ```
//!
//! ## Apollo Client Configuration
//!
//! Configure Apollo clients to connect to Batata:
//!
//! ### Java Client
//!
//! ```properties
//! apollo.meta=http://localhost:8848
//! app.id=your-app-id
//! apollo.cluster=default
//! ```
//!
//! ### Go Client
//!
//! ```go
//! c := &config.AppConfig{
//!     AppID:         "your-app-id",
//!     Cluster:       "default",
//!     NamespaceName: "application",
//!     IP:            "localhost:8848",
//! }
//! ```

pub mod api;
pub mod mapping;
pub mod model;
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
    ConfigFormat, ConfigQueryParams, CreateGrayReleaseRequest, GrayReleaseRule, GrayReleaseStatus,
    GrayRule, NamespaceLock, NotificationQueryParams, NotificationRequest, WatchedKey,
};
pub use service::{
    ApolloAdvancedService, ApolloConfigService, ApolloNotificationService, ApolloOpenApiService,
    ConfigChangeEvent,
};
