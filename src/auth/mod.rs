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
    mod permission;
    mod role;
    pub mod route;
    mod user;
}

// Authentication data models and structures
pub mod model;

// Authentication service implementations
pub mod service {
    pub mod auth;
    pub mod permission;
    pub mod role;
    pub mod user;
}
