//! Data models module
//!
//! This module re-exports shared types from batata-server-common.

// Backward compatibility re-exports
pub mod common;

// Re-export all sub-modules from server-common
pub use batata_server_common::model::app_state;
pub use batata_server_common::model::config;
pub use batata_server_common::model::constants;
pub use batata_server_common::model::response;
pub use batata_server_common::model::tls;

// Re-export commonly used types at the module level
pub use batata_server_common::model::constants::*;
pub use batata_server_common::model::{
    AppState, Configuration, ConsoleException, ErrorResult, GrpcTlsConfig, Result,
};
