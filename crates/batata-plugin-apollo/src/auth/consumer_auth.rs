//! Consumer-token generation for the Apollo openapi authentication.
//!
//! Upstream reference:
//! `apollo-portal/.../openapi/service/ConsumerTokenService.java`.
//!
//! Apollo issues a consumer token as `"{consumerId}-{uuid}"` — an opaque,
//! random string. The openapi client sends this string verbatim in the
//! `Authorization` header and `ConsumerAuthenticationFilter` looks it up
//! directly in `apollo_consumer_token`. batata keeps the same contract: the
//! plaintext token is returned to the caller on creation and is the exact value
//! expected on subsequent requests.

/// Builds a consumer token string for the given consumer id.
///
/// Upstream `ConsumerTokenService.createToken` stores
/// `"{consumerId}-{randomUuid}"`.
pub fn generate_consumer_token(consumer_id: i64) -> String {
    let uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
    format!("{}-{}", consumer_id, &uuid[..32])
}
