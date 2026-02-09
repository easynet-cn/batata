//! Apollo Config API route configuration
//!
//! This module provides Apollo Config client API compatibility by re-exporting
//! the batata-plugin-apollo routes and services.
//!
//! ## Supported APIs
//!
//! - `GET /configs/{appId}/{clusterName}/{namespace}` - Get configuration
//! - `GET /configfiles/{appId}/{clusterName}/{namespace}` - Get config as plain text
//! - `GET /configfiles/json/{appId}/{clusterName}/{namespace}` - Get config as JSON
//! - `GET /notifications/v2` - Long polling for configuration updates

// Re-export routes from the plugin
pub use batata_plugin_apollo::{
    apollo_advanced_routes, apollo_config_routes, apollo_config_routes_with_prefix,
    apollo_openapi_routes, configure_apollo_routes,
};

// Re-export services for server initialization
pub use batata_plugin_apollo::{
    ApolloAdvancedService, ApolloBranchService, ApolloConfigService, ApolloNotificationService,
    ApolloOpenApiService, ConfigChangeEvent,
};
