//! Auth plugin implementations
//!
//! Provides pluggable authentication backends following the Nacos 3.x
//! `AuthPluginService` SPI pattern. The active plugin is selected via
//! config key `batata.core.auth.system.type`.

pub mod nacos;

use std::sync::Arc;

use batata_common::AuthPlugin;
use batata_persistence::PersistenceService;

use crate::model::LdapConfig;

/// Create an auth plugin based on the configured auth system type.
///
/// # Arguments
/// * `auth_type` - Plugin name: "nacos" (default), "ldap"
/// * `secret_key` - JWT signing secret (Base64-encoded)
/// * `token_expire_seconds` - JWT token TTL
/// * `persistence` - Persistence service for user/role/permission queries
/// * `ldap_config` - Optional LDAP configuration (required for "ldap" type)
pub fn create_auth_plugin(
    auth_type: &str,
    secret_key: String,
    token_expire_seconds: i64,
    persistence: Arc<dyn PersistenceService>,
    ldap_config: Option<LdapConfig>,
) -> Arc<dyn AuthPlugin> {
    match auth_type {
        "ldap" => {
            let nacos_plugin =
                nacos::NacosAuthPlugin::new(secret_key, token_expire_seconds, persistence);
            let ldap_config = ldap_config.expect("LDAP config required for ldap auth type");
            let ldap_service = crate::service::ldap::LdapAuthService::new(ldap_config);
            Arc::new(nacos::LdapAuthPlugin::new(nacos_plugin, ldap_service))
        }
        _ => {
            // "nacos" is the default
            Arc::new(nacos::NacosAuthPlugin::new(
                secret_key,
                token_expire_seconds,
                persistence,
            ))
        }
    }
}
