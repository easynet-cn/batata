//! SQL-based persistence backend (MySQL/PostgreSQL via SeaORM)
//!
//! This module implements the `PersistenceService` trait by wrapping existing
//! SeaORM database operations. It is split into sub-modules by domain:
//! - `config`: Configuration CRUD, history, export/import
//! - `namespace`: Namespace CRUD
//! - `auth`: Users, roles, permissions
//! - `capacity`: Tenant and group capacity management

mod ai_resource;
mod auth;
mod capacity;
mod config;
mod namespace;

use async_trait::async_trait;
use sea_orm::{prelude::Expr, *};

use crate::entity::users;
use crate::model::*;
use crate::traits::*;

/// External database persistence service
///
/// Wraps a SeaORM `DatabaseConnection` and implements all persistence traits
/// by delegating to direct database queries.
pub struct ExternalDbPersistService {
    db: DatabaseConnection,
}

impl ExternalDbPersistService {
    /// Create a new ExternalDbPersistService with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Get a reference to the underlying database connection
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}

// ============================================================================
// PersistenceService implementation
// ============================================================================

#[async_trait]
impl PersistenceService for ExternalDbPersistService {
    fn storage_backend(&self) -> StorageBackend {
        StorageBackend::ExternalDb
    }

    fn deploy_topology(&self) -> DeployTopology {
        DeployTopology::Standalone
    }

    async fn health_check(&self) -> anyhow::Result<()> {
        // Execute a simple query to verify connectivity
        users::Entity::find()
            .select_only()
            .column_as(Expr::cust("1"), "health")
            .into_tuple::<i32>()
            .one(&self.db)
            .await?;
        Ok(())
    }
}
