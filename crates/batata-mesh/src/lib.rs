//! Batata Service Mesh Support
//!
//! This crate provides service mesh integration for Batata, including:
//! - xDS protocol support (EDS, CDS, LDS, RDS, ADS)
//! - Istio MCP integration (ServiceEntry, VirtualService, DestinationRule)
//! - Service discovery to xDS resource conversion
//!
//! # Architecture
//!
//! The mesh module is organized as follows:
//! - `xds` - xDS protocol types and generated code
//! - `conversion` - Batata/Nacos to xDS resource conversion
//! - `server` - xDS server implementations
//! - `snapshot` - Resource snapshot management
//! - `grpc` - gRPC service implementations for ADS
//! - `sync` - Batata/Nacos to xDS synchronization bridge
//! - `mcp` - Istio MCP (Mesh Configuration Protocol) support

pub mod conversion;
pub mod grpc;
pub mod mcp;
pub mod server;
pub mod snapshot;
pub mod sync;
pub mod xds;

// Re-export commonly used types
pub use grpc::{
    AggregatedDiscoveryServiceImpl, XdsGrpcServer, create_ads_service, start_xds_grpc_server,
};
pub use mcp::{McpServer, McpServerConfig};
pub use server::XdsServer;
pub use snapshot::{DeltaClientState, ResourceSnapshot};
pub use sync::{Debouncer, NacosSyncBridge, SyncBridgeConfig};
