//! Data models module
//!
//! This module re-exports shared types from batata-server-common.

// Backward compatibility re-exports
/// `common` module.
pub mod common;

// Re-export all sub-modules from server-common
/// Re-exported item.
pub use batata_server_common::model::app_state;
/// Re-exported item.
pub use batata_server_common::model::config;
/// Re-exported item.
pub use batata_server_common::model::constants;
/// Re-exported item.
pub use batata_server_common::model::response;
/// Re-exported item.
pub use batata_server_common::model::tls;

// Re-export commonly used types at the module level
/// Re-exported item.
pub use batata_server_common::model::constants::*;
/// Re-exported item.
pub use batata_server_common::model::{
    AppState, Configuration, ConsoleException, ErrorResult, GrpcTlsConfig, Result,
};
