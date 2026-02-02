//! Input validation utilities for Batata API
//!
//! This module provides validation functions and traits for API requests.

use validator::ValidationError;

/// Maximum length for data_id field
pub const MAX_DATA_ID_LENGTH: usize = 256;

/// Maximum length for group field
pub const MAX_GROUP_LENGTH: usize = 128;

/// Maximum length for namespace_id field
pub const MAX_NAMESPACE_ID_LENGTH: usize = 128;

/// Maximum length for service_name field
pub const MAX_SERVICE_NAME_LENGTH: usize = 512;

/// Maximum length for content field (1MB)
pub const MAX_CONTENT_LENGTH: usize = 1024 * 1024;

/// Maximum length for username field
pub const MAX_USERNAME_LENGTH: usize = 64;

/// Maximum length for password field
pub const MAX_PASSWORD_LENGTH: usize = 128;

/// Validate data_id format
///
/// Data ID must:
/// - Not be empty
/// - Not exceed MAX_DATA_ID_LENGTH characters
/// - Contain only alphanumeric characters, dots, hyphens, and underscores
pub fn validate_data_id(data_id: &str) -> Result<(), ValidationError> {
    if data_id.is_empty() {
        return Err(ValidationError::new("data_id_empty"));
    }
    if data_id.len() > MAX_DATA_ID_LENGTH {
        return Err(ValidationError::new("data_id_too_long"));
    }
    if !data_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == ':')
    {
        return Err(ValidationError::new("data_id_invalid_chars"));
    }
    Ok(())
}

/// Validate group format
pub fn validate_group(group: &str) -> Result<(), ValidationError> {
    if group.is_empty() {
        return Err(ValidationError::new("group_empty"));
    }
    if group.len() > MAX_GROUP_LENGTH {
        return Err(ValidationError::new("group_too_long"));
    }
    if !group
        .chars()
        .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == ':')
    {
        return Err(ValidationError::new("group_invalid_chars"));
    }
    Ok(())
}

/// Validate namespace_id format (can be empty for default namespace)
pub fn validate_namespace_id(namespace_id: &str) -> Result<(), ValidationError> {
    if namespace_id.len() > MAX_NAMESPACE_ID_LENGTH {
        return Err(ValidationError::new("namespace_id_too_long"));
    }
    if !namespace_id.is_empty()
        && !namespace_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ValidationError::new("namespace_id_invalid_chars"));
    }
    Ok(())
}

/// Validate service_name format
pub fn validate_service_name(service_name: &str) -> Result<(), ValidationError> {
    if service_name.is_empty() {
        return Err(ValidationError::new("service_name_empty"));
    }
    if service_name.len() > MAX_SERVICE_NAME_LENGTH {
        return Err(ValidationError::new("service_name_too_long"));
    }
    Ok(())
}

/// Validate content length
pub fn validate_content(content: &str) -> Result<(), ValidationError> {
    if content.len() > MAX_CONTENT_LENGTH {
        return Err(ValidationError::new("content_too_long"));
    }
    Ok(())
}

/// Validate username format
pub fn validate_username(username: &str) -> Result<(), ValidationError> {
    if username.is_empty() {
        return Err(ValidationError::new("username_empty"));
    }
    if username.len() > MAX_USERNAME_LENGTH {
        return Err(ValidationError::new("username_too_long"));
    }
    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '@' || c == '.')
    {
        return Err(ValidationError::new("username_invalid_chars"));
    }
    Ok(())
}

/// Validate password (basic length check, not security policy)
pub fn validate_password(password: &str) -> Result<(), ValidationError> {
    if password.is_empty() {
        return Err(ValidationError::new("password_empty"));
    }
    if password.len() > MAX_PASSWORD_LENGTH {
        return Err(ValidationError::new("password_too_long"));
    }
    if password.len() < 6 {
        return Err(ValidationError::new("password_too_short"));
    }
    Ok(())
}

/// Validate IP address format
pub fn validate_ip(ip: &str) -> Result<(), ValidationError> {
    if ip.is_empty() {
        return Err(ValidationError::new("ip_empty"));
    }
    // Basic IP validation (v4 or v6)
    if ip.parse::<std::net::IpAddr>().is_err() {
        return Err(ValidationError::new("ip_invalid"));
    }
    Ok(())
}

/// Validate port number
pub fn validate_port(port: u16) -> Result<(), ValidationError> {
    if port == 0 {
        return Err(ValidationError::new("port_invalid"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_data_id() {
        assert!(validate_data_id("my-config.yaml").is_ok());
        assert!(validate_data_id("config_v1:prod").is_ok());
        assert!(validate_data_id("").is_err());
        assert!(validate_data_id("invalid/path").is_err());
        assert!(validate_data_id(&"a".repeat(MAX_DATA_ID_LENGTH + 1)).is_err());
    }

    #[test]
    fn test_validate_group() {
        assert!(validate_group("DEFAULT_GROUP").is_ok());
        assert!(validate_group("my-group_v1").is_ok());
        assert!(validate_group("").is_err());
        assert!(validate_group("invalid/group").is_err());
    }

    #[test]
    fn test_validate_namespace_id() {
        assert!(validate_namespace_id("").is_ok()); // Empty is valid (default namespace)
        assert!(validate_namespace_id("my-namespace").is_ok());
        assert!(validate_namespace_id("invalid/namespace").is_err());
    }

    #[test]
    fn test_validate_username() {
        assert!(validate_username("nacos").is_ok());
        assert!(validate_username("user@example.com").is_ok());
        assert!(validate_username("").is_err());
        assert!(validate_username("user with space").is_err());
    }

    #[test]
    fn test_validate_password() {
        assert!(validate_password("password123").is_ok());
        assert!(validate_password("").is_err());
        assert!(validate_password("short").is_err()); // Less than 6 chars
    }

    #[test]
    fn test_validate_ip() {
        assert!(validate_ip("192.168.1.1").is_ok());
        assert!(validate_ip("::1").is_ok());
        assert!(validate_ip("").is_err());
        assert!(validate_ip("not-an-ip").is_err());
    }

    #[test]
    fn test_validate_port() {
        assert!(validate_port(8080).is_ok());
        assert!(validate_port(0).is_err());
    }
}
