// Authentication and authorization module
// Delegates to batata-server-common for HTTP handler implementations

// API version 3 (current) endpoints - re-exported from batata-server-common
pub mod v3 {
    pub mod oauth {
        pub use batata_server_common::api::auth::v3::oauth::*;
    }
    pub mod route {
        pub use batata_server_common::api::auth::v3::route::*;
    }
}

// Authentication data models and structures
pub mod model;

// Re-export authentication service implementations from batata-auth crate
pub use batata_auth::service;
