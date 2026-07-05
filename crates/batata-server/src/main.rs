//! Main entry point for Batata server (Nacos-compatible).
//!
//! This file delegates server startup to the `AppBuilder` in the `builder` module,
//! which orchestrates all initialization phases in a clear, step-by-step manner.
//!
//! # Architecture
//!
//! The server startup is organized into 13 phases, each handled by a dedicated
//! builder method in `AppBuilder`:
//!
//! 1.  **Configuration & logging** - Load config, init logging, metrics, auth caches
//! 2.  **Deployment mode & plugins** - Determine mode, register protocol adapters
//! 2.5 **Plugin CF collection** - Collect RocksDB column families from plugins
//! 3.  **Persistence layer** - Initialize database, Raft, cluster manager
//! 4.  **Shared services & state** - Create naming, health check, encryption, etc.
//! 5.  **Shutdown & AI services** - Setup graceful shutdown, AI services
//! 6.  **Console-remote early return** - Handle console-only deployment
//! 7.  **gRPC servers** - Start SDK, cluster, and Raft gRPC servers
//! 8.  **Background tasks** - Health checks, warmup poller, MCP index refresh
//! 9.  **Cluster & Raft init** - Join cluster, sync membership
//! 10. **xDS service mesh** - Start xDS server if enabled
//! 11. **Plugin initialization** - Initialize protocol adapter plugins
//! 12. **HTTP servers** - Start main, console, plugin, and MCP servers
//! 13. **Graceful shutdown** - Orderly shutdown of all components
//!
//! For details on each phase, see [`batata_server::builder::app_builder::AppBuilder`].

use tracing::error;

use batata_server::builder::AppBuilder;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Run the complete server lifecycle via the builder.
    // All phase logic lives in AppBuilder; main.rs stays minimal.
    if let Err(e) = AppBuilder::run().await {
        error!("Fatal server error: {}", e);
        return Err(e);
    }

    Ok(())
}
