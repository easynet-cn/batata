//! Module `auth` of the `batata-server` crate.
// Authentication and authorization module
// Delegates to batata-server-common for HTTP handler implementations

// API version 3 (current) endpoints - re-exported from batata-server-common
/// `v3` module.
pub mod v3 {
    /// `oauth` module.
    pub mod oauth {
        /// Re-exported item.
        pub use batata_server_common::api::auth::v3::oauth::*;
    }
    /// `route` module.
    pub mod route {
        /// Re-exported item.
        pub use batata_server_common::api::auth::v3::route::*;
    }
}

// Authentication data models and structures
/// `model` module.
pub mod model;

// Re-export authentication service implementations from batata-auth crate
/// Re-exported item.
pub use batata_auth::service;
