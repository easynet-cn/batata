//! Creation and validation of portal user tokens (openapi / session tokens).
//!
//! Upstream reference: `apollo-portal/.../service/UserTokenService.java`.
//!
//! Apollo splits the token into a `tokenPrefix` (the first 32 chars, indexed
//! for fast lookup) and a `tokenHash` (`sha256` of the full plaintext). The
//! plaintext is returned to the caller exactly once when the token is created
//! and is never stored, so a leaked database cannot be used to impersonate a
//! user. batata keeps the same contract.

use batata_common::datetime::DateTimeExt;
use batata_common::datetime::NaiveDateTimeExt;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::persistence::traits::ApolloPersistenceService;
use crate::persistence::traits::UserTokenPersistence;

/// Upstream personal-access-token prefix (`apollo_pat_{prefix}_{secret}`).
pub const TOKEN_PREFIX: &str = "apollo_pat_";
const TOKEN_PREFIX_BYTES: usize = 6;
const TOKEN_SECRET_BYTES: usize = 24;

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
        expires: Option<batata_common::datetime::DateTimeUtc>,
    ) -> anyhow::Result<(String, crate::entity::apollo_user_token::Model)> {
        self.create_with_options(user_id, name, created_by, expires, "{}", 0)
            .await
    }

    /// Issues a token with an explicit upstream JSON scope document.
    pub async fn create_with_scope(
        &self,
        user_id: &str,
        name: &str,
        created_by: &str,
        expires: Option<batata_common::datetime::DateTimeUtc>,
        scopes: &str,
    ) -> anyhow::Result<(String, crate::entity::apollo_user_token::Model)> {
        self.create_with_options(user_id, name, created_by, expires, scopes, 0)
            .await
    }

    /// Issues a token with an explicit scope document and a per-second rate
    /// limit. `0` means unlimited, matching upstream `resolveRateLimit`.
    pub async fn create_with_options(
        &self,
        user_id: &str,
        name: &str,
        created_by: &str,
        expires: Option<batata_common::datetime::DateTimeUtc>,
        scopes: &str,
        rate_limit: i32,
    ) -> anyhow::Result<(String, crate::entity::apollo_user_token::Model)> {
        if rate_limit < 0 {
            anyhow::bail!("rate limit must not be negative");
        }
        // Newly issued tokens are unrestricted by default; upstream stores an
        // explicit empty JSON scope for that case. A missing `scopes` column
        // means deny-all and is reserved for legacy/imported rows.
        let scopes = if scopes.trim().is_empty() {
            "{}"
        } else {
            scopes
        };
        let prefix_uuid = uuid::Uuid::new_v4();
        let prefix = hex(&prefix_uuid.as_bytes()[..TOKEN_PREFIX_BYTES]);
        let secret = format!(
            "{}{}",
            hex(uuid::Uuid::new_v4().as_bytes()),
            hex(&uuid::Uuid::new_v4().as_bytes()[..TOKEN_SECRET_BYTES - 16])
        );
        let token = format!("{}{}_{}", TOKEN_PREFIX, prefix, secret);
        let hash = sha256_hex(&token);
        let expires = expires.unwrap_or_else(|| {
            batata_common::datetime::Utc::now() + batata_common::datetime::Duration::days(365)
        });
        let model = <dyn UserTokenPersistence>::create_user_token(
            &*self.persistence,
            user_id,
            name,
            &prefix,
            &hash,
            Some(scopes),
            rate_limit,
            expires,
            created_by,
        )
        .await?;
        Ok((token, model))
    }

    /// Validates a presented plaintext token.
    ///
    /// Returns the stored model when the token's hash matches a non-deleted,
    /// non-revoked, non-expired token; otherwise `None`. Usage fields are
    /// refreshed separately by [`Self::record_usage`].
    pub async fn validate(
        &self,
        token: &str,
    ) -> anyhow::Result<Option<crate::entity::apollo_user_token::Model>> {
        let prefix = match parse_token_prefix(token) {
            Some(prefix) => prefix,
            None => return Ok(None),
        };
        let model =
            <dyn UserTokenPersistence>::get_user_token_by_prefix(&*self.persistence, &prefix)
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
        if let Some(revoked) = model.revoked_at
            && !revoked.and_utc().timestamp().eq(&0)
        {
            return Ok(None);
        }
        if model.expires.and_utc().timestamp() < batata_common::datetime::Utc::now().timestamp() {
            return Ok(None);
        }
        Ok(Some(model))
    }

    /// Records upstream `lastUsedTime`, `lastUsedIp` and `lastUsedUserAgent`
    /// after a successful authentication.
    ///
    /// Upstream `UserTokenService.authenticate` saves these fields on every
    /// request. The write is best-effort: a telemetry failure must not turn a
    /// successfully authenticated request into an error.
    pub async fn record_usage(
        &self,
        model: &crate::entity::apollo_user_token::Model,
        last_used_ip: Option<&str>,
        last_used_user_agent: Option<&str>,
    ) {
        let now = batata_common::datetime::Utc::now().naive_utc();
        if let Err(error) = <dyn UserTokenPersistence>::touch_user_token(
            &*self.persistence,
            model.id,
            now,
            last_used_ip,
            last_used_user_agent,
            &model.user_id,
        )
        .await
        {
            tracing::warn!(%error, "failed to refresh Apollo user-token last-used fields");
        }
    }

    /// Lists every token of a user (non-deleted).
    pub async fn list(
        &self,
        user_id: &str,
    ) -> anyhow::Result<Vec<crate::entity::apollo_user_token::Model>> {
        <dyn UserTokenPersistence>::list_user_tokens(&*self.persistence, user_id).await
    }

    /// Revokes (soft-deletes) a token by id.
    pub async fn revoke(&self, id: i64) -> anyhow::Result<()> {
        <dyn UserTokenPersistence>::delete_user_token(&*self.persistence, id).await
    }
}

/// Parses the lookup prefix from either an upstream PAT or a legacy batata
/// token. Legacy rows stored the first 32 characters as their prefix.
pub fn parse_token_prefix(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    if let Some(remaining) = token.strip_prefix(TOKEN_PREFIX) {
        let (prefix, _) = remaining.split_once('_')?;
        return if prefix.is_empty() {
            None
        } else {
            Some(prefix.to_string())
        };
    }
    if token.len() < 32 {
        return None;
    }
    Some(token[..32].to_string())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
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
