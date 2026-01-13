//! Batata Core - Cluster and connection management
//!
//! This crate provides:
//! - Server member management
//! - Connection management
//! - Health checking
//! - Task scheduling
//! - Abstraction layer for multi-registry support

pub mod abstraction;
pub mod model;
pub mod service;

// Re-export cluster module
pub mod cluster {
    pub use crate::service::cluster::{
        ClusterHealthSummary, ServerMemberManager, ServerMemberManagerConfig,
    };
}

// Re-export commonly used types
pub use model::{Configuration, Connection, ConnectionMeta, GrpcClient, PageParam};
pub use service::{ConfigKey, ConfigSubscriber, ConfigSubscriberManager};

// Re-export common functions
pub use batata_common::local_ip;
