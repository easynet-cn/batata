// Authentication and authorization HTTP handlers
// This module handles user authentication, role-based access control, and permission management

// API version 3 (current) endpoints
pub mod v3 {
    mod admin;
    mod auth;
    pub mod oauth;
    mod permission;
    mod role;
    pub mod route;
    mod user;
}

// Authentication data models and structures (re-exported from batata-auth)
pub mod model {
    pub use batata_auth::model::*;
}

// Re-export authentication service implementations from batata-auth crate
pub use batata_auth::service;
