//! Request resource signing for AccessKey/SecretKey authentication
//!
//! Implements HMAC-SHA256 based request signing compatible with
//! Nacos AccessKey/SecretKey authentication.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Sign a resource string with HMAC-SHA256
///
/// Compatible with Nacos SignUtil.sign()
pub fn sign_resource(resource: &str, secret_key: &str) -> String {
    use base64::Engine;

    let mut mac =
        HmacSha256::new_from_slice(secret_key.as_bytes()).expect("HMAC can take key of any size");
    mac.update(resource.as_bytes());
    let result = mac.finalize();
    base64::engine::general_purpose::STANDARD.encode(result.into_bytes())
}

/// Build the string to sign for a request
///
/// Format: "{timestamp}+{resource}"
pub fn build_sign_string(timestamp: i64, resource: &str) -> String {
    if resource.is_empty() {
        timestamp.to_string()
    } else {
        format!("{}+{}", timestamp, resource)
    }
}

/// Generate request signature headers
///
/// Returns headers to add to the request for AK/SK authentication
pub fn sign_request(
    access_key: &str,
    secret_key: &str,
    resource: &str,
) -> std::collections::HashMap<String, String> {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let sign_str = build_sign_string(timestamp, resource);
    let signature = sign_resource(&sign_str, secret_key);

    let mut headers = std::collections::HashMap::new();
    headers.insert("ak".to_string(), access_key.to_string());
    headers.insert("data".to_string(), timestamp.to_string());
    headers.insert("sign".to_string(), signature);
    if !resource.is_empty() {
        headers.insert("resource".to_string(), resource.to_string());
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_resource() {
        let signature = sign_resource("test-resource", "my-secret-key");
        assert!(!signature.is_empty());
        // Same input should produce same output
        let signature2 = sign_resource("test-resource", "my-secret-key");
        assert_eq!(signature, signature2);
    }

    #[test]
    fn test_different_keys_different_signatures() {
        let sig1 = sign_resource("resource", "key1");
        let sig2 = sign_resource("resource", "key2");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_different_resources_different_signatures() {
        let sig1 = sign_resource("resource1", "key");
        let sig2 = sign_resource("resource2", "key");
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_build_sign_string() {
        assert_eq!(build_sign_string(12345, "ns"), "12345+ns");
        assert_eq!(build_sign_string(12345, ""), "12345");
    }

    #[test]
    fn test_sign_request_headers() {
        let headers = sign_request("my-ak", "my-sk", "public");
        assert_eq!(headers.get("ak").unwrap(), "my-ak");
        assert!(headers.contains_key("data"));
        assert!(headers.contains_key("sign"));
        assert_eq!(headers.get("resource").unwrap(), "public");
    }

    #[test]
    fn test_sign_request_no_resource() {
        let headers = sign_request("ak", "sk", "");
        assert!(!headers.contains_key("resource"));
    }
}
