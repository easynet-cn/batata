//! Spring Security authority service for portal users.

use std::sync::Arc;

use crate::persistence::traits::{ApolloPersistenceService, AuthorityPersistence};

/// Reads and mutates the authorities attached to a portal account.
pub struct AuthorityService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl AuthorityService {
    /// Creates the service.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Lists authority names for an account.
    pub async fn list(&self, username: &str) -> anyhow::Result<Vec<String>> {
        AuthorityPersistence::list_authorities(&self.persistence, username).await
    }

    /// Whether an account holds an authority.
    pub async fn has(&self, username: &str, authority: &str) -> anyhow::Result<bool> {
        AuthorityPersistence::has_authority(&self.persistence, username, authority).await
    }

    /// Grants an authority to an account.
    pub async fn grant(&self, username: &str, authority: &str) -> anyhow::Result<()> {
        AuthorityPersistence::assign_authority(&self.persistence, username, authority).await?;
        Ok(())
    }

    /// Revokes an authority from an account.
    pub async fn revoke(&self, username: &str, authority: &str) -> anyhow::Result<()> {
        AuthorityPersistence::remove_authority(&self.persistence, username, authority).await
    }
}
