// Authentication and authorization module
// This module handles user authentication, role-based access control, and permission management

// API version 1 (legacy) endpoints
pub mod v1 {
    mod auth;
    pub mod route;
}

// API version 3 (current) endpoints
pub mod v3 {
    mod auth;
    pub mod oauth;
    mod permission;
    mod role;
    pub mod route;
    mod user;
}

// Authentication data models and structures
pub mod model;

// Re-export authentication service implementations from batata-auth crate
pub use batata_auth::service;
