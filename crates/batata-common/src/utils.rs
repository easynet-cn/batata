//! Utility functions for Batata
//!
//! Common helper functions used across the codebase.

use std::sync::LazyLock;

use if_addrs::IfAddr;

/// Regex pattern for validating identifiers (dataId, group, etc.)
static VALID_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new("^[a-zA-Z0-9_.:-]*$").expect("Invalid regex pattern"));

/// Validate a string contains only allowed characters
///
/// Allowed characters: alphanumeric, underscore, dot, colon, hyphen
///
/// # Examples
///
/// ```
/// use batata_common::is_valid;
///
/// assert!(is_valid("my-config.yaml"));
/// assert!(is_valid("app_name:v1"));
/// assert!(!is_valid("invalid/path"));
/// assert!(!is_valid("with spaces"));
/// ```
pub fn is_valid(str: &str) -> bool {
    VALID_PATTERN.is_match(str)
}

/// Get the local IP address
///
/// Returns the first non-loopback IPv4 address found,
/// or "127.0.0.1" as fallback.
///
/// # Examples
///
/// ```
/// use batata_common::local_ip;
///
/// let ip = local_ip();
/// assert!(!ip.is_empty());
/// ```
pub fn local_ip() -> String {
    if_addrs::get_if_addrs()
        .ok()
        .and_then(|addrs| {
            addrs
                .into_iter()
                .find(|iface| !iface.is_loopback() && matches!(iface.addr, IfAddr::V4(_)))
                .and_then(|iface| match iface.addr {
                    IfAddr::V4(addr) => Some(addr.ip.to_string()),
                    _ => None,
                })
        })
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_alphanumeric() {
        assert!(is_valid("abc123"));
        assert!(is_valid("ABC123"));
        assert!(is_valid("test_value"));
        assert!(is_valid("test-value"));
        assert!(is_valid("test.value"));
        assert!(is_valid("test:value"));
    }

    #[test]
    fn test_is_valid_empty() {
        assert!(is_valid(""));
    }

    #[test]
    fn test_is_valid_invalid_chars() {
        assert!(!is_valid("test value")); // space
        assert!(!is_valid("test@value")); // @
        assert!(!is_valid("test#value")); // #
        assert!(!is_valid("test$value")); // $
        assert!(!is_valid("test/value")); // /
    }

    #[test]
    fn test_local_ip_returns_valid_ip() {
        let ip = local_ip();
        // Should either be a valid IP or fallback to 127.0.0.1
        assert!(
            ip == "127.0.0.1" || ip.split('.').filter_map(|s| s.parse::<u8>().ok()).count() == 4
        );
    }
}
