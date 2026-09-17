use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Represents the `ApolloPluginConfig` entity.
pub struct ApolloPluginConfig {
    #[serde(default = "default_enabled")]
    /// The `enabled` field.
    pub enabled: bool,
    #[serde(default = "default_port")]
    /// The `port` field.
    pub port: u16,
    #[serde(default = "default_http_workers")]
    /// The `http_workers` field.
    pub http_workers: usize,
    #[serde(default = "default_apollo_version")]
    /// The supported Apollo version this plugin is compatible with
    /// (mirrors upstream `apollo-portal`'s reported `apolloVersion`).
    pub version: String,
}

fn default_enabled() -> bool {
    false
}

fn default_port() -> u16 {
    8080
}

fn default_http_workers() -> usize {
    4
}

fn default_apollo_version() -> String {
    // Current latest Apollo release: https://github.com/apolloconfig/apollo/releases
    "2.5.1".to_string()
}

impl Default for ApolloPluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            port: default_port(),
            http_workers: default_http_workers(),
            version: default_apollo_version(),
        }
    }
}

impl ApolloPluginConfig {
    /// Builds an `ApolloPluginConfig` from a `config::Config` source.
    pub fn from_config(config: &config::Config) -> Self {
        let enabled = config
            .get_bool("batata.plugin.apollo.enabled")
            .unwrap_or(false);
        let port = config.get_int("batata.plugin.apollo.port").unwrap_or(8080) as u16;
        let http_workers = {
            let v = config
                .get_int("batata.plugin.apollo.http.workers")
                .unwrap_or(0) as usize;
            if v == 0 {
                let cpus = std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4);
                std::cmp::min(4, cpus / 2).max(2)
            } else {
                v
            }
        };

        let version = config
            .get_string("batata.plugin.apollo.version")
            .unwrap_or_else(|_| default_apollo_version());

        Self {
            enabled,
            port,
            http_workers,
            version,
        }
    }
}

/// Security configuration for the Apollo-compatible plugin.
///
/// Upstream reference:
/// - `apollo.adminservice.access.token(s)` — the static token(s) accepted by
///   `AdminServiceAuthenticationFilter`.
/// - `apollo.portal.signature` — the secret salt used by
///   `ConsumerAuthUtil` to sign and verify openapi consumer tokens.
///
/// Every field is optional: when the admin access tokens are empty the
/// admin-service filter is disabled (mirroring upstream behaviour where the
/// filter only activates once `apollo.adminservice.access.enabled=true` and a
/// token is configured). The portal signature salt defaults to a fixed value so
/// consumer tokens remain verifiable out of the box; operators should override
/// it via the environment in production.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Comma-separated list of accepted admin-service access tokens.
    /// Empty means the admin-service filter is disabled.
    pub admin_access_tokens: Vec<String>,
    /// Secret salt used to sign/verify openapi consumer tokens
    /// (`apollo.portal.signature`).
    pub portal_signature: String,
    /// Whether admin-service access control is enabled.
    pub admin_access_control_enabled: bool,
    /// Whether openapi (portal) access control is enabled.
    ///
    /// Mirrors the admin side: upstream enables the consumer/user-token
    /// filters only once consumers exist and the filter is switched on.
    /// Batata defaults this to `false` so an unconfigured install (and the
    /// in-process test suite) is not locked out of every `/openapi/v1`
    /// endpoint; set `APOLLO_OPENAPI_AUTH_ENABLED=true` to enforce
    /// consumer tokens (and user tokens on portal-management paths).
    pub openapi_auth_enabled: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            admin_access_tokens: Vec::new(),
            // Mirrors upstream's default `apollo.portal.signature` placeholder.
            portal_signature: "apollo-portal-signature".to_string(),
            admin_access_control_enabled: false,
            openapi_auth_enabled: false,
        }
    }
}

impl AuthConfig {
    /// Build the `AuthConfig` from the process environment.
    ///
    /// Recognized variables:
    /// - `APOLLO_ADMIN_SERVICE_ACCESS_TOKENS` — comma-separated token list.
    ///   When non-empty, admin-service access control is enabled.
    /// - `APOLLO_PORTAL_SIGNATURE` — the consumer-token signing salt.
    /// - `APOLLO_OPENAPI_AUTH_ENABLED` — `true` to enforce consumer / user
    ///   tokens on `/openapi/v1` (default `false`, i.e. openapi stays open).
    pub fn from_env() -> Self {
        let mut cfg = AuthConfig::default();
        if let Ok(raw) = std::env::var("APOLLO_ADMIN_SERVICE_ACCESS_TOKENS") {
            let tokens: Vec<String> = raw
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !tokens.is_empty() {
                cfg.admin_access_tokens = tokens;
                cfg.admin_access_control_enabled = true;
            }
        }
        if let Ok(sig) = std::env::var("APOLLO_PORTAL_SIGNATURE")
            && !sig.is_empty() {
                cfg.portal_signature = sig;
            }
        if let Ok(flag) = std::env::var("APOLLO_OPENAPI_AUTH_ENABLED") {
            cfg.openapi_auth_enabled = matches!(
                flag.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
        }
        cfg
    }

    /// Whether a presented admin-service token is accepted.
    pub fn is_valid_admin_token(&self, token: &str) -> bool {
        if !self.admin_access_control_enabled || self.admin_access_tokens.is_empty() {
            return true;
        }
        let token_bytes = token.as_bytes();
        self.admin_access_tokens
            .iter()
            .any(|t| bool::from(t.as_bytes().ct_eq(token_bytes)))
    }
}
