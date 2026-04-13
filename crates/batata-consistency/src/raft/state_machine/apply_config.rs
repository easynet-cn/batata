//! Config-domain apply handlers for the Raft state machine.
//!
//! Split out of `mod.rs` purely for file-size / review hygiene — no
//! behavioral change. These handlers are invoked by `apply_request()`'s
//! top-level match on `RaftRequest` and must be called through a
//! `RocksStateMachine` owning receiver (same as before the move).

use md5::Digest;
use tracing::{debug, error};

use super::RocksStateMachine;
use super::records::{StoredConfig, StoredConfigGray, StoredConfigHistory};
use crate::raft::request::{ConfigDeleteHistoryInfo, ConfigHistoryInfo, RaftResponse};

impl RocksStateMachine {
    /// Publish config and optionally insert history in a single atomic WriteBatch.
    /// This replaces the previous two-step approach (apply_config_publish + apply_config_history_insert)
    /// with a single RocksDB WriteBatch, reducing from 2 fsyncs to 1 and ensuring atomicity.
    /// The op_type (Insert/Update) is determined here from the existing value, removing the
    /// need for a pre-write read in the distributed persistence layer.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_config_publish_batched(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        md5: &str,
        config_type: Option<String>,
        app_name: Option<String>,
        tag: Option<String>,
        desc: Option<String>,
        src_user: Option<String>,
        src_ip: Option<String>,
        r#use: Option<String>,
        effect: Option<String>,
        schema: Option<String>,
        encrypted_data_key: Option<String>,
        cas_md5: Option<&str>,
        history: Option<ConfigHistoryInfo>,
    ) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);
        let now = chrono::Utc::now().timestamp_millis();

        // Single read: fetch existing StoredConfig for CAS check, created_time
        // preservation, and op_type determination (Insert vs Update).
        let existing_stored: Option<StoredConfig> =
            match self.db.get_cf(self.cf_config(), key.as_bytes()) {
                Ok(Some(bytes)) => bincode::deserialize(&bytes).ok(),
                Ok(None) => None,
                Err(e) => {
                    if cas_md5.is_some() {
                        return RaftResponse::failure(format!("CAS check failed: {}", e));
                    }
                    None
                }
            };

        // CAS (Compare-And-Swap) check
        if let Some(expected_md5) = cas_md5 {
            match &existing_stored {
                Some(existing) => {
                    if existing.md5 != expected_md5 {
                        return RaftResponse::failure(format!(
                            "CAS conflict: expected md5={}, actual md5={}",
                            expected_md5, existing.md5
                        ));
                    }
                }
                None => {
                    if !expected_md5.is_empty() {
                        return RaftResponse::failure(
                            "CAS conflict: config does not exist".to_string(),
                        );
                    }
                }
            }
        }

        // Determine op_type from existing value (eliminates pre-write read in distributed layer)
        let is_update = existing_stored.is_some();

        // Preserve created_time from existing config on update
        let created_time = existing_stored
            .as_ref()
            .map(|c| c.created_time)
            .unwrap_or(now);

        let stored = StoredConfig {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            content: content.to_string(),
            md5: md5.to_string(),
            config_type,
            app_name: app_name.clone(),
            config_tags: tag,
            desc,
            r#use,
            effect,
            schema,
            encrypted_data_key: encrypted_data_key.clone(),
            src_user: src_user.clone(),
            src_ip: src_ip.clone(),
            created_time,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };

        // Build a single WriteBatch for config + history (1 fsync instead of 2)
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(self.cf_config(), key.as_bytes(), encoded);

        // If history info is provided, add history entry to the same batch
        if let Some(hi) = history {
            let op_type = if hi.op_type.is_empty() {
                if is_update { "U" } else { "I" }.to_string()
            } else {
                hi.op_type
            };
            let history_key = Self::config_history_key(data_id, group, tenant, now as u64);
            let history = StoredConfigHistory {
                id: now,
                data_id: data_id.to_string(),
                group: group.to_string(),
                tenant: tenant.to_string(),
                content: content.to_string(),
                md5: md5.to_string(),
                app_name: app_name.clone().unwrap_or_default(),
                src_user: src_user.clone(),
                src_ip: src_ip.clone(),
                op_type,
                publish_type: hi.publish_type.unwrap_or_else(|| "formal".to_string()),
                gray_name: String::new(),
                ext_info: hi.ext_info.unwrap_or_default(),
                encrypted_data_key: encrypted_data_key.clone().unwrap_or_default(),
                created_time: now,
                modified_time: now,
            };
            let history_encoded = match bincode::serialize(&history) {
                Ok(b) => b,
                Err(e) => return RaftResponse::failure(format!("history encode error: {}", e)),
            };
            batch.put_cf(
                self.cf_config_history(),
                history_key.as_bytes(),
                history_encoded,
            );
        }

        match self.db.write_opt(batch, &self.write_opts) {
            Ok(_) => {
                debug!("Config published: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to publish config: {}", e);
                RaftResponse::failure(format!("Failed to publish config: {}", e))
            }
        }
    }

    /// Remove config and optionally insert delete history in a single atomic WriteBatch.
    pub(super) fn apply_config_remove_batched(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        history: Option<Box<ConfigDeleteHistoryInfo>>,
    ) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);
        let now = chrono::Utc::now().timestamp_millis();

        let mut batch = rocksdb::WriteBatch::default();
        batch.delete_cf(self.cf_config(), key.as_bytes());

        if let Some(hi) = history.map(|b| *b) {
            let history_key = Self::config_history_key(data_id, group, tenant, now as u64);
            let history = StoredConfigHistory {
                id: now,
                data_id: data_id.to_string(),
                group: group.to_string(),
                tenant: tenant.to_string(),
                content: hi.content,
                md5: hi.md5,
                app_name: hi.app_name,
                src_user: Some(hi.src_user),
                src_ip: Some(hi.src_ip),
                op_type: "D".to_string(),
                publish_type: "formal".to_string(),
                gray_name: String::new(),
                ext_info: hi.ext_info,
                encrypted_data_key: hi.encrypted_data_key,
                created_time: now,
                modified_time: now,
            };
            let encoded = match bincode::serialize(&history) {
                Ok(b) => b,
                Err(e) => return RaftResponse::failure(format!("history encode error: {}", e)),
            };
            batch.put_cf(self.cf_config_history(), history_key.as_bytes(), encoded);
        }

        match self.db.write_opt(batch, &self.write_opts) {
            Ok(_) => {
                debug!("Config removed: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to remove config: {}", e);
                RaftResponse::failure(format!("Failed to remove config: {}", e))
            }
        }
    }

    // Gray config operations
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_config_gray_publish(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        gray_name: &str,
        gray_rule: &str,
        app_name: Option<String>,
        encrypted_data_key: Option<String>,
        src_user: Option<String>,
        src_ip: Option<String>,
        cas_md5: Option<String>,
    ) -> RaftResponse {
        let key = Self::config_gray_key(data_id, group, tenant, gray_name);
        let now = chrono::Utc::now().timestamp_millis();
        let md5_val = const_hex::encode(md5::Md5::digest(content.as_bytes()));

        // CAS check: if cas_md5 provided, verify current gray MD5 matches
        if let Some(ref expected_md5) = cas_md5 {
            let cf = self.cf_config_gray();
            match self.db.get_cf(cf, key.as_bytes()) {
                Ok(Some(bytes)) => {
                    if let Ok(existing) = bincode::deserialize::<StoredConfigGray>(&bytes) {
                        let current_md5 = existing.md5.as_str();
                        if current_md5 != expected_md5.as_str() {
                            return RaftResponse::failure(format!(
                                "CAS conflict: expected md5={}, actual md5={}",
                                expected_md5, current_md5
                            ));
                        }
                    }
                }
                Ok(None) => {
                    if !expected_md5.is_empty() {
                        return RaftResponse::failure(
                            "CAS conflict: gray config does not exist".to_string(),
                        );
                    }
                }
                Err(e) => {
                    return RaftResponse::failure(format!("CAS check failed: {}", e));
                }
            }
        }

        let stored = StoredConfigGray {
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            content: content.to_string(),
            md5: md5_val,
            app_name: app_name.unwrap_or_default(),
            gray_name: gray_name.to_string(),
            gray_rule: gray_rule.to_string(),
            encrypted_data_key: encrypted_data_key.unwrap_or_default(),
            src_user: src_user.unwrap_or_default(),
            src_ip: src_ip.unwrap_or_default(),
            created_time: now,
            modified_time: now,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };

        match self.db.put_cf_opt(
            self.cf_config_gray(),
            key.as_bytes(),
            encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Gray config published: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to publish gray config: {}", e);
                RaftResponse::failure(format!("Failed to publish gray config: {}", e))
            }
        }
    }

    pub(super) fn apply_config_gray_remove(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        gray_name: &str,
    ) -> RaftResponse {
        let cf = self.cf_config_gray();

        // If gray_name is specified, delete only that specific gray config
        if !gray_name.is_empty() {
            let key = Self::config_gray_key(data_id, group, tenant, gray_name);
            match self.db.delete_cf_opt(cf, key.as_bytes(), &self.write_opts) {
                Ok(_) => {
                    debug!("Gray config removed: {}", key);
                    return RaftResponse::success();
                }
                Err(e) => {
                    error!("Failed to remove gray config: {}", e);
                    return RaftResponse::failure(format!("Failed to remove gray config: {}", e));
                }
            }
        }

        // Scan and delete all gray configs for this data_id/group/tenant
        let prefix = format!("{}@@{}@@{}@@", tenant, group, data_id);

        let mut keys_to_delete = Vec::new();
        let iter = self.db.prefix_iterator_cf(cf, prefix.as_bytes());
        for item in iter {
            match item {
                Ok((key, _)) => {
                    let key_str = String::from_utf8_lossy(&key);
                    if !key_str.starts_with(&prefix) {
                        break;
                    }
                    keys_to_delete.push(key.to_vec());
                }
                Err(e) => {
                    error!("Failed to iterate gray configs: {}", e);
                    return RaftResponse::failure(format!("Failed to iterate gray configs: {}", e));
                }
            }
        }

        if keys_to_delete.is_empty() {
            return RaftResponse::success();
        }

        let mut batch = rocksdb::WriteBatch::default();
        for key in &keys_to_delete {
            batch.delete_cf(cf, key);
        }

        match self.db.write_opt(batch, &self.write_opts) {
            Ok(_) => {
                debug!(
                    "Gray configs removed for {}/{}/{}: {} entries",
                    tenant,
                    group,
                    data_id,
                    keys_to_delete.len()
                );
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to remove gray configs: {}", e);
                RaftResponse::failure(format!("Failed to remove gray configs: {}", e))
            }
        }
    }

    // Config history operations
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply_config_history_insert(
        &self,
        id: i64,
        data_id: &str,
        group: &str,
        tenant: &str,
        content: &str,
        md5: &str,
        app_name: Option<String>,
        src_user: Option<String>,
        src_ip: Option<String>,
        op_type: &str,
        publish_type: Option<String>,
        gray_name: Option<String>,
        ext_info: Option<String>,
        encrypted_data_key: Option<String>,
        created_time: i64,
        last_modified_time: i64,
    ) -> RaftResponse {
        let key = Self::config_history_key(data_id, group, tenant, id as u64);
        let stored = StoredConfigHistory {
            id,
            data_id: data_id.to_string(),
            group: group.to_string(),
            tenant: tenant.to_string(),
            content: content.to_string(),
            md5: md5.to_string(),
            app_name: app_name.unwrap_or_default(),
            src_user,
            src_ip,
            op_type: op_type.to_string(),
            publish_type: publish_type.unwrap_or_else(|| "formal".to_string()),
            gray_name: gray_name.unwrap_or_default(),
            ext_info: ext_info.unwrap_or_default(),
            encrypted_data_key: encrypted_data_key.unwrap_or_default(),
            created_time,
            modified_time: last_modified_time,
        };
        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };

        match self.db.put_cf_opt(
            self.cf_config_history(),
            key.as_bytes(),
            encoded,
            &self.write_opts,
        ) {
            Ok(_) => {
                debug!("Config history inserted: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to insert config history: {}", e);
                RaftResponse::failure(format!("Failed to insert config history: {}", e))
            }
        }
    }

    // Config tags operations - tags are stored as comma-separated in the config entry
    pub(super) fn apply_config_tags_update(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        tag: &str,
    ) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);

        let mut stored: StoredConfig = match self.db.get_cf(self.cf_config(), key.as_bytes()) {
            Ok(Some(bytes)) => match bincode::deserialize(&bytes) {
                Ok(s) => s,
                Err(e) => return RaftResponse::failure(format!("decode error: {}", e)),
            },
            _ => return RaftResponse::failure("Config not found for tag update"),
        };
        stored.config_tags = Some(tag.to_string());
        stored.modified_time = chrono::Utc::now().timestamp_millis();

        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };
        match self
            .db
            .put_cf_opt(self.cf_config(), key.as_bytes(), encoded, &self.write_opts)
        {
            Ok(_) => {
                debug!("Config tags updated: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to update config tags: {}", e);
                RaftResponse::failure(format!("Failed to update config tags: {}", e))
            }
        }
    }

    pub(super) fn apply_config_tags_delete(
        &self,
        data_id: &str,
        group: &str,
        tenant: &str,
        _tag: &str,
    ) -> RaftResponse {
        let key = Self::config_key(data_id, group, tenant);

        let mut stored: StoredConfig = match self.db.get_cf(self.cf_config(), key.as_bytes()) {
            Ok(Some(bytes)) => match bincode::deserialize(&bytes) {
                Ok(s) => s,
                Err(e) => return RaftResponse::failure(format!("decode error: {}", e)),
            },
            _ => return RaftResponse::failure("Config not found for tag delete"),
        };
        stored.config_tags = Some(String::new());
        stored.modified_time = chrono::Utc::now().timestamp_millis();

        let encoded = match bincode::serialize(&stored) {
            Ok(b) => b,
            Err(e) => return RaftResponse::failure(format!("encode error: {}", e)),
        };
        match self
            .db
            .put_cf_opt(self.cf_config(), key.as_bytes(), encoded, &self.write_opts)
        {
            Ok(_) => {
                debug!("Config tags deleted: {}", key);
                RaftResponse::success()
            }
            Err(e) => {
                error!("Failed to delete config tags: {}", e);
                RaftResponse::failure(format!("Failed to delete config tags: {}", e))
            }
        }
    }
}
