//! Run openraft's own storage conformance suite against batata's adapters.
//!
//! openraft 0.10.1 ships `openraft::testing::log::Suite`, ~45 assertions about
//! what a `RaftLogStorage` + `RaftStateMachine` pair must do: membership
//! recovery from log vs snapshot, vote persistence, conflict truncation,
//! purge/delete boundaries, log-id ordering, apply and snapshot transfer.
//! This is the contract the consensus layer relies on; a custom store that
//! quietly violates one of those invariants can lose committed entries after
//! a restart without any of our own tests noticing.
//!
//! The suite needs a `StoreBuilder` that can create a *fresh* isolated pair on
//! every case. We give it a `TempDir` guard so each case starts with empty
//! RocksDB directories and the guard keeps them alive until the case ends.
//! Test code only: no production behaviour is changed by this file.

use batata_consistency::raft::log_store::RocksLogStore;
use batata_consistency::raft::state_machine::RocksStateMachine;
use batata_consistency::raft::types::TypeConfig;
use openraft::StorageError;
use openraft::testing::log::{StoreBuilder, Suite};
use openraft::type_config::TypeConfigExt;
use tempfile::TempDir;

struct RocksStoreBuilder;

impl StoreBuilder<TypeConfig, RocksLogStore, RocksStateMachine, TempDir> for RocksStoreBuilder {
    async fn build(
        &self,
    ) -> Result<(TempDir, RocksLogStore, RocksStateMachine), StorageError<TypeConfig>> {
        let dir = tempfile::tempdir()
            .map_err(|error| StorageError::read(TypeConfig::err_from_error(&error)))?;
        let logs = RocksLogStore::new(dir.path().join("raft-log"))
            .await
            .map_err(|error| StorageError::read(TypeConfig::err_from_error(&error)))?;
        // `RaftNode` always enables persistent snapshots at startup. Mirror
        // that wiring here: without a snapshot directory `build_snapshot()`
        // writes only a transient body, so the suite's transfer_snapshot case
        // sees no current snapshot after a build. A bare state machine would
        // test a configuration production never uses.
        let state_machine = RocksStateMachine::new(dir.path().join("state"))
            .await
            .map_err(|error| StorageError::read(TypeConfig::err_from_error(&error)))?
            .with_snapshot_dir(dir.path().join("snapshot"));
        Ok((dir, logs, state_machine))
    }
}

#[test]
fn conformance_builder_mirrors_production_snapshot_persistence() {
    let source = include_str!("openraft_storage_conformance.rs");
    // Split the token so this guard's own text cannot satisfy the check.
    let token = concat!(".with_snapshot_dir", "(dir.path().join(\"snapshot\"))");
    assert!(
        source.contains(token),
        "conformance builder must enable persistent snapshots: {token}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rocks_storage_passes_the_openraft_conformance_suite() {
    Suite::<TypeConfig, RocksLogStore, RocksStateMachine, RocksStoreBuilder, TempDir>::test_all(
        RocksStoreBuilder,
    )
    .await
    .expect("openraft storage conformance suite must pass");
}
