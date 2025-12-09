// Console web interface module
// This module provides the web console API endpoints for management and monitoring

pub mod v3 {
    pub mod cluster;      // Cluster management console endpoints
    pub mod config;       // Configuration management console endpoints
    pub mod health;       // Health check console endpoints
    pub mod history;      // Configuration history console endpoints
    pub mod namespace;    // Namespace management console endpoints
    pub mod route;        // Console routing and API endpoints
    pub mod server_state; // Server status and monitoring endpoints
}
