//! Utility functions for Batata
//!
//! Common helper functions used across the codebase.

use std::sync::LazyLock;
use std::time::Duration;

use if_addrs::IfAddr;
use moka::sync::Cache;

/// Regex pattern for validating identifiers (dataId, group, etc.)
static VALID_PATTERN: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new("^[a-zA-Z0-9_.:-]*$").expect("Invalid regex pattern"));

/// Global regex cache with TTL of 1 hour and max 10,000 entries
/// This prevents repeated compilation of the same regex patterns
static REGEX_CACHE: LazyLock<Cache<String, regex::Regex>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(10_000)
        .time_to_live(Duration::from_secs(3600))
        .build()
});

/// Get or compile a regex pattern with caching
///
/// This function caches compiled regex patterns to avoid repeated compilation.
/// Returns None if the pattern is invalid.
///
/// # Examples
///
/// ```
/// use batata_common::utils::get_or_compile_regex;
///
/// let re = get_or_compile_regex("^test.*$");
/// assert!(re.is_some());
/// assert!(re.unwrap().is_match("test123"));
///
/// // Invalid pattern returns None
/// let invalid = get_or_compile_regex("[invalid");
/// assert!(invalid.is_none());
/// ```
pub fn get_or_compile_regex(pattern: &str) -> Option<regex::Regex> {
    if let Some(re) = REGEX_CACHE.get(pattern) {
        return Some(re);
    }

    match regex::Regex::new(pattern) {
        Ok(re) => {
            REGEX_CACHE.insert(pattern.to_string(), re.clone());
            Some(re)
        }
        Err(_) => None,
    }
}

/// Check if a text matches a cached regex pattern
///
/// This is a convenience function that combines caching and matching.
/// Returns false if the pattern is invalid.
///
/// # Examples
///
/// ```
/// use batata_common::utils::regex_matches;
///
/// assert!(regex_matches("^test.*", "test123"));
/// assert!(!regex_matches("^test.*", "hello"));
/// assert!(!regex_matches("[invalid", "any")); // Invalid pattern returns false
/// ```
pub fn regex_matches(pattern: &str, text: &str) -> bool {
    get_or_compile_regex(pattern)
        .map(|re| re.is_match(text))
        .unwrap_or(false)
}

