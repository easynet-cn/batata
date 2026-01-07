//! Batata Persistence - Database entities and persistence layer
//!
//! This crate provides:
//! - SeaORM entity definitions (auto-generated)
//! - Repository traits and implementations
//! - Database migrations

pub mod entity;

// Re-export sea-orm for convenience
pub use sea_orm;

// Re-export entity prelude
pub use entity::prelude::*;
