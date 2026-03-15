//! Transaction API for atomic multi-operation support

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{TxnOp, TxnResponse, WriteMeta, WriteOptions};

/// Transaction operations
impl ConsulClient {
    /// Execute a transaction (multiple KV operations atomically)
    pub async fn txn(
        &self,
        ops: &[TxnOp],
        opts: &WriteOptions,
    ) -> Result<(TxnResponse, WriteMeta)> {
        self.put("/v1/txn", Some(&ops), opts, &[]).await
    }
}
