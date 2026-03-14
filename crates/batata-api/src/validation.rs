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

    #[test]
    fn test_validate_data_id_boundary_length() {
        // Exactly at max length should pass
        let max_len_id = "a".repeat(MAX_DATA_ID_LENGTH);
        assert!(validate_data_id(&max_len_id).is_ok());

        // One over should fail
        let over_len_id = "a".repeat(MAX_DATA_ID_LENGTH + 1);
        assert!(validate_data_id(&over_len_id).is_err());
    }

    #[test]
    fn test_validate_data_id_allowed_chars() {
        assert!(validate_data_id("my-app.config").is_ok());
        assert!(validate_data_id("app_v2").is_ok());
        assert!(validate_data_id("ns:config").is_ok());
        assert!(validate_data_id("config123").is_ok());
    }

    #[test]
    fn test_validate_data_id_disallowed_chars() {
        assert!(validate_data_id("path/to/config").is_err());
        assert!(validate_data_id("config with space").is_err());
        assert!(validate_data_id("config@v1").is_err());
        assert!(validate_data_id("config#tag").is_err());
        assert!(validate_data_id("config$var").is_err());
    }

    #[test]
    fn test_validate_group_boundary() {
        let max_group = "g".repeat(MAX_GROUP_LENGTH);
        assert!(validate_group(&max_group).is_ok());

        let over_group = "g".repeat(MAX_GROUP_LENGTH + 1);
        assert!(validate_group(&over_group).is_err());
    }

    #[test]
    fn test_validate_namespace_id_empty_is_valid() {
        assert!(validate_namespace_id("").is_ok());
    }

    #[test]
    fn test_validate_namespace_id_allowed_chars() {
        assert!(validate_namespace_id("my-namespace").is_ok());
        assert!(validate_namespace_id("ns_123").is_ok());
        assert!(validate_namespace_id("production").is_ok());
    }

    #[test]
    fn test_validate_namespace_id_disallowed_chars() {
        assert!(validate_namespace_id("ns/path").is_err());
        assert!(validate_namespace_id("ns.name").is_err());
        assert!(validate_namespace_id("ns:name").is_err());
        assert!(validate_namespace_id("ns name").is_err());
    }

    #[test]
    fn test_validate_namespace_id_boundary() {
        let max_ns = "n".repeat(MAX_NAMESPACE_ID_LENGTH);
        assert!(validate_namespace_id(&max_ns).is_ok());

        let over_ns = "n".repeat(MAX_NAMESPACE_ID_LENGTH + 1);
        assert!(validate_namespace_id(&over_ns).is_err());
    }

    #[test]
    fn test_validate_service_name_boundary() {
        let max_name = "s".repeat(MAX_SERVICE_NAME_LENGTH);
        assert!(validate_service_name(&max_name).is_ok());

        let over_name = "s".repeat(MAX_SERVICE_NAME_LENGTH + 1);
        assert!(validate_service_name(&over_name).is_err());
    }

    #[test]
    fn test_validate_service_name_empty() {
        assert!(validate_service_name("").is_err());
    }

    #[test]
    fn test_validate_content_boundary() {
        let max_content = "c".repeat(MAX_CONTENT_LENGTH);
        assert!(validate_content(&max_content).is_ok());

        let over_content = "c".repeat(MAX_CONTENT_LENGTH + 1);
        assert!(validate_content(&over_content).is_err());
    }

    #[test]
    fn test_validate_content_empty() {
        assert!(validate_content("").is_ok());
    }

    #[test]
    fn test_validate_username_allowed_chars() {
        assert!(validate_username("admin").is_ok());
        assert!(validate_username("user_123").is_ok());
        assert!(validate_username("user-name").is_ok());
        assert!(validate_username("user@domain.com").is_ok());
        assert!(validate_username("first.last").is_ok());
    }

    #[test]
    fn test_validate_username_disallowed_chars() {
        assert!(validate_username("user name").is_err());
        assert!(validate_username("user#tag").is_err());
        assert!(validate_username("user$var").is_err());
    }

    #[test]
    fn test_validate_username_boundary() {
        let max_user = "u".repeat(MAX_USERNAME_LENGTH);
        assert!(validate_username(&max_user).is_ok());

        let over_user = "u".repeat(MAX_USERNAME_LENGTH + 1);
        assert!(validate_username(&over_user).is_err());
    }

    #[test]
    fn test_validate_password_boundary() {
        // Min length is 6
        assert!(validate_password("12345").is_err());
        assert!(validate_password("123456").is_ok());

        let max_pass = "p".repeat(MAX_PASSWORD_LENGTH);
        assert!(validate_password(&max_pass).is_ok());

        let over_pass = "p".repeat(MAX_PASSWORD_LENGTH + 1);
        assert!(validate_password(&over_pass).is_err());
    }

    #[test]
    fn test_validate_ip_v4() {
        assert!(validate_ip("192.168.1.1").is_ok());
        assert!(validate_ip("10.0.0.1").is_ok());
        assert!(validate_ip("0.0.0.0").is_ok());
        assert!(validate_ip("255.255.255.255").is_ok());
    }

    #[test]
    fn test_validate_ip_v6() {
        assert!(validate_ip("::1").is_ok());
        assert!(validate_ip("::").is_ok());
        assert!(validate_ip("2001:db8::1").is_ok());
        assert!(validate_ip("fe80::1%eth0").is_err()); // Zone ID not always valid
    }

    #[test]
    fn test_validate_ip_invalid() {
        assert!(validate_ip("").is_err());
        assert!(validate_ip("not-an-ip").is_err());
        assert!(validate_ip("999.999.999.999").is_err());
        assert!(validate_ip("192.168.1").is_err());
    }

    #[test]
    fn test_validate_port_valid() {
        assert!(validate_port(1).is_ok());
        assert!(validate_port(80).is_ok());
        assert!(validate_port(443).is_ok());
        assert!(validate_port(8080).is_ok());
        assert!(validate_port(8848).is_ok());
        assert!(validate_port(65535).is_ok());
    }

    #[test]
    fn test_validate_port_zero() {
        assert!(validate_port(0).is_err());
    }

    #[test]
    fn test_validation_constants() {
        assert_eq!(MAX_DATA_ID_LENGTH, 256);
        assert_eq!(MAX_GROUP_LENGTH, 128);
        assert_eq!(MAX_NAMESPACE_ID_LENGTH, 128);
        assert_eq!(MAX_SERVICE_NAME_LENGTH, 512);
        assert_eq!(MAX_CONTENT_LENGTH, 1024 * 1024);
        assert_eq!(MAX_USERNAME_LENGTH, 64);
        assert_eq!(MAX_PASSWORD_LENGTH, 128);
    }

    // Property-based tests
    mod proptest_validation {
        use super::super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_valid_data_id_always_accepted(
                data_id in "[a-zA-Z0-9._:-]{1,256}"
            ) {
                prop_assert!(validate_data_id(&data_id).is_ok());
            }

            #[test]
            fn test_oversized_data_id_rejected(
                data_id in "[a-zA-Z0-9]{257,300}"
            ) {
                prop_assert!(validate_data_id(&data_id).is_err());
            }

            #[test]
            fn test_valid_group_always_accepted(
                group in "[a-zA-Z0-9._:-]{1,128}"
            ) {
                prop_assert!(validate_group(&group).is_ok());
            }

            #[test]
            fn test_valid_namespace_id_accepted(
                ns in "[a-zA-Z0-9_-]{0,128}"
            ) {
                prop_assert!(validate_namespace_id(&ns).is_ok());
            }

            #[test]
            fn test_valid_username_accepted(
                username in "[a-zA-Z0-9_.@-]{1,64}"
            ) {
                prop_assert!(validate_username(&username).is_ok());
            }

            #[test]
            fn test_valid_password_accepted(
                password in "[a-zA-Z0-9!@#$%^&*]{6,128}"
            ) {
                prop_assert!(validate_password(&password).is_ok());
            }

            #[test]
            fn test_valid_ipv4_accepted(
                a in 0u8..=255u8,
                b in 0u8..=255u8,
                c in 0u8..=255u8,
                d in 0u8..=255u8,
            ) {
                let ip = format!("{}.{}.{}.{}", a, b, c, d);
                prop_assert!(validate_ip(&ip).is_ok());
            }

            #[test]
            fn test_valid_port_accepted(port in 1u16..=65535u16) {
                prop_assert!(validate_port(port).is_ok());
            }

            #[test]
            fn test_service_name_non_empty_accepted(
                name in "[a-zA-Z0-9._-]{1,512}"
            ) {
                prop_assert!(validate_service_name(&name).is_ok());
            }

            #[test]
            fn test_content_within_limit_accepted(
                content in ".{0,1024}"
            ) {
                prop_assert!(validate_content(&content).is_ok());
            }
        }
    }
}
