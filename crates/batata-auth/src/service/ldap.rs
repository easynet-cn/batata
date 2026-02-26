//! LDAP Authentication Service
//!
//! Provides LDAP-based authentication for users.
//! Supports:
//! - Simple bind authentication
//! - User search and verification
//! - Connection pooling with timeout
//! - Case-sensitive/insensitive username matching

use std::time::Duration;

use ldap3::{Ldap, LdapConnAsync, LdapConnSettings, Scope, SearchEntry};

use crate::model::{AuthResult, LdapConfig};

/// LDAP Authentication Service
///
/// Manages LDAP connections and provides authentication operations.
pub struct LdapAuthService {
    config: LdapConfig,
}

impl LdapAuthService {
    /// Create a new LDAP authentication service
    pub fn new(config: LdapConfig) -> Self {
        Self { config }
    }

    /// Check if LDAP authentication is properly configured
    pub fn is_configured(&self) -> bool {
        self.config.is_configured()
    }

    /// Get the LDAP configuration
    pub fn config(&self) -> &LdapConfig {
        &self.config
    }

    /// Create a new LDAP connection
    async fn create_connection(&self) -> anyhow::Result<Ldap> {
        let settings =
            LdapConnSettings::new().set_conn_timeout(Duration::from_millis(self.config.timeout_ms));

        let (conn, ldap) = LdapConnAsync::with_settings(settings, &self.config.url).await?;

        // Spawn the connection driver
        tokio::spawn(async move {
            if let Err(e) = conn.drive().await {
                tracing::warn!("LDAP connection driver error: {}", e);
            }
        });

        Ok(ldap)
    }

    /// Get or create an LDAP connection
    async fn get_connection(&self) -> anyhow::Result<Ldap> {
        // For now, create a new connection each time
        // In a production system, you might want connection pooling
        self.create_connection().await
    }

    /// Authenticate a user with LDAP
    ///
    /// This performs a two-step authentication:
    /// 1. Bind with admin credentials to search for the user
    /// 2. Bind with user credentials to verify the password
    pub async fn authenticate(&self, username: &str, password: &str) -> AuthResult {
        if !self.is_configured() {
            return AuthResult {
                success: false,
                username: username.to_string(),
                error_message: Some("LDAP is not configured".to_string()),
                is_ldap_user: false,
            };
        }

        // Normalize username if case-insensitive
        let normalized_username = if self.config.case_sensitive {
            username.to_string()
        } else {
            username.to_lowercase()
        };

        // Try to authenticate
        match self.do_authenticate(&normalized_username, password).await {
            Ok(success) => {
                if success {
                    tracing::info!(
                        username = %normalized_username,
                        "LDAP authentication successful"
                    );
                    AuthResult {
                        success: true,
                        username: normalized_username,
                        error_message: None,
                        is_ldap_user: true,
                    }
                } else {
                    tracing::warn!(
                        username = %normalized_username,
                        "LDAP authentication failed: invalid credentials"
                    );
                    AuthResult {
                        success: false,
                        username: normalized_username,
                        error_message: Some("Invalid username or password".to_string()),
                        is_ldap_user: true,
                    }
                }
            }
            Err(e) => {
                tracing::error!(
                    username = %normalized_username,
                    error = %e,
                    "LDAP authentication error"
                );
                AuthResult {
                    success: false,
                    username: normalized_username,
                    error_message: Some(format!("LDAP error: {}", e)),
                    is_ldap_user: false,
                }
            }
        }
    }

