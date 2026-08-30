// Authentication and authorization HTTP handlers
// This module handles user authentication, role-based access control, and permission management

// API version 3 (current) endpoints
/// Provides the `v3` module.
pub mod v3 {
    mod admin;
    mod auth;
    pub mod oauth;
    pub mod oidc;
    mod permission;
    mod role;
/// Provides the `route` module.
    pub mod route;
    mod user;
    mod visibility;
}

// Authentication data models and structures (re-exported from batata-auth)
/// Provides the `model` module.
pub mod model {
    pub use batata_auth::model::*;
}

// Re-export authentication service implementations from batata-auth crate
pub use batata_auth::service;
