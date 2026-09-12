//! Creation and validation of portal user tokens (openapi / session tokens).
//!
//! Upstream reference: `apollo-portal/.../service/UserTokenService.java`.
//!
//! Apollo splits the token into a `tokenPrefix` (the first 32 chars, indexed
//! for fast lookup) and a `tokenHash` (`sha256` of the full plaintext). The
//! plaintext is returned to the caller exactly once when the token is created
//! and is never stored, so a leaked database cannot be used to impersonate a
//! user. batata keeps the same contract.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::persistence::traits::UserTokenPersistence;
use crate::persistence::traits::ApolloPersistenceService;

/// Service for issuing and validating portal user tokens.
pub struct UserTokenService {
    persistence: std::sync::Arc<dyn ApolloPersistenceService>,
}

impl UserTokenService {
    /// Builds a new `UserTokenService`.
    pub fn new(persistence: std::sync::Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Generates a random plaintext token and stores only its prefix + hash.
    ///
    /// Returns the plaintext token (returned to the caller exactly once) and
    /// the stored model (without the plaintext).
    pub async fn create(
        &self,
        user_id: &str,
        name: &str,
        created_by: &str,
        expires: Option<chrono::DateTime<chrono::Utc>>,
    ) -> anyhow::Result<(String, crate::entity::apollo_user_token::Model)> {
        let uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
        let token = format!("{}-{}", &uuid[..32], &uuid[32..]);
        let prefix = &token[..32];
        let hash = sha256_hex(&token);
        let expires = expires.unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::days(365));
        let model = <dyn UserTokenPersistence>::create_user_token(
            &*self.persistence,
            user_id,
            name,
            prefix,
            &hash,
            None,
            expires,
            created_by,
        )
        .await?;
        Ok((token, model))
    }

    /// Validates a presented plaintext token.
    ///
    /// Returns the stored model when the token's hash matches a non-deleted,
    /// non-revoked, non-expired token; otherwise `None`. On success the
    /// `last_used_time` is refreshed.
    pub async fn validate(
        &self,
        token: &str,
    ) -> anyhow::Result<Option<crate::entity::apollo_user_token::Model>> {
        if token.len() < 32 {
            return Ok(None);
        }
        let prefix = &token[..32];
        let model = <dyn UserTokenPersistence>::get_user_token_by_prefix(&*self.persistence, prefix)
            .await?;
        let model = match model {
            Some(m) => m,
            None => return Ok(None),
        };
        let expected = sha256_hex(token);
        if !bool::from(model.token_hash.as_bytes().ct_eq(expected.as_bytes())) {
            return Ok(None);
        }
        if model.is_deleted {
            return Ok(None);
        }
        if let Some(revoked) = model.revoked_at {
            if !revoked.and_utc().timestamp().eq(&0) {
                return Ok(None);
            }
        }
        if model.expires.and_utc().timestamp() < chrono::Utc::now().timestamp() {
            return Ok(None);
        }
        Ok(Some(model))
    }

    /// Lists every token of a user (non-deleted).
    pub async fn list(&self, user_id: &str) -> anyhow::Result<Vec<crate::entity::apollo_user_token::Model>> {
        <dyn UserTokenPersistence>::list_user_tokens(&*self.persistence, user_id).await
    }

    /// Revokes (soft-deletes) a token by id.
    pub async fn revoke(&self, id: i64) -> anyhow::Result<()> {
        <dyn UserTokenPersistence>::delete_user_token(&*self.persistence, id).await
    }
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