    /// Internal authentication implementation
    async fn do_authenticate(&self, username: &str, password: &str) -> anyhow::Result<bool> {
        let mut ldap = self.get_connection().await?;

        // If we have bind credentials, first bind as admin and search for user
        if !self.config.bind_dn.is_empty() {
            // Bind as admin
            let bind_result = ldap
                .simple_bind(&self.config.bind_dn, &self.config.bind_password)
                .await?;

            if bind_result.rc != 0 {
                anyhow::bail!("Failed to bind as admin: {}", bind_result.text);
            }

            // Search for the user
            let filter = self.config.build_search_filter(username);
            let search_result = ldap
                .search(&self.config.base_dn, Scope::Subtree, &filter, vec!["dn"])
                .await?;

            let (entries, _result) = search_result.success()?;

            if entries.is_empty() {
                tracing::debug!(username = %username, "User not found in LDAP");
                let _ = ldap.unbind().await;
                return Ok(false);
            }

            // Get the user's DN from search results
            let user_dn = if let Some(entry) = entries.into_iter().next() {
                SearchEntry::construct(entry).dn
            } else {
                let _ = ldap.unbind().await;
                return Ok(false);
            };

            // Unbind admin connection
            let _ = ldap.unbind().await;

            // Create new connection and bind as the user
            let mut user_ldap = self.get_connection().await?;
            let user_bind_result = user_ldap.simple_bind(&user_dn, password).await?;

            let success = user_bind_result.rc == 0;
            let _ = user_ldap.unbind().await;

            Ok(success)
        } else {
            // Direct bind with user DN pattern
            let user_dn = self.config.build_user_dn(username);
            let bind_result = ldap.simple_bind(&user_dn, password).await?;

            let success = bind_result.rc == 0;
            let _ = ldap.unbind().await;

            Ok(success)
        }
    }

    /// Search for a user in LDAP
    ///
    /// Returns the user's DN if found, None otherwise.
    pub async fn search_user(&self, username: &str) -> anyhow::Result<Option<String>> {
        if !self.is_configured() {
            anyhow::bail!("LDAP is not configured");
        }

        let mut ldap = self.get_connection().await?;

        // Bind as admin if configured
        if !self.config.bind_dn.is_empty() {
            let bind_result = ldap
                .simple_bind(&self.config.bind_dn, &self.config.bind_password)
                .await?;

            if bind_result.rc != 0 {
                anyhow::bail!("Failed to bind as admin: {}", bind_result.text);
            }
        }

        // Search for the user
        let filter = self.config.build_search_filter(username);
        let search_result = ldap
            .search(&self.config.base_dn, Scope::Subtree, &filter, vec!["dn"])
            .await?;

        let (entries, _result) = search_result.success()?;
        let _ = ldap.unbind().await;

        if let Some(entry) = entries.into_iter().next() {
            Ok(Some(SearchEntry::construct(entry).dn))
        } else {
            Ok(None)
        }
    }

    /// Check if a user exists in LDAP
    pub async fn user_exists(&self, username: &str) -> anyhow::Result<bool> {
        Ok(self.search_user(username).await?.is_some())
    }

    /// Test the LDAP connection
    pub async fn test_connection(&self) -> anyhow::Result<bool> {
        if !self.is_configured() {
            return Ok(false);
        }

        let mut ldap = self.get_connection().await?;

        // If we have bind credentials, try to bind
        if !self.config.bind_dn.is_empty() {
            let bind_result = ldap
                .simple_bind(&self.config.bind_dn, &self.config.bind_password)
                .await?;

            let success = bind_result.rc == 0;
            let _ = ldap.unbind().await;
            Ok(success)
        } else {
            // Just test anonymous bind (may not be allowed)
            let bind_result = ldap.simple_bind("", "").await?;
            let _ = ldap.unbind().await;
            Ok(bind_result.rc == 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ldap_auth_service_creation() {
        let config = LdapConfig::default();
        let service = LdapAuthService::new(config);
        assert!(!service.is_configured());
    }

    #[test]
    fn test_ldap_auth_service_configured() {
        let config = LdapConfig {
            url: "ldap://localhost:389".to_string(),
            ..Default::default()
        };
        let service = LdapAuthService::new(config);
        assert!(service.is_configured());
    }

    #[tokio::test]
    async fn test_ldap_authenticate_not_configured() {
        let config = LdapConfig::default();
        let service = LdapAuthService::new(config);

        let result = service.authenticate("user", "password").await;
        assert!(!result.success);
        assert!(result.error_message.is_some());
        assert!(result.error_message.unwrap().contains("not configured"));
    }
}
