// Integration tests for authentication service
// Tests JWT token encoding, decoding, and caching

use base64::{Engine as _, engine::general_purpose::STANDARD};
use batata::auth::service::auth::{
    decode_jwt_token, decode_jwt_token_cached, encode_jwt_token, invalidate_token,
};

// Generate a valid base64 secret key for testing
fn test_secret_key() -> String {
    STANDARD.encode("test-secret-key-that-is-long-enough-for-hs256-algorithm")
}

#[test]
fn test_encode_decode_jwt_token() {
    let secret = test_secret_key();
    let username = "test_user";
    let expire_seconds = 3600;

    // Encode token
    let token = encode_jwt_token(username, &secret, expire_seconds);
    assert!(token.is_ok());
    let token = token.unwrap();

    // Decode token
    let decoded = decode_jwt_token(&token, &secret);
    assert!(decoded.is_ok());
    let decoded = decoded.unwrap();

    assert_eq!(decoded.claims.sub, username);
}

#[test]
fn test_token_expiration() {
    let secret = test_secret_key();
    let username = "test_user";

    // Create token that expired 120 seconds ago (2 minutes in the past)
    // This exceeds the default JWT validation leeway of 60 seconds
    let token = encode_jwt_token(username, &secret, -120).unwrap();

    // Should fail to decode expired token immediately
    let decoded = decode_jwt_token(&token, &secret);
    assert!(
        decoded.is_err(),
        "Token expired beyond leeway should fail validation"
    );
}

#[test]
fn test_invalid_secret_key() {
    let secret1 = test_secret_key();
    let secret2 = STANDARD.encode("different-secret-key-for-testing-purposes-here");
    let username = "test_user";

    // Encode with one secret
    let token = encode_jwt_token(username, &secret1, 3600).unwrap();

    // Try to decode with different secret
    let decoded = decode_jwt_token(&token, &secret2);
    assert!(decoded.is_err());
}

#[test]
fn test_cached_token_validation() {
    let secret = test_secret_key();
    let username = "cached_user";

    // Create token
    let token = encode_jwt_token(username, &secret, 3600).unwrap();

    // First call - cache miss, performs validation
    let result1 = decode_jwt_token_cached(&token, &secret);
    assert!(result1.is_ok());
    assert_eq!(result1.unwrap().claims.sub, username);

    // Second call - should hit cache
    let result2 = decode_jwt_token_cached(&token, &secret);
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap().claims.sub, username);
}

#[test]
fn test_invalidate_token() {
    let secret = test_secret_key();
    let username = "invalidate_test_user";

    // Create and cache token
    let token = encode_jwt_token(username, &secret, 3600).unwrap();
    let _ = decode_jwt_token_cached(&token, &secret);

    // Invalidate token
    invalidate_token(&token);

    // Should still work (re-validates)
    let result = decode_jwt_token_cached(&token, &secret);
    assert!(result.is_ok());
}

#[test]
fn test_malformed_token() {
    let secret = test_secret_key();

    let result = decode_jwt_token("not.a.valid.token", &secret);
    assert!(result.is_err());

    let result2 = decode_jwt_token("completely-invalid", &secret);
    assert!(result2.is_err());
}

#[test]
fn test_empty_username() {
    let secret = test_secret_key();

    // Empty username should still work (it's valid JWT)
    let token = encode_jwt_token("", &secret, 3600).unwrap();
    let decoded = decode_jwt_token(&token, &secret);
    assert!(decoded.is_ok());
    assert_eq!(decoded.unwrap().claims.sub, "");
}

#[test]
fn test_special_characters_in_username() {
    let secret = test_secret_key();
    let username = "user@example.com/admin#test";

    let token = encode_jwt_token(username, &secret, 3600).unwrap();
    let decoded = decode_jwt_token(&token, &secret);
    assert!(decoded.is_ok());
    assert_eq!(decoded.unwrap().claims.sub, username);
}

#[test]
fn test_unicode_username() {
    let secret = test_secret_key();
    let username = "用户名_ユーザー_🚀";

    let token = encode_jwt_token(username, &secret, 3600).unwrap();
    let decoded = decode_jwt_token(&token, &secret);
    assert!(decoded.is_ok());
    assert_eq!(decoded.unwrap().claims.sub, username);
}

#[test]
fn test_long_expiration() {
    let secret = test_secret_key();
    let username = "long_lived_user";
    let one_year = 365 * 24 * 3600;

    let token = encode_jwt_token(username, &secret, one_year).unwrap();
    let decoded = decode_jwt_token(&token, &secret);
    assert!(decoded.is_ok());
}

#[test]
fn test_concurrent_token_validation() {
    use std::sync::Arc;
    use std::thread;

    let secret = Arc::new(test_secret_key());
    let username = "concurrent_user";
    let token = Arc::new(encode_jwt_token(username, &secret, 3600).unwrap());

    let mut handles = vec![];

    for _ in 0..10 {
        let secret = secret.clone();
        let token = token.clone();
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let result = decode_jwt_token_cached(&token, &secret);
                assert!(result.is_ok());
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
