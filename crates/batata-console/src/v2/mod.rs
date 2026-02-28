//! Console V2 API handlers
//!
//! This module provides the HTTP handlers for the V2 console API.
//! Migrated from batata-server to align with Nacos 3.x architecture
//! where console routes belong to the console module.

pub mod health;
pub mod namespace;
pub mod route;
