use std::sync::LazyLock;
use std::time::Duration;

use chrono;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use moka::sync::Cache;

use crate::auth::model::NacosJwtPayload;

/// JWT Token cache to avoid repeated validation of the same token
/// Cache key: token string
/// Cache value: validated username from token claims
static TOKEN_CACHE: LazyLock<Cache<String, String>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(10_000)
        .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
        .build()
});

/// Decode and validate JWT token with caching
/// Returns cached result if token was recently validated
pub fn decode_jwt_token_cached(
    token: &str,
    secret_key: &str,
) -> jsonwebtoken::errors::Result<jsonwebtoken::TokenData<NacosJwtPayload>> {
    // Check cache first
    if let Some(username) = TOKEN_CACHE.get(&token.to_string()) {
        // Return a reconstructed TokenData from cache
        // Note: We only cache valid tokens, so exp should still be valid within TTL
        let payload = NacosJwtPayload {
            sub: username,
            exp: chrono::Utc::now().timestamp() + 300, // Approximate exp
        };
        return Ok(jsonwebtoken::TokenData {
            header: jsonwebtoken::Header::default(),
            claims: payload,
        });
    }

    // Cache miss - perform actual validation
    let result = decode_jwt_token(token, secret_key)?;

    // Cache the successful result
    TOKEN_CACHE.insert(token.to_string(), result.claims.sub.clone());

    Ok(result)
}

/// Decode and validate JWT token without caching (original behavior)
pub fn decode_jwt_token(
    token: &str,
    secret_key: &str,
) -> jsonwebtoken::errors::Result<jsonwebtoken::TokenData<NacosJwtPayload>> {
    let decoding_key = DecodingKey::from_base64_secret(secret_key)?;
    decode::<NacosJwtPayload>(token, &decoding_key, &Validation::default())
}

/// Invalidate a token from the cache (e.g., on logout)
#[allow(dead_code)]
pub fn invalidate_token(token: &str) {
    TOKEN_CACHE.invalidate(&token.to_string());
}

/// Clear the entire token cache
#[allow(dead_code)]
pub fn clear_token_cache() {
    TOKEN_CACHE.invalidate_all();
}

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