/// Convert a glob-style pattern (with * and ?) to a regex pattern and match
///
/// Supports:
/// - `*` matches any sequence of characters
/// - `?` matches any single character
///
/// # Examples
///
/// ```
/// use batata_common::utils::glob_matches;
///
/// assert!(glob_matches("test*", "test123"));
/// assert!(glob_matches("test?", "testA"));
/// assert!(!glob_matches("test?", "test"));
/// ```
pub fn glob_matches(pattern: &str, text: &str) -> bool {
    let regex_pattern = format!(
        "^{}$",
        regex::escape(pattern)
            .replace("\\*", ".*")
            .replace("\\?", ".")
    );
    regex_matches(&regex_pattern, text)
}

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

    #[test]
    fn test_glob_matches_wildcard() {
        assert!(glob_matches("*", "anything"));
        assert!(glob_matches("*", ""));
        assert!(glob_matches("test*", "test"));
        assert!(glob_matches("test*", "test123"));
        assert!(glob_matches("*test", "mytest"));
        assert!(glob_matches("*test*", "mytestvalue"));
    }

    #[test]
    fn test_glob_matches_question_mark() {
        assert!(glob_matches("test?", "testA"));
        assert!(glob_matches("test?", "test1"));
        assert!(!glob_matches("test?", "test"));
        assert!(!glob_matches("test?", "test12"));
        assert!(glob_matches("?est", "test"));
        assert!(glob_matches("t?st", "test"));
    }

    #[test]
    fn test_glob_matches_combined() {
        assert!(glob_matches("t*t?", "testA"));
        assert!(glob_matches("*.yaml", "config.yaml"));
        assert!(glob_matches("app-*-config.?ml", "app-prod-config.yml"));
        assert!(!glob_matches("app-*-config.?ml", "app-prod-config.toml"));
    }

    #[test]
    fn test_glob_matches_exact() {
        assert!(glob_matches("exact", "exact"));
        assert!(!glob_matches("exact", "exactx"));
        assert!(!glob_matches("exact", "xexact"));
    }

    #[test]
    fn test_regex_matches_basic() {
        assert!(regex_matches("^test.*", "test123"));
        assert!(regex_matches("^test.*", "test"));
        assert!(!regex_matches("^test.*", "atest"));
    }

    #[test]
    fn test_regex_matches_invalid_pattern() {
        assert!(!regex_matches("[invalid", "any"));
        assert!(!regex_matches("(unclosed", "any"));
    }

    #[test]
    fn test_get_or_compile_regex_caching() {
        let pattern = "^cached_test_\\d+$";
        let re1 = get_or_compile_regex(pattern);
        assert!(re1.is_some());
        // Second call should use cache
        let re2 = get_or_compile_regex(pattern);
        assert!(re2.is_some());
        assert!(re2.unwrap().is_match("cached_test_123"));
    }

    #[test]
    fn test_get_or_compile_regex_invalid() {
        let result = get_or_compile_regex("[invalid");
        assert!(result.is_none());
    }

    #[test]
    fn test_is_valid_unicode() {
        assert!(!is_valid("日本語"));
        assert!(!is_valid("über"));
        assert!(!is_valid("café"));
    }

    #[test]
    fn test_is_valid_special_allowed_chars() {
        assert!(is_valid("a.b.c"));
        assert!(is_valid("a-b-c"));
        assert!(is_valid("a_b_c"));
        assert!(is_valid("a:b:c"));
        assert!(is_valid("a.b-c_d:e"));
        assert!(is_valid("192.168.1.1"));
    }

    #[test]
    fn test_is_valid_boundary_chars() {
        assert!(!is_valid("a b"));
        assert!(!is_valid("a\tb"));
        assert!(!is_valid("a\nb"));
        assert!(!is_valid("a!b"));
        assert!(!is_valid("a+b"));
        assert!(!is_valid("a=b"));
        assert!(!is_valid("a&b"));
        assert!(!is_valid("a%b"));
    }

    mod proptest_utils {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_valid_string_contains_only_allowed_chars(
                s in "[a-zA-Z0-9_.:-]*"
            ) {
                prop_assert!(is_valid(&s));
            }

            #[test]
            fn test_invalid_string_with_space(
                prefix in "[a-zA-Z]{1,5}",
                suffix in "[a-zA-Z]{1,5}",
            ) {
                let s = format!("{} {}", prefix, suffix);
                prop_assert!(!is_valid(&s));
            }

            #[test]
            fn test_glob_exact_match(s in "[a-zA-Z0-9]{1,20}") {
                prop_assert!(glob_matches(&s, &s));
            }

            #[test]
            fn test_glob_star_matches_anything(s in "[a-zA-Z0-9]{0,50}") {
                prop_assert!(glob_matches("*", &s));
            }

            #[test]
            fn test_glob_prefix_star(
                prefix in "[a-zA-Z]{1,10}",
                suffix in "[a-zA-Z0-9]{0,20}",
            ) {
                let text = format!("{}{}", prefix, suffix);
                let pattern = format!("{}*", prefix);
                prop_assert!(glob_matches(&pattern, &text));
            }

            #[test]
            fn test_regex_cache_consistent(
                pattern in "(\\^)?[a-zA-Z0-9]+\\*?"
            ) {
                let re1 = get_or_compile_regex(&pattern);
                let re2 = get_or_compile_regex(&pattern);
                // Both should return the same validity
                prop_assert_eq!(re1.is_some(), re2.is_some());
            }
        }
    }
}
