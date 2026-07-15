//! Legacy trait implementations for backward compatibility
//!
//! This module provides a simplified `ApolloPersistence` trait for backward compatibility
//! with existing code. New code should use `ApolloPersistenceService` from traits module.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;

/// Legacy Apollo persistence trait (simplified interface)
///
/// This trait is kept for backward compatibility. New implementations should use
/// `ApolloPersistenceService` which includes all the specialized persistence traits.
#[async_trait]
pub trait ApolloPersistence: Send + Sync {
    /// Health check for the storage backend
    async fn health_check(&self) -> anyhow::Result<()>;

    /// Get the underlying database connection if available (SQL mode only)
    fn get_db_connection(&self) -> Option<DatabaseConnection>;
}

/// External database persistence (MySQL/PostgreSQL via SeaORM)
///
/// This struct provides a simple wrapper around a database connection
/// for legacy code that only needs health check and connection access.
pub struct ExternalDbApolloPersistence {
    db: std::sync::Arc<DatabaseConnection>,
}

impl ExternalDbApolloPersistence {
    pub fn new(db: std::sync::Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &std::sync::Arc<DatabaseConnection> {
        &self.db
    }
}

#[async_trait]
impl ApolloPersistence for ExternalDbApolloPersistence {
    async fn health_check(&self) -> anyhow::Result<()> {
        use crate::entity::prelude::ApolloApp;
        use sea_orm::{EntityTrait, prelude::Expr, QuerySelect};

        ApolloApp::find()
            .select_only()
            .column_as(Expr::cust("1"), "health")
            .into_tuple::<i32>()
            .one(self.db.as_ref())
            .await?;
        Ok(())
    }

    fn get_db_connection(&self) -> Option<DatabaseConnection> {
        Some(self.db.as_ref().clone())
    }
}

// Note: EmbeddedApolloPersistence is now defined in embedded/mod.rs
// and implements ApolloPersistenceService (the new comprehensive trait)