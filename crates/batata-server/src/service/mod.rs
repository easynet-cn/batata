//! Module `service` of the `batata-server` crate.
// Service layer implementations
// This module contains all business service implementations for handling application logic

// Local naming service - uses local api::naming::model types for gRPC compatibility
/// `naming` module.
pub mod naming;

// Config fuzzy watch manager
/// `config_fuzzy_watch` module.
pub mod config_fuzzy_watch;

// Naming fuzzy watch manager
/// `naming_fuzzy_watch` module.
pub mod naming_fuzzy_watch;

// AI persistent operation services
/// `ai` module.
pub mod ai;

// Local implementations (gRPC handlers and RPC)
/// `ai_handler` module.
pub mod ai_handler; // AI module gRPC handlers (MCP + A2A)
/// `cluster_handler` module.
pub mod cluster_handler; // Cluster module gRPC handlers
/// `config_handler` module.
pub mod config_handler; // Config module gRPC handlers
/// `connection_limit` module.
pub mod connection_limit; // Connection limit checker for gRPC
#[cfg(feature = "consul")]
/// `consul_event_handler` module.
pub mod consul_event_handler; // Consul event broadcast handler
/// `distro_handler` module.
pub mod distro_handler; // Distro protocol gRPC handlers
/// `encryption_manager` module.
pub mod encryption_manager; // Encryption manager with hot reload
/// `handler` module.
pub mod handler; // Request handlers for gRPC communication
/// `handler_macros` module.
pub mod handler_macros; // gRPC handler macros and utilities
/// `lock` module.
pub mod lock; // In-memory distributed lock service
/// `lock_handler` module.
pub mod lock_handler; // Lock module gRPC handlers
/// `naming_handler` module.
pub mod naming_handler; // Naming module gRPC handlers
/// `rpc` module.
pub mod rpc; // Remote procedure call services
/// `tps_checker` module.
pub mod tps_checker; // TPS control checker for gRPC rate limiting
