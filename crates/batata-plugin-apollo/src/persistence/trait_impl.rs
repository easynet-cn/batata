use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait, QuerySelect};

#[async_trait]
pub trait ApolloPersistence: Send + Sync {
    async fn health_check(&self) -> anyhow::Result<()>;
}

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
        use crate::model::ApolloApp;
        use sea_orm::prelude::Expr;

        ApolloApp::find()
            .select_only()
            .column_as(Expr::cust("1"), "health")
            .into_tuple::<i32>()
            .one(self.db.as_ref())
            .await?;
        Ok(())
    }
}

pub struct EmbeddedApolloPersistence {
    db: std::sync::Arc<rocksdb::DB>,
}

impl EmbeddedApolloPersistence {
    pub fn new(db: std::sync::Arc<rocksdb::DB>) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &std::sync::Arc<rocksdb::DB> {
        &self.db
    }
}

#[async_trait]
impl ApolloPersistence for EmbeddedApolloPersistence {
    async fn health_check(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
