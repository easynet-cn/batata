//! Auth plugin implementations
//!
//! Provides pluggable authentication backends. The active plugin is
//! selected via config key `batata.core.auth.system.type`.

pub mod default_auth;

use std::sync::Arc;

use batata_common::AuthPlugin;

use crate::model::LdapConfig;
use crate::service::auth_data::AuthDataService;

/// Create an auth plugin based on the configured auth system type.
///
/// # Arguments
/// * `auth_type` - Plugin name: "nacos" (default), "ldap"
/// * `secret_key` - JWT signing secret (Base64-encoded)
/// * `token_expire_seconds` - JWT token TTL
/// * `auth_data` - Backend for user/role/permission queries: the local store in
///   merged/server mode, or the core over HTTP in console mode. The plugin
///   itself always runs in this process, so token validation and RBAC decisions
///   stay local.
/// * `ldap_config` - Optional LDAP configuration (required for "ldap" type)
pub fn create_auth_plugin(
    auth_type: &str,
    secret_key: String,
    token_expire_seconds: i64,
    auth_data: Arc<dyn AuthDataService>,
    ldap_config: Option<LdapConfig>,
) -> Arc<dyn AuthPlugin> {
    match auth_type {
        "ldap" => {
            let default_plugin =
                default_auth::DefaultAuthPlugin::new(secret_key, token_expire_seconds, auth_data);
            let ldap_config = ldap_config.expect("LDAP config required for ldap auth type");
            let ldap_service = crate::service::ldap::LdapAuthService::new(ldap_config);

            Arc::new(default_auth::LdapAuthPlugin::new(
                default_plugin,
                ldap_service,
            ))
        }
        _ => {
            // "default" auth type is the default
            Arc::new(default_auth::DefaultAuthPlugin::new(
                secret_key,
                token_expire_seconds,
                auth_data,
            ))
        }
    }
}
