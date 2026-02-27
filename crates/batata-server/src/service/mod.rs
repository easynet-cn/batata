// Service layer implementations
// This module contains all business service implementations for handling application logic

// Re-export from batata-config crate
pub use batata_config::service::config;
pub use batata_config::service::export as config_export;
pub use batata_config::service::history;
pub use batata_config::service::import as config_import;
pub use batata_config::service::namespace;

// Local naming service - uses local api::naming::model types for gRPC compatibility
pub mod naming;

// Config fuzzy watch manager
pub mod config_fuzzy_watch;

// Naming fuzzy watch manager
pub mod naming_fuzzy_watch;

// AI persistent operation services
pub mod ai;

// Local implementations (gRPC handlers and RPC)
pub mod ai_handler; // AI module gRPC handlers (MCP + A2A)
pub mod cluster_handler; // Cluster module gRPC handlers
pub mod config_handler; // Config module gRPC handlers
pub mod distro_handler; // Distro protocol gRPC handlers
pub mod encryption_manager; // Encryption manager with hot reload
pub mod handler; // Request handlers for gRPC communication
pub mod handler_macros; // gRPC handler macros and utilities
pub mod lock; // In-memory distributed lock service
pub mod lock_handler; // Lock module gRPC handlers
pub mod naming_handler; // Naming module gRPC handlers
pub mod rpc; // Remote procedure call services
