//! JWT token service
//!
//! Provides JWT token encoding, decoding, caching, and revocation.

use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use moka::sync::Cache;
use parking_lot::RwLock;

use crate::model::NacosJwtPayload;

/// Cached token data containing the full payload
#[derive(Clone)]
struct CachedTokenData {
    claims: NacosJwtPayload,
}

/// JWT Token cache to avoid repeated validation of the same token
static TOKEN_CACHE: LazyLock<Cache<String, CachedTokenData>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(50_000)
        .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
        .build()
});

/// Token blacklist for revoked tokens (by token string)
/// TTL of 24 hours to cover token expiration
static TOKEN_BLACKLIST: LazyLock<Cache<String, ()>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(100_000)
        .time_to_live(Duration::from_secs(86400)) // 24 hours TTL
        .build()
});

/// User blacklist for revoked users (all their tokens are invalid)
/// This is cleared when the user logs in again
static USER_BLACKLIST: LazyLock<RwLock<HashSet<String>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

/// Decode and validate JWT token with caching
///
/// This function:
/// 1. Checks if the token is in the blacklist (revoked)
/// 2. Checks if the user is in the user blacklist
/// 3. Returns cached result if available and not expired
/// 4. Otherwise validates the token and caches the result
pub fn decode_jwt_token_cached(
    token: &str,
    secret_key: &str,
) -> jsonwebtoken::errors::Result<jsonwebtoken::TokenData<NacosJwtPayload>> {
    // Check if token is blacklisted (revoked)
    if TOKEN_BLACKLIST.contains_key(token) {
        return Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidToken,
        ));
    }

    // Check cache first - use token directly for lookup
    if let Some(cached) = TOKEN_CACHE.get(token) {
        let now = chrono::Utc::now().timestamp();
        if cached.claims.exp > now {
            // Check if user is blacklisted
            if is_user_blacklisted(&cached.claims.sub) {
                TOKEN_CACHE.invalidate(token);
                return Err(jsonwebtoken::errors::Error::from(
                    jsonwebtoken::errors::ErrorKind::InvalidToken,
                ));
            }
            return Ok(jsonwebtoken::TokenData {
                header: jsonwebtoken::Header::default(),
                claims: cached.claims,
            });
        }
        // Token expired in cache, invalidate it
        TOKEN_CACHE.invalidate(token);
    }

    // Cache miss or expired - perform actual validation
    let result = decode_jwt_token(token, secret_key)?;

    // Check if user is blacklisted
    if is_user_blacklisted(&result.claims.sub) {
        return Err(jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidToken,
        ));
    }

    // Only allocate String once when inserting to cache
    TOKEN_CACHE.insert(
        token.to_string(),
        CachedTokenData {
            claims: result.claims.clone(),
        },
    );

    Ok(result)
}

/// Decode and validate JWT token without caching
pub fn decode_jwt_token(
    token: &str,
    secret_key: &str,
) -> jsonwebtoken::errors::Result<jsonwebtoken::TokenData<NacosJwtPayload>> {
    let decoding_key = DecodingKey::from_base64_secret(secret_key)?;
    decode::<NacosJwtPayload>(token, &decoding_key, &Validation::default())
}

/// Invalidate a token from the cache
pub fn invalidate_token(token: &str) {
    TOKEN_CACHE.invalidate(token);
}

/// Revoke a specific token (add to blacklist)
///
/// The token will be rejected for authentication until it expires from the blacklist.
pub fn revoke_token(token: &str) {
    TOKEN_CACHE.invalidate(token);
    TOKEN_BLACKLIST.insert(token.to_string(), ());
}

/// Revoke all tokens for a specific user
///
/// All existing tokens for this user will be rejected.
/// Call `unblacklist_user` when the user logs in again to allow new tokens.
pub fn revoke_user_tokens(username: &str) {
    USER_BLACKLIST.write().insert(username.to_string());
    // No need to invalidate_all(): decode_jwt_token_cached() already checks
    // USER_BLACKLIST and invalidates individual tokens for blacklisted users
    // on cache hit. New requests will be rejected immediately.
}

/// Remove a user from the blacklist (typically called after successful login)
pub fn unblacklist_user(username: &str) {
    USER_BLACKLIST.write().remove(username);
}

/// Check if a user is blacklisted
fn is_user_blacklisted(username: &str) -> bool {
    USER_BLACKLIST.read().contains(username)
}

/// Check if a token is blacklisted
pub fn is_token_blacklisted(token: &str) -> bool {
    TOKEN_BLACKLIST.contains_key(token)
}

