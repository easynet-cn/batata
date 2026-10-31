//! AI-storage apply handlers for the Raft state machine.
//!
//! The AI module (resources, search index, index tasks) lives in column
//! families of the Raft state machine's RocksDB, so its writes have to be
//! applied here to be replicated. The distributed persistence backend never
//! touches those column families directly — it proposes
//! [`RaftRequest::AiWrite`] and lets every node apply the same batch.
//!
//! Both handlers are deliberately generic: a batch is a list of key/value
//! mutations plus an optional ID allocation, so adding an AI feature does not
//! mean adding another `RaftRequest` variant.

use rocksdb::WriteBatch;
use tracing::error;

use super::RocksStateMachine;
use crate::raft::request::{AiWriteOp, RaftResponse};

impl RocksStateMachine {
    /// Apply a batch of AI storage mutations as one `WriteBatch`.
    ///
    /// Compare-and-set ops ([`AiWriteOp::PutIfUnchanged`] and
    /// [`AiWriteOp::DeleteIfUnchanged`]) are decided against committed state,
    /// i.e. before this batch lands, so a batch must not compare a key it also
    /// writes. Whether **every** comparison matched is reported in
    /// [`RaftResponse::data`] as a little-endian `i64` (`1` or `0`).
    pub(super) fn apply_ai_write(&self, ops: &[AiWriteOp]) -> RaftResponse {
        if ops.is_empty() {
            return RaftResponse::success_with_data(1i64.to_le_bytes().to_vec());
        }

        let mut batch = WriteBatch::default();
        let mut compared = true;
        for op in ops {
            let cf = match op {
                AiWriteOp::Put { cf, .. }
                | AiWriteOp::Delete { cf, .. }
                | AiWriteOp::DeletePrefix { cf, .. }
                | AiWriteOp::PutIfUnchanged { cf, .. }
                | AiWriteOp::DeleteIfUnchanged { cf, .. } => self.db.cf_handle(cf),
            };
            let Some(cf) = cf else {
                error!(
                    "AI write rejected: column family not found (database may be corrupted or \
                     opened without the AI column families)"
                );
                return RaftResponse::failure("unknown column family for AI write");
            };

            match op {
                AiWriteOp::Put { key, value, .. } => batch.put_cf(cf, key, value),
                AiWriteOp::Delete { key, .. } => batch.delete_cf(cf, key),
                AiWriteOp::PutIfUnchanged {
                    key,
                    expected,
                    value,
                    ..
                } => match self.compare_cf(cf, key, expected.as_deref()) {
                    Ok(true) => batch.put_cf(cf, key, value),
                    // Lost the race: the mutation is dropped, not applied.
                    Ok(false) => compared = false,
                    Err(e) => return RaftResponse::failure(format!("AI compare failed: {e}")),
                },
                AiWriteOp::DeleteIfUnchanged { key, expected, .. } => {
                    match self.compare_cf(cf, key, expected.as_deref()) {
                        Ok(true) => batch.delete_cf(cf, key),
                        Ok(false) => compared = false,
                        Err(e) => return RaftResponse::failure(format!("AI compare failed: {e}")),
                    }
                }
                AiWriteOp::DeletePrefix { prefix, .. } => {
                    // The scan runs against committed state, before this batch
                    // is written, and each node holds the same state, so the
                    // key set it yields is identical everywhere.
                    let iter = self.db.prefix_iterator_cf(cf, prefix);
                    for item in iter {
                        let (key, _) = match item {
                            Ok(v) => v,
                            Err(e) => {
                                return RaftResponse::failure(format!(
                                    "AI prefix delete failed to iterate: {e}"
                                ));
                            }
                        };
                        if !key.starts_with(prefix.as_slice()) {
                            break;
                        }
                        // `__` is reserved for the internal auto-increment
                        // counters these column families keep; the readers
                        // skip them, so deletes must too.
                        if key.starts_with(b"__") {
                            continue;
                        }
                        batch.delete_cf(cf, &key);
                    }
                }
            }
        }

        self.db.write(batch).map_or_else(
            |e| {
                error!("AI write batch failed: {e}");
                RaftResponse::failure(format!("AI write failed: {e}"))
            },
            |_| RaftResponse::success_with_data((i64::from(compared)).to_le_bytes().to_vec()),
        )
    }

    /// Whether `key` in `cf` currently holds exactly `expected`.
    ///
    /// `expected: None` means "the key must not exist".
    fn compare_cf(
        &self,
        cf: &rocksdb::ColumnFamily,
        key: &[u8],
        expected: Option<&[u8]>,
    ) -> anyhow::Result<bool> {
        let stored = self.db.get_cf(cf, key)?;
        Ok(match expected {
            Some(expected) => stored.as_deref() == Some(expected),
            None => stored.is_none(),
        })
    }

    /// Bump an AI auto-increment counter and return the IDs it handed out.
    ///
    /// Allocation is done here rather than on the proposing node so that the
    /// read-modify-write is covered by the same total order that covers the
    /// writes using the IDs. Two concurrent proposers otherwise read the same
    /// counter value and collide.
    pub(super) fn apply_ai_allocate_ids(
        &self,
        cf: &str,
        counter_key: &str,
        count: u32,
    ) -> RaftResponse {
        if count == 0 {
            return RaftResponse::success_with_data(Vec::new());
        }

        let Some(cf_handle) = self.db.cf_handle(cf) else {
            error!("AI id allocation rejected: column family '{cf}' not found");
            return RaftResponse::failure("unknown column family for AI id allocation");
        };

        let current = match self.db.get_cf(cf_handle, counter_key.as_bytes()) {
            Ok(Some(bytes)) => match String::from_utf8(bytes.to_vec())
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
            {
                Some(v) => v,
                None => {
                    return RaftResponse::failure(format!(
                        "AI id counter '{counter_key}' is not an integer"
                    ));
                }
            },
            Ok(None) => 0,
            Err(e) => return RaftResponse::failure(format!("AI id counter read failed: {e}")),
        };

        let mut ids = Vec::with_capacity(count as usize);
        let mut next = current;
        for _ in 0..count {
            next += 1;
            ids.push(next);
        }

        let mut batch = WriteBatch::default();
        batch.put_cf(
            cf_handle,
            counter_key.as_bytes(),
            next.to_string().as_bytes(),
        );
        self.db.write(batch).map_or_else(
            |e| {
                error!("AI id allocation failed: {e}");
                RaftResponse::failure(format!("AI id allocation failed: {e}"))
            },
            |_| {
                let mut data = Vec::with_capacity(ids.len() * 8);
                for id in &ids {
                    data.extend_from_slice(&id.to_le_bytes());
                }
                RaftResponse::success_with_data(data)
            },
        )
    }
}
