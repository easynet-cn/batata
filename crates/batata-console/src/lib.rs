//! Batata Console - Management console backend
//!
//! This crate provides:
//! - Console API endpoints
//! - Data source abstraction (local/remote)
//! - Cluster management UI API
//! - Metrics API

pub mod datasource;
pub mod model;
pub mod v3;

// Re-export commonly used types
pub use datasource::{
    ConsoleDataSource, ConsoleDataSourceConfig, create_datasource, local::LocalDataSource,
    remote::RemoteDataSource,
};
pub use model::*;

// Re-export v3 routes and metrics
pub use v3::metrics::{METRICS, Metrics};
pub use v3::route::routes as v3_routes;
