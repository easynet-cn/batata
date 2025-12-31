// Service layer implementations
// This module contains all business service implementations for handling application logic

pub mod config; // Configuration management services
pub mod config_handler; // Config module gRPC handlers
pub mod handler; // Request handlers for gRPC communication
pub mod history; // Configuration history and versioning services
pub mod namespace; // Namespace and tenant management services
pub mod naming; // Naming/service discovery services
pub mod naming_handler; // Naming module gRPC handlers
pub mod rpc; // Remote procedure call services
