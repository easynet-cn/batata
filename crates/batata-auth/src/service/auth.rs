//! JWT token service
//!
//! Provides JWT token encoding, decoding, caching, and revocation.

use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use moka::sync::Cache;
use std::sync::RwLock;

use crate::model::NacosJwtPayload;

/// Cached token data containing the full payload
#[derive(Clone)]
struct CachedTokenData {
    claims: NacosJwtPayload,
}

/// JWT Token cache to avoid repeated validation of the same token
static TOKEN_CACHE: LazyLock<Cache<String, CachedTokenData>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(10_000)
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
    if let Ok(mut blacklist) = USER_BLACKLIST.write() {
        blacklist.insert(username.to_string());
    }
    // Also invalidate any cached tokens for this user
    // Note: This is O(n) but happens rarely (only on user revocation)
    TOKEN_CACHE.invalidate_all();
}

/// Remove a user from the blacklist (typically called after successful login)
pub fn unblacklist_user(username: &str) {
    if let Ok(mut blacklist) = USER_BLACKLIST.write() {
        blacklist.remove(username);
    }
}

/// Check if a user is blacklisted
fn is_user_blacklisted(username: &str) -> bool {
    USER_BLACKLIST
        .read()
        .map(|blacklist| blacklist.contains(username))
        .unwrap_or(false)
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
    if let Ok(mut blacklist) = USER_BLACKLIST.write() {
        blacklist.clear();
    }
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
