//! Portal user service.

use std::sync::Arc;

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::api::dto::UserDTO;
use crate::persistence::traits::{ApolloPersistenceService, UserPersistence};

/// Legacy prefix used by older batata SHA-256 password hashes. The current
/// bootstrap writes a bare 64-char hex digest because `apollo_users.password`
/// is `varchar(64)` upstream; the prefixed form does not fit on SQL backends.
const SHA256_PASSWORD_PREFIX: &str = "sha256:";
const SHA256_HEX_LEN: usize = 64;

fn is_sha256_hex(value: &str) -> bool {
    value.len() == SHA256_HEX_LEN && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// CRUD operations for `apollo_users`.
pub struct UserService {
    persistence: Arc<dyn ApolloPersistenceService>,
}

impl UserService {
    /// Creates the service.
    pub fn new(persistence: Arc<dyn ApolloPersistenceService>) -> Self {
        Self { persistence }
    }

    /// Looks a user up by account name.
    pub async fn get(&self, username: &str) -> anyhow::Result<Option<UserDTO>> {
        Ok(UserPersistence::get_user(&self.persistence, username)
            .await?
            .map(|model| Self::model_to_dto(&model)))
    }

    /// Lists every portal user.
    pub async fn list(&self) -> anyhow::Result<Vec<UserDTO>> {
        let models = UserPersistence::list_users(&self.persistence).await?;
        Ok(models.iter().map(Self::model_to_dto).collect())
    }

    /// Creates a new user, rejecting duplicate account names.
    pub async fn create(&self, dto: UserDTO) -> anyhow::Result<UserDTO> {
        if UserPersistence::get_user(&self.persistence, &dto.username)
            .await?
            .is_some()
        {
            anyhow::bail!("User already exists: {}", dto.username);
        }
        let model = UserPersistence::create_user(&self.persistence, dto).await?;
        Ok(Self::model_to_dto(&model))
    }

    /// Creates or updates a user.
    pub async fn create_or_update(&self, dto: UserDTO) -> anyhow::Result<UserDTO> {
        let exists = UserPersistence::get_user(&self.persistence, &dto.username)
            .await?
            .is_some();
        let model = if exists {
            let username = dto.username.clone();
            UserPersistence::update_user(&self.persistence, &username, dto).await?
        } else {
            UserPersistence::create_user(&self.persistence, dto).await?
        };
        Ok(Self::model_to_dto(&model))
    }

    /// Verifies a presented password against the stored representation.
    ///
    /// Four formats are accepted so batata can read both its own rows and
    /// upstream-imported ones:
    /// - 64-char hex — upstream/bootstrap SHA-256 digest (fits `varchar(64)`);
    /// - `sha256:<hex>` — legacy batata rows written before the column limit
    ///   was discovered (only valid on backends without a strict length cap);
    /// - `$2...` — upstream Apollo's BCrypt hashes;
    /// - plaintext — legacy batata rows created before password login existed.
    ///
    /// Comparisons are constant-time where a hash is available.
    pub fn verify_password(stored: &str, presented: &str) -> bool {
        let expected = stored
            .strip_prefix(SHA256_PASSWORD_PREFIX)
            .or_else(|| is_sha256_hex(stored).then_some(stored));
        if let Some(expected) = expected {
            let digest = Sha256::digest(presented.as_bytes());
            let actual = digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            return bool::from(expected.as_bytes().ct_eq(actual.as_bytes()));
        }
        if stored.starts_with("$2") {
            return bcrypt::verify(presented, stored).unwrap_or(false);
        }
        bool::from(stored.as_bytes().ct_eq(presented.as_bytes()))
    }

    /// Authenticates a user, returning its public DTO on success.
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> anyhow::Result<Option<UserDTO>> {
        let Some(model) = UserPersistence::get_user(&self.persistence, username).await? else {
            // Still hash the presented password to avoid a cheap timing oracle.
            let _ = Sha256::digest(password.as_bytes());
            return Ok(None);
        };
        if !model.enabled || !Self::verify_password(&model.password, password) {
            return Ok(None);
        }
        Ok(Some(Self::model_to_dto(&model)))
    }

    /// Enables or disables a user without touching the stored password.
    pub async fn set_enabled(&self, username: &str, enabled: bool) -> anyhow::Result<UserDTO> {
        let current = UserPersistence::get_user(&self.persistence, username)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User not found: {}", username))?;
        let dto = UserDTO {
            id: Some(current.id),
            username: current.username.clone(),
            password: current.password.clone(),
            email: Some(current.email.clone()),
            enabled,
            data_change_created_by: Some(current.data_change_created_by.clone()),
            data_change_created_time: Some(
                current
                    .data_change_created_time
                    .and_utc()
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
        };
        let model = UserPersistence::update_user(&self.persistence, username, dto).await?;
        Ok(Self::model_to_dto(&model))
    }

    /// Deletes a user.
    pub async fn delete(&self, username: &str) -> anyhow::Result<()> {
        UserPersistence::delete_user(&self.persistence, username).await
    }

    /// Converts a stored row to its public DTO.
    pub fn model_to_dto(model: &crate::entity::apollo_users::Model) -> UserDTO {
        UserDTO {
            id: Some(model.id),
            username: model.username.clone(),
            password: String::new(),
            email: Some(model.email.clone()),
            enabled: model.enabled,
            data_change_created_by: Some(model.data_change_created_by.clone()),
            data_change_created_time: Some(
                model
                    .data_change_created_time
                    .and_utc()
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SHA256_PASSWORD_PREFIX, UserService};

    #[test]
    fn bare_sha256_hex_is_the_schema_width_password_format() {
        let digest = "8d3aa1a6f227d714692a9d5a7fbbda496fb09f17f7207a11ffd0a4cca6cf35b7";
        assert_eq!(digest.len(), 64);
        assert!(UserService::verify_password(digest, "apollo"));
        assert!(!UserService::verify_password(digest, "wrong"));
    }

    #[test]
    fn legacy_prefixed_sha256_and_plaintext_remain_accepted() {
        let digest = "8d3aa1a6f227d714692a9d5a7fbbda496fb09f17f7207a11ffd0a4cca6cf35b7";
        let prefixed = format!("{SHA256_PASSWORD_PREFIX}{digest}");
        assert!(UserService::verify_password(&prefixed, "apollo"));
        assert!(UserService::verify_password("plain-text", "plain-text"));
        assert!(!UserService::verify_password("plain-text", "other"));
    }
}
