// Authentication and authorization models
// Re-exports from batata_auth with local extensions

// Re-export all auth types and constants from batata_auth
pub use batata_auth::model::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_constants() {
        assert_eq!(GLOBAL_ADMIN_ROLE, "ROLE_ADMIN");
        assert_eq!(AUTHORIZATION_HEADER, "Authorization");
        assert_eq!(TOKEN_PREFIX, "Bearer ");
        assert_eq!(DEFAULT_USER, "nacos");
        assert_eq!(DEFAULT_TOKEN_EXPIRE_SECONDS, 18000);
    }

    #[test]
    fn test_ldap_constants() {
        assert_eq!(NACOS_CORE_AUTH_LDAP_URL, "nacos.core.auth.ldap.url");
        assert_eq!(LDAP_PREFIX, "LDAP_");
    }

    #[test]
    fn test_user_creation() {
        let user = User {
            username: "testuser".to_string(),
            password: "testpass".to_string(),
        };
        assert_eq!(user.username, "testuser");
        assert_eq!(user.password, "testpass");
    }

    #[test]
    fn test_resource_constants() {
        assert_eq!(Resource::SPLITTER, ":");
        assert_eq!(Resource::ANY, "*");
        assert_eq!(Resource::ACTION, "action");
    }

    #[test]
    fn test_auth_context_default() {
        let ctx = AuthContext::default();
        assert!(ctx.username.is_empty());
        assert!(ctx.jwt_error.is_none());
        assert_eq!(ctx.jwt_error_string(), "");
    }
}