/// Clear the entire token cache
pub fn clear_token_cache() {
    TOKEN_CACHE.invalidate_all();
}

/// Clear all blacklists (for testing or admin purposes)
pub fn clear_all_blacklists() {
    TOKEN_BLACKLIST.invalidate_all();
    USER_BLACKLIST.write().clear();
}

/// Encode a JWT token
pub fn encode_jwt_token(
    sub: &str,
    secret_key: &str,
    expire_seconds: i64,
) -> jsonwebtoken::errors::Result<String> {
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(expire_seconds))
        .unwrap_or_else(chrono::Utc::now)
        .timestamp();

    let payload = NacosJwtPayload {
        sub: sub.to_string(),
        exp,
    };

    let header = Header {
        typ: None,
        alg: Algorithm::HS256,
        cty: None,
        jku: None,
        jwk: None,
        kid: None,
        x5u: None,
        x5c: None,
        x5t: None,
        x5t_s256: None,
    };

    let encoding_key = EncodingKey::from_base64_secret(secret_key)?;
    encode(&header, &payload, &encoding_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Valid base64-encoded 256-bit secret key for testing
    const TEST_SECRET: &str = "c2VjcmV0S2V5Rm9yVGVzdGluZzEyMzQ1Njc4OTAxMjM0NTY=";

    // Counter for generating unique usernames per test to avoid cross-test interference
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_user(prefix: &str) -> String {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("{}_{}", prefix, id)
    }

    #[test]
    fn test_encode_jwt_token_success() {
        let token = encode_jwt_token("admin_enc", TEST_SECRET, 3600).unwrap();
        assert!(!token.is_empty());
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let user = unique_user("roundtrip");
        let token = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();
        let decoded = decode_jwt_token(&token, TEST_SECRET).unwrap();
        assert_eq!(decoded.claims.sub, user);
        assert!(decoded.claims.exp > chrono::Utc::now().timestamp());
    }

    #[test]
    fn test_decode_with_wrong_secret() {
        let token = encode_jwt_token("wrongsec", TEST_SECRET, 3600).unwrap();
        let wrong_secret = "d3JvbmdLZXlGb3JUZXN0aW5nMTIzNDU2Nzg5MDEyMzQ1Ng==";
        assert!(decode_jwt_token(&token, wrong_secret).is_err());
    }

    #[test]
    fn test_decode_expired_token() {
        let token = encode_jwt_token("expired", TEST_SECRET, -120).unwrap();
        assert!(decode_jwt_token(&token, TEST_SECRET).is_err());
    }

    #[test]
    fn test_decode_invalid_token() {
        assert!(decode_jwt_token("not.a.valid.jwt", TEST_SECRET).is_err());
    }

    #[test]
    fn test_decode_empty_token() {
        assert!(decode_jwt_token("", TEST_SECRET).is_err());
    }

    #[test]
    fn test_decode_cached_returns_same_result() {
        let user = unique_user("cached");
        let token = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();
        let result1 = decode_jwt_token_cached(&token, TEST_SECRET).unwrap();
        let result2 = decode_jwt_token_cached(&token, TEST_SECRET).unwrap();
        assert_eq!(result1.claims.sub, result2.claims.sub);
        assert_eq!(result1.claims.exp, result2.claims.exp);
    }

    #[test]
    fn test_revoke_token() {
        let user = unique_user("revoke");
        let token = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();

        // Token should work before revocation
        assert!(decode_jwt_token(&token, TEST_SECRET).is_ok());

        revoke_token(&token);
        assert!(TOKEN_BLACKLIST.contains_key(&token));

        // Cached decode should fail (blacklisted)
        assert!(decode_jwt_token_cached(&token, TEST_SECRET).is_err());
    }

    #[test]
    fn test_revoke_user_tokens() {
        let user = unique_user("blocked");
        let token = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();

        // Prime the cache
        assert!(decode_jwt_token_cached(&token, TEST_SECRET).is_ok());

        // Revoke all tokens for user
        revoke_user_tokens(&user);

        // Cached token should now fail
        assert!(decode_jwt_token_cached(&token, TEST_SECRET).is_err());

        // Cleanup: unblacklist so we don't affect other tests
        unblacklist_user(&user);
    }

    #[test]
    fn test_unblacklist_user() {
        let user = unique_user("unblock");

        // Blacklist user
        revoke_user_tokens(&user);

        // Create token — should fail due to user blacklist
        let token = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();
        assert!(decode_jwt_token_cached(&token, TEST_SECRET).is_err());

        // Unblacklist user
        unblacklist_user(&user);

        // New token should work now (invalidate cached failure first)
        invalidate_token(&token);
        let token2 = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();
        assert!(decode_jwt_token_cached(&token2, TEST_SECRET).is_ok());
    }

    #[test]
    fn test_invalidate_token() {
        let user = unique_user("invalidate");
        let token = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();

        // Prime cache
        assert!(decode_jwt_token_cached(&token, TEST_SECRET).is_ok());

        // Invalidate from cache (not blacklist)
        invalidate_token(&token);
        assert!(!is_token_blacklisted(&token));

        // Should still work (re-validates)
        assert!(decode_jwt_token_cached(&token, TEST_SECRET).is_ok());
    }

    #[test]
    fn test_token_cache_invalidation() {
        let user = unique_user("cacheinv");
        let token = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();
        // Prime cache
        assert!(decode_jwt_token_cached(&token, TEST_SECRET).is_ok());
        // Invalidate this specific token
        TOKEN_CACHE.invalidate(&token);
        // Should still validate (just re-decodes)
        assert!(decode_jwt_token(&token, TEST_SECRET).is_ok());
    }

    #[test]
    fn test_blacklist_operations() {
        // Test token blacklist add/check (without clearing shared state)
        let user = unique_user("blops");
        let token = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();

        // Not blacklisted initially
        assert!(!TOKEN_BLACKLIST.contains_key(&token));

        // Add to blacklist
        TOKEN_BLACKLIST.insert(token.clone(), ());
        assert!(TOKEN_BLACKLIST.contains_key(&token));

        // Remove just this token (clean up without affecting others)
        TOKEN_BLACKLIST.invalidate(&token);
        assert!(!TOKEN_BLACKLIST.contains_key(&token));
    }

    #[test]
    fn test_encode_with_invalid_secret() {
        assert!(encode_jwt_token("user", "not_valid_base64!!!", 3600).is_err());
    }

    #[test]
    fn test_token_with_special_characters_in_subject() {
        let token = encode_jwt_token("user@domain.com", TEST_SECRET, 3600).unwrap();
        let decoded = decode_jwt_token(&token, TEST_SECRET).unwrap();
        assert_eq!(decoded.claims.sub, "user@domain.com");
    }

    #[test]
    fn test_token_with_empty_subject() {
        let token = encode_jwt_token("", TEST_SECRET, 3600).unwrap();
        let decoded = decode_jwt_token(&token, TEST_SECRET).unwrap();
        assert_eq!(decoded.claims.sub, "");
    }

    #[test]
    fn test_multiple_tokens_same_user() {
        let user = unique_user("multi");
        let token1 = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();
        let token2 = encode_jwt_token(&user, TEST_SECRET, 3600).unwrap();
        assert!(decode_jwt_token(&token1, TEST_SECRET).is_ok());
        assert!(decode_jwt_token(&token2, TEST_SECRET).is_ok());
    }

    mod proptest_auth {
        use super::*;
        use proptest::prelude::*;

        // Valid base64 secret for testing
        const TEST_SECRET: &str = "c2VjcmV0S2V5Rm9yVGVzdGluZzEyMzQ1Njc4OTAxMjM0NTY=";

        proptest! {
            #[test]
            fn test_encode_decode_roundtrip_any_username(
                username in "[a-zA-Z0-9@._-]{1,100}"
            ) {
                let token = encode_jwt_token(&username, TEST_SECRET, 3600).unwrap();
                let decoded = decode_jwt_token(&token, TEST_SECRET).unwrap();
                prop_assert_eq!(decoded.claims.sub, username);
            }

            #[test]
            fn test_jwt_always_has_three_parts(
                username in "[a-zA-Z]{1,20}"
            ) {
                let token = encode_jwt_token(&username, TEST_SECRET, 3600).unwrap();
                prop_assert_eq!(token.split('.').count(), 3);
            }

            #[test]
            fn test_different_usernames_produce_different_tokens(
                user1 in "[a-z]{5,10}",
                user2 in "[A-Z]{5,10}",
            ) {
                let token1 = encode_jwt_token(&user1, TEST_SECRET, 3600).unwrap();
                let token2 = encode_jwt_token(&user2, TEST_SECRET, 3600).unwrap();
                if user1 != user2 {
                    prop_assert_ne!(token1, token2);
                }
            }

            #[test]
            fn test_positive_expiry_creates_future_token(
                expire in 1i64..=86400i64
            ) {
                let token = encode_jwt_token("user", TEST_SECRET, expire).unwrap();
                let decoded = decode_jwt_token(&token, TEST_SECRET).unwrap();
                prop_assert!(decoded.claims.exp > chrono::Utc::now().timestamp() - 1);
            }
        }
    }
}
