//! Transaction API for atomic multi-operation support.
//!
//! Maps 1:1 to Consul Go SDK's `api/txn.go`. Supports KV, Node, Service,
//! and Check operations in a single atomic transaction.

use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{
    KVPair, TxnCheck, TxnCheckOp, TxnKVOp, TxnNode, TxnNodeOp, TxnOp, TxnResponse, TxnService,
    TxnServiceOp, WriteMeta, WriteOptions,
};

/// Maximum number of operations allowed per transaction (matches Consul).
pub const MAX_TXN_OPS: usize = 128;

/// Transaction operations
impl ConsulClient {
    /// Execute a transaction containing KV/Node/Service/Check operations.
    ///
    /// Returns `(ok, response, meta)`. `ok` mirrors Consul Go SDK's
    /// first return value — `true` when all operations succeeded.
    /// On failure, `response.errors` contains per-op error details.
    pub async fn txn(
        &self,
        ops: &[TxnOp],
        opts: &WriteOptions,
    ) -> Result<(bool, TxnResponse, WriteMeta)> {
        if ops.len() > MAX_TXN_OPS {
            return Err(crate::error::ConsulError::Other(format!(
                "too many operations in transaction: {} (max {})",
                ops.len(),
                MAX_TXN_OPS
            )));
        }
        // Consul returns 409 Conflict with body when any op fails (partial
        // failure is still a parseable response). Go SDK treats 200 and 409
        // identically — match that contract.
        let (resp, meta): (TxnResponse, WriteMeta) = self
            .put_accepting_conflict("/v1/txn", Some(&ops), opts, &[])
            .await?;
        let ok = resp.errors.is_empty();
        Ok((ok, resp, meta))
    }

    /// Back-compat alias: runs the same endpoint as `txn`.
    pub async fn txn_kv_ops(
        &self,
        ops: &[TxnOp],
        opts: &WriteOptions,
    ) -> Result<(bool, TxnResponse, WriteMeta)> {
        self.txn(ops, opts).await
    }
}

// ---------------------------------------------------------------------------
// Builder helpers mirroring Go SDK idioms
// ---------------------------------------------------------------------------

impl TxnOp {
    /// Build a KV `set` op.
    pub fn kv_set(key: impl Into<String>, value_base64: impl Into<String>) -> Self {
        Self {
            kv: Some(TxnKVOp {
                verb: "set".into(),
                key: key.into(),
                value: Some(value_base64.into()),
                flags: 0,
                index: 0,
                session: None,
            }),
            ..Default::default()
        }
    }

    /// Build a KV `get` op.
    pub fn kv_get(key: impl Into<String>) -> Self {
        Self {
            kv: Some(TxnKVOp {
                verb: "get".into(),
                key: key.into(),
                value: None,
                flags: 0,
                index: 0,
                session: None,
            }),
            ..Default::default()
        }
    }

    /// Build a KV `cas` op with expected ModifyIndex.
    pub fn kv_cas(key: impl Into<String>, value_base64: impl Into<String>, index: u64) -> Self {
        Self {
            kv: Some(TxnKVOp {
                verb: "cas".into(),
                key: key.into(),
                value: Some(value_base64.into()),
                flags: 0,
                index,
                session: None,
            }),
            ..Default::default()
        }
    }

    /// Build a KV `delete` op.
    pub fn kv_delete(key: impl Into<String>) -> Self {
        Self {
            kv: Some(TxnKVOp {
                verb: "delete".into(),
                key: key.into(),
                value: None,
                flags: 0,
                index: 0,
                session: None,
            }),
            ..Default::default()
        }
    }

    /// Build a Node `set` op.
    pub fn node_set(node: TxnNode) -> Self {
        Self {
            node: Some(TxnNodeOp {
                verb: "set".into(),
                node,
            }),
            ..Default::default()
        }
    }

    /// Build a Service `set` op scoped to a node.
    pub fn service_set(node: impl Into<String>, service: TxnService) -> Self {
        Self {
            service: Some(TxnServiceOp {
                verb: "set".into(),
                node: node.into(),
                service,
            }),
            ..Default::default()
        }
    }

    /// Build a Check `set` op.
    pub fn check_set(check: TxnCheck) -> Self {
        Self {
            check: Some(TxnCheckOp {
                verb: "set".into(),
                check,
            }),
            ..Default::default()
        }
    }
}

/// Flatten a TxnResponse into the list of returned KVPairs (for callers
/// that only care about KV results — matches the Go SDK ergonomics where
/// txn results are iterated for `KV` presence).
pub fn txn_results_to_kv(resp: &TxnResponse) -> Vec<KVPair> {
    resp.results.iter().filter_map(|r| r.kv.clone()).collect()
}
